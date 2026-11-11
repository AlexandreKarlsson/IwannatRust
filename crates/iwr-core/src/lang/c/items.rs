//! Items of a C file: function definitions (prototypes are skipped), `struct` / `union` /
//! `enum` definitions, `typedef`s, globals, `#define`s; `#include`s as the module's uses.
//! In C++ also classes with their members, methods defined outside their class, `using` aliases.

use super::text::*;
use super::Dialect;
use crate::model::*;
use crate::lang::{Body, TreeBody};
use crate::parse::{Ctx, FnBody};
use tree_sitter::{Node, Tree};

struct File<'a> {
    file: FileId,
    src: &'a str,
    lines: Vec<&'a str>,
    mid: ModuleId,
    mpath: &'a str,
    cpp: bool,
}

/// The class a member is written in, and who may reach the member.
#[derive(Clone, Copy)]
struct InClass<'a> {
    name: &'a str,
    vis: Visibility,
}

/// Collect the items of one parsed file into `ctx`.
pub(crate) fn collect_file(ctx: &mut Ctx, tree: &Tree, file: FileId, src: &str, mid: ModuleId, mpath: &str, dialect: Dialect) {
    let f = File { file, src, lines: src.lines().collect(), mid, mpath, cpp: dialect == Dialect::Cpp };
    top_level(ctx, &f, tree, tree.root_node());
}

