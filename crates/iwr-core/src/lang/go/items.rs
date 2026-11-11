//! Items of a Go file: functions, methods (grouped under their receiver's type), `struct`s,
//! `interface`s (traits with their method declarations), other named types (aliases),
//! `const` and `var` declarations; `import`s as the module's uses.

use super::{line, local_import, type_name};
use crate::lang::ts::*;
use crate::lang::{Body, TreeBody};
use crate::model::*;
use crate::parse::{Ctx, FnBody};
use tree_sitter::{Node, Tree};

struct File<'a> {
    file: FileId,
    src: &'a str,
    lines: Vec<&'a str>,
    mid: ModuleId,
    mpath: &'a str,
    /// A `_test.go` file.
    test: bool,
    /// `package main`.
    main: bool,
}

/// Collect the items of one parsed file into `ctx`. `mods` are the project's `go.mod` modules.
pub(crate) fn collect_file(ctx: &mut Ctx, tree: &Tree, file: FileId, src: &str, mid: ModuleId, mpath: &str, mods: &[(String, String)]) {
    let root = tree.root_node();
    let main = named(root).into_iter().find(|c| c.kind() == "package_clause").is_some_and(|c| named(c).into_iter().any(|x| raw(x, src) == "main"));
    let test = ctx.project.files[file].path.ends_with("_test.go");
    let f = File { file, src, lines: src.lines().collect(), mid, mpath, test, main };
    for (_, path) in super::imports(root, src) {
        let used = local_import(mods, &path).unwrap_or(path);
        if used != mpath && !ctx.project.modules[mid].uses.contains(&used) {
            ctx.project.modules[mid].uses.push(used.clone());
            ctx.use_paths.push((mid, vec![used]));
        }
    }
    for c in named(root) {
        match c.kind() {
            "function_declaration" => function(ctx, &f, tree, c, None),
            "method_declaration" => {
                let recv = field(c, "receiver").and_then(|r| named(r).into_iter().next());
                let owner = recv.and_then(|r| field(r, "type")).and_then(|t| type_name(t, src));
                let by_pointer = recv.and_then(|r| field(r, "type")).is_some_and(|t| t.kind() == "pointer_type");
                if let Some(owner) = owner {
                    function(ctx, &f, tree, c, Some((&owner, by_pointer)));
                }
            }
            "type_declaration" => {
                for spec in named(c) {
                    type_spec(ctx, &f, c, spec);
                }
            }
            "const_declaration" | "var_declaration" => values(ctx, &f, c),
            _ => {}
        }
    }
}

/// An exported name starts with a capital; the rest is private to the package.
fn vis_of(name: &str) -> Visibility {
    if name.starts_with(|c: char| c.is_uppercase()) { Visibility::Public } else { Visibility::Private }
}

/// The doc of a declaration: the comment above its line.
fn doc_of(f: &File, n: Node) -> Option<String> {
    doc_above(&f.lines, n.start_position().row + 1, true)
}

/// The parameters of a parameter list: `a, b int` is two of them, `...T` a variadic tail.
fn params_of(list: Option<Node>, src: &str) -> Vec<Param> {
    let mut out = Vec::new();
    let Some(list) = list else { return out };
    for p in named(list) {
        if !matches!(p.kind(), "parameter_declaration" | "variadic_parameter_declaration") {
            continue;
        }
        let mut ty = field(p, "type").map(|t| line(t, src)).unwrap_or_default();
        if p.kind() == "variadic_parameter_declaration" {
            ty = format!("...{}", ty);
        }
        let mut cur = p.walk();
        let names: Vec<String> = p.children_by_field_name("name", &mut cur).map(|x| raw(x, src).to_string()).collect();
        if names.is_empty() {
            out.push(Param { name: String::new(), ty: ty.clone() });
        }
        for name in names {
            out.push(Param { name, ty: ty.clone() });
        }
    }
    out
}

/// What a function returns, and whether its last result is an `error`.
fn result_of(n: Node, src: &str) -> (Option<String>, Vec<String>) {
    let Some(r) = field(n, "result") else { return (None, vec![]) };
    let types = if r.kind() == "parameter_list" { params_of(Some(r), src).into_iter().map(|p| p.ty).collect() } else { vec![line(r, src)] };
    (Some(line(r, src)), types)
}

