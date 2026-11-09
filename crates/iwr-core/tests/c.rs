//! C analysis: the `cdemo` fixture (a small task scheduler in C) through the whole pipeline.

use iwr_core::model::*;
use std::path::Path;

fn project() -> Project {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cdemo");
    iwr_core::analyze_path(&dir).unwrap()
}

fn item<'a>(p: &'a Project, name: &str) -> &'a Item {
    p.items.iter().find(|i| i.name == name).unwrap_or_else(|| panic!("no item {name}"))
}

#[test]
fn module_paths() {
    assert_eq!(iwr_core::c::module_path("src/net/sock.c"), "crate::net::sock");
    assert_eq!(iwr_core::c::module_path("include/net/sock.h"), "crate::net::sock");
    assert_eq!(iwr_core::c::module_path("main.c"), "crate");
    assert_eq!(iwr_core::c::module_path("src/main.c"), "crate");
    assert_eq!(iwr_core::c::module_path("foo.c"), "crate::foo");
}

#[test]
fn collects_modules_and_items() {
    let p = project();
    assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
    assert!(!p.has_rust());
    assert_eq!(p.root_label(), "cdemo");
    let paths: Vec<&str> = p.modules.iter().map(|m| m.path.as_str()).collect();
    assert_eq!(paths, ["crate", "crate::task", "crate::store"]);
    // the header joins the .c of the same name; the .c is the module's file
    let task = p.modules.iter().find(|m| m.path == "crate::task").unwrap();
    assert!(p.files[task.file.unwrap()].path.ends_with("src/task.c"));
    assert!(task.uses.contains(&"task".to_string()));
    assert_eq!(p.files[task.file.unwrap()].lang, Some(SourceLang::C));
    assert!(p.modules[0].doc.as_deref().unwrap().starts_with("Schedule a few tasks"));

    let names: Vec<&str> = p.items.iter().map(|i| i.name.as_str()).collect();
    for n in ["main", "fill", "names", "NAMES_LEN", "MAX_TASKS", "priority", "task", "task_t", "task_cmp", "task_new", "task_weight", "task_free", "store", "store_add", "store_pop", "store_clear", "made"] {
        assert!(names.contains(&n), "missing {n}");
    }
    // include guards and prototypes are not items
    assert!(!names.contains(&"TASK_H"));
    assert_eq!(names.iter().filter(|n| **n == "task_new").count(), 1);

    let fill = item(&p, "fill");
    assert_eq!(fill.vis, Visibility::Private);
    assert_eq!(fill.signature, "static int fill(struct store *s)");
    assert_eq!(fill.doc.as_deref(), Some("Fill the store from the names table; 0 on success."));
    let f = fill.fn_info().unwrap();
    assert_eq!(f.params[0].ty, "struct store *");
    assert_eq!(f.ret.as_deref(), Some("int"));
    assert_eq!(f.loops, 1);
    assert_eq!(f.branches, 2);

    let new = item(&p, "task_new");
    assert_eq!(new.fn_info().unwrap().ret.as_deref(), Some("task_t *"));
    assert_eq!(new.fn_info().unwrap().params.len(), 3);
    assert_eq!(p.main, Some(item(&p, "main").id));

    let task = item(&p, "task");
    assert_eq!(task.kind, ItemKind::Struct);
    match &task.extra {
        ItemExtra::Struct(s) => {
            assert_eq!(s.fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["name", "prio", "cost", "next"]);
            assert_eq!(s.fields[3].ty, "struct task *");
        }
        _ => panic!(),
    }
    let prio = item(&p, "priority");
    match &prio.extra {
        ItemExtra::Enum(e) => assert_eq!(e.variants.len(), 3),
        _ => panic!(),
    }
    assert_eq!(item(&p, "task_t").signature, "typedef struct task task_t");
    assert_eq!(item(&p, "task_cmp").signature, "typedef int (*task_cmp)(const task_t *, const task_t *)");
    assert_eq!(item(&p, "MAX_TASKS").signature, "#define MAX_TASKS 64");
    assert_eq!(item(&p, "MAX_TASKS").kind, ItemKind::Const);
    assert_eq!(item(&p, "made").kind, ItemKind::Static);
    assert_eq!(item(&p, "names").kind, ItemKind::Const);
    assert!(p.external_crates.contains(&"stdio.h".to_string()));
}

