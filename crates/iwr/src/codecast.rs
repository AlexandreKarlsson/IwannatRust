//! Finding and reading a codecast on disk: a single file (`codecast.md`, legacy `codecast.txt`)
//! or a `codecast/` directory with `index.md` and one file per part (`01-welcome.md`, …).
//! A sub-folder of that directory with the same files inside is a topic: a codecast of its own
//! about the same code (`codecast/error_handling/`), which borrows the directory's
//! `codecast.toml`, fonts and `glossary.md` unless it has its own.

use anyhow::{Context, Result};
use iwr_core::config::{self, Config};
use iwr_core::script::{self, PartFile};
use std::path::{Path, PathBuf};

pub const AUDIO_EXTS: [&str; 5] = ["mp3", "ogg", "wav", "m4a", "webm"];

/// A codecast read from disk.
pub struct Loaded {
    /// the script text (a directory is assembled into one text)
    pub text: String,
    /// where `audio:` paths resolve
    pub dir: PathBuf,
    /// the project's own glossary (`glossary.md` next to the script), if any
    pub glossary: Option<String>,
    /// part files whose `codecast voice` recording was left out because their text changed since
    /// (`01-welcome`, or `01-welcome.ja` for a recording in another language)
    pub stale: Vec<String>,
}

/// Where a script's relative names (`audio:`, fonts, glossary) resolve: the `codecast/`
/// directory itself, or the directory of a single-file script.
pub fn script_dir(script: &Path) -> PathBuf {
    if script.is_dir() { script.to_path_buf() } else { script.parent().map(Path::to_path_buf).unwrap_or_default() }
}

/// The project glossary file next to a script: `codecast/glossary.md` for a directory,
/// `glossary.md` beside a single-file script. A topic without one has its codecast's.
pub fn glossary_path(script: &Path) -> PathBuf {
    asset_path(script, "glossary.md")
}

/// Does `dir` hold a codecast's files: an `index.md` or part files?
fn is_cast_dir(dir: &Path) -> bool {
    let Ok(rd) = std::fs::read_dir(dir) else { return false };
    rd.flatten().any(|e| {
        let p = e.path();
        p.is_file() && p.extension().and_then(|x| x.to_str()) == Some("md") && p.file_stem().and_then(|x| x.to_str()) != Some("glossary")
    })
}

/// The `codecast/` directory a topic belongs to: the folder above a script directory, when it
/// is one (named `codecast`, or with an `index.md` or a `codecast.toml`). `None` for a
/// codecast that is not a topic.
pub fn parent_cast(script: &Path) -> Option<PathBuf> {
    if !script.is_dir() {
        return None;
    }
    let dir = script.canonicalize().ok()?;
    let up = dir.parent()?;
    let is_cast = up.file_name().is_some_and(|n| n == "codecast") || up.join("index.md").is_file() || up.join(config::FILE).is_file();
    is_cast.then(|| up.to_path_buf())
}

/// Where a file of a codecast is read from (`name` relative to its directory): a topic that
/// does not have it takes its codecast's.
pub fn asset_path(script: &Path, name: &str) -> PathBuf {
    let own = script_dir(script).join(name);
    if own.exists() {
        return own;
    }
    match parent_cast(script).map(|up| up.join(name)) {
        Some(shared) if shared.exists() => shared,
        _ => own,
    }
}

/// The directory `codecast.toml` and its fonts are in, and are saved to: a topic without a
/// `codecast.toml` of its own shares its codecast's look.
pub fn config_dir(script: &Path) -> PathBuf {
    let own = script_dir(script);
    if own.join(config::FILE).is_file() {
        return own;
    }
    parent_cast(script).unwrap_or(own)
}

/// The project a script explains when `--path` is not given: the folder its `codecast/`
/// directory (or file) is in; for a topic, the one above that.
pub fn default_project(script: &Path) -> PathBuf {
    let abs = script.canonicalize().unwrap_or_else(|_| script.to_path_buf());
    let cast = parent_cast(&abs).unwrap_or(abs);
    cast.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
}

/// The `index.md` of a codecast directory (empty without one). A topic whose index has no
/// `# Title` is titled after its folder (`02-error_handling` → `error handling`).
fn index_text(dir: &Path) -> String {
    let index = std::fs::read_to_string(dir.join("index.md")).unwrap_or_default();
    if index.lines().any(|l| l.starts_with("# ")) || parent_cast(dir).is_none() {
        return index;
    }
    let name = dir.canonicalize().ok().and_then(|d| Some(d.file_name()?.to_string_lossy().to_string())).unwrap_or_default();
    format!("# {}\n\n{}", iwr_core::casts::topic_label(&name), index)
}

