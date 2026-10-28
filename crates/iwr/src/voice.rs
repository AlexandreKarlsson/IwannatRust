//! `codecast voice`: record a `codecast/` directory with Kokoro TTS (open-weight, runs on the CPU).
//!
//! Each part file `NN-name.md` gets `NN-name.ogg` (Opus, ~18 kbps: small enough to commit) and
//! `NN-name.times` (the start time and text hash of every cue). The loader turns the times into the
//! part's `times:` line, so the player's recording mode follows the cues and the `.md` files stay as
//! written; a part whose text changed since is spoken by TTS until it is recorded again. Kokoro runs in a Python
//! sidecar (`kokoro-onnx`) set up on first use under the user's cache (`~/.cache/codecast/kokoro`, [`crate::python`]; `CODECAST_KOKORO` overrides).
//!
//! `--lang ja` (any codecast language Kokoro has voices for: ja, fr, es, it, pt) records the part
//! translated the way the player translates it ([`crate::translate`], same cache) into
//! `NN-name.ja.ogg` + `NN-name.ja.times`. The times file hashes the English cues, so the
//! recording goes stale when the English text changes. Japanese is phonemized by misaki
//! (installed in the venv on first use), since espeak cannot read it.
//!
//! A codecast with `sp:` lines is recorded by a pair of voices (`--voice2`, default
//! [`v::second_voice`]): `NN-name.af_heart+am_fenrir.ogg`, the second voice saying those lines.
//!
//! The UI's Generate page runs the same recording through `codecast serve` ([`record`] with a progress
//! callback and a cancel flag), shows every part's state ([`status`]) and previews voices ([`preview`]).

use crate::codecast;
use anyhow::{bail, Context, Result};
use iwr_core::voice::{self as v, PartStatus, Recording, VoiceStatus};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

const SIDECAR: &str = include_str!("kokoro_voice.py");
const MODEL_URL: &str = "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0";
const MODEL: &str = "kokoro-v1.0.int8.onnx";
const VOICES: &str = "voices-v1.0.bin";

/// One setup at a time: two jobs must not install into the same venv together.
static SETUP: Mutex<()> = Mutex::new(());

pub use iwr_core::voice::DEFAULT_VOICE;

pub struct Opts {
    /// a Kokoro voice, or a pair `af_heart+am_michael` whose second voice says the `sp:` lines
    pub voice: String,
    /// the voice of the `sp:` lines when `voice` is not a pair (default: [`v::second_voice`])
    pub voice2: Option<String>,
    pub speed: f32,
    /// Kokoro's language (`en-us`, `en-gb`), or a codecast language to translate into (`ja`)
    pub lang: String,
    /// the project, for the names kept as code when translating (default: the directory's parent)
    pub path: Option<PathBuf>,
    /// silence between cues, seconds
    pub gap: f32,
    /// Opus compression level, 0..1 (libsndfile; 0.95 ≈ 18 kbps)
    pub level: f32,
    /// re-record parts that look up to date
    pub force: bool,
    /// record only these part stems, each one even when up to date (empty: every part that needs it)
    pub only: Vec<String>,
    /// translate again instead of taking the cached translation
    pub retranslate: bool,
    /// synthesize every cue again instead of taking the cached clips
    pub fresh: bool,
}

/// What a recording reports while it works.
pub enum Event {
    /// a step: setting up, translating a part, loading the model, recording
    Stage(String),
    /// the parts about to be recorded, with their cue counts
    Plan(Vec<(String, usize)>),
    /// cue `done` (1-based) of part `part` (index in the plan) is synthesized
    Cue { part: usize, done: usize },
    /// a part's file is written
    Part { part: usize, duration: f64, kb: u64 },
    /// worth a line: a skipped part, the sidecar's own messages
    Log(String),
}

fn header(o: &Opts) -> String {
    format!("# codecast voice: kokoro {} speed {} lang {} gap {} opus {}", o.voice, o.speed, o.lang, o.gap, o.level)
}

