//! Build a control-flow graph from a function body.
//!
//! The graph is statement-level: consecutive plain statements are merged into
//! one `Block` node; `if` / `match` / loops / `return` / `?` / panics / awaits
//! / calls to project functions get their own nodes so the UI can show them
//! distinctly.
//!
//! The [`Builder`] knows nothing about a language: it takes labels, spans and
//! [`Facts`] and keeps the open ends, loops and conditions straight. A driver
//! per language walks its syntax tree and calls it: the Rust one is below
//! (`syn`), the C one in [`crate::c::cfg`] (tree-sitter).

use crate::model::*;
use crate::text::*;
use syn::visit::Visit;

/// A node whose "next" edge is still pending, with the kind and label that edge will carry.
type Open = (usize, CfgEdgeKind, Option<String>);

/// A loop (or a C `switch`) being built: `break` edges collect here until it closes.
struct Frame {
    head: usize,
    breaks: Vec<Open>,
    /// A `switch`: `break` leaves it, `continue` skips it and goes to the loop around.
    is_switch: bool,
}

/// Shared part of the CFG construction, driven by a language-specific walker.
pub(crate) struct Builder<'a> {
    cfg: Cfg,
    file: FileId,
    /// Open ends: nodes whose "next" edge is still pending, with the label to use.
    open: Vec<Open>,
    frames: Vec<Frame>,
    exit: usize,
    calls: &'a [CallEdge],
    depth: usize,
    /// Source lines of the file (to read `//` comments, which the parsers drop).
    lines: Vec<&'a str>,
    /// Comment found above the current statement, given to the next node created.
    pending_note: Option<String>,
    /// Conditions leading to the current point: (number of open frames when pushed, description).
    conds: Vec<(usize, String)>,
    /// `let x =` of a `let x = match …` / `let x = if …`: put in front of that node's label
    let_prefix: Option<String>,
}

/// Facts about a statement's expression that decide how it is rendered.
#[derive(Default, Clone, Copy)]
pub(crate) struct Facts {
    pub has_try: bool,
    pub has_await: bool,
    pub has_panic: bool,
    pub has_unwrap: bool,
    pub has_return: bool,
    /// A closure inside (its body is shown as child blocks).
    pub has_closure: bool,
    /// A new local is defined (`let`, a C declaration).
    pub is_let: bool,
    /// Existing state changes (assignment, compound assignment, a mutating method).
    pub is_mutation: bool,
}

/// One arm of a `match` / `switch` given to [`Builder::match_node`].
pub(crate) struct Arm {
    pub pat: String,
    /// `Ok(c) => c`: the arm only hands a value on, shown on the edge instead of a box.
    pub trivial: Option<String>,
}

/// Logging macros: noise in a flow graph unless a comment says why they matter.
fn is_log_macro(path: &syn::Path) -> bool {
    let first = path.segments.first().map(|s| s.ident.to_string()).unwrap_or_default();
    let last = path_last(path);
    matches!(first.as_str(), "log" | "tracing") || matches!(last.as_str(), "trace" | "debug" | "info" | "warn" | "error" | "log" | "event")
}

/// Input / output, read from the statement's text (Rust macros and std paths, C stdio and sockets).
pub(crate) fn is_io(text: &str) -> bool {
    const IO: &[&str] = &[
        "println!", "print!", "eprintln!", "eprint!", "write!", "writeln!", "dbg!", "std::fs", "fs::", "File::", "OpenOptions", "stdin()", "stdout()", "stderr()", "read_to_string", "read_line", "TcpStream", "UdpSocket", "TcpListener", ".flush()",
        "printf(", "puts(", "putchar(", "scanf(", "getchar(", "fgets(", "fopen(", "fread(", "fwrite(", "fclose(", "perror(", "read(", "write(", "recv(", "send(", "socket(", "fflush(",
    ];
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    IO.iter().any(|w| compact.contains(&w.replace(' ', "")))
}

/// `std::process::exit(code)` / `process::abort()` / C `exit(code)`: the exit code, or "abort".
fn halt_of(text: &str) -> Option<String> {
    // the statement itself must be the call: `x.unwrap_or_else(|| exit(1))` only sometimes stops
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let call = compact.trim_start_matches("std::").trim_start_matches("process::");
    if call.starts_with("abort(") {
        return Some("abort".into());
    }
    let rest = call.strip_prefix("exit(")?;
    Some(rest.chars().take_while(|c| *c != ')').collect())
}