/// Is there a general codecast at `script`: a single file, or a directory with an `index.md`
/// or part files of its own (not only topics)?
pub fn has_general(script: &Path) -> bool {
    script.is_file() || is_cast_dir(script)
}

/// The topics of a `codecast/` directory: its sub-folders that hold a codecast's files, in
/// name order (`01-…`, `02-…` to order them). Hidden folders are skipped; `translations/` and
/// `fonts/` hold no script, so they are not topics.
pub fn topics(script: &Path) -> Vec<iwr_core::casts::Topic> {
    let Ok(rd) = std::fs::read_dir(script) else { return vec![] };
    let mut dirs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir() && is_cast_dir(p)).collect();
    dirs.sort();
    dirs.iter()
        .filter_map(|d| {
            let dir = d.file_name()?.to_str()?.to_string();
            if dir.starts_with('.') {
                return None;
            }
            let info = info(d)?;
            // the title written in the topic's `index.md`, else the folder name (see `index_text`)
            let label = if info.title.trim().is_empty() { iwr_core::casts::topic_label(&dir) } else { info.title.clone() };
            Some(iwr_core::casts::Topic { dir, label, info })
        })
        .collect()
}

/// What `GET /api/topics` answers for the codecast at `script`, `current` being the topic
/// that plays (empty: the general codecast).
pub fn topic_list(script: &Path, current: &str) -> iwr_core::casts::Topics {
    let general = has_general(script);
    let title = if general { info(script).map(|i| i.title).unwrap_or_default() } else { String::new() };
    iwr_core::casts::Topics { general, title, current: current.to_string(), topics: topics(script) }
}

/// The codecast's look and defaults (`codecast.toml` next to the script; a topic without one
/// has its codecast's), if it has one.
pub fn read_config(script: &Path) -> Result<Option<Config>> {
    let path = config_dir(script).join(config::FILE);
    let Ok(text) = std::fs::read_to_string(&path) else { return Ok(None) };
    Config::from_toml(&text).map(Some).map_err(|e| anyhow::anyhow!("{}: {}", path.display(), e))
}

/// Problems in `codecast.toml`: bad values, font files that are not there.
pub fn check_config(script: &Path) -> Vec<String> {
    match read_config(script) {
        Ok(Some(c)) => {
            let dir = config_dir(script);
            let mut bad: Vec<String> = c.problems().into_iter().map(|b| format!("{}: {}", config::FILE, b)).collect();
            for f in c.font_files() {
                if !dir.join(&f).is_file() {
                    bad.push(format!("{}: font file not found: {}", config::FILE, dir.join(&f).display()));
                }
            }
            bad
        }
        Ok(None) => vec![],
        Err(e) => vec![e.to_string()],
    }
}

/// The codecast shipped with a project, if any: `codecast/` dir, `codecast.md`, then `codecast.txt`.
pub fn locate(project: &Path) -> Option<PathBuf> {
    let base = if project.is_file() { project.parent()?.to_path_buf() } else { project.to_path_buf() };
    for name in ["codecast", "codecast.md", "codecast.txt"] {
        let p = base.join(name);
        if (name == "codecast" && p.is_dir()) || (name != "codecast" && p.is_file()) {
            return Some(p);
        }
    }
    None
}

/// The folder a served path stands for: itself, or the folder of a single file.
pub fn project_dir(project: &Path) -> PathBuf {
    let p = project.canonicalize().unwrap_or_else(|_| project.to_path_buf());
    if p.is_file() {
        p.parent().map(Path::to_path_buf).unwrap_or(p)
    } else {
        p
    }
}

/// Where the cast map starts: the git root above the project, else the project's own folder.
pub fn map_top(project: &Path) -> PathBuf {
    let dir = project_dir(project);
    dir.ancestors().find(|a| a.join(".git").exists()).map(Path::to_path_buf).unwrap_or(dir)
}

