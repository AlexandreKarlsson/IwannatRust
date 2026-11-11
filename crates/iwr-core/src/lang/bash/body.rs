//! Calls and counts inside a shell function or script: a command is a call when it names a
//! shell function of the project (same script first) or one of its scripts (`./build.sh`,
//! `bash build.sh`); programs from outside (`rsync`, `echo`) are not calls.

use super::{arguments, basename, command_name, statements};
use crate::lang::ts::*;
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
    /// The shell item called `name`: a function, or a script by its file name.
    fn shell_item(&self, name: &str) -> Option<ItemId> {
        let cands: Vec<ItemId> = self.ix.fns.get(name)?.iter().copied().filter(|id| self.p.files[self.p.items[*id].span.file].lang == Some(SourceLang::Bash)).collect();
        prefer_module(self.p, self.ix, &cands, self.module)
    }

    fn command(&mut self, n: Node) {
        let name = command_name(n, self.src);
        let args = arguments(n, self.src);
        // `bash x.sh`, `exec ./x.sh`: the script is the first argument that is one
        let runs = matches!(name.as_str(), "bash" | "sh" | "exec" | "source" | "." | "sudo" | "env" | "nohup" | "time");
        let target = if runs { args.iter().find(|a| !a.starts_with('-')).cloned().unwrap_or_default() } else { name.clone() };
        let to = if target.contains('/') || runs { self.shell_item(basename(&target)) } else { self.shell_item(&target) };
        if let Some(to) = to {
            let callee = if runs { format!("{} {}", name, basename(&target)) } else { name };
            self.edges.push(CallEdge {
                from: self.from,
                to: Some(to),
                callee,
                kind: CallKind::Call,
                span: span(n, self.file, self.src),
                in_loop: self.loop_depth > 0,
                in_branch: self.branch_depth > 0,
                propagated: false,
                unwrapped: false,
                is_await: false,
                is_recursive: to == self.from,
            });
        }
    }

    fn visit_all(&mut self, n: Node) {
        for c in named(n) {
            self.visit(c);
        }
    }

    fn visit(&mut self, n: Node) {
        match n.kind() {
            "command" => {
                self.command(n);
                self.visit_all(n);
            }
            "if_statement" | "case_statement" => {
                self.stats.branches += 1;
                let mut cur = n.walk();
                let heads: Vec<usize> = n.children_by_field_name("condition", &mut cur).chain(field(n, "value")).map(|c| c.id()).collect();
                for c in named(n) {
                    let inside = !heads.contains(&c.id());
                    self.branch_depth += inside as usize;
                    self.visit(c);
                    self.branch_depth -= inside as usize;
                }
            }
            "while_statement" | "for_statement" | "c_style_for_statement" => {
                self.stats.loops += 1;
                let body = field(n, "body").map(|b| b.id());
                for c in named(n) {
                    let inside = Some(c.id()) == body;
                    self.loop_depth += inside as usize;
                    self.visit(c);
                    self.loop_depth -= inside as usize;
                }
            }
            // a function defined inside another is not part of this body's flow
            "function_definition" => {}
            _ => self.visit_all(n),
        }
    }
}

/// The calls of a function definition (or of a script's own commands) and its counts.
pub(crate) fn analyze(p: &Project, ix: &Index, item: ItemId, file: FileId, node: Node, src: &str) -> (Vec<CallEdge>, Stats) {
    let mut v = Visitor { p, ix, file, from: item, module: p.items[item].module, src, loop_depth: 0, branch_depth: 0, edges: vec![], stats: Stats::default() };
    for s in statements(node) {
        v.visit(s);
    }
    (v.edges, v.stats)
}
