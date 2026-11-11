//! Go analysis: the `godemo` fixture (a small task scheduler) through the whole pipeline.

use iwr_core::model::*;
use std::path::Path;

fn project() -> Project {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/godemo");
    iwr_core::analyze_path(&dir).unwrap()
}

fn item<'a>(p: &'a Project, path: &str) -> &'a Item {
    p.items.iter().find(|i| i.path == path).unwrap_or_else(|| panic!("no item {path}"))
}

#[test]
fn a_package_is_a_module() {
    assert_eq!(iwr_core::lang::go::module_path("main.go"), "crate");
    assert_eq!(iwr_core::lang::go::module_path("store/task.go"), "crate::store");
    assert_eq!(iwr_core::lang::go::module_path("cmd/app/main.go"), "crate::cmd::app");

    let p = project();
    assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
    let paths: Vec<&str> = p.modules.iter().map(|m| m.path.as_str()).collect();
    assert_eq!(paths, ["crate", "crate::store"]);
    // the file with the package comment stands for the package, not its test file
    let store = &p.modules[1];
    assert!(p.files[store.file.unwrap()].path.ends_with("store/store.go"));
    assert_eq!(store.doc.as_deref(), Some("Package store keeps the tasks waiting to run."));
    assert!(p.modules[0].doc.as_deref().unwrap().starts_with("Command godemo"));
    // an import under the go.mod module path is a package of the project, the rest is outside
    assert!(p.modules[0].uses.contains(&"crate::store".to_string()));
    assert_eq!(p.external_crates, ["errors", "fmt", "os", "testing"]);
    assert_eq!(p.main, Some(item(&p, "crate::main").id));
    assert_eq!(p.root_label(), "godemo");
}

#[test]
fn items() {
    let p = project();
    let fill = item(&p, "crate::fill");
    assert_eq!(fill.signature, "func fill(s *store.Store, names []string) error");
    assert_eq!(fill.vis, Visibility::Private);
    assert_eq!(fill.doc.as_deref(), Some("fill puts the named tasks into the store."));
    let f = fill.fn_info().unwrap();
    assert_eq!(f.params.iter().map(|p| (p.name.as_str(), p.ty.as_str())).collect::<Vec<_>>(), [("s", "*store.Store"), ("names", "[]string")]);
    assert!(f.returns_result);
    assert_eq!((f.loops, f.branches), (1, 2));
    let run = item(&p, "crate::run").fn_info().unwrap();
    assert_eq!(run.ret.as_deref(), Some("(int, error)"));
    assert_eq!(run.result_ok.as_deref(), Some("int"));
    // `a, b Weigher` is two parameters
    assert_eq!(item(&p, "crate::store::heavier").fn_info().unwrap().params.len(), 2);

    let task = item(&p, "crate::store::Task");
    assert_eq!(task.kind, ItemKind::Struct);
    assert_eq!(task.vis, Visibility::Public);
    let ItemExtra::Struct(s) = &task.extra else { panic!() };
    assert_eq!(s.fields.iter().map(|f| (f.name.as_str(), f.vis)).collect::<Vec<_>>(), [("Name", Visibility::Public), ("Priority", Visibility::Public), ("weight", Visibility::Private)]);

    // a method hangs under its receiver's type through a group
    let add = item(&p, "crate::store::Store::Add");
    assert_eq!(add.kind, ItemKind::Method);
    assert_eq!(add.fn_info().unwrap().owner_type, Some(item(&p, "crate::store::Store").id));
    assert_eq!(add.fn_info().unwrap().self_kind, SelfKind::RefMut);
    let group = &p.items[add.fn_info().unwrap().impl_id.unwrap()];
    assert_eq!(group.name, "func (Store)");
    let ItemExtra::Impl(g) = &group.extra else { panic!() };
    assert_eq!(g.methods.len(), 4);

    // an interface is a trait with its method declarations; a type that has them implements it
    let weigher = item(&p, "crate::store::Weigher");
    assert_eq!(weigher.kind, ItemKind::Trait);
    let decl = item(&p, "crate::store::Weigher::Weight");
    assert!(decl.fn_info().unwrap().cfg.is_none());
    assert_eq!(decl.doc.as_deref(), Some("Weight is higher for what goes first."));
    assert!(p.type_rels.iter().any(|r| r.from == task.id && r.to == weigher.id && r.kind == TypeRelKind::Implements));
    assert_eq!(item(&p, "crate::store::Task::Weight").fn_info().unwrap().trait_id, Some(weigher.id));

    assert_eq!(item(&p, "crate::store::Priority").signature, "type Priority int");
    assert_eq!(item(&p, "crate::store::Low").signature, "const Low Priority");
    // a spec without a type repeats the one before
    assert!(matches!(&item(&p, "crate::store::High").extra, ItemExtra::Const { ty } if ty == "Priority"));
    assert_eq!(item(&p, "crate::store::ErrFull").kind, ItemKind::Static);
    assert!(item(&p, "crate::store::TestAdd").fn_info().unwrap().is_test);
}