/// What the cast map shows about a codecast: read from the script alone (no analysis, nothing
/// renamed or written), `None` when it cannot be read.
pub fn info(script_path: &Path) -> Option<iwr_core::casts::CastInfo> {
    let mut langs: Vec<String> = Vec::new();
    let text = if script_path.is_dir() {
        let index = index_text(script_path);
        let mut entries: Vec<PathBuf> = std::fs::read_dir(script_path).ok()?.filter_map(|e| e.ok().map(|e| e.path())).collect();
        entries.sort();
        let mut parts = Vec::new();
        for f in entries.iter().filter(|f| f.extension().and_then(|e| e.to_str()) == Some("md")) {
            let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
            if stem == "index" || stem == "glossary" {
                continue;
            }
            // a recording named after the part alone is English
            if AUDIO_EXTS.iter().any(|e| script_path.join(format!("{stem}.{e}")).is_file()) {
                langs.push("en".into());
            }
            langs.extend(variants(&entries, &stem).into_iter().map(|(lang, _)| lang));
            parts.push(PartFile { text: std::fs::read_to_string(f).ok()?, stem, audio: None, times: vec![], voices: vec![] });
        }
        script::assemble(&index, &parts)
    } else {
        std::fs::read_to_string(script_path).ok()?
    };
    langs.sort();
    langs.dedup();
    let sc = script::parse(&text);
    Some(iwr_core::casts::CastInfo { title: sc.title.clone(), parts: sc.parts.len(), cues: sc.cue_count(), secs: sc.duration(false, 1.0).0, langs })
}

/// Folders a scan for codecasts does not enter: hidden ones, build output, dependencies, and a
/// codecast's own directory.
fn skip_dir(name: &str) -> bool {
    name.starts_with('.') || matches!(name, "target" | "node_modules" | "codecast")
}

/// Is there Rust or C source somewhere under `dir`? Gives up (no) after a few thousand entries.
fn has_source(dir: &Path, seen: &mut usize) -> bool {
    let Ok(rd) = std::fs::read_dir(dir) else { return false };
    let mut dirs = Vec::new();
    for e in rd.flatten() {
        *seen += 1;
        if *seen > 5000 {
            return false;
        }
        let p = e.path();
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            if !skip_dir(&e.file_name().to_string_lossy()) {
                dirs.push(p);
            }
        } else if p.extension().and_then(|x| x.to_str()).is_some_and(|x| matches!(x, "rs" | "c" | "h")) {
            return true;
        }
    }
    dirs.iter().any(|d| has_source(d, seen))
}

/// Most folders a scan for codecasts visits before it gives up (a huge checkout).
const MAX_SCAN: usize = 50_000;

/// The codecasts around `project`: every folder under the git root (see [`map_top`]) that has
/// one and sources to show, as a tree. Build output, dependencies, hidden folders and the codecast folders
/// themselves are not entered; links are not followed. The topics of each codecast are nodes
/// under its folder; `topic` is the one that plays in the served folder (empty: none).
pub fn discover(project: &Path, topic: &str) -> iwr_core::casts::CastMap {
    let top = map_top(project);
    let current = project_dir(project);
    let rel = |p: &Path| p.strip_prefix(&top).unwrap_or(Path::new("")).components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("/");
    let mut found = Vec::new();
    let mut with_topics = Vec::new();
    let mut stack = vec![top.clone()];
    let mut seen = 0usize;
    let mut partial = false;
    while let Some(dir) = stack.pop() {
        seen += 1;
        if seen > MAX_SCAN {
            partial = true;
            break;
        }
        // a `codecast.md` in a folder without sources is a document, not a codecast to play
        if let Some(s) = locate(&dir).filter(|_| has_source(&dir, &mut 0)) {
            if let Some(i) = info(&s).filter(|_| has_general(&s)) {
                found.push((rel(&dir), Some(i)));
            }
            let t = topics(&s);
            if !t.is_empty() {
                with_topics.push((rel(&dir), t));
            }
        }
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !skip_dir(&name) && e.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push(e.path());
            }
        }
    }
    let name = top.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| top.display().to_string());
    let mut map = iwr_core::casts::CastMap::build_with_topics(&top.display().to_string(), &name, found, with_topics, &rel(&current), topic);
    map.partial = partial;
    map
}

