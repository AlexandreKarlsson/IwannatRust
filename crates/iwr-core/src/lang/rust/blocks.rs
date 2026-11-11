//! The block tree of a Rust function (Code view): `syn` statements into the language-neutral
//! [`Builder`].

use crate::blocks::Builder;
use crate::cfg::Facts;
use crate::model::*;
use crate::text::*;
use syn::visit::Visit;

/// Identifiers bound by a pattern.
fn pat_idents(p: &syn::Pat, out: &mut Vec<String>) {
    struct V<'a>(&'a mut Vec<String>);
    impl<'ast, 'a> Visit<'ast> for V<'a> {
        fn visit_pat_ident(&mut self, p: &'ast syn::PatIdent) {
            self.0.push(p.ident.to_string());
            syn::visit::visit_pat_ident(self, p);
        }
    }
    V(out).visit_pat(p);
}

/// Single-segment path identifiers used in an expression, not descending into nested blocks/closures.
fn expr_idents(e: &syn::Expr, shallow: bool) -> Vec<String> {
    struct V {
        out: Vec<String>,
        shallow: bool,
    }
    impl<'ast> Visit<'ast> for V {
        fn visit_expr_path(&mut self, p: &'ast syn::ExprPath) {
            if p.qself.is_none() && p.path.segments.len() == 1 {
                let s = p.path.segments[0].ident.to_string();
                if s != "self" && s != "Self" && !s.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) && !self.out.contains(&s) {
                    self.out.push(s);
                }
            }
        }
        fn visit_expr_field(&mut self, f: &'ast syn::ExprField) {
            // `self.x` / `task.name`: the base is the variable
            self.visit_expr(&f.base);
        }
        fn visit_expr_method_call(&mut self, m: &'ast syn::ExprMethodCall) {
            self.visit_expr(&m.receiver);
            for a in &m.args {
                self.visit_expr(a);
            }
        }
        fn visit_block(&mut self, b: &'ast syn::Block) {
            if !self.shallow {
                syn::visit::visit_block(self, b);
            }
        }
        fn visit_expr_closure(&mut self, c: &'ast syn::ExprClosure) {
            if !self.shallow {
                syn::visit::visit_expr_closure(self, c);
            }
        }
        fn visit_macro(&mut self, mac: &'ast syn::Macro) {
            if let Ok(args) = mac.parse_body_with(syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated) {
                for a in &args {
                    self.visit_expr(a);
                }
            }
        }
    }
    let mut v = V { out: vec![], shallow };
    v.visit_expr(e);
    v.out
}

fn facts(e: &syn::Expr) -> Facts {
    struct V(Facts);
    impl<'ast> Visit<'ast> for V {
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
        }
        fn visit_expr_closure(&mut self, _e: &'ast syn::ExprClosure) {
            self.0.has_closure = true;
        }
    }
    let mut v = V(Facts::default());
    v.visit_expr(e);
    v.0
}

fn is_structured(e: &syn::Expr) -> bool {
    matches!(e, syn::Expr::If(_) | syn::Expr::Match(_) | syn::Expr::Loop(_) | syn::Expr::While(_) | syn::Expr::ForLoop(_) | syn::Expr::Block(_) | syn::Expr::Unsafe(_))
}
impl<'a> Builder<'a> {
    fn uses_of(&self, e: &syn::Expr, shallow: bool) -> Vec<String> {
        self.uses_in_scope(expr_idents(e, shallow))
    }

    fn block(&mut self, b: &syn::Block, out: &mut Vec<Block>) {
        self.push_scope(&[]);
        for s in &b.stmts {
            self.stmt(s, out);
        }
        self.pop_scope();
    }

