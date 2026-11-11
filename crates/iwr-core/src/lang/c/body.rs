//! Calls and counts inside a C function body: `f(x)` resolved to the project function of that
//! name (same file first); calls through pointers (`p->cb(x)`) are not resolved.
//! C++: `x.m()` / `p->m()` / `this->m()` and a bare `m()` inside a method resolve to methods by
//! name (the heuristics of [`crate::resolve`]), `Type::f()` to that type's method, `ns::f()` to
//! the function, `Type(…)` and `new Type(…)` are constructions.

use super::text::*;
use crate::model::*;
use crate::resolve::{prefer_module, resolve_assoc, resolve_method, resolve_type, Index, Stats};
use std::collections::HashSet;
use tree_sitter::Node;

struct Visitor<'a> {
    p: &'a Project,
    ix: &'a Index,
    file: FileId,
    from: ItemId,
    module: ModuleId,
    src: &'a str,
    /// The type whose method this body is (C++).
    owner: Option<String>,
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

    /// A call by a bare name: a function, or inside a method another method of the same type,
    /// or a type being constructed.
    fn call_by_name(&mut self, name: String, span: Span) {
        if matches!(name.as_str(), "assert" | "abort") {
            self.stats.panics += 1;
        }
        let own = self.owner.as_deref().and_then(|o| resolve_assoc(self.p, self.ix, o, &name, self.module));
        if let Some(m) = own {
            self.push(Some(m), name, CallKind::MethodCall, span);
        } else if let Some(to) = self.ix.fns.get(&name).and_then(|c| prefer_module(self.p, self.ix, c, self.module)) {
            self.push(Some(to), name, CallKind::Call, span);
        } else if let Some(t) = resolve_type(self.p, self.ix, &name, self.module) {
            self.push(Some(t), name, CallKind::Construct, span);
        } else {
            self.push(None, name, CallKind::Call, span);
        }
    }

    fn visit(&mut self, n: Node) {
        match n.kind() {
            "call_expression" => {
                if let Some(f) = field(n, "function") {
                    let span = span(n, self.file, self.src);
                    match f.kind() {
                        "identifier" => self.call_by_name(raw(f, self.src).to_string(), span),
                        "template_function" => {
                            if let Some(name) = field(f, "name") {
                                self.call_by_name(raw(name, self.src).to_string(), span);
                            }
                        }
                        // `x.m()` / `p->m()`: a method when the project has one of that name
                        // (in C a call through a pointer, target unknown)
                        "field_expression" => {
                            let name = field(f, "field").map(|x| raw(x, self.src).to_string()).unwrap_or_default();
                            let recv = field(f, "argument");
                            let on_this = recv.is_some_and(|r| r.kind() == "this");
                            if let Some(to) = resolve_method(self.p, self.ix, &name, on_this, self.owner.as_deref(), self.module, &self.hints) {
                                let callee = format!("{}.{}", crate::shorten(&recv.map(|r| text(r, self.src)).unwrap_or_default(), 20), name);
                                self.push(Some(to), callee, CallKind::MethodCall, span);
                            }
                        }
                        // `Type::f()` / `ns::f()`
                        "qualified_identifier" => {
                            let callee = text(f, self.src);
                            let name = declarator_name(f, self.src).unwrap_or_default();
                            let scope = callee.rsplit("::").nth(1).unwrap_or("").split('<').next().unwrap_or("").to_string();
                            let to = resolve_assoc(self.p, self.ix, &scope, &name, self.module).or_else(|| self.ix.fns.get(&name).and_then(|c| prefer_module(self.p, self.ix, c, self.module)));
                            match to {
                                Some(to) => self.push(Some(to), callee, CallKind::Call, span),
                                None => match resolve_type(self.p, self.ix, &name, self.module) {
                                    Some(t) => self.push(Some(t), callee, CallKind::Construct, span),
                                    None => self.push(None, callee, CallKind::Call, span),
                                },
                            }
                        }
                        // `(*fp)(x)`, `table[i](x)`: a call through a pointer, unknown target
                        _ => {}
                    }
                }
                self.visit_all(n);
            }
            "new_expression" => {
                let ty = field(n, "type").map(|t| text(t, self.src)).unwrap_or_default();
                let name = ty.rsplit("::").next().unwrap_or("").split('<').next().unwrap_or("").to_string();
                if let Some(t) = resolve_type(self.p, self.ix, &name, self.module) {
                    self.push(Some(t), format!("new {}", ty), CallKind::Construct, span(n, self.file, self.src));
                }
                self.visit_all(n);
            }
            "throw_statement" => {
                self.stats.panics += 1;
                self.visit_all(n);
            }
            "if_statement" => {
                self.stats.branches += 1;
                if let Some(c) = field(n, "condition") {
                    self.visit(c);
                }
                self.branch_depth += 1;
                if let Some(c) = field(n, "consequence") {
                    self.visit(c);
                }
                if let Some(a) = field(n, "alternative") {
                    self.visit_all(a);
                }
                self.branch_depth -= 1;
            }
            "switch_statement" => {
                self.stats.branches += 1;
                if let Some(c) = field(n, "condition") {
                    self.visit(c);
                }
                self.branch_depth += 1;
                if let Some(b) = field(n, "body") {
                    self.visit(b);
                }
                self.branch_depth -= 1;
            }
            "while_statement" | "do_statement" | "for_statement" | "for_range_loop" => {
                self.stats.loops += 1;
                for c in named(n) {
                    if field(n, "body").is_some_and(|b| b.id() == c.id()) {
                        self.loop_depth += 1;
                        self.visit(c);
                        self.loop_depth -= 1;
                    } else {
                        self.visit(c);
                    }
                }
            }
            "function_definition" => {
                // a nested function (GNU extension) is not part of this body's flow
            }
            _ => self.visit_all(n),
        }
    }
}

/// The calls of a function definition and the counts shown in its summary.
pub(crate) fn analyze(p: &Project, ix: &Index, item: ItemId, file: FileId, fn_node: Node, src: &str) -> (Vec<CallEdge>, Stats) {
    let owner = p.items[item].fn_info().and_then(|f| f.owner_type_name.clone());
    let mut hints = HashSet::new();
    type_names_in(fn_node, src, &mut hints);
    let mut v = Visitor { p, ix, file, from: item, module: p.items[item].module, src, owner, hints, loop_depth: 0, branch_depth: 0, edges: vec![], stats: Stats::default() };
    if let Some(body) = field(fn_node, "body") {
        v.visit(body);
    }
    (v.edges, v.stats)
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