/// Read a script file or directory.
pub fn load(path: &Path) -> Result<Loaded> {
    if path.is_dir() {
        for (old, new) in migrate_names(path) {
            eprintln!("renamed {old} → {new}");
        }
        let index = index_text(path);
        let mut parts: Vec<PartFile> = Vec::new();
        let mut entries: Vec<PathBuf> = std::fs::read_dir(path).with_context(|| format!("reading {}", path.display()))?.filter_map(|e| e.ok().map(|e| e.path())).collect();
        entries.sort();
        for f in &entries {
            if f.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
            if stem == "index" || stem == "glossary" {
                continue;
            }
            let text = std::fs::read_to_string(f).with_context(|| format!("reading {}", f.display()))?;
            let audio = AUDIO_EXTS.iter().map(|e| format!("{}.{}", stem, e)).find(|n| path.join(n).is_file());
            parts.push(PartFile { stem, text, audio, times: vec![], voices: vec![] });
        }
        let glossary = std::fs::read_to_string(glossary_path(path)).ok();
        // a generated recording plays only while its cues still say what was recorded; else the
        // part is spoken by TTS until `codecast voice` runs again
        let pron = pronounce(&script::parse(&script::assemble(&index, &parts)), glossary.as_deref());
        let mut stale = Vec::new();
        for pf in parts.iter_mut() {
            let hashes = hashes(&part_spoken(&pf.text, &pron).0);
            let times = read_times(&path.join(format!("{}.times", pf.stem)));
            if !times.is_empty() && pf.audio.is_some() {
                if hashes.iter().eq(times.iter().map(|(_, h)| h)) {
                    pf.times = times.iter().map(|(t, _)| *t).collect();
                    // made by `codecast voice` before recordings were named by voice: listed under its voice
                    let made_by = std::fs::read_to_string(path.join(format!("{}.times", pf.stem))).ok().and_then(|t| header_voice(&t));
                    if let (Some(voice), Some(audio)) = (made_by, pf.audio.clone()) {
                        pf.voices.push(script::Voiced { lang: "en".into(), voice, audio, times: pf.times.clone() });
                    }
                } else {
                    pf.audio = None;
                    stale.push(pf.stem.clone());
                }
            }
            // `01-welcome_en_am-adam.ogg` (one voice), `01-welcome_ja.ogg` / `01-welcome_ja_jf-alpha.ogg`
            // (Japanese) + their `.times`: each current while the English text it was made from is
            // (the hashes are of the English cues)
            for (lang, voice) in variants(&entries, &pf.stem) {
                let stem = variant_stem(&pf.stem, &lang, &voice);
                let Some(audio) = AUDIO_EXTS.iter().map(|e| format!("{}.{}", stem, e)).find(|n| path.join(n).is_file()) else { continue };
                let times = read_times(&path.join(format!("{}.times", stem)));
                if !times.is_empty() && hashes.iter().eq(times.iter().map(|(_, h)| h)) {
                    pf.voices.push(script::Voiced { lang, voice, audio, times: times.into_iter().map(|(t, _)| t).collect() });
                } else {
                    stale.push(stem);
                }
            }
        }
        Ok(Loaded { text: script::assemble(&index, &parts), dir: path.to_path_buf(), glossary, stale })
    } else {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading script {}", path.display()))?;
        Ok(Loaded { text, dir: path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from(".")), glossary: std::fs::read_to_string(glossary_path(path)).ok(), stale: vec![] })
    }
}

/// The file stem of a part's recording in `lang` by `voice`: `01-welcome_en_am-michael`,
/// `01-welcome_ja_jf-alpha` (the voice's `_` written `-`, so `_` only separates the fields);
/// without a voice, `01-welcome_ja`, and the part's own `01-welcome` for English.
pub fn variant_stem(stem: &str, lang: &str, voice: &str) -> String {
    match (lang, voice) {
        ("en", "") => stem.to_string(),
        (l, "") => format!("{stem}_{l}"),
        (l, v) => format!("{stem}_{l}_{}", v.replace('_', "-")),
    }
}

/// The name of a part's video in `lang`: `01-welcome_ja.mp4`, with subtitles `01-welcome_ja_sub-en.mp4`.
pub fn video_name(stem: &str, lang: &str, sub: &str) -> String {
    if sub.is_empty() { format!("{stem}_{lang}.mp4") } else { format!("{stem}_{lang}_sub-{sub}.mp4") }
}

/// (language, voice) of a recording's file stem, for the part `stem`: `01-welcome_en_am-michael` →
/// (`en`, `am_michael`), `01-welcome_ja` → (`ja`, ``), `01-welcome_ja_jf-alpha` → (`ja`, `jf_alpha`).
pub fn parse_variant(file_stem: &str, stem: &str) -> Option<(String, String)> {
    let rest = file_stem.strip_prefix(stem)?.strip_prefix('_')?;
    let (l, v) = match rest.split_once('_') {
        Some((l, v)) => (l, v.replace('-', "_")),
        None => (rest, String::new()),
    };
    let lang_ok = l == "en" || iwr_core::i18n::lang(l).is_some();
    let voice_ok = if v.is_empty() { l != "en" } else { iwr_core::voice::is_voice_id(&v) };
    (lang_ok && voice_ok).then(|| (l.to_string(), v))
}

