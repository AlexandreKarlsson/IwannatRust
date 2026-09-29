//! Second pass: link everything together once all items are known.
//! - impl targets / traits → item ids
//! - calls inside bodies → callee item ids (name based heuristics; no type checker)
//! - type relationships (fields, signatures, impls, supertraits)
//! - per-function stats and control-flow graphs

use crate::cfg;
use crate::model::*;
use crate::parse::{Collected, FnBody};
use crate::text::*;
use std::collections::{HashMap, HashSet};
use syn::visit::Visit;

struct Index {
    /// type name → ids (struct / enum / trait / alias)
    types: HashMap<String, Vec<ItemId>>,
    /// free fn name → ids
    fns: HashMap<String, Vec<ItemId>>,
    /// method name → ids
    methods: HashMap<String, Vec<ItemId>>,
    /// full path → id
    paths: HashMap<String, ItemId>,
}

fn build_index(p: &Project) -> Index {
    let mut ix = Index { types: HashMap::new(), fns: HashMap::new(), methods: HashMap::new(), paths: HashMap::new() };
    for it in &p.items {
        ix.paths.insert(it.path.clone(), it.id);
        match it.kind {
            ItemKind::Struct | ItemKind::Enum | ItemKind::Trait | ItemKind::TypeAlias => {
                ix.types.entry(it.name.clone()).or_default().push(it.id)
            }
            ItemKind::Function => ix.fns.entry(it.name.clone()).or_default().push(it.id),
            ItemKind::Method => ix.methods.entry(it.name.clone()).or_default().push(it.id),
            _ => {}
        }
    }
    ix
}

/// Pick the candidate closest to `module` (same module, then ancestor, then anything).
fn prefer_module(p: &Project, cands: &[ItemId], module: ModuleId) -> Option<ItemId> {
    if cands.is_empty() {
        return None;
    }
    if let Some(c) = cands.iter().find(|c| p.items[**c].module == module) {
        return Some(*c);
    }
    // prefer non-test, non-trait-declaration candidates
    let mpath = &p.modules[module].path;
    let mut best: Option<(usize, ItemId)> = None;
    for c in cands {
        let cpath = &p.modules[p.items[*c].module].path;
        let common = mpath.split("::").zip(cpath.split("::")).take_while(|(a, b)| a == b).count();
        if best.map(|(s, _)| common > s).unwrap_or(true) {
            best = Some((common, *c));
        }
    }
    best.map(|(_, c)| c)
}

fn resolve_type(p: &Project, ix: &Index, name: &str, module: ModuleId) -> Option<ItemId> {
    ix.types.get(name).and_then(|c| prefer_module(p, c, module))
}

/// Resolve `Type::method` / `Trait::method`.
fn resolve_assoc(p: &Project, ix: &Index, ty: &str, method: &str, module: ModuleId) -> Option<ItemId> {
    let cands = ix.methods.get(method)?;
    let ty = if ty == "Self" { return None } else { ty };
    let filtered: Vec<ItemId> = cands
        .iter()
        .copied()
        .filter(|id| {
            let f = p.items[*id].fn_info().unwrap();
            f.owner_type_name.as_deref() == Some(ty) || (f.impl_id.is_none() && f.trait_name.as_deref() == Some(ty))
        })
        .collect();
    // prefer inherent impls over trait impls over trait declarations
    let inherent: Vec<ItemId> = filtered.iter().copied().filter(|id| {
        let f = p.items[*id].fn_info().unwrap();
        f.impl_id.is_some() && f.trait_name.is_none()
    }).collect();
    if let Some(c) = prefer_module(p, &inherent, module) {
        return Some(c);
    }
    let in_impl: Vec<ItemId> = filtered.iter().copied().filter(|id| p.items[*id].fn_info().unwrap().impl_id.is_some()).collect();
    if let Some(c) = prefer_module(p, &in_impl, module) {
        return Some(c);
    }
    prefer_module(p, &filtered, module)
}

