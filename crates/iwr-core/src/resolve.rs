//! Second pass: link everything together once all items are known.
//! - impl targets / traits → item ids
//! - calls inside bodies → callee item ids (name based heuristics; no type checker)
//! - type relationships (fields, signatures, impls, supertraits)
//! - per-function stats and control-flow graphs

use crate::lang::{self, Analyzed};
use crate::model::*;
use crate::parse::{Collected, FnBody};
use crate::progress::{Progress, Stage};
use crate::text::*;
use std::collections::{HashMap, HashSet};

/// Name indexes of the project, for the heuristics below and for the front-ends' call resolvers.
pub(crate) struct Index {
    /// type name → ids (struct / enum / trait / alias)
    pub(crate) types: HashMap<String, Vec<ItemId>>,
    /// free fn name → ids
    pub(crate) fns: HashMap<String, Vec<ItemId>>,
    /// method name → ids
    methods: HashMap<String, Vec<ItemId>>,
    /// full path → id
    pub(crate) paths: HashMap<String, ItemId>,
    /// module → its crate (the top module under a workspace's virtual root; 0 in a single crate)
    crate_of: Vec<ModuleId>,
    /// crate → the other crates its sources name (`torc_proto::call`, `use torc_proto`)
    reach: HashMap<ModuleId, HashSet<ModuleId>>,
}