/// The older dotted names (`01-welcome.am_adam`, `01-welcome.ja`, `01-welcome.ja.jf_alpha`):
/// (language, voice), only to rename them.
fn parse_dotted_variant(file_stem: &str, stem: &str) -> Option<(String, String)> {
    let rest = file_stem.strip_prefix(stem)?.strip_prefix('.')?;
    let is_lang = |l: &str| l != "en" && iwr_core::i18n::lang(l).is_some();
    match rest.split_once('.') {
        None if is_lang(rest) => Some((rest.into(), String::new())),
        None if iwr_core::voice::is_voice_id(rest) => Some(("en".into(), rest.into())),
        Some((l, v)) if is_lang(l) && iwr_core::voice::is_voice_id(v) => Some((l.into(), v.into())),
        _ => None,
    }
}

/// Rename the recordings and videos of a codecast directory from the older dotted names
/// (`01-welcome.ja.jf_alpha.ogg`, `.times`, `.mp4`, `.sub-en.mp4`) to the current ones
/// (`01-welcome_ja_jf-alpha.ogg`, `01-welcome_ja.mp4`); returns what moved. A file whose new name
/// is taken, or that cannot be renamed (a read-only copy), stays.
pub fn migrate_names(dir: &Path) -> Vec<(String, String)> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let names: Vec<String> = rd.filter_map(|e| e.ok()?.file_name().into_string().ok()).collect();
    let stems: Vec<&str> = names.iter().filter_map(|n| n.strip_suffix(".md")).filter(|s| !matches!(*s, "index" | "glossary")).collect();
    let mut moved = vec![];
    for name in &names {
        let Some((base, ext)) = name.rsplit_once('.') else { continue };
        let new = if ext == "mp4" {
            // `<recording stem>[.sub-xx].mp4`, not a half-filmed `.part.mp4`
            let (rec, sub) = match base.rsplit_once(".sub-") { Some((r, s)) => (r, s), None => (base, "") };
            if rec.ends_with(".part") {
                continue;
            }
            stems.iter().filter(|s| rec.starts_with(**s)).max_by_key(|s| s.len()).and_then(|s| {
                let (lang, _) = if rec == *s { ("en".to_string(), String::new()) } else { parse_dotted_variant(rec, s)? };
                Some(video_name(s, &lang, sub))
            })
        } else if ext == "times" || AUDIO_EXTS.contains(&ext) {
            stems.iter().filter(|s| base.starts_with(**s)).max_by_key(|s| s.len()).and_then(|s| {
                let (lang, voice) = parse_dotted_variant(base, s)?;
                Some(format!("{}.{ext}", variant_stem(s, &lang, &voice)))
            })
        } else {
            None
        };
        let Some(new) = new else { continue };
        if new == *name || dir.join(&new).exists() {
            continue;
        }
        if std::fs::rename(dir.join(name), dir.join(&new)).is_ok() {
            moved.push((name.clone(), new));
        }
    }
    moved
}

/// The recordings of part `stem` other than its own `01-welcome.ogg`, from a directory listing:
/// (language, voice), each once, in name order.
pub fn variants(entries: &[PathBuf], stem: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = entries
        .iter()
        .filter(|f| f.extension().and_then(|e| e.to_str()).is_some_and(|e| e == "times" || AUDIO_EXTS.contains(&e)))
        .filter_map(|f| parse_variant(f.file_stem()?.to_str()?, stem))
        .collect();
    out.dedup();
    out
}

/// The voice a `.times` file written by `codecast voice` was recorded with (its header line).
pub fn header_voice(times: &str) -> Option<String> {
    let words: Vec<&str> = times.lines().next()?.strip_prefix("# codecast voice: ")?.split_whitespace().collect();
    words.iter().position(|w| *w == "kokoro").and_then(|i| words.get(i + 1)).map(|s| s.to_string())
}

/// How to record a stale stem again: ` --voice am_adam`, ` --lang ja`, ` --lang ja --voice jf_alpha`.
pub fn lang_arg(stem: &str) -> String {
    // `<part>_<lang>_<voice>` or `<part>_<lang>`: the part may hold `_` itself, so read from the right
    let found = stem.rsplitn(3, '_').collect::<Vec<_>>();
    let pick = match found.as_slice() {
        [_, _, part] => parse_variant(stem, part).or_else(|| stem.rsplit_once('_').and_then(|(p, _)| parse_variant(stem, p))),
        [_, part] => parse_variant(stem, part),
        _ => None,
    };
    match pick {
        Some((l, v)) => {
            let mut a = String::new();
            if l != "en" {
                a.push_str(&format!(" --lang {}", l));
            }
            if !v.is_empty() {
                a.push_str(&format!(" --voice {}", v));
            }
            a
        }
        None => String::new(),
    }
}

/// A `.times` file written by `codecast voice`: one `start-seconds text-hash` line per cue, `#` comments.
pub fn read_times(path: &Path) -> Vec<(f64, String)> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            Some((w.next()?.parse().ok()?, w.next().unwrap_or_default().to_string()))
        })
        .collect()
}

