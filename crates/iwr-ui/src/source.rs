//! Source lines with syntax colours (`iwr_core::highlight`) and, in the gutter, the
//! colour of the item and of the block each line belongs to (the same colours as
//! the boxes of the Code view and of the graphs).

use crate::code::{block_color, item_kind_color};
use dioxus::prelude::*;
use iwr_core::highlight::{self, Line, Tok};
use iwr_core::model::{FileId, ItemKind, Project, Span};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// What the source panes need per file, computed once per file content.
pub struct Shaded {
    pub lines: Vec<Line>,
    /// Per line: colour of the innermost item, colour of the deepest block.
    pub colors: Vec<(Option<&'static str>, Option<&'static str>)>,
}

/// Content address and length of the file it was computed from (a reloaded project has new contents), result.
type Cached = (usize, usize, Rc<Shaded>);

thread_local! {
    static CACHE: RefCell<HashMap<FileId, Cached>> = RefCell::new(HashMap::new());
}

/// Highlighted lines and gutter colours of a file (cached).
pub fn shaded(p: &Project, file: FileId) -> Rc<Shaded> {
    let sf = p.file(file);
    let key = (sf.content.as_ptr() as usize, sf.content.len());
    if let Some(s) = CACHE.with(|c| c.borrow().get(&file).filter(|e| (e.0, e.1) == key).map(|e| e.2.clone())) {
        return s;
    }
    let lines = if sf.rust { highlight::rust(&sf.content) } else { sf.content.lines().map(|l| vec![(Tok::Plain, l.to_string())]).collect() };
    let mut colors = vec![(None, None); lines.len()];
    // innermost item first wins: visit items from the largest span to the smallest
    let mut items: Vec<_> = p.items.iter().filter(|i| i.span.file == file && i.kind != ItemKind::Module).collect();
    items.sort_by_key(|i| std::cmp::Reverse(i.span.line_end - i.span.line_start));
    for it in items {
        let color = item_kind_color(it);
        let blocks = it.fn_info().and_then(|f| f.blocks.as_ref());
        for ln in it.span.line_start..=it.span.line_end.min(lines.len()) {
            let Some(slot) = colors.get_mut(ln.wrapping_sub(1)) else { continue };
            let block = blocks.and_then(|b| b.children.iter().find_map(|c| c.deepest_at(ln))).map(|b| block_color(b.kind));
            *slot = (Some(color), block);
        }
    }
    let s = Rc::new(Shaded { lines, colors });
    CACHE.with(|c| c.borrow_mut().insert(file, (key.0, key.1, s.clone())));
    s
}

/// Columns (char range) of `mark` on line `ln`, as the source panes underline it.
pub fn mark_range(mark: Option<Span>, file: FileId, ln: usize, line_len: usize) -> Option<(usize, usize)> {
    match mark.filter(|m| m.file == file && m.contains_line(ln)) {
        Some(m) if m.col_end > 1 && m.line_start == m.line_end => {
            let a = m.col_start.saturating_sub(1).min(line_len);
            Some((a, m.col_end.min(line_len).max(a)))
        }
        Some(_) => Some((0, line_len)),
        None => None,
    }
}

/// Cut a line's runs where the mark starts and ends: `(css class, text, marked)`.
fn runs(line: &Line, mark: Option<(usize, usize)>) -> Vec<(&'static str, String, bool)> {
    let mut out = Vec::new();
    let mut col = 0;
    for (tok, text) in line {
        let n = text.chars().count();
        let (a, b) = mark.map(|(a, b)| (a.clamp(col, col + n) - col, b.clamp(col, col + n) - col)).unwrap_or((0, 0));
        for (from, to, marked) in [(0, a, false), (a, b, true), (b, n, false)] {
            if to > from {
                out.push((tok.class(), text.chars().skip(from).take(to - from).collect(), marked));
            }
        }
        col += n;
    }
    out
}

/// Inside of a source line: number, gutter colours, coloured code.
pub fn line_body(sh: &Shaded, ln: usize, mark: Option<(usize, usize)>) -> Element {
    let empty = Vec::new();
    let line = sh.lines.get(ln - 1).unwrap_or(&empty);
    let (ic, bc) = sh.colors.get(ln - 1).copied().unwrap_or_default();
    let gut = format!("--ic: {}; --bc: {}", ic.unwrap_or("transparent"), bc.unwrap_or("transparent"));
    rsx! {
        span { class: "ln", "{ln}" }
        span { class: "gut", style: "{gut}" }
        span { class: "code", style: "{gut}",
            for (cls, text, marked) in runs(line, mark) {
                span { class: if marked { format!("{cls} mark") } else { cls.to_string() }, "{text}" }
            }
        }
    }
}

/// A short piece of Rust (a signature, a type) with syntax colours.
#[component]
pub fn RustText(text: String) -> Element {
    let runs: Vec<(Tok, String)> = highlight::rust(&text).into_iter().enumerate().flat_map(|(i, l)| (i > 0).then(|| (Tok::Plain, "\n".to_string())).into_iter().chain(l)).collect();
    rsx! {
        for (tok, t) in runs {
            span { class: tok.class(), "{t}" }
        }
    }
}
