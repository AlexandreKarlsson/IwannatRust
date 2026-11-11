//! C++ analysis: the `cppdemo` fixture (a few shapes) through the whole pipeline.

use iwr_core::model::*;
use std::path::Path;

fn project() -> Project {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cppdemo");
    iwr_core::analyze_path(&dir).unwrap()
}

fn item<'a>(p: &'a Project, path: &str) -> &'a Item {
    p.items.iter().find(|i| i.path == path).unwrap_or_else(|| panic!("no item {path}"))
}

#[test]
fn files_and_modules() {
    let p = project();
    assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
    assert!(p.files.iter().filter(|f| f.is_code()).all(|f| f.lang == Some(SourceLang::Cpp)));
    let paths: Vec<&str> = p.modules.iter().map(|m| m.path.as_str()).collect();
    assert_eq!(paths, ["crate", "crate::shape"]);
    // the header joins the source of the same name, which is the module's file
    let shape = &p.modules[1];
    assert!(p.files[shape.file.unwrap()].path.ends_with("src/shape.cpp"));
    assert_eq!(p.main, Some(item(&p, "crate::main").id));
    assert_eq!(p.external_crates, ["iostream", "stdexcept", "string", "vector"]);
}

#[test]
fn a_header_is_cpp_next_to_cpp_sources() {
    let files = vec![("src/a.cpp".to_string(), "#include \"a.h\"\nint A::get() { return 1; }\n".to_string()), ("src/a.h".to_string(), "class A {\npublic:\n    int get();\n};\n".to_string())];
    let p = iwr_core::analyze_sources("x", "", files);
    assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
    assert!(p.files.iter().all(|f| f.lang == Some(SourceLang::Cpp)));
    assert_eq!(item(&p, "crate::a::A").kind, ItemKind::Struct);
    assert_eq!(item(&p, "crate::a::A::get").kind, ItemKind::Method);
    // alone, a `.h` is C
    let p = iwr_core::analyze_sources("x", "", vec![("a.h".to_string(), "int f(void);\n".to_string())]);
    assert_eq!(p.files[0].lang, Some(SourceLang::C));
}

#[test]
fn classes_methods_and_bases() {
    let p = project();
    let shape = item(&p, "crate::shape::Shape");
    assert_eq!(shape.kind, ItemKind::Struct);
    assert_eq!(shape.signature, "class Shape");
    assert_eq!(shape.doc.as_deref(), Some("Anything with an area."));
    let ItemExtra::Struct(s) = &shape.extra else { panic!() };
    // methods are not fields; `fill` comes after `protected:`
    assert_eq!(s.fields.iter().map(|f| (f.name.as_str(), f.ty.as_str(), f.vis)).collect::<Vec<_>>(), [("fill", "Fill", Visibility::Restricted)]);

    let circle = item(&p, "crate::shape::Circle");
    assert_eq!(circle.signature, "class Circle : Shape");
    let ItemExtra::Struct(s) = &circle.extra else { panic!() };
    assert_eq!(s.fields[0].vis, Visibility::Private);
    let rect = item(&p, "crate::shape::Rect");
    assert!(p.type_rels.iter().any(|r| r.from == circle.id && r.to == shape.id && r.kind == TypeRelKind::Extends));
    assert!(p.type_rels.iter().any(|r| r.from == rect.id && r.to == shape.id && r.kind == TypeRelKind::Extends));

    // a method written in the class, one written outside as `Type::name`, a pure virtual one
    let scale = item(&p, "crate::shape::Circle::scale");
    assert_eq!(scale.kind, ItemKind::Method);
    assert_eq!(scale.fn_info().unwrap().owner_type, Some(circle.id));
    assert_eq!(scale.fn_info().unwrap().self_kind, SelfKind::RefMut);
    assert_eq!(scale.doc.as_deref(), Some("Grow by a factor."));
    let describe = item(&p, "crate::shape::Shape::describe");
    assert_eq!(describe.signature, "std::string Shape::describe() const");
    assert_eq!(describe.fn_info().unwrap().owner_type, Some(shape.id));
    assert_eq!(describe.fn_info().unwrap().self_kind, SelfKind::Ref);
    assert!(describe.fn_info().unwrap().cfg.is_some());
    let area = item(&p, "crate::shape::Shape::area");
    assert!(area.fn_info().unwrap().cfg.is_none());
    assert_eq!(area.signature, "virtual double area() const = 0");
    // a declaration whose definition is elsewhere is not a second item
    assert_eq!(p.items.iter().filter(|i| i.path == "crate::shape::Shape::describe").count(), 1);
    // the constructor: its initializers are not part of the signature
    assert_eq!(item(&p, "crate::shape::Rect::Rect").signature, "Rect::Rect(double w, double h)");

    // the views reach the methods of a type through its groups, one per file
    let groups: Vec<&Item> = p.items.iter().filter(|i| matches!(&i.extra, ItemExtra::Impl(ii) if ii.target_id == Some(shape.id))).collect();
    assert_eq!(groups.len(), 2);
    assert!(groups.iter().all(|g| g.name == "Shape::"));

    assert_eq!(item(&p, "crate::shape::Shapes").signature, "using Shapes = std::vector<Shape *>");
    assert_eq!(item(&p, "crate::shape::PI").kind, ItemKind::Const);
    // a template is entered
    assert_eq!(item(&p, "crate::shape::clamp").kind, ItemKind::Function);
}

