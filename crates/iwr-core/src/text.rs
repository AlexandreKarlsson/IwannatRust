//! Helpers to turn syn / proc-macro2 things into readable strings and spans.

use crate::model::{FileId, Span};
use quote::ToTokens;

/// Render a token stream and clean up the spacing so it reads like source.
pub fn tokens_to_string<T: ToTokens>(t: &T) -> String {
    clean_tokens(&t.to_token_stream().to_string())
}

/// `quote!` output puts spaces around every token. This makes it look like rustfmt-ish code.
pub fn clean_tokens(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut i = 0;
    // generic depth: `<` right after an identifier opens generics, so the matching `>` is not a comparison
    let mut generic_depth: usize = 0;
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    while i < n {
        let c = chars[i];
        if c == '<' && generic_open(&out) {
            generic_depth += 1;
        }
        if c == ' ' {
            let prev = out.chars().last();
            let next = chars.get(i + 1).copied();
            let next2 = chars.get(i + 2).copied();
            let in_generic = generic_depth > 0;
            let drop = match (prev, next) {
                (_, Some(',')) | (_, Some(';')) | (_, Some(')')) | (_, Some(']')) => true,
                (_, Some('>')) => in_generic && next2 != Some('=') && !out.ends_with('-'),
                (_, Some('.')) => next2 != Some('.'),
                (Some('.'), _) => !out.ends_with(".."),
                (Some('('), _) | (Some('['), _) => true,
                (Some('<'), _) => in_generic,
                (Some('&'), _) => {
                    // unary reference (`&x`, `&mut x`) vs binary `a && b` / `a & b`
                    let before: Vec<char> = out.chars().rev().skip(1).take(1).collect();
                    let b = before.first().copied();
                    !out.ends_with("&&") && matches!(b, None | Some(' ') | Some('(') | Some(',') | Some('=') | Some('[') | Some('<'))
                }
                (_, Some('(')) => matches!(prev, Some(c) if is_word(c) || c == '!' || c == '>') && !ends_with_keyword(&out),
                (_, Some('<')) => generic_open(&out),
                (_, Some(':')) => true,
                (Some(':'), _) => out.ends_with("::"),
                (_, Some('!')) => matches!(prev, Some(c) if is_word(c)),
                (Some('!'), Some('[')) | (Some('!'), Some('{')) => true,
                (Some('#'), _) => true,
                (_, Some('[')) => matches!(prev, Some('#')) || matches!(prev, Some(c) if is_word(c)),
                (_, Some('?')) => true,
                _ => false,
            };
            if !drop {
                out.push(' ');
            }
        } else {
            out.push(c);
            if c == '>' && generic_depth > 0 && !out.ends_with("->") {
                generic_depth -= 1;
            }
        }
        i += 1;
    }
    out
}

/// Does a `<` following `s` open a generic argument list (as opposed to a comparison)?
/// Heuristic: after a type-like word (Uppercase start), after `::`, or after a closing `>`.
fn generic_open(s: &str) -> bool {
    let t = s.trim_end();
    if t.ends_with("::") || t.ends_with('>') {
        return true;
    }
    let word: String = t.chars().rev().take_while(|c| c.is_alphanumeric() || *c == '_').collect::<Vec<_>>().into_iter().rev().collect();
    word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) || matches!(word.as_str(), "impl" | "dyn" | "fn")
}

fn ends_with_keyword(s: &str) -> bool {
    let word: String = s
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    matches!(
        word.as_str(),
        "if" | "while" | "match" | "in" | "return" | "for" | "let" | "else" | "as" | "mut" | "ref" | "move" | "where" | "impl" | "dyn" | "fn"
    )
}

/// Build a `Span` from anything that has a proc_macro2 span (requires `span-locations`).
pub fn span_of<T: ToTokens>(t: &T, file: FileId) -> Span {
    let ts = t.to_token_stream();
    let mut iter = ts.into_iter();
    let first = iter.next();
    let last = iter.last();
    match (first, last) {
        (Some(f), Some(l)) => {
            let s = f.span().start();
            let e = l.span().end();
            Span { file, line_start: s.line, col_start: s.column + 1, line_end: e.line, col_end: e.column + 1 }
        }
        (Some(f), None) => {
            let s = f.span().start();
            let e = f.span().end();
            Span { file, line_start: s.line, col_start: s.column + 1, line_end: e.line, col_end: e.column + 1 }
        }
        _ => Span { file, ..Default::default() },
    }
}

/// Convert a proc_macro2 span directly.
pub fn pm_span(span: proc_macro2::Span, file: FileId) -> Span {
    let s = span.start();
    let e = span.end();
    Span { file, line_start: s.line, col_start: s.column + 1, line_end: e.line, col_end: e.column + 1 }
}

/// Merge two spans into one covering both.
pub fn span_union(a: Span, b: Span) -> Span {
    if a.file != b.file {
        return a;
    }
    let (ls, cs) = if (a.line_start, a.col_start) <= (b.line_start, b.col_start) {
        (a.line_start, a.col_start)
    } else {
        (b.line_start, b.col_start)
    };
    let (le, ce) = if (a.line_end, a.col_end) >= (b.line_end, b.col_end) {
        (a.line_end, a.col_end)
    } else {
        (b.line_end, b.col_end)
    };
    Span { file: a.file, line_start: ls, col_start: cs, line_end: le, col_end: ce }
}

/// Extract `///` doc comments from attributes.
pub fn doc_of(attrs: &[syn::Attribute]) -> Option<String> {
    let mut lines = Vec::new();
    for a in attrs {
        if a.path().is_ident("doc") {
            if let syn::Meta::NameValue(nv) = &a.meta {
                if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &nv.value {
                    let v = s.value();
                    lines.push(v.strip_prefix(' ').unwrap_or(&v).to_string());
                }
            }
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n").trim_end().to_string())
    }
}

/// Non-doc attributes as text, e.g. `derive(Debug, Clone)`, `test`, `inline`.
pub fn attrs_of(attrs: &[syn::Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter(|a| !a.path().is_ident("doc"))
        .map(|a| tokens_to_string(&a.meta))
        .collect()
}

/// Names listed in `#[derive(...)]`.
pub fn derives_of(attrs: &[syn::Attribute]) -> Vec<String> {
    let mut out = Vec::new();
    for a in attrs {
        if a.path().is_ident("derive") {
            let _ = a.parse_nested_meta(|meta| {
                if let Some(id) = meta.path.get_ident() {
                    out.push(id.to_string());
                } else {
                    out.push(tokens_to_string(&meta.path));
                }
                Ok(())
            });
        }
    }
    out
}

pub use crate::shorten;

/// Collect every identifier-like path segment name used in a type, e.g. `Vec<Client>` → [Vec, Client].
pub fn type_idents(ty: &syn::Type, out: &mut Vec<String>) {
    use syn::visit::Visit;
    struct V<'a>(&'a mut Vec<String>);
    impl<'ast, 'a> Visit<'ast> for V<'a> {
        fn visit_path_segment(&mut self, seg: &'ast syn::PathSegment) {
            self.0.push(seg.ident.to_string());
            syn::visit::visit_path_segment(self, seg);
        }
    }
    V(out).visit_type(ty);
}

/// Last segment of a path as string.
pub fn path_last(p: &syn::Path) -> String {
    p.segments.last().map(|s| s.ident.to_string()).unwrap_or_default()
}

/// Segments of a path as strings (without generics).
pub fn path_segments(p: &syn::Path) -> Vec<String> {
    p.segments.iter().map(|s| s.ident.to_string()).collect()
}
