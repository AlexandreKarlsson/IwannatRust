//! The block tree of a Go function (Code view): definitions, branches, `switch` / `select`
//! cases as arms, loops, jumps; locals attached to the blocks that read them.

use super::cfg::{called, case_body, case_label, cases, exit_code, head};
use super::{idents, line, stmts};
use crate::blocks::Builder;
use crate::cfg::Facts;
use crate::lang::ts::*;
use crate::model::*;
use tree_sitter::Node;

struct Driver<'s> {
    src: &'s str,
}

/// Does the subtree hold a `return` (not looking inside function literals)?
fn has_return(n: Node) -> bool {
    match n.kind() {
        "return_statement" => true,
        "func_literal" => false,
        _ => named(n).into_iter().any(has_return),
    }
}

impl<'s> Driver<'s> {
    fn facts(&self, n: Node) -> Facts {
        let stops = called(n, self.src).as_deref() == Some("panic") || exit_code(n, self.src).is_some();
        Facts { has_panic: stops, has_return: has_return(n), ..Default::default() }
    }

    /// Locals in scope that `n` reads.
    fn uses(&self, b: &Builder, n: Node, shallow: bool) -> Vec<String> {
        let mut names = Vec::new();
        idents(n, self.src, shallow, &mut names);
        b.uses_in_scope(names)
    }

    /// The names on the left of `:=` (or of a `var` spec), `_` left out.
    fn declared(&self, n: Node) -> Vec<String> {
        let mut names = Vec::new();
        match n.kind() {
            "short_var_declaration" | "range_clause" | "receive_statement" => {
                if let Some(l) = field(n, "left") {
                    idents(l, self.src, true, &mut names);
                }
            }
            "var_declaration" | "const_declaration" => {
                for spec in named(n).into_iter().flat_map(|c| if c.kind() == "var_spec_list" { named(c) } else { vec![c] }) {
                    let mut cur = spec.walk();
                    names.extend(spec.children_by_field_name("name", &mut cur).map(|x| raw(x, self.src).to_string()));
                }
            }
            _ => {}
        }
        names.retain(|x| x != "_");
        names
    }

    fn block(&self, b: &mut Builder, n: Node, out: &mut Vec<Block>) {
        b.push_scope(&[]);
        for c in stmts(n) {
            self.stmt(b, c, out);
        }
        b.pop_scope();
    }

    /// A definition (`x := f()`, `var x = …`): what it defines, what its right side reads.
    fn definition(&self, b: &mut Builder, n: Node, out: &mut Vec<Block>) {
        let sp = span(n, b.file(), self.src);
        let defs = self.declared(n);
        let kind = b.leaf_kind(&self.facts(n), &sp, BlockKind::Let);
        let text = line(n, self.src);
        let mut blk = b.new_block(kind, text.clone(), text, sp);
        let mut names = Vec::new();
        match field(n, "right") {
            Some(r) => idents(r, self.src, false, &mut names),
            None => idents(n, self.src, false, &mut names),
        }
        blk.uses = b.uses_in_scope(names.into_iter().filter(|x| !defs.contains(x)).collect());
        b.define(&defs);
        blk.defines = defs;
        out.push(blk);
    }

