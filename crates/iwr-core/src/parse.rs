//! First pass: parse every file with `syn` and collect items + module tree.
//! Bodies are kept around so the second pass (`resolve`) can build CFGs and
//! resolve calls once every item is known.

use crate::model::*;
use crate::text::*;
use std::collections::HashMap;

/// Output of the first pass.
pub struct Collected {
    pub project: Project,
    /// (function item id, body, file) for every function that has a body.
    pub bodies: Vec<FnBody>,
    /// Raw `use` paths per module, for external crate detection.
    pub use_paths: Vec<(ModuleId, Vec<String>)>,
}

pub struct FnBody {
    pub item: ItemId,
    pub file: FileId,
    pub block: syn::Block,
    pub sig: syn::Signature,
}

struct Ctx<'a> {
    project: &'a mut Project,
    bodies: Vec<FnBody>,
    use_paths: Vec<(ModuleId, Vec<String>)>,
    module_index: HashMap<String, ModuleId>,
}

impl<'a> Ctx<'a> {
    fn module_for(&mut self, path: &str) -> ModuleId {
        if let Some(id) = self.module_index.get(path) {
            return *id;
        }
        let (parent, name) = match path.rsplit_once("::") {
            Some((p, n)) => (Some(self.module_for(p)), n.to_string()),
            // crate roots of a multi-crate workspace hang under the virtual `crate` root (module 0)
            None if path != "crate" => (Some(self.module_for("crate")), path.to_string()),
            None => (None, path.to_string()),
        };
        let id = self.project.modules.len();
        self.project.modules.push(Module {
            id,
            name,
            path: path.to_string(),
            file: None,
            parent,
            children: vec![],
            items: vec![],
            uses: vec![],
            span: None,
            doc: None,
        });
        if let Some(p) = parent {
            self.project.modules[p].children.push(id);
        }
        self.module_index.insert(path.to_string(), id);
        id
    }

    fn push_item(&mut self, mut item: Item) -> ItemId {
        let id = self.project.items.len();
        item.id = id;
        self.project.modules[item.module].items.push(id);
        self.project.items.push(item);
        id
    }
}

/// Module path for a file, derived from its location. See README for the rules.
pub fn module_path_for_file(rel_path: &str) -> String {
    let parts: Vec<&str> = rel_path.split('/').filter(|p| !p.is_empty()).collect();
    let src_idx = parts.iter().rposition(|p| *p == "src");
    let (prefix, rest): (Vec<&str>, Vec<&str>) = match src_idx {
        Some(i) => (parts[..i].to_vec(), parts[i + 1..].to_vec()),
        None => (vec![], parts.clone()),
    };
    let mut segs: Vec<String> = Vec::new();
    // crate root name
    if src_idx.is_some() {
        segs.push("crate".to_string());
        if let Some(last) = prefix.last() {
            // multi-crate layout: crates/foo/src -> crate::foo? No: name it after the crate dir.
            segs = vec![last.to_string()];
        }
    }
    if src_idx.is_none() {
        // files outside any `src/` (tests/, benches/, examples/*.rs, build.rs): <parent dir>::<stem>
        let n = rest.len();
        if n >= 2 {
            segs.push(rest[n - 2].to_string());
        }
        if let Some(last) = rest.last() {
            segs.push(last.trim_end_matches(".rs").to_string());
        }
        return if segs.is_empty() { "crate".to_string() } else { segs.join("::") };
    }
    for (i, p) in rest.iter().enumerate() {
        let is_last = i + 1 == rest.len();
        let stem = if is_last { p.trim_end_matches(".rs") } else { p };
        if is_last && (stem == "mod" || stem == "lib" || stem == "main") {
            continue;
        }
        segs.push(stem.to_string());
    }
    if segs.is_empty() {
        "crate".to_string()
    } else {
        segs.join("::")
    }
}

fn vis_of(v: &syn::Visibility) -> Visibility {
    match v {
        syn::Visibility::Public(_) => Visibility::Public,
        syn::Visibility::Restricted(r) => {
            if r.path.is_ident("crate") {
                Visibility::Crate
            } else {
                Visibility::Restricted
            }
        }
        syn::Visibility::Inherited => Visibility::Private,
    }
}