/// Resolve `x.method()` without type info.
fn resolve_method(p: &Project, ix: &Index, method: &str, receiver_is_self: bool, owner: Option<&str>, module: ModuleId, hint_types: &HashSet<String>) -> Option<ItemId> {
    let cands = ix.methods.get(method)?;
    let with_body: Vec<ItemId> = cands.iter().copied().filter(|id| p.items[*id].fn_info().unwrap().impl_id.is_some()).collect();
    let pool: &[ItemId] = if with_body.is_empty() { cands } else { &with_body };
    if receiver_is_self {
        if let Some(o) = owner {
            if let Some(c) = pool.iter().find(|id| p.items[**id].fn_info().unwrap().owner_type_name.as_deref() == Some(o)) {
                return Some(*c);
            }
        }
    }
    // receiver is a generic `T: Trait` / `dyn Trait` / `impl Trait`: point at the trait declaration
    for id in cands {
        let f = p.items[*id].fn_info().unwrap();
        if f.impl_id.is_none() {
            if let Some(tn) = &f.trait_name {
                let owners_hinted = pool.iter().any(|o| p.items[*o].fn_info().unwrap().owner_type_name.as_ref().map(|t| hint_types.contains(t)).unwrap_or(false));
                if hint_types.contains(tn) && !owners_hinted {
                    return Some(*id);
                }
            }
        }
    }
    if pool.len() == 1 {
        return Some(pool[0]);
    }
    // prefer methods whose owner type is mentioned in this function
    let hinted: Vec<ItemId> = pool
        .iter()
        .copied()
        .filter(|id| {
            let f = p.items[*id].fn_info().unwrap();
            f.owner_type_name.as_ref().map(|t| hint_types.contains(t)).unwrap_or(false)
        })
        .collect();
    if let Some(c) = prefer_module(p, &hinted, module) {
        return Some(c);
    }
    // if all candidates are impls of the same trait method, pick the trait declaration when it exists
    let trait_names: HashSet<&str> = pool.iter().filter_map(|id| p.items[*id].fn_info().unwrap().trait_name.as_deref()).collect();
    if trait_names.len() == 1 && pool.iter().all(|id| p.items[*id].fn_info().unwrap().trait_name.is_some()) {
        let tn = trait_names.into_iter().next().unwrap();
        if let Some(decl) = cands.iter().find(|id| {
            let f = p.items[**id].fn_info().unwrap();
            f.impl_id.is_none() && f.trait_name.as_deref() == Some(tn)
        }) {
            return Some(*decl);
        }
    }
    prefer_module(p, pool, module)
}

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

