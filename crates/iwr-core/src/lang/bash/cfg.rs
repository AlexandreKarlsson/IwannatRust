//! The flow graph of a shell function or script: commands into the language-neutral
//! [`Builder`]. `exit` stops the script, `return N` with `N` not 0 is an error path,
//! `a || b` / `a && b` is a branch when `b` leaves (`exit`, `return`, `break`), is a `{ }`
//! group or calls a function of the project.

use super::{arguments, command_name, line, list_operator, note_above, statements};
use crate::cfg::{Builder, Facts};
use crate::lang::ts::*;
use crate::model::*;
use tree_sitter::Node;

struct Driver<'s> {
    src: &'s str,
    lines: Vec<&'s str>,
}

/// Commands that read or print.
const IO: &[&str] = &["echo", "printf", "read", "cat", "tee", "curl", "wget"];

/// Statements with a body of their own.
pub(crate) fn is_compound(kind: &str) -> bool {
    matches!(kind, "if_statement" | "while_statement" | "for_statement" | "c_style_for_statement" | "case_statement" | "compound_statement" | "subshell" | "list")
}

/// The command a statement is, looking through its redirections and a leading `!`.
pub(crate) fn inner_command(n: Node) -> Option<Node> {
    match n.kind() {
        "command" => Some(n),
        "redirected_statement" => field(n, "body").and_then(inner_command),
        "negated_command" => named(n).into_iter().next().and_then(inner_command),
        _ => None,
    }
}

/// `exit`, `return`, `break` or `continue`, when the statement is one of them.
pub(crate) fn leaves(n: Node, src: &str) -> Option<String> {
    let name = command_name(inner_command(n)?, src);
    matches!(name.as_str(), "exit" | "return" | "break" | "continue").then_some(name)
}

/// The head of a statement that has a body: everything before the body, without the `; do`.
pub(crate) fn head(n: Node, src: &str) -> String {
    let text = match field(n, "body") {
        Some(b) => one_line(&src[n.start_byte()..b.start_byte()]),
        None => line(n, src),
    };
    text.trim_end().trim_end_matches(';').trim_end().to_string()
}

/// The test of an `if` / `elif` / `while`, and the statements it guards.
pub(crate) fn guarded(n: Node) -> (Vec<Node>, Vec<Node>) {
    let mut cur = n.walk();
    let mut conds: Vec<Node> = n.children_by_field_name("condition", &mut cur).filter(|c| c.is_named()).collect();
    let mut rest: Vec<Node> = named(n).into_iter().filter(|c| !matches!(c.kind(), "elif_clause" | "else_clause")).collect();
    if conds.is_empty() && !rest.is_empty() {
        // an `elif` clause has no field for its test: it is the first statement
        conds.push(rest[0]);
    }
    rest.retain(|c| !conds.iter().any(|x| x.id() == c.id()));
    (conds, rest)
}

/// `a|b` for the patterns of a case item.
pub(crate) fn case_label(c: Node, src: &str) -> String {
    let mut cur = c.walk();
    let values: Vec<String> = c.children_by_field_name("value", &mut cur).map(|v| line(v, src)).collect();
    values.join("|")
}

/// The statements of a case item, its patterns left out.
pub(crate) fn case_body(c: Node) -> Vec<Node> {
    let mut cur = c.walk();
    let values: Vec<usize> = c.children_by_field_name("value", &mut cur).map(|v| v.id()).collect();
    named(c).into_iter().filter(|s| !values.contains(&s.id())).collect()
}

impl<'s> Driver<'s> {
    fn block(&self, b: &mut Builder, stmts: &[Node]) {
        for s in stmts {
            self.stmt(b, *s);
        }
    }

    fn stmt(&self, b: &mut Builder, n: Node) {
        b.set_note(note_above(&self.lines, n.start_position().row + 1));
        self.stmt_inner(b, n);
        b.clear_note();
    }

