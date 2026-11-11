//! The block tree of a C function (Code view): declarations, branches, `switch` cases as
//! arms, loops, jumps; locals attached to the blocks that read them.

use super::text::*;
use crate::blocks::Builder;
use crate::cfg::Facts;
use crate::model::*;
use tree_sitter::Node;

struct Driver<'s> {
    src: &'s str,
}

impl<'s> Driver<'s> {
    fn text(&self, n: Node) -> String {
        text(n, self.src)
    }

    fn stmt_text(&self, n: Node) -> String {
        self.text(n).trim_end_matches(';').trim_end().to_string()
    }

    fn facts(&self, n: Node) -> Facts {
        Facts { has_panic: calls_one_of(n, self.src, &["assert", "abort"]), has_return: has_kind(n, &["return_statement"]), ..Default::default() }
    }

    /// Locals in scope that `n` reads.
    fn uses(&self, b: &Builder, n: Node, shallow: bool) -> Vec<String> {
        let mut names = Vec::new();
        idents(n, self.src, shallow, &mut names);
        b.uses_in_scope(names)
    }

    /// Names a declaration introduces.
    fn declared(&self, n: Node) -> Vec<String> {
        let mut cur = n.walk();
        let names: Vec<String> = n.children_by_field_name("declarator", &mut cur).filter_map(|d| declarator_name(d, self.src)).collect();
        names
    }

    /// A `{ }` block, or a single statement standing for one.
    fn body(&self, b: &mut Builder, n: Node, out: &mut Vec<Block>) {
        if n.kind() == "compound_statement" {
            self.block(b, n, out);
        } else {
            b.push_scope(&[]);
            self.stmt(b, n, out);
            b.pop_scope();
        }
    }

    fn block(&self, b: &mut Builder, n: Node, out: &mut Vec<Block>) {
        b.push_scope(&[]);
        for c in named(n) {
            self.stmt(b, c, out);
        }
        b.pop_scope();
    }

