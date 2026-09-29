//! Build a control-flow graph from a function body.
//!
//! The graph is statement-level: consecutive plain statements are merged into
//! one `Block` node; `if` / `match` / loops / `return` / `?` / panics / awaits
//! / calls to project functions get their own nodes so the UI can show them
//! distinctly.

use crate::model::*;
use crate::text::*;
use syn::visit::Visit;

struct Builder<'a> {
    cfg: Cfg,
    file: FileId,
    /// Open ends: nodes whose "next" edge is still pending, with the label to use.
    open: Vec<(usize, CfgEdgeKind, Option<String>)>,
    /// (loop node id, pending break sources)
    loops: Vec<(usize, Vec<(usize, CfgEdgeKind, Option<String>)>)>,
    exit: usize,
    calls: &'a [CallEdge],
    depth: usize,
}

fn contains(outer: &Span, inner: &Span) -> bool {
    outer.file == inner.file
        && (outer.line_start, outer.col_start) <= (inner.line_start, inner.col_start)
        && (outer.line_end, outer.col_end) >= (inner.line_end, inner.col_end)
}

/// Facts about an expression subtree that decide how it is rendered.
#[derive(Default)]
struct ExprFacts {
    has_try: bool,
    has_await: bool,
    has_panic: bool,
    has_unwrap: bool,
    has_return: bool,
}

struct FactsVisitor(ExprFacts);
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

fn facts(e: &syn::Expr) -> ExprFacts {
    let mut v = FactsVisitor(ExprFacts::default());
    v.visit_expr(e);
    v.0
}

