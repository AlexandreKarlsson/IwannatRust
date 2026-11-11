//! The block tree of a shell function or script (Code view): assignments, `if` chains, `case`
//! items as arms, loops, the commands that leave; variables attached to the blocks that read them.

use super::cfg::{case_body, case_label, guarded, head, inner_command, leaves};
use super::{arguments, command_name, line, list_operator, note_above, statements};
use crate::blocks::Builder;
use crate::cfg::Facts;
use crate::lang::ts::*;
use crate::model::*;
use tree_sitter::Node;

struct Driver<'s> {
    src: &'s str,
    lines: Vec<&'s str>,
}

/// The variables `n` reads (`$x`, `${x}`, names inside `$(( ))`), not the ones it sets.
fn reads(n: Node, src: &str, out: &mut Vec<String>) {
    match n.kind() {
        "variable_name" => {
            let s = raw(n, src).to_string();
            if !out.contains(&s) {
                out.push(s);
            }
        }
        "variable_assignment" => {
            let name = field(n, "name").map(|x| x.id());
            for c in named(n).into_iter().filter(|c| Some(c.id()) != name) {
                reads(c, src, out);
            }
        }
        _ => {
            for c in named(n) {
                reads(c, src, out);
            }
        }
    }
}

impl<'s> Driver<'s> {
    fn facts(&self, n: Node) -> Facts {
        let left = leaves(n, self.src);
        Facts { has_return: left.as_deref() == Some("return"), has_panic: left.as_deref() == Some("exit"), ..Default::default() }
    }

    fn uses(&self, b: &Builder, n: Node) -> Vec<String> {
        let mut names = Vec::new();
        reads(n, self.src, &mut names);
        b.uses_in_scope(names)
    }

    /// The variables a statement sets: `x=1`, `local a b=2`, `read line`.
    fn sets(&self, n: Node) -> Vec<String> {
        match n.kind() {
            "variable_assignment" => field(n, "name").map(|x| vec![raw(x, self.src).to_string()]).unwrap_or_default(),
            "declaration_command" => named(n).into_iter().filter_map(|c| match c.kind() {
                "variable_assignment" => field(c, "name").map(|x| raw(x, self.src).to_string()),
                "variable_name" => Some(raw(c, self.src).to_string()),
                _ => None,
            }).collect(),
            _ => match inner_command(n) {
                Some(cmd) if command_name(cmd, self.src) == "read" => arguments(cmd, self.src).into_iter().filter(|a| !a.starts_with('-')).collect(),
                _ => vec![],
            },
        }
    }

    /// A block with the `#` comment above it as its note.
    fn new_block(&self, b: &mut Builder, kind: BlockKind, label: String, n: Node) -> Block {
        let mut blk = b.new_block(kind, label.clone(), label, span(n, b.file(), self.src));
        blk.note = note_above(&self.lines, n.start_position().row + 1);
        blk
    }

    fn block(&self, b: &mut Builder, stmts: &[Node], out: &mut Vec<Block>) {
        for s in stmts {
            self.stmt(b, *s, out);
        }
    }

    /// One statement that is not a compound: a command, an assignment, a pipeline.
    fn leaf(&self, b: &mut Builder, n: Node, out: &mut Vec<Block>) {
        let text = line(n, self.src);
        if text.is_empty() {
            return;
        }
        let sp = span(n, b.file(), self.src);
        let sets = self.sets(n);
        let default = if sets.is_empty() { BlockKind::Stmt } else { BlockKind::Let };
        let kind = match leaves(n, self.src).as_deref() {
            Some("break") => BlockKind::Break,
            Some("continue") => BlockKind::Continue,
            _ => b.leaf_kind(&self.facts(n), &sp, default),
        };
        let mut blk = self.new_block(b, kind, text, n);
        blk.uses = self.uses(b, n);
        b.define(&sets);
        blk.defines = sets;
        out.push(blk);
    }