/// The sentence for a halt: "exit with code 2", "print error, exit with code 2", "abort".
fn halt_sentence(code: &str, printed: Option<&str>) -> String {
    let stop = if code == "abort" { "abort".to_string() } else { format!("exit with code {}", code) };
    match printed {
        Some(p) if p.starts_with("eprint") || p.contains("stderr") || p.starts_with("perror") => format!("print error, {}", stop),
        Some(_) => format!("print message, {}", stop),
        None => stop,
    }
}

/// An arm body that only hands a value on (`c`, `0`, `&x`, `None`): no box, it goes on the edge.
fn is_trivial(e: &syn::Expr) -> bool {
    match e {
        syn::Expr::Path(_) | syn::Expr::Lit(_) => true,
        syn::Expr::Reference(r) => is_trivial(&r.expr),
        syn::Expr::Paren(p) => is_trivial(&p.expr),
        syn::Expr::Block(b) => b.block.stmts.len() == 1 && matches!(&b.block.stmts[0], syn::Stmt::Expr(x, None) if is_trivial(x)),
        _ => false,
    }
}

/// Does this expression change existing state (assignment, compound assignment, a mutating method)?
fn is_mutation(e: &syn::Expr) -> bool {
    const MUT: &[&str] = &["push", "push_str", "push_back", "push_front", "insert", "remove", "clear", "extend", "pop", "truncate", "sort", "sort_by", "sort_by_key", "sort_unstable", "retain", "dedup", "drain", "append", "swap", "entry", "set"];
    match e {
        syn::Expr::Assign(_) => true,
        syn::Expr::Binary(b) => matches!(
            b.op,
            syn::BinOp::AddAssign(_) | syn::BinOp::SubAssign(_) | syn::BinOp::MulAssign(_) | syn::BinOp::DivAssign(_) | syn::BinOp::RemAssign(_) | syn::BinOp::BitOrAssign(_) | syn::BinOp::BitAndAssign(_) | syn::BinOp::BitXorAssign(_) | syn::BinOp::ShlAssign(_) | syn::BinOp::ShrAssign(_)
        ),
        syn::Expr::MethodCall(m) => MUT.contains(&m.method.to_string().as_str()) || is_mutation(&m.receiver),
        _ => false,
    }
}

pub use crate::views::NOTE_MAX;

/// A short single-line comment directly above `line` (1-based), if any: `// …` or a one-line `/* … */`.
pub(crate) fn note_above(lines: &[&str], line: usize) -> Option<String> {
    let comment = |i: usize| -> Option<&str> {
        let t = lines.get(i)?.trim();
        if t.starts_with("//") && !t.starts_with("///") && !t.starts_with("//!") {
            return Some(t[2..].trim());
        }
        if t.starts_with("/*") && !t.starts_with("/**") && !t.starts_with("/*!") && t.ends_with("*/") {
            return Some(t[2..t.len() - 2].trim());
        }
        None
    };
    if line < 2 {
        return None;
    }
    let text = comment(line - 2)?;
    // part of a longer comment block: that is an explanation, not a title
    if line >= 3 && comment(line - 3).is_some() {
        return None;
    }
    (!text.is_empty() && text.chars().count() <= NOTE_MAX).then(|| text.to_string())
}

pub(crate) fn contains(outer: &Span, inner: &Span) -> bool {
    outer.file == inner.file
        && (outer.line_start, outer.col_start) <= (inner.line_start, inner.col_start)
        && (outer.line_end, outer.col_end) >= (inner.line_end, inner.col_end)
}