/// A cue as it will be spoken, and whether the second voice says it (`sp:`).
pub type Spoken = (String, bool);

/// A part file's cues as they will be spoken, and why it cannot be recorded by `codecast voice`, if so
/// (generated tours expand at load time, after `times:` is given out).
pub fn part_spoken(text: &str, pron: &[(String, String)]) -> (Vec<Spoken>, Option<&'static str>) {
    let s = script::parse(text);
    let cues: Vec<_> = s.parts.iter().flat_map(|p| &p.cues).collect();
    let skip = if s.parts.len() > 1 {
        Some("more than one `##` part in the file")
    } else if cues.iter().any(|c| c.tour.is_some()) {
        Some("a generated tour line (its cues are only known at load time)")
    } else {
        None
    };
    (cues.iter().map(|c| (iwr_core::speech::spoken(&c.say, pron), c.second())).collect(), skip)
}

/// The hashes a `.times` file keeps of a part's cues: a line given to the other voice is stale
/// too. A line of the main voice hashes its text alone, as before there was a second voice.
pub fn hashes(spoken: &[Spoken]) -> Vec<String> {
    spoken.iter().map(|(t, second)| if *second { text_hash(&format!("sp:{}", t)) } else { text_hash(t) }).collect()
}

/// Stable short hash of a cue's spoken text (FNV-1a), to tell when a `.times` file is stale.
pub fn text_hash(s: &str) -> String {
    let h = s.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    format!("{:016x}", h)
}

/// Audio files a script refers to (script-level and per part), relative to its directory.
pub fn audio_files(s: &script::Script) -> Vec<String> {
    let mut v: Vec<String> = s.audio.iter().cloned().collect();
    v.extend(s.parts.iter().filter_map(|p| p.audio.clone()));
    v.extend(s.parts.iter().flat_map(|p| p.voices.iter().map(|v| v.audio.clone())));
    v.retain(|a| !a.contains("://") && !a.starts_with("data:"));
    v.dedup();
    v
}

/// A safe relative file name inside the codecast directory (no path escapes, audio only).
pub fn is_safe_audio_name(name: &str) -> bool {
    config::is_safe_path(name) && Path::new(name).extension().and_then(|e| e.to_str()).map(|e| AUDIO_EXTS.contains(&e.to_ascii_lowercase().as_str())).unwrap_or(false)
}

/// A file the player may fetch from the codecast directory: a recording or a font.
pub fn is_safe_asset_name(name: &str) -> bool {
    is_safe_audio_name(name) || (config::is_safe_path(name) && config::is_font_file(name))
}

/// Problems in a project glossary: entries whose directives do not resolve.
pub fn check_glossary(p: &iwr_core::Project, text: &str) -> Vec<String> {
    let mut bad = Vec::new();
    for e in iwr_core::glossary::parse(text) {
        let at = format!("glossary / `## {}`", e.term);
        if let Some(show) = &e.show {
            let (v, r) = script::parse_show(show);
            if v.is_none() {
                bad.push(format!("{}: unknown view in `@ {}`", at, show));
            }
            if let Some(r) = r {
                if script::resolve(p, r).is_none() {
                    bad.push(format!("{}: unresolved ref `{}` in `@`", at, r));
                }
            }
        }
        for r in e.hl.iter().flatten().chain(e.code.iter()) {
            if script::resolve(p, r).is_none() {
                bad.push(format!("{}: unresolved ref `{}`", at, r));
            }
        }
    }
    bad
}

/// Pronunciations in effect for a script: built-in, then the project glossary's `pronounce:`
/// lines, then the script's own.
pub fn pronounce(s: &script::Script, glossary: Option<&str>) -> Vec<(String, String)> {
    let mut v = iwr_core::speech::builtin_pronounce();
    if let Some(g) = glossary {
        v.extend(iwr_core::speech::parse_pronounce(g));
    }
    v.extend(s.pronounce.iter().cloned());
    v
}

