//! Finding and reading a codecast on disk: a single file (`codecast.md`, legacy `codecast.txt`)
//! or a `codecast/` directory with `index.md` and one file per part (`01-welcome.md`, …).

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
    /// part files whose `iwr voice` recording was left out because their text changed since
    /// (`01-welcome`, or `01-welcome.ja` for a recording in another language)
    pub stale: Vec<String>,
}

/// Where a script's relative names (`audio:`, fonts, glossary) resolve: the `codecast/`
/// directory itself, or the directory of a single-file script.
pub fn script_dir(script: &Path) -> PathBuf {
    if script.is_dir() { script.to_path_buf() } else { script.parent().map(Path::to_path_buf).unwrap_or_default() }
}

/// The project glossary file next to a script: `codecast/glossary.md` for a directory,
/// `glossary.md` beside a single-file script.
pub fn glossary_path(script: &Path) -> PathBuf {
    script_dir(script).join("glossary.md")
}

/// The codecast's look and defaults (`codecast.toml` next to the script), if it has one.
pub fn read_config(script: &Path) -> Result<Option<Config>> {
    let path = script_dir(script).join(config::FILE);
    let Ok(text) = std::fs::read_to_string(&path) else { return Ok(None) };
    Config::from_toml(&text).map(Some).map_err(|e| anyhow::anyhow!("{}: {}", path.display(), e))
}

/// Problems in `codecast.toml`: bad values, font files that are not there.
pub fn check_config(script: &Path) -> Vec<String> {
    match read_config(script) {
        Ok(Some(c)) => {
            let dir = script_dir(script);
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

/// Read a script file or directory.
pub fn load(path: &Path) -> Result<Loaded> {
    if path.is_dir() {
        let index = std::fs::read_to_string(path.join("index.md")).unwrap_or_default();
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
        // part is spoken by TTS until `iwr voice` runs again
        let pron = pronounce(&script::parse(&script::assemble(&index, &parts)), glossary.as_deref());
        let mut stale = Vec::new();
        for pf in parts.iter_mut() {
            let hashes: Vec<String> = part_spoken(&pf.text, &pron).0.iter().map(|s| text_hash(s)).collect();
            let times = read_times(&path.join(format!("{}.times", pf.stem)));
            if !times.is_empty() && pf.audio.is_some() {
                if hashes.iter().eq(times.iter().map(|(_, h)| h)) {
                    pf.times = times.into_iter().map(|(t, _)| t).collect();
                } else {
                    pf.audio = None;
                    stale.push(pf.stem.clone());
                }
            }
            // `01-welcome.ja.ogg` + `.ja.times`: the part in Japanese, current while the English
            // text it was translated from is (the hashes are of the English cues)
            for lang in iwr_core::i18n::LANGS.iter().filter(|l| l.code != "en") {
                let stem = format!("{}.{}", pf.stem, lang.code);
                let Some(audio) = AUDIO_EXTS.iter().map(|e| format!("{}.{}", stem, e)).find(|n| path.join(n).is_file()) else { continue };
                let times = read_times(&path.join(format!("{}.times", stem)));
                if !times.is_empty() && hashes.iter().eq(times.iter().map(|(_, h)| h)) {
                    pf.voices.push(script::Voiced { lang: lang.code.into(), audio, times: times.into_iter().map(|(t, _)| t).collect() });
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

/// ` --lang ja` for a stale stem `01-welcome.ja`, nothing for an English one.
pub fn lang_arg(stem: &str) -> String {
    stem.rsplit_once('.').map(|(_, l)| format!(" --lang {}", l)).unwrap_or_default()
}

/// A `.times` file written by `iwr voice`: one `start-seconds text-hash` line per cue, `#` comments.
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

/// A part file's cues as they will be spoken, and why it cannot be recorded by `iwr voice`, if so
/// (generated tours expand at load time, after `times:` is given out).
pub fn part_spoken(text: &str, pron: &[(String, String)]) -> (Vec<String>, Option<&'static str>) {
    let s = script::parse(text);
    let cues: Vec<_> = s.parts.iter().flat_map(|p| &p.cues).collect();
    let skip = if s.parts.len() > 1 {
        Some("more than one `##` part in the file")
    } else if cues.iter().any(|c| c.tour.is_some()) {
        Some("a generated tour line (its cues are only known at load time)")
    } else {
        None
    };
    (cues.iter().map(|c| iwr_core::speech::spoken(&c.say, pron)).collect(), skip)
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
