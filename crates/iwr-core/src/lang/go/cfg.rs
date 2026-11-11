//! The flow graph of a Go function: statements into the language-neutral [`Builder`].
//! `switch` / `select` cases do not fall through; a `return` whose last value is not `nil` in
//! a function that returns an `error` is an error path; `panic`, `os.Exit` and `log.Fatal`
//! stop the function.

use super::{line, stmts};
use crate::cfg::{Builder, Facts};
use crate::lang::ts::*;
use crate::model::*;
use std::cell::RefCell;
use std::collections::HashMap;
use tree_sitter::Node;

struct Driver<'s> {
    src: &'s str,
    /// The function's last result is an `error`.
    returns_error: bool,
    /// Label name → the node that marks it.
    labels: RefCell<HashMap<String, usize>>,
    /// `goto` nodes waiting for their label: (jump node, label).
    gotos: RefCell<Vec<(usize, String)>>,
}

/// The function a call statement calls, as written: `panic`, `os.Exit`, `log.Fatalf`.
pub(crate) fn called(n: Node, src: &str) -> Option<String> {
    let e = if n.kind() == "expression_statement" { named(n).into_iter().next()? } else { n };
    (e.kind() == "call_expression").then(|| field(e, "function").map(|f| raw(f, src).to_string())).flatten()
}

/// `os.Exit(2)` → `2`; `log.Fatal(…)` exits with 1.
pub(crate) fn exit_code(n: Node, src: &str) -> Option<String> {
    let name = called(n, src)?;
    if name == "os.Exit" {
        let e = named(n).into_iter().next()?;
        return Some(field(e, "arguments").map(|a| line(a, src).trim_matches(['(', ')']).to_string()).unwrap_or_default());
    }
    (name.starts_with("log.Fatal") || name.starts_with("log.Panic")).then(|| "1".to_string())
}

/// The head of a statement that has a `{ }` body: everything before the body, on one line.
pub(crate) fn head(n: Node, body: Option<Node>, src: &str) -> String {
    match body {
        Some(b) => one_line(&src[n.start_byte()..b.start_byte()]),
        None => line(n, src),
    }
}

/// `case a, b` / `default`, for the cases of a `switch` or `select`.
pub(crate) fn case_label(c: Node, src: &str) -> String {
    match c.kind() {
        "default_case" => "default".into(),
        "expression_case" => format!("case {}", field(c, "value").map(|v| line(v, src)).unwrap_or_default()),
        "communication_case" => format!("case {}", field(c, "communication").map(|v| line(v, src)).unwrap_or_default()),
        _ => {
            // a type case: every `type:` child
            let mut cur = c.walk();
            let types: Vec<String> = c.children_by_field_name("type", &mut cur).map(|t| line(t, src)).collect();
            format!("case {}", types.join(", "))
        }
    }
}

/// The cases of a `switch` / `select` statement.
pub(crate) fn cases(n: Node) -> Vec<Node> {
    named(n).into_iter().filter(|c| matches!(c.kind(), "expression_case" | "type_case" | "communication_case" | "default_case")).collect()
}

/// The statements of a case, its values left out.
pub(crate) fn case_body(c: Node) -> Vec<Node> {
    named(c).into_iter().filter(|s| s.kind() == "statement_list").flat_map(named).collect()
}

impl<'s> Driver<'s> {
    fn block(&self, b: &mut Builder, n: Node) {
        for c in stmts(n) {
            self.stmt(b, c);
        }
    }

    fn stmt(&self, b: &mut Builder, n: Node) {
        b.note_for(n.start_position().row + 1);
        self.stmt_inner(b, n);
        b.clear_note();
    }