    fn stmt(&mut self, s: &syn::Stmt, out: &mut Vec<Block>) {
        match s {
            syn::Stmt::Local(l) => {
                let span = span_of(l, self.file());
                let mut defs = Vec::new();
                pat_idents(&l.pat, &mut defs);
                let pat = tokens_to_string(&l.pat);
                match &l.init {
                    Some(init) if is_structured(&init.expr) => {
                        let mut b = self.new_block(BlockKind::Let, format!("let {} =", pat), tokens_to_string(l), span);
                        let mut kids = Vec::new();
                        self.expr(&init.expr, &mut kids);
                        b.children = kids;
                        self.define(&defs);
                        b.defines = defs;
                        out.push(b);
                    }
                    Some(init) => {
                        let e = &*init.expr;
                        let f = facts(e);
                        let kind = self.leaf_kind(&f, &span, BlockKind::Let);
                        let mut b = self.new_block(kind, format!("let {} = {}", pat, tokens_to_string(e)), tokens_to_string(l), span);
                        b.uses = self.uses_of(e, false);
                        if f.has_closure {
                            self.closure_children(e, &mut b);
                        }
                        self.define(&defs);
                        b.defines = defs;
                        out.push(b);
                    }
                    None => {
                        let mut b = self.new_block(BlockKind::Let, format!("let {}", pat), tokens_to_string(l), span);
                        self.define(&defs);
                        b.defines = defs;
                        out.push(b);
                    }
                }
            }
            syn::Stmt::Expr(e, _) => self.expr(e, out),
            syn::Stmt::Item(_) => {}
            syn::Stmt::Macro(m) => {
                let span = span_of(m, self.file());
                let name = path_last(&m.mac.path);
                let kind = if matches!(name.as_str(), "panic" | "unreachable" | "todo" | "unimplemented" | "assert" | "assert_eq" | "assert_ne") { BlockKind::Panic } else { BlockKind::Macro };
                let mut b = self.new_block(kind, tokens_to_string(m), tokens_to_string(m), span);
                if let Ok(args) = m.mac.parse_body_with(syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated) {
                    for a in &args {
                        for u in self.uses_of(a, false) {
                            if !b.uses.contains(&u) {
                                b.uses.push(u);
                            }
                        }
                    }
                }
                out.push(b);
            }
        }
    }