#[derive(Default)]
struct Stats {
    question_marks: usize,
    unwraps: usize,
    expects: usize,
    panics: usize,
    awaits: usize,
    loops: usize,
    branches: usize,
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
                    if let Some(id) = self.ix.fns.get(n).and_then(|c| prefer_module(self.p, c, self.module)) {
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
                        c.iter().copied().find(|id| self.p.items[*id].path.ends_with(&suffix))
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

pub fn finish(c: Collected) -> Project {
    let Collected { mut project, bodies, use_paths } = c;

    // ---- link impls to types / traits, methods to owner types ----
    let ix = build_index(&project);
    let n = project.items.len();
    for id in 0..n {
        let module = project.items[id].module;
        let (target, trait_name) = match &project.items[id].extra {
            ItemExtra::Impl(i) => (i.target.clone(), i.trait_name.clone()),
            _ => continue,
        };
        let target_id = resolve_type(&project, &ix, &target, module);
        let trait_id = trait_name
            .as_ref()
            .and_then(|t| {
                let last = t.rsplit("::").next().unwrap_or(t);
                let last = last.split('<').next().unwrap_or(last);
                resolve_type(&project, &ix, last, module)
            })
            .filter(|t| project.items[*t].kind == ItemKind::Trait);
        let methods = match &project.items[id].extra {
            ItemExtra::Impl(i) => i.methods.clone(),
            _ => vec![],
        };
        if let ItemExtra::Impl(i) = &mut project.items[id].extra {
            i.target_id = target_id;
            i.trait_id = trait_id;
        }
        for m in methods {
            if let Some(f) = project.items[m].fn_info_mut() {
                f.owner_type = target_id;
                f.trait_id = trait_id;
            }
        }
        if let (Some(t), Some(tr)) = (target_id, trait_id) {
            project.type_rels.push(TypeRel { from: t, to: tr, kind: TypeRelKind::Implements, label: None });
        }
    }
    // supertraits
    for id in 0..n {
        let module = project.items[id].module;
        let supers = match &project.items[id].extra {
            ItemExtra::Trait(t) => t.supertraits.clone(),
            _ => continue,
        };
        let ids: Vec<ItemId> = supers.iter().filter_map(|s| resolve_type(&project, &ix, s, module)).collect();
        for s in &ids {
            project.type_rels.push(TypeRel { from: id, to: *s, kind: TypeRelKind::Supertrait, label: None });
        }
        if let ItemExtra::Trait(t) = &mut project.items[id].extra {
            t.supertrait_ids = ids;
        }
    }
    // field references
    for id in 0..n {
        let module = project.items[id].module;
        let mut rels = Vec::new();
        let mut resolve_fields = |fields: &mut Vec<Field>, prefix: &str| {
            for f in fields.iter_mut() {
                if let Ok(ty) = syn::parse_str::<syn::Type>(&f.ty) {
                    let mut names = Vec::new();
                    type_idents(&ty, &mut names);
                    for nme in names {
                        if let Some(t) = resolve_type(&project, &ix, &nme, module) {
                            if t != id && !f.refs.contains(&t) {
                                f.refs.push(t);
                                rels.push((t, format!("{}{}", prefix, f.name)));
                            }
                        }
                    }
                }
            }
        };
        let mut extra = project.items[id].extra.clone();
        match &mut extra {
            ItemExtra::Struct(s) => resolve_fields(&mut s.fields, ""),
            ItemExtra::Enum(e) => {
                for v in &mut e.variants {
                    resolve_fields(&mut v.fields, &format!("{}.", v.name));
                }
            }
            ItemExtra::TypeAlias { target } => {
                if let Ok(ty) = syn::parse_str::<syn::Type>(target) {
                    let mut names = Vec::new();
                    type_idents(&ty, &mut names);
                    for nme in names {
                        if let Some(t) = resolve_type(&project, &ix, &nme, module) {
                            if t != id {
                                project.type_rels.push(TypeRel { from: id, to: t, kind: TypeRelKind::Aliases, label: None });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        project.items[id].extra = extra;
        for (t, label) in rels {
            project.type_rels.push(TypeRel { from: id, to: t, kind: TypeRelKind::Contains, label: Some(label) });
        }
    }
    // signature references
    for id in 0..n {
        let module = project.items[id].module;
        let Some(f) = project.items[id].fn_info() else { continue };
        let mut names = Vec::new();
        for p in &f.params {
            if let Ok(ty) = syn::parse_str::<syn::Type>(&p.ty) {
                type_idents(&ty, &mut names);
            }
        }
        if let Some(r) = &f.ret {
            if let Ok(ty) = syn::parse_str::<syn::Type>(r) {
                type_idents(&ty, &mut names);
            }
        }
        let mut seen = HashSet::new();
        for nme in names {
            if let Some(t) = resolve_type(&project, &ix, &nme, module) {
                if seen.insert(t) {
                    project.type_rels.push(TypeRel { from: id, to: t, kind: TypeRelKind::UsesInSignature, label: None });
                }
            }
        }
    }

    // ---- calls + CFG ----
    let mut all_calls: Vec<CallEdge> = Vec::new();
    let mut cfgs: Vec<(ItemId, Cfg, Block, Stats, usize)> = Vec::new();
    for FnBody { item, file, block, sig } in &bodies {
        let module = project.items[*item].module;
        let owner = project.items[*item].fn_info().and_then(|f| f.owner_type_name.clone());
        let mut hints = hint_types(block, sig);
        // owner type's generics / bounds / field types are visible to methods (`S: Storage` etc)
        if let Some(owner_id) = project.items[*item].fn_info().and_then(|f| f.owner_type) {
            let owner_item = &project.items[owner_id];
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
            p: &project,
            ix: &ix,
            file: *file,
            from: *item,
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
        let fn_span = project.items[*item].span;
        let cfg = cfg::build(block, *file, &v.edges, fn_span);
        let blocks = crate::blocks::build(sig, block, *file, &v.edges, fn_span);
        let body_lines = fn_span.line_end + 1 - fn_span.line_start;
        cfgs.push((*item, cfg, blocks, v.stats, body_lines));
        all_calls.extend(v.edges);
    }
    for (item, cfg, blocks, stats, body_lines) in cfgs {
        let recursive = all_calls.iter().any(|c| c.from == item && c.to == Some(item));
        if let Some(f) = project.items[item].fn_info_mut() {
            f.cfg = Some(cfg);
            f.blocks = Some(blocks);
            f.question_marks = stats.question_marks;
            f.unwraps = stats.unwraps;
            f.expects = stats.expects;
            f.panics = stats.panics;
            f.awaits = stats.awaits;
            f.loops = stats.loops;
            f.branches = stats.branches;
            f.is_recursive = recursive;
            f.body_lines = body_lines;
        }
    }
    project.calls = all_calls;

    // ---- external crates ----
    let local_roots: HashSet<String> = project.modules.iter().map(|m| m.name.clone()).collect();
    let mut ext: Vec<String> = Vec::new();
    for (_, paths) in &use_paths {
        for p in paths {
            let first = p.split("::").next().unwrap_or("");
            if matches!(first, "crate" | "self" | "super" | "") || local_roots.contains(first) {
                continue;
            }
            if !ext.contains(&first.to_string()) {
                ext.push(first.to_string());
            }
        }
    }
    ext.sort();
    project.external_crates = ext;
    project
}
