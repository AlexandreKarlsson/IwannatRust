//! Go sources, read with tree-sitter: one module per package (a directory), items for
//! functions, methods (grouped per receiver type), structs, interfaces (as traits), other
//! named types, constants and variables, then the calls, flow graph and block tree of every body.
//!
//! No type checking: `pkg.Func()` is resolved through the file's imports (those that start with
//! the `go.mod` module path are packages of the project), `x.Method()` by name with the type
//! names written in the function as hints. A type implements an interface of the project when
//! it has all its methods ([`link`]).

pub mod blocks;
pub(crate) mod body;
pub mod cfg;
mod items;

use crate::lang::ts::*;
use crate::lang::{Analyzed, Source};
use crate::model::*;
use crate::parse::Ctx;
use crate::resolve::Index;
use std::collections::{HashMap, HashSet};
use tree_sitter::Node;

/// A parser for Go.
pub fn parser() -> tree_sitter::Parser {
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_go::LANGUAGE.into()).expect("tree-sitter-go grammar");
    p
}

/// Module path of a Go file: its directory (`store/task.go` → `crate::store`); the files at
/// the root are the root module.
pub fn module_path(rel_path: &str) -> String {
    let mut segs = vec!["crate"];
    let dirs: Vec<&str> = rel_path.split('/').filter(|p| !p.is_empty()).collect();
    segs.extend(&dirs[..dirs.len().saturating_sub(1)]);
    segs.join("::")
}

/// The text of a node on one line, as written (Go sources are formatted by `gofmt`).
pub(crate) fn line(n: Node, src: &str) -> String {
    one_line(raw(n, src))
}

/// The statements of a `{ }` block (or of a case): tree-sitter-go wraps them in a `statement_list`.
pub(crate) fn stmts(n: Node) -> Vec<Node> {
    named(n).into_iter().flat_map(|c| if c.kind() == "statement_list" { named(c) } else { vec![c] }).collect()
}

/// Identifiers an expression reads: variables and functions, not the field after a `.`.
/// `shallow` stops at nested blocks and function literals.
pub(crate) fn idents(n: Node, src: &str, shallow: bool, out: &mut Vec<String>) {
    match n.kind() {
        "identifier" => {
            let s = raw(n, src).to_string();
            if !out.contains(&s) {
                out.push(s);
            }
        }
        "block" | "func_literal" if shallow => {}
        _ => {
            for c in named(n) {
                idents(c, src, shallow, out);
            }
        }
    }
}

/// The name of the type a type expression is about: `Store` for `*Store`, `store.Store`,
/// `Stack[T]`, `[]Store`.
pub(crate) fn type_name(n: Node, src: &str) -> Option<String> {
    match n.kind() {
        "type_identifier" => Some(raw(n, src).to_string()),
        "qualified_type" | "generic_type" => field(n, "name").or_else(|| field(n, "type")).and_then(|x| type_name(x, src)),
        _ => named(n).into_iter().find_map(|c| type_name(c, src)),
    }
}

/// The `import`s of the file `n` is in: name used in the code → import path.
pub(crate) fn imports(n: Node, src: &str) -> Vec<(String, String)> {
    let mut root = n;
    while let Some(p) = root.parent() {
        root = p;
    }
    let mut out = Vec::new();
    for decl in named(root).into_iter().filter(|c| c.kind() == "import_declaration") {
        let specs: Vec<Node> = named(decl).into_iter().flat_map(|c| if c.kind() == "import_spec_list" { named(c) } else { vec![c] }).filter(|c| c.kind() == "import_spec").collect();
        for s in specs {
            let Some(path) = field(s, "path").map(|p| raw(p, src).trim_matches(['"', '`']).to_string()) else { continue };
            let name = field(s, "name").map(|x| raw(x, src).to_string()).unwrap_or_else(|| path.rsplit('/').next().unwrap_or(&path).to_string());
            out.push((name, path));
        }
    }
    out
}

/// The module paths of the project's `go.mod` files (`example.com/app`), longest first.
pub(crate) fn go_modules(p: &Project) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = p
        .files
        .iter()
        .filter(|f| f.path == "go.mod" || f.path.ends_with("/go.mod"))
        .filter_map(|f| {
            let name = f.content.lines().find_map(|l| l.trim().strip_prefix("module "))?.trim().trim_matches('"').to_string();
            let dir = f.path.strip_suffix("go.mod").unwrap_or("").trim_end_matches('/').to_string();
            Some((name, dir))
        })
        .collect();
    out.sort_by_key(|(name, _)| std::cmp::Reverse(name.len()));
    out
}

