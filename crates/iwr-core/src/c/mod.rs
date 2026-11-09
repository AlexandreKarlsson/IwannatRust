//! C sources, read with tree-sitter: one module per file (a header joins the `.c` of the same
//! name), items for functions, structs, unions, enums, typedefs, globals and macros, then the
//! calls, flow graph and block tree of every function body.
//!
//! No preprocessing: `#include`s become the module's uses, `#define`s are items, the code
//! inside `#if` blocks is read as it stands (every branch). Parsing is tolerant: a file with
//! errors still gives its items; the first error is reported in the project's diagnostics.

pub mod blocks;
pub mod body;
pub mod cfg;
pub mod items;
pub(crate) mod text;

use crate::model::*;
use crate::parse::Ctx;

/// A parser for C.
pub fn parser() -> tree_sitter::Parser {
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c::LANGUAGE.into()).expect("tree-sitter-c grammar");
    p
}

/// Module path of a C file: the directory under `src/` / `include/` / `lib/` then the file stem,
/// so `src/net/sock.c` and `include/net/sock.h` are both `crate::net::sock`. `main.c` is the root.
pub fn module_path(rel_path: &str) -> String {
    let parts: Vec<&str> = rel_path.split('/').filter(|p| !p.is_empty()).collect();
    let src_idx = parts.iter().rposition(|p| matches!(*p, "src" | "include" | "lib"));
    let rest = match src_idx {
        Some(i) => &parts[i + 1..],
        None => &parts[..],
    };
    let mut segs = vec!["crate".to_string()];
    for (i, p) in rest.iter().enumerate() {
        let is_last = i + 1 == rest.len();
        let stem = if is_last { p.rsplit_once('.').map(|(s, _)| s).unwrap_or(p) } else { p };
        if is_last && stem == "main" {
            continue;
        }
        segs.push(stem.to_string());
    }
    segs.join("::")
}

/// First pass for the C files of a project: parse each, create its module, collect its items.
pub(crate) fn collect(ctx: &mut Ctx, files: Vec<(FileId, String, String)>) {
    let mut parser = parser();
    for (file, path, content) in files {
        let Some(tree) = parser.parse(&content, None) else {
            ctx.project.diagnostics.push(format!("{}: could not parse", path));
            continue;
        };
        if let Some(err) = first_error(tree.root_node()) {
            let p = err.start_position();
            ctx.project.diagnostics.push(format!("{}:{}:{}: syntax error ({})", path, p.row + 1, p.column + 1, if err.is_missing() { "missing token" } else { "unexpected text" }));
        }
        let mpath = module_path(&path);
        let mid = ctx.module_for(&mpath);
        if path.split('/').any(|c| c == "tests" || c == "test") {
            ctx.project.modules[mid].is_test = true;
        }
        let lines: Vec<&str> = content.lines().collect();
        let header_so_far = ctx.project.modules[mid].file.is_some_and(|f| ctx.project.files[f].path.ends_with(".h"));
        let takes_over = ctx.project.modules[mid].file.is_none() || path.ends_with(".c") && header_so_far;
        if takes_over {
            // the `.c` is the module's file and says what it is for; a header only until its `.c` shows up
            ctx.project.modules[mid].file = Some(file);
            if let Some(doc) = text::file_doc(&lines) {
                ctx.project.modules[mid].doc = Some(doc);
            }
        } else if ctx.project.modules[mid].doc.is_none() {
            ctx.project.modules[mid].doc = text::file_doc(&lines);
        }
        items::collect_file(ctx, &tree, file, &content, mid, &mpath);
    }
}

/// The first error or missing node of a tree, if any.
fn first_error(n: tree_sitter::Node) -> Option<tree_sitter::Node> {
    if !n.has_error() {
        return None;
    }
    if n.is_error() || n.is_missing() {
        return Some(n);
    }
    let mut cur = n.walk();
    let kids: Vec<tree_sitter::Node> = n.children(&mut cur).collect();
    kids.into_iter().find_map(first_error)
}
