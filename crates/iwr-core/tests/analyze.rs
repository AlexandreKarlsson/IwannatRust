use iwr_core::model::*;
use iwr_core::views::{self, Mode, ViewOptions};

const SRC: &str = r#"
//! Test crate.
mod util {
    /// Doubles.
    pub fn double(x: u32) -> u32 { x * 2 }
    pub trait Shape { fn area(&self) -> f64; }
    pub struct Square(pub f64);
    impl Shape for Square { fn area(&self) -> f64 { self.0 * self.0 } }
}
use util::{double, Shape, Square};

#[derive(Debug)]
enum MyError { Bad(String) }

fn parse(s: &str) -> Result<u32, MyError> {
    let n: u32 = s.parse().map_err(|_| MyError::Bad(s.into()))?;
    if n > 10 { return Err(MyError::Bad("big".into())); }
    Ok(n)
}

fn fact(n: u32) -> u32 { if n == 0 { 1 } else { n * fact(n - 1) } }

fn main() {
    let v = parse("3").unwrap();
    for i in 0..v {
        if i % 2 == 0 { continue; }
        println!("{}", double(i));
    }
    let sq = Square(2.0);
    let _a = sq.area();
    match fact(3) { 6 => println!("ok"), _ => panic!("bad") }
    while v > 0 { break; }
}
"#;

fn project() -> Project {
    iwr_core::analyze_source(SRC)
}

#[test]
fn collects_items_and_modules() {
    let p = project();
    assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
    assert!(p.main.is_some());
    let names: Vec<&str> = p.items.iter().map(|i| i.name.as_str()).collect();
    for n in ["double", "Shape", "Square", "area", "MyError", "parse", "fact", "main"] {
        assert!(names.contains(&n), "missing {n}");
    }
    assert_eq!(p.modules.iter().filter(|m| m.path == "crate::util").count(), 1);
    let double = p.items.iter().find(|i| i.name == "double").unwrap();
    assert_eq!(double.doc.as_deref(), Some("Doubles."));
    assert_eq!(double.signature, "pub fn double(x: u32) -> u32");
    assert_eq!(double.path, "crate::util::double");
}

#[test]
fn resolves_calls_and_flags() {
    let p = project();
    let main = p.item(p.main.unwrap());
    let callees: Vec<String> = p.callees(main.id).iter().filter_map(|c| c.to.map(|t| p.item(t).name.clone())).collect();
    for n in ["parse", "double", "area", "fact", "Square"] {
        assert!(callees.contains(&n.to_string()), "main should call {n}: {:?}", callees);
    }
    let parse_call = p.callees(main.id).into_iter().find(|c| c.callee == "parse").unwrap();
    assert!(parse_call.unwrapped);
    let double_call = p.callees(main.id).into_iter().find(|c| c.callee == "double").unwrap();
    assert!(double_call.in_loop && double_call.in_branch == false);
    let fact = p.items.iter().find(|i| i.name == "fact").unwrap().fn_info().unwrap();
    assert!(fact.is_recursive);
    let parse = p.items.iter().find(|i| i.name == "parse").unwrap().fn_info().unwrap();
    assert!(parse.returns_result);
    assert_eq!(parse.result_err.as_deref(), Some("MyError"));
    assert_eq!(parse.question_marks, 1);
    let mainf = main.fn_info().unwrap();
    assert_eq!(mainf.loops, 2);
    assert!(mainf.branches >= 2);
    assert_eq!(mainf.unwraps, 1);
    assert_eq!(mainf.panics, 1);
}

#[test]
fn type_relations() {
    let p = project();
    let square = p.items.iter().find(|i| i.name == "Square").unwrap().id;
    let shape = p.items.iter().find(|i| i.name == "Shape").unwrap().id;
    assert!(p.type_rels.iter().any(|r| r.from == square && r.to == shape && r.kind == TypeRelKind::Implements));
    let area_impl = p.items.iter().find(|i| i.name == "area" && i.fn_info().unwrap().impl_id.is_some()).unwrap();
    assert_eq!(area_impl.fn_info().unwrap().trait_id, Some(shape));
    assert_eq!(area_impl.fn_info().unwrap().owner_type, Some(square));
}

