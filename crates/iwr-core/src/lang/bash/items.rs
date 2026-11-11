//! Items of a shell script: its functions, the variables set at its top level (`readonly`
//! ones are constants), and the script itself when it runs commands outside its functions;
//! `source x.sh` as a use of that script's module.

use super::{arguments, basename, command_name, doc_above, line, statements};
use crate::lang::ts::*;
use crate::lang::{Body, TreeBody};
use crate::model::*;
use crate::parse::{Ctx, FnBody};
use std::collections::HashSet;
use tree_sitter::{Node, Tree};

/// Collect the items of one parsed script into `ctx`; returns the script's own item, if it has one.
pub(crate) fn collect_file(ctx: &mut Ctx, tree: &Tree, file: FileId, src: &str, mid: ModuleId, mpath: &str) -> Option<ItemId> {
    let root = tree.root_node();
    let lines: Vec<&str> = src.lines().collect();
    let path = ctx.project.files[file].path.clone();
    let mut seen: HashSet<String> = HashSet::new();
    for c in named(root) {
        match c.kind() {
            "function_definition" => {
                let Some(name) = field(c, "name").map(|x| raw(x, src).to_string()) else { continue };
                let is_test = ctx.project.modules[mid].is_test || name.starts_with("test_");
                let id = ctx.push_item(Item {
                    id: 0,
                    kind: ItemKind::Function,
                    name: name.clone(),
                    path: format!("{}::{}", mpath, name),
                    module: mid,
                    vis: if name.starts_with('_') { Visibility::Private } else { Visibility::Public },
                    span: span(c, file, src),
                    signature: format!("{}()", name),
                    doc: doc_above(&lines, c.start_position().row + 1),
                    attrs: vec![],
                    extra: ItemExtra::Fn(FnInfo { is_test, ..Default::default() }),
                });
                ctx.bodies.push(FnBody { item: id, file, body: Body::Bash(TreeBody { tree: tree.clone(), start: c.start_byte(), end: c.end_byte() }) });
            }
            "variable_assignment" => variable(ctx, file, src, &lines, mid, mpath, c, c, &mut seen),
            "declaration_command" => {
                for a in named(c).into_iter().filter(|a| a.kind() == "variable_assignment") {
                    variable(ctx, file, src, &lines, mid, mpath, c, a, &mut seen);
                }
            }
            "command" if matches!(command_name(c, src).as_str(), "source" | ".") => {
                // `source "$DIR/lib/log.sh"`: the script of that name in the project
                let Some(target) = arguments(c, src).into_iter().next() else { continue };
                let name = basename(&target).to_string();
                let used = ctx.project.files.iter().find(|f| f.lang == Some(SourceLang::Bash) && basename(&f.path) == name).map(|f| super::module_path(&f.path));
                if let Some(used) = used.filter(|u| u != mpath && !ctx.project.modules[mid].uses.contains(u)) {
                    ctx.project.modules[mid].uses.push(used.clone());
                    ctx.use_paths.push((mid, vec![used]));
                }
            }
            _ => {}
        }
    }
    // the script itself: what it runs when started
    if statements(root).is_empty() {
        return None;
    }
    let name = basename(&path).to_string();
    let id = ctx.push_item(Item {
        id: 0,
        kind: ItemKind::Function,
        name: name.clone(),
        path: format!("{}::{}", mpath, name),
        module: mid,
        vis: Visibility::Public,
        span: span(root, file, src),
        signature: name,
        doc: ctx.project.modules[mid].doc.clone(),
        attrs: vec!["script".into()],
        extra: ItemExtra::Fn(FnInfo { is_main: true, is_test: ctx.project.modules[mid].is_test, ..Default::default() }),
    });
    ctx.bodies.push(FnBody { item: id, file, body: Body::Bash(TreeBody { tree: tree.clone(), start: root.start_byte(), end: root.end_byte() }) });
    Some(id)
}

/// A variable set at the top level (`NAME=value`, `readonly NAME=value`): one item, at its first assignment.
fn variable(ctx: &mut Ctx, file: FileId, src: &str, lines: &[&str], mid: ModuleId, mpath: &str, stmt: Node, assign: Node, seen: &mut HashSet<String>) {
    let Some(name) = field(assign, "name").map(|x| raw(x, src).to_string()) else { return };
    if !seen.insert(name.clone()) {
        return;
    }
    let head = line(stmt, src);
    let constant = head.starts_with("readonly ") || (head.starts_with("declare ") || head.starts_with("typeset ")) && head.split_whitespace().nth(1).is_some_and(|f| f.starts_with('-') && f.contains('r'));
    ctx.push_item(Item {
        id: 0,
        kind: if constant { ItemKind::Const } else { ItemKind::Static },
        name: name.clone(),
        path: format!("{}::{}", mpath, name),
        module: mid,
        vis: Visibility::Public,
        span: span(stmt, file, src),
        signature: crate::shorten(&head, 70),
        doc: doc_above(lines, stmt.start_position().row + 1),
        attrs: if head.starts_with("export ") { vec!["export".into()] } else { vec![] },
        extra: ItemExtra::Const { ty: String::new() },
    });
}
