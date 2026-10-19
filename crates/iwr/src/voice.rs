//! `iwr voice`: record a `codecast/` directory with Kokoro TTS (open-weight, runs on the CPU).
//!
//! Each part file `NN-name.md` gets `NN-name.ogg` (Opus, ~18 kbps: small enough to commit) and
//! `NN-name.times` (the start time and text hash of every cue). The loader turns the times into the
//! part's `times:` line, so the player's recording mode follows the cues and the `.md` files stay as
//! written; a part whose text changed since is spoken by TTS until it is recorded again. Kokoro runs in a Python
//! sidecar (`kokoro-onnx`) set up on first use under `~/.cache/iwr/kokoro` (`IWR_KOKORO` overrides).

use crate::codecast;
use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SIDECAR: &str = include_str!("kokoro_voice.py");
const MODEL_URL: &str = "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0";
const MODEL: &str = "kokoro-v1.0.int8.onnx";
const VOICES: &str = "voices-v1.0.bin";

pub struct Opts {
    pub voice: String,
    pub speed: f32,
    pub lang: String,
    /// silence between cues, seconds
    pub gap: f32,
    /// Opus compression level, 0..1 (libsndfile; 0.95 ≈ 18 kbps)
    pub level: f32,
    /// re-record parts that look up to date, and replace recordings `iwr voice` did not make
    pub force: bool,
}

fn header(o: &Opts) -> String {
    format!("# iwr voice: kokoro {} speed {} lang {} gap {} opus {}", o.voice, o.speed, o.lang, o.gap, o.level)
}

pub fn run(dir: &Path, o: &Opts) -> Result<()> {
    if !dir.is_dir() {
        bail!("iwr voice records a codecast/ directory (one audio file per part file); {} is not one", dir.display());
    }
    let loaded = codecast::load(dir)?;
    let pron = codecast::pronounce(&iwr_core::script::parse(&loaded.text), loaded.glossary.as_deref());
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?.filter_map(|e| e.ok().map(|e| e.path())).filter(|f| f.extension().and_then(|e| e.to_str()) == Some("md")).collect();
    files.sort();
    let mut todo = Vec::new();
    for f in files {
        let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
        if stem == "index" || stem == "glossary" {
            continue;
        }
        let (spoken, skip) = codecast::part_spoken(&std::fs::read_to_string(&f)?, &pron);
        if let Some(why) = skip {
            eprintln!("skip {}: {}", stem, why);
            continue;
        }
        let times_path = dir.join(format!("{}.times", stem));
        let own = std::fs::read_to_string(&times_path).ok();
        let out = dir.join(format!("{}.ogg", stem));
        // recordings with the part's stem: made by `iwr voice` when a `.times` file is there
        let audio: Vec<PathBuf> = codecast::AUDIO_EXTS.iter().map(|e| dir.join(format!("{}.{}", stem, e))).filter(|f| f.is_file()).collect();
        if let (Some(f), None, false) = (audio.first(), &own, o.force) {
            eprintln!("skip {}: has its own recording {} (--force replaces it)", stem, f.display());
            continue;
        }
        let hashes: Vec<String> = spoken.iter().map(|s| codecast::text_hash(s)).collect();
        let up_to_date = own.as_deref().is_some_and(|t| t.lines().next() == Some(header(o).as_str())) && codecast::read_times(&times_path).iter().map(|(_, h)| h).eq(hashes.iter()) && audio == [out.clone()];
        if up_to_date && !o.force {
            continue;
        }
        for f in audio.iter().filter(|f| **f != out) {
            std::fs::remove_file(f).with_context(|| format!("removing {}", f.display()))?;
        }
        todo.push((stem, spoken, hashes));
    }
    if todo.is_empty() {
        eprintln!("all parts up to date");
        return Ok(());
    }
    let home = setup()?;
    let abs = dir.canonicalize()?;
    let parts: Vec<_> = todo.iter().map(|(stem, spoken, _)| serde_json::json!({ "out": abs.join(format!("{}.ogg", stem)), "cues": spoken })).collect();
    let job = serde_json::json!({
        "model": home.join(MODEL), "voices": home.join(VOICES), "voice": o.voice, "speed": o.speed, "lang": o.lang,
        "gap": o.gap, "level": o.level, "cache": home.join("clips"), "parts": parts,
    });
    eprintln!("recording {} part(s) with kokoro voice {}…", todo.len(), o.voice);
    let mut child = Command::new(home.join("venv/bin/python")).arg(home.join("kokoro_voice.py")).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().context("starting the kokoro sidecar")?;
    child.stdin.take().context("sidecar stdin")?.write_all(job.to_string().as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        bail!("kokoro sidecar failed ({})", out.status);
    }
    #[derive(serde::Deserialize)]
    struct Done {
        times: Vec<f64>,
        duration: f64,
    }
    let done: Vec<Done> = serde_json::from_slice(&out.stdout).context("reading the sidecar's result")?;
    for ((stem, spoken, hashes), d) in todo.iter().zip(done) {
        let mut text = header(o) + "\n";
        for (t, h) in d.times.iter().zip(hashes) {
            text.push_str(&format!("{} {}\n", t, h));
        }
        std::fs::write(dir.join(format!("{}.times", stem)), text)?;
        let kb = std::fs::metadata(dir.join(format!("{}.ogg", stem))).map(|m| m.len() / 1024).unwrap_or(0);
        eprintln!("  {}.ogg  {} cue(s), {}:{:02}, {} KB", stem, spoken.len(), d.duration as u64 / 60, d.duration as u64 % 60, kb);
    }
    Ok(())
}

/// The sidecar's directory, with a Python venv (`kokoro-onnx`, `soundfile`) and the model files.
fn setup() -> Result<PathBuf> {
    let home = match std::env::var_os("IWR_KOKORO") {
        Some(h) => PathBuf::from(h),
        None => std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache"))).context("no HOME")?.join("iwr/kokoro"),
    };
    std::fs::create_dir_all(&home)?;
    let run = |cmd: &mut Command, what: &str| -> Result<()> {
        eprintln!("{}…", what);
        let st = cmd.status().with_context(|| what.to_string())?;
        if !st.success() {
            bail!("{} failed ({})", what, st);
        }
        Ok(())
    };
    if !home.join("venv/bin/python").is_file() {
        run(Command::new("python3").args(["-m", "venv"]).arg(home.join("venv")), "creating a Python venv for kokoro")?;
    }
    if !Command::new(home.join("venv/bin/python")).args(["-c", "import kokoro_onnx, soundfile"]).stderr(Stdio::null()).status()?.success() {
        run(Command::new(home.join("venv/bin/pip")).args(["install", "-q", "kokoro-onnx", "soundfile"]), "installing kokoro-onnx")?;
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