/// Parse all files and collect items.
pub fn collect(name: &str, root: &str, files: Vec<(String, String)>) -> Collected {
    let mut project = Project { name: name.to_string(), root: root.to_string(), ..Default::default() };
    let mut ctx = Ctx { project: &mut project, bodies: vec![], use_paths: vec![], module_index: HashMap::new() };
    ctx.module_for("crate");

    for (path, content) in files {
        let file_id = ctx.project.files.len();
        ctx.project.files.push(SourceFile { id: file_id, path: path.clone(), content: content.clone() });
        let ast = match syn::parse_file(&content) {
            Ok(a) => a,
            Err(e) => {
                let s = e.span().start();
                ctx.project.diagnostics.push(format!("{}:{}:{}: {}", path, s.line, s.column + 1, e));
                continue;
            }
        };
        let mpath = module_path_for_file(&path);
        let mid = ctx.module_for(&mpath);
        if ctx.project.modules[mid].file.is_none() {
            ctx.project.modules[mid].file = Some(file_id);
        }
        if ctx.project.modules[mid].doc.is_none() {
            ctx.project.modules[mid].doc = doc_of(&ast.attrs);
        }
        collect_items(&mut ctx, &ast.items, mid, &mpath, file_id);
    }

    // sort children/items deterministically by name for stable output
    for m in &mut ctx.project.modules {
        m.children.sort();
    }
    let Ctx { bodies, use_paths, .. } = ctx;
    Collected { project, bodies, use_paths }
}