/// A function, or a method of `owner` (receiver type, taken by pointer?); also an interface's
/// method declaration (no body).
fn function(ctx: &mut Ctx, f: &File, tree: &Tree, n: Node, owner: Option<(&str, bool)>) {
    let Some(name) = field(n, "name").map(|x| raw(x, f.src).to_string()) else { return };
    let body = field(n, "body");
    let signature = match body {
        Some(b) => one_line(&f.src[n.start_byte()..b.start_byte()]),
        None => line(n, f.src),
    };
    let (ret, results) = result_of(n, f.src);
    let mut info = FnInfo { params: params_of(field(n, "parameters"), f.src), ret, ..Default::default() };
    if results.last().is_some_and(|t| t == "error") {
        info.returns_result = true;
        info.result_err = Some("error".into());
        let ok = &results[..results.len() - 1];
        info.result_ok = (!ok.is_empty()).then(|| ok.join(", "));
    }
    if let Some((_, by_pointer)) = owner {
        info.self_kind = if by_pointer { SelfKind::RefMut } else { SelfKind::Ref };
    }
    info.is_main = owner.is_none() && name == "main" && f.main;
    info.is_test = f.test && owner.is_none() && ["Test", "Benchmark", "Example", "Fuzz"].iter().any(|p| name.starts_with(p));
    let is_main = info.is_main;
    let id = ctx.push_item(Item {
        id: 0,
        kind: if owner.is_some() { ItemKind::Method } else { ItemKind::Function },
        name: name.clone(),
        path: match owner {
            Some((o, _)) => format!("{}::{}::{}", f.mpath, o, name),
            None => format!("{}::{}", f.mpath, name),
        },
        module: f.mid,
        vis: vis_of(&name),
        span: span(n, f.file, f.src),
        signature,
        doc: doc_of(f, n),
        attrs: vec![],
        extra: ItemExtra::Fn(info),
    });
    if let Some((o, _)) = owner {
        ctx.group_method(id, o, &format!("func ({})", o));
    }
    if is_main && ctx.project.main.is_none() {
        ctx.project.main = Some(id);
    }
    if body.is_some() {
        ctx.bodies.push(FnBody { item: id, file: f.file, body: Body::Go(TreeBody { tree: tree.clone(), start: n.start_byte(), end: n.end_byte() }) });
    }
}

/// One `type` spec: a struct, an interface, or another named type.
fn type_spec(ctx: &mut Ctx, f: &File, decl: Node, spec: Node) {
    if !matches!(spec.kind(), "type_spec" | "type_alias") {
        return;
    }
    let Some(name) = field(spec, "name").map(|x| raw(x, f.src).to_string()) else { return };
    let Some(ty) = field(spec, "type") else { return };
    // `type X struct` is one line with its declaration; in a `type ( … )` group each spec has its own comment
    let doc = doc_of(f, spec).or_else(|| if spec.start_position().row == decl.start_position().row { doc_of(f, decl) } else { None });
    let params = field(spec, "type_parameters").map(|p| line(p, f.src)).unwrap_or_default();
    let item = |kind: ItemKind, signature: String, extra: ItemExtra| Item {
        id: 0,
        kind,
        name: name.clone(),
        path: format!("{}::{}", f.mpath, name),
        module: f.mid,
        vis: vis_of(&name),
        span: span(spec, f.file, f.src),
        signature,
        doc: doc.clone(),
        attrs: vec![],
        extra,
    };
    match ty.kind() {
        "struct_type" if spec.kind() == "type_spec" => {
            let fields = named(ty).into_iter().flat_map(named).filter(|c| c.kind() == "field_declaration").flat_map(|fd| fields_of(fd, f.src)).collect();
            ctx.push_item(item(ItemKind::Struct, format!("type {}{} struct", name, params), ItemExtra::Struct(StructInfo { fields, derives: vec![], is_tuple: false, is_unit: false })));
        }
        "interface_type" if spec.kind() == "type_spec" => {
            // an embedded interface is one this one requires; type sets (`~int | string`) are not
            let supertraits: Vec<String> = named(ty).into_iter().filter(|c| c.kind() == "type_elem").filter(|c| !raw(*c, f.src).contains(['|', '~'])).filter_map(|c| type_name(c, f.src)).collect();
            let tid = ctx.push_item(item(ItemKind::Trait, format!("type {}{} interface", name, params), ItemExtra::Trait(TraitInfo { supertraits, supertrait_ids: vec![], methods: vec![] })));
            let mut methods = Vec::new();
            for m in named(ty).into_iter().filter(|c| c.kind() == "method_elem") {
                let Some(mname) = field(m, "name").map(|x| raw(x, f.src).to_string()) else { continue };
                let (ret, results) = result_of(m, f.src);
                let returns_error = results.last().is_some_and(|t| t == "error");
                let info = FnInfo { params: params_of(field(m, "parameters"), f.src), ret, returns_result: returns_error, result_err: returns_error.then(|| "error".to_string()), trait_id: Some(tid), trait_name: Some(name.clone()), self_kind: SelfKind::Ref, ..Default::default() };
                methods.push(ctx.push_item(Item {
                    id: 0,
                    kind: ItemKind::Method,
                    name: mname.clone(),
                    path: format!("{}::{}::{}", f.mpath, name, mname),
                    module: f.mid,
                    vis: vis_of(&mname),
                    span: span(m, f.file, f.src),
                    signature: line(m, f.src),
                    doc: doc_of(f, m),
                    attrs: vec![],
                    extra: ItemExtra::Fn(info),
                }));
            }
            if let ItemExtra::Trait(t) = &mut ctx.project.items[tid].extra {
                t.methods = methods;
            }
        }
        _ => {
            let target = line(ty, f.src);
            let eq = if spec.kind() == "type_alias" { " =" } else { "" };
            ctx.push_item(item(ItemKind::TypeAlias, format!("type {}{}{} {}", name, params, eq, crate::shorten(&target, 60)), ItemExtra::TypeAlias { target }));
        }
    }
}