/// `codecast voice`: record, telling progress on stderr.
pub fn run(dir: &Path, o: &Opts) -> Result<()> {
    let mut plan = Vec::new();
    record(dir, o, &AtomicBool::new(false), &mut |e| match e {
        Event::Stage(s) | Event::Log(s) => eprintln!("{}", s),
        Event::Plan(p) => plan = p,
        Event::Cue { part, done } => eprintln!("  {} cue {}/{}", plan[part].0, done, plan[part].1),
        Event::Part { part, duration, kb } => eprintln!("  {}.ogg  {} cue(s), {}:{:02}, {} KB", plan[part].0, plan[part].1, duration as u64 / 60, duration as u64 % 60, kb),
    })
}

/// Record the parts that need it (or `o.only`), reporting each step; stops when `cancel` is set.
pub fn record(dir: &Path, o: &Opts, cancel: &AtomicBool, report: &mut dyn FnMut(Event)) -> Result<()> {
    if !dir.is_dir() {
        bail!("codecast voice records a codecast/ directory (one audio file per part file); {} is not one", dir.display());
    }
    let project = o.path.clone().or_else(|| dir.canonicalize().ok()?.parent().map(Path::to_path_buf)).unwrap_or_else(|| PathBuf::from("."));
    // a codecast language: translate each part, record it with a voice of that language
    let target: Option<&str> = iwr_core::i18n::lang(&o.lang).map(|l| l.code).filter(|c| *c != "en");
    let loaded = codecast::load(dir)?;
    let script = iwr_core::script::parse(&loaded.text);
    let (first, second) = v::split_pair(&o.voice);
    let (first, lang) = match target {
        Some(code) => {
            let l = v::voice_lang(code).filter(|l| l.code != "en").with_context(|| format!("Kokoro has no {} voice (it speaks ja, fr, es, it, pt)", code))?;
            (if first == DEFAULT_VOICE { l.default_voice } else { first }, l.kokoro.to_string())
        }
        None => (first, o.lang.clone()),
    };
    // a codecast with `sp:` lines is recorded by a pair of voices, every part of it, so that the
    // player offers one recording to pick; parts without them share their clips with the single voice
    let voice = match script.has_second_voice() {
        true => v::pair(first, Some(second.or(o.voice2.as_deref()).unwrap_or_else(|| v::second_voice(first)))),
        false => first.to_string(),
    };
    let o = &Opts { voice, voice2: None, lang, path: None, only: o.only.clone(), ..*o };
    let names = match target {
        Some(_) => iwr_core::i18n::names(&iwr_core::analyze_path(&project)?),
        None => Default::default(),
    };
    let pron = codecast::pronounce(&script, loaded.glossary.as_deref());
    let mut todo = Vec::new();
    for f in part_files(dir)? {
        let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
        // every part, recorded now or not: the player lists recordings by voice
        migrate(dir, &stem, target.unwrap_or("en"), report)?;
        let asked = o.only.contains(&stem);
        if !o.only.is_empty() && !asked {
            continue;
        }
        let text = std::fs::read_to_string(&f)?;
        let (english, skip) = codecast::part_spoken(&text, &pron);
        if let Some(why) = skip {
            report(Event::Log(format!("skip {}: {}", stem, why)));
            continue;
        }
        // the hashes are of the English cues: a translation goes stale with its source
        let hashes = codecast::hashes(&english);
        let code = target.unwrap_or("en");
        // one recording per voice: `01-welcome.am_adam.ogg`, `01-welcome.ja.jf_alpha.ogg`
        let stem = codecast::variant_stem(&stem, code, &o.voice);
        let times_path = dir.join(format!("{}.times", stem));
        let own = std::fs::read_to_string(&times_path).ok();
        let out = dir.join(format!("{}.ogg", stem));
        let up_to_date = own.as_deref().is_some_and(|t| t.lines().next() == Some(header(o).as_str())) && codecast::read_times(&times_path).iter().map(|(_, h)| h).eq(hashes.iter()) && out.is_file();
        if up_to_date && !(o.force || o.retranslate || o.fresh || asked) {
            continue;
        }
        if cancel.load(Ordering::Relaxed) {
            bail!("cancelled");
        }
        let spoken = match target {
            Some(code) => {
                report(Event::Stage(format!("translating {} into {}", stem, code)));
                translated(&text, code, &names, &pron, o.retranslate, dir)?
            }
            None => english,
        };
        // the same voice's recording in another format goes once this one is made
        let other: Vec<PathBuf> = codecast::AUDIO_EXTS.iter().map(|e| dir.join(format!("{}.{}", stem, e))).filter(|f| f.is_file() && *f != out).collect();
        todo.push((stem, spoken, hashes, other));
    }
    if todo.is_empty() {
        report(Event::Log("all parts up to date".into()));
        return Ok(());
    }
    report(Event::Plan(todo.iter().map(|(stem, spoken, _, _)| (stem.clone(), spoken.len())).collect()));
    let home = setup(target == Some("ja"), report)?;
    if cancel.load(Ordering::Relaxed) {
        bail!("cancelled");
    }
    // other recordings of a part go once it is about to be recorded
    for f in todo.iter().flat_map(|t| &t.3) {
        std::fs::remove_file(f).with_context(|| format!("removing {}", f.display()))?;
    }
    let abs = dir.canonicalize()?;
    let parts: Vec<_> = todo.iter().map(|(stem, spoken, _, _)| part_job(abs.join(format!("{}.ogg", stem)), spoken)).collect();
    report(Event::Stage(format!("loading Kokoro, voice {}", o.voice)));
    let hdr = header(o);
    sidecar(&home, job(&home, o, parts), cancel, report, &mut |part, times, duration| {
        let (stem, _, hashes, _) = &todo[part];
        let mut text = format!("{}\n# duration {}\n", hdr, duration);
        for (t, h) in times.iter().zip(hashes) {
            text.push_str(&format!("{} {}\n", t, h));
        }
        std::fs::write(dir.join(format!("{}.times", stem)), text)?;
        Ok(std::fs::metadata(dir.join(format!("{}.ogg", stem))).map(|m| m.len() / 1024).unwrap_or(0))
    })
}