struct FactsVisitor(Facts);
impl<'ast> Visit<'ast> for FactsVisitor {
    fn visit_expr_try(&mut self, e: &'ast syn::ExprTry) {
        self.0.has_try = true;
        syn::visit::visit_expr_try(self, e);
    }
    fn visit_expr_await(&mut self, e: &'ast syn::ExprAwait) {
        self.0.has_await = true;
        syn::visit::visit_expr_await(self, e);
    }
    fn visit_expr_return(&mut self, e: &'ast syn::ExprReturn) {
        self.0.has_return = true;
        syn::visit::visit_expr_return(self, e);
    }
    fn visit_expr_method_call(&mut self, e: &'ast syn::ExprMethodCall) {
        let m = e.method.to_string();
        if m == "unwrap" || m == "expect" {
            self.0.has_unwrap = true;
        }
        syn::visit::visit_expr_method_call(self, e);
    }
    fn visit_expr_macro(&mut self, e: &'ast syn::ExprMacro) {
        let name = path_last(&e.mac.path);
        if matches!(name.as_str(), "panic" | "unreachable" | "todo" | "unimplemented" | "assert" | "assert_eq" | "assert_ne") {
            self.0.has_panic = true;
        }
        syn::visit::visit_expr_macro(self, e);
    }
    fn visit_expr_closure(&mut self, _e: &'ast syn::ExprClosure) {
        // closures run later / elsewhere: do not attribute their control flow to this statement
    }
}

fn facts(e: &syn::Expr) -> Facts {
    let mut v = FactsVisitor(Facts::default());
    v.visit_expr(e);
    v.0.is_mutation = is_mutation(e);
    v.0
}

// ---- language-neutral construction ----

impl<'a> Builder<'a> {
    /// Start a graph: the entry and exit nodes at the function's first and last position.
    pub(crate) fn new(file: FileId, calls: &'a [CallEdge], fn_span: Span, source: &'a str) -> Self {
        let lines = source.lines().collect();
        let mut b = Builder { cfg: Cfg::default(), file, open: vec![], frames: vec![], exit: 0, calls, depth: 0, lines, pending_note: None, conds: vec![], let_prefix: None };
        let entry = b.add(CfgNodeKind::Entry, "start".into(), "function entry".into(), Span { file, line_start: fn_span.line_start, col_start: fn_span.col_start, line_end: fn_span.line_start, col_end: fn_span.col_start });
        let exit = b.add(CfgNodeKind::Exit, "end".into(), "function exit".into(), Span { file, line_start: fn_span.line_end, col_start: fn_span.col_end, line_end: fn_span.line_end, col_end: fn_span.col_end });
        b.exit = exit;
        b.open.push((entry, CfgEdgeKind::Next, None));
        b
    }

    /// Close the graph: whatever is still open falls through to the exit. `tail` labels those
    /// fall-through edges (the value of an implicit tail expression).
    pub(crate) fn finish(mut self, tail: Option<String>) -> Cfg {
        let exit = self.exit;
        let open = std::mem::take(&mut self.open);
        for (from, kind, label) in open {
            let label = if kind == CfgEdgeKind::Next { None } else { label };
            self.edge(from, exit, kind, label);
        }
        if let Some(lbl) = tail {
            for edge in &mut self.cfg.edges {
                if edge.to == exit && edge.kind == CfgEdgeKind::Next && edge.label.is_none() {
                    edge.label = Some(lbl.clone());
                }
            }
        }
        self.cfg
    }

    pub(crate) fn file(&self) -> FileId {
        self.file
    }

    /// Remember the `//` comment above a statement starting at `line`; the next node created takes it.
    pub(crate) fn note_for(&mut self, line: usize) {
        self.pending_note = note_above(&self.lines, line);
    }

    pub(crate) fn clear_note(&mut self) {
        self.pending_note = None;
    }

    fn add(&mut self, kind: CfgNodeKind, label: String, detail: String, span: Span) -> usize {
        let id = self.cfg.nodes.len();
        let calls: Vec<ItemId> = self
            .calls
            .iter()
            .filter(|c| c.to.is_some() && c.kind != CallKind::Construct && contains(&span, &c.span))
            .filter_map(|c| c.to)
            .collect();
        let mut calls_dedup = Vec::new();
        for c in calls {
            if !calls_dedup.contains(&c) {
                calls_dedup.push(c);
            }
        }
        let note = self.pending_note.take();
        let in_loop = self.frames.iter().rev().find(|f| !f.is_switch).map(|f| f.head);
        self.cfg.nodes.push(CfgNode { id, kind, label, detail, span, calls: calls_dedup, depth: self.depth, note, in_loop });
        id
    }

