//! Calls and counts inside a Rust function body: a `syn` visitor resolves every call by name
//! (see the heuristics in [`crate::resolve`]) and tags it (`in_loop`, `propagated`, `awaited`, …).

use crate::lang::Analyzed;
use crate::model::*;
use crate::resolve::{prefer_module, resolve_assoc, resolve_method, resolve_type, Index, Stats};
use crate::text::*;
use std::collections::HashSet;
use syn::visit::Visit;

struct CallVisitor<'a> {
    p: &'a Project,
    ix: &'a Index,
    file: FileId,
    from: ItemId,
    module: ModuleId,
    owner: Option<String>,
    hint_types: HashSet<String>,
    loop_depth: usize,
    branch_depth: usize,
    propagated: HashSet<(usize, usize, usize, usize)>,
    unwrapped: HashSet<(usize, usize, usize, usize)>,
    awaited: HashSet<(usize, usize, usize, usize)>,
    edges: Vec<CallEdge>,
    stats: Stats,
}

fn key(s: &Span) -> (usize, usize, usize, usize) {
    (s.line_start, s.col_start, s.line_end, s.col_end)
}

fn strip_await(e: &syn::Expr) -> &syn::Expr {
    match e {
        syn::Expr::Await(a) => strip_await(&a.base),
        syn::Expr::Paren(p) => strip_await(&p.expr),
        other => other,
    }
}

impl<'a> CallVisitor<'a> {
    fn push(&mut self, to: Option<ItemId>, callee: String, kind: CallKind, span: Span) {
        let k = key(&span);
        self.edges.push(CallEdge {
            from: self.from,
            to,
            callee,
            kind,
            span,
            in_loop: self.loop_depth > 0,
            in_branch: self.branch_depth > 0,
            propagated: self.propagated.contains(&k),
            unwrapped: self.unwrapped.contains(&k),
            is_await: self.awaited.contains(&k),
            is_recursive: to == Some(self.from),
        });
    }
}

impl<'ast, 'a> Visit<'ast> for CallVisitor<'a> {
    fn visit_expr_call(&mut self, c: &'ast syn::ExprCall) {
        let span = span_of(c, self.file);
        if let syn::Expr::Path(ep) = &*c.func {
            let segs = path_segments(&ep.path);
            let callee = tokens_to_string(&ep.path);
            match segs.len() {
                1 => {
                    let n = &segs[0];
                    if let Some(id) = self.ix.fns.get(n).and_then(|c| prefer_module(self.p, self.ix, c, self.module)) {
                        self.push(Some(id), callee, CallKind::Call, span);
                    } else if let Some(id) = resolve_type(self.p, self.ix, n, self.module) {
                        // tuple struct constructor
                        self.push(Some(id), callee, CallKind::Construct, span);
                    } else if !matches!(n.as_str(), "Some" | "Ok" | "Err" | "Box") {
                        self.push(None, callee, CallKind::Call, span);
                    }
                }
                _ => {
                    let last = segs[segs.len() - 1].clone();
                    let ty = segs[segs.len() - 2].clone();
                    let ty = if ty == "Self" { self.owner.clone().unwrap_or(ty) } else { ty };
                    if let Some(id) = resolve_assoc(self.p, self.ix, &ty, &last, self.module) {
                        self.push(Some(id), callee, CallKind::Call, span);
                    } else if let Some(tid) = resolve_type(self.p, self.ix, &ty, self.module) {
                        // Enum::Variant(..) or unknown assoc fn on a project type
                        let is_enum = self.p.items[tid].kind == ItemKind::Enum;
                        if is_enum {
                            self.push(Some(tid), callee, CallKind::Construct, span);
                        } else {
                            self.push(None, callee, CallKind::Call, span);
                        }
                    } else if let Some(id) = self.ix.fns.get(&last).and_then(|c| {
                        // module::function
                        let suffix = format!("{}::{}", ty, last);
                        let found: Vec<ItemId> = c.iter().copied().filter(|id| self.p.items[*id].path.ends_with(&suffix)).collect();
                        prefer_module(self.p, self.ix, &found, self.module)
                    }) {
                        self.push(Some(id), callee, CallKind::Call, span);
                    } else {
                        self.push(None, callee, CallKind::Call, span);
                    }
                }
            }
        }
        syn::visit::visit_expr_call(self, c);
    }

