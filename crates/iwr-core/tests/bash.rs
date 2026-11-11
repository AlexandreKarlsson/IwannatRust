//! Shell analysis: the `shdemo` fixture (a deploy script, its build script, a log library)
//! through the whole pipeline.

use iwr_core::model::*;
use std::path::Path;

fn project() -> Project {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shdemo");
    iwr_core::analyze_path(&dir).unwrap()
}

fn item<'a>(p: &'a Project, path: &str) -> &'a Item {
    p.items.iter().find(|i| i.path == path).unwrap_or_else(|| panic!("no item {path}"))
}

#[test]
fn a_script_is_a_module() {
    assert_eq!(iwr_core::lang::bash::module_path("deploy.sh"), "crate::deploy");
    assert_eq!(iwr_core::lang::bash::module_path("scripts/iwr"), "crate::scripts::iwr");

    let p = project();
    assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
    let paths: Vec<&str> = p.modules.iter().filter(|m| m.file.is_some()).map(|m| m.path.as_str()).collect();
    assert_eq!(paths, ["crate::build", "crate::deploy", "crate::lib::log"]);
    let deploy = p.modules.iter().find(|m| m.path == "crate::deploy").unwrap();
    assert_eq!(deploy.doc.as_deref(), Some("Deploy the site: build it, then copy it to every host."));
    // `source …/lib/log.sh` is a use of that script's module
    assert_eq!(deploy.uses, ["crate::lib::log"]);
    // the script no other script names is where the project starts
    assert_eq!(p.main, Some(item(&p, "crate::deploy::deploy.sh").id));
}

#[test]
fn a_script_without_extension_is_told_by_its_first_line() {
    let files = vec![("bin/run".to_string(), "#!/usr/bin/env bash\necho hi\n".to_string()), ("README".to_string(), "#!not a script\n".to_string()), ("notes".to_string(), "echo no\n".to_string())];
    let p = iwr_core::analyze_sources("x", "", files);
    assert_eq!(p.files.iter().map(|f| f.lang).collect::<Vec<_>>(), [Some(SourceLang::Bash), None, None]);
    assert_eq!(item(&p, "crate::bin::run::run").kind, ItemKind::Function);
    assert_eq!(SourceLang::of_file("x", "#!/usr/bin/python\n"), None);
    assert_eq!(SourceLang::of_file("x", "#!/usr/bin/env -S bash -e\n"), Some(SourceLang::Bash));
}

#[test]
fn items() {
    let p = project();
    let copy = item(&p, "crate::deploy::copy_to");
    assert_eq!(copy.signature, "copy_to()");
    assert_eq!(copy.doc.as_deref(), Some("Copy the build to one host; 1 when the host is down."));
    assert_eq!(copy.fn_info().unwrap().branches, 1);
    assert_eq!(item(&p, "crate::deploy::TARGET_DIR").kind, ItemKind::Const);
    assert_eq!(item(&p, "crate::deploy::TARGET_DIR").signature, "readonly TARGET_DIR=/var/www/site");
    assert_eq!(item(&p, "crate::deploy::DRY_RUN").kind, ItemKind::Static);
    // a variable is one item, at its first assignment
    assert_eq!(p.items.iter().filter(|i| i.name == "DRY_RUN").count(), 1);
    // the script itself: the commands outside its functions
    let script = item(&p, "crate::deploy::deploy.sh");
    assert!(script.fn_info().unwrap().is_main);
    assert_eq!((script.fn_info().unwrap().loops, script.fn_info().unwrap().branches), (2, 2));
    // a library of functions runs nothing by itself
    assert!(!p.items.iter().any(|i| i.name == "log.sh"));
    assert_eq!(item(&p, "crate::lib::log::die").doc.as_deref(), Some("Print the message and stop the script."));
}