    /// Closures inside a statement become child blocks so their body is explorable.
    fn closure_children(&mut self, e: &syn::Expr, parent: &mut Block) {
        struct V<'b>(Vec<&'b syn::ExprClosure>);
        impl<'ast> Visit<'ast> for V<'ast> {
            fn visit_expr_closure(&mut self, c: &'ast syn::ExprClosure) {
                self.0.push(c);
            }
        }
        let mut v = V(vec![]);
        v.visit_expr(e);
        for c in v.0 {
            let span = span_of(c, self.file());
            let args: Vec<String> = c.inputs.iter().map(tokens_to_string).collect();
            let mut b = self.new_block(BlockKind::Closure, format!("|{}|", args.join(", ")), tokens_to_string(c), span);
            let mut defs = Vec::new();
            for p in &c.inputs {
                pat_idents(p, &mut defs);
            }
            self.push_scope(&defs);
            let mut kids = Vec::new();
            match &*c.body {
                syn::Expr::Block(bl) => self.block(&bl.block, &mut kids),
                other => self.expr(other, &mut kids),
            }
            self.pop_scope();
            b.defines = defs;
            b.children = kids;
            parent.children.push(b);
        }
    }

    fn expr(&mut self, e: &syn::Expr, out: &mut Vec<Block>) {
        let span = span_of(e, self.file());
        match e {
            syn::Expr::If(i) => {
                let mut cur = i;
                let mut first = true;
                loop {
                    let cond = tokens_to_string(&cur.cond);
                    let kind = if first { BlockKind::If } else { BlockKind::ElseIf };
                    let mut b = self.new_block(kind, format!("{} {}", if first { "if" } else { "else if" }, cond), cond.clone(), span_of(cur, self.file()));
                    b.uses = self.uses_of(&cur.cond, true);
                    // `if let PAT = expr` binds variables for the then-branch
                    let mut defs = Vec::new();
                    if let syn::Expr::Let(l) = &*cur.cond {
                        pat_idents(&l.pat, &mut defs);
                    }
                    self.push_scope(&defs);
                    let mut kids = Vec::new();
                    self.block(&cur.then_branch, &mut kids);
                    self.pop_scope();
                    b.defines = defs;
                    b.children = kids;
                    out.push(b);
                    first = false;
                    match cur.else_branch.as_ref().map(|(_, e)| &**e) {
                        Some(syn::Expr::If(next)) => cur = next,
                        Some(syn::Expr::Block(bl)) => {
                            let mut eb = self.new_block(BlockKind::Else, "else".into(), "else".into(), span_of(bl, self.file()));
                            let mut kids = Vec::new();
                            self.block(&bl.block, &mut kids);
                            eb.children = kids;
                            out.push(eb);
                            break;
                        }
                        Some(other) => {
                            let mut eb = self.new_block(BlockKind::Else, "else".into(), "else".into(), span_of(other, self.file()));
                            let mut kids = Vec::new();
                            self.expr(other, &mut kids);
                            eb.children = kids;
                            out.push(eb);
                            break;
                        }
                        None => break,
                    }
                }
            }
            syn::Expr::Match(m) => {
                let scrut = tokens_to_string(&m.expr);
                let mut b = self.new_block(BlockKind::Match, format!("match {}", scrut), scrut, span);
                b.uses = self.uses_of(&m.expr, true);
                for arm in &m.arms {
                    let mut pat = tokens_to_string(&arm.pat);
                    if let Some((_, g)) = &arm.guard {
                        pat.push_str(" if ");
                        pat.push_str(&tokens_to_string(g));
                    }
                    let mut ab = self.new_block(BlockKind::Arm, pat.clone(), pat, span_of(arm, self.file()));
                    let mut defs = Vec::new();
                    pat_idents(&arm.pat, &mut defs);
                    self.push_scope(&defs);
                    let mut kids = Vec::new();
                    match &*arm.body {
                        syn::Expr::Block(bl) => self.block(&bl.block, &mut kids),
                        other => self.expr(other, &mut kids),
                    }
                    self.pop_scope();
                    ab.defines = defs;
                    ab.children = kids;
                    b.children.push(ab);
                }
                out.push(b);
            }
            syn::Expr::Loop(l) => {
                let mut b = self.new_block(BlockKind::Loop, "loop".into(), "loop".into(), span);
                let mut kids = Vec::new();
                self.block(&l.body, &mut kids);
                b.children = kids;
                out.push(b);
            }
            syn::Expr::While(w) => {
                let cond = tokens_to_string(&w.cond);
                let mut b = self.new_block(BlockKind::Loop, format!("while {}", cond), cond, span);
                b.uses = self.uses_of(&w.cond, true);
                let mut defs = Vec::new();
                if let syn::Expr::Let(l) = &*w.cond {
                    pat_idents(&l.pat, &mut defs);
                }
                self.push_scope(&defs);
                let mut kids = Vec::new();
                self.block(&w.body, &mut kids);
                self.pop_scope();
                b.defines = defs;
                b.children = kids;
                out.push(b);
            }
            syn::Expr::ForLoop(f) => {
                let label = format!("for {} in {}", tokens_to_string(&f.pat), tokens_to_string(&f.expr));
                let mut b = self.new_block(BlockKind::Loop, label.clone(), label, span);
                b.uses = self.uses_of(&f.expr, true);
                let mut defs = Vec::new();
                pat_idents(&f.pat, &mut defs);
                self.push_scope(&defs);
                let mut kids = Vec::new();
                self.block(&f.body, &mut kids);
                self.pop_scope();
                b.defines = defs;
                b.children = kids;
                out.push(b);
            }
            syn::Expr::Block(bl) => self.block(&bl.block, out),
            syn::Expr::Unsafe(u) => {
                let mut b = self.new_block(BlockKind::Unsafe, "unsafe".into(), "unsafe".into(), span);
                let mut kids = Vec::new();
                self.block(&u.block, &mut kids);
                b.children = kids;
                out.push(b);
            }
            syn::Expr::Async(a) => {
                let mut b = self.new_block(BlockKind::Await, "async".into(), "async".into(), span);
                let mut kids = Vec::new();
                self.block(&a.block, &mut kids);
                b.children = kids;
                out.push(b);
            }
            syn::Expr::Return(r) => {
                let mut b = self.new_block(BlockKind::Return, tokens_to_string(e), tokens_to_string(e), span);
                if let Some(x) = &r.expr {
                    b.uses = self.uses_of(x, false);
                }
                out.push(b);
            }
            syn::Expr::Break(_) => out.push(self.new_block(BlockKind::Break, "break".into(), tokens_to_string(e), span)),
            syn::Expr::Continue(_) => out.push(self.new_block(BlockKind::Continue, "continue".into(), tokens_to_string(e), span)),
            _ => {
                let f = facts(e);
                let kind = self.leaf_kind(&f, &span, BlockKind::Stmt);
                let text = tokens_to_string(e);
                let mut b = self.new_block(kind, text.clone(), text, span);
                b.uses = self.uses_of(e, false);
                if f.has_closure {
                    self.closure_children(e, &mut b);
                }
                out.push(b);
            }
        }
    }
}

/// Build the block tree of a Rust function.
pub fn build(sig: &syn::Signature, body: &syn::Block, file: FileId, calls: &[CallEdge], fn_span: Span, source: &str) -> Block {
    let mut params = Vec::new();
    for input in &sig.inputs {
        match input {
            syn::FnArg::Receiver(_) => params.push("self".to_string()),
            syn::FnArg::Typed(t) => pat_idents(&t.pat, &mut params),
        }
    }
    let mut b = Builder::new(file, calls, &params);
    let mut kids = Vec::new();
    b.block(body, &mut kids);
    b.root(tokens_to_string(sig), fn_span, params, kids, source)
}