    fn edge(&mut self, from: usize, to: usize, kind: CfgEdgeKind, label: Option<String>) {
        if self.cfg.edges.iter().any(|e| e.from == from && e.to == to && e.kind == kind && e.label == label) {
            return;
        }
        self.cfg.edges.push(CfgEdge { from, to, label, kind });
    }

    /// Connect all open ends to `to`, then clear them.
    fn flow_into(&mut self, to: usize) {
        let open = std::mem::take(&mut self.open);
        for (from, kind, label) in open {
            self.edge(from, to, kind, label);
        }
    }

    /// Append a node after the current open ends.
    pub(crate) fn seq(&mut self, kind: CfgNodeKind, label: String, detail: String, span: Span) -> usize {
        let id = self.add(kind, label, detail, span);
        self.flow_into(id);
        self.open.push((id, CfgEdgeKind::Next, None));
        id
    }

    /// Does this statement contain a call to a project function?
    pub(crate) fn has_calls(&self, span: &Span) -> bool {
        self.calls.iter().any(|c| c.to.is_some() && c.kind != CallKind::Construct && contains(span, &c.span))
    }

    /// What leads to the current point inside the innermost loop, e.g. the `if` around a `break`.
    fn cond_here(&self) -> Option<String> {
        self.conds.last().filter(|(level, _)| *level == self.frames.len()).map(|(_, c)| c.clone())
    }

    /// A plain statement: merged into the previous Block node when possible,
    /// unless it contains something interesting (call / ? / await / unwrap).
    pub(crate) fn plain(&mut self, label: &str, detail: String, span: Span, f: Facts) {
        if let Some(code) = halt_of(&detail) {
            self.halt(&code, detail, span);
            return;
        }
        let has_calls = self.has_calls(&span);
        let kind = if f.has_return {
            CfgNodeKind::Return
        } else if f.has_try {
            CfgNodeKind::Propagate
        } else if f.has_panic || f.has_unwrap {
            CfgNodeKind::Panic
        } else if has_calls {
            CfgNodeKind::Call
        } else if f.has_await {
            CfgNodeKind::Await
        } else if is_io(&detail) {
            CfgNodeKind::Io
        } else if f.is_let {
            CfgNodeKind::Let
        } else if f.is_mutation {
            CfgNodeKind::Assign
        } else {
            CfgNodeKind::Block
        };
        if matches!(kind, CfgNodeKind::Block | CfgNodeKind::Let | CfgNodeKind::Assign | CfgNodeKind::Io) {
            // try to merge with previous block node
            if self.open.len() == 1 && self.pending_note.is_none() {
                let (prev, k, _) = self.open[0].clone();
                let p = &self.cfg.nodes[prev];
                let same_paragraph = p.note.is_none() || span.line_start <= p.span.line_end + 1;
                if k == CfgEdgeKind::Next && p.kind == kind && p.depth == self.depth && same_paragraph {
                    let n = &mut self.cfg.nodes[prev];
                    // a group of definitions / changes shows as one short title, so it can hold more lines
                    let cap = if matches!(kind, CfgNodeKind::Let | CfgNodeKind::Assign) { 8 } else { 4 };
                    if n.label.lines().count() < cap {
                        n.label.push('\n');
                        n.label.push_str(&shorten(label, 40));
                        n.detail.push('\n');
                        n.detail.push_str(&detail);
                        n.span = span_union(n.span, span);
                        return;
                    }
                }
            }
            self.seq(kind, shorten(label, 40), detail, span);
            return;
        }
        let id = self.seq(kind, shorten(label, 40), detail, span);
        let exit = self.exit;
        match kind {
            CfgNodeKind::Propagate => self.edge(id, exit, CfgEdgeKind::Error, Some("Err / None".into())),
            CfgNodeKind::Panic => self.edge(id, exit, CfgEdgeKind::Error, Some("panic".into())),
            CfgNodeKind::Return => {
                self.open.clear();
                let label = self.exit_label("return");
                self.edge(id, exit, CfgEdgeKind::Return, Some(label));
            }
            _ => {}
        }
    }

