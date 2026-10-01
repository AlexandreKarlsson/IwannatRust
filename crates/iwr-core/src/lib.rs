#![allow(clippy::too_many_arguments, clippy::type_complexity, clippy::large_enum_variant)]
//! # iwr-core
//!
//! Analyze Rust source code into a graph model and build visual layouts from it.
//! Independent of any UI or documentation platform.
//!
//! Pipeline: source files → [`parse`] (items, modules) → [`resolve`] (calls, CFG,
//! type relations) → [`model::Project`] → [`views`] (per-mode graphs) → [`layout`]
//! (positions) → [`guide`] (step-by-step walkthrough).

pub mod brief;
pub mod model;
pub mod guide;
pub mod script;
pub mod glossary;
pub mod speech;
pub mod layout;
pub mod views;

#[cfg(feature = "parse")]
pub mod blocks;
#[cfg(feature = "parse")]
pub mod cfg;
#[cfg(feature = "parse")]
pub mod parse;
#[cfg(feature = "parse")]
pub mod resolve;
#[cfg(feature = "parse")]
pub mod text;
#[cfg(feature = "fs")]
pub mod loader;

pub use model::Project;

/// Analyze in-memory source files. `files` are `(relative path, content)`.
#[cfg(feature = "parse")]
pub fn analyze_sources(name: &str, root: &str, files: Vec<(String, String)>) -> Project {
    let collected = parse::collect(name, root, files);
    resolve::finish(collected)
}

/// Analyze a single source text as `src/main.rs`.
#[cfg(feature = "parse")]
pub fn analyze_source(source: &str) -> Project {
    analyze_sources("snippet", "", vec![("src/main.rs".to_string(), source.to_string())])
}

/// Analyze a project directory (or single file) on disk.
#[cfg(feature = "fs")]
pub fn analyze_path(path: &std::path::Path) -> std::io::Result<Project> {
    let loaded = loader::load(path)?;
    Ok(analyze_sources(&loaded.name, &loaded.root.to_string_lossy(), loaded.files))
}

/// Shorten a label for display (kept here so the UI can use it without the `parse` feature).
pub fn shorten(s: &str, max: usize) -> String {
    let s: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() <= max {
        s
    } else {
        let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}
