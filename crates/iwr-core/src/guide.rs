//! Automatic tour of a program: a welcome and the architecture, the data types,
//! the call tree, then one part per function, walked statement by statement and
//! told in plain sentences. Each step tells the UI which view to show, what to
//! focus and what to say. `script::from_guide` turns the steps into a codecast.
//!
//! The tour stays on one function until it is fully explained; the functions it
//! calls are visited afterwards, in the order they were first mentioned (breadth
//! first), each one once. That keeps the camera still and the story linear.

use crate::model::*;
use crate::views::Mode;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuideStep {
    pub title: String,
    pub text: String,
    pub mode: Mode,
    /// Part (chapter) this step belongs to; consecutive steps with the same part are grouped.
    pub part: String,
    /// Function (or type/module item) in focus.
    pub focus_item: Option<ItemId>,
    /// CFG node inside `focus_item` in focus (for ControlFlow / BranchTree).
    pub focus_cfg: Option<usize>,
    /// Other items to highlight (call chain so far, related types...).
    pub highlight_items: Vec<ItemId>,
    /// Explicit script refs to highlight (module paths, item paths); used before `span`.
    pub refs: Vec<String>,
    pub span: Option<Span>,
    /// Call stack at this step (outermost first).
    pub stack: Vec<ItemId>,
    pub kind: StepKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    Overview,
    Enter,
    Statement,
    Branch,
    Loop,
    Call,
    Error,
    Return,
    Leave,
}

#[derive(Debug, Clone)]
pub struct GuideOptions {
    pub max_steps: usize,
    pub max_depth: usize,
    /// Skip tiny helper functions (getters etc.) when diving.
    pub min_body_lines: usize,
    /// How many modules / types the overview describes one by one.
    pub max_overview_items: usize,
}

impl Default for GuideOptions {
    fn default() -> Self {
        Self { max_steps: 300, max_depth: 6, min_body_lines: 0, max_overview_items: 10 }
    }
}

fn label(p: &Project, id: ItemId) -> String {
    let it = p.item(id);
    match it.fn_info().and_then(|f| f.owner_type_name.clone().or(f.trait_name.clone())) {
        Some(o) if it.kind == ItemKind::Method => format!("{}::{}", o, it.name),
        _ => it.name.clone(),
    }
}

/// First paragraph of a doc comment, as one line ending with a period.
fn doc_sentence(doc: Option<&String>) -> Option<String> {
    let d = doc?;
    let para: Vec<&str> = d.lines().map(str::trim).take_while(|l| !l.is_empty()).collect();
    if para.is_empty() {
        return None;
    }
    let mut s = para.join(" ");
    if !s.ends_with('.') && !s.ends_with('!') && !s.ends_with('?') {
        s.push('.');
    }
    Some(s)
}

/// "a, b and c"
fn join_and(items: &[String]) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        n => format!("{} and {}", items[..n - 1].join(", "), items[n - 1]),
    }
}

/// Sentence connector for the i-th of n steps inside a function.
fn connector(i: usize, n: usize) -> &'static str {
    if n > 1 && i + 1 == n {
        return "Finally,";
    }
    match i {
        0 => "First,",
        _ => ["Then", "Next,", "After that,", "Then", "Now"][i % 5],
    }
}

fn code_block(detail: &str) -> String {
    format!("```rust\n{}\n```\n", detail.trim())
}

struct Gen<'a> {
    p: &'a Project,
    opts: &'a GuideOptions,
    steps: Vec<GuideStep>,
    /// functions to visit, with the caller that mentioned them first and their depth
    queue: VecDeque<(ItemId, Option<ItemId>, usize)>,
    queued: HashSet<ItemId>,
    walked: HashSet<ItemId>,
}

impl<'a> Gen<'a> {
    fn push(&mut self, s: GuideStep) {
        if self.steps.len() < self.opts.max_steps {
            self.steps.push(s);
        }
    }

    fn overview(&mut self, part: &str, title: &str, text: String, mode: Mode, refs: Vec<String>, focus: Option<ItemId>) {
        let span = focus.map(|f| self.p.item(f).span);
        self.push(GuideStep { title: title.to_string(), text, mode, part: part.to_string(), focus_item: focus, focus_cfg: None, highlight_items: focus.into_iter().collect(), refs, span, stack: vec![], kind: StepKind::Overview });
    }