/// The module of the project an import path names (`example.com/app/store` → `crate::store`),
/// or `None` for a package from outside.
pub(crate) fn local_import(mods: &[(String, String)], path: &str) -> Option<String> {
    let (name, dir) = mods.iter().find(|(name, _)| path == name || path.starts_with(&format!("{}/", name)))?;
    let rest = path[name.len()..].trim_start_matches('/');
    let full = [dir.as_str(), rest].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join("/");
    Some(if full.is_empty() { "crate".to_string() } else { format!("crate::{}", full.replace('/', "::")) })
}

/// First pass for the Go files of a project: parse each, create its package's module, collect its items.
pub(crate) fn collect(ctx: &mut Ctx, files: Vec<Source>) {
    let mut parser = parser();
    let mods = go_modules(ctx.project);
    for Source { file, path, content } in files {
        ctx.parsing(file);
        let Some(tree) = parser.parse(&content, None) else {
            ctx.project.diagnostics.push(format!("{}: could not parse", path));
            continue;
        };
        if let Some(d) = syntax_error(tree.root_node(), &path) {
            ctx.project.diagnostics.push(d);
        }
        let mpath = module_path(&path);
        let mid = ctx.module_for(&mpath);
        let lines: Vec<&str> = content.lines().collect();
        // the package's doc is the comment above a `package` line; its file stands for the module
        let doc = named(tree.root_node()).into_iter().find(|c| c.kind() == "package_clause").and_then(|c| doc_above(&lines, c.start_position().row + 1, true));
        let is_test = path.ends_with("_test.go");
        let current = ctx.project.modules[mid].file;
        let takes_over = match current {
            None => true,
            Some(f) => ctx.project.files[f].path.ends_with("_test.go") && !is_test || ctx.project.modules[mid].doc.is_none() && doc.is_some(),
        };
        if takes_over {
            ctx.project.modules[mid].file = Some(file);
        }
        if ctx.project.modules[mid].doc.is_none() {
            ctx.project.modules[mid].doc = doc;
        }
        items::collect_file(ctx, &tree, file, &content, mid, &mpath, &mods);
    }
}

/// Second pass for one Go function: its calls, flow graph, block tree and counts.
pub(crate) fn analyze(p: &Project, ix: &Index, item: ItemId, file: FileId, node: Node) -> Analyzed {
    let fn_span = p.items[item].span;
    let src = &p.files[file].content;
    let (calls, stats) = body::analyze(p, ix, item, file, node, src);
    let cfg = cfg::build(node, file, &calls, fn_span, src);
    let blocks = blocks::build(node, file, &calls, fn_span, src);
    Analyzed { calls, cfg, blocks, stats }
}

/// Interfaces are satisfied without saying so: a Go type that has every method of a Go
/// interface of the project implements it. Run once owners are linked.
pub(crate) fn link(p: &mut Project) {
    let is_go = |p: &Project, it: &Item| p.files[it.span.file].lang == Some(SourceLang::Go);
    // type → names of its methods
    let mut methods: HashMap<ItemId, HashSet<String>> = HashMap::new();
    for it in &p.items {
        if let Some(owner) = it.fn_info().filter(|f| f.impl_id.is_some()).and_then(|f| f.owner_type) {
            methods.entry(owner).or_default().insert(it.name.clone());
        }
    }
    let mut found: Vec<(ItemId, ItemId)> = Vec::new();
    for tr in p.items.iter().filter(|i| i.kind == ItemKind::Trait && is_go(p, i)) {
        let ItemExtra::Trait(t) = &tr.extra else { continue };
        let wanted: Vec<&str> = t.methods.iter().map(|m| p.items[*m].name.as_str()).collect();
        if wanted.is_empty() {
            continue;
        }
        for (ty, have) in &methods {
            if is_go(p, &p.items[*ty]) && wanted.iter().all(|w| have.contains(*w)) {
                found.push((*ty, tr.id));
            }
        }
    }
    found.sort();
    for (ty, tr) in found {
        p.type_rels.push(TypeRel { from: ty, to: tr, kind: TypeRelKind::Implements, label: None });
        // the methods that answer the interface point at it: "implemented by" works from the interface
        let wanted: Vec<String> = match &p.items[tr].extra {
            ItemExtra::Trait(t) => t.methods.iter().map(|m| p.items[*m].name.clone()).collect(),
            _ => vec![],
        };
        let tr_name = p.items[tr].name.clone();
        for it in &mut p.items {
            let name = it.name.clone();
            if let Some(f) = it.fn_info_mut().filter(|f| f.owner_type == Some(ty) && f.impl_id.is_some() && f.trait_id.is_none()) {
                if wanted.contains(&name) {
                    f.trait_id = Some(tr);
                    f.trait_name = Some(tr_name.clone());
                }
            }
        }
    }
}
