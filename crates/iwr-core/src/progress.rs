//! How far an analysis is: what the server tells the loading page while a big project parses.
//!
//! The analyzers call a `&mut dyn FnMut(&Progress)` as they go; `codecast serve` keeps the last
//! value in a [`Status`] that `/api/status` answers and the UI's loading page polls. Shared by the
//! server and the UI (serde), so it stays free of file-system and parser dependencies.

use serde::{Deserialize, Serialize};

/// The step of the pipeline an analysis is in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Nothing started yet.
    #[default]
    Idle,
    /// Walking the directory and reading the files.
    Reading,
    /// Parsing the sources into items and modules.
    Parsing,
    /// Linking impls to types and traits.
    Linking,
    /// Resolving the calls and building the flow of every function.
    Calls,
    /// The project is ready.
    Done,
    /// The analysis failed (see [`Status::error`]).
    Failed,
}

/// A snapshot of the analysis: the stage, how many of its steps are done, what it is on.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    pub stage: Stage,
    /// Steps done in this stage (files read / parsed, functions resolved).
    pub done: usize,
    /// Steps the stage has; `0` when not known yet (while reading the tree).
    pub total: usize,
    /// What the stage is on right now: a file path, a function name.
    pub current: String,
    /// Running counts of what was found so far.
    pub files: usize,
    pub items: usize,
    pub calls: usize,
}

impl Progress {
    /// The stage's completion in `0.0..=1.0`, `None` while its size is unknown.
    pub fn fraction(&self) -> Option<f64> {
        (self.total > 0).then(|| (self.done as f64 / self.total as f64).clamp(0.0, 1.0))
    }
}

/// What `/api/status` answers: the project being analyzed and how far it is.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Status {
    /// The project's name (its directory until `Cargo.toml` is read).
    pub name: String,
    /// The path given to `codecast serve`.
    pub path: String,
    /// `true` once a project is served (`/api/project` answers); a re-analysis keeps it.
    pub ready: bool,
    /// `true` while an analysis runs (the first one, or a re-analysis after an edit).
    pub busy: bool,
    pub progress: Progress,
    /// Milliseconds since this analysis started.
    pub elapsed_ms: u64,
    /// Why the last analysis failed, if it did.
    pub error: Option<String>,
}