#[test]
fn calls() {
    let p = project();
    let to = |from: &str, callee: &str| p.calls.iter().find(|c| p.items[c.from].path == from && c.callee == callee).unwrap_or_else(|| panic!("no call {from} -> {callee}")).to.map(|t| p.items[t].path.as_str());
    assert_eq!(to("crate::deploy::deploy.sh", "copy_to"), Some("crate::deploy::copy_to"));
    // a function of a sourced script, another script run by its path
    assert_eq!(to("crate::deploy::copy_to", "warn"), Some("crate::lib::log::warn"));
    assert_eq!(to("crate::deploy::deploy.sh", "./build.sh"), Some("crate::build::build.sh"));
    // programs from outside are not calls
    assert!(!p.calls.iter().any(|c| matches!(c.callee.as_str(), "rsync" | "echo" | "ping" | "set")));
    let in_loop = p.calls.iter().find(|c| c.callee == "copy_to").unwrap();
    assert!(in_loop.in_loop);
}

#[test]
fn a_shell_command_is_not_a_function_of_another_language() {
    let files = vec![("src/main.rs".to_string(), "fn main() { build(); }\nfn build() {}\n".to_string()), ("run.sh".to_string(), "build\n./tool\n".to_string())];
    let p = iwr_core::analyze_sources("x", "", files);
    assert!(!p.calls.iter().any(|c| p.items[c.from].name == "run.sh"));
    // the Rust entry point stays the project's
    assert_eq!(p.main.map(|m| p.items[m].path.as_str()), Some("crate::main"));
}

#[test]
fn flow() {
    let p = project();
    let nodes = |path: &str| -> Vec<(CfgNodeKind, String)> { item(&p, path).fn_info().unwrap().cfg.as_ref().unwrap().nodes.iter().map(|n| (n.kind, n.label.clone())).collect() };
    let copy = nodes("crate::deploy::copy_to");
    assert!(copy.contains(&(CfgNodeKind::If, "if ! ping -c 1 \"$host\" > /dev/null".into())), "{copy:?}");
    assert!(copy.contains(&(CfgNodeKind::ReturnErr, "return 1".into())));
    // the `#` comment above a statement titles its node
    let cfg = item(&p, "crate::deploy::copy_to").fn_info().unwrap().cfg.as_ref().unwrap();
    assert!(cfg.nodes.iter().any(|n| n.note.as_deref() == Some("the real work")));
    assert!(nodes("crate::deploy::usage").iter().any(|(k, l)| *k == CfgNodeKind::Halt && l.contains("exit with code 2")));

    let script = nodes("crate::deploy::deploy.sh");
    assert!(script.contains(&(CfgNodeKind::Match, "case \"${1:-}\"".into())), "{script:?}");
    // `test || function`: a branch, taken when the test fails
    assert!(script.contains(&(CfgNodeKind::If, "[ $# -gt 0 ] ||".into())), "{script:?}");
    assert!(script.contains(&(CfgNodeKind::Loop, "for host in \"$@\"".into())));
    assert!(script.contains(&(CfgNodeKind::Loop, "while [ \"$failed\" -gt 3 ]".into())));
    let cfg = item(&p, "crate::deploy::deploy.sh").fn_info().unwrap().cfg.as_ref().unwrap();
    assert!(cfg.edges.iter().any(|e| e.label.as_deref() == Some("fails")));
    // `*)` is the default: the case cannot be skipped
    assert!(!cfg.edges.iter().any(|e| e.label.as_deref() == Some("no match")));
    // a plain `a && b` stays one statement
    assert!(nodes("crate::build::build.sh").iter().any(|(_, l)| l.contains("cp -r src/. build/ && echo")));

    let blocks = item(&p, "crate::deploy::deploy.sh").fn_info().unwrap().blocks.clone().unwrap();
    assert_eq!(blocks.label, "deploy.sh");
    let case = blocks.children.iter().find(|b| b.kind == BlockKind::Match).unwrap();
    assert_eq!(case.children.iter().map(|a| a.label.as_str()).collect::<Vec<_>>(), ["-n", "-h|--help", "*"]);
    let chain: Vec<(BlockKind, &str)> = blocks.children.iter().rev().take(3).rev().map(|b| (b.kind, b.label.as_str())).collect();
    assert_eq!(chain, [(BlockKind::If, "if [ \"$failed\" -gt 0 ]"), (BlockKind::ElseIf, "elif [ \"$DRY_RUN\" -eq 1 ]"), (BlockKind::Else, "else")]);
    let lp = blocks.children.iter().find(|b| b.kind == BlockKind::Loop).unwrap();
    assert_eq!(lp.defines, ["host"]);
    assert_eq!(lp.children[0].uses, ["host", "failed"]);
}