    /// The program stops: a print just before (same paragraph) joins the node, nothing follows.
    fn halt(&mut self, code: &str, detail: String, span: Span) {
        let prev = match self.open.as_slice() {
            [(prev, CfgEdgeKind::Next, _)] => Some(*prev),
            _ => None,
        };
        let printed = prev.filter(|p| {
            let n = &self.cfg.nodes[*p];
            n.kind == CfgNodeKind::Io && n.depth == self.depth && n.detail.lines().count() == 1 && n.detail.trim_start().starts_with(['e', 'p', 'f']) && (n.detail.contains("print") || n.detail.contains("puts") || n.detail.contains("perror"))
        });
        let id = match printed {
            Some(p) => {
                let n = &mut self.cfg.nodes[p];
                n.kind = CfgNodeKind::Halt;
                n.label = halt_sentence(code, Some(n.detail.trim_start()));
                n.detail = format!("{}\n{}", n.detail, detail);
                n.span = span_union(n.span, span);
                if n.note.is_none() {
                    n.note = self.pending_note.take();
                }
                self.open.clear();
                p
            }
            None => {
                let id = self.seq(CfgNodeKind::Halt, halt_sentence(code, None), detail, span);
                self.open.clear();
                id
            }
        };
        let exit = self.exit;
        let label = if code == "abort" { "abort".to_string() } else { format!("exit {}", code) };
        self.edge(id, exit, CfgEdgeKind::Error, Some(label));
    }

    /// Label of an edge leaving a loop early: the condition that leads there, then what happens.
    fn exit_label(&self, what: &str) -> String {
        match self.cond_here() {
            Some(c) if !self.frames.is_empty() => format!("{} → {}", shorten(&c, 32), what),
            _ => what.to_string(),
        }
    }

    /// `let x = if …` / `let x = match …`: the binding goes in front of the branch node's label.
    pub(crate) fn set_let_prefix(&mut self, prefix: String) {
        self.let_prefix = Some(prefix);
    }

    /// A branch: `cond` is the condition as written; `then` / `els` build the two sides.
    pub(crate) fn if_node(&mut self, cond: &str, cond_span: Span, then: impl FnOnce(&mut Self), els: Option<impl FnOnce(&mut Self)>) {
        let pre = self.let_prefix.take().map(|p| format!("{} ", p)).unwrap_or_default();
        let id = self.seq(CfgNodeKind::If, shorten(&format!("{}if {}", pre, cond), 40), format!("{}if {}", pre, cond), cond_span);
        self.open.clear();
        self.depth += 1;
        let what = self.cfg.nodes[id].note.clone().unwrap_or_else(|| format!("if {}", cond));
        let level = self.frames.len();
        // then
        self.conds.push((level, what.clone()));
        self.open.push((id, CfgEdgeKind::True, Some("true".into())));
        then(self);
        let then_ends = std::mem::take(&mut self.open);
        self.conds.pop();
        // else
        self.conds.push((level, format!("not ({})", what)));
        self.open.push((id, CfgEdgeKind::False, Some("false".into())));
        if let Some(e) = els {
            e(self);
        }
        self.conds.pop();
        self.depth -= 1;
        let mut ends = std::mem::take(&mut self.open);
        ends.extend(then_ends);
        self.open = ends;
    }

    /// A `match`: one node, one edge per arm; `body(self, i)` builds arm `i`'s body.
    pub(crate) fn match_node(&mut self, scrut: &str, scrut_span: Span, arms: &[Arm], mut body: impl FnMut(&mut Self, usize)) {
        let pre = self.let_prefix.take().map(|p| format!("{} ", p)).unwrap_or_default();
        let id = self.seq(CfgNodeKind::Match, shorten(&format!("{}match {}", pre, scrut), 40), format!("{}match {}", pre, scrut), scrut_span);
        self.open.clear();
        self.depth += 1;
        let mut ends = Vec::new();
        for (i, arm) in arms.iter().enumerate() {
            if let Some(value) = &arm.trivial {
                // `Ok(c) => c`: nothing happens but handing the value on
                let label = format!("{} → {}", arm.pat, value);
                ends.push((id, CfgEdgeKind::Arm, Some(shorten(&label, 30))));
                continue;
            }
            self.open.push((id, CfgEdgeKind::Arm, Some(shorten(&arm.pat, 24))));
            let level = self.frames.len();
            self.conds.push((level, arm.pat.clone()));
            body(self, i);
            self.conds.pop();
            ends.extend(std::mem::take(&mut self.open));
        }
        self.depth -= 1;
        self.open = ends;
    }