#[test]
fn resolves_calls_and_relations() {
    let p = project();
    let edge = |from: &str, callee: &str| p.calls.iter().find(|c| p.items[c.from].name == from && c.callee == callee).unwrap_or_else(|| panic!("no call {from} -> {callee}"));
    assert_eq!(edge("fill", "task_new").to, Some(item(&p, "task_new").id));
    assert!(edge("fill", "task_new").in_loop);
    assert!(edge("fill", "task_free").in_branch);
    assert_eq!(edge("main", "fill").to, Some(item(&p, "fill").id));
    assert!(edge("store_clear", "store_pop").in_loop);
    assert_eq!(edge("task_weight", "assert").to, None);
    assert_eq!(item(&p, "task_weight").fn_info().unwrap().panics, 1);

    let rel = |from: &str, to: &str, kind: TypeRelKind| p.type_rels.iter().filter(|r| p.items[r.from].name == from && p.items[r.to].name == to && r.kind == kind).count();
    assert_eq!(rel("store", "task_t", TypeRelKind::Contains), 1);
    assert_eq!(rel("task", "priority", TypeRelKind::Contains), 1);
    assert_eq!(rel("task_t", "task", TypeRelKind::Aliases), 1);
    assert_eq!(rel("task_cmp", "task_t", TypeRelKind::Aliases), 1);
    assert_eq!(rel("store_add", "task_t", TypeRelKind::UsesInSignature), 1);
}

#[test]
fn switch_goto_and_halt_in_the_flow() {
    let p = project();
    // switch: one arm edge per case label, breaks leave the switch, fall-through joins the next case
    let cfg = item(&p, "task_weight").fn_info().unwrap().cfg.as_ref().unwrap();
    let sw = cfg.nodes.iter().find(|n| n.kind == CfgNodeKind::Match).unwrap();
    assert_eq!(sw.label, "switch t->prio");
    let arms: Vec<&str> = cfg.edges.iter().filter(|e| e.from == sw.id && e.kind == CfgEdgeKind::Arm).map(|e| e.label.as_deref().unwrap()).collect();
    assert_eq!(arms, ["case URGENT", "case NORMAL", "case LOW", "default"]);
    let low = cfg.edges.iter().filter(|e| e.from == sw.id && e.label.as_deref() == Some("case LOW")).next().unwrap();
    let normal = cfg.edges.iter().filter(|e| e.from == sw.id && e.label.as_deref() == Some("case NORMAL")).next().unwrap();
    assert_eq!(low.to, normal.to, "an empty case falls into the next");
    assert_eq!(cfg.edges.iter().filter(|e| e.kind == CfgEdgeKind::Break).count(), 2);
    assert!(cfg.nodes.iter().any(|n| n.kind == CfgNodeKind::Panic && n.label.starts_with("assert")));
    assert_eq!(cfg.nodes.iter().find(|n| n.label.starts_with("w = 2")).unwrap().note.as_deref(), Some("urgent tasks jump the queue"));

    // goto: jump nodes linked to the label's node
    let cfg = item(&p, "fill").fn_info().unwrap().cfg.as_ref().unwrap();
    let done = cfg.nodes.iter().find(|n| n.label == "done:").unwrap();
    let gotos: Vec<&CfgNode> = cfg.nodes.iter().filter(|n| n.label == "goto done").collect();
    assert_eq!(gotos.len(), 2);
    for g in gotos {
        assert!(cfg.edges.iter().any(|e| e.from == g.id && e.to == done.id && e.label.as_deref() == Some("goto done")));
    }
    assert!(cfg.nodes.iter().any(|n| n.kind == CfgNodeKind::Loop && n.label.starts_with("for (int i = 0;")));

    // exit() after a print: one halt node; `return EXIT_FAILURE` is an error return
    let cfg = item(&p, "main").fn_info().unwrap().cfg.as_ref().unwrap();
    let halt = cfg.nodes.iter().find(|n| n.kind == CfgNodeKind::Halt).unwrap();
    assert_eq!(halt.label, "print message, exit with code 2");
    assert!(cfg.nodes.iter().any(|n| n.kind == CfgNodeKind::ReturnErr && n.label == "return EXIT_FAILURE"));
    // strings keep their spaces
    assert!(cfg.nodes.iter().any(|n| n.detail.contains("\"%s (%d)\\n\"")), "{:?}", cfg.nodes.iter().map(|n| &n.detail).collect::<Vec<_>>());
    // while with an assignment in its condition
    assert!(cfg.nodes.iter().any(|n| n.kind == CfgNodeKind::Loop && n.label.starts_with("while (t = store_pop(&s))")));

    let cfg = item(&p, "store_clear").fn_info().unwrap().cfg.as_ref().unwrap();
    assert!(cfg.nodes.iter().any(|n| n.kind == CfgNodeKind::Loop && n.label == "do … while t != NULL"));
}