/// Give a recording `codecast voice` made under the older one-per-language name (`01-welcome.ogg`,
/// `01-welcome.ja.ogg`) its voice's name (`01-welcome.am_adam.ogg`), so that recording another
/// voice keeps it. Hand-made recordings (no `.times`) stay as they are.
fn migrate(dir: &Path, stem: &str, lang: &str, report: &mut dyn FnMut(Event)) -> Result<()> {
    let old = codecast::variant_stem(stem, lang, "");
    let Ok(times) = std::fs::read_to_string(dir.join(format!("{}.times", old))) else { return Ok(()) };
    let Some(voice) = codecast::header_voice(&times) else { return Ok(()) };
    let new = codecast::variant_stem(stem, lang, &voice);
    let Some(ext) = codecast::AUDIO_EXTS.iter().find(|e| dir.join(format!("{}.{}", old, e)).is_file()) else { return Ok(()) };
    if dir.join(format!("{}.times", new)).is_file() {
        return Ok(());
    }
    std::fs::rename(dir.join(format!("{}.{}", old, ext)), dir.join(format!("{}.{}", new, ext)))?;
    std::fs::rename(dir.join(format!("{}.times", old)), dir.join(format!("{}.times", new)))?;
    report(Event::Log(format!("renamed {}.{} to {}.{} (one file per voice)", old, ext, new, ext)));
    Ok(())
}

/// The `.md` part files of a codecast directory, in order (not `index.md`, `glossary.md`).
fn part_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?.filter_map(|e| e.ok().map(|e| e.path())).filter(|f| f.extension().and_then(|e| e.to_str()) == Some("md")).collect();
    files.retain(|f| !matches!(f.file_stem().and_then(|s| s.to_str()), Some("index" | "glossary")));
    files.sort();
    Ok(files)
}

/// A part of the sidecar's job: its `out` file, the spoken cues, which of them the second voice says.
fn part_job(out: PathBuf, spoken: &[codecast::Spoken]) -> serde_json::Value {
    let (cues, second): (Vec<&str>, Vec<bool>) = spoken.iter().map(|(t, s)| (t.as_str(), *s)).unzip();
    serde_json::json!({ "out": out, "cues": cues, "second": second })
}