fn collect_items(ctx: &mut Ctx, items: &[syn::Item], mid: ModuleId, mpath: &str, file: FileId) {
    for it in items {
        match it {
            syn::Item::Fn(f) => {
                let id = push_fn(ctx, &f.sig, &f.vis, &f.attrs, mid, mpath, file, span_of(f, file), None, None, None);
                ctx.bodies.push(FnBody { item: id, file, block: (*f.block).clone(), sig: f.sig.clone() });
            }
            syn::Item::Struct(s) => {
                let is_tuple = matches!(s.fields, syn::Fields::Unnamed(_));
                let is_unit = matches!(s.fields, syn::Fields::Unit);
                let fields = fields_of(&s.fields);
                let sig = format!("{}struct {}{}", vis_prefix(&s.vis), s.ident, tokens_to_string(&s.generics));
                ctx.push_item(Item {
                    id: 0,
                    kind: ItemKind::Struct,
                    name: s.ident.to_string(),
                    path: format!("{}::{}", mpath, s.ident),
                    module: mid,
                    vis: vis_of(&s.vis),
                    span: span_of(s, file),
                    signature: sig,
                    doc: doc_of(&s.attrs),
                    attrs: attrs_of(&s.attrs),
                    extra: ItemExtra::Struct(StructInfo { fields, derives: derives_of(&s.attrs), is_tuple, is_unit }),
                });
            }
            syn::Item::Enum(e) => {
                let variants = e
                    .variants
                    .iter()
                    .map(|v| Variant { name: v.ident.to_string(), fields: fields_of(&v.fields) })
                    .collect();
                let sig = format!("{}enum {}{}", vis_prefix(&e.vis), e.ident, tokens_to_string(&e.generics));
                ctx.push_item(Item {
                    id: 0,
                    kind: ItemKind::Enum,
                    name: e.ident.to_string(),
                    path: format!("{}::{}", mpath, e.ident),
                    module: mid,
                    vis: vis_of(&e.vis),
                    span: span_of(e, file),
                    signature: sig,
                    doc: doc_of(&e.attrs),
                    attrs: attrs_of(&e.attrs),
                    extra: ItemExtra::Enum(EnumInfo { variants, derives: derives_of(&e.attrs) }),
                });
            }
            syn::Item::Trait(t) => {
                let supertraits: Vec<String> = t
                    .supertraits
                    .iter()
                    .filter_map(|b| match b {
                        syn::TypeParamBound::Trait(tb) => Some(path_last(&tb.path)),
                        _ => None,
                    })
                    .collect();
                let sig = format!("{}trait {}{}", vis_prefix(&t.vis), t.ident, tokens_to_string(&t.generics));
                let tid = ctx.push_item(Item {
                    id: 0,
                    kind: ItemKind::Trait,
                    name: t.ident.to_string(),
                    path: format!("{}::{}", mpath, t.ident),
                    module: mid,
                    vis: vis_of(&t.vis),
                    span: span_of(t, file),
                    signature: sig,
                    doc: doc_of(&t.attrs),
                    attrs: attrs_of(&t.attrs),
                    extra: ItemExtra::Trait(TraitInfo { supertraits, supertrait_ids: vec![], methods: vec![] }),
                });
                let tpath = format!("{}::{}", mpath, t.ident);
                let mut methods = Vec::new();
                for ti in &t.items {
                    if let syn::TraitItem::Fn(m) = ti {
                        let id = push_fn(
                            ctx,
                            &m.sig,
                            &syn::Visibility::Inherited,
                            &m.attrs,
                            mid,
                            &tpath,
                            file,
                            span_of(m, file),
                            None,
                            Some((t.ident.to_string(), tid)),
                            None,
                        );
                        methods.push(id);
                        if let Some(b) = &m.default {
                            ctx.bodies.push(FnBody { item: id, file, block: b.clone(), sig: m.sig.clone() });
                        }
                    }
                }
                if let ItemExtra::Trait(ti) = &mut ctx.project.items[tid].extra {
                    ti.methods = methods;
                }
            }
            syn::Item::Impl(im) => {
                let target = tokens_to_string(&im.self_ty);
                let target_short = match &*im.self_ty {
                    syn::Type::Path(p) => path_last(&p.path),
                    _ => target.clone(),
                };
                let trait_name = im.trait_.as_ref().map(|(_, p, _)| tokens_to_string(p));
                let name = match &trait_name {
                    Some(t) => format!("impl {} for {}", t, target),
                    None => format!("impl {}", target),
                };
                let sig = format!("impl{} {}", tokens_to_string(&im.generics), &name[5..]);
                let iid = ctx.push_item(Item {
                    id: 0,
                    kind: ItemKind::Impl,
                    name: name.clone(),
                    path: format!("{}::<{}>", mpath, name),
                    module: mid,
                    vis: Visibility::Private,
                    span: span_of(im, file),
                    signature: sig,
                    doc: doc_of(&im.attrs),
                    attrs: attrs_of(&im.attrs),
                    extra: ItemExtra::Impl(ImplInfo {
                        target: target_short.clone(),
                        target_id: None,
                        trait_name: trait_name.clone(),
                        trait_id: None,
                        methods: vec![],
                    }),
                });
                let tpath = format!("{}::{}", mpath, target_short);
                let mut methods = Vec::new();
                for ii in &im.items {
                    if let syn::ImplItem::Fn(m) = ii {
                        let id = push_fn(
                            ctx,
                            &m.sig,
                            &m.vis,
                            &m.attrs,
                            mid,
                            &tpath,
                            file,
                            span_of(m, file),
                            Some((target_short.clone(), iid)),
                            trait_name.clone().map(|t| (t, usize::MAX)),
                            None,
                        );
                        methods.push(id);
                        ctx.bodies.push(FnBody { item: id, file, block: m.block.clone(), sig: m.sig.clone() });
                    }
                }
                if let ItemExtra::Impl(ii) = &mut ctx.project.items[iid].extra {
                    ii.methods = methods;
                }
            }
            syn::Item::Type(t) => {
                let target = tokens_to_string(&t.ty);
                ctx.push_item(Item {
                    id: 0,
                    kind: ItemKind::TypeAlias,
                    name: t.ident.to_string(),
                    path: format!("{}::{}", mpath, t.ident),
                    module: mid,
                    vis: vis_of(&t.vis),
                    span: span_of(t, file),
                    signature: format!("{}type {}{} = {}", vis_prefix(&t.vis), t.ident, tokens_to_string(&t.generics), target),
                    doc: doc_of(&t.attrs),
                    attrs: attrs_of(&t.attrs),
                    extra: ItemExtra::TypeAlias { target },
                });
            }
            syn::Item::Const(c) => {
                let ty = tokens_to_string(&c.ty);
                ctx.push_item(Item {
                    id: 0,
                    kind: ItemKind::Const,
                    name: c.ident.to_string(),
                    path: format!("{}::{}", mpath, c.ident),
                    module: mid,
                    vis: vis_of(&c.vis),
                    span: span_of(c, file),
                    signature: format!("{}const {}: {}", vis_prefix(&c.vis), c.ident, ty),
                    doc: doc_of(&c.attrs),
                    attrs: attrs_of(&c.attrs),
                    extra: ItemExtra::Const { ty },
                });
            }
            syn::Item::Static(s) => {
                let ty = tokens_to_string(&s.ty);
                ctx.push_item(Item {
                    id: 0,
                    kind: ItemKind::Static,
                    name: s.ident.to_string(),
                    path: format!("{}::{}", mpath, s.ident),
                    module: mid,
                    vis: vis_of(&s.vis),
                    span: span_of(s, file),
                    signature: format!("{}static {}: {}", vis_prefix(&s.vis), s.ident, ty),
                    doc: doc_of(&s.attrs),
                    attrs: attrs_of(&s.attrs),
                    extra: ItemExtra::Const { ty },
                });
            }
            syn::Item::Mod(m) => {
                let child_path = format!("{}::{}", mpath, m.ident);
                let cid = ctx.module_for(&child_path);
                if let Some((_, items)) = &m.content {
                    ctx.project.modules[cid].span = Some(span_of(m, file));
                    ctx.project.modules[cid].doc = doc_of(&m.attrs);
                    ctx.project.modules[cid].file = Some(file);
                    collect_items(ctx, items, cid, &child_path, file);
                } else if ctx.project.modules[cid].doc.is_none() {
                    ctx.project.modules[cid].doc = doc_of(&m.attrs);
                }
            }
            syn::Item::Use(u) => {
                let mut paths = Vec::new();
                flatten_use(&u.tree, String::new(), &mut paths);
                ctx.project.modules[mid].uses.extend(paths.iter().cloned());
                ctx.use_paths.push((mid, paths));
            }
            _ => {}
        }
    }
}