#[test]
fn block_tree_scopes() {
    let p = project();
    let root = item(&p, "fill").fn_info().unwrap().blocks.as_ref().unwrap();
    assert_eq!(root.kind, BlockKind::Fn);
    assert_eq!(root.defines, ["s"]);
    let lp = root.children.iter().find(|b| b.kind == BlockKind::Loop).unwrap();
    assert_eq!(lp.defines, ["i"]);
    let call = &lp.children[0];
    assert_eq!(call.kind, BlockKind::Call);
    assert_eq!(call.defines, ["t"]);
    assert_eq!(call.uses, ["i"]);
    let iff = lp.children.iter().find(|b| b.label.starts_with("if store_add")).unwrap();
    assert_eq!(iff.uses, ["s", "t"]);
    assert!(iff.children.iter().any(|b| b.kind == BlockKind::Break && b.label == "goto done"));

    let root = item(&p, "task_weight").fn_info().unwrap().blocks.as_ref().unwrap();
    let sw = root.children.iter().find(|b| b.kind == BlockKind::Match).unwrap();
    assert_eq!(sw.children.iter().map(|a| a.label.as_str()).collect::<Vec<_>>(), ["case URGENT", "case NORMAL", "case LOW", "default"]);
    assert_eq!(sw.children[0].children[0].note.as_deref(), Some("urgent tasks jump the queue"));
}

#[test]
fn mixed_project_keeps_both_languages() {
    let files = vec![
        ("src/main.rs".to_string(), "fn main() { helper(); }\nfn helper() {}".to_string()),
        ("csrc/fast.c".to_string(), "/** Fast path. */\nint fast(int x) { return x * 2; }".to_string()),
    ];
    let p = iwr_core::analyze_sources("mixed", "", files);
    assert!(p.has_rust());
    assert_eq!(p.root_label(), "crate mixed");
    assert!(p.items.iter().any(|i| i.name == "helper"));
    let fast = item(&p, "fast");
    assert_eq!(p.modules[fast.module].path, "crate::csrc::fast");
    assert_eq!(fast.doc.as_deref(), Some("Fast path."));
    assert_eq!(p.files.iter().filter(|f| f.is_code()).count(), 2);
}

#[test]
fn tolerates_syntax_errors() {
    let p = iwr_core::analyze_sources("bad", "", vec![("a.c".to_string(), "int ok(void) { return 1; }\nint broken( { \nint after(void) { return 2; }".to_string())]);
    assert!(p.items.iter().any(|i| i.name == "ok"));
    assert_eq!(p.diagnostics.len(), 1, "{:?}", p.diagnostics);
    assert!(p.diagnostics[0].starts_with("a.c:") && p.diagnostics[0].contains("syntax error"), "{}", p.diagnostics[0]);
}

#[test]
fn highlights_c() {
    use iwr_core::highlight::{self, Tok};
    let lines = highlight::file("x.c", "#include <stdio.h>\n/** doc */\nstatic int main(void) { size_t n = NULL; return 0; } // end");
    assert_eq!(lines[0][0], (Tok::Attr, "#include <stdio.h>".to_string()));
    assert_eq!(lines[1][0].0, Tok::Doc);
    let l = &lines[2];
    let tok = |w: &str| l.iter().find(|(_, t)| t == w).map(|(k, _)| *k).unwrap_or_else(|| panic!("no token {w}"));
    assert_eq!(tok("static"), Tok::Keyword);
    assert_eq!(tok("int"), Tok::Type);
    assert_eq!(tok("main"), Tok::Fn);
    assert_eq!(tok("size_t"), Tok::Type);
    assert_eq!(tok("NULL"), Tok::Builtin);
    assert_eq!(tok("0"), Tok::Num);
    assert_eq!(tok("// end"), Tok::Comment);
}