#[test]
fn cfg_shapes() {
    let p = project();
    let main = p.item(p.main.unwrap()).fn_info().unwrap();
    let cfg = main.cfg.as_ref().unwrap();
    let kinds: Vec<CfgNodeKind> = cfg.nodes.iter().map(|n| n.kind).collect();
    assert_eq!(kinds[0], CfgNodeKind::Entry);
    assert_eq!(kinds[1], CfgNodeKind::Exit);
    assert!(kinds.contains(&CfgNodeKind::Loop));
    assert!(kinds.contains(&CfgNodeKind::If));
    assert!(kinds.contains(&CfgNodeKind::Match));
    assert!(kinds.contains(&CfgNodeKind::Jump));
    assert!(kinds.contains(&CfgNodeKind::Panic));
    assert!(cfg.edges.iter().any(|e| e.kind == CfgEdgeKind::LoopBack));
    assert!(cfg.edges.iter().any(|e| e.kind == CfgEdgeKind::Continue));
    assert!(cfg.edges.iter().any(|e| e.kind == CfgEdgeKind::Break));
    // every node except exit has a successor; every node except entry has a predecessor
    for n in &cfg.nodes {
        if n.kind != CfgNodeKind::Exit {
            assert!(cfg.edges.iter().any(|e| e.from == n.id), "node {} ({:?}) has no successor", n.id, n.kind);
        }
        if n.kind != CfgNodeKind::Entry {
            assert!(cfg.edges.iter().any(|e| e.to == n.id), "node {} ({:?}) has no predecessor", n.id, n.kind);
        }
    }
    let parse = p.items.iter().find(|i| i.name == "parse").unwrap().fn_info().unwrap();
    let pcfg = parse.cfg.as_ref().unwrap();
    assert!(pcfg.nodes.iter().any(|n| n.kind == CfgNodeKind::Propagate));
    assert!(pcfg.nodes.iter().any(|n| n.kind == CfgNodeKind::Return));
    assert!(pcfg.edges.iter().any(|e| e.kind == CfgEdgeKind::Error));
}

#[test]
fn views_build_and_layout() {
    let p = project();
    for mode in Mode::GRAPHS {
        let g = views::build(&p, mode, &ViewOptions::default());
        assert!(!g.nodes.is_empty(), "{mode:?} empty");
        assert!(g.width > 0.0 && g.height > 0.0);
        for e in &g.edges {
            assert!(g.node(&e.from).is_some() && g.node(&e.to).is_some(), "{mode:?}: dangling edge {} -> {}", e.from, e.to);
            if mode != Mode::Structure {
                assert!(e.points.len() >= 2, "{mode:?}: edge {} has no route", e.id);
            }
        }
        // no two non-container nodes of the same depth overlap
        let nodes: Vec<&views::VNode> = g.nodes.iter().filter(|n| !n.container).collect();
        for (i, a) in nodes.iter().enumerate() {
            for b in nodes.iter().skip(i + 1) {
                if a.parent != b.parent {
                    continue;
                }
                let overlap = a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
                assert!(!overlap, "{mode:?}: {} overlaps {}", a.id, b.id);
            }
        }
    }
    // expanding a call-tree node adds children
    let g0 = views::build(&p, Mode::CallTree, &ViewOptions { depth: 1, ..Default::default() });
    let mut opts = ViewOptions { depth: 1, ..Default::default() };
    let parse_node = g0.nodes.iter().find(|n| n.label == "parse").unwrap();
    assert!(!parse_node.expanded);
    opts.expanded.insert(parse_node.id.clone());
    let g1 = views::build(&p, Mode::CallTree, &opts);
    assert!(g1.nodes.len() >= g0.nodes.len());
}

#[test]
fn guide_walks_from_main() {
    let p = project();
    let steps = iwr_core::guide::build(&p, None, &iwr_core::guide::GuideOptions::default());
    assert!(steps.len() > 10);
    assert!(steps.iter().any(|s| s.title.contains("Start at main")));
    assert!(steps.iter().any(|s| s.title.contains("Enter parse")));
    assert!(steps.iter().any(|s| s.kind == iwr_core::guide::StepKind::Loop));
    assert!(steps.iter().any(|s| s.text.contains("recursion")));
    // every step referencing a cfg node points at a real node of the focused function
    for s in &steps {
        if let (Some(f), Some(c)) = (s.focus_item, s.focus_cfg) {
            let cfg = p.item(f).fn_info().unwrap().cfg.as_ref().unwrap();
            assert!(c < cfg.nodes.len());
        }
    }
}

#[test]
fn module_paths() {
    use iwr_core::parse::module_path_for_file as m;
    assert_eq!(m("src/main.rs"), "crate");
    assert_eq!(m("src/lib.rs"), "crate");
    assert_eq!(m("src/net/mod.rs"), "crate::net");
    assert_eq!(m("src/net/client.rs"), "crate::net::client");
    assert_eq!(m("crates/foo/src/lib.rs"), "foo");
    assert_eq!(m("crates/foo/src/a/b.rs"), "foo::a::b");
    assert_eq!(m("tests/it.rs"), "tests::it");
    assert_eq!(m("build.rs"), "build");
}