    /// An `if` or `elif` and what follows it in the chain.
    fn if_chain(&self, b: &mut Builder, n: Node, rest: &[Node]) {
        let (conds, then) = guarded(n);
        let cond = conds.iter().map(|c| line(*c, self.src)).collect::<Vec<_>>().join("; ");
        let cond_span = conds.first().map(|c| span(*c, b.file(), self.src)).unwrap_or(span(n, b.file(), self.src));
        let els: Option<Box<dyn FnOnce(&mut Builder) + '_>> = match rest.split_first() {
            Some((next, more)) if next.kind() == "elif_clause" => Some(Box::new(move |b: &mut Builder| self.if_chain(b, *next, more))),
            Some((last, _)) => Some(Box::new(move |b: &mut Builder| self.block(b, &named(*last)))),
            None => None,
        };
        b.if_node(&cond, cond_span, |b| self.block(b, &then), els);
    }

    fn stmt_inner(&self, b: &mut Builder, n: Node) {
        let file = b.file();
        let sp = span(n, file, self.src);
        let text = line(n, self.src);
        match n.kind() {
            "command" | "redirected_statement" | "negated_command" if inner_command(n).is_some() => {
                let cmd = inner_command(n).unwrap();
                let name = command_name(cmd, self.src);
                let args = arguments(cmd, self.src);
                match name.as_str() {
                    "exit" => b.halt(args.first().map(String::as_str).unwrap_or("0"), text, sp),
                    "return" => {
                        let is_err = args.first().is_some_and(|c| c != "0");
                        let what = text.clone();
                        b.return_node(is_err, text, sp, &what);
                    }
                    "break" => b.break_node(text, sp),
                    "continue" => b.continue_node(text, sp),
                    _ => b.plain(&text, text.clone(), sp, Facts { is_io: IO.contains(&name.as_str()), ..Default::default() }),
                }
            }
            "redirected_statement" => match field(n, "body") {
                // `while read l; do … done < file`: the loop is what matters
                Some(body) if is_compound(body.kind()) => self.stmt_inner(b, body),
                _ => b.plain(&text, text.clone(), sp, Facts { is_io: true, ..Default::default() }),
            },
            "variable_assignment" | "declaration_command" => b.plain(&text, text.clone(), sp, Facts { is_let: true, ..Default::default() }),
            "if_statement" => {
                let rest: Vec<Node> = named(n).into_iter().filter(|c| matches!(c.kind(), "elif_clause" | "else_clause")).collect();
                self.if_chain(b, n, &rest);
            }
            "while_statement" => {
                let label = head(n, self.src);
                let (conds, _) = guarded(n);
                let forever = conds.len() == 1 && matches!(line(conds[0], self.src).as_str(), "true" | ":");
                let head_span = conds.first().map(|c| span(*c, file, self.src)).unwrap_or(sp);
                let body = field(n, "body").map(named).unwrap_or_default();
                b.loop_node(&label, head_span, forever, |b| self.block(b, &body));
            }
            "for_statement" | "c_style_for_statement" => {
                let label = head(n, self.src);
                let body = field(n, "body");
                let head_span = match body {
                    Some(bd) => Span { line_end: bd.start_position().row + 1, col_end: span(bd, file, self.src).col_start, ..sp },
                    None => sp,
                };
                let stmts = body.map(named).unwrap_or_default();
                b.loop_node(&label, head_span, false, |b| self.block(b, &stmts));
            }
            "case_statement" => {
                let value = field(n, "value");
                let label = format!("case {}", value.map(|v| line(v, self.src)).unwrap_or_default());
                let items: Vec<Node> = named(n).into_iter().filter(|c| c.kind() == "case_item").collect();
                let labels: Vec<String> = items.iter().map(|c| case_label(*c, self.src)).collect();
                let has_default = labels.iter().any(|l| l == "*");
                b.cases_node(&label, value.map(|v| span(v, file, self.src)).unwrap_or(sp), &labels, has_default, |b, i| self.block(b, &case_body(items[i])));
            }
            "list" => {
                let sides = named(n);
                let (Some(left), Some(right)) = (sides.first().copied(), sides.get(1).copied()) else { return };
                let right_span = span(right, file, self.src);
                let branches = leaves(right, self.src).is_some() || matches!(right.kind(), "compound_statement" | "subshell") || b.has_calls(&right_span);
                if !branches {
                    b.plain(&text, text.clone(), sp, Facts::default());
                    return;
                }
                // `a || b`: b runs when a fails; `a && b`: when it succeeds
                let or = list_operator(n) == "||";
                let label = format!("{} {}", line(left, self.src), if or { "||" } else { "&&" });
                let edges = if or { ("fails", "ok") } else { ("ok", "fails") };
                b.branch_node(&label, edges, span(left, file, self.src), |b| self.stmt_inner(b, right), None::<fn(&mut Builder)>);
            }
            "compound_statement" | "subshell" | "do_group" => self.block(b, &named(n)),
            "function_definition" => {}
            _ => {
                if !text.is_empty() {
                    b.plain(&text, text.clone(), sp, Facts::default());
                }
            }
        }
    }
}

/// Build the CFG of a shell function definition, or of a script's commands outside its
/// functions (`node` is then the whole program). `calls` are its (already resolved) call edges.
pub fn build(node: Node, file: FileId, calls: &[CallEdge], fn_span: Span, src: &str) -> Cfg {
    let d = Driver { src, lines: src.lines().collect() };
    let mut b = Builder::new(file, calls, fn_span, src);
    d.block(&mut b, &statements(node));
    b.finish(None)
}