    fn stmt(&self, b: &mut Builder, n: Node, out: &mut Vec<Block>) {
        match n.kind() {
            "if_statement" => {
                let mut clause = n;
                let mut first = true;
                let rest: Vec<Node> = named(n).into_iter().filter(|c| matches!(c.kind(), "elif_clause" | "else_clause")).collect();
                let mut rest = rest.into_iter();
                loop {
                    let (conds, then) = guarded(clause);
                    let cond = conds.iter().map(|c| line(*c, self.src)).collect::<Vec<_>>().join("; ");
                    let kind = if first { BlockKind::If } else { BlockKind::ElseIf };
                    let mut blk = self.new_block(b, kind, format!("{} {}", if first { "if" } else { "elif" }, cond), clause);
                    for c in &conds {
                        blk.uses.extend(self.uses(b, *c));
                    }
                    blk.uses.dedup();
                    self.block(b, &then, &mut blk.children);
                    out.push(blk);
                    first = false;
                    match rest.next() {
                        Some(next) if next.kind() == "elif_clause" => clause = next,
                        Some(last) => {
                            let mut eb = self.new_block(b, BlockKind::Else, "else".into(), last);
                            self.block(b, &named(last), &mut eb.children);
                            out.push(eb);
                            break;
                        }
                        None => break,
                    }
                }
            }
            "while_statement" | "for_statement" | "c_style_for_statement" => {
                let mut blk = self.new_block(b, BlockKind::Loop, head(n, self.src), n);
                let defs: Vec<String> = field(n, "variable").map(|v| vec![raw(v, self.src).to_string()]).unwrap_or_default();
                let body = field(n, "body");
                let mut names = Vec::new();
                for c in named(n).into_iter().filter(|c| body.is_none_or(|bd| bd.id() != c.id())) {
                    reads(c, self.src, &mut names);
                }
                blk.uses = b.uses_in_scope(names.into_iter().filter(|x| !defs.contains(x)).collect());
                b.define(&defs);
                self.block(b, &body.map(named).unwrap_or_default(), &mut blk.children);
                blk.defines = defs;
                out.push(blk);
            }
            "case_statement" => {
                let value = field(n, "value");
                let mut blk = self.new_block(b, BlockKind::Match, format!("case {}", value.map(|v| line(v, self.src)).unwrap_or_default()), n);
                if let Some(v) = value {
                    blk.uses = self.uses(b, v);
                }
                for c in named(n).into_iter().filter(|c| c.kind() == "case_item") {
                    let mut ab = self.new_block(b, BlockKind::Arm, case_label(c, self.src), c);
                    self.block(b, &case_body(c), &mut ab.children);
                    blk.children.push(ab);
                }
                out.push(blk);
            }
            "list" => {
                let sides = named(n);
                match (sides.first().copied(), sides.get(1).copied()) {
                    // `cmd || { …; }`: the group is what happens when the command fails
                    (Some(left), Some(right)) if matches!(right.kind(), "compound_statement" | "subshell") => {
                        let mut blk = self.new_block(b, BlockKind::If, format!("{} {}", line(left, self.src), list_operator(n)), n);
                        blk.uses = self.uses(b, left);
                        self.block(b, &named(right), &mut blk.children);
                        out.push(blk);
                    }
                    (Some(_), Some(right)) => {
                        // `cmd || exit 1`: one line, shown as what its right side does
                        let before = out.len();
                        self.leaf(b, n, out);
                        if let (Some(blk), Some(left)) = (out.get_mut(before), leaves(right, self.src)) {
                            blk.kind = match left.as_str() {
                                "return" => BlockKind::Return,
                                "exit" => BlockKind::Panic,
                                "break" => BlockKind::Break,
                                _ => BlockKind::Continue,
                            };
                        }
                    }
                    _ => self.leaf(b, n, out),
                }
            }
            "redirected_statement" if field(n, "body").is_some_and(|x| super::cfg::is_compound(x.kind())) => {
                if let Some(body) = field(n, "body") {
                    self.stmt(b, body, out);
                }
            }
            "compound_statement" | "subshell" | "do_group" => self.block(b, &named(n), out),
            "function_definition" => {}
            _ => self.leaf(b, n, out),
        }
    }
}

/// Build the block tree of a shell function definition, or of a script's own commands;
/// `signature` is the root's label.
pub fn build(node: Node, file: FileId, calls: &[CallEdge], fn_span: Span, src: &str, signature: &str) -> Block {
    let mut b = Builder::new(file, calls, &[]);
    let d = Driver { src, lines: src.lines().collect() };
    let mut kids = Vec::new();
    d.block(&mut b, &statements(node), &mut kids);
    b.root(signature.to_string(), fn_span, vec![], kids, src)
}
