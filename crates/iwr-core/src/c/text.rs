//! Reading a tree-sitter C tree: node text, spans, declarator names, the comments above a node.

use crate::model::{FileId, Span};
use tree_sitter::Node;

/// The source text of a node, as written.
pub(crate) fn raw<'s>(n: Node, src: &'s str) -> &'s str {
    n.utf8_text(src.as_bytes()).unwrap_or("")
}

/// The source text of a node on one line, with the spacing of formatted code.
pub(crate) fn text(n: Node, src: &str) -> String {
    clean(raw(n, src))
}

/// Collapse whitespace and line breaks; no space inside `( )`, before `,` `;` `)` `]`, around `->`
/// and `.`. String and character literals are kept as written.
pub(crate) fn clean(s: &str) -> String {
    let joined = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = String::with_capacity(joined.len());
    let chars: Vec<char> = joined.chars().collect();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, &c) in chars.iter().enumerate() {
        if let Some(q) = quote {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        if c == '"' || c == '\'' {
            quote = Some(c);
            out.push(c);
            continue;
        }
        if c == ' ' {
            let prev = out.chars().last();
            let next = chars.get(i + 1).copied();
            let drop = matches!(next, Some(',') | Some(';') | Some(')') | Some(']') | Some('.'))
                || matches!(prev, Some('(') | Some('[') | Some('.'))
                || (next == Some('-') && chars.get(i + 2) == Some(&'>'))
                || out.ends_with("->")
                || (next == Some('(') && prev.is_some_and(|p| p.is_alphanumeric() || p == '_') && !ends_with_keyword(&out));
            if !drop {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn ends_with_keyword(s: &str) -> bool {
    let word: String = s.chars().rev().take_while(|c| c.is_alphanumeric() || *c == '_').collect::<Vec<_>>().into_iter().rev().collect();
    matches!(word.as_str(), "if" | "while" | "for" | "switch" | "return" | "sizeof" | "else" | "do")
}

/// Rows and byte columns of tree-sitter → the model's 1-based lines and character columns.
pub(crate) fn span(n: Node, file: FileId, src: &str) -> Span {
    let s = n.start_position();
    let e = n.end_position();
    Span { file, line_start: s.row + 1, col_start: col_chars(src, n.start_byte(), s.column) + 1, line_end: e.row + 1, col_end: col_chars(src, n.end_byte(), e.column) + 1 }
}

/// Characters between the start of the line and `byte`, whose byte column is `col`.
fn col_chars(src: &str, byte: usize, col: usize) -> usize {
    let start = byte.saturating_sub(col);
    src.get(start..byte).map(|s| s.chars().count()).unwrap_or(col)
}

/// Named children of a node (comments left out).
pub(crate) fn named(n: Node) -> Vec<Node> {
    let mut cur = n.walk();
    let v: Vec<Node> = n.named_children(&mut cur).filter(|c| c.kind() != "comment").collect();
    v
}

/// Every child of a node, punctuation included (comments left out).
pub(crate) fn children(n: Node) -> Vec<Node> {
    let mut cur = n.walk();
    let v: Vec<Node> = n.children(&mut cur).filter(|c| c.kind() != "comment").collect();
    v
}

pub(crate) fn field<'t>(n: Node<'t>, name: &str) -> Option<Node<'t>> {
    n.child_by_field_name(name)
}

/// The name a declarator declares: `x` in `*x`, `x[3]`, `x(int)`, `(*x)(int)`, `x = 1`.
pub(crate) fn declarator_name(n: Node, src: &str) -> Option<String> {
    match n.kind() {
        "identifier" | "field_identifier" | "type_identifier" | "statement_identifier" => Some(raw(n, src).to_string()),
        "pointer_declarator" | "function_declarator" | "array_declarator" | "init_declarator" | "attributed_declarator" | "pointer_type_declarator" | "function_type_declarator" | "array_type_declarator" | "pointer_field_declarator" | "function_field_declarator" | "array_field_declarator" => {
            declarator_name(field(n, "declarator").or_else(|| named(n).into_iter().next())?, src)
        }
        "parenthesized_declarator" | "parenthesized_type_declarator" | "parenthesized_field_declarator" => named(n).into_iter().find_map(|c| declarator_name(c, src)),
        _ => named(n).into_iter().find_map(|c| declarator_name(c, src)),
    }
}

/// Does this declarator declare a function (not a pointer to one)? `f(int)` yes, `(*f)(int)` no.
pub(crate) fn is_function_declarator(n: Node) -> bool {
    match n.kind() {
        "function_declarator" | "function_type_declarator" | "function_field_declarator" => {
            let inner = field(n, "declarator").or_else(|| named(n).into_iter().next());
            matches!(inner.map(|d| d.kind()), Some("identifier") | Some("field_identifier") | Some("type_identifier") | Some("attributed_declarator"))
        }
        "pointer_declarator" | "array_declarator" | "attributed_declarator" | "pointer_type_declarator" | "init_declarator" => field(n, "declarator").or_else(|| named(n).into_iter().next()).is_some_and(is_function_declarator),
        _ => false,
    }
}

/// `*` and `[]` wrapping a declarator, as text: the part of the type written on the name's side.
pub(crate) fn declarator_decoration(n: Node, src: &str) -> String {
    match n.kind() {
        "pointer_declarator" | "pointer_type_declarator" | "pointer_field_declarator" | "abstract_pointer_declarator" => format!("*{}", field(n, "declarator").map(|d| declarator_decoration(d, src)).unwrap_or_default()),
        "array_declarator" | "array_type_declarator" | "array_field_declarator" | "abstract_array_declarator" => {
            let inner = field(n, "declarator").map(|d| declarator_decoration(d, src)).unwrap_or_default();
            let size = field(n, "size").map(|s| text(s, src)).unwrap_or_default();
            format!("{}[{}]", inner, size)
        }
        "init_declarator" | "attributed_declarator" => field(n, "declarator").or_else(|| named(n).into_iter().next()).map(|d| declarator_decoration(d, src)).unwrap_or_default(),
        _ => String::new(),
    }
}

/// The parameter list of a function declarator, if any.
pub(crate) fn parameter_list(n: Node) -> Option<Node> {
    match n.kind() {
        "function_declarator" | "function_type_declarator" | "function_field_declarator" => field(n, "parameters"),
        "pointer_declarator" | "array_declarator" | "attributed_declarator" | "init_declarator" | "parenthesized_declarator" | "pointer_type_declarator" => {
            field(n, "declarator").or_else(|| named(n).into_iter().next()).and_then(parameter_list)
        }
        _ => None,
    }
}

/// The text of a parenthesized condition without its parentheses: `(x > 0)` → `x > 0`.
pub(crate) fn unparen(n: Node, src: &str) -> String {
    let t = text(n, src);
    if n.kind() == "parenthesized_expression" {
        t.strip_prefix('(').and_then(|s| s.strip_suffix(')')).map(|s| s.trim().to_string()).unwrap_or(t)
    } else {
        t
    }
}

/// Is a comment block the kind that documents what follows (`/** */`, `/*! */`, `///`, `//!`)?
fn is_doc_marker(first_line: &str) -> bool {
    let t = first_line.trim_start();
    (t.starts_with("/**") && !t.starts_with("/**/")) || t.starts_with("/*!") || t.starts_with("///") || t.starts_with("//!")
}

/// The comment block ending on `end` (0-based line index), as (first line index, lines).
fn comment_block_ending(lines: &[&str], end: usize) -> Option<(usize, Vec<String>)> {
    let last = lines.get(end)?.trim();
    if last.ends_with("*/") {
        // walk up to the `/*`
        let mut i = end;
        loop {
            if lines[i].contains("/*") {
                break;
            }
            if i == 0 {
                return None;
            }
            i -= 1;
        }
        let block: Vec<String> = lines[i..=end].iter().map(|l| l.to_string()).collect();
        return Some((i, block));
    }
    if last.starts_with("//") {
        let mut i = end;
        while i > 0 && lines[i - 1].trim().starts_with("//") {
            i -= 1;
        }
        let block: Vec<String> = lines[i..=end].iter().map(|l| l.to_string()).collect();
        return Some((i, block));
    }
    None
}

/// The text of a comment block without its markers: `/**`, `*/`, leading ` * `, `///`, `//`.
fn strip_comment(block: &[String]) -> String {
    let mut out = Vec::new();
    for (i, l) in block.iter().enumerate() {
        let mut t = l.trim();
        if i == 0 {
            t = t.trim_start_matches('/').trim_start_matches('*').trim_start_matches('!');
        } else if t.starts_with("//") {
            t = t.trim_start_matches('/').trim_start_matches('!');
        } else {
            t = t.trim_start_matches('*');
        }
        if i + 1 == block.len() {
            t = t.trim_end_matches("*/").trim_end_matches('*');
        }
        let t = t.strip_prefix(' ').unwrap_or(t).trim_end();
        out.push(t.to_string());
    }
    while out.first().is_some_and(|l| l.trim().is_empty()) {
        out.remove(0);
    }
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    out.join("\n")
}

/// The comment directly above `line` (1-based), documentation of the thing starting there.
/// Doc markers (`/** */`, `///`) always count; a plain `/* */` or `//` block counts when `plain_ok`
/// (a top-level definition: C has no other place to document one).
pub(crate) fn doc_above(lines: &[&str], line: usize, plain_ok: bool) -> Option<String> {
    if line < 2 {
        return None;
    }
    let (start, block) = comment_block_ending(lines, line - 2)?;
    let doc = is_doc_marker(&block[0]);
    if !doc && !plain_ok {
        return None;
    }
    // a one-line note that titles the statement below belongs to the flow, not to the docs
    let _ = start;
    let text = strip_comment(&block);
    (!text.is_empty()).then_some(text)
}

/// The comment at the top of a file that describes the file: `/** */`, `/*! */` or `//!` lines,
/// or any first comment followed by a blank line before the code.
pub(crate) fn file_doc(lines: &[&str]) -> Option<String> {
    let first = lines.iter().position(|l| !l.trim().is_empty())?;
    let t = lines[first].trim();
    if !(t.starts_with("/*") || t.starts_with("//")) {
        return None;
    }
    // find where the block ends
    let mut end = first;
    if t.starts_with("/*") {
        while end < lines.len() && !lines[end].contains("*/") {
            end += 1;
        }
        if end >= lines.len() {
            return None;
        }
    } else {
        while end + 1 < lines.len() && lines[end + 1].trim().starts_with("//") {
            end += 1;
        }
    }
    let block: Vec<String> = lines[first..=end].iter().map(|l| l.to_string()).collect();
    let marked = is_doc_marker(&block[0]);
    let blank_after = lines.get(end + 1).map(|l| l.trim().is_empty()).unwrap_or(true);
    if !marked && !blank_after {
        return None;
    }
    let text = strip_comment(&block);
    // a licence header is not a description
    let lower = text.to_lowercase();
    if lower.contains("copyright") || lower.contains("license") || lower.contains("licence") {
        return None;
    }
    (!text.is_empty()).then_some(text)
}

/// Identifiers read by an expression: locals, parameters, function names; not the field after
/// `.` / `->`, not types. `shallow` stops at nested `{ }` blocks.
pub(crate) fn idents(n: Node, src: &str, shallow: bool, out: &mut Vec<String>) {
    match n.kind() {
        "identifier" => {
            let s = raw(n, src).to_string();
            if !out.contains(&s) {
                out.push(s);
            }
        }
        "compound_statement" if shallow => {}
        _ => {
            for c in named(n) {
                idents(c, src, shallow, out);
            }
        }
    }
}

/// Does the subtree call one of `names` (`assert`, `abort`, …)?
pub(crate) fn calls_one_of(n: Node, src: &str, names: &[&str]) -> bool {
    if n.kind() == "call_expression" {
        if let Some(f) = field(n, "function") {
            if names.contains(&raw(f, src)) {
                return true;
            }
        }
    }
    named(n).into_iter().any(|c| calls_one_of(c, src, names))
}

/// Does the subtree hold a node of one of `kinds` (not descending into nested blocks)?
pub(crate) fn has_kind(n: Node, kinds: &[&str]) -> bool {
    if kinds.contains(&n.kind()) {
        return true;
    }
    if n.kind() == "compound_statement" {
        return false;
    }
    named(n).into_iter().any(|c| has_kind(c, kinds))
}
