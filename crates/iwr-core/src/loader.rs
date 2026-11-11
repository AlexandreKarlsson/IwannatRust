//! Load a cargo project, a C project, or a bare directory of source files from disk.

use crate::model::SourceLang;
use crate::progress::{Progress, Stage};
use std::path::{Path, PathBuf};

pub struct Loaded {
    pub name: String,
    pub root: PathBuf,
    pub files: Vec<(String, String)>,
}

fn is_ignored_dir(name: &str) -> bool {
    matches!(name, "target" | ".git" | "node_modules" | ".dx" | "dist" | ".idea" | ".vscode" | "__pycache__")
}

const MAX_TEXT: u64 = 512 * 1024;

/// Read a file that is not a source as text when it is small and valid UTF-8; empty string otherwise.
fn read_other(p: &Path) -> String {
    match std::fs::metadata(p) {
        Ok(m) if m.len() <= MAX_TEXT => std::fs::read(p).ok().and_then(|b| String::from_utf8(b).ok()).unwrap_or_default(),
        _ => String::new(),
    }
}

/// Walk `root` and collect its files (relative path, content); sources first, entry points before the rest.
pub fn load(root: &Path) -> std::io::Result<Loaded> {
    load_with(root, &mut |_| {})
}

/// [`load`] that reports every 25 files read (`Stage::Reading`, `total` unknown).
pub fn load_with(root: &Path, report: &mut dyn FnMut(&Progress)) -> std::io::Result<Loaded> {
    let root = root.canonicalize()?;
    let (root, single) = if root.is_file() { (root.parent().unwrap().to_path_buf(), Some(root.clone())) } else { (root, None) };
    let mut name = root.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "project".into());
    let cargo = root.join("Cargo.toml");
    if cargo.exists() {
        if let Ok(text) = std::fs::read_to_string(&cargo) {
            if let Ok(v) = text.parse::<toml::Table>() {
                if let Some(n) = v.get("package").and_then(|p| p.get("name")).and_then(|n| n.as_str()) {
                    name = n.to_string();
                }
            }
        }
    }
    let mut files = Vec::new();
    if let Some(f) = single {
        let rel = f.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
        files.push((rel, std::fs::read_to_string(&f)?));
    } else {
        for entry in walkdir::WalkDir::new(&root)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|e| !(e.file_type().is_dir() && e.file_name().to_str().map(is_ignored_dir).unwrap_or(false)))
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
            if files.len() % 25 == 0 {
                report(&Progress { stage: Stage::Reading, done: files.len(), files: files.len(), current: rel.clone(), ..Default::default() });
            }
            if SourceLang::of_path(&rel).is_some() {
                // a source is read whole, whatever its size; bytes that are not UTF-8 (old C
                // comments in Latin-1) become `�` rather than losing the file
                if let Ok(bytes) = std::fs::read(p) {
                    files.push((rel, String::from_utf8_lossy(&bytes).into_owned()));
                }
            } else {
                files.push((rel, read_other(p)));
            }
        }
    }
    // put entry points first so `crate` module gets its file
    files.sort_by_key(|(p, _)| {
        (SourceLang::entry_rank(p), p.clone())
    });
    Ok(Loaded { name, root, files })
}