/// The sidecar's job: model files, voices and delivery, the parts ([`part_job`]). A pair whose
/// two voices are the same speaks the second one higher ([`v::SECOND_PITCH`]).
fn job(home: &Path, o: &Opts, parts: Vec<serde_json::Value>) -> serde_json::Value {
    let (voice, voice2) = v::split_pair(&o.voice);
    let pitch2 = if voice2 == Some(voice) { v::SECOND_PITCH } else { 1.0 };
    serde_json::json!({
        "model": home.join(MODEL), "voices": home.join(VOICES), "voice": voice, "voice2": voice2.unwrap_or(voice), "pitch2": pitch2,
        "speed": o.speed, "lang": o.lang, "gap": o.gap, "level": o.level, "cache": home.join("clips"), "fresh": o.fresh, "parts": parts,
    })
}

/// Stores what goes with a written part (index, cue start times, duration); answers its size in KB.
type PartDone<'a> = dyn FnMut(usize, &[f64], f64) -> Result<u64> + 'a;

/// Run the Kokoro sidecar on a job, turning its `@` lines into events; `done(part, cue start
/// times, duration)` stores what goes with a written part and answers its size in KB. Killed when
/// `cancel` is set.
fn sidecar(home: &Path, job: serde_json::Value, cancel: &AtomicBool, report: &mut dyn FnMut(Event), done: &mut PartDone) -> Result<()> {
    let mut child = Command::new(crate::python::python(home)).arg(home.join("kokoro_voice.py")).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().context("starting the kokoro sidecar")?;
    child.stdin.take().context("sidecar stdin")?.write_all(job.to_string().as_bytes())?;
    let stderr = child.stderr.take().context("sidecar stderr")?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stderr).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    #[derive(serde::Deserialize)]
    struct Written {
        times: Vec<f64>,
        duration: f64,
    }
    let mut written = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            bail!("cancelled");
        }
        let line = match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(l) => l,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        };
        if line == "@ready" {
            report(Event::Stage("recording".into()));
        } else if let Some(rest) = line.strip_prefix("@cue ") {
            let mut w = rest.split_whitespace().filter_map(|x| x.parse::<usize>().ok());
            if let (Some(part), Some(n)) = (w.next(), w.next()) {
                report(Event::Cue { part, done: n });
            }
        } else if let Some(rest) = line.strip_prefix("@part ") {
            let w: Written = serde_json::from_str(rest).context("reading the sidecar's progress")?;
            let kb = done(written, &w.times, w.duration)?;
            report(Event::Part { part: written, duration: w.duration, kb });
            written += 1;
        } else if !line.trim().is_empty() {
            report(Event::Log(line));
        }
    }
    let st = child.wait()?;
    if !st.success() {
        bail!("kokoro sidecar failed ({})", st);
    }
    Ok(())
}

/// A part file's cues translated into `lang` (as the player translates them), as they will be spoken.
fn translated(text: &str, lang: &str, names: &std::collections::HashSet<String>, pron: &[(String, String)], fresh: bool, dir: &Path) -> Result<Vec<codecast::Spoken>> {
    let mut s = iwr_core::script::parse(text);
    let texts = iwr_core::i18n::texts(&s, &[], names);
    let done = crate::translate::translate(lang, &texts, fresh, Some(dir))?;
    let map: std::collections::HashMap<String, String> = texts.into_iter().zip(done).collect();
    iwr_core::i18n::apply(&mut s, &mut [], names, &|t| map.get(t).cloned());
    Ok(s.parts.iter().flat_map(|p| &p.cues).map(|c| (iwr_core::speech::spoken(&c.say, pron), c.second())).collect())
}

fn home() -> Result<PathBuf> {
    crate::python::home("CODECAST_KOKORO", "kokoro")
}

/// Is Kokoro set up (venv and model files), so that recording starts at once?
pub fn ready() -> bool {
    home().is_ok_and(|h| crate::python::python(&h).is_file() && h.join(MODEL).is_file() && h.join(VOICES).is_file())
}

