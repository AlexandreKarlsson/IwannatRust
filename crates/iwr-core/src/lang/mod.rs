//! Language front-ends: one per [`SourceLang`], each turning the files of its language into the
//! shared model. This is the only place the analyzer asks "which language?".
//!
//! A front-end is two functions:
//! - `collect(ctx, files)`: first pass. Parse each file, create its module, push its items, keep
//!   every function body ([`Body`]) for later.
//! - `analyze(…) -> Analyzed`: second pass, once every item of the project is known. Resolve the
//!   calls of one body, then drive the language-neutral [`crate::cfg::Builder`] and
//!   [`crate::blocks::Builder`] over it.
//!
//! To add a language: a variant of [`SourceLang`] with its row in every method there (the
//! compiler lists them), a variant of [`Body`], a module here, and an arm in [`collect`] and
//! [`analyze_body`]. Front-ends on tree-sitter share [`ts`] (node text, spans, comments as docs).

pub mod c;
pub mod rust;
pub(crate) mod ts;

use crate::model::*;
use crate::parse::Ctx;
use crate::resolve::{Index, Stats};

/// One source file handed to a front-end.
pub(crate) struct Source {
    pub file: FileId,
    pub path: String,
    pub content: String,
}

/// A function body kept for the second pass, in the syntax tree of its language.
pub enum Body {
    Rust { block: syn::Block, sig: syn::Signature },
    C(TreeBody),
}

/// A function definition inside a tree-sitter tree.
pub struct TreeBody {
    pub tree: tree_sitter::Tree,
    /// Byte range of the definition's node.
    pub start: usize,
    pub end: usize,
}

impl TreeBody {
    /// The definition's node (the whole file when the range no longer names one).
    pub(crate) fn node(&self) -> tree_sitter::Node<'_> {
        self.tree.root_node().descendant_for_byte_range(self.start, self.end).unwrap_or_else(|| self.tree.root_node())
    }
}

/// What a front-end found in one function body.
pub(crate) struct Analyzed {
    pub calls: Vec<CallEdge>,
    pub cfg: Cfg,
    pub blocks: Block,
    pub stats: Stats,
}

/// First pass over the files of one language.
pub(crate) fn collect(lang: SourceLang, ctx: &mut Ctx, files: Vec<Source>) {
    match lang {
        SourceLang::Rust => rust::collect(ctx, files),
        SourceLang::C => c::collect(ctx, files),
    }
}

/// Second pass over one function body: its calls, flow graph, block tree and counts.
pub(crate) fn analyze_body(p: &Project, ix: &Index, item: ItemId, file: FileId, body: &Body) -> Analyzed {
    match body {
        Body::Rust { block, sig } => rust::analyze(p, ix, item, file, block, sig),
        Body::C(t) => c::analyze(p, ix, item, file, t.node()),
    }
}
