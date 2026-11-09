//! Calls and counts inside a C function body: `f(x)` resolved to the project function of that
//! name (same file first); calls through pointers (`p->cb(x)`) are not resolved.

use super::text::*;
use crate::model::*;
use crate::resolve::{prefer_module, Index, Stats};
use tree_sitter::Node;

struct Visitor<'a> {
    p: &'a Project,
    ix: &'a Index,
    file: FileId,
    from: ItemId,
    module: ModuleId,
    src: &'a str,
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

    fn visit(&mut self, n: Node) {
        match n.kind() {
            "call_expression" => {
                if let Some(f) = field(n, "function") {
                    let span = span(n, self.file, self.src);
                    if f.kind() == "identifier" {
                        let name = raw(f, self.src).to_string();
                        if matches!(name.as_str(), "assert" | "abort") {
                            self.stats.panics += 1;
                        }
                        let to = self.ix.fns.get(&name).and_then(|c| prefer_module(self.p, self.ix, c, self.module));
                        self.push(to, name, CallKind::Call, span);
                    }
                    // `p->cb(x)`, `(*fp)(x)`, `table[i](x)`: a call through a pointer, unknown target
                }
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
            "while_statement" | "do_statement" | "for_statement" => {
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
    let mut v = Visitor { p, ix, file, from: item, module: p.items[item].module, src, loop_depth: 0, branch_depth: 0, edges: vec![], stats: Stats::default() };
    if let Some(body) = field(fn_node, "body") {
        v.visit(body);
    }
    (v.edges, v.stats)
}