/// Walk the top-level nodes, entering preprocessor conditionals, `extern "C" { }` blocks,
/// namespaces and templates.
fn top_level(ctx: &mut Ctx, f: &File, tree: &Tree, n: Node) {
    for c in named(n) {
        match c.kind() {
            "function_definition" => function(ctx, f, tree, c, None),
            "declaration" => declaration(ctx, f, tree, c),
            "struct_specifier" | "union_specifier" | "enum_specifier" | "class_specifier" => {
                type_specifier(ctx, f, tree, c, None);
            }
            "type_definition" => typedef(ctx, f, tree, c),
            "alias_declaration" => using_alias(ctx, f, c),
            "namespace_definition" | "template_declaration" | "export_declaration" => top_level(ctx, f, tree, c),
            "preproc_def" => define(ctx, f, c),
            "preproc_function_def" => define_fn(ctx, f, c),
            "preproc_include" => include(ctx, f, c),
            "preproc_if" | "preproc_ifdef" | "preproc_else" | "preproc_elif" | "preproc_elifdef" | "declaration_list" => top_level(ctx, f, tree, c),
            "linkage_specification" => {
                if let Some(body) = field(c, "body") {
                    if body.kind() == "declaration_list" {
                        top_level(ctx, f, tree, body);
                    } else {
                        // `extern "C" int f(void) { }`: one item
                        let mut cur = body.walk();
                        let _ = &mut cur;
                        match body.kind() {
                            "function_definition" => function(ctx, f, tree, body, None),
                            "declaration" => declaration(ctx, f, tree, body),
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// `static` among the specifiers of a declaration → private to the file.
fn vis_of(n: Node, src: &str) -> Visibility {
    let is_static = children(n).into_iter().any(|c| c.kind() == "storage_class_specifier" && raw(c, src) == "static");
    if is_static { Visibility::Private } else { Visibility::Public }
}

fn has_specifier(n: Node, src: &str, word: &str) -> bool {
    children(n).into_iter().any(|c| matches!(c.kind(), "storage_class_specifier" | "type_qualifier") && raw(c, src) == word)
}

/// The type of a declaration as one short text: `struct task *`, `unsigned int`, `struct {…}`.
fn type_text(ty: Option<Node>, src: &str) -> String {
    let Some(ty) = ty else { return String::new() };
    match ty.kind() {
        "struct_specifier" | "union_specifier" | "enum_specifier" | "class_specifier" => {
            let kw = ty.kind().trim_end_matches("_specifier");
            match field(ty, "name") {
                Some(name) => format!("{} {}", kw, raw(name, src)),
                None => format!("{} {{…}}", kw),
            }
        }
        _ => text(ty, src),
    }
}

/// Type text with the declarator's `*` / `[]` attached: (`char`, `*name`) → `char *`.
fn full_type(ty: &str, deco: &str) -> String {
    if deco.is_empty() {
        ty.to_string()
    } else if deco.starts_with('*') {
        format!("{} {}", ty, deco)
    } else {
        format!("{}{}", ty, deco)
    }
}

fn doc_of(f: &File, n: Node) -> Option<String> {
    doc_above(&f.lines, n.start_position().row + 1, true)
}

/// The parameters of a parameter list: name and type (`...` for a variadic tail; `void` is none).
fn params_of(list: Option<Node>, src: &str) -> Vec<Param> {
    let mut out = Vec::new();
    let Some(list) = list else { return out };
    for p in named(list) {
        match p.kind() {
            "parameter_declaration" | "optional_parameter_declaration" => {
                let ty = type_text(field(p, "type"), src);
                let decl = field(p, "declarator");
                let name = decl.and_then(|d| declarator_name(d, src)).unwrap_or_default();
                let deco = decl.map(|d| declarator_decoration(d, src)).unwrap_or_default();
                let quals: Vec<&str> = children(p).into_iter().filter(|c| c.kind() == "type_qualifier").map(|c| raw(c, src)).collect();
                let mut full = full_type(&ty, &deco);
                if !quals.is_empty() {
                    full = format!("{} {}", quals.join(" "), full);
                }
                if full == "void" && name.is_empty() {
                    continue;
                }
                out.push(Param { name, ty: full });
            }
            "variadic_parameter" | "variadic_parameter_declaration" => out.push(Param { name: "...".into(), ty: String::new() }),
            "identifier" => out.push(Param { name: raw(p, src).into(), ty: String::new() }),
            _ => {}
        }
    }
    out
}

/// A function definition: the item, its body kept for the second pass. In C++ a definition
/// inside a class (`class_`) or named `Type::name` is a method of that type.
fn function(ctx: &mut Ctx, f: &File, tree: &Tree, n: Node, class_: Option<InClass>) {
    let Some(decl) = field(n, "declarator") else { return };
    let Some(name) = declarator_name(decl, f.src) else { return };
    let Some(body) = field(n, "body") else { return };
    // the signature is everything before the body (and before a constructor's initializers)
    let sig_end = children(n).into_iter().find(|c| c.kind() == "field_initializer_list").map(|c| c.start_byte()).unwrap_or(body.start_byte());
    let signature = clean(&f.src[n.start_byte()..sig_end]);
    let owner = if f.cpp { scope_of(decl, f.src).or(class_.map(|c| c.name.to_string())) } else { None };
    let id = push_function(ctx, f, n, decl, &name, signature, owner.as_deref(), class_.map(|c| c.vis));
    let body = TreeBody { tree: tree.clone(), start: n.start_byte(), end: n.end_byte() };
    ctx.bodies.push(FnBody { item: id, file: f.file, body: if f.cpp { Body::Cpp(body) } else { Body::C(body) } });
}

/// The item of a function or method whose declarator is `decl`, inside the node `n`.
fn push_function(ctx: &mut Ctx, f: &File, n: Node, decl: Node, name: &str, signature: String, owner: Option<&str>, vis: Option<Visibility>) -> ItemId {
    let ret_ty = type_text(field(n, "type"), f.src);
    let stars = "*".repeat(pointer_depth(decl));
    let ret = full_type(&ret_ty, &stars);
    let mut info = FnInfo { params: params_of(parameter_list(decl), f.src), ret: (ret != "void" && !ret.is_empty()).then_some(ret), ..Default::default() };
    info.is_main = name == "main" && owner.is_none();
    info.is_test = name.starts_with("test_") || ctx.project.modules[f.mid].is_test;
    if owner.is_some() && !has_specifier(n, f.src, "static") {
        // a `const` method only reads its object
        let is_const = function_declarator(decl).is_some_and(|d| children(d).into_iter().any(|c| c.kind() == "type_qualifier" && raw(c, f.src) == "const"));
        info.self_kind = if is_const { SelfKind::Ref } else { SelfKind::RefMut };
    }
    let mut attrs = Vec::new();
    for word in ["inline", "virtual", "static", "constexpr"] {
        if (word != "static" || owner.is_some()) && has_specifier(n, f.src, word) {
            attrs.push(word.to_string());
        }
    }
    let id = ctx.push_item(Item {
        id: 0,
        kind: if owner.is_some() { ItemKind::Method } else { ItemKind::Function },
        name: name.to_string(),
        path: match owner {
            Some(o) => format!("{}::{}::{}", f.mpath, o, name),
            None => format!("{}::{}", f.mpath, name),
        },
        module: f.mid,
        vis: vis.unwrap_or_else(|| if owner.is_some() { Visibility::Public } else { vis_of(n, f.src) }),
        span: span(n, f.file, f.src),
        signature,
        doc: doc_of(f, n),
        attrs,
        extra: ItemExtra::Fn(info),
    });
    if let Some(o) = owner {
        ctx.group_method(id, o, &format!("{}::", o));
    }
    if name == "main" && owner.is_none() && ctx.project.main.is_none() {
        ctx.project.main = Some(id);
    }
    id
}

/// `*` layers between a function's declarator and its name: the pointer depth of the return type.
fn pointer_depth(n: Node) -> usize {
    match n.kind() {
        "pointer_declarator" => 1 + field(n, "declarator").map(pointer_depth).unwrap_or(0),
        "function_declarator" => 0,
        "reference_declarator" => named(n).into_iter().next().map(pointer_depth).unwrap_or(0),
        _ => field(n, "declarator").map(pointer_depth).unwrap_or(0),
    }
}

/// A top-level declaration: a global (or several), or a `struct x { } var;` with its type.
fn declaration(ctx: &mut Ctx, f: &File, tree: &Tree, n: Node) {
    if let Some(ty) = field(n, "type") {
        if matches!(ty.kind(), "struct_specifier" | "union_specifier" | "enum_specifier" | "class_specifier") && field(ty, "body").is_some() {
            type_specifier(ctx, f, tree, ty, None);
        }
    }
    if has_specifier(n, f.src, "extern") {
        return;
    }
    let ty = type_text(field(n, "type"), f.src);
    let is_const = has_specifier(n, f.src, "const");
    let mut cur = n.walk();
    let decls: Vec<Node> = n.children_by_field_name("declarator", &mut cur).collect();
    for d in decls {
        if is_function_declarator(d) {
            continue; // a prototype: the definition is the item
        }
        let Some(name) = declarator_name(d, f.src) else { continue };
        let full = full_type(&ty, &declarator_decoration(d, f.src));
        let kind = if is_const || has_specifier(n, f.src, "constexpr") { ItemKind::Const } else { ItemKind::Static };
        let head = clean(&f.src[n.start_byte()..d.end_byte()]);
        let signature = match head.split_once(" = ") {
            Some((h, _)) => h.to_string(),
            None => head,
        };
        ctx.push_item(Item {
            id: 0,
            kind,
            name: name.clone(),
            path: format!("{}::{}", f.mpath, name),
            module: f.mid,
            vis: vis_of(n, f.src),
            span: span(n, f.file, f.src),
            signature,
            doc: doc_of(f, n),
            attrs: vec![],
            extra: ItemExtra::Const { ty: full },
        });
    }
}

/// A `struct` / `union` / `enum` with a body: a type item. `alias` names an anonymous one
/// (`typedef struct { } name;`). Returns the item id when one was made.
fn type_specifier(ctx: &mut Ctx, f: &File, tree: &Tree, n: Node, alias: Option<&str>) -> Option<ItemId> {
    let body = field(n, "body")?;
    let kw = n.kind().trim_end_matches("_specifier");
    let tag = field(n, "name").map(|x| raw(x, f.src).to_string());
    let name = tag.clone().or_else(|| alias.map(|a| a.to_string()))?;
    let mut attrs = Vec::new();
    if tag.is_none() {
        attrs.push("typedef".to_string());
    }
    let bases: Vec<String> = children(n).into_iter().filter(|c| c.kind() == "base_class_clause").flat_map(named).filter(|b| b.kind() != "access_specifier").map(|b| text(b, f.src)).collect();
    let signature = if bases.is_empty() { format!("{} {}", kw, name) } else { format!("{} {} : {}", kw, name, bases.join(", ")) };
    // members of a `class` are private until an access specifier says otherwise
    let member_vis = if kw == "class" { Visibility::Private } else { Visibility::Public };
    // the doc sits above the `typedef` / `struct` line, which may be the parent's
    let doc_node = n.parent().filter(|p| matches!(p.kind(), "type_definition" | "declaration" | "template_declaration")).unwrap_or(n);
    let extra = match kw {
        "enum" => {
            let variants = named(body).into_iter().filter(|e| e.kind() == "enumerator").map(|e| Variant { name: field(e, "name").map(|x| raw(x, f.src).to_string()).unwrap_or_default(), fields: vec![] }).collect();
            ItemExtra::Enum(EnumInfo { variants, derives: vec![] })
        }
        _ => {
            if kw != "struct" {
                attrs.push(kw.to_string());
            }
            ItemExtra::Struct(StructInfo { fields: fields_of(body, f.src, member_vis), derives: vec![], is_tuple: false, is_unit: false })
        }
    };
    let id = ctx.push_item(Item {
        id: 0,
        kind: if kw == "enum" { ItemKind::Enum } else { ItemKind::Struct },
        name: name.clone(),
        path: format!("{}::{}", f.mpath, name),
        module: f.mid,
        vis: Visibility::Public,
        span: span(n, f.file, f.src),
        signature,
        doc: doc_of(f, doc_node),
        attrs,
        extra,
    });
    for b in bases {
        ctx.rels.push((id, b, TypeRelKind::Extends));
    }
    if f.cpp && kw != "enum" {
        members(ctx, f, tree, body, &name, member_vis);
    }
    Some(id)
}

/// `public` / `protected` / `private` as the model's visibility.
fn access(n: Node, src: &str) -> Visibility {
    match raw(n, src).trim_end_matches(':').trim() {
        "public" => Visibility::Public,
        "protected" => Visibility::Restricted,
        _ => Visibility::Private,
    }
}

/// The members of a C++ class body that are not fields: methods defined in the class, pure
/// virtual ones (declared only: there is no definition elsewhere), types nested in the class.
fn members(ctx: &mut Ctx, f: &File, tree: &Tree, body: Node, class_: &str, default_vis: Visibility) {
    let mut vis = default_vis;
    for m in named(body) {
        let m = if m.kind() == "template_declaration" { named(m).into_iter().last().unwrap_or(m) } else { m };
        match m.kind() {
            "access_specifier" => vis = access(m, f.src),
            "function_definition" => function(ctx, f, tree, m, Some(InClass { name: class_, vis })),
            "field_declaration" | "declaration" => {
                if let Some(ty) = field(m, "type").filter(|t| field(*t, "body").is_some()) {
                    type_specifier(ctx, f, tree, ty, None);
                }
                let Some(decl) = field(m, "declarator").filter(|d| is_function_declarator(*d)) else { continue };
                // `virtual double area() const = 0;`
                let pure = field(m, "default_value").is_some_and(|v| raw(v, f.src) == "0");
                if !pure {
                    continue;
                }
                let Some(name) = declarator_name(decl, f.src) else { continue };
                let signature = clean(raw(m, f.src).trim_end_matches(';'));
                push_function(ctx, f, m, decl, &name, signature, Some(class_), Some(vis));
            }
            "preproc_if" | "preproc_ifdef" | "preproc_else" | "preproc_elif" => members(ctx, f, tree, m, class_, vis),
            _ => {}
        }
    }
}

/// The fields of a `struct` / `union` body; nested anonymous members are one field each.
fn fields_of(body: Node, src: &str, default_vis: Visibility) -> Vec<Field> {
    let mut out = Vec::new();
    let mut vis = default_vis;
    for fd in named(body) {
        match fd.kind() {
            "access_specifier" => vis = access(fd, src),
            "field_declaration" => {
                let ty = type_text(field(fd, "type"), src);
                let mut cur = fd.walk();
                let decls: Vec<Node> = fd.children_by_field_name("declarator", &mut cur).collect();
                if decls.is_empty() && !ty.is_empty() {
                    out.push(Field { name: String::new(), ty: ty.clone(), vis, refs: vec![] });
                }
                for d in decls {
                    if is_function_declarator(d) {
                        continue; // a method's declaration
                    }
                    let name = declarator_name(d, src).unwrap_or_default();
                    let full = full_type(&ty, &declarator_decoration(d, src));
                    out.push(Field { name, ty: full, vis, refs: vec![] });
                }
            }
            "preproc_if" | "preproc_ifdef" | "preproc_else" | "preproc_elif" => out.extend(fields_of(fd, src, vis)),
            _ => {}
        }
    }
    out
}

/// `typedef`: a new name for a type (alias item), or the definition of a struct / union / enum
/// named by it.
fn typedef(ctx: &mut Ctx, f: &File, tree: &Tree, n: Node) {
    let Some(decl) = field(n, "declarator") else { return };
    let Some(name) = declarator_name(decl, f.src) else { return };
    let ty = field(n, "type");
    if let Some(ty) = ty {
        if matches!(ty.kind(), "struct_specifier" | "union_specifier" | "enum_specifier") && field(ty, "body").is_some() {
            let tag = field(ty, "name").map(|x| raw(x, f.src).to_string());
            type_specifier(ctx, f, tree, ty, Some(&name));
            match tag {
                // `typedef struct node { } node_t;`: the struct is `node`, `node_t` its alias
                Some(t) if t != name => alias_item(ctx, f, n, &name, &format!("{} {}", ty.kind().trim_end_matches("_specifier"), t), None),
                _ => {}
            }
            return;
        }
    }
    let base = type_text(ty, f.src);
    match parameter_list(decl) {
        // `typedef int (*cmp)(const void *, const void *)`: a function pointer
        Some(params) => {
            let params = params_of(Some(params), f.src).iter().map(|p| if p.name.is_empty() { p.ty.clone() } else if p.ty.is_empty() { p.name.clone() } else { format!("{} {}", p.ty, p.name) }).collect::<Vec<_>>().join(", ");
            let target = format!("{} (*)({})", base, params);
            alias_item(ctx, f, n, &name, &target, Some(format!("typedef {} (*{})({})", base, name, params)));
        }
        None => alias_item(ctx, f, n, &name, &full_type(&base, &declarator_decoration(decl, f.src)), None),
    }
}

/// `using Shapes = std::vector<Shape *>;` (C++): an alias item.
fn using_alias(ctx: &mut Ctx, f: &File, n: Node) {
    let Some(name) = field(n, "name").map(|x| raw(x, f.src).to_string()) else { return };
    let target = field(n, "type").map(|t| text(t, f.src)).unwrap_or_default();
    alias_item(ctx, f, n, &name, &target, Some(format!("using {} = {}", name, target)));
}

/// A `typedef` alias item; `signature` when the default `typedef TARGET NAME` does not read well.
fn alias_item(ctx: &mut Ctx, f: &File, n: Node, name: &str, target: &str, signature: Option<String>) {
    ctx.push_item(Item {
        id: 0,
        kind: ItemKind::TypeAlias,
        name: name.to_string(),
        path: format!("{}::{}", f.mpath, name),
        module: f.mid,
        vis: Visibility::Public,
        span: span(n, f.file, f.src),
        signature: signature.unwrap_or_else(|| format!("typedef {} {}", target, name)),
        doc: doc_of(f, n),
        attrs: vec![],
        extra: ItemExtra::TypeAlias { target: target.to_string() },
    });
}

/// `#define NAME value`: a constant. The include guard (`#ifndef X_H` / `#define X_H`) is not one.
fn define(ctx: &mut Ctx, f: &File, n: Node) {
    let Some(name) = field(n, "name").map(|x| raw(x, f.src).to_string()) else { return };
    let value = field(n, "value").map(|v| clean(raw(v, f.src))).unwrap_or_default();
    let guard = value.is_empty() && n.parent().is_some_and(|p| p.kind() == "preproc_ifdef" && field(p, "name").is_some_and(|g| raw(g, f.src) == name));
    if guard {
        return;
    }
    let signature = if value.is_empty() { format!("#define {}", name) } else { format!("#define {} {}", name, value) };
    ctx.push_item(Item {
        id: 0,
        kind: ItemKind::Const,
        name: name.clone(),
        path: format!("{}::{}", f.mpath, name),
        module: f.mid,
        vis: Visibility::Public,
        span: span(n, f.file, f.src),
        signature,
        doc: doc_of(f, n),
        attrs: vec!["macro".into()],
        extra: ItemExtra::Const { ty: "#define".into() },
    });
}

/// `#define NAME(a, b) …`: a function-like macro, a function item without a body.
fn define_fn(ctx: &mut Ctx, f: &File, n: Node) {
    let Some(name) = field(n, "name").map(|x| raw(x, f.src).to_string()) else { return };
    let params: Vec<Param> = field(n, "parameters").map(|p| named(p).into_iter().map(|i| Param { name: raw(i, f.src).to_string(), ty: String::new() }).collect()).unwrap_or_default();
    let signature = format!("#define {}({})", name, params.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", "));
    ctx.push_item(Item {
        id: 0,
        kind: ItemKind::Function,
        name: name.clone(),
        path: format!("{}::{}", f.mpath, name),
        module: f.mid,
        vis: Visibility::Public,
        span: span(n, f.file, f.src),
        signature,
        doc: doc_of(f, n),
        attrs: vec!["macro".into()],
        extra: ItemExtra::Fn(FnInfo { params, ..Default::default() }),
    });
}

/// `#include "x.h"` is a use of module `x`; `#include <stdio.h>` names an external library.
fn include(ctx: &mut Ctx, f: &File, n: Node) {
    let Some(path) = field(n, "path") else { return };
    let t = raw(path, f.src).trim().to_string();
    let used = if let Some(inner) = t.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        let stem = inner.rsplit('/').next().unwrap_or(inner);
        stem.rsplit_once('.').map(|(s, _)| s).unwrap_or(stem).to_string()
    } else {
        t.trim_start_matches('<').trim_end_matches('>').to_string()
    };
    if used.is_empty() {
        return;
    }
    ctx.project.modules[f.mid].uses.push(used.clone());
    ctx.use_paths.push((f.mid, vec![used]));
}
