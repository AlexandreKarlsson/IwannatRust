//! Load a cargo project (or a bare directory of `.rs` files) from disk.

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

/// Read a non-Rust file as text when it is small and valid UTF-8; empty string otherwise.
fn read_other(p: &Path) -> String {
    match std::fs::metadata(p) {
        Ok(m) if m.len() <= MAX_TEXT => std::fs::read(p).ok().and_then(|b| String::from_utf8(b).ok()).unwrap_or_default(),
        _ => String::new(),
    }
}

/// Walk `root` and collect `.rs` files (relative path, content).
pub fn load(root: &Path) -> std::io::Result<Loaded> {
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
            if p.extension().map(|e| e == "rs").unwrap_or(false) {
                if let Ok(c) = std::fs::read_to_string(p) {
                    files.push((rel, c));
                }
            } else {
                files.push((rel, read_other(p)));
            }
        }
    }
    // put entry points first so `crate` module gets its file
    files.sort_by_key(|(p, _)| {
        let pri = if p.ends_with("src/main.rs") { 0 } else if p.ends_with("src/lib.rs") { 1 } else if p.ends_with("mod.rs") { 2 } else { 3 };
        (pri, p.clone())
    });
    Ok(Loaded { name, root, files })
}