/// The fields one line of a struct declares: `a, b int` is two, an embedded type is named after itself.
fn fields_of(fd: Node, src: &str) -> Vec<Field> {
    let ty = field(fd, "type").map(|t| line(t, src)).unwrap_or_default();
    let mut cur = fd.walk();
    let names: Vec<String> = fd.children_by_field_name("name", &mut cur).map(|x| raw(x, src).to_string()).collect();
    if names.is_empty() {
        let name = field(fd, "type").and_then(|t| type_name(t, src)).unwrap_or_default();
        return vec![Field { vis: vis_of(&name), name, ty, refs: vec![] }];
    }
    names.into_iter().map(|name| Field { vis: vis_of(&name), name, ty: ty.clone(), refs: vec![] }).collect()
}

/// `const` and `var` declarations: one item per name. In a `const ( … )` group a spec without
/// a type repeats the one before (`iota` lists).
fn values(ctx: &mut Ctx, f: &File, decl: Node) {
    let kw = if decl.kind() == "const_declaration" { "const" } else { "var" };
    let specs: Vec<Node> = named(decl).into_iter().flat_map(|c| if c.kind() == "var_spec_list" { named(c) } else { vec![c] }).filter(|c| matches!(c.kind(), "const_spec" | "var_spec")).collect();
    let mut last_ty = String::new();
    for spec in specs {
        let ty = match field(spec, "type") {
            Some(t) => line(t, f.src),
            None if field(spec, "value").is_none() => last_ty.clone(),
            None => String::new(),
        };
        last_ty = ty.clone();
        let doc = doc_of(f, spec).or_else(|| if spec.start_position().row == decl.start_position().row { doc_of(f, decl) } else { None });
        let mut cur = spec.walk();
        let names: Vec<String> = spec.children_by_field_name("name", &mut cur).map(|x| raw(x, f.src).to_string()).collect();
        for name in names {
            if name == "_" {
                continue;
            }
            let text = line(spec, f.src);
            ctx.push_item(Item {
                id: 0,
                kind: if kw == "const" { ItemKind::Const } else { ItemKind::Static },
                name: name.clone(),
                path: format!("{}::{}", f.mpath, name),
                module: f.mid,
                vis: vis_of(&name),
                span: span(spec, f.file, f.src),
                signature: format!("{} {}", kw, crate::shorten(text.split(" = ").next().unwrap_or(&text), 60)),
                doc: doc.clone(),
                attrs: vec![],
                extra: ItemExtra::Const { ty: ty.clone() },
            });
        }
    }
}