    fn stmt_inner(&self, b: &mut Builder, n: Node) {
        let file = b.file();
        let sp = span(n, file, self.src);
        let text = line(n, self.src);
        match n.kind() {
            "block" => self.block(b, n),
            "short_var_declaration" | "var_declaration" | "const_declaration" => b.plain(&text, text.clone(), sp, Facts { is_let: true, ..Default::default() }),
            "assignment_statement" | "inc_statement" | "dec_statement" | "send_statement" => b.plain(&text, text.clone(), sp, Facts { is_mutation: true, ..Default::default() }),
            "expression_statement" => {
                if called(n, self.src).as_deref() == Some("panic") {
                    b.panic_node(text, sp, "panic", true);
                } else if let Some(code) = exit_code(n, self.src) {
                    b.halt(&code, text, sp);
                } else {
                    let io = called(n, self.src).is_some_and(|c| c.starts_with("fmt.") || c.starts_with("os.") || c.starts_with("io.") || c.starts_with("bufio."));
                    b.plain(&text, text.clone(), sp, Facts { is_io: io, ..Default::default() });
                }
            }
            "if_statement" => {
                // `if err := f(); err != nil`: the statement first, then the test
                if let Some(init) = field(n, "initializer") {
                    self.stmt_inner(b, init);
                }
                let cond_node = field(n, "condition");
                let cond = cond_node.map(|c| line(c, self.src)).unwrap_or_default();
                let cond_span = cond_node.map(|c| span(c, file, self.src)).unwrap_or(sp);
                let then = field(n, "consequence");
                let els = field(n, "alternative");
                b.if_node(&cond, cond_span, |b| if let Some(t) = then { self.stmt(b, t) }, els.map(|e| move |b: &mut Builder| self.stmt(b, e)));
            }
            "for_statement" => {
                let body = field(n, "body");
                let label = head(n, body, self.src);
                let clause = named(n).into_iter().find(|c| body.is_none_or(|bd| bd.id() != c.id()));
                // `for { }` and `for i := 0; ; i++ { }` only end on a `break`
                let forever = match clause {
                    None => true,
                    Some(c) if c.kind() == "for_clause" => field(c, "condition").is_none(),
                    Some(_) => false,
                };
                let head_span = match body {
                    Some(bd) => Span { line_end: bd.start_position().row + 1, col_end: span(bd, file, self.src).col_start, ..sp },
                    None => sp,
                };
                b.loop_node(&label, head_span, forever, |b| if let Some(s) = body { self.block(b, s) });
            }
            "expression_switch_statement" | "type_switch_statement" | "select_statement" => {
                if let Some(init) = field(n, "initializer") {
                    self.stmt_inner(b, init);
                }
                let cs = cases(n);
                // the head: from the keyword to the `{`
                let label = match cs.first() {
                    Some(c) => one_line(self.src[n.start_byte()..c.start_byte()].trim_end().trim_end_matches('{')),
                    None => text.clone(),
                };
                let head_span = Span { line_end: sp.line_start, col_end: sp.col_start + label.chars().count(), ..sp };
                let labels: Vec<String> = cs.iter().map(|c| case_label(*c, self.src)).collect();
                let has_default = cs.iter().any(|c| c.kind() == "default_case");
                b.cases_node(&label, head_span, &labels, has_default, |b, i| {
                    for s in case_body(cs[i]) {
                        self.stmt(b, s);
                    }
                });
            }
            "return_statement" => {
                let values: Vec<String> = named(n).into_iter().flat_map(|l| if l.kind() == "expression_list" { named(l) } else { vec![l] }).map(|e| line(e, self.src)).collect();
                let is_err = self.returns_error && values.last().is_some_and(|v| v != "nil");
                let what = match values.as_slice() {
                    [] => "return".to_string(),
                    _ if is_err => "return error".to_string(),
                    [one] if one.len() <= 12 && !one.contains('(') => format!("return {}", one),
                    _ => "return".to_string(),
                };
                b.return_node(is_err, text, sp, &what);
            }
            "break_statement" => b.break_node(text, sp),
            "continue_statement" => b.continue_node(text, sp),
            "goto_statement" => {
                let label = named(n).into_iter().next().map(|l| raw(l, self.src).to_string()).unwrap_or_default();
                let id = b.jump_node(&text, text.clone(), sp);
                self.gotos.borrow_mut().push((id, label));
            }
            "labeled_statement" => {
                let label_node = field(n, "label");
                let label = label_node.map(|l| raw(l, self.src).to_string()).unwrap_or_default();
                let id = b.seq(CfgNodeKind::Block, format!("{}:", label), format!("{}:", label), span(label_node.unwrap_or(n), file, self.src));
                self.labels.borrow_mut().insert(label, id);
                for c in named(n).into_iter().filter(|c| c.kind() != "label_name") {
                    self.stmt(b, c);
                }
            }
            "empty_statement" => {}
            // `defer f()`, `go f()`, `fallthrough`, a bare `{ }`…
            _ => {
                if !text.is_empty() {
                    b.plain(&text, text.clone(), sp, Facts::default());
                }
            }
        }
    }
}

/// Does the function declared by `fn_node` return an `error` last?
pub(crate) fn returns_error(fn_node: Node, src: &str) -> bool {
    field(fn_node, "result").is_some_and(|r| {
        let t = line(r, src);
        t == "error" || t.trim_end_matches(')').ends_with(" error")
    })
}

/// Build the CFG of a Go function or method declaration. `calls` are its (already resolved) call edges.
pub fn build(fn_node: Node, file: FileId, calls: &[CallEdge], fn_span: Span, src: &str) -> Cfg {
    let d = Driver { src, returns_error: returns_error(fn_node, src), labels: RefCell::new(HashMap::new()), gotos: RefCell::new(vec![]) };
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