    /// Queue a callee for a later part; returns how to refer to it in the current sentence.
    fn mention(&mut self, from: ItemId, to: ItemId, depth: usize) -> &'static str {
        if to == from {
            return "itself";
        }
        if self.walked.contains(&to) {
            return "seen";
        }
        if self.queued.contains(&to) {
            return "later";
        }
        let it = self.p.item(to);
        let Some(f) = it.fn_info() else { return "opaque" };
        if f.cfg.is_none() {
            return "trait";
        }
        if depth + 1 >= self.opts.max_depth || f.body_lines < self.opts.min_body_lines {
            return "skip";
        }
        self.queued.insert(to);
        self.queue.push_back((to, Some(from), depth + 1));
        "later"
    }

    fn walk_fn(&mut self, id: ItemId, caller: Option<ItemId>, depth: usize) {
        let p = self.p;
        let it = p.item(id);
        let Some(f) = it.fn_info() else { return };
        let name = label(p, id);
        let part = format!("{}()", name);
        self.walked.insert(id);

        // ---- enter
        let mut intro = match caller {
            None if f.is_main => format!("Let's start at the top, in `{}`, the entry point of the program.", name),
            None => format!("Let's start with `{}`.", name),
            Some(c) => {
                let how = p.calls.iter().find(|e| e.from == c && e.to == Some(id)).map(|e| {
                    if e.in_loop {
                        " inside a loop"
                    } else if e.propagated {
                        " with a question mark, so its errors bubble up"
                    } else if e.in_branch {
                        " on one of its branches"
                    } else {
                        ""
                    }
                }).unwrap_or("");
                format!("Now let's take a closer look at `{}`, which `{}` calls{}.", name, label(p, c), how)
            }
        };
        if let Some(d) = doc_sentence(it.doc.as_ref()) {
            intro.push(' ');
            intro.push_str(&d);
        }
        let mut facts: Vec<String> = Vec::new();
        let params: Vec<String> = f.params.iter().filter(|a| a.name != "self").map(|a| format!("`{}`", a.name)).collect();
        let takes = if params.is_empty() { String::new() } else { format!("takes {} and ", join_and(&params)) };
        let gives = match (&f.ret, f.returns_result, f.returns_option) {
            (None, _, _) => "returns nothing".to_string(),
            (Some(_), true, _) => format!("returns a `Result`: either `Ok` with {}, or an `Err` of `{}` that the caller has to deal with", f.result_ok.as_deref().map(|o| format!("`{}`", o)).unwrap_or_else(|| "a value".into()), f.result_err.as_deref().unwrap_or("_")),
            (Some(r), _, true) => format!("returns `{}`, so there may be no value at all", r),
            (Some(r), _, _) => format!("returns `{}`", r),
        };
        facts.push(format!("It {}{}.", takes, gives));
        if f.is_async {
            facts.push("It is async, so it only runs when something awaits it.".into());
        }
        if f.is_recursive {
            facts.push("It calls itself: this is recursion, and we will see the base case that stops it.".into());
        }
        let mut shape: Vec<String> = Vec::new();
        if f.loops > 0 {
            shape.push(format!("{} loop{}", f.loops, if f.loops > 1 { "s" } else { "" }));
        }
        if f.branches > 0 {
            shape.push(format!("{} decision{}", f.branches, if f.branches > 1 { "s" } else { "" }));
        }
        if f.question_marks > 0 {
            shape.push(format!("{} place{} where an error can escape", f.question_marks, if f.question_marks > 1 { "s" } else { "" }));
        }
        if !shape.is_empty() {
            let verb = if shape.len() == 1 && shape[0].starts_with("1 ") { "is" } else { "are" };
            facts.push(format!("Inside there {} {}.", verb, join_and(&shape)));
        }
        if let Some(o) = &f.owner_type_name {
            facts.push(format!("It is a method of `{}`.", o));
        }
        let text = format!("{}\n\n{}", intro, facts.join(" "));
        self.push(GuideStep {
            title: format!("{} {}()", if caller.is_none() { "Start at" } else { "Enter" }, name),
            text,
            mode: Mode::ControlFlow,
            part: part.clone(),
            focus_item: Some(id),
            focus_cfg: Some(0),
            highlight_items: vec![id],
            refs: vec![],
            span: Some(it.span),
            stack: vec![id],
            kind: StepKind::Enter,
        });
        let Some(cfg) = f.cfg.as_ref() else { return };

        // ---- body, in reading order: DFS preferring the "main" successor
        let mut visited: HashSet<usize> = HashSet::new();
        let mut order: Vec<usize> = Vec::new();
        fn dfs(cfg: &Cfg, n: usize, visited: &mut HashSet<usize>, order: &mut Vec<usize>) {
            if !visited.insert(n) {
                return;
            }
            order.push(n);
            let mut succ: Vec<&CfgEdge> = cfg.succ(n).collect();
            succ.sort_by_key(|e| match e.kind {
                CfgEdgeKind::True | CfgEdgeKind::LoopBody => 0,
                CfgEdgeKind::Arm => 1,
                CfgEdgeKind::Next => 2,
                CfgEdgeKind::False => 3,
                CfgEdgeKind::Break | CfgEdgeKind::Continue | CfgEdgeKind::LoopBack => 4,
                CfgEdgeKind::Error | CfgEdgeKind::Return => 5,
            });
            for e in succ {
                if e.to == 1 {
                    continue; // exit node last
                }
                dfs(cfg, e.to, visited, order);
            }
        }
        dfs(cfg, 0, &mut visited, &mut order);
        let body: Vec<usize> = order.into_iter().filter(|n| *n != 0 && !matches!(cfg.nodes[*n].kind, CfgNodeKind::Entry | CfgNodeKind::Exit)).collect();
        let n_body = body.len();
        for (i, n) in body.iter().enumerate() {
            self.cfg_step(id, &part, cfg, *n, connector(i, n_body), depth);
            if self.steps.len() >= self.opts.max_steps {
                break;
            }
        }

        // ---- leave
        let ret = match (&f.ret, f.returns_result, f.returns_option) {
            (None, _, _) => "it returns nothing".to_string(),
            (Some(_), true, _) => "it hands back either `Ok` with the value, or an `Err` for the caller to handle".to_string(),
            (Some(r), _, true) => format!("it returns `{}`: `Some` with a value, or `None`", r),
            (Some(r), _, _) => format!("it returns `{}`", r),
        };
        let next = match self.queue.front() {
            Some((nid, Some(c), _)) if *c == id => format!("Next, let's open `{}`, which we just saw it call.", label(p, *nid)),
            Some((nid, _, _)) => format!("Next, let's open `{}`.", label(p, *nid)),
            None if caller.is_none() => "That was the whole program. Thanks for following along.".to_string(),
            None => "And that is the end of the tour: every function on the path from the entry point has been visited. Thanks for following along.".to_string(),
        };
        self.push(GuideStep {
            title: format!("Leave {}()", name),
            text: format!("That is all of `{}`: {}. {}", name, ret, next),
            mode: Mode::ControlFlow,
            part,
            focus_item: Some(id),
            focus_cfg: Some(1),
            highlight_items: vec![id],
            refs: vec![],
            span: Some(Span { line_start: it.span.line_end, line_end: it.span.line_end, ..it.span }),
            stack: vec![id],
            kind: StepKind::Leave,
        });
    }

    fn cfg_step(&mut self, fn_id: ItemId, part: &str, cfg: &Cfg, n: usize, conn: &str, depth: usize) {
        let p = self.p;
        let node = &cfg.nodes[n];
        let succ: Vec<&CfgEdge> = cfg.succ(n).collect();
        let fname = label(p, fn_id);
        let short = crate::shorten(&node.detail, 70);
        // what we say about the callees of this node
        let mut call_notes: Vec<String> = Vec::new();
        let mut call_names: Vec<String> = Vec::new();
        for c in node.calls.clone() {
            let cl = label(p, c);
            call_names.push(format!("`{}`", cl));
            let ci = p.item(c);
            match self.mention(fn_id, c, depth) {
                "itself" => call_notes.push(format!("This is recursion: `{}` calls itself here, and the calls unwind once the base case is reached.", fname)),
                "seen" => call_notes.push(format!("We already walked through `{}`.", cl)),
                "later" => {
                    let what = doc_sentence(ci.doc.as_ref()).map(|d| format!(" {}", d)).unwrap_or_default();
                    call_notes.push(format!("We will open `{}` in a moment.{}", cl, what));
                }
                "trait" => {
                    let impls: Vec<String> = ci.fn_info().and_then(|cf| cf.trait_id).map(|tid| p.items.iter().filter(|o| o.fn_info().map(|of| of.trait_id == Some(tid) && of.impl_id.is_some() && o.name == ci.name).unwrap_or(false)).map(|o| format!("`{}`", label(p, o.id))).collect()).unwrap_or_default();
                    call_notes.push(format!(
                        "`{}` is a trait method, so which code runs depends on the concrete type{}.",
                        cl,
                        if impls.is_empty() { String::new() } else { format!(": here it could be {}", join_and(&impls)) }
                    ));
                }
                _ => {}
            }
        }
        let callees = join_and(&call_names);
        let (title, text, kind) = match node.kind {
            CfgNodeKind::Exit | CfgNodeKind::Entry => return,
            CfgNodeKind::Block => (
                "Plain statements".to_string(),
                format!("{} {}: `{}`. Straight-line code, nothing branches here.", conn, if node.detail.matches(';').count() > 1 { "a few plain statements" } else { "a plain statement" }, short),
                StepKind::Statement,
            ),
            CfgNodeKind::If => {
                let t = succ.iter().find(|e| e.kind == CfgEdgeKind::True).map(|e| crate::shorten(&cfg.nodes[e.to].label, 40));
                let f = succ.iter().find(|e| e.kind == CfgEdgeKind::False).map(|e| crate::shorten(&cfg.nodes[e.to].label, 40));
                let cond = node.detail.trim_start_matches("if ").trim();
                let mut s = format!("{} a decision: if `{}`", conn, crate::shorten(cond, 60));
                match (t, f) {
                    (Some(t), Some(f)) if f != "exit" => s.push_str(&format!(", we go to `{}`; otherwise we continue with `{}`.", t, f)),
                    (Some(t), _) => s.push_str(&format!(", we go to `{}`; otherwise we carry on below.", t)),
                    _ => s.push('.'),
                }
                ("Decision".to_string(), s, StepKind::Branch)
            }
            CfgNodeKind::Match => {
                let arms: Vec<String> = succ.iter().filter(|e| e.kind == CfgEdgeKind::Arm).map(|e| format!("`{}` leads to `{}`", e.label.clone().unwrap_or_default(), crate::shorten(&cfg.nodes[e.to].label, 36))).collect();
                (
                    format!("Match with {} arms", arms.len()),
                    format!("{} a match on `{}`. It picks one of {} arms: {}.", conn, crate::shorten(node.detail.trim_start_matches("match ").trim(), 50), arms.len(), join_and(&arms)),
                    StepKind::Branch,
                )
            }
            CfgNodeKind::Loop => {
                let has_break = cfg.edges.iter().any(|e| e.kind == CfgEdgeKind::Break);
                let mut s = format!("{} a loop: `{}`. The body runs once per iteration, and when the loop is done control continues below it.", conn, short);
                if has_break {
                    s.push_str(" A break inside can leave it early.");
                }
                ("Loop".to_string(), s, StepKind::Loop)
            }
            CfgNodeKind::Call => (
                format!("Call {}", callees),
                format!("{}{} it calls {}. {}", code_block(&node.detail), conn, callees, call_notes.join(" ")),
                StepKind::Call,
            ),
            CfgNodeKind::Propagate => (
                "Error propagation with ?".to_string(),
                format!(
                    "{}{} the question mark. {} can fail, and if it does, `{}` stops right here and hands that error back to its caller. Otherwise the value is unwrapped and we carry on. {}",
                    code_block(&node.detail),
                    conn,
                    if callees.is_empty() { "This expression".to_string() } else { callees.clone() },
                    fname,
                    call_notes.join(" ")
                ),
                StepKind::Error,
            ),
            CfgNodeKind::Panic => (
                "Possible panic".to_string(),
                format!("Watch out here: `{}` can panic. No error goes back to the caller; the thread simply stops, unless something catches it. {}", short, call_notes.join(" ")),
                StepKind::Error,
            ),
            CfgNodeKind::Return => (
                "Early return".to_string(),
                format!("{} an early exit: `{}`. `{}` leaves here and skips the rest of its body. {}", conn, short, fname, call_notes.join(" ")),
                StepKind::Return,
            ),
            CfgNodeKind::Jump => (
                node.label.clone(),
                if node.label == "break" { "A break: it leaves the enclosing loop immediately.".to_string() } else { "A continue: it skips straight to the next iteration of the loop.".to_string() },
                StepKind::Loop,
            ),
            CfgNodeKind::Await => (
                "Await".to_string(),
                format!("{} an await: `{}`. The function pauses here until the future is ready, and other tasks can run in the meantime. {}", conn, short, call_notes.join(" ")),
                StepKind::Statement,
            ),
        };
        self.push(GuideStep { title, text: text.trim_end().to_string(), mode: Mode::ControlFlow, part: part.to_string(), focus_item: Some(fn_id), focus_cfg: Some(n), highlight_items: vec![fn_id], refs: vec![], span: Some(node.span), stack: vec![fn_id], kind });
    }
}