/// The sidecar's directory, with a Python venv (`kokoro-onnx`, `soundfile`; `misaki[ja]` for
/// Japanese) and the model files.
fn setup(japanese: bool, report: &mut dyn FnMut(Event)) -> Result<PathBuf> {
    let _guard = SETUP.lock().unwrap_or_else(|e| e.into_inner());
    let home = home()?;
    std::fs::create_dir_all(&home)?;
    let mut run = |cmd: &mut Command, what: &str| -> Result<()> {
        report(Event::Stage(format!("{}…", what)));
        let st = cmd.status().with_context(|| what.to_string())?;
        if !st.success() {
            bail!("{} failed ({})", what, st);
        }
        Ok(())
    };
    crate::python::venv(&home, &["kokoro-onnx", "soundfile"], "import kokoro_onnx, soundfile", &mut run)?;
    if japanese {
        crate::python::venv(&home, &["misaki[ja]"], "from misaki import ja", &mut run)?;
    }
    for f in [MODEL, VOICES] {
        if !home.join(f).is_file() {
            let part = home.join(format!("{}.part", f));
            run(Command::new("curl").args(["-fSL", "--progress-bar", "-o"]).arg(&part).arg(format!("{}/{}", MODEL_URL, f)), &format!("downloading {}", f))?;
            std::fs::rename(&part, home.join(f))?;
        }
    }
    std::fs::write(home.join("kokoro_voice.py"), SIDECAR)?;
    Ok(home)
}

/// Every part of a codecast and its recordings, per language, for the Generate page.
pub fn status(script: Option<&Path>) -> VoiceStatus {
    let mut st = VoiceStatus { ready: ready(), ..Default::default() };
    let dir = match script {
        Some(d) if d.is_dir() => d,
        Some(f) => {
            st.problem = Some(format!("{} is a single file: recordings are made per part, for a codecast/ directory (index.md + one file per part)", f.display()));
            return st;
        }
        None => {
            st.problem = Some("no codecast next to the project: create codecast/ with index.md and one file per part".into());
            return st;
        }
    };
    st.dir = Some(dir.display().to_string());
    let Ok(loaded) = codecast::load(dir) else { return st };
    let script = iwr_core::script::parse(&loaded.text);
    st.second = script.has_second_voice();
    let pron = codecast::pronounce(&script, loaded.glossary.as_deref());
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir).map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).collect()).unwrap_or_default();
    entries.sort();
    for f in part_files(dir).unwrap_or_default() {
        let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
        let text = std::fs::read_to_string(&f).unwrap_or_default();
        let (spoken, skip) = codecast::part_spoken(&text, &pron);
        let hashes = codecast::hashes(&spoken);
        let name = iwr_core::script::parse(&text).parts.first().map(|p| p.name.clone()).unwrap_or_default();
        let mut p = PartStatus { stem: stem.clone(), name, cues: spoken.len(), skip: skip.map(str::to_string), ..Default::default() };
        // the part's own `01-welcome.ogg`, then one file per language and voice
        let mut variants = vec![("en".to_string(), String::new())];
        variants.extend(codecast::variants(&entries, &stem));
        for (lang, voice) in variants {
            let vstem = codecast::variant_stem(&stem, &lang, &voice);
            let Some(audio) = codecast::AUDIO_EXTS.iter().map(|e| format!("{}.{}", vstem, e)).find(|f| dir.join(f).is_file()) else { continue };
            let kb = std::fs::metadata(dir.join(&audio)).map(|m| m.len() / 1024).unwrap_or(0);
            let times_path = dir.join(format!("{}.times", vstem));
            let Ok(times_text) = std::fs::read_to_string(&times_path) else {
                p.recordings.push(Recording { lang, state: "own".into(), file: audio, kb, ..Default::default() });
                continue;
            };
            // `# codecast voice: kokoro VOICE speed SPEED …`, `# duration SECONDS`
            let words: Vec<&str> = times_text.lines().next().unwrap_or_default().split_whitespace().collect();
            let after = |k: &str| words.iter().position(|w| *w == k).and_then(|i| words.get(i + 1)).map(|s| s.to_string());
            let duration = times_text.lines().find_map(|l| l.strip_prefix("# duration ")).and_then(|d| d.trim().parse().ok());
            let fresh = codecast::read_times(&times_path).iter().map(|(_, h)| h).eq(hashes.iter());
            let voice = Some(voice).filter(|v| !v.is_empty()).or_else(|| after("kokoro"));
            if !fresh {
                if let Some(vc) = &voice {
                    let arg = if lang == "en" { after("lang").unwrap_or_else(|| "en-us".into()) } else { lang.clone() };
                    let num = |k: &str, d: f32| after(k).and_then(|s| s.parse().ok()).unwrap_or(d);
                    let g = match st.stale.iter_mut().position(|g| g.lang == lang && g.voice == *vc) {
                        Some(i) => &mut st.stale[i],
                        None => {
                            st.stale.push(v::StaleGroup { lang: lang.clone(), voice: vc.clone(), arg, speed: num("speed", 1.0), gap: num("gap", 0.4), level: num("opus", 0.95), ..Default::default() });
                            st.stale.last_mut().unwrap()
                        }
                    };
                    g.parts.push(stem.clone());
                    g.files.push(audio.clone());
                    g.files.push(format!("{}.times", vstem));
                }
            }
            p.recordings.push(Recording { lang, state: if fresh { "recorded" } else { "stale" }.into(), voice, file: audio, speed: after("speed").and_then(|s| s.parse().ok()), duration, kb });
        }
        st.parts.push(p);
    }
    st.orphans = orphans(dir, &entries, &st.parts.iter().map(|p| p.stem.clone()).collect::<Vec<_>>());
    st
}