#[test]
fn block_tree() {
    use iwr_core::model::BlockKind;
    let p = project();
    let main = p.item(p.main.unwrap()).fn_info().unwrap();
    let b = main.blocks.as_ref().unwrap();
    assert_eq!(b.kind, BlockKind::Fn);
    let kinds: Vec<BlockKind> = b.children.iter().map(|c| c.kind).collect();
    assert_eq!(kinds[0], BlockKind::Panic); // let v = parse("3").unwrap()
    assert_eq!(b.children[0].defines, vec!["v".to_string()]);
    let loop_b = b.children.iter().find(|c| c.kind == BlockKind::Loop).unwrap();
    assert_eq!(loop_b.defines, vec!["i".to_string()]);
    assert_eq!(loop_b.uses, vec!["v".to_string()]);
    let if_b = loop_b.children.iter().find(|c| c.kind == BlockKind::If).unwrap();
    assert_eq!(if_b.uses, vec!["i".to_string()]);
    assert_eq!(if_b.children[0].kind, BlockKind::Continue);
    let call = loop_b.children.iter().find(|c| c.kind == BlockKind::Macro).unwrap();
    assert!(call.uses.contains(&"i".to_string()));
    assert_eq!(call.calls.len(), 1); // double
    let m = b.children.iter().find(|c| c.kind == BlockKind::Match).unwrap();
    assert_eq!(m.children.len(), 2);
    assert_eq!(m.children[1].children[0].kind, BlockKind::Panic);
    assert!(b.deepest_at(loop_b.span.line_start).map(|d| d.id == loop_b.id).unwrap_or(false));
    let sig = p.item(p.main.unwrap()).signature.clone();
    assert_eq!(sig, "fn main()");
}

#[test]
fn script_roundtrip_and_refs() {
    use iwr_core::script::{self, Cue};
    let p = project();
    let text = "# T\naudio: a.mp3\n\n## One\n@ flow:crate::main\n! crate::main/b1 src/main.rs:3-4\n= src/main.rs:3:5-9\n[1.5] first\nsecond\n## Two\n!\nthird\n";
    let s = script::parse(text);
    assert_eq!(s.title, "T");
    assert_eq!(s.audio.as_deref(), Some("a.mp3"));
    assert_eq!(s.parts.len(), 2);
    assert_eq!(s.parts[0].cues[0], Cue { say: "first".into(), show: Some("flow:crate::main".into()), hl: Some(vec!["crate::main/b1".into(), "src/main.rs:3-4".into()]), code: Some("src/main.rs:3:5-9".into()), t: Some(1.5) });
    assert_eq!(s.parts[0].cues[1].say, "second");
    assert_eq!(s.parts[1].cues[0].hl, Some(vec![]));
    assert_eq!(script::parse(&script::to_text(&s)), s);
    assert!(script::check(&p, &s).is_empty(), "{:?}", script::check(&p, &s));
    let r = script::resolve(&p, "crate::main/b1").unwrap();
    assert!(r.inner && r.item == p.main);
    let r = script::resolve(&p, "main").unwrap();
    assert!(!r.inner && r.item == p.main);
    let r = script::resolve(&p, "util").unwrap();
    assert!(r.module.is_some());
    let r = script::resolve(&p, "src/main.rs:3:5-9").unwrap();
    assert_eq!((r.span.line_start, r.span.col_start, r.span.col_end), (3, 5, 9));
    assert!(script::resolve(&p, "crate::nope").is_none());
    assert!(script::resolve(&p, "crate::main/b999").is_none());
    let bad = script::check(&p, &script::parse("@ nowhere:crate::main\n! crate::nope\nx"));
    assert_eq!(bad.len(), 2);
    // built-in guide becomes a valid script
    let steps = iwr_core::guide::build(&p, None, &iwr_core::guide::GuideOptions::default());
    let g = script::from_guide(&p, &steps);
    assert!(g.parts.len() >= 2);
    assert!(script::check(&p, &g).is_empty());
    assert_eq!(script::speakable("a `b` **c**\n```rust\nx\n```\n• d"), "a b c d");
    let brief = iwr_core::brief::build(&p, &iwr_core::brief::BriefOptions { only: None, bodies: true, overview: true });
    assert!(brief.contains("fn crate::main") && brief.contains("b1 ") && brief.contains("trait crate::util::Shape"));
}