#[test]
fn calls() {
    let p = project();
    let to = |from: &str, callee: &str| p.calls.iter().find(|c| p.items[c.from].path == from && c.callee == callee).unwrap_or_else(|| panic!("no call {from} -> {callee}")).to.map(|t| p.items[t].path.as_str());
    assert_eq!(to("crate::main", "make_shapes"), Some("crate::make_shapes"));
    assert_eq!(to("crate::main", "geo::total_area"), Some("crate::shape::total_area"));
    assert_eq!(to("crate::main", "s.describe"), Some("crate::shape::Shape::describe"));
    // a bare name inside a method is a method of the same type
    assert_eq!(to("crate::shape::Shape::describe", "area"), Some("crate::shape::Shape::area"));
    let new = p.calls.iter().find(|c| c.callee == "new geo::Circle").unwrap();
    assert_eq!(new.kind, CallKind::Construct);
    assert_eq!(new.to, Some(item(&p, "crate::shape::Circle").id));
}

#[test]
fn flow_of_try_catch_throw_and_range_for() {
    let p = project();
    let main = item(&p, "crate::main").fn_info().unwrap();
    let cfg = main.cfg.as_ref().unwrap();
    assert!(cfg.nodes.iter().any(|n| n.kind == CfgNodeKind::If && n.label.starts_with("catch (const std::exception &e)")), "{:?}", cfg.nodes.iter().map(|n| &n.label).collect::<Vec<_>>());
    assert!(cfg.nodes.iter().any(|n| n.kind == CfgNodeKind::Loop && n.label == "for (auto *s : shapes)"));
    assert!(cfg.nodes.iter().any(|n| n.kind == CfgNodeKind::ReturnErr || n.label.contains("return 1")));
    let blocks = main.blocks.as_ref().unwrap();
    let labels: Vec<&str> = blocks.children.iter().map(|b| b.label.as_str()).collect();
    assert_eq!(labels, ["geo::Shapes shapes = make_shapes()", "try", "catch (const std::exception &e)", "for (auto *s : shapes)", "return 0"]);
    assert_eq!(blocks.children[3].defines, ["s"]);

    let ctor = item(&p, "crate::shape::Rect::Rect").fn_info().unwrap();
    assert_eq!(ctor.panics, 1);
    assert!(ctor.cfg.as_ref().unwrap().nodes.iter().any(|n| n.kind == CfgNodeKind::Panic && n.label.starts_with("throw")));
}

#[test]
fn highlights_cpp() {
    use iwr_core::highlight::{file, Tok};
    let lines = file("a.cpp", "class Shape : public Base {\n    virtual double area() const = 0;\n};\n");
    let tok = |l: usize, w: &str| lines[l].iter().find(|(_, t)| t == w).map(|(k, _)| *k).unwrap_or_else(|| panic!("no {w}"));
    assert_eq!(tok(0, "class"), Tok::Keyword);
    assert_eq!(tok(0, "Shape"), Tok::Type);
    assert_eq!(tok(1, "virtual"), Tok::Keyword);
    assert_eq!(tok(1, "double"), Tok::Type);
    assert_eq!(tok(1, "area"), Tok::Fn);
}