    /// A C `switch`: like a match, but a case without `break` falls into the next one and
    /// `break` leaves the switch, not the loop around it.
    pub(crate) fn switch_node(&mut self, scrut: &str, scrut_span: Span, cases: &[String], mut body: impl FnMut(&mut Self, usize)) {
        let id = self.seq(CfgNodeKind::Match, shorten(&format!("switch {}", scrut), 40), format!("switch {}", scrut), scrut_span);
        self.open.clear();
        self.depth += 1;
        self.frames.push(Frame { head: id, breaks: vec![], is_switch: true });
        let outer_conds = std::mem::take(&mut self.conds);
        for (i, case) in cases.iter().enumerate() {
            // the open ends of the case before stay: that is the fall-through
            self.open.push((id, CfgEdgeKind::Arm, Some(shorten(case, 24))));
            let level = self.frames.len();
            self.conds.push((level, case.clone()));
            body(self, i);
            self.conds.pop();
        }
        self.conds = outer_conds;
        self.depth -= 1;
        let frame = self.frames.pop().unwrap();
        let mut ends = frame.breaks;
        ends.extend(std::mem::take(&mut self.open));
        if !cases.iter().any(|c| c == "default") {
            // no default: the switch can be skipped
            ends.push((id, CfgEdgeKind::Arm, Some("no match".into())));
        }
        self.open = ends;
    }

    /// A loop node; `body` builds its body. `infinite`: a `loop { }` / `for (;;)` has no "done" exit.
    pub(crate) fn loop_node(&mut self, label: &str, head_span: Span, infinite: bool, body: impl FnOnce(&mut Self)) {
        let id = self.seq(CfgNodeKind::Loop, shorten(label, 40), label.to_string(), head_span);
        self.open.clear();
        self.frames.push(Frame { head: id, breaks: vec![], is_switch: false });
        // conditions outside the loop do not explain its exits
        let outer_conds = std::mem::take(&mut self.conds);
        self.depth += 1;
        self.open.push((id, CfgEdgeKind::LoopBody, Some("each iteration".into())));
        body(self);
        // back edges
        let ends = std::mem::take(&mut self.open);
        for (from, _, _) in ends {
            self.edge(from, id, CfgEdgeKind::LoopBack, Some("next iteration".into()));
        }
        self.depth -= 1;
        self.conds = outer_conds;
        let frame = self.frames.pop().unwrap();
        self.open = frame.breaks;
        if !infinite {
            self.open.push((id, CfgEdgeKind::False, Some("done".into())));
        }
    }

    /// An explicit return; `what` labels the edge to the exit ("return", "return Err", "return NULL").
    pub(crate) fn return_node(&mut self, is_err: bool, text: String, span: Span, what: &str) {
        let kind = if is_err { CfgNodeKind::ReturnErr } else { CfgNodeKind::Return };
        let id = self.seq(kind, shorten(&text, 40), text, span);
        self.open.clear();
        let exit = self.exit;
        let label = self.exit_label(what);
        self.edge(id, exit, CfgEdgeKind::Return, Some(label));
    }

    /// `break`: the edge waits for the innermost loop (or switch) to close.
    pub(crate) fn break_node(&mut self, text: String, span: Span) {
        let id = self.seq(CfgNodeKind::Jump, "break".into(), text, span);
        self.open.clear();
        let label = self.cond_here().map(|c| shorten(&c, 40)).unwrap_or_else(|| "break".into());
        if let Some(frame) = self.frames.last_mut() {
            frame.breaks.push((id, CfgEdgeKind::Break, Some(label)));
        }
    }

    /// `continue`: back to the head of the innermost loop (a switch in between does not count).
    pub(crate) fn continue_node(&mut self, text: String, span: Span) {
        let id = self.seq(CfgNodeKind::Jump, "continue".into(), text, span);
        self.open.clear();
        if let Some(head) = self.frames.iter().rev().find(|f| !f.is_switch).map(|f| f.head) {
            self.edge(id, head, CfgEdgeKind::Continue, Some("continue".into()));
        }
    }

