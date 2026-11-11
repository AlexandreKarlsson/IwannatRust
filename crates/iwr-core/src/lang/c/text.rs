//! Reading a tree-sitter C tree: declarator names and decorations, conditions, the identifiers
//! an expression reads. The grammar-independent helpers are in [`crate::lang::ts`].

pub(crate) use crate::lang::ts::*;
use tree_sitter::Node;

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