fn build_index(p: &Project) -> Index {
    let mut ix = Index { types: HashMap::new(), fns: HashMap::new(), methods: HashMap::new(), paths: HashMap::new(), crate_of: vec![], reach: HashMap::new() };
    let workspace = p.modules.first().map(|m| m.file.is_none() && !m.children.is_empty()).unwrap_or(false);
    ix.crate_of = p
        .modules
        .iter()
        .map(|m| {
            // crates are Rust's: the modules of the other languages all see each other
            if m.file.is_some_and(|f| p.files[f].lang != Some(SourceLang::Rust)) {
                return 0;
            }
            let mut cur = m.id;
            while let Some(parent) = p.modules[cur].parent.filter(|pa| *pa != 0 || !workspace) {
                cur = parent;
            }
            cur
        })
        .collect();
    if workspace {
        let crates: Vec<(ModuleId, String)> = p.modules[0].children.iter().map(|c| (*c, p.modules[*c].name.replace('-', "_"))).collect();
        for m in &p.modules {
            let Some(f) = m.file else { continue };
            let src = &p.files[f].content;
            let from = ix.crate_of[m.id];
            for (c, ident) in &crates {
                if *c != from && src.match_indices(ident.as_str()).any(|(i, _)| {
                    let before = src[..i].chars().next_back().map(|ch| !(ch.is_alphanumeric() || ch == '_')).unwrap_or(true);
                    let after = &src[i + ident.len()..];
                    before && (after.starts_with("::") || after.starts_with(';') || after.starts_with(" as ") || after.starts_with(" {"))
                }) {
                    ix.reach.entry(from).or_default().insert(*c);
                }
            }
        }
    }
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

/// `id` lives in the crate of `module`, or in a crate that crate names.
fn reachable(p: &Project, ix: &Index, id: ItemId, module: ModuleId) -> bool {
    let (from, to) = (ix.crate_of[module], ix.crate_of[p.items[id].module]);
    from == to || ix.reach.get(&from).map(|r| r.contains(&to)).unwrap_or(false)
}

/// Pick the candidate closest to `module` (same module, then ancestor, then anything in a crate
/// this one can reach: another crate of a workspace only when its sources name it).
pub(crate) fn prefer_module(p: &Project, ix: &Index, cands: &[ItemId], module: ModuleId) -> Option<ItemId> {
    let cands: Vec<ItemId> = cands.iter().copied().filter(|c| reachable(p, ix, *c, module)).collect();
    let cands = &cands[..];
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

pub(crate) fn resolve_type(p: &Project, ix: &Index, name: &str, module: ModuleId) -> Option<ItemId> {
    ix.types.get(name).and_then(|c| prefer_module(p, ix, c, module))
}

/// Resolve `Type::method` / `Trait::method`.
pub(crate) fn resolve_assoc(p: &Project, ix: &Index, ty: &str, method: &str, module: ModuleId) -> Option<ItemId> {
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
    if let Some(c) = prefer_module(p, ix, &inherent, module) {
        return Some(c);
    }
    let in_impl: Vec<ItemId> = filtered.iter().copied().filter(|id| p.items[*id].fn_info().unwrap().impl_id.is_some()).collect();
    if let Some(c) = prefer_module(p, ix, &in_impl, module) {
        return Some(c);
    }
    prefer_module(p, ix, &filtered, module)
}

/// Resolve `x.method()` without type info.
pub(crate) fn resolve_method(p: &Project, ix: &Index, method: &str, receiver_is_self: bool, owner: Option<&str>, module: ModuleId, hint_types: &HashSet<String>) -> Option<ItemId> {
    let cands: Vec<ItemId> = ix.methods.get(method)?.iter().copied().filter(|id| reachable(p, ix, *id, module)).collect();
    let cands = &cands[..];
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
    if let Some(c) = prefer_module(p, ix, &hinted, module) {
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
    prefer_module(p, ix, pool, module)
}

/// Counts shown in a function's summary.
#[derive(Default)]
pub(crate) struct Stats {
    pub question_marks: usize,
    pub unwraps: usize,
    pub expects: usize,
    pub panics: usize,
    pub awaits: usize,
    pub loops: usize,
    pub branches: usize,
}

pub fn finish(c: Collected, report: &mut dyn FnMut(&Progress)) -> Project {
    let Collected { mut project, bodies, use_paths, rels } = c;
    let counts = |p: &Project, calls: usize| Progress { files: p.files.len(), items: p.items.len(), calls, ..Default::default() };
    report(&Progress { stage: Stage::Linking, ..counts(&project, 0) });

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
    // relations the front-ends read by name
    for (from, name, kind) in rels {
        let last = name.rsplit("::").next().unwrap_or(&name);
        let last = last.split('<').next().unwrap_or(last);
        if let Some(to) = resolve_type(&project, &ix, last, project.items[from].module).filter(|t| *t != from) {
            project.type_rels.push(TypeRel { from, to, kind, label: None });
        }
    }
    lang::link(&mut project);
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
                for nme in type_names(&f.ty) {
                    if let Some(t) = resolve_type(&project, &ix, &nme, module) {
                        if t != id && !f.refs.contains(&t) {
                            f.refs.push(t);
                            rels.push((t, format!("{}{}", prefix, f.name)));
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
                let mut seen = HashSet::new();
                for nme in type_names(target) {
                    if let Some(t) = resolve_type(&project, &ix, &nme, module) {
                        if t != id && seen.insert(t) {
                            project.type_rels.push(TypeRel { from: id, to: t, kind: TypeRelKind::Aliases, label: None });
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
            names.extend(type_names(&p.ty));
        }
        if let Some(r) = &f.ret {
            names.extend(type_names(r));
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
    for (i, FnBody { item, file, body }) in bodies.iter().enumerate() {
        // every 8th function: the loading page follows the names going by
        if i % 8 == 0 {
            report(&Progress { stage: Stage::Calls, done: i, total: bodies.len(), current: project.items[*item].name.clone(), ..counts(&project, all_calls.len()) });
        }
        let fn_span = project.items[*item].span;
        let body_lines = fn_span.line_end + 1 - fn_span.line_start;
        let Analyzed { calls, cfg, blocks, stats } = lang::analyze_body(&project, &ix, *item, *file, body);
        cfgs.push((*item, cfg, blocks, stats, body_lines));
        all_calls.extend(calls);
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
    report(&Progress { stage: Stage::Done, done: bodies.len(), total: bodies.len(), ..counts(&project, project.calls.len()) });
    project
}