impl<'a> Builder<'a> {
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
        self.cfg.nodes.push(CfgNode { id, kind, label, detail, span, calls: calls_dedup, depth: self.depth });
        id
    }

    fn edge(&mut self, from: usize, to: usize, kind: CfgEdgeKind, label: Option<String>) {
        if self.cfg.edges.iter().any(|e| e.from == from && e.to == to && e.kind == kind) {
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
    fn seq(&mut self, kind: CfgNodeKind, label: String, detail: String, span: Span) -> usize {
        let id = self.add(kind, label, detail, span);
        self.flow_into(id);
        self.open.push((id, CfgEdgeKind::Next, None));
        id
    }

    fn block(&mut self, b: &syn::Block) {
        for stmt in &b.stmts {
            self.stmt(stmt);
        }
    }

    fn stmt(&mut self, s: &syn::Stmt) {
        match s {
            syn::Stmt::Local(l) => {
                let span = span_of(l, self.file);
                let pat = tokens_to_string(&l.pat);
                if let Some(init) = &l.init {
                    let e = &*init.expr;
                    if is_structured(e) {
                        // `let x = if ... { } else { }` : structural expr gets its own nodes
                        let label = format!("let {} =", pat);
                        self.seq(CfgNodeKind::Block, shorten(&label, 40), tokens_to_string(l), span_of(&l.pat, self.file));
                        self.expr(e);
                        return;
                    }
                    self.plain(&format!("let {} = {}", pat, tokens_to_string(e)), tokens_to_string(l), span, Some(e));
                } else {
                    self.plain(&format!("let {}", pat), tokens_to_string(l), span, None);
                }
            }
            syn::Stmt::Expr(e, _) => self.expr(e),
            syn::Stmt::Item(_) => {}
            syn::Stmt::Macro(m) => {
                let span = span_of(m, self.file);
                let text = tokens_to_string(m);
                let name = path_last(&m.mac.path);
                if matches!(name.as_str(), "panic" | "unreachable" | "todo" | "unimplemented") {
                    let id = self.seq(CfgNodeKind::Panic, shorten(&text, 40), text, span);
                    self.open.clear();
                    let exit = self.exit;
                    self.edge(id, exit, CfgEdgeKind::Error, Some("panic".into()));
                } else if matches!(name.as_str(), "assert" | "assert_eq" | "assert_ne" | "debug_assert") {
                    let id = self.seq(CfgNodeKind::Panic, shorten(&text, 40), text, span);
                    let exit = self.exit;
                    self.edge(id, exit, CfgEdgeKind::Error, Some("assert fails".into()));
                } else {
                    self.plain(&text, text.clone(), span, None);
                }
            }
        }
    }

    /// A plain statement: merged into the previous Block node when possible,
    /// unless it contains something interesting (call / ? / await / unwrap).
    fn plain(&mut self, label: &str, detail: String, span: Span, e: Option<&syn::Expr>) {
        let f = e.map(facts).unwrap_or_default();
        let has_calls = self.calls.iter().any(|c| c.to.is_some() && c.kind != CallKind::Construct && contains(&span, &c.span));
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
        } else {
            CfgNodeKind::Block
        };
        if kind == CfgNodeKind::Block {
            // try to merge with previous block node
            if self.open.len() == 1 {
                let (prev, k, _) = self.open[0].clone();
                if k == CfgEdgeKind::Next && self.cfg.nodes[prev].kind == CfgNodeKind::Block && self.cfg.nodes[prev].depth == self.depth {
                    let n = &mut self.cfg.nodes[prev];
                    if n.label.lines().count() < 4 {
                        n.label.push('\n');
                        n.label.push_str(&shorten(label, 40));
                        n.detail.push('\n');
                        n.detail.push_str(&detail);
                        n.span = span_union(n.span, span);
                        return;
                    }
                }
            }
            self.seq(CfgNodeKind::Block, shorten(label, 40), detail, span);
            return;
        }
        let id = self.seq(kind, shorten(label, 40), detail, span);
        let exit = self.exit;
        match kind {
            CfgNodeKind::Propagate => self.edge(id, exit, CfgEdgeKind::Error, Some("Err / None".into())),
            CfgNodeKind::Panic => self.edge(id, exit, CfgEdgeKind::Error, Some("panic".into())),
            CfgNodeKind::Return => {
                self.open.clear();
                self.edge(id, exit, CfgEdgeKind::Return, Some("return".into()));
            }
            _ => {}
        }
    }

    fn expr(&mut self, e: &syn::Expr) {
        let span = span_of(e, self.file);
        match e {
            syn::Expr::If(i) => {
                let cond = tokens_to_string(&i.cond);
                let id = self.seq(CfgNodeKind::If, shorten(&format!("if {}", cond), 40), format!("if {}", cond), span_of(&i.cond, self.file));
                self.open.clear();
                self.depth += 1;
                // then
                self.open.push((id, CfgEdgeKind::True, Some("true".into())));
                self.block(&i.then_branch);
                let then_ends = std::mem::take(&mut self.open);
                // else
                self.open.push((id, CfgEdgeKind::False, Some("false".into())));
                if let Some((_, else_e)) = &i.else_branch {
                    self.expr(else_e);
                }
                self.depth -= 1;
                let mut ends = std::mem::take(&mut self.open);
                ends.extend(then_ends);
                self.open = ends;
            }
            syn::Expr::Match(m) => {
                let scrut = tokens_to_string(&m.expr);
                let id = self.seq(CfgNodeKind::Match, shorten(&format!("match {}", scrut), 40), format!("match {}", scrut), span_of(&m.expr, self.file));
                self.open.clear();
                self.depth += 1;
                let mut ends = Vec::new();
                for arm in &m.arms {
                    let mut pat = tokens_to_string(&arm.pat);
                    if let Some((_, g)) = &arm.guard {
                        pat.push_str(" if ");
                        pat.push_str(&tokens_to_string(g));
                    }
                    self.open.push((id, CfgEdgeKind::Arm, Some(shorten(&pat, 24))));
                    self.arm_body(&arm.body);
                    ends.extend(std::mem::take(&mut self.open));
                }
                self.depth -= 1;
                self.open = ends;
            }
            syn::Expr::Loop(l) => self.loop_node("loop", &l.body, span, None),
            syn::Expr::While(w) => {
                let cond = tokens_to_string(&w.cond);
                self.loop_node(&format!("while {}", cond), &w.body, span_of(&w.cond, self.file), Some(span))
            }
            syn::Expr::ForLoop(f) => {
                let label = format!("for {} in {}", tokens_to_string(&f.pat), tokens_to_string(&f.expr));
                self.loop_node(&label, &f.body, span_union(span_of(&f.pat, self.file), span_of(&f.expr, self.file)), Some(span))
            }
            syn::Expr::Block(b) => self.block(&b.block),
            syn::Expr::Unsafe(u) => self.block(&u.block),
            syn::Expr::Async(a) => self.block(&a.block),
            syn::Expr::Return(r) => {
                let text = tokens_to_string(e);
                let has_calls = r.expr.as_ref().map(|x| {
                    let s = span_of(&**x, self.file);
                    self.calls.iter().any(|c| c.to.is_some() && contains(&s, &c.span))
                }).unwrap_or(false);
                let _ = has_calls;
                let id = self.seq(CfgNodeKind::Return, shorten(&text, 40), text, span);
                self.open.clear();
                let exit = self.exit;
                let label = r.expr.as_ref().map(|x| return_label(x)).unwrap_or_else(|| "return".into());
                self.edge(id, exit, CfgEdgeKind::Return, Some(label));
            }
            syn::Expr::Break(_) => {
                let id = self.seq(CfgNodeKind::Jump, "break".into(), tokens_to_string(e), span);
                self.open.clear();
                if let Some((_, breaks)) = self.loops.last_mut() {
                    breaks.push((id, CfgEdgeKind::Break, Some("break".into())));
                }
            }
            syn::Expr::Continue(_) => {
                let id = self.seq(CfgNodeKind::Jump, "continue".into(), tokens_to_string(e), span);
                self.open.clear();
                if let Some((head, _)) = self.loops.last() {
                    let head = *head;
                    self.edge(id, head, CfgEdgeKind::Continue, Some("continue".into()));
                }
            }
            syn::Expr::Try(t) if is_structured(&t.expr) => {
                self.expr(&t.expr);
                let id = self.seq(CfgNodeKind::Propagate, "?".into(), tokens_to_string(e), span);
                let exit = self.exit;
                self.edge(id, exit, CfgEdgeKind::Error, Some("Err / None".into()));
            }
            _ => {
                let text = tokens_to_string(e);
                self.plain(&text, text.clone(), span, Some(e));
            }
        }
    }

    fn arm_body(&mut self, body: &syn::Expr) {
        match body {
            syn::Expr::Block(b) => self.block(&b.block),
            other => self.expr(other),
        }
    }

    fn loop_node(&mut self, label: &str, body: &syn::Block, head_span: Span, _whole: Option<Span>) {
        let id = self.seq(CfgNodeKind::Loop, shorten(label, 40), label.to_string(), head_span);
        self.open.clear();
        self.loops.push((id, vec![]));
        self.depth += 1;
        self.open.push((id, CfgEdgeKind::LoopBody, Some("each iteration".into())));
        self.block(body);
        // back edges
        let ends = std::mem::take(&mut self.open);
        for (from, _, _) in ends {
            self.edge(from, id, CfgEdgeKind::LoopBack, Some("next iteration".into()));
        }
        self.depth -= 1;
        let (_, breaks) = self.loops.pop().unwrap();
        self.open = breaks;
        if !label.starts_with("loop") {
            self.open.push((id, CfgEdgeKind::False, Some("done".into())));
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

/// Build the CFG of a function body. `calls` are the (already resolved) call edges of that function.
pub fn build(block: &syn::Block, file: FileId, calls: &[CallEdge], fn_span: Span) -> Cfg {
    let mut b = Builder { cfg: Cfg::default(), file, open: vec![], loops: vec![], exit: 0, calls, depth: 0 };
    let entry = b.add(CfgNodeKind::Entry, "start".into(), "function entry".into(), Span { file, line_start: fn_span.line_start, col_start: fn_span.col_start, line_end: fn_span.line_start, col_end: fn_span.col_start });
    let exit = b.add(CfgNodeKind::Exit, "end".into(), "function exit".into(), Span { file, line_start: fn_span.line_end, col_start: fn_span.col_end, line_end: fn_span.line_end, col_end: fn_span.col_end });
    b.exit = exit;
    b.open.push((entry, CfgEdgeKind::Next, None));
    b.block(block);
    // whatever is still open falls through to the exit
    let open = std::mem::take(&mut b.open);
    for (from, kind, label) in open {
        let label = if kind == CfgEdgeKind::Next { None } else { label };
        b.edge(from, exit, kind, label);
    }
    // label the implicit tail expression as the return value
    if let Some(syn::Stmt::Expr(e, None)) = block.stmts.last() {
        let lbl = return_label(e);
        for edge in &mut b.cfg.edges {
            if edge.to == exit && edge.kind == CfgEdgeKind::Next && edge.label.is_none() {
                edge.label = Some(lbl.clone());
            }
        }
    }
    b.cfg
}
