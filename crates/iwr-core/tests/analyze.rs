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
    assert_eq!(s.parts[0].cues[0], Cue { say: "first".into(), show: Some("flow:crate::main".into()), hl: Some(vec!["crate::main/b1".into(), "src/main.rs:3-4".into()]), code: Some("src/main.rs:3:5-9".into()), t: Some(1.5), questions: vec![], ops: vec![] });
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
        PartFile { stem: "01-welcome".into(), text: "# Part file title\n## Welcome\n@ arch\nhi\n".into(), audio: Some("01-welcome.mp3".into()) },
        PartFile { stem: "02-data".into(), text: "@ types\ndata\n".into(), audio: None },
        PartFile { stem: "03-end".into(), text: "## The end\naudio: custom.ogg\nbye\n".into(), audio: Some("03-end.mp3".into()) },
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
    assert_eq!(speech::parse_pronounce("# T\npronounce: Dioxus = dee ox us\npronounce: bad\n"), own);

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