#[cfg(test)]
mod tutorial_tests {
    /// The UI ships the tutorial with its recording: editing a sentence without recording it
    /// again (`codecast voice crates/iwr-ui/assets/tutorial --path crates/iwr-core/tests/fixtures/demo`) fails here.
    #[test]
    fn tutorial_recording_matches_its_text() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../iwr-ui/assets/tutorial");
        let loaded = super::load(&dir).unwrap();
        assert!(loaded.stale.is_empty(), "record the tutorial again: {:?}", loaded.stale);
        let s = iwr_core::script::parse(&loaded.text);
        assert!(s.parts.iter().all(|p| p.voices.iter().any(|v| v.voice == "af_heart" && v.times.len() == p.cues.len())));
    }

    /// The same for the Japanese tutorial, written in Japanese (`codecast voice
    /// crates/iwr-ui/assets/tutorial/ja --lang ja --voice jf_alpha --written --path
    /// crates/iwr-core/tests/fixtures/demo`).
    #[test]
    fn japanese_tutorial_recording_matches_its_text() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../iwr-ui/assets/tutorial/ja");
        let loaded = super::load(&dir).unwrap();
        assert!(loaded.stale.is_empty(), "record the Japanese tutorial again: {:?}", loaded.stale);
        let s = iwr_core::script::parse(&loaded.text);
        assert!(s.parts.iter().all(|p| p.voices.iter().any(|v| v.lang == "ja" && v.voice == "jf_alpha" && v.times.len() == p.cues.len())));
    }
}

#[cfg(test)]
mod naming_tests {
    use super::*;

    #[test]
    fn stems_use_underscores_only() {
        assert_eq!(variant_stem("01-welcome", "en", ""), "01-welcome");
        assert_eq!(variant_stem("01-welcome", "ja", ""), "01-welcome_ja");
        assert_eq!(variant_stem("01-welcome", "en", "am_michael"), "01-welcome_en_am-michael");
        assert_eq!(variant_stem("01-welcome", "ja", "jf_alpha"), "01-welcome_ja_jf-alpha");
        assert_eq!(variant_stem("01-a", "en", "am_michael+af_heart"), "01-a_en_am-michael+af-heart");
        assert_eq!(video_name("01-welcome", "ja", ""), "01-welcome_ja.mp4");
        assert_eq!(video_name("01-welcome", "en", "ja"), "01-welcome_en_sub-ja.mp4");
    }

    #[test]
    fn parses_its_own_stems() {
        assert_eq!(parse_variant("01-welcome_en_am-michael", "01-welcome"), Some(("en".into(), "am_michael".into())));
        assert_eq!(parse_variant("01-welcome_ja_jf-alpha", "01-welcome"), Some(("ja".into(), "jf_alpha".into())));
        assert_eq!(parse_variant("01-welcome_ja", "01-welcome"), Some(("ja".into(), String::new())));
        assert_eq!(parse_variant("01-welcome_en", "01-welcome"), None);
        assert_eq!(parse_variant("02-how_it-runs_ja", "02-how"), None);
        assert_eq!(parse_variant("02-how_it-runs_ja", "02-how_it-runs"), Some(("ja".into(), String::new())));
        assert_eq!(lang_arg("01-welcome_ja_jf-alpha"), " --lang ja --voice jf_alpha");
        assert_eq!(lang_arg("02-how_it-runs_en_am-adam"), " --voice am_adam");
        assert_eq!(lang_arg("02-how_it-runs_ja"), " --lang ja");
    }