    fn stmt(&self, b: &mut Builder, n: Node, out: &mut Vec<Block>) {
        let file = b.file();
        let sp = span(n, file, self.src);
        match n.kind() {
            "compound_statement" => self.block(b, n, out),
            "preproc_if" | "preproc_ifdef" | "preproc_else" | "preproc_elif" | "preproc_elifdef" => {
                for c in named(n) {
                    self.stmt(b, c, out);
                }
            }
            "attributed_statement" => {
                if let Some(s) = named(n).into_iter().find(|c| c.kind() != "attribute_declaration") {
                    self.stmt(b, s, out);
                }
            }
            "declaration" => {
                let defs = self.declared(n);
                let f = self.facts(n);
                let kind = b.leaf_kind(&f, &sp, BlockKind::Let);
                let text = self.stmt_text(n);
                let mut blk = b.new_block(kind, text.clone(), text, sp);
                let mut cur = n.walk();
                let mut names = Vec::new();
                for d in n.children_by_field_name("declarator", &mut cur) {
                    if let Some(v) = field(d, "value") {
                        idents(v, self.src, false, &mut names);
                    }
                }
                blk.uses = b.uses_in_scope(names);
                b.define(&defs);
                blk.defines = defs;
                out.push(blk);
            }
            "expression_statement" => {
                let Some(e) = named(n).into_iter().next() else { return };
                let f = self.facts(e);
                let kind = b.leaf_kind(&f, &sp, BlockKind::Stmt);
                let text = self.stmt_text(n);
                let mut blk = b.new_block(kind, text.clone(), text, sp);
                blk.uses = self.uses(b, e, false);
                out.push(blk);
            }
            "if_statement" => {
                let mut cur = n;
                let mut first = true;
                loop {
                    let cond_node = field(cur, "condition");
                    let cond = cond_node.map(|c| unparen(c, self.src)).unwrap_or_default();
                    let kind = if first { BlockKind::If } else { BlockKind::ElseIf };
                    let mut blk = b.new_block(kind, format!("{} {}", if first { "if" } else { "else if" }, cond), cond.clone(), span(cur, file, self.src));
                    if let Some(c) = cond_node {
                        blk.uses = self.uses(b, c, true);
                    }
                    let mut kids = Vec::new();
                    if let Some(t) = field(cur, "consequence") {
                        self.body(b, t, &mut kids);
                    }
                    blk.children = kids;
                    out.push(blk);
                    first = false;
                    let els = field(cur, "alternative").and_then(|a| named(a).into_iter().next());
                    match els {
                        Some(next) if next.kind() == "if_statement" => cur = next,
                        Some(other) => {
                            let mut eb = b.new_block(BlockKind::Else, "else".into(), "else".into(), span(other, file, self.src));
                            let mut kids = Vec::new();
                            self.body(b, other, &mut kids);
                            eb.children = kids;
                            out.push(eb);
                            break;
                        }
                        None => break,
                    }
                }
            }
            "switch_statement" => {
                let cond_node = field(n, "condition");
                let scrut = cond_node.map(|c| unparen(c, self.src)).unwrap_or_default();
                let mut blk = b.new_block(BlockKind::Match, format!("switch {}", scrut), scrut, sp);
                if let Some(c) = cond_node {
                    blk.uses = self.uses(b, c, true);
                }
                if let Some(body) = field(n, "body") {
                    for c in named(body).into_iter().filter(|c| c.kind() == "case_statement") {
                        let label = match field(c, "value") {
                            Some(v) => format!("case {}", self.text(v)),
                            None => "default".into(),
                        };
                        let mut ab = b.new_block(BlockKind::Arm, label.clone(), label, span(c, file, self.src));
                        let value = field(c, "value").map(|v| v.id());
                        b.push_scope(&[]);
                        let mut kids = Vec::new();
                        for s in named(c).into_iter().filter(|s| Some(s.id()) != value) {
                            self.stmt(b, s, &mut kids);
                        }
                        b.pop_scope();
                        ab.children = kids;
                        blk.children.push(ab);
                    }
                }
                out.push(blk);
            }
            "while_statement" | "do_statement" => {
                let cond_node = field(n, "condition");
                let cond = cond_node.map(|c| unparen(c, self.src)).unwrap_or_default();
                let label = if n.kind() == "while_statement" { format!("while {}", cond) } else { format!("do … while {}", cond) };
                let mut blk = b.new_block(BlockKind::Loop, label, cond, sp);
                if let Some(c) = cond_node {
                    blk.uses = self.uses(b, c, true);
                }
                let mut kids = Vec::new();
                if let Some(s) = field(n, "body") {
                    self.body(b, s, &mut kids);
                }
                blk.children = kids;
                out.push(blk);
            }
            "for_statement" => {
                let init = field(n, "initializer");
                let cond = field(n, "condition");
                let upd = field(n, "update");
                let label = format!("for ({}; {}; {})", init.map(|c| self.stmt_text(c)).unwrap_or_default(), cond.map(|c| self.text(c)).unwrap_or_default(), upd.map(|c| self.text(c)).unwrap_or_default());
                let mut blk = b.new_block(BlockKind::Loop, label.clone(), label, sp);
                let defs: Vec<String> = init.filter(|i| i.kind() == "declaration").map(|i| self.declared(i)).unwrap_or_default();
                let mut names = Vec::new();
                for part in [init, cond, upd].into_iter().flatten() {
                    idents(part, self.src, true, &mut names);
                }
                blk.uses = b.uses_in_scope(names.into_iter().filter(|x| !defs.contains(x)).collect());
                b.push_scope(&defs);
                let mut kids = Vec::new();
                if let Some(s) = field(n, "body") {
                    self.body(b, s, &mut kids);
                }
                b.pop_scope();
                blk.defines = defs;
                blk.children = kids;
                out.push(blk);
            }
            "return_statement" => {
                let text = self.stmt_text(n);
                let mut blk = b.new_block(BlockKind::Return, text.clone(), text, sp);
                if let Some(e) = named(n).into_iter().next() {
                    blk.uses = self.uses(b, e, false);
                }
                out.push(blk);
            }
            "break_statement" => out.push(b.new_block(BlockKind::Break, "break".into(), self.stmt_text(n), sp)),
            "continue_statement" => out.push(b.new_block(BlockKind::Continue, "continue".into(), self.stmt_text(n), sp)),
            "goto_statement" => {
                let text = self.stmt_text(n);
                out.push(b.new_block(BlockKind::Break, text.clone(), text, sp));
            }
            "labeled_statement" => {
                let label = field(n, "label").map(|l| raw(l, self.src).to_string()).unwrap_or_default();
                let mut blk = b.new_block(BlockKind::Stmt, format!("{}:", label), format!("{}:", label), sp);
                let mut kids = Vec::new();
                for c in named(n).into_iter().filter(|c| c.kind() != "statement_identifier") {
                    self.stmt(b, c, &mut kids);
                }
                blk.children = kids;
                out.push(blk);
            }
            "case_statement" => {
                let value = field(n, "value").map(|v| v.id());
                for s in named(n).into_iter().filter(|s| Some(s.id()) != value) {
                    self.stmt(b, s, out);
                }
            }
            _ => {
                let text = self.stmt_text(n);
                if !text.is_empty() {
                    let f = self.facts(n);
                    let kind = b.leaf_kind(&f, &sp, BlockKind::Stmt);
                    let mut blk = b.new_block(kind, text.clone(), text, sp);
                    blk.uses = self.uses(b, n, false);
                    out.push(blk);
                }
            }
        }
    }
}

/// The parameter names of a function definition.
fn param_names(fn_node: Node, src: &str) -> Vec<String> {
    let Some(list) = field(fn_node, "declarator").and_then(parameter_list) else { return vec![] };
    named(list).into_iter().filter_map(|p| match p.kind() {
        "parameter_declaration" => field(p, "declarator").and_then(|d| declarator_name(d, src)),
        "identifier" => Some(raw(p, src).to_string()),
        _ => None,
    }).collect()
}

/// Build the block tree of a C function definition.
pub fn build(fn_node: Node, file: FileId, calls: &[CallEdge], fn_span: Span, src: &str) -> Block {
    let params = param_names(fn_node, src);
    let mut b = Builder::new(file, calls, &params);
    let d = Driver { src };
    let mut kids = Vec::new();
    let body = field(fn_node, "body");
    if let Some(body) = body {
        d.block(&mut b, body, &mut kids);
    }
    let signature = match body {
        Some(bd) => clean(&src[fn_node.start_byte()..bd.start_byte()]),
        None => text(fn_node, src),
    };
    b.root(signature, fn_span, params, kids, src)
}
