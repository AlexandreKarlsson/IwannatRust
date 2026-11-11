#![allow(clippy::too_many_arguments, clippy::type_complexity, clippy::large_enum_variant)]
//! # iwr-core
//!
//! Analyze Rust and C source code into a graph model and build visual layouts from it.
//! Independent of any UI or documentation platform.
//!
//! Pipeline: source files → [`parse`] (items, modules; one front-end per language in [`lang`]:
//! Rust with `syn`, C with tree-sitter) → [`resolve`] (calls, CFG,
//! type relations) → [`model::Project`] → [`views`] (per-mode graphs) → [`layout`]
//! (positions) → [`guide`] (step-by-step walkthrough).

pub mod brief;
pub mod casts;
pub mod prompt;
pub mod config;
pub mod bytes;
pub mod chart;
pub mod diagram;
pub mod sequence;
pub mod model;
pub mod progress;
pub mod guide;
pub mod script;
pub mod tutorial;
pub mod tours;
pub mod glossary;
pub mod highlight;
pub mod i18n;
pub mod tr;
pub mod speech;
pub mod voice;
pub mod layout;
pub mod views;

#[cfg(feature = "parse")]
pub mod blocks;
#[cfg(feature = "parse")]
pub mod lang;
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
    analyze_sources_with(name, root, files, &mut |_| {})
}

/// [`analyze_sources`] that reports how far it is: `report` is called per file parsed and per
/// function resolved (a loading page follows it; see [`progress`]).
#[cfg(feature = "parse")]
pub fn analyze_sources_with(name: &str, root: &str, files: Vec<(String, String)>, report: &mut dyn FnMut(&progress::Progress)) -> Project {
    let collected = parse::collect(name, root, files, report);
    resolve::finish(collected, report)
}

/// Analyze a single source text as `src/main.rs`.
#[cfg(feature = "parse")]
pub fn analyze_source(source: &str) -> Project {
    analyze_sources("snippet", "", vec![("src/main.rs".to_string(), source.to_string())])
}

/// Analyze a project directory (or single file) on disk.
#[cfg(feature = "fs")]
pub fn analyze_path(path: &std::path::Path) -> std::io::Result<Project> {
    analyze_path_with(path, &mut |_| {})
}

/// [`analyze_path`] that reports how far it is (files found while reading, then as
/// [`analyze_sources_with`]).
#[cfg(feature = "fs")]
pub fn analyze_path_with(path: &std::path::Path, report: &mut dyn FnMut(&progress::Progress)) -> std::io::Result<Project> {
    let loaded = loader::load_with(path, report)?;
    Ok(analyze_sources_with(&loaded.name, &loaded.root.to_string_lossy(), loaded.files, report))
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