    fn visit_expr_method_call(&mut self, m: &'ast syn::ExprMethodCall) {
        let span = span_of(m, self.file);
        let name = m.method.to_string();
        match name.as_str() {
            "unwrap" => {
                self.stats.unwraps += 1;
                self.unwrapped.insert(key(&span_of(strip_await(&m.receiver), self.file)));
            }
            "expect" => {
                self.stats.expects += 1;
                self.unwrapped.insert(key(&span_of(strip_await(&m.receiver), self.file)));
            }
            _ => {}
        }
        let receiver_is_self = matches!(&*m.receiver, syn::Expr::Path(p) if p.path.is_ident("self"));
        let callee = format!("{}.{}", shorten(&tokens_to_string(&m.receiver), 20), name);
        let resolved = resolve_method(self.p, self.ix, &name, receiver_is_self, self.owner.as_deref(), self.module, &self.hint_types);
        if resolved.is_some() {
            self.push(resolved, callee, CallKind::MethodCall, span);
        }
        syn::visit::visit_expr_method_call(self, m);
    }

    fn visit_expr_struct(&mut self, s: &'ast syn::ExprStruct) {
        let segs = path_segments(&s.path);
        let name = if segs.len() >= 2 && self.ix.types.contains_key(&segs[segs.len() - 2]) { segs[segs.len() - 2].clone() } else { segs.last().cloned().unwrap_or_default() };
        let name = if name == "Self" { self.owner.clone().unwrap_or(name) } else { name };
        if let Some(id) = resolve_type(self.p, self.ix, &name, self.module) {
            self.push(Some(id), tokens_to_string(&s.path), CallKind::Construct, span_of(s, self.file));
        }
        syn::visit::visit_expr_struct(self, s);
    }

    fn visit_expr_macro(&mut self, e: &'ast syn::ExprMacro) {
        self.visit_macro(&e.mac);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        let name = path_last(&mac.path);
        let span = span_of(mac, self.file);
        if matches!(name.as_str(), "panic" | "unreachable" | "todo" | "unimplemented") {
            self.stats.panics += 1;
        }
        self.push(None, format!("{}!", name), CallKind::Macro, span);
        // look for calls inside macro arguments, e.g. println!("{}", compute(x))
        if let Ok(args) = mac.parse_body_with(syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated) {
            for a in &args {
                self.visit_expr(a);
            }
        }
    }

    fn visit_expr_try(&mut self, t: &'ast syn::ExprTry) {
        self.stats.question_marks += 1;
        let inner = strip_await(&t.expr);
        self.propagated.insert(key(&span_of(inner, self.file)));
        syn::visit::visit_expr_try(self, t);
    }

    fn visit_expr_await(&mut self, a: &'ast syn::ExprAwait) {
        self.stats.awaits += 1;
        self.awaited.insert(key(&span_of(strip_await(&a.base), self.file)));
        syn::visit::visit_expr_await(self, a);
    }

    fn visit_expr_if(&mut self, i: &'ast syn::ExprIf) {
        self.stats.branches += 1;
        self.visit_expr(&i.cond);
        self.branch_depth += 1;
        self.visit_block(&i.then_branch);
        if let Some((_, e)) = &i.else_branch {
            self.visit_expr(e);
        }
        self.branch_depth -= 1;
    }

    fn visit_expr_match(&mut self, m: &'ast syn::ExprMatch) {
        self.stats.branches += 1;
        self.visit_expr(&m.expr);
        self.branch_depth += 1;
        for arm in &m.arms {
            self.visit_arm(arm);
        }
        self.branch_depth -= 1;
    }

