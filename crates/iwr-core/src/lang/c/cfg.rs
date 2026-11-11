//! The flow graph of a C function: statements into the language-neutral [`Builder`].
//! `switch` cases fall through unless they `break`; `goto` jumps to its label's node.

use super::text::*;
use crate::cfg::{Builder, Facts};
use crate::model::*;
use crate::shorten;
use std::cell::RefCell;
use std::collections::HashMap;
use tree_sitter::Node;

struct Driver<'s> {
    src: &'s str,
    /// Label name → the node that marks it (`cleanup:`).
    labels: RefCell<HashMap<String, usize>>,
    /// `goto` nodes waiting for their label: (jump node, label).
    gotos: RefCell<Vec<(usize, String)>>,
}

/// The edge label for a `return`, and whether it is an error path (`return -1`, `return NULL`).
fn return_label(expr: Option<&str>) -> (String, bool) {
    let Some(t) = expr else { return ("return".into(), false) };
    let is_err = t == "NULL" || t.starts_with('-') || t == "false" || t == "EXIT_FAILURE";
    if t.len() <= 12 && !t.contains('(') {
        (format!("return {}", t), is_err)
    } else {
        ("return".into(), is_err)
    }
}

/// `while (1)` / `for (;;)`: no exit but a `break`.
fn is_forever(cond: Option<&str>) -> bool {
    matches!(cond, None | Some("1") | Some("true"))
}

impl<'s> Driver<'s> {
    fn text(&self, n: Node) -> String {
        text(n, self.src)
    }

    /// Statement text without its `;`.
    fn stmt_text(&self, n: Node) -> String {
        self.text(n).trim_end_matches(';').trim_end().to_string()
    }

    fn block(&self, b: &mut Builder, n: Node) {
        for c in named(n) {
            self.stmt(b, c);
        }
    }

    fn stmt(&self, b: &mut Builder, n: Node) {
        b.note_for(n.start_position().row + 1);
        self.stmt_inner(b, n);
        b.clear_note();
    }

    fn facts(&self, n: Node) -> Facts {
        Facts { has_panic: calls_one_of(n, self.src, &["assert"]), ..Default::default() }
    }