    /// A jump to a label (`goto`): the edge is added by the driver once the label is known.
    pub(crate) fn jump_node(&mut self, label: &str, text: String, span: Span) -> usize {
        let id = self.seq(CfgNodeKind::Jump, label.to_string(), text, span);
        self.open.clear();
        id
    }

    /// An edge the driver resolves itself (a `goto` to its label).
    pub(crate) fn link(&mut self, from: usize, to: usize, kind: CfgEdgeKind, label: Option<String>) {
        self.edge(from, to, kind, label);
    }

    /// `panic!` / `abort()` / a failing `assert`: an error edge to the exit; `stops` when nothing follows.
    pub(crate) fn panic_node(&mut self, text: String, span: Span, edge_label: &str, stops: bool) {
        let id = self.seq(CfgNodeKind::Panic, shorten(&text, 40), text, span);
        if stops {
            self.open.clear();
        }
        let exit = self.exit;
        self.edge(id, exit, CfgEdgeKind::Error, Some(edge_label.into()));
    }

    /// A logging statement: shown only when a comment above says why it matters.
    pub(crate) fn log_node(&mut self, text: String, span: Span) {
        if self.pending_note.is_some() {
            self.seq(CfgNodeKind::Log, shorten(&text, 40), text, span);
        }
    }

    /// `?` after a structured expression: the error edge to the exit.
    pub(crate) fn propagate_node(&mut self, text: String, span: Span) {
        let id = self.seq(CfgNodeKind::Propagate, "?".into(), text, span);
        let exit = self.exit;
        self.edge(id, exit, CfgEdgeKind::Error, Some("Err / None".into()));
    }
}

// ---- Rust driver (syn) ----

impl<'a> Builder<'a> {
    fn block(&mut self, b: &syn::Block) {
        for stmt in &b.stmts {
            self.stmt(stmt);
        }
    }

    fn stmt(&mut self, s: &syn::Stmt) {
        self.note_for(span_of(s, self.file).line_start);
        self.stmt_inner(s);
        self.clear_note();
    }

    fn stmt_inner(&mut self, s: &syn::Stmt) {
        match s {
            syn::Stmt::Local(l) => {
                let span = span_of(l, self.file);
                let pat = tokens_to_string(&l.pat);
                if let Some(init) = &l.init {
                    let e = &*init.expr;
                    if matches!(e, syn::Expr::If(_) | syn::Expr::Match(_)) {
                        // `let x = match …`: one node, "let x = match …" (or the comment above), arms from it
                        self.set_let_prefix(format!("let {} =", pat));
                        self.expr(e);
                        self.let_prefix = None;
                        return;
                    }
                    if is_structured(e) {
                        // `let x = loop { … }` : structural expr gets its own nodes
                        let label = format!("let {} =", pat);
                        self.seq(CfgNodeKind::Block, shorten(&label, 40), tokens_to_string(l), span_of(&l.pat, self.file));
                        self.expr(e);
                        return;
                    }
                    let mut f = facts(e);
                    f.is_let = true;
                    self.plain(&format!("let {} = {}", pat, tokens_to_string(e)), tokens_to_string(l), span, f);
                } else {
                    self.plain(&format!("let {}", pat), tokens_to_string(l), span, Facts { is_let: true, ..Default::default() });
                }
            }
            syn::Stmt::Expr(e, _) => self.expr(e),
            syn::Stmt::Item(_) => {}
            syn::Stmt::Macro(m) => {
                let span = span_of(m, self.file);
                let text = tokens_to_string(m);
                let name = path_last(&m.mac.path);
                if is_log_macro(&m.mac.path) {
                    // logging: shown only when a comment above says why it matters
                    self.log_node(text, span);
                    return;
                }
                if matches!(name.as_str(), "panic" | "unreachable" | "todo" | "unimplemented") {
                    self.panic_node(text, span, "panic", true);
                } else if matches!(name.as_str(), "assert" | "assert_eq" | "assert_ne" | "debug_assert") {
                    self.panic_node(text, span, "assert fails", false);
                } else {
                    self.plain(&text, text.clone(), span, Facts::default());
                }
            }
        }
    }

