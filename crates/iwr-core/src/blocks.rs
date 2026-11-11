//! Simplified "code as blocks" tree: the nested structure of a function body
//! (loops, branches, calls, statements) without expression noise. Variables are
//! attached to the block that uses them, so they only show up where they matter.
//!
//! Like [`crate::cfg`], the [`Builder`] is language-neutral (ids, scopes, block
//! kinds, notes); the walkers are in [`crate::lang`] (`rust::blocks`, `c::blocks`).

use crate::cfg::{contains, Facts};
use crate::model::*;
use crate::shorten;
pub use crate::model::{Block, BlockKind};
use std::collections::HashSet;

/// Shared part of the block tree construction, driven by a language-specific walker.
pub(crate) struct Builder<'a> {
    file: FileId,
    calls: &'a [CallEdge],
    /// Locals in scope, innermost last; `uses` keeps only names found here.
    scope: Vec<HashSet<String>>,
    next_id: usize,
}


// ---- language-neutral construction ----

impl<'a> Builder<'a> {
    /// A builder whose outermost scope holds the function's parameters.
    pub(crate) fn new(file: FileId, calls: &'a [CallEdge], params: &[String]) -> Self {
        Builder { file, calls, scope: vec![params.iter().cloned().collect()], next_id: 1 }
    }

    pub(crate) fn file(&self) -> FileId {
        self.file
    }

    /// A block with the next id; its `calls` are the project functions called inside `span`.
    pub(crate) fn new_block(&mut self, kind: BlockKind, label: String, detail: String, span: Span) -> Block {
        let id = self.next_id;
        self.next_id += 1;
        let mut calls: Vec<ItemId> = self.calls.iter().filter(|c| c.to.is_some() && c.kind != CallKind::Construct && contains(&span, &c.span)).filter_map(|c| c.to).collect();
        calls.dedup();
        Block { id, kind, label: shorten(&label, 70), detail, span, children: vec![], defines: vec![], uses: vec![], calls, note: None }
    }

    pub(crate) fn in_scope(&self, name: &str) -> bool {
        self.scope.iter().any(|s| s.contains(name))
    }

    /// Names a block defines, visible to what follows it in the same scope.
    pub(crate) fn define(&mut self, names: &[String]) {
        if let Some(s) = self.scope.last_mut() {
            for n in names {
                s.insert(n.clone());
            }
        }
    }

    /// A nested scope holding `names` (a loop pattern, an arm pattern, closure arguments).
    pub(crate) fn push_scope(&mut self, names: &[String]) {
        self.scope.push(names.iter().cloned().collect());
    }

    pub(crate) fn pop_scope(&mut self) {
        self.scope.pop();
    }

    /// Of the names an expression reads, those that are locals in scope.
    pub(crate) fn uses_in_scope(&self, names: Vec<String>) -> Vec<String> {
        names.into_iter().filter(|n| self.in_scope(n)).collect()
    }

    /// The kind of a leaf statement: what it does that matters most, else `default`.
    pub(crate) fn leaf_kind(&self, f: &Facts, span: &Span, default: BlockKind) -> BlockKind {
        let has_calls = self.calls.iter().any(|c| c.to.is_some() && c.kind != CallKind::Construct && contains(span, &c.span));
        if f.has_return {
            BlockKind::Return
        } else if f.has_try {
            BlockKind::Propagate
        } else if f.has_panic || f.has_unwrap {
            BlockKind::Panic
        } else if f.has_await {
            BlockKind::Await
        } else if has_calls {
            BlockKind::Call
        } else {
            default
        }
    }

    /// The root block of a function (its signature) with `children`, every block given its note.
    pub(crate) fn root(self, signature: String, fn_span: Span, params: Vec<String>, children: Vec<Block>, source: &str) -> Block {
        let mut root = Block { id: 0, kind: BlockKind::Fn, label: signature.clone(), detail: signature, span: fn_span, children, defines: params, uses: vec![], calls: vec![], note: None };
        let lines: Vec<&str> = source.lines().collect();
        add_notes(&mut root, &lines);
        root
    }
}

/// Give every block the short `//` comment just above it, when the block starts its own line
/// (`} else if` would read the comment closing the branch before).
fn add_notes(b: &mut Block, lines: &[&str]) {
    for c in &mut b.children {
        let starts_line = lines.get(c.span.line_start.wrapping_sub(1)).is_some_and(|l| l.chars().take(c.span.col_start.saturating_sub(1)).all(char::is_whitespace));
        if starts_line {
            c.note = crate::cfg::note_above(lines, c.span.line_start);
        }
        add_notes(c, lines);
    }
}