    fn stmt_inner(&self, b: &mut Builder, n: Node) {
        let file = b.file();
        let sp = span(n, file, self.src);
        match n.kind() {
            "compound_statement" | "preproc_if" | "preproc_ifdef" | "preproc_else" | "preproc_elif" | "preproc_elifdef" => self.block(b, n),
            "attributed_statement" => {
                if let Some(s) = named(n).into_iter().find(|c| c.kind() != "attribute_declaration") {
                    self.stmt_inner(b, s);
                }
            }
            "declaration" => {
                let f = Facts { is_let: true, ..self.facts(n) };
                let text = self.stmt_text(n);
                b.plain(&text, text.clone(), sp, f);
            }
            "expression_statement" => {
                let Some(e) = named(n).into_iter().next() else { return };
                let text = self.stmt_text(n);
                if e.kind() == "call_expression" && field(e, "function").is_some_and(|f| raw(f, self.src) == "assert") {
                    b.panic_node(text, sp, "assert fails", false);
                    return;
                }
                let f = Facts { is_mutation: matches!(e.kind(), "assignment_expression" | "update_expression"), ..self.facts(e) };
                b.plain(&text, text.clone(), sp, f);
            }
            "if_statement" => {
                let cond_node = field(n, "condition");
                let cond = cond_node.map(|c| unparen(c, self.src)).unwrap_or_default();
                let cond_span = cond_node.map(|c| span(c, file, self.src)).unwrap_or(sp);
                let then = field(n, "consequence");
                let els = field(n, "alternative").and_then(|a| named(a).into_iter().next());
                b.if_node(&cond, cond_span, |b| if let Some(t) = then { self.stmt(b, t) }, els.map(|e| move |b: &mut Builder| self.stmt(b, e)));
            }
            "switch_statement" => {
                let cond_node = field(n, "condition");
                let scrut = cond_node.map(|c| unparen(c, self.src)).unwrap_or_default();
                let scrut_span = cond_node.map(|c| span(c, file, self.src)).unwrap_or(sp);
                let cases: Vec<Node> = field(n, "body").map(|body| named(body).into_iter().filter(|c| c.kind() == "case_statement").collect()).unwrap_or_default();
                let labels: Vec<String> = cases.iter().map(|c| self.case_label(*c)).collect();
                let bodies: Vec<Vec<Node>> = cases.iter().map(|c| self.case_body(*c)).collect();
                b.switch_node(&scrut, scrut_span, &labels, |b, i| {
                    for s in &bodies[i] {
                        self.stmt(b, *s);
                    }
                });
            }
            "while_statement" => {
                let cond_node = field(n, "condition");
                let cond = cond_node.map(|c| unparen(c, self.src)).unwrap_or_default();
                let head = cond_node.map(|c| span(c, file, self.src)).unwrap_or(sp);
                let body = field(n, "body");
                b.loop_node(&format!("while {}", cond), head, is_forever(Some(&cond)), |b| if let Some(s) = body { self.stmt(b, s) });
            }
            "do_statement" => {
                let cond_node = field(n, "condition");
                let cond = cond_node.map(|c| unparen(c, self.src)).unwrap_or_default();
                let head = cond_node.map(|c| span(c, file, self.src)).unwrap_or(sp);
                let body = field(n, "body");
                b.loop_node(&format!("do … while {}", cond), head, is_forever(Some(&cond)), |b| if let Some(s) = body { self.stmt(b, s) });
            }
            "for_statement" => {
                let init = field(n, "initializer").map(|c| self.stmt_text(c)).unwrap_or_default();
                let cond = field(n, "condition").map(|c| self.text(c));
                let upd = field(n, "update").map(|c| self.text(c)).unwrap_or_default();
                let label = format!("for ({}; {}; {})", init, cond.clone().unwrap_or_default(), upd);
                let body = field(n, "body");
                // the head: from `for` to the body
                let head = match body {
                    Some(bd) => Span { line_end: bd.start_position().row + 1, col_end: span(bd, file, self.src).col_start, ..sp },
                    None => sp,
                };
                b.loop_node(&label, head, is_forever(cond.as_deref()), |b| if let Some(s) = body { self.stmt(b, s) });
            }
            "return_statement" => {
                let expr = named(n).into_iter().next().map(|e| self.text(e));
                let (what, is_err) = return_label(expr.as_deref());
                b.return_node(is_err, self.stmt_text(n), sp, &what);
            }
            "break_statement" => b.break_node(self.stmt_text(n), sp),
            "continue_statement" => b.continue_node(self.stmt_text(n), sp),
            "goto_statement" => {
                let label = field(n, "label").map(|l| raw(l, self.src).to_string()).unwrap_or_default();
                let id = b.jump_node(&format!("goto {}", label), self.stmt_text(n), sp);
                self.gotos.borrow_mut().push((id, label));
            }
            "labeled_statement" => {
                let label = field(n, "label").map(|l| raw(l, self.src).to_string()).unwrap_or_default();
                let mark = span(field(n, "label").unwrap_or(n), file, self.src);
                let id = b.seq(CfgNodeKind::Block, format!("{}:", label), format!("{}:", label), mark);
                self.labels.borrow_mut().insert(label, id);
                for c in named(n).into_iter().filter(|c| c.kind() != "statement_identifier") {
                    self.stmt(b, c);
                }
            }
            "case_statement" => {
                // a case outside a switch body (after a preprocessor branch): its statements
                for s in self.case_body(n) {
                    self.stmt(b, s);
                }
            }
            _ => {
                let text = self.stmt_text(n);
                if !text.is_empty() {
                    b.plain(&text, text.clone(), sp, self.facts(n));
                }
            }
        }
    }

    /// `case 3` / `default`.
    fn case_label(&self, c: Node) -> String {
        match field(c, "value") {
            Some(v) => format!("case {}", shorten(&self.text(v), 20)),
            None => "default".into(),
        }
    }

    /// The statements of a case, its value left out.
    fn case_body<'t>(&self, c: Node<'t>) -> Vec<Node<'t>> {
        let value = field(c, "value").map(|v| v.id());
        named(c).into_iter().filter(|s| Some(s.id()) != value).collect()
    }
}

/// Build the CFG of a C function definition. `calls` are its (already resolved) call edges.
pub fn build(fn_node: Node, file: FileId, calls: &[CallEdge], fn_span: Span, src: &str) -> Cfg {
    let d = Driver { src, labels: RefCell::new(HashMap::new()), gotos: RefCell::new(vec![]) };
    let mut b = Builder::new(file, calls, fn_span, src);
    if let Some(body) = field(fn_node, "body") {
        d.block(&mut b, body);
    }
    // `goto`s: forward jumps leave like a break, backward ones loop like a continue
    let labels = d.labels.borrow();
    for (from, label) in d.gotos.borrow().iter() {
        if let Some(&to) = labels.get(label) {
            let kind = if to < *from { CfgEdgeKind::Continue } else { CfgEdgeKind::Break };
            b.link(*from, to, kind, Some(format!("goto {}", label)));
        }
    }
    b.finish(None)
}
