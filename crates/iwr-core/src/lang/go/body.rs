//! Calls and counts inside a Go function body. `f(x)` is the function of that name (same
//! package first); `pkg.F(x)` goes through the file's imports to a package of the project;
//! `x.M()` is a method found by name, the receiver's own type first, then the types the
//! function names; `T{…}` is a construction. Builtins (`len`, `append`, …) are not calls.

use super::{imports, local_import, type_name};
use crate::lang::ts::*;
use crate::model::*;
use crate::resolve::{prefer_module, resolve_method, resolve_type, Index, Stats};
use std::collections::{HashMap, HashSet};
use tree_sitter::Node;

/// Predeclared functions, and the predeclared types a conversion `int(x)` names.
const BUILTINS: &[&str] = &[
    "append", "cap", "clear", "close", "complex", "copy", "delete", "imag", "len", "make", "max", "min", "new", "print", "println", "real", "recover",
    "bool", "byte", "complex64", "complex128", "error", "float32", "float64", "int", "int8", "int16", "int32", "int64", "rune", "string", "uint", "uint8", "uint16", "uint32", "uint64", "uintptr", "any",
];

struct Visitor<'a> {
    p: &'a Project,
    ix: &'a Index,
    file: FileId,
    from: ItemId,
    module: ModuleId,
    src: &'a str,
    /// The receiver's type and the name it has in the body.
    owner: Option<String>,
    receiver: Option<String>,
    /// Imported package name → its module in the project (`None`: a package from outside).
    packages: HashMap<String, Option<String>>,
    /// Type names written in the function: hints for which type a method call is on.
    hints: HashSet<String>,
    loop_depth: usize,
    branch_depth: usize,
    edges: Vec<CallEdge>,
    stats: Stats,
}

impl<'a> Visitor<'a> {
    fn push(&mut self, to: Option<ItemId>, callee: String, kind: CallKind, span: Span) {
        self.edges.push(CallEdge {
            from: self.from,
            to,
            callee,
            kind,
            span,
            in_loop: self.loop_depth > 0,
            in_branch: self.branch_depth > 0,
            propagated: false,
            unwrapped: false,
            is_await: false,
            is_recursive: to == Some(self.from),
        });
    }

    fn visit_all(&mut self, n: Node) {
        for c in named(n) {
            self.visit(c);
        }
    }

    fn call(&mut self, n: Node, f: Node) {
        let span = span(n, self.file, self.src);
        match f.kind() {
            "identifier" => {
                let name = raw(f, self.src).to_string();
                if name == "panic" {
                    self.stats.panics += 1;
                    self.push(None, name, CallKind::Call, span);
                } else if BUILTINS.contains(&name.as_str()) {
                    // part of the language
                } else if let Some(to) = self.ix.fns.get(&name).and_then(|c| prefer_module(self.p, self.ix, c, self.module)) {
                    self.push(Some(to), name, CallKind::Call, span);
                } else if resolve_type(self.p, self.ix, &name, self.module).is_none() {
                    // not a conversion `T(x)`: a function value, a closure
                    self.push(None, name, CallKind::Call, span);
                }
            }
            "selector_expression" => {
                let name = field(f, "field").map(|x| raw(x, self.src).to_string()).unwrap_or_default();
                let operand = field(f, "operand");
                let on = operand.filter(|o| o.kind() == "identifier").map(|o| raw(o, self.src).to_string());
                let callee = format!("{}.{}", crate::shorten(&operand.map(|o| raw(o, self.src)).unwrap_or_default().split_whitespace().collect::<String>(), 20), name);
                // `pkg.F()`: a package of the project, or one from outside
                if let Some(pkg) = on.as_ref().and_then(|o| self.packages.get(o)) {
                    let to = pkg.as_ref().and_then(|m| self.ix.paths.get(&format!("{}::{}", m, name)).copied());
                    match to {
                        Some(t) if self.p.items[t].is_callable() => self.push(Some(t), callee, CallKind::Call, span),
                        Some(_) => {} // a conversion to a type of that package
                        None => self.push(None, callee, CallKind::Call, span),
                    }
                    return;
                }
                let on_receiver = on.is_some() && on == self.receiver;
                if let Some(to) = resolve_method(self.p, self.ix, &name, on_receiver, self.owner.as_deref(), self.module, &self.hints) {
                    self.push(Some(to), callee, CallKind::MethodCall, span);
                }
            }
            _ => {}
        }
    }

    fn visit(&mut self, n: Node) {
        match n.kind() {
            "call_expression" => {
                if let Some(f) = field(n, "function") {
                    self.call(n, f);
                }
                self.visit_all(n);
            }
            "composite_literal" => {
                if let Some(name) = field(n, "type").and_then(|t| type_name(t, self.src)) {
                    if let Some(t) = resolve_type(self.p, self.ix, &name, self.module) {
                        let callee = field(n, "type").map(|t| raw(t, self.src).to_string()).unwrap_or(name);
                        self.push(Some(t), callee, CallKind::Construct, span(n, self.file, self.src));
                    }
                }
                self.visit_all(n);
            }
            "if_statement" => {
                self.stats.branches += 1;
                for part in ["initializer", "condition"] {
                    if let Some(c) = field(n, part) {
                        self.visit(c);
                    }
                }
                self.branch_depth += 1;
                for part in ["consequence", "alternative"] {
                    if let Some(c) = field(n, part) {
                        self.visit(c);
                    }
                }
                self.branch_depth -= 1;
            }
            "expression_switch_statement" | "type_switch_statement" | "select_statement" => {
                self.stats.branches += 1;
                for c in named(n) {
                    let is_case = matches!(c.kind(), "expression_case" | "type_case" | "communication_case" | "default_case");
                    self.branch_depth += is_case as usize;
                    self.visit(c);
                    self.branch_depth -= is_case as usize;
                }
            }
            "for_statement" => {
                self.stats.loops += 1;
                for c in named(n) {
                    let is_body = c.kind() == "block";
                    self.loop_depth += is_body as usize;
                    self.visit(c);
                    self.loop_depth -= is_body as usize;
                }
            }
            _ => self.visit_all(n),
        }
    }
}

/// Every type name written under `n`.
fn type_names_in(n: Node, src: &str, out: &mut HashSet<String>) {
    if n.kind() == "type_identifier" {
        out.insert(raw(n, src).to_string());
    }
    for c in named(n) {
        type_names_in(c, src, out);
    }
}

/// The calls of a function or method declaration and the counts shown in its summary.
pub(crate) fn analyze(p: &Project, ix: &Index, item: ItemId, file: FileId, fn_node: Node, src: &str) -> (Vec<CallEdge>, Stats) {
    let mods = super::go_modules(p);
    let packages = imports(fn_node, src).into_iter().map(|(name, path)| (name, local_import(&mods, &path))).collect();
    let receiver = field(fn_node, "receiver").and_then(|r| named(r).into_iter().next()).and_then(|r| field(r, "name")).map(|x| raw(x, src).to_string());
    let owner = p.items[item].fn_info().and_then(|f| f.owner_type_name.clone());
    let mut hints = HashSet::new();
    type_names_in(fn_node, src, &mut hints);
    let mut v = Visitor { p, ix, file, from: item, module: p.items[item].module, src, owner, receiver, packages, hints, loop_depth: 0, branch_depth: 0, edges: vec![], stats: Stats::default() };
    if let Some(body) = field(fn_node, "body") {
        v.visit(body);
    }
    (v.edges, v.stats)
}
