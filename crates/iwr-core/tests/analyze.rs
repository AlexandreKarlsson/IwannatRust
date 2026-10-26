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
    assert!(pcfg.nodes.iter().any(|n| matches!(n.kind, CfgNodeKind::Return | CfgNodeKind::ReturnErr)));
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
fn crate_named_after_its_package() {
    let files = vec![
        ("pkgs/config-handler/Cargo.toml".to_string(), "[package]\nname = \"z-config-handler\"\n\n[dependencies]\nname = \"x\"\n".to_string()),
        ("pkgs/config-handler/src/main.rs".to_string(), "mod poll;\nfn main() { poll::run(); }\n".to_string()),
        ("pkgs/config-handler/src/poll.rs".to_string(), "pub fn run() {}\n".to_string()),
        ("pkgs/plain/src/lib.rs".to_string(), "pub fn f() {}\n".to_string()),
    ];
    let p = iwr_core::analyze_sources("ws", "", files);
    let paths: Vec<&str> = p.modules.iter().map(|m| m.path.as_str()).collect();
    assert!(paths.contains(&"z-config-handler"), "{paths:?}");
    assert!(paths.contains(&"z-config-handler::poll"), "{paths:?}");
    assert!(paths.contains(&"plain"), "{paths:?}");
    assert!(!paths.contains(&"config-handler"), "{paths:?}");
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
    assert_eq!(s.parts[0].cues[0], Cue { say: "first".into(), show: Some("flow:crate::main".into()), hl: Some(vec!["crate::main/b1".into(), "src/main.rs:3-4".into()]), code: Some("src/main.rs:3:5-9".into()), t: Some(1.5), questions: vec![], ops: vec![], side: None, pop: vec![], tour: None, focus: None });
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
    // line refs into a non-Rust file
    let tf = iwr_core::analyze_sources("ws", "", vec![
        ("src/main.rs".to_string(), "fn main() {}\n".to_string()),
        ("infra/main.tf".to_string(), "locals {\n  name = \"x\"\n}\n".to_string()),
    ]);
    let r = script::resolve(&tf, "infra/main.tf:2:3-6").unwrap();
    assert_eq!((r.span.line_start, r.span.col_start, r.span.col_end), (2, 3, 6));
    assert!(script::resolve(&tf, "infra/main.tf:1-3").is_some());
    assert!(script::resolve(&tf, "infra/nope.tf:1-3").is_none());
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

#[test]
fn script_diagrams() {
    use iwr_core::script;
    let p = project();
    let text = "# T\n\n## Bus\n```mermaid\n---\ntitle: Bus\n---\ngraph LR\n  subgraph stop[🚏 Stop]\n    alice[🧍 Alice]\n  end\n  stop --> bus[(🚌 Bus)]\n```\n@ diagram\n! stop\nWaiting.\n! alice bus\nBoarding.\n? Who drives?\n  ! bus\n  Nobody.\n@ diagram:Bus\n! nope\nBad.\n@ code:crate::main\n! crate::main/b1\nBack to code.\n";
    let s = script::parse(text);
    let part = &s.parts[0];
    assert_eq!(part.diagrams.len(), 1);
    assert_eq!(part.diagrams[0].title.as_deref(), Some("Bus"));
    assert!(part.diagrams[0].errors.is_empty(), "{:?}", part.diagrams[0].errors);
    assert_eq!(part.cues.iter().map(|c| c.say.as_str()).collect::<Vec<_>>(), ["Waiting.", "Boarding.", "Bad.", "Back to code."]);
    assert_eq!(part.cues[1].questions.len(), 1);
    assert_eq!(script::parse(&script::to_text(&s)), s);
    let bad = script::check(&p, &s);
    assert_eq!(bad.len(), 1, "{:?}", bad);
    assert!(bad[0].contains("`nope` is not a node"), "{}", bad[0]);
    assert!(script::find_diagram(part, Some("2")).is_none() && script::find_diagram(part, Some("bus")).is_some());
    // `@ diagram` without a block; a block with a bad arrow
    let bad = script::check(&p, &script::parse("## X\n@ diagram\nhi\n"));
    assert_eq!(bad.len(), 1, "{:?}", bad);
    assert!(bad[0].contains("no ```mermaid block"));
    let bad = script::check(&p, &script::parse("## X\n```mermaid\ngraph LR\na -> b\n```\n@ diagram\n! a\nhi\n"));
    assert_eq!(bad.len(), 1, "{:?}", bad);
    assert!(bad[0].contains("diagram 1: line 2"), "{}", bad[0]);
    let g = iwr_core::diagram::graph(&part.diagrams[0], &[]);
    assert_eq!(g.nodes.len(), 3);
    assert_eq!(g.mode, Some(iwr_core::views::Mode::Diagram));
}

#[test]
fn script_diagram_moves() {
    use iwr_core::diagram::Op;
    use iwr_core::script;
    let p = project();
    let text = "## Bus\n```mermaid\ngraph LR\nsubgraph stop[Stop]\n alice[Alice]\nend\nstop --> bus[Bus]\n```\n@ diagram\n> alice bus\n! alice\nAlice boards.\n> alice\n> ghost bus\n> a b c\nOff again.\n";
    let s = script::parse(text);
    let c = &s.parts[0].cues;
    assert_eq!(c[0].ops, vec![Op::Move { node: "alice".into(), to: Some("bus".into()) }]);
    assert_eq!(c[1].ops.len(), 3);
    assert_eq!(script::parse(&script::to_text(&s)), s);
    let bad = script::check(&p, &s);
    assert_eq!(bad.len(), 2, "{:?}", bad);
    assert!(bad[0].contains("`ghost` in `>`") && bad[1].contains("cannot read `> a b c`"), "{:?}", bad);
    let bad = script::check(&p, &script::parse("## X\n> a b\nhi\n"));
    assert_eq!(bad.len(), 1, "{:?}", bad);
    assert!(bad[0].contains("no diagram"));
    // hidden nodes: shown and hidden by `>`, names checked
    let text = "## Bus\n```mermaid\ngraph LR\nalice[Alice]\ncopy[Copy]:::hidden\n```\n@ diagram\n> +copy -alice\nSwap.\n> +ghost\nOops.\n";
    let s = script::parse(text);
    assert_eq!(s.parts[0].cues[0].ops, vec![Op::Show("copy".into()), Op::Hide("alice".into())]);
    assert_eq!(script::parse(&script::to_text(&s)), s);
    let bad = script::check(&p, &s);
    assert_eq!(bad.len(), 1, "{:?}", bad);
    assert!(bad[0].contains("`ghost` in `>`"));
    let g = iwr_core::diagram::graph(&s.parts[0].diagrams[0], &s.parts[0].cues[0].ops);
    assert!(g.node("copy").is_some() && g.node("alice").is_none());
}

#[test]
fn script_plan_page() {
    use iwr_core::script::{self, PartFile};
    let p = project();
    let parts = vec![
        PartFile { stem: "01-welcome".into(), text: "## Welcome\n@ plan\nhi\n".into(), audio: None, times: vec![], voices: vec![] },
        PartFile { stem: "02-data".into(), text: "## Data\nplan: the types, by hand\n@ types\ndata\n".into(), audio: None, times: vec![], voices: vec![] },
        PartFile { stem: "03-end".into(), text: "## End\nbye\n".into(), audio: None, times: vec![], voices: vec![] },
    ];
    let index = "# T\nplan: a short tour\n\n- [Welcome](01-welcome.md) — where we go\n- `02-data.md`: overridden by the file\n- 03-end.md\n";
    assert_eq!(script::index_parts(index, &["01-welcome".into(), "02-data".into(), "03-end".into()]).len(), 3);
    let s = script::parse(&script::assemble(index, &parts));
    assert_eq!(s.summary.as_deref(), Some("a short tour"));
    assert_eq!(s.parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Welcome", "Data", "End"]);
    let sums: Vec<Option<&str>> = s.parts.iter().map(|p| p.summary.as_deref()).collect();
    assert_eq!(sums, [Some("where we go"), Some("the types, by hand"), None]);
    assert_eq!(script::parse(&script::to_text(&s)), s);
    assert!(script::check(&p, &s).is_empty(), "{:?}", script::check(&p, &s));
    let bad = script::check(&p, &script::parse("## X\n@ plan:nope\nhi\n"));
    assert_eq!(bad.len(), 1, "{:?}", bad);
}

#[test]
fn script_directory_assembly_and_part_audio() {
    use iwr_core::script::{self, PartFile};
    // per-part audio and a second `#` heading (one per part file) are tolerated
    let s = script::parse("# T\n\n## One\naudio: one.mp3\n[0.5] a\n\n# ignored\n## Two\nb\n");
    assert_eq!(s.parts[0].audio.as_deref(), Some("one.mp3"));
    assert_eq!(s.parts[1].audio, None);
    assert_eq!(s.parts[1].cues[0].say, "b");
    assert_eq!(script::parse(&script::to_text(&s)), s);
    assert_eq!(script::part_name_from_stem("02-how_it-runs"), "How it runs");
    assert_eq!(script::part_name_from_stem("welcome"), "Welcome");
    // directory: index lists the order; unlisted files follow by name; sibling audio is picked up
    let parts = vec![
        PartFile { stem: "01-welcome".into(), text: "# Part file title\n## Welcome\n@ arch\nhi\n".into(), audio: Some("01-welcome.mp3".into()), times: vec![], voices: vec![] },
        PartFile { stem: "02-data".into(), text: "@ types\ndata\n".into(), audio: None, times: vec![], voices: vec![] },
        PartFile { stem: "03-end".into(), text: "## The end\naudio: custom.ogg\nbye\n".into(), audio: Some("03-end.mp3".into()), times: vec![], voices: vec![] },
    ];
    let index = "# Tour\n\n- [The end](03-end.md)\n- 01-welcome.md\n";
    let text = script::assemble(index, &parts);
    let s = script::parse(&text);
    assert_eq!(s.title, "Tour");
    assert_eq!(s.parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["The end", "Welcome", "Data"]);
    assert_eq!(s.parts[0].audio.as_deref(), Some("custom.ogg"));
    assert_eq!(s.parts[1].audio.as_deref(), Some("01-welcome.mp3"));
    assert_eq!(s.parts[2].audio, None);
    assert_eq!(s.parts[1].cues[0].show.as_deref(), Some("arch"));
    assert_eq!(s.cue_count(), 3);
    // the part list does not become cues
    assert!(!text.contains("03-end.md"));
    // `iwr voice` times: given to the cues in order, a cue's own `[t]` wins
    let parts = vec![PartFile { stem: "01-a".into(), text: "## A\none\n[9] two\nthree\n".into(), audio: Some("01-a.mp3".into()), times: vec![0.0, 2.5, 5.25], voices: vec![] }];
    let s = script::parse(&script::assemble("# T\n", &parts));
    assert_eq!(s.parts[0].cues.iter().map(|c| c.t).collect::<Vec<_>>(), [Some(0.0), Some(9.0), Some(5.25)]);
}

#[test]
fn structure_hides_kinds() {
    let p = project();
    let all = views::build(&p, Mode::Structure, &ViewOptions::default());
    let has = |g: &views::Graph, label: &str| g.nodes.iter().any(|n| n.label.contains(label));
    assert!(has(&all, "Square") && has(&all, "double"));
    let mut opts = ViewOptions::default();
    opts.hidden_kinds = [ItemKind::Struct, ItemKind::Trait, ItemKind::Impl].into_iter().collect();
    let g = views::build(&p, Mode::Structure, &opts);
    assert!(has(&g, "double") && !g.nodes.iter().any(|n| matches!(n.kind, views::NodeKind::Struct | views::NodeKind::Trait | views::NodeKind::Impl)));
    // the impl block is hidden, so its method shows at module level
    assert!(g.nodes.iter().any(|n| n.kind == views::NodeKind::Method && n.depth == 1), "{:?}", g.nodes.iter().map(|n| (&n.label, n.depth)).collect::<Vec<_>>());
    opts.hidden_kinds.insert(ItemKind::Method);
    let g = views::build(&p, Mode::Structure, &opts);
    assert!(!g.nodes.iter().any(|n| n.kind == views::NodeKind::Method));
}

#[test]
fn script_questions_and_glossary() {
    use iwr_core::glossary;
    use iwr_core::script;
    let p = project();
    let text = "# T\nglossary: off\n\n## P\n@ arch\nThe parser is a module.\n? What is a module?\n  @ arch\n  ! crate::main\n  A box of code.\n  Second line.\n! crate::main\n? Before the next cue?\n  Yes.\nSecond cue.\n";
    let s = script::parse(text);
    assert!(!s.glossary);
    let cues = &s.parts[0].cues;
    assert_eq!(cues.len(), 2);
    // a `?` right after a cue belongs to that cue; one after a directive belongs to the next cue
    assert_eq!(cues[0].questions.len(), 1);
    assert_eq!(cues[0].questions[0].ask, "What is a module?");
    assert_eq!(cues[0].questions[0].answer, "A box of code.\nSecond line.");
    assert_eq!(cues[0].questions[0].hl, Some(vec!["crate::main".to_string()]));
    assert_eq!(cues[1].questions[0].ask, "Before the next cue?");
    assert_eq!(cues[1].hl, Some(vec!["crate::main".to_string()]));
    assert_eq!(script::parse(&script::to_text(&s)), s);
    assert!(script::check(&p, &s).is_empty(), "{:?}", script::check(&p, &s));
    let bad = script::check(&p, &script::parse("## P\nHi.\n? Empty?\n! crate::nope\nNext.\n"));
    assert!(bad.iter().any(|b| b.contains("without an answer")), "{:?}", bad);

    let g = glossary::builtin();
    assert!(g.len() > 20);
    assert!(g.iter().all(|e| !e.ask.is_empty() && !e.answer.is_empty()));
    let hits: Vec<&str> = glossary::mentioned(&g, "The parser is a module inside this crate, and `?` is the question mark.").iter().map(|e| e.term.as_str()).collect();
    assert_eq!(hits, vec!["module", "crate", "question mark"]);
    // whole words only, plural tolerated, case-insensitive
    assert!(glossary::mentioned(&g, "Crates everywhere").iter().any(|e| e.term == "crate"));
    assert!(glossary::mentioned(&g, "the implementation").iter().all(|e| e.term != "impl"));
    // type names are case-sensitive: an English "box" is not `Box`, "an option" is not `Option`
    assert!(glossary::mentioned(&g, "Every box is a module, you have an option.").iter().all(|e| e.term != "Box" && e.term != "Option"));
    assert!(glossary::mentioned(&g, "It returns an Option, or a Box<dyn Store>.").iter().any(|e| e.term == "Option"));
    let qs = glossary::questions_for(&cues[0], &g, &["module".into()], 3);
    assert_eq!(qs.len(), 1, "the script's own question stays; the asked glossary term is not offered again");
    let own = glossary::parse("# G\n## widget\nask: What is a widget?\nA thing.\n## crate\nOur crate.\n");
    let merged = glossary::merge(&[g.clone(), own]);
    assert_eq!(merged.iter().find(|e| e.term == "crate").unwrap().answer, "Our crate.");
    assert!(merged.iter().any(|e| e.term == "widget"));
}

#[test]
fn speech_says_code_as_words() {
    use iwr_core::speech::{self, Segment};
    let pron = speech::builtin_pronounce();
    let say = |c: &str| speech::say_code(c, &pron);
    assert_eq!(say("crate::parser::parse_all"), "parse all");
    assert_eq!(say("Task::weight"), "Task weight");
    assert_eq!(say("MemoryStorage"), "Memory Storage");
    assert_eq!(say("INPUT"), "INPUT");
    assert_eq!(say("t.clone()"), "t dot clone");
    assert_eq!(say("storage.save(t.clone())?"), "storage dot save of t dot clone, question mark");
    assert_eq!(say("Result<usize, AppError>"), "Result of u size and App Error");
    assert_eq!(say("&tasks"), "a reference to tasks");
    assert_eq!(say("&mut x"), "a mutable reference to x");
    assert_eq!(say("&str"), "string slice");
    assert_eq!(say("|t| t.weight()"), "the closure taking t, t dot weight");
    assert_eq!(say("println!(\"{}\", x)"), "print line");
    assert_eq!(say("#[derive(Debug, Clone)]"), "the attribute derive of Debug and Clone");
    assert_eq!(say("0..task.steps"), "0 up to task dot steps");
    assert_eq!(say("if task.priority == Priority::Low && count > 2"), "if task dot priority equals Priority Low and count is greater than 2");
    assert_eq!(say("'a"), "lifetime a");
    assert_eq!(say("u32"), "u 32");
    // pronounce overrides win
    let own = vec![("Dioxus".to_string(), "dee ox us".to_string())];
    assert_eq!(speech::say_code("Dioxus", &own), "dee ox us");
    assert_eq!(speech::say_code("gw/data/battery/<node>", &own), "gw slash data slash battery slash node");
    assert_eq!(speech::say_code("/dev/gw-device/rebutton-pad-<n>", &own), "dev slash gw device slash rebutton pad n");
    assert_eq!(speech::say_code("024d_other002_<serial_number>", &own), "024d other002 serial number");
    assert_eq!(speech::say_code("a/b", &own), "a over b");
    assert_eq!(speech::say_code("Vec<T>", &own), speech::say_code("Vec<T>", &vec![]));
    assert_eq!(speech::parse_pronounce("# T\npronounce: Dioxus = dee ox us\npronounce: bad\n"), own);
    // spelled acronyms, plural included; a later entry beats the built-in one.
    // TCP and UDP are spelled by the built-in list (speech::BUILTIN_SPELL), CoAP needs the line.
    let mut sp = pron.clone();
    sp.extend(speech::parse_pronounce("spell: IP, CoAP HTTP\nspell: TTS"));
    assert_eq!(speech::spoken("CoAP runs over UDP, HTTP over TCP; IPs and TTS.", &sp), "C O A P runs over U D P, H T T P over T C P; I P's and T T S.");
    assert_eq!(speech::spell("CoAP"), "C O A P");

    // bare words: code-looking ones become segments, English does not
    let segs = speech::segments("Storage is a trait. MemoryStorage saves with t.clone(). Wow! Really? parse_all?", &pron);
    let codes: Vec<&str> = segs.iter().filter_map(|s| if let Segment::Code { code, .. } = s { Some(code.as_str()) } else { None }).collect();
    assert_eq!(codes, vec!["MemoryStorage", "t.clone()", "parse_all"]);
    assert_eq!(speech::spoken("First, run hands the input to parse_all and gets tasks back.", &pron), "First, run hands the input to parse all and gets tasks back.");
    assert_eq!(speech::spoken("`for t in &tasks` walks the list.", &pron), "for t in a reference to tasks, walks the list.");
    assert_eq!(speech::spoken("The `?` after a call means: if it is `Ok`, carry on.", &pron), "The question mark after a call means: if it is Ok, carry on.");
    assert_eq!(speech::spoken("Lint has 1.5 steps, maybe 3.", &pron), "Lint has 1.5 steps, maybe 3.");
    assert!(speech::unspoken("parse_all and `a::b`", &pron).is_empty());
    assert!(!speech::looks_like_code("Rust") && !speech::looks_like_code("Hello!") && speech::looks_like_code("vec!") && speech::looks_like_code("HashMap"));
}

#[test]
fn architecture_hides_test_modules() {
    let p = iwr_core::analyze_sources(
        "demo",
        "/demo",
        vec![
            ("src/main.rs".into(), "mod net;\nfn main() {}\n#[cfg(test)]\nmod tests { #[test] fn t() {} }\n".into()),
            ("src/net.rs".into(), "pub fn send() {}\n#[cfg(not(test))]\nmod real { pub fn x() {} }\n".into()),
            ("tests/api.rs".into(), "#[test]\nfn api() {}\n".into()),
        ],
    );
    let labels = |show_tests: bool| -> Vec<String> { views::build(&p, Mode::Architecture, &ViewOptions { show_tests, ..Default::default() }).nodes.into_iter().map(|n| n.label).collect() };
    let hidden = labels(false);
    // `net::real` has no arrows: folded into `net`
    assert!(hidden.contains(&"net".to_string()) && !hidden.contains(&"real".to_string()), "{:?}", hidden);
    assert!(!hidden.iter().any(|l| l == "tests" || l == "api"), "{:?}", hidden);
    let shown = labels(true);
    assert!(shown.contains(&"tests".to_string()), "{:?}", shown);
}

#[test]
fn script_side_panel() {
    use iwr_core::script::{self, Side};
    let p = project();
    let text = "## Frame\n```bytes Header\nmagic 4 ascii Magic\nlen u16 Body length\nbody len The payload\nsample: 49 57 52 31 00 02 68 69\n```\n```md Steps\n- read the header\n  - check `magic`\n- read `len` bytes\n```\n@ code:crate::main\n| bytes:Header\n! crate::main/b1 len\nThe length comes first.\n| md\n! magic\nThen the steps.\n? And then?\n  | bytes\n  ! body\n  The body.\n|\nDone.\n| bytes:Nope\nBad.\n| a | b |\n";
    let s = script::parse(text);
    let part = &s.parts[0];
    assert_eq!((part.layouts.len(), part.notes.len()), (1, 1));
    assert!(part.layouts[0].errors.is_empty(), "{:?}", part.layouts[0].errors);
    assert_eq!(part.notes[0].text, "- read the header\n  - check `magic`\n- read `len` bytes");
    // a markdown table row is a cue, not a directive
    assert_eq!(part.cues.iter().map(|c| c.say.as_str()).collect::<Vec<_>>(), ["The length comes first.", "Then the steps.", "Done.", "Bad.", "| a | b |"]);
    assert_eq!(part.cues[1].questions[0].side.as_deref(), Some("bytes"));
    assert!(matches!(script::side_at(part, 0), Some(Side::Bytes(_))));
    assert!(matches!(script::side_at(part, 1), Some(Side::Note(_))));
    assert!(script::side_at(part, 2).is_none());
    assert_eq!(script::parse(&script::to_text(&s)), s);
    // `len` is a field of the panel; `magic` is not one of the note's (notes have no refs)
    let bad = script::check(&p, &s);
    assert_eq!(bad.len(), 2, "{:?}", bad);
    assert!(bad[0].contains("unresolved ref `magic`"), "{}", bad[0]);
    assert!(bad[1].contains("no byte layout `Nope`"), "{}", bad[1]);
}

#[test]
fn script_popups() {
    use iwr_core::script;
    let text = "## Request\n^ GET /config/wifi\n^ Answer: `200` with the wifi file\nThe gateway asks for one file.\nNo popup here.\n? Where is it kept?\n  ^ /data/config.db\n  In the config database.\n";
    let s = script::parse(text);
    let cues = &s.parts[0].cues;
    assert_eq!(cues[0].pop, ["GET /config/wifi", "Answer: `200` with the wifi file"]);
    // shown, never spoken
    assert_eq!(cues[0].say, "The gateway asks for one file.");
    assert!(cues[1].pop.is_empty());
    assert_eq!(cues[1].questions[0].pop, ["/data/config.db"]);
    assert_eq!(cues[1].questions[0].answer.trim(), "In the config database.");
    assert_eq!(script::parse(&script::to_text(&s)), s);
}

#[test]
fn script_hard_to_say() {
    use iwr_core::script::hard_to_say;
    assert_eq!(hard_to_say("It asks GET /config/wifi, then reads /srv/gw and src/main.rs."), ["GET /config/wifi", "/srv/gw", "src/main.rs"]);
    assert_eq!(hard_to_say("The gw-config-server sends a well-formed Wi-Fi file."), ["gw-config-server", "well-formed", "Wi-Fi"]);
    // a dash between words, numbers and URLs are fine
    assert!(hard_to_say("One file - one origin, at 1.5.0, see https://example.com/a/b.").is_empty());
}

#[test]
fn script_examples() {
    use iwr_core::script::{self, Side};
    let p = project();
    let text = "## Upload\n```json Request body\n{\n  \"user\": \"ann\",\n  \"tags\": [\"a\"]\n}\n```\n```csv\nid,email\n1,ann@x.io\n```\n@ code:crate::main\n| example:Request body\n! user\nThe client sends this.\n| csv\n! email\nOne row per user.\n| json:Nope\nBad.\n";
    let s = script::parse(text);
    let part = &s.parts[0];
    assert_eq!(part.examples.len(), 2);
    assert_eq!(part.examples[0].ids(), ["user", "tags"]);
    assert_eq!(part.examples[1].table().unwrap()[1], ["1", "ann@x.io"]);
    assert!(matches!(script::side_at(part, 0), Some(Side::Example(e)) if e.lang == "json"));
    assert!(matches!(script::side_at(part, 1), Some(Side::Example(e)) if e.lang == "csv"));
    assert_eq!(script::parse(&script::to_text(&s)), s);
    let bad = script::check(&p, &s);
    assert_eq!(bad.len(), 1, "{:?}", bad);
    assert!(bad[0].contains("no json example `Nope`"), "{}", bad[0]);
}

#[test]
fn cfg_comments_and_folded_loops() {
    let src = "fn largest(v: &[u32], stop: u32) -> Option<u32> {
    let mut best = None;
    // Find the next biggest value
    for x in v {
        // Hit the stop value
        if *x == stop {
            break;
        }
        if *x == 0 {
            return None;
        }
        best = Some(*x);
    }
    // This comment is far too long to be used as the label of a node in the graph, really
    best
}
";
    let p = iwr_core::analyze_source(src);
    let f = p.items.iter().find(|i| i.name == "largest").unwrap();
    let cfg = f.fn_info().unwrap().cfg.as_ref().unwrap();
    let lp = cfg.nodes.iter().find(|n| n.kind == CfgNodeKind::Loop).unwrap();
    assert_eq!(lp.note.as_deref(), Some("Find the next biggest value"));
    assert!(cfg.nodes.iter().all(|n| n.note.as_deref().map_or(true, |s| s.len() <= 60)));
    let brk = cfg.edges.iter().find(|e| e.kind == CfgEdgeKind::Break).unwrap();
    assert_eq!(brk.label.as_deref(), Some("Hit the stop value"));
    let ret = cfg.edges.iter().find(|e| e.kind == CfgEdgeKind::Return && e.label.as_deref().unwrap_or("").contains("→")).unwrap();
    assert_eq!(ret.label.as_deref(), Some("if * x == 0 → return None"));

    // folded by default: one node for the loop, labelled by the comment, exits kept
    let opts = ViewOptions { root: Some(f.id), ..Default::default() };
    let g = views::build(&p, Mode::ControlFlow, &opts);
    let entry = g.nodes.iter().find(|n| n.kind == views::NodeKind::Entry).unwrap();
    assert_eq!(entry.label, "largest");
    // Code view blocks carry the same comments; `} else if` lines never pick one up
    let blocks = f.fn_info().unwrap().blocks.as_ref().unwrap();
    let mut notes = Vec::new();
    fn walk(b: &iwr_core::model::Block, out: &mut Vec<(String, String)>) {
        if let Some(n) = &b.note {
            out.push((b.label.clone(), n.clone()));
        }
        b.children.iter().for_each(|c| walk(c, out));
    }
    walk(blocks, &mut notes);
    assert!(notes.iter().any(|(l, n)| l.starts_with("for x in v") && n == "Find the next biggest value"), "{notes:?}");
    assert!(notes.iter().any(|(l, n)| l.starts_with("if * x == stop") && n == "Hit the stop value"), "{notes:?}");
    let lnode = g.node(&format!("c{}", lp.id)).unwrap();
    assert_eq!(lnode.label, "Find the next biggest value");
    assert!(lnode.expandable && !lnode.expanded);
    assert!(g.nodes.iter().all(|n| n.kind != views::NodeKind::Jump));
    assert!(g.edges.iter().any(|e| e.from == lnode.id && e.label.as_deref() == Some("Hit the stop value")));
    assert!(g.edges.iter().all(|e| e.from != e.to));
    // unfolded: the body comes back
    let mut opts = opts;
    opts.expanded.insert(lnode.id.clone());
    let g2 = views::build(&p, Mode::ControlFlow, &opts);
    assert!(g2.nodes.len() > g.nodes.len());
    assert!(g2.nodes.iter().any(|n| n.label == "Hit the stop value"));
}

#[test]
fn script_architecture_tour() {
    let p = iwr_core::analyze_sources(
        "demo",
        "",
        vec![
            ("src/main.rs".into(), "//! Runs tasks.\nmod net;\nmod store;\nfn main() {}\n".into()),
            ("src/net.rs".into(), "//! Talks to the network.\n//!\n//! Long details here.\npub fn get() {}\n".into()),
            ("src/store.rs".into(), "pub fn put() {}\n".into()),
        ],
    );
    // architecture boxes: a short one-line `//!` is the label, the file goes underneath
    let g = views::build(&p, Mode::Architecture, &ViewOptions::default());
    let net = g.nodes.iter().find(|n| n.caption == "src/net.rs").unwrap();
    assert_eq!(net.label, "Talks to the network");
    let store = g.nodes.iter().find(|n| n.label == "store").unwrap();
    assert!(store.caption.is_empty());
    let root = g.nodes.iter().find(|n| n.label == "Runs tasks").unwrap();
    assert_eq!(root.caption, "crate demo · src/main.rs");
    assert_eq!(views::short_doc_line(Some("A sentence that goes on
over two lines.")), None);
    // rough edges: a TODO, unsafe, an unwrap
    let r = iwr_core::analyze_source("fn f(v: Option<u32>) -> u32 {\n    // TODO: retry later\n    let x = unsafe { g() };\n    v.unwrap() + x\n}\nunsafe fn g() -> u32 { 1 }\nfn main() { f(None); }\n");
    let rough = iwr_core::tours::generate(&r, "todo_tour").unwrap().cues;
    let says: Vec<&str> = rough.iter().map(|c| c.say.as_str()).collect();
    assert!(says[0].contains("1 note") && says[0].contains("2 unsafe blocks") && says[0].contains("1 place"), "{says:?}");
    assert!(says.iter().any(|s| *s == "In `f`, a TODO: retry later"), "{says:?}");
    assert!(says.iter().any(|s| s.contains("`v.unwrap() + x` can panic") || s.contains("unwrap") && s.contains("can panic")), "{says:?}");
    let text = "# T\n\n## Map\n@ arch\nHello.\narchitecture_tour\n? Why?\n  Because.\nBye.\n";
    let s = iwr_core::script::parse(text);
    assert_eq!(s.parts[0].cues.len(), 3);
    assert_eq!(s.parts[0].cues[1].tour.as_deref(), Some("architecture_tour"));
    assert_eq!(iwr_core::script::parse(&iwr_core::script::to_text(&s)), s);
    assert!(iwr_core::script::check(&p, &s).is_empty(), "{:?}", iwr_core::script::check(&p, &s));
    let mut e = s.clone();
    iwr_core::script::expand_tours(&p, &mut e);
    let cues = &e.parts[0].cues;
    // hello, intro, net, store, bye
    assert_eq!(cues.len(), 5);
    assert!(cues[1].say.contains("Runs tasks.") && cues[1].show.as_deref() == Some("arch"));
    assert_eq!(cues[1].questions.len(), 1);
    assert!(cues[2].say.contains("`net` module. Talks to the network.") && !cues[2].say.contains("Long details"));
    assert_eq!(cues[2].hl.as_deref(), Some(&["crate::net".to_string()][..]));
    assert!(cues[3].say.contains("no module doc yet"));
    let bad = iwr_core::script::check(&p, &iwr_core::script::parse("## X\nfoo_tour\n"));
    assert!(bad.iter().any(|b| b.contains("unknown tour `foo_tour`")));
}

#[test]
fn every_tour_builds_and_checks_on_the_demo() {
    let demo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/demo");
    let p = iwr_core::analyze_path(&demo).unwrap();
    for line in ["architecture_tour", "types_tour", "errors_tour", "entry_tour", "flow_tour:crate::run", "todo_tour", "files_tour", "types_tour:crate::storage", "errors_tour:crate::parser"] {
        let cues = iwr_core::tours::generate(&p, line).unwrap_or_else(|e| panic!("{line}: {e}")).cues;
        assert!(cues.len() >= if line == "todo_tour" { 1 } else { 2 }, "{line}: {} cues", cues.len());
        assert!(cues[0].show.is_some(), "{line}: first cue sets no view");
        assert!(cues.iter().all(|c| !c.say.trim().is_empty()), "{line}: empty cue");
        let s = iwr_core::script::parse(&format!("## T\n{line}\n"));
        let bad = iwr_core::script::check(&p, &s);
        assert!(bad.is_empty(), "{line}: {bad:?}");
    }
    let s = tutorial(&p);
    assert!(iwr_core::script::check(&p, &s).is_empty(), "tutorial: {:?}", iwr_core::script::check(&p, &s));
    for (line, err) in [("flow_tour", "needs a function"), ("flow_tour:crate::nope", "not a function"), ("types_tour:crate::nope", "not a module"), ("foo_tour", "unknown tour")] {
        let e = iwr_core::tours::generate(&p, line).unwrap_err();
        assert!(e.contains(err), "{line}: {e}");
    }
}

#[test]
fn cfg_hides_logs_and_names_statement_kinds() {
    let src = "fn f(v: &mut Vec<u32>, k: u32) -> Result<u32, String> {
    log::info!(\"start\");
    debug!(\"k = {}\", k);
    // Tell the operator
    log::warn!(\"careful\");
    // Pick the mode
    let mode = match k {
        0 => 1,
        _ => 2,
    };
    let total = k + mode;
    v.push(total);
    println!(\"{}\", total);
    if total > 9 {
        return Err(\"too big\".into());
    }
    Ok(total)
}
";
    let p = iwr_core::analyze_source(src);
    let f = p.items.iter().find(|i| i.name == "f").unwrap();
    let cfg = f.fn_info().unwrap().cfg.as_ref().unwrap();
    let kinds: Vec<CfgNodeKind> = cfg.nodes.iter().map(|n| n.kind).collect();
    // only the commented log stays
    let logs: Vec<&CfgNode> = cfg.nodes.iter().filter(|n| n.kind == CfgNodeKind::Log).collect();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].note.as_deref(), Some("Tell the operator"));
    assert!(cfg.nodes.iter().all(|n| !n.detail.contains("start") && !n.detail.contains("k =")));
    // `let mode = match k` is one node carrying the comment, arms leave from it
    let m = cfg.nodes.iter().find(|n| n.kind == CfgNodeKind::Match).unwrap();
    assert_eq!(m.label, "let mode = match k");
    assert_eq!(m.note.as_deref(), Some("Pick the mode"));
    assert_eq!(cfg.edges.iter().filter(|e| e.from == m.id && e.kind == CfgEdgeKind::Arm).count(), 2);
    assert!(!cfg.nodes.iter().any(|n| n.label == "let mode ="));
    for k in [CfgNodeKind::Let, CfgNodeKind::Assign, CfgNodeKind::Io, CfgNodeKind::ReturnErr] {
        assert!(kinds.contains(&k), "{k:?} missing: {kinds:?}");
    }
}

#[test]
fn architecture_hubs_get_chips() {
    // six modules all using `model`: it becomes a hub, its arrows hidden behind chips
    let mut files: Vec<(String, String)> = vec![("src/main.rs".into(), "mod model; mod a; mod b; mod c; mod d; mod e;\nfn main() {}\n".into()), ("src/model.rs".into(), "pub struct T;\n".into())];
    for m in ["a", "b", "c", "d", "e"] {
        files.push((format!("src/{m}.rs"), format!("use crate::model::T;\npub fn {m}(t: T) -> T {{ t }}\n")));
    }
    files.push(("src/a.rs".into(), "use crate::model::T;\npub fn a(t: T) -> T { crate::b::b(t) }\n".into()));
    files.remove(2);
    let p = iwr_core::analyze_sources("hub", "", files);
    let g = views::build(&p, Mode::Architecture, &ViewOptions::default());
    let model = g.nodes.iter().find(|n| n.label == "model").unwrap();
    assert_eq!(model.chips.len(), 1, "{:?}", model.chips);
    assert_eq!(model.chips[0].node, model.id);
    assert_eq!(model.chips[0].text, "MO");
    let users = g.nodes.iter().filter(|n| n.chips.iter().any(|c| c.node == model.id && n.id != model.id)).count();
    assert_eq!(users, 5);
    assert!(g.edges.iter().filter(|e| e.to == model.id).all(|e| e.hub));
    // a → b is a plain arrow; module tree edges are gone
    let a = g.nodes.iter().find(|n| n.label == "a").unwrap();
    let b = g.nodes.iter().find(|n| n.label == "b").unwrap();
    assert!(g.edges.iter().any(|e| e.from == a.id && e.to == b.id && !e.hub && e.weight >= 1));
    assert!(g.edges.iter().all(|e| e.kind != views::EdgeKind::Contains));
    // users sit above what they use
    assert!(a.y < b.y && b.y < model.y);
}

#[test]
fn architecture_folds_arrowless_subtrees() {
    // `proto` holds a tree of generated modules nobody calls into: one box, "+3 inside"
    let p = iwr_core::analyze_sources(
        "fold",
        "",
        vec![
            ("src/main.rs".into(), "mod proto;\nmod net;\nfn main() { net::go(); }\n".into()),
            ("src/net.rs".into(), "pub fn go() {}\n".into()),
            ("src/proto.rs".into(), "pub mod a { pub mod b { pub struct B; } pub mod c { pub struct C; } }\n".into()),
        ],
    );
    let g = views::build(&p, Mode::Architecture, &ViewOptions::default());
    let proto = g.nodes.iter().find(|n| n.label == "proto").unwrap();
    assert!(proto.badges.iter().any(|b| b == "+3 inside"), "{:?}", proto.badges);
    assert!(!g.nodes.iter().any(|n| n.label == "b" || n.label == "a"));
    assert!(g.nodes.iter().any(|n| n.label == "net"));
}

#[test]
fn architecture_folds_crates_of_a_big_workspace() {
    // two crates with 30 modules each: too many boxes, so one box per crate and one arrow between them
    let mut files = Vec::new();
    for c in ["app", "lib"] {
        let mods: String = (0..30).map(|i| format!("pub mod m{i};\n")).collect();
        files.push((format!("crates/{c}/src/lib.rs"), mods));
        for i in 0..30 {
            let body = if c == "app" { format!("pub fn f{i}() {{ lib::m{i}::g{i}(); }}\n") } else { format!("pub fn g{i}() {{}}\n") };
            files.push((format!("crates/{c}/src/m{i}.rs"), body));
        }
    }
    let p = iwr_core::analyze_sources("ws", "", files);
    let g = views::build(&p, Mode::Architecture, &ViewOptions::default());
    let mods: Vec<&views::VNode> = g.nodes.iter().filter(|n| n.kind == views::NodeKind::Module).collect();
    assert!(mods.len() <= 3, "{:?}", mods.iter().map(|n| &n.label).collect::<Vec<_>>());
    let app = mods.iter().find(|n| n.label.contains("app")).unwrap();
    let lib = mods.iter().find(|n| n.label.contains("lib")).unwrap();
    assert!(app.badges.iter().any(|b| b == "+30 inside"), "{:?}", app.badges);
    assert!(app.sublabel.starts_with("30 fn"), "{}", app.sublabel);
    let e = g.edges.iter().find(|e| e.from == app.id && e.to == lib.id).unwrap();
    assert_eq!(e.weight, 30);
    // scoped into a crate, its modules are back
    let m = p.modules.iter().find(|m| m.path == "app").unwrap().id;
    let g = views::build(&p, Mode::Architecture, &ViewOptions { module: Some(m), ..Default::default() });
    assert!(g.nodes.iter().filter(|n| n.kind == views::NodeKind::Module).count() >= 30, "{:?}", g.nodes.iter().map(|n| (&n.label, &n.badges)).collect::<Vec<_>>());
}

#[test]
fn calls_do_not_jump_to_crates_never_named() {
    // `a` names `shared` but not `b`: `helper()` and `x.flush()` stay unresolved instead of landing in `b`
    let p = iwr_core::analyze_sources(
        "ws",
        "",
        vec![
            ("crates/a/src/main.rs".into(), "fn main() { helper(); shared::go(); let x = 1; x.flush(); }\n".into()),
            ("crates/b/src/lib.rs".into(), "pub fn helper() {}\npub struct W;\nimpl W { pub fn flush(&self) {} }\n".into()),
            ("crates/shared/src/lib.rs".into(), "pub fn go() {}\n".into()),
        ],
    );
    let main = p.items.iter().find(|i| i.name == "main").unwrap().id;
    let to: Vec<String> = p.calls.iter().filter(|c| c.from == main).filter_map(|c| c.to).map(|t| p.item(t).path.clone()).collect();
    assert_eq!(to, vec!["shared::go".to_string()], "{:?}", to);
}

#[test]
fn cfg_exit_stops_and_trivial_arms_go_on_edges() {
    let src = "fn main() {
    // Load config or write a default; exit 2 on error
    let cfg = match load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!(\"config: {e:#}\");
            std::process::exit(2);
        }
    };
    log::info!(\"config {}\", cfg);
    let n = std::env::args().nth(1).unwrap_or_else(|| std::process::exit(1));
    run(cfg, n);
}
fn load() -> Result<u32, String> { Ok(1) }
fn run(_a: u32, _b: String) {}
";
    let p = iwr_core::analyze_source(src);
    let cfg = p.item(p.main.unwrap()).fn_info().unwrap().cfg.as_ref().unwrap();
    let m = cfg.nodes.iter().find(|n| n.kind == CfgNodeKind::Match).unwrap();
    assert_eq!(m.note.as_deref(), Some("Load config or write a default; exit 2 on error"));
    // `Ok(c) => c`: no box, the arm edge says it
    assert!(cfg.edges.iter().any(|e| e.from == m.id && e.label.as_deref() == Some("Ok(c) → c")), "{:?}", cfg.edges);
    assert!(!cfg.nodes.iter().any(|n| n.label == "c"));
    // eprintln + exit: one node, a sentence, nothing after it but the exit
    let halts: Vec<&CfgNode> = cfg.nodes.iter().filter(|n| n.kind == CfgNodeKind::Halt).collect();
    assert_eq!(halts.len(), 1, "{:?}", cfg.nodes.iter().map(|n| (&n.label, n.kind)).collect::<Vec<_>>());
    assert_eq!(halts[0].label, "print error, exit with code 2");
    let out: Vec<&CfgEdge> = cfg.edges.iter().filter(|e| e.from == halts[0].id).collect();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].to, 1);
    assert_eq!(out[0].label.as_deref(), Some("exit 2"));
    // drawn: the halt ends in its own stub right below it, not at the function's end
    let g = views::build(&p, Mode::ControlFlow, &ViewOptions { root: p.main, ..Default::default() });
    let h = g.nodes.iter().find(|n| n.kind == views::NodeKind::Halt).unwrap();
    let outs: Vec<&views::VEdge> = g.edges.iter().filter(|e| e.from == h.id).collect();
    assert_eq!(outs.len(), 1);
    let stub = g.node(&outs[0].to).unwrap();
    assert!(stub.kind == views::NodeKind::Exit && stub.label == "exit 2" && stub.id != "c1");
    assert!(stub.y > h.y);
    let end = g.node("c1").unwrap();
    assert!(g.nodes.iter().filter(|n| n.id != "c1").all(|n| n.y + n.h <= end.y));
}

#[test]
fn types_view_labels_by_doc() {
    let p = iwr_core::analyze_source("/// A unit of work.\npub struct Task { pub name: String }\n/// Stores tasks somewhere, in memory or on disk, for as long as the run lasts.\npub trait Store { fn put(&self); }\npub enum Plain { A }\nfn main() {}\n");
    let g = views::build(&p, Mode::Types, &ViewOptions::default());
    let task = g.nodes.iter().find(|n| n.caption.ends_with(" Task")).unwrap();
    assert_eq!(task.label, "A unit of work");
    assert!(task.caption.starts_with("struct"), "{}", task.caption);
    // too long for a label, or no doc: the name stays
    assert!(g.nodes.iter().any(|n| n.label == "Store" && n.caption.is_empty()));
    assert!(g.nodes.iter().any(|n| n.label == "Plain" && n.caption.is_empty()));
}

#[test]
fn doc_pages_are_shown_and_read_section_by_section() {
    let src = "//! z-rebutton-handler — HTTP bridge for Seeed ReButton WiFi button
//!
//! Receives POST /api/v1/wirelessbutton from the ReButton hardware.
//!
//! ## Button value mapping  (zcc_node_util.cpp::ToDeviceValue)
//!
//! | button_input_type   | call_button value |
//! |---------------------|-------------------|
//! | INPUT_DOUBLE_CLICK  | 240 (ENTER)       |
//! | INPUT_NONE          | (skipped)         |
//!
//! ## MQTT topics published
//! | Topic                      | QoS | Retain |
//! |----------------------------|-----|--------|
//! | gw/data/battery/<node>     | 0   | false  |
//!
//! ## Rebutton pad
//! The USB cradle is served here too (`pad.rs`).

mod net {
    //! Talks to the network.
    pub fn send() {}
}
fn main() { net::send(); }
";
    let p = iwr_core::analyze_source(src);
    let stops = iwr_core::guide::doc_page("t", p.modules[0].doc.as_ref()).unwrap();
    assert_eq!(stops.len(), 4);
    assert!(stops[0].say.contains("HTTP bridge") && stops[0].say.contains("Receives POST"), "{}", stops[0].say);
    assert!(stops[0].note.text.contains("## MQTT topics published"));
    assert_eq!(stops[1].note.title.as_deref(), Some("t · Button value mapping  (zcc_node_util.cpp::ToDeviceValue)"));
    assert!(stops[1].note.text.starts_with("## Button value mapping") && stops[1].note.text.contains("\n| button_input_type"));
    assert!(stops[1].say.contains("INPUT_DOUBLE_CLICK: 240 (ENTER).") && stops[1].say.contains("INPUT_NONE: (skipped)."), "{}", stops[1].say);
    assert!(stops[2].say.contains("gw/data/battery/<node>: QoS 0, Retain false."), "{}", stops[2].say);
    assert_eq!(stops[3].say, "Rebutton pad. The USB cradle is served here too (`pad.rs`).");
    assert!(iwr_core::guide::doc_page("t", p.modules[1].doc.as_ref()).is_none());

    // the generated tour opens each page in the side panel and closes it after
    let mut s = iwr_core::script::parse("## Map\narchitecture_tour\nBye.\n");
    iwr_core::script::expand_tours(&p, &mut s);
    let part = &s.parts[0];
    assert_eq!(part.notes.len(), 4);
    let sides: Vec<Option<&str>> = part.cues.iter().map(|c| c.side.as_deref()).collect();
    assert!(sides[0].unwrap().starts_with("md:"), "{sides:?}");
    assert!(iwr_core::script::side_at(part, 1).is_some());
    let after = part.cues.iter().position(|c| c.say.starts_with("Now the modules")).unwrap();
    assert_eq!(sides[after], Some(""));
    assert!(iwr_core::script::check(&p, &s).is_empty(), "{:?}", iwr_core::script::check(&p, &s));

    // the built-in tour too
    let steps = iwr_core::guide::build(&p, None, &Default::default());
    let g = iwr_core::script::from_guide(&p, &steps);
    assert_eq!(g.parts[0].notes.len(), 4);
    assert!(iwr_core::script::check(&p, &g).is_empty(), "{:?}", iwr_core::script::check(&p, &g));
}

#[test]
fn cfg_groups_of_lets_get_a_short_title() {
    assert_eq!(views::target_names("let (a, mut b): (u8, u8) = (1, 2)"), vec!["a", "b"]);
    assert_eq!(views::target_names("let Some(x) = opt else { return }"), vec!["x"]);
    assert_eq!(views::target_names("self.count += 1"), vec!["count"]);
    assert_eq!(views::target_names("x <<= 2"), vec!["x"]);
    assert_eq!(views::target_names("self.items.push(v)"), vec!["items"]);
    let src = "fn main() {\n    let width = 3;\n    let height = 4;\n    let mut area = width * height;\n    let depth = 2;\n    let name = \"box\";\n    println!(\"{} {} {}\", area, depth, name);\n}\n";
    let p = iwr_core::analyze_source(src);
    let g = views::build(&p, Mode::ControlFlow, &ViewOptions { root: p.main, ..Default::default() });
    let lets = g.nodes.iter().find(|n| n.kind == views::NodeKind::Let).unwrap();
    assert_eq!(lets.label, "define width, height, area +2");
    assert!(lets.sublabel.starts_with("let width = 3"), "{}", lets.sublabel);
    assert!(lets.sublabel.contains("… 2 more"), "{}", lets.sublabel);
}

#[test]
fn cfg_unfolds_a_callee_in_place() {
    let src = "fn helper(x: u32) -> u32 {\n    if x > 2 { return 1; }\n    x + 1\n}\nfn main() {\n    let y = helper(3);\n    println!(\"{}\", y);\n}\n";
    let p = iwr_core::analyze_source(src);
    let helper = p.items.iter().find(|i| i.name == "helper").unwrap().id;
    let mut opts = ViewOptions { root: p.main, ..Default::default() };
    let g = views::build(&p, Mode::ControlFlow, &opts);
    let call = g.nodes.iter().find(|n| n.kind == views::NodeKind::Call).unwrap();
    assert!(call.expandable && !call.expanded);
    let after: Vec<String> = g.edges.iter().filter(|e| e.from == call.id).map(|e| e.to.clone()).collect();
    opts.expanded.insert(call.id.clone());
    let g2 = views::build(&p, Mode::ControlFlow, &opts);
    let inner = format!("{}/f{}/", call.id, helper);
    // call → callee's entry; callee's end → what came after the call
    assert!(g2.edges.iter().any(|e| e.from == call.id && e.to == format!("{}c0", inner) && e.kind == views::EdgeKind::Call));
    for to in &after {
        assert!(g2.edges.iter().any(|e| e.from == format!("{}c1", inner) && &e.to == to));
    }
    assert_eq!(g2.node(&format!("{}c0", inner)).unwrap().label, "helper");
    assert!(g2.nodes.iter().any(|n| n.kind == views::NodeKind::If && n.id.starts_with(&inner)));
    let frame = g2.node(&format!("{}frame", inner)).unwrap();
    assert!(frame.container);
    assert_eq!(views::cfg_owner(&format!("{}c2", inner)), Some(helper));
    assert_eq!(views::cfg_owner("c2"), None);
    // every edge ends on a node
    assert!(g2.edges.iter().all(|e| g2.node(&e.from).is_some() && g2.node(&e.to).is_some()));
}

#[test]
fn script_panels_and_folders() {
    use iwr_core::script::{self, Focus, Panel};
    let p = iwr_core::analyze_sources(
        "demo",
        "/demo",
        vec![
            ("src/main.rs".into(), "mod store;\nfn main() { if true { store::save::put(); } }\n".into()),
            ("src/store/mod.rs".into(), "pub mod save;\npub struct Db;\n".into()),
            ("src/store/save.rs".into(), "pub fn put() {}\n".into()),
            ("docs/notes.md".into(), "# notes\n".into()),
        ],
    );
    // `+` lines: sticky inside a part, `+` alone resets, other `+ …` lines are cues
    let s = script::parse("## A\n+ files\none\ntwo\n+ source\nthree\n+\nfour\n+ 1 is one\n+ search\nfive\n");
    let c = &s.parts[0].cues;
    assert_eq!(c.len(), 6);
    assert_eq!(c[4].say, "+ 1 is one");
    let at: Vec<Option<Focus>> = (0..6).map(|i| script::focus_at(&s.parts[0], i)).collect();
    assert_eq!(at, [Some(Focus::Panel(Panel::Files)), Some(Focus::Panel(Panel::Files)), Some(Focus::Panel(Panel::Source)), None, None, Some(Focus::Spot("search"))]);
    assert_eq!(script::focus_of("zoom"), Some(Some(Focus::Zoom(None))));
    assert_eq!(script::focus_of("zoom tree"), Some(Some(Focus::Zoom(Some("files")))));
    assert_eq!(script::focus_of("zoom player"), Some(Some(Focus::Zoom(Some("player")))));
    assert_eq!(script::focus_of("zoom nowhere"), None);
    assert_eq!(script::focus_of("zoom"), script::focus_of(" zoom "));
    assert_eq!(script::parse(&script::to_text(&s)), s);
    // folder refs: exact path, unique suffix, trailing slash; a file wins over a folder
    assert_eq!(script::resolve_dir(&p, "src/store/").as_deref(), Some("src/store"));
    assert_eq!(script::resolve_dir(&p, "store").as_deref(), Some("src/store"));
    assert_eq!(script::resolve_dir(&p, "docs").as_deref(), Some("docs"));
    assert!(script::resolve_dir(&p, "nope").is_none());
    assert!(script::resolve_dir(&p, "src/main.rs").is_none());
    assert!(script::check(&p, &script::parse("## A\n! src/store/ docs\nx\n")).is_empty());
    assert_eq!(script::check(&p, &script::parse("## A\n! src/nope/\nx\n")).len(), 1);
    // the tutorial codecast is valid on any project, its folder cue on this one
    for proj in [&p, &project()] {
        let s = tutorial(proj);
        assert_eq!(s.parts.len(), 8);
        assert!(script::check(proj, &s).is_empty(), "{:?}", script::check(proj, &s));
    }
    let s = tutorial(&p);
    let files = s.parts.iter().find(|x| x.name == "Files and folders").unwrap();
    assert_eq!(files.cues[1].hl, Some(vec!["src/store/".to_string()]));
    assert_eq!(script::focus_at(files, 0), Some(Focus::Panel(Panel::Files)));
}

/// The UI's tutorial codecast, filled for `p`.
fn tutorial(p: &Project) -> iwr_core::script::Script {
    use iwr_core::script::{self, PartFile};
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../iwr-ui/assets/tutorial");
    let mut stems: Vec<String> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()?.file_name().to_str()?.strip_suffix(".md").map(String::from)).filter(|s| s != "index").collect();
    stems.sort();
    let parts: Vec<PartFile> = stems.iter().map(|st| PartFile { stem: st.clone(), text: iwr_core::tutorial::fill(p, &std::fs::read_to_string(dir.join(format!("{st}.md"))).unwrap()), audio: None, times: vec![], voices: vec![] }).collect();
    script::parse(&script::assemble(&std::fs::read_to_string(dir.join("index.md")).unwrap(), &parts))
}