    fn expr(&mut self, e: &syn::Expr) {
        let span = span_of(e, self.file);
        match e {
            syn::Expr::If(i) => {
                let cond = tokens_to_string(&i.cond);
                let els = i.else_branch.as_ref().map(|(_, else_e)| move |b: &mut Self| b.expr(else_e));
                self.if_node(&cond, span_of(&i.cond, self.file), |b| b.block(&i.then_branch), els);
            }
            syn::Expr::Match(m) => {
                let scrut = tokens_to_string(&m.expr);
                let arms: Vec<Arm> = m
                    .arms
                    .iter()
                    .map(|arm| {
                        let mut pat = tokens_to_string(&arm.pat);
                        if let Some((_, g)) = &arm.guard {
                            pat.push_str(" if ");
                            pat.push_str(&tokens_to_string(g));
                        }
                        let trivial = is_trivial(&arm.body).then(|| tokens_to_string(&arm.body).trim_start_matches("{ ").trim_end_matches(" }").to_string());
                        Arm { pat, trivial }
                    })
                    .collect();
                self.match_node(&scrut, span_of(&m.expr, self.file), &arms, |b, i| b.arm_body(&m.arms[i].body));
            }
            syn::Expr::Loop(l) => self.loop_node("loop", span, true, |b| b.block(&l.body)),
            syn::Expr::While(w) => {
                let cond = tokens_to_string(&w.cond);
                self.loop_node(&format!("while {}", cond), span_of(&w.cond, self.file), false, |b| b.block(&w.body))
            }
            syn::Expr::ForLoop(f) => {
                let label = format!("for {} in {}", tokens_to_string(&f.pat), tokens_to_string(&f.expr));
                self.loop_node(&label, span_union(span_of(&f.pat, self.file), span_of(&f.expr, self.file)), false, |b| b.block(&f.body))
            }
            syn::Expr::Block(b) => self.block(&b.block),
            syn::Expr::Unsafe(u) => self.block(&u.block),
            syn::Expr::Async(a) => self.block(&a.block),
            syn::Expr::Return(r) => {
                let text = tokens_to_string(e);
                let what = r.expr.as_ref().map(|x| return_label(x)).unwrap_or_else(|| "return".into());
                self.return_node(what == "return Err", text, span, &what);
            }
            syn::Expr::Break(_) => self.break_node(tokens_to_string(e), span),
            syn::Expr::Continue(_) => self.continue_node(tokens_to_string(e), span),
            syn::Expr::Try(t) if is_structured(&t.expr) => {
                self.expr(&t.expr);
                self.propagate_node(tokens_to_string(e), span);
            }
            syn::Expr::Macro(m) if is_log_macro(&m.mac.path) => self.log_node(tokens_to_string(e), span),
            _ => {
                let text = tokens_to_string(e);
                self.plain(&text, text.clone(), span, facts(e));
            }
        }
    }

    fn arm_body(&mut self, body: &syn::Expr) {
        match body {
            syn::Expr::Block(b) => self.block(&b.block),
            other => self.expr(other),
        }
    }
}

fn return_label(e: &syn::Expr) -> String {
    match e {
        syn::Expr::Call(c) => {
            let name = tokens_to_string(&c.func);
            match name.as_str() {
                "Ok" => "return Ok".into(),
                "Err" => "return Err".into(),
                "Some" => "return Some".into(),
                _ => "return".into(),
            }
        }
        syn::Expr::Path(p) if p.path.is_ident("None") => "return None".into(),
        _ => "return".into(),
    }
}

fn is_structured(e: &syn::Expr) -> bool {
    matches!(
        e,
        syn::Expr::If(_) | syn::Expr::Match(_) | syn::Expr::Loop(_) | syn::Expr::While(_) | syn::Expr::ForLoop(_) | syn::Expr::Block(_) | syn::Expr::Unsafe(_)
    )
}

/// Build the CFG of a Rust function body. `calls` are the (already resolved) call edges of that function.
pub fn build<'a>(block: &syn::Block, file: FileId, calls: &'a [CallEdge], fn_span: Span, source: &'a str) -> Cfg {
    let mut b = Builder::new(file, calls, fn_span, source);
    b.block(block);
    // label the implicit tail expression as the return value
    let tail = match block.stmts.last() {
        Some(syn::Stmt::Expr(e, None)) => Some(return_label(e)),
        _ => None,
    };
    b.finish(tail)
}