#[test]
fn calls() {
    let p = project();
    let to = |from: &str, callee: &str| p.calls.iter().find(|c| p.items[c.from].path == from && c.callee == callee).unwrap_or_else(|| panic!("no call {from} -> {callee}")).to.map(|t| p.items[t].path.as_str());
    // through the import, to the package of the project
    assert_eq!(to("crate::fill", "store.NewTask"), Some("crate::store::NewTask"));
    assert_eq!(to("crate::main", "fill"), Some("crate::fill"));
    assert_eq!(to("crate::fill", "s.Add"), Some("crate::store::Store::Add"));
    // a package from outside stays a call without a target
    assert_eq!(to("crate::run", "fmt.Errorf"), None);
    // on a value whose type is an interface: the interface's method
    assert_eq!(to("crate::store::heavier", "a.Weight"), Some("crate::store::Weigher::Weight"));
    let made = p.calls.iter().find(|c| c.callee == "Task").unwrap();
    assert_eq!(made.kind, CallKind::Construct);
    // builtins and conversions are not calls
    assert!(!p.calls.iter().any(|c| matches!(c.callee.as_str(), "len" | "append" | "int")));
    assert_eq!(item(&p, "crate::main").fn_info().unwrap().panics, 1);
}

#[test]
fn flow() {
    let p = project();
    let kinds = |path: &str| -> Vec<(CfgNodeKind, String)> { item(&p, path).fn_info().unwrap().cfg.as_ref().unwrap().nodes.iter().map(|n| (n.kind, n.label.clone())).collect() };
    let fill = kinds("crate::fill");
    assert!(fill.contains(&(CfgNodeKind::Loop, "for i, name := range names".into())), "{fill:?}");
    assert!(fill.contains(&(CfgNodeKind::If, "if err != nil".into())));
    // an error handed back is an error path, `return nil` is not
    assert!(fill.contains(&(CfgNodeKind::ReturnErr, "return err".into())));
    assert!(fill.contains(&(CfgNodeKind::Return, "return nil".into())));
    // the comment above a statement titles its node
    let cfg = item(&p, "crate::fill").fn_info().unwrap().cfg.as_ref().unwrap();
    assert!(cfg.nodes.iter().any(|n| n.note.as_deref() == Some("a full store is the caller's problem")));

    let run = kinds("crate::run");
    assert!(run.contains(&(CfgNodeKind::Match, "switch t.Priority".into())), "{run:?}");
    let main = kinds("crate::main");
    assert!(main.iter().any(|(k, _)| *k == CfgNodeKind::Halt), "{main:?}");
    assert!(main.iter().any(|(k, l)| *k == CfgNodeKind::Panic && l == "panic(err)"));

    let blocks = item(&p, "crate::run").fn_info().unwrap().blocks.clone().unwrap();
    let lp = &blocks.children[1];
    assert_eq!((lp.kind, lp.label.as_str()), (BlockKind::Loop, "for s.Len() > 0"));
    let sw = lp.children.iter().find(|b| b.kind == BlockKind::Match).unwrap();
    assert_eq!(sw.children.iter().map(|a| a.label.as_str()).collect::<Vec<_>>(), ["case store.High", "case store.Low, store.Normal", "default"]);
    assert_eq!(lp.children[0].defines, ["t", "err"]);
}

#[test]
fn switch_without_default_select_and_goto() {
    let src = "package x\n\nfunc f(ch chan int, v any) int {\n\tselect {\n\tcase x := <-ch:\n\t\treturn x\n\tdefault:\n\t}\n\tswitch t := v.(type) {\n\tcase int:\n\t\treturn t\n\t}\n\tfor {\n\t\tbreak\n\t}\n\treturn 0\n}\n";
    let p = iwr_core::analyze_sources("x", "", vec![("x.go".to_string(), src.to_string())]);
    assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
    let f = p.items.iter().find(|i| i.name == "f").unwrap().fn_info().unwrap();
    let cfg = f.cfg.as_ref().unwrap();
    let labels: Vec<&str> = cfg.nodes.iter().map(|n| n.label.as_str()).collect();
    assert!(labels.contains(&"select"), "{labels:?}");
    assert!(labels.contains(&"switch t := v.(type)"), "{labels:?}");
    // no default: the switch can be skipped
    assert!(cfg.edges.iter().any(|e| e.label.as_deref() == Some("no match")));
    let blocks = f.blocks.as_ref().unwrap();
    assert_eq!(blocks.children[1].defines, ["t"]);
    assert_eq!(blocks.children[0].children[0].label, "case x := <-ch");
}

#[test]
fn highlights_go() {
    use iwr_core::highlight::{file, Tok};
    let lines = file("a.go", "func (s *Store) Len() int {\n\treturn len(`a\nb`)\n}\n");
    let tok = |l: usize, w: &str| lines[l].iter().find(|(_, t)| t == w).map(|(k, _)| *k).unwrap_or_else(|| panic!("no {w}"));
    assert_eq!(tok(0, "func"), Tok::Keyword);
    assert_eq!(tok(0, "int"), Tok::Type);
    assert_eq!(tok(1, "len"), Tok::Builtin);
    assert_eq!(tok(1, "`a"), Tok::Str);
    assert_eq!(tok(2, "b`"), Tok::Str);
}

#[test]
fn packages_reach_each_other_without_a_root_package() {
    // no Go file at the root: `cmd/app` and `internal/store` are still one project
    let files = vec![
        ("go.mod".to_string(), "module example.com/app\n".to_string()),
        ("cmd/app/main.go".to_string(), "package main\n\nimport \"example.com/app/internal/store\"\n\nfunc main() {\n\tstore.Open()\n}\n".to_string()),
        ("internal/store/store.go".to_string(), "package store\n\nfunc Open() {}\n".to_string()),
    ];
    let p = iwr_core::analyze_sources("app", "", files);
    let call = p.calls.iter().find(|c| c.callee == "store.Open").unwrap();
    assert_eq!(call.to.map(|t| p.items[t].path.as_str()), Some("crate::internal::store::Open"));
    assert_eq!(p.main.map(|m| p.items[m].path.as_str()), Some("crate::cmd::app::main"));
}