/// Files `codecast voice` made (a `.times` with its header, and the recording beside it) for part
/// stems that no part file has any more.
fn orphans(dir: &Path, entries: &[PathBuf], stems: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for f in entries.iter().filter(|f| f.extension().and_then(|e| e.to_str()) == Some("times")) {
        let Some(fstem) = f.file_stem().and_then(|s| s.to_str()) else { continue };
        let owned = stems.iter().any(|s| fstem == s || codecast::parse_variant(fstem, s).is_some());
        if owned || std::fs::read_to_string(f).ok().and_then(|t| codecast::header_voice(&t)).is_none() {
            continue;
        }
        out.push(format!("{}.times", fstem));
        out.extend(codecast::AUDIO_EXTS.iter().map(|e| format!("{}.{}", fstem, e)).filter(|n| dir.join(n).is_file()));
    }
    out
}

/// Delete files of the codecast directory by name (names from [`status`] only).
pub fn delete(dir: &Path, names: &[String]) -> Result<usize> {
    let mut n = 0;
    for name in names {
        if name.contains('/') || name.contains("..") {
            continue;
        }
        let f = dir.join(name);
        if f.is_file() {
            std::fs::remove_file(&f).with_context(|| format!("removing {}", f.display()))?;
            n += 1;
        }
    }
    Ok(n)
}

/// A short sample of a voice (Ogg Opus bytes), cached under the Kokoro directory.
pub fn preview(voice: &str, speed: f32, lang: &str) -> Result<Vec<u8>> {
    let l = v::voice_lang(lang).context("unknown language")?;
    if !v::VOICES.contains(&voice) {
        bail!("unknown voice {:?}", voice);
    }
    let home = setup(l.kokoro == "ja", &mut |_| {})?;
    let out = home.join("previews").join(format!("{}-{}-{}.ogg", voice, speed, l.arg));
    if !out.is_file() {
        std::fs::create_dir_all(out.parent().unwrap())?;
        let o = Opts { voice: voice.into(), voice2: None, speed, lang: l.kokoro.into(), path: None, gap: 0.2, level: 0.9, force: false, only: vec![], retranslate: false, fresh: false };
        let parts = vec![part_job(out.clone(), &[(l.sample.to_string(), false)])];
        sidecar(&home, job(&home, &o, parts), &AtomicBool::new(false), &mut |_| {}, &mut |_, _, _| Ok(0))?;
    }
    Ok(std::fs::read(&out)?)
}