    #[test]
    fn renames_dotted_names_once() {
        let dir = std::env::temp_dir().join(format!("codecast-names-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for f in ["01-welcome.md", "01-welcome.am_michael.ogg", "01-welcome.am_michael.times", "01-welcome.ja.jf_alpha.ogg", "01-welcome.ja.jf_alpha.times", "01-welcome.am_michael.mp4", "01-welcome.ja.jf_alpha.sub-en.mp4", "01-welcome.ja.jf_alpha.part.mp4", "01-welcome.ogg"] {
            std::fs::write(dir.join(f), "").unwrap();
        }
        let moved = migrate_names(&dir);
        assert_eq!(moved.len(), 6);
        let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        names.sort();
        assert_eq!(names, ["01-welcome.md", "01-welcome.ogg", "01-welcome_en.mp4", "01-welcome_en_am-michael.ogg", "01-welcome_en_am-michael.times", "01-welcome_ja_jf-alpha.ogg", "01-welcome_ja_jf-alpha.times", "01-welcome_ja_sub-en.mp4", "01-welcome.ja.jf_alpha.part.mp4"].iter().map(|s| s.to_string()).collect::<Vec<_>>().tap_sort());
        assert!(migrate_names(&dir).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    trait TapSort {
        fn tap_sort(self) -> Self;
    }
    impl TapSort for Vec<String> {
        fn tap_sort(mut self) -> Self {
            self.sort();
            self
        }
    }
}

#[cfg(test)]
mod discover_tests {
    use super::*;

    /// A codecast in the top folder and in a sub-folder are both on the map, the folder between
    /// them too; build output and hidden folders are not searched.
    #[test]
    fn finds_the_codecasts_under_the_git_root() {
        let top = std::env::temp_dir().join(format!("iwr-discover-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&top);
        let write = |rel: &str, text: &str| {
            let f = top.join(rel);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, text).unwrap();
        };
        write(".git/HEAD", "");
        write("codecast.md", "# Whole\n\n## One\nHello.\n");
        write("src/main.rs", "fn main() {}\n");
        write("crates/core/src/lib.rs", "");
        write("docs/codecast.md", "# The format\n");
        write("crates/core/codecast/index.md", "# Core\n");
        write("crates/core/codecast/01-a.md", "## A\nFirst.\n");
        write("crates/core/codecast/01-a_ja.ogg", "");
        write("target/codecast.md", "# Built\n");
        write(".hidden/codecast.md", "# Hidden\n");
        let top = top.canonicalize().unwrap();

        // served from the sub-folder: the scan still starts at the git root
        let m = discover(&top.join("crates/core"), "");
        let paths: Vec<&str> = m.nodes.iter().map(|n| n.path.as_str()).collect();
        assert_eq!(paths, ["", "crates", "crates/core"]);
        assert_eq!(m.nodes[0].cast.as_ref().unwrap().title, "Whole");
        assert!(m.nodes[1].cast.is_none());
        let core = m.nodes[2].cast.as_ref().unwrap();
        assert_eq!((core.title.as_str(), core.parts, core.langs.as_slice()), ("Core", 1, &["ja".to_string()][..]));
        assert!(m.nodes[2].current && !m.partial);
        std::fs::remove_dir_all(&top).unwrap();
    }
}

#[cfg(test)]
mod topic_tests {
    use super::*;

    /// Sub-folders of `codecast/` with a codecast's files are topics, named after the folder
    /// unless their `index.md` has a title; they borrow the glossary and the look.
    #[test]
    fn topics_are_the_sub_folders_with_a_script() {
        let top = std::env::temp_dir().join(format!("iwr-topics-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&top);
        let write = |rel: &str, text: &str| {
            let f = top.join(rel);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, text).unwrap();
        };
        write("src/main.rs", "fn main() {}\n");
        write("codecast/index.md", "# Whole\n");
        write("codecast/01-a.md", "## A\nFirst.\n");
        write("codecast/glossary.md", "## term\nWhat it is.\n");
        write("codecast/codecast.toml", "");
        write("codecast/translations/ja.json", "{}");
        write("codecast/fonts/x.woff2", "");
        write("codecast/02-error_handling/01-why.md", "## Why\nBecause.\n");
        write("codecast/01-the_parser/index.md", "# Parsing, step by step\n");
        write("codecast/01-the_parser/01-tokens.md", "## Tokens\nOne.\n");
        write("codecast/01-the_parser/glossary.md", "## token\nA word.\n");
        let top = top.canonicalize().unwrap();
        let cast = top.join("codecast");

        let t = topic_list(&cast, "02-error_handling");
        assert!(t.general);
        assert_eq!(t.title, "Whole");
        let seen: Vec<(&str, &str, usize)> = t.topics.iter().map(|t| (t.dir.as_str(), t.label.as_str(), t.info.parts)).collect();
        assert_eq!(seen, [("01-the_parser", "Parsing, step by step", 1), ("02-error_handling", "error handling", 1)]);

        let (parser, errors) = (cast.join("01-the_parser"), cast.join("02-error_handling"));
        assert_eq!(parent_cast(&errors), Some(cast.clone()));
        assert_eq!(parent_cast(&cast), None);
        assert_eq!(glossary_path(&errors), cast.join("glossary.md"));
        assert_eq!(glossary_path(&parser), parser.join("glossary.md"));
        assert_eq!(config_dir(&errors), cast);
        assert_eq!(default_project(&errors), top);
        assert_eq!(default_project(&cast), top);
        // the general codecast does not read the topics' files
        assert_eq!(script::parse(&load(&cast).unwrap().text).parts.len(), 1);
        assert_eq!(script::parse(&load(&errors).unwrap().text).title, "error handling");

        // on the map, under the folder; the one that plays is marked
        let m = discover(&top, "02-error_handling");
        let names: Vec<(&str, bool)> = m.nodes.iter().map(|n| (n.name.as_str(), n.current)).collect();
        assert_eq!(names[1..], [("Parsing, step by step", false), ("error handling", true)]);

        // topics alone: no general codecast
        std::fs::remove_file(cast.join("index.md")).unwrap();
        std::fs::remove_file(cast.join("01-a.md")).unwrap();
        assert!(!topic_list(&cast, "").general);
        assert!(discover(&top, "").nodes[0].cast.is_none());
        std::fs::remove_dir_all(&top).unwrap();
    }
}
