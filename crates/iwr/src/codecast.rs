//! Finding and reading a codecast on disk: a single file (`codecast.md`, legacy `codecast.txt`)
//! or a `codecast/` directory with `index.md` and one file per part (`01-welcome.md`, …).

use anyhow::{Context, Result};
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
}

/// The project glossary file next to a script: `codecast/glossary.md` for a directory,
/// `glossary.md` beside a single-file script.
pub fn glossary_path(script: &Path) -> PathBuf {
    let dir = if script.is_dir() { script.to_path_buf() } else { script.parent().map(Path::to_path_buf).unwrap_or_default() };
    dir.join("glossary.md")
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
            parts.push(PartFile { stem, text, audio });
        }
        Ok(Loaded { text: script::assemble(&index, &parts), dir: path.to_path_buf(), glossary: std::fs::read_to_string(glossary_path(path)).ok() })
    } else {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading script {}", path.display()))?;
        Ok(Loaded { text, dir: path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from(".")), glossary: std::fs::read_to_string(glossary_path(path)).ok() })
    }
}

/// Audio files a script refers to (script-level and per part), relative to its directory.
pub fn audio_files(s: &script::Script) -> Vec<String> {
    let mut v: Vec<String> = s.audio.iter().cloned().collect();
    v.extend(s.parts.iter().filter_map(|p| p.audio.clone()));
    v.retain(|a| !a.contains("://") && !a.starts_with("data:"));
    v.dedup();
    v
}

/// A safe relative file name inside the codecast directory (no path escapes, audio only).
pub fn is_safe_audio_name(name: &str) -> bool {
    !name.is_empty() && !name.contains("..") && !name.starts_with('/') && !name.contains('\\') && Path::new(name).extension().and_then(|e| e.to_str()).map(|e| AUDIO_EXTS.contains(&e.to_ascii_lowercase().as_str())).unwrap_or(false)
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