    fn visit_expr_loop(&mut self, l: &'ast syn::ExprLoop) {
        self.stats.loops += 1;
        self.loop_depth += 1;
        self.visit_block(&l.body);
        self.loop_depth -= 1;
    }
    fn visit_expr_while(&mut self, w: &'ast syn::ExprWhile) {
        self.stats.loops += 1;
        self.visit_expr(&w.cond);
        self.loop_depth += 1;
        self.visit_block(&w.body);
        self.loop_depth -= 1;
    }
    fn visit_expr_for_loop(&mut self, f: &'ast syn::ExprForLoop) {
        self.stats.loops += 1;
        self.visit_expr(&f.expr);
        self.loop_depth += 1;
        self.visit_block(&f.body);
        self.loop_depth -= 1;
    }
    fn visit_item(&mut self, _i: &'ast syn::Item) {
        // nested items (fn inside fn) are not part of this body's flow
    }
}

/// Identifiers used as types inside the body + signature (hints for method resolution).
fn hint_types(body: &syn::Block, sig: &syn::Signature) -> HashSet<String> {
    struct V(HashSet<String>);
    impl<'ast> Visit<'ast> for V {
        fn visit_type_path(&mut self, t: &'ast syn::TypePath) {
            for s in &t.path.segments {
                self.0.insert(s.ident.to_string());
            }
            syn::visit::visit_type_path(self, t);
        }
        fn visit_expr_path(&mut self, p: &'ast syn::ExprPath) {
            let segs = path_segments(&p.path);
            if segs.len() >= 2 {
                self.0.insert(segs[segs.len() - 2].clone());
            }
            syn::visit::visit_expr_path(self, p);
        }
        fn visit_expr_struct(&mut self, s: &'ast syn::ExprStruct) {
            for seg in &s.path.segments {
                self.0.insert(seg.ident.to_string());
            }
            syn::visit::visit_expr_struct(self, s);
        }
    }
    let mut v = V(HashSet::new());
    v.visit_block(body);
    v.visit_signature(sig);
    v.0
}

/// Calls, flow graph and block tree of one Rust function body.
pub(crate) fn analyze(p: &Project, ix: &Index, item: ItemId, file: FileId, block: &syn::Block, sig: &syn::Signature) -> Analyzed {
    let fn_span = p.items[item].span;
    let source = &p.files[file].content;
    let module = p.items[item].module;
    let owner = p.items[item].fn_info().and_then(|f| f.owner_type_name.clone());
    let mut hints = hint_types(block, sig);
    // owner type's generics / bounds / field types are visible to methods (`S: Storage` etc)
    if let Some(owner_id) = p.items[item].fn_info().and_then(|f| f.owner_type) {
        let owner_item = &p.items[owner_id];
        let mut text = owner_item.signature.clone();
        if let ItemExtra::Struct(s) = &owner_item.extra {
            for f in &s.fields {
                text.push(' ');
                text.push_str(&f.ty);
            }
        }
        for w in text.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
            if !w.is_empty() {
                hints.insert(w.to_string());
            }
        }
    }
    let mut v = CallVisitor {
        p,
        ix,
        file,
        from: item,
        module,
        owner,
        hint_types: hints,
        loop_depth: 0,
        branch_depth: 0,
        propagated: HashSet::new(),
        unwrapped: HashSet::new(),
        awaited: HashSet::new(),
        edges: vec![],
        stats: Stats::default(),
    };
    // two passes: the first fills the `?`/unwrap/await span sets, the second attributes them to edges
    v.visit_block(block);
    v.edges.clear();
    v.stats = Stats::default();
    v.visit_block(block);
    let cfg = super::cfg::build(block, file, &v.edges, fn_span, source);
    let blocks = super::blocks::build(sig, block, file, &v.edges, fn_span, source);
    Analyzed { calls: v.edges, cfg, blocks, stats: v.stats }
}