    fn stmt(&self, b: &mut Builder, n: Node, out: &mut Vec<Block>) {
        let file = b.file();
        let sp = span(n, file, self.src);
        match n.kind() {
            "block" => self.block(b, n, out),
            "short_var_declaration" | "var_declaration" | "const_declaration" => self.definition(b, n, out),
            "if_statement" => {
                // the names of an `if x := f(); …` live as long as the whole chain
                b.push_scope(&[]);
                let mut cur = n;
                let mut first = true;
                let mut scopes = 1;
                loop {
                    if let Some(init) = field(cur, "initializer") {
                        if !first {
                            b.push_scope(&[]);
                            scopes += 1;
                        }
                        self.stmt(b, init, out);
                    }
                    let cond_node = field(cur, "condition");
                    let cond = cond_node.map(|c| line(c, self.src)).unwrap_or_default();
                    let kind = if first { BlockKind::If } else { BlockKind::ElseIf };
                    let mut blk = b.new_block(kind, format!("{} {}", if first { "if" } else { "else if" }, cond), cond.clone(), span(cur, file, self.src));
                    if let Some(c) = cond_node {
                        blk.uses = self.uses(b, c, true);
                    }
                    let mut kids = Vec::new();
                    if let Some(t) = field(cur, "consequence") {
                        self.block(b, t, &mut kids);
                    }
                    blk.children = kids;
                    out.push(blk);
                    first = false;
                    match field(cur, "alternative") {
                        Some(next) if next.kind() == "if_statement" => cur = next,
                        Some(other) => {
                            let mut eb = b.new_block(BlockKind::Else, "else".into(), "else".into(), span(other, file, self.src));
                            let mut kids = Vec::new();
                            self.block(b, other, &mut kids);
                            eb.children = kids;
                            out.push(eb);
                            break;
                        }
                        None => break,
                    }
                }
                for _ in 0..scopes {
                    b.pop_scope();
                }
            }
            "for_statement" => {
                let body = field(n, "body");
                let label = head(n, body, self.src);
                let mut blk = b.new_block(BlockKind::Loop, label.clone(), label, sp);
                let clause = named(n).into_iter().find(|c| body.is_none_or(|bd| bd.id() != c.id()));
                // what the head defines: the `range` variables, the `i := 0` of a three-part head
                let defs: Vec<String> = match clause {
                    Some(c) if c.kind() == "range_clause" => self.declared(c),
                    Some(c) if c.kind() == "for_clause" => field(c, "initializer").map(|i| self.declared(i)).unwrap_or_default(),
                    _ => vec![],
                };
                if let Some(c) = clause {
                    let mut names = Vec::new();
                    idents(c, self.src, true, &mut names);
                    blk.uses = b.uses_in_scope(names.into_iter().filter(|x| !defs.contains(x)).collect());
                }
                b.push_scope(&defs);
                let mut kids = Vec::new();
                if let Some(s) = body {
                    self.block(b, s, &mut kids);
                }
                b.pop_scope();
                blk.defines = defs;
                blk.children = kids;
                out.push(blk);
            }
            "expression_switch_statement" | "type_switch_statement" | "select_statement" => {
                b.push_scope(&[]);
                if let Some(init) = field(n, "initializer") {
                    self.stmt(b, init, out);
                }
                let cs = cases(n);
                let label = match cs.first() {
                    Some(c) => one_line(self.src[n.start_byte()..c.start_byte()].trim_end().trim_end_matches('{')),
                    None => line(n, self.src),
                };
                let mut blk = b.new_block(BlockKind::Match, label.clone(), label, sp);
                // `switch t := v.(type)`: `t` is defined for the cases
                let alias: Vec<String> = field(n, "alias").map(|a| {
                    let mut v = Vec::new();
                    idents(a, self.src, true, &mut v);
                    v
                }).unwrap_or_default();
                if let Some(v) = field(n, "value") {
                    blk.uses = self.uses(b, v, true);
                }
                blk.defines = alias.clone();
                for c in cs {
                    let label = case_label(c, self.src);
                    let mut ab = b.new_block(BlockKind::Arm, label.clone(), label, span(c, file, self.src));
                    let mut defs = alias.clone();
                    if let Some(comm) = field(c, "communication") {
                        defs.extend(self.declared(comm));
                        ab.uses = self.uses(b, comm, true).into_iter().filter(|x| !defs.contains(x)).collect();
                    } else if let Some(v) = field(c, "value") {
                        ab.uses = self.uses(b, v, true);
                    }
                    b.push_scope(&defs);
                    let mut kids = Vec::new();
                    for s in case_body(c) {
                        self.stmt(b, s, &mut kids);
                    }
                    b.pop_scope();
                    ab.children = kids;
                    blk.children.push(ab);
                }
                b.pop_scope();
                out.push(blk);
            }
            "return_statement" => {
                let text = line(n, self.src);
                let mut blk = b.new_block(BlockKind::Return, text.clone(), text, sp);
                blk.uses = self.uses(b, n, false);
                out.push(blk);
            }
            "break_statement" => out.push(b.new_block(BlockKind::Break, line(n, self.src), line(n, self.src), sp)),
            "continue_statement" => out.push(b.new_block(BlockKind::Continue, line(n, self.src), line(n, self.src), sp)),
            "goto_statement" => out.push(b.new_block(BlockKind::Break, line(n, self.src), line(n, self.src), sp)),
            "labeled_statement" => {
                let label = field(n, "label").map(|l| raw(l, self.src).to_string()).unwrap_or_default();
                let mut blk = b.new_block(BlockKind::Stmt, format!("{}:", label), format!("{}:", label), sp);
                let mut kids = Vec::new();
                for c in named(n).into_iter().filter(|c| c.kind() != "label_name") {
                    self.stmt(b, c, &mut kids);
                }
                blk.children = kids;
                out.push(blk);
            }
            "empty_statement" => {}
            _ => {
                let text = line(n, self.src);
                if !text.is_empty() {
                    let kind = b.leaf_kind(&self.facts(n), &sp, BlockKind::Stmt);
                    let mut blk = b.new_block(kind, text.clone(), text, sp);
                    blk.uses = self.uses(b, n, false);
                    out.push(blk);
                }
            }
        }
    }
}

/// The names a function's body starts with: its receiver and parameters.
fn param_names(fn_node: Node, src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for list in [field(fn_node, "receiver"), field(fn_node, "parameters")].into_iter().flatten() {
        for p in named(list) {
            let mut cur = p.walk();
            out.extend(p.children_by_field_name("name", &mut cur).map(|x| raw(x, src).to_string()));
        }
    }
    out.retain(|x| x != "_");
    out
}

/// Build the block tree of a Go function or method declaration.
pub fn build(fn_node: Node, file: FileId, calls: &[CallEdge], fn_span: Span, src: &str) -> Block {
    let params = param_names(fn_node, src);
    let mut b = Builder::new(file, calls, &params);
    let d = Driver { src };
    let mut kids = Vec::new();
    let body = field(fn_node, "body");
    if let Some(body) = body {
        d.block(&mut b, body, &mut kids);
    }
    b.root(head(fn_node, body, src), fn_span, params, kids, src)
}