fn module_display(path: &str) -> String {
    path.trim_start_matches("crate::").to_string()
}

/// Build the tour starting at `root` (defaults to `main`).
pub fn build(p: &Project, root: Option<ItemId>, opts: &GuideOptions) -> Vec<GuideStep> {
    let mut g = Gen { p, opts, steps: vec![], queue: VecDeque::new(), queued: HashSet::new(), walked: HashSet::new() };
    let root = root.or(p.main).or_else(|| crate::views::default_root(p));
    let n_fn = p.functions().count();
    let n_ty = p.items.iter().filter(|i| i.is_type()).count();
    let mods: Vec<&Module> = p.modules.iter().filter(|m| m.path != "crate" && !m.items.is_empty()).collect();

    // ---- welcome + architecture
    let goal = p.modules.first().and_then(|m| doc_sentence(m.doc.as_ref())).map(|d| format!(" {}", d)).unwrap_or_default();
    let size = format!(
        "It is {} source file{} and {} module{}, with {} function{} and {} type{}.",
        p.files.iter().filter(|f| f.rust).count(),
        if p.files.iter().filter(|f| f.rust).count() == 1 { "" } else { "s" },
        p.modules.len(),
        if p.modules.len() == 1 { "" } else { "s" },
        n_fn,
        if n_fn == 1 { "" } else { "s" },
        n_ty,
        if n_ty == 1 { "" } else { "s" }
    );
    g.overview(
        "Welcome",
        &format!("Welcome to {}", p.name),
        format!("Welcome to **{}**.{} {}\n\nLet's start with the big picture. This is the architecture: every box is a module, and the arrows show which module depends on which.", p.name, goal, size),
        Mode::Architecture,
        vec![],
        None,
    );
    let shown = mods.iter().take(opts.max_overview_items).count();
    for (i, m) in mods.iter().take(opts.max_overview_items).enumerate() {
        let fns = m.items.iter().filter(|i| p.item(**i).is_callable()).count();
        let tys = m.items.iter().filter(|i| p.item(**i).is_type()).count();
        let what = doc_sentence(m.doc.as_ref()).unwrap_or_else(|| {
            let names: Vec<String> = m.items.iter().filter(|i| p.item(**i).kind != ItemKind::Method && p.item(**i).kind != ItemKind::Impl).take(4).map(|i| format!("`{}`", p.item(*i).name)).collect();
            format!("It holds {} function{} and {} type{}{}.", fns, if fns == 1 { "" } else { "s" }, tys, if tys == 1 { "" } else { "s" }, if names.is_empty() { String::new() } else { format!(", among them {}", join_and(&names)) })
        });
        let lead = match i {
            0 => "First, the",
            _ if i + 1 == shown => "And last, the",
            _ => ["Next to it, the", "Then the", "The"][i % 3],
        };
        g.overview("Welcome", &format!("Module {}", module_display(&m.path)), format!("{} `{}` module. {}", lead, module_display(&m.path), what), Mode::Architecture, vec![m.path.clone()], None);
    }

    // ---- data types
    if n_ty > 0 {
        let types: Vec<&Item> = p.items.iter().filter(|i| i.is_type()).collect();
        g.overview(
            "The data",
            "Data types",
            "That is the map. Now the data these modules share. Structs and enums hold the values; traits describe behaviour that several types can share. Green arrows mean *contains*, purple dashed arrows mean *implements*.".into(),
            Mode::Types,
            vec![],
            None,
        );
        let mut ranked: Vec<&Item> = types.clone();
        ranked.sort_by_key(|i| match i.kind {
            ItemKind::Trait => 0,
            ItemKind::Enum if i.name.ends_with("Error") => 1,
            ItemKind::Struct => 2,
            ItemKind::Enum => 3,
            _ => 4,
        });
        let shown = ranked.iter().take(opts.max_overview_items).count();
        for (i, it) in ranked.iter().take(opts.max_overview_items).enumerate() {
            let lead = match i {
                0 => "Let's start with",
                _ if i + 1 == shown => "And finally",
                _ => ["Next,", "Then", "Then there is"][i % 3],
            };
            let desc = match &it.extra {
                ItemExtra::Struct(s) => {
                    let names: Vec<String> = s.fields.iter().take(5).map(|f| format!("`{}`", f.name)).collect();
                    if s.is_unit {
                        format!("`{}`, a struct with no fields, a plain marker.", it.name)
                    } else if names.is_empty() || s.is_tuple {
                        format!("`{}`, a struct with {} field{}.", it.name, s.fields.len(), if s.fields.len() == 1 { "" } else { "s" })
                    } else {
                        format!("`{}`, a struct with {} field{}: {}{}.", it.name, s.fields.len(), if s.fields.len() == 1 { "" } else { "s" }, join_and(&names), if s.fields.len() > 5 { " and more" } else { "" })
                    }
                }
                ItemExtra::Enum(e) => {
                    let names: Vec<String> = e.variants.iter().take(6).map(|v| format!("`{}`", v.name)).collect();
                    let role = if it.name.ends_with("Error") { "an error type" } else { "an enum" };
                    format!("`{}`, {} with {} variant{}: {}{}.", it.name, role, e.variants.len(), if e.variants.len() == 1 { "" } else { "s" }, join_and(&names), if e.variants.len() > 6 { " and more" } else { "" })
                }
                ItemExtra::Trait(t) => {
                    let impls: Vec<String> = p.items.iter().filter(|o| matches!(&o.extra, ItemExtra::Impl(ii) if ii.trait_id == Some(it.id))).filter_map(|o| match &o.extra { ItemExtra::Impl(ii) => Some(format!("`{}`", ii.target)), _ => None }).collect();
                    let methods: Vec<String> = t.methods.iter().take(4).map(|m| format!("`{}`", p.item(*m).name)).collect();
                    format!(
                        "the `{}` trait{}{}.",
                        it.name,
                        if methods.is_empty() { String::new() } else { format!(", with {}", join_and(&methods)) },
                        if impls.is_empty() { String::new() } else { format!("; it is implemented by {}", join_and(&impls)) }
                    )
                }
                ItemExtra::TypeAlias { target } => format!("`{}`, an alias for `{}`.", it.name, target),
                _ => format!("`{}`.", it.name),
            };
            let doc = doc_sentence(it.doc.as_ref()).map(|d| format!(" {}", d)).unwrap_or_default();
            g.overview("The data", &it.name.clone(), format!("{} {}{}", lead, desc, doc), Mode::Types, vec![it.path.clone()], None);
        }
    }

    // ---- call tree + functions
    if let Some(r) = root {
        let direct: Vec<String> = {
            let mut seen = HashSet::new();
            p.callees(r).iter().filter_map(|e| e.to).filter(|t| seen.insert(*t)).take(5).map(|t| format!("`{}`", label(p, t))).collect()
        };
        g.overview(
            "Who calls whom",
            "Call tree",
            format!(
                "Now that we know the data, let's follow the program as it runs. Everything starts in `{}`. This call tree shows who calls whom from there{}. We will walk that path one function at a time, staying in each one until it is fully explained.",
                label(p, r),
                if direct.is_empty() { String::new() } else { format!(": it calls {}", join_and(&direct)) }
            ),
            Mode::CallTree,
            vec![p.item(r).path.clone()],
            Some(r),
        );
        g.queued.insert(r);
        g.queue.push_back((r, None, 0));
        while let Some((id, caller, depth)) = g.queue.pop_front() {
            if g.steps.len() >= opts.max_steps {
                break;
            }
            g.walk_fn(id, caller, depth);
        }
    } else {
        g.overview("Who calls whom", "No entry point", "No `fn main` found. Pick a function in the sidebar and choose *codecast from here* to walk through it.".into(), Mode::CallTree, vec![], None);
    }
    g.steps
}
