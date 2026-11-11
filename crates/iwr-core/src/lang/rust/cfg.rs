//! The flow graph of a Rust function: `syn` statements into the language-neutral [`Builder`].

use crate::cfg::{Arm, Builder, Facts};
use crate::model::*;
use crate::text::*;
use syn::visit::Visit;

/// Logging macros: noise in a flow graph unless a comment says why they matter.
fn is_log_macro(path: &syn::Path) -> bool {
    let first = path.segments.first().map(|s| s.ident.to_string()).unwrap_or_default();
    let last = path_last(path);
    matches!(first.as_str(), "log" | "tracing") || matches!(last.as_str(), "trace" | "debug" | "info" | "warn" | "error" | "log" | "event")
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

impl<'a> Builder<'a> {
    fn block(&mut self, b: &syn::Block) {
        for stmt in &b.stmts {
            self.stmt(stmt);
        }
    }

    fn stmt(&mut self, s: &syn::Stmt) {
        self.note_for(span_of(s, self.file()).line_start);
        self.stmt_inner(s);
        self.clear_note();
    }

    fn stmt_inner(&mut self, s: &syn::Stmt) {
        match s {
            syn::Stmt::Local(l) => {
                let span = span_of(l, self.file());
                let pat = tokens_to_string(&l.pat);
                if let Some(init) = &l.init {
                    let e = &*init.expr;
                    if matches!(e, syn::Expr::If(_) | syn::Expr::Match(_)) {
                        // `let x = match …`: one node, "let x = match …" (or the comment above), arms from it
                        self.set_let_prefix(format!("let {} =", pat));
                        self.expr(e);
                        self.clear_let_prefix();
                        return;
                    }
                    if is_structured(e) {
                        // `let x = loop { … }` : structural expr gets its own nodes
                        let label = format!("let {} =", pat);
                        self.seq(CfgNodeKind::Block, shorten(&label, 40), tokens_to_string(l), span_of(&l.pat, self.file()));
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
                let span = span_of(m, self.file());
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
        let span = span_of(e, self.file());
        match e {
            syn::Expr::If(i) => {
                let cond = tokens_to_string(&i.cond);
                let els = i.else_branch.as_ref().map(|(_, else_e)| move |b: &mut Self| b.expr(else_e));
                self.if_node(&cond, span_of(&i.cond, self.file()), |b| b.block(&i.then_branch), els);
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
                self.match_node(&scrut, span_of(&m.expr, self.file()), &arms, |b, i| b.arm_body(&m.arms[i].body));
            }
            syn::Expr::Loop(l) => self.loop_node("loop", span, true, |b| b.block(&l.body)),
            syn::Expr::While(w) => {
                let cond = tokens_to_string(&w.cond);
                self.loop_node(&format!("while {}", cond), span_of(&w.cond, self.file()), false, |b| b.block(&w.body))
            }
            syn::Expr::ForLoop(f) => {
                let label = format!("for {} in {}", tokens_to_string(&f.pat), tokens_to_string(&f.expr));
                self.loop_node(&label, span_union(span_of(&f.pat, self.file()), span_of(&f.expr, self.file())), false, |b| b.block(&f.body))
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