fn flatten_use(tree: &syn::UseTree, prefix: String, out: &mut Vec<String>) {
    match tree {
        syn::UseTree::Path(p) => {
            let np = if prefix.is_empty() { p.ident.to_string() } else { format!("{}::{}", prefix, p.ident) };
            flatten_use(&p.tree, np, out);
        }
        syn::UseTree::Name(n) => {
            out.push(if prefix.is_empty() { n.ident.to_string() } else { format!("{}::{}", prefix, n.ident) });
        }
        syn::UseTree::Rename(r) => {
            out.push(if prefix.is_empty() { r.ident.to_string() } else { format!("{}::{}", prefix, r.ident) });
        }
        syn::UseTree::Glob(_) => out.push(format!("{}::*", prefix)),
        syn::UseTree::Group(g) => {
            for t in &g.items {
                flatten_use(t, prefix.clone(), out);
            }
        }
    }
}

fn vis_prefix(v: &syn::Visibility) -> String {
    match v {
        syn::Visibility::Inherited => String::new(),
        other => format!("{} ", tokens_to_string(other)),
    }
}

fn fields_of(f: &syn::Fields) -> Vec<Field> {
    match f {
        syn::Fields::Named(n) => n
            .named
            .iter()
            .map(|f| Field {
                name: f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default(),
                ty: tokens_to_string(&f.ty),
                vis: vis_of(&f.vis),
                refs: vec![],
            })
            .collect(),
        syn::Fields::Unnamed(u) => u
            .unnamed
            .iter()
            .enumerate()
            .map(|(i, f)| Field { name: i.to_string(), ty: tokens_to_string(&f.ty), vis: vis_of(&f.vis), refs: vec![] })
            .collect(),
        syn::Fields::Unit => vec![],
    }
}

#[allow(clippy::too_many_arguments)]
fn push_fn(
    ctx: &mut Ctx,
    sig: &syn::Signature,
    vis: &syn::Visibility,
    attrs: &[syn::Attribute],
    mid: ModuleId,
    parent_path: &str,
    _file: FileId,
    span: Span,
    owner: Option<(String, ItemId)>,
    trait_: Option<(String, ItemId)>,
    _unused: Option<()>,
) -> ItemId {
    let mut info = FnInfo {
        is_async: sig.asyncness.is_some(),
        is_unsafe: sig.unsafety.is_some(),
        is_const: sig.constness.is_some(),
        ..Default::default()
    };
    for input in &sig.inputs {
        match input {
            syn::FnArg::Receiver(r) => {
                info.self_kind = if r.reference.is_some() {
                    if r.mutability.is_some() {
                        SelfKind::RefMut
                    } else {
                        SelfKind::Ref
                    }
                } else {
                    SelfKind::Owned
                };
            }
            syn::FnArg::Typed(t) => {
                info.params.push(Param { name: tokens_to_string(&t.pat), ty: tokens_to_string(&t.ty) });
            }
        }
    }
    if let syn::ReturnType::Type(_, ty) = &sig.output {
        info.ret = Some(tokens_to_string(ty));
        if let syn::Type::Path(p) = &**ty {
            if let Some(seg) = p.path.segments.last() {
                let n = seg.ident.to_string();
                let args: Vec<String> = match &seg.arguments {
                    syn::PathArguments::AngleBracketed(a) => a
                        .args
                        .iter()
                        .filter_map(|ga| match ga {
                            syn::GenericArgument::Type(t) => Some(tokens_to_string(t)),
                            _ => None,
                        })
                        .collect(),
                    _ => vec![],
                };
                if n == "Result" {
                    info.returns_result = true;
                    info.result_ok = args.first().cloned();
                    info.result_err = args.get(1).cloned();
                } else if n == "Option" {
                    info.returns_option = true;
                } else if n.ends_with("Result") {
                    // type alias like io::Result<T> / anyhow::Result<T>
                    info.returns_result = true;
                    info.result_ok = args.first().cloned();
                    info.result_err = args.get(1).cloned();
                }
            }
        }
    }
    let kind = if owner.is_some() || trait_.is_some() { ItemKind::Method } else { ItemKind::Function };
    if let Some((tname, iid)) = &owner {
        info.impl_id = Some(*iid);
        info.owner_type_name = Some(tname.clone());
    }
    if let Some((tname, tid)) = &trait_ {
        info.trait_name = Some(tname.clone());
        if *tid != usize::MAX {
            info.trait_id = Some(*tid);
        }
    }
    info.is_test = attrs.iter().any(|a| a.path().is_ident("test") || tokens_to_string(&a.meta).contains("::test"));
    let module = &ctx.project.modules[mid];
    let is_root = module.path == "crate" || module.file.map(|f| ctx.project.files[f].path.ends_with("main.rs")).unwrap_or(false);
    info.is_main = kind == ItemKind::Function && sig.ident == "main" && is_root;
    let name = sig.ident.to_string();
    let signature = format!("{}{}", vis_prefix(vis), tokens_to_string(sig));
    let item = Item {
        id: 0,
        kind,
        name: name.clone(),
        path: format!("{}::{}", parent_path, name),
        module: mid,
        vis: vis_of(vis),
        span,
        signature,
        doc: doc_of(attrs),
        attrs: attrs_of(attrs),
        extra: ItemExtra::Fn(info),
    };
    let id = ctx.push_item(item);
    if ctx.project.items[id].fn_info().map(|f| f.is_main).unwrap_or(false) && ctx.project.main.is_none() {
        ctx.project.main = Some(id);
    }
    id
}
