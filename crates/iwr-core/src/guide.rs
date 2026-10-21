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
    /// Markdown for the side panel while this step is shown (a doc page, see [`doc_page`]).
    #[serde(default)]
    pub note: Option<crate::script::Note>,
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

pub(crate) fn label(p: &Project, id: ItemId) -> String {
    let it = p.item(id);
    match it.fn_info().and_then(|f| f.owner_type_name.clone().or(f.trait_name.clone())) {
        Some(o) if it.kind == ItemKind::Method => format!("{}::{}", o, it.name),
        _ => it.name.clone(),
    }
}

/// First paragraph of a doc comment, as one line ending with a period.
pub(crate) fn doc_sentence(doc: Option<&String>) -> Option<String> {
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

/// One stop of a doc page: the markdown the side panel shows and what is said meanwhile.
#[derive(Debug, Clone, PartialEq)]
pub struct DocStop {
    pub note: crate::script::Note,
    pub say: String,
}

/// A doc comment written as a page (`#` headings, tables or lists) read in full: the first stop
/// shows the whole page and reads what comes before the first heading, then one stop per
/// heading shows and reads its section. `None` for a plain doc, which [`doc_sentence`] reads.
pub fn doc_page(title: &str, doc: Option<&String>) -> Option<Vec<DocStop>> {
    let doc = doc?;
    let mut fence = false;
    let mut rich = false;
    // (heading, body lines); the first has no heading
    let mut sections: Vec<(Option<String>, Vec<&str>)> = vec![(None, vec![])];
    for line in doc.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            fence = !fence;
        }
        let heading = if fence { None } else { md_heading(t) };
        match heading {
            Some(h) => sections.push((Some(h.to_string()), vec![])),
            None => sections.last_mut().unwrap().1.push(line),
        }
        rich |= !fence && (heading.is_some() || t.starts_with('|') || t.starts_with("- ") || t.starts_with("* "));
    }
    if !rich {
        return None;
    }
    let mut stops = vec![DocStop { note: crate::script::Note { title: Some(title.to_string()), text: doc.clone() }, say: spoken_md(&sections[0].1.join("\n")) }];
    for (h, body) in sections.iter().skip(1) {
        let h = h.clone().unwrap_or_default();
        let text = body.join("\n").trim().to_string();
        let say = format!("{}. {}", h.trim_end_matches(['.', ':']), spoken_md(&text)).trim().to_string();
        // the heading again in the body: the panel's title line cuts long ones
        stops.push(DocStop { note: crate::script::Note { title: Some(format!("{} · {}", title, h)), text: format!("## {}\n\n{}", h, text) }, say });
    }
    Some(stops)
}

fn md_heading(t: &str) -> Option<&str> {
    let h = t.trim_start_matches('#');
    (h.len() < t.len() && t.len() - h.len() <= 6 && h.starts_with(' ')).then(|| h.trim()).filter(|h| !h.is_empty())
}

/// Markdown as sentences to say: paragraphs and list items as they are, each table row as
/// "first cell: header value, …" (at most 8 rows), code blocks left to the screen.
pub fn spoken_md(md: &str) -> String {
    const MAX_ROWS: usize = 8;
    let mut out: Vec<String> = Vec::new();
    let mut para: Vec<&str> = Vec::new();
    let mut head: Option<Vec<String>> = None;
    let mut rows = 0;
    let mut fence = false;
    let sentence = |s: &str| {
        let s = s.trim();
        if s.ends_with(['.', '!', '?', ':']) { s.to_string() } else { format!("{}.", s) }
    };
    let cells = |t: &str| t.trim_matches('|').split('|').map(|c| c.trim().to_string()).collect::<Vec<_>>();
    for line in md.lines().chain(std::iter::once("")) {
        let t = line.trim();
        if t.starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        let row = t.starts_with('|');
        let item = t.strip_prefix("- ").or_else(|| t.strip_prefix("* "));
        if (t.is_empty() || row || item.is_some()) && !para.is_empty() {
            out.push(sentence(&para.join(" ")));
            para.clear();
        }
        if !row {
            if rows > MAX_ROWS {
                out.push(format!("And {} more rows.", rows - MAX_ROWS));
            }
            head = None;
            rows = 0;
        }
        if row {
            if t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) {
                continue;
            }
            let c = cells(t);
            let Some(h) = &head else {
                head = Some(c);
                continue;
            };
            rows += 1;
            if rows > MAX_ROWS {
                continue;
            }
            let rest: Vec<String> = c.iter().enumerate().skip(1).filter(|(_, v)| !v.is_empty()).map(|(i, v)| if c.len() == 2 { v.clone() } else { format!("{} {}", h.get(i).map(String::as_str).unwrap_or(""), v).trim().to_string() }).collect();
            out.push(if rest.is_empty() { sentence(&c[0]) } else { sentence(&format!("{}: {}", c[0], rest.join(", "))) });
        } else if let Some(i) = item {
            out.push(sentence(i));
        } else if !t.is_empty() {
            para.push(t);
        }
    }
    out.join(" ")
}

/// What a module is for: the first paragraph of its `//!` doc, else what it holds.
pub fn module_sentence(p: &Project, m: &Module) -> String {
    doc_sentence(m.doc.as_ref()).unwrap_or_else(|| {
        let fns = m.items.iter().filter(|i| p.item(**i).is_callable()).count();
        let tys = m.items.iter().filter(|i| p.item(**i).is_type()).count();
        let names: Vec<String> = m.items.iter().filter(|i| p.item(**i).kind != ItemKind::Method && p.item(**i).kind != ItemKind::Impl).take(4).map(|i| format!("`{}`", p.item(*i).name)).collect();
        format!("It has no module doc yet. It holds {} function{} and {} type{}{}.", fns, if fns == 1 { "" } else { "s" }, tys, if tys == 1 { "" } else { "s" }, if names.is_empty() { String::new() } else { format!(", among them {}", join_and(&names)) })
    })
}

/// Modules worth a stop in a tour, in the Architecture view's reading order: top to bottom
/// (users before what they use, the entry point first), then left to right. Only boxes the
/// view shows: no tests, nothing folded inside another box, not the crate root of a
/// single crate (the intro speaks for it). `scope` limits it like the view's module scope.
pub fn tour_modules(p: &Project, scope: Option<ModuleId>) -> Vec<&Module> {
    let g = crate::views::build(p, Mode::Architecture, &crate::views::ViewOptions { module: scope, ..Default::default() });
    let mut boxes: Vec<(f64, f64, ModuleId)> = g
        .nodes
        .iter()
        .filter_map(|n| n.id.strip_prefix('m').and_then(|x| x.parse::<ModuleId>().ok()).map(|m| (n.y, n.x, m)))
        .collect();
    boxes.sort_by(|a, b| (a.0, a.1).partial_cmp(&(b.0, b.1)).unwrap_or(std::cmp::Ordering::Equal));
    boxes.into_iter().map(|(_, _, m)| p.module(m)).filter(|m| m.path != "crate" && Some(m.id) != scope).collect()
}

/// Lead-in for the i-th of n stops: "First, the", "Then the", ... "And last, the".
pub fn tour_lead(i: usize, n: usize) -> &'static str {
    match i {
        0 => "First, the",
        _ if i + 1 == n => "And last, the",
        _ => ["Then the", "Next, the", "The"][i % 3],
    }
}

/// One sentence about a type: what kind it is, its fields / variants / implementors, then its doc.
pub fn type_sentence(p: &Project, it: &Item) -> String {
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
    format!("{}{}", desc, doc)
}

/// Types in tour order: traits, error enums, structs, other enums.
pub fn tour_types(p: &Project) -> Vec<&Item> {
    let mut ranked: Vec<&Item> = p.items.iter().filter(|i| i.is_type() && !crate::views::is_test_module(p, i.module)).collect();
    ranked.sort_by_key(|i| match i.kind {
        ItemKind::Trait => 0,
        ItemKind::Enum if i.name.ends_with("Error") => 1,
        ItemKind::Struct => 2,
        ItemKind::Enum => 3,
        _ => 4,
    });
    ranked
}

/// "a, b and c"
pub(crate) fn join_and(items: &[String]) -> String {
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
    /// one function only: no callees queued, no "next" at the end
    single: bool,
}

impl<'a> Gen<'a> {
    fn push(&mut self, s: GuideStep) {
        if self.steps.len() < self.opts.max_steps {
            self.steps.push(s);
        }
    }

    fn overview(&mut self, part: &str, title: &str, text: String, mode: Mode, refs: Vec<String>, focus: Option<ItemId>) {
        let span = focus.map(|f| self.p.item(f).span);
        self.push(GuideStep { title: title.to_string(), text, mode, part: part.to_string(), focus_item: focus, focus_cfg: None, highlight_items: focus.into_iter().collect(), refs, span, stack: vec![], kind: StepKind::Overview, note: None });
    }

    /// An architecture step showing `note` in the side panel.
    fn overview_note(&mut self, part: &str, title: &str, text: String, refs: Vec<String>, note: crate::script::Note) {
        let n = self.steps.len();
        self.overview(part, title, text, Mode::Architecture, refs, None);
        if let Some(added) = self.steps.get_mut(n) {
            added.note = Some(note);
        }
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
            note: None,
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
            _ if self.single => String::new(),
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
            note: None,
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
            CfgNodeKind::Io => (
                "Input / output".to_string(),
                format!("{} input or output: `{}`. This is where the program talks to the outside world.", conn, short),
                StepKind::Statement,
            ),
            CfgNodeKind::Assign => (
                "State changes".to_string(),
                format!("{} a change of state: `{}`. Something that already exists is modified here.", conn, short),
                StepKind::Statement,
            ),
            CfgNodeKind::Let if crate::views::group_title(node).is_some() => (
                crate::views::group_title(node).unwrap_or_default(),
                format!("{} a few definitions: `{}`. Straight-line code, nothing branches here.", conn, short),
                StepKind::Statement,
            ),
            CfgNodeKind::Block | CfgNodeKind::Let | CfgNodeKind::Log => (
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
            CfgNodeKind::Halt => (
                "The program stops".to_string(),
                format!("{} the program stops here: {}. No caller gets control back; the whole process ends.", conn, node.label),
                StepKind::Return,
            ),
            CfgNodeKind::Return | CfgNodeKind::ReturnErr => (
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
        // the author's own short comment says best what this step is for
        let (title, text) = match &node.note {
            Some(note) => {
                let end = if note.ends_with(['.', '!', '?']) { "" } else { "." };
                (note.clone(), format!("{}{} {}", note, end, text.trim_end()))
            }
            None => (title, text),
        };
        self.push(GuideStep { title, text: text.trim_end().to_string(), mode: Mode::ControlFlow, part: part.to_string(), focus_item: Some(fn_id), focus_cfg: Some(n), highlight_items: vec![fn_id], refs: vec![], span: Some(node.span), stack: vec![fn_id], kind, note: None });
    }
}

fn module_display(path: &str) -> String {
    path.trim_start_matches("crate::").to_string()
}

/// The guide's chapter on one function: enter, each step of its body in reading order, leave.
/// Callees are named but not visited.
pub fn function_steps(p: &Project, id: ItemId, opts: &GuideOptions) -> Vec<GuideStep> {
    let opts = GuideOptions { max_depth: 1, ..opts.clone() };
    let mut g = Gen { p, opts: &opts, steps: vec![], queue: VecDeque::new(), queued: HashSet::new(), walked: HashSet::new(), single: true };
    g.walk_fn(id, None, 0);
    g.steps
}

/// Build the tour starting at `root` (defaults to `main`).
pub fn build(p: &Project, root: Option<ItemId>, opts: &GuideOptions) -> Vec<GuideStep> {
    let mut g = Gen { p, opts, steps: vec![], queue: VecDeque::new(), queued: HashSet::new(), walked: HashSet::new(), single: false };
    let root = root.or(p.main).or_else(|| crate::views::default_root(p));
    let n_fn = p.functions().count();
    let n_ty = p.items.iter().filter(|i| i.is_type()).count();
    let mods: Vec<&Module> = tour_modules(p, None).into_iter().filter(|m| !m.items.is_empty() || m.doc.is_some()).collect();

    // ---- welcome + architecture
    // the crate doc as a page: shown from the welcome on, its sections read before the map
    let mut page = doc_page(&p.name, p.modules.first().and_then(|m| m.doc.as_ref())).unwrap_or_default().into_iter();
    let first = page.next();
    let goal = match &first {
        Some(st) => Some(st.say.clone()).filter(|s| !s.is_empty()),
        None => p.modules.first().and_then(|m| doc_sentence(m.doc.as_ref())),
    }
    .map(|d| format!(" {}", d))
    .unwrap_or_default();
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
    let big_picture = "Let's start with the big picture. This is the architecture: every box is a module, and the arrows show which module depends on which.";
    match first {
        Some(st) => {
            g.overview_note("Welcome", &format!("Welcome to {}", p.name), format!("Welcome to **{}**.{} {}", p.name, goal, size), vec![], st.note);
            for st in page {
                g.overview_note("Welcome", &format!("About {}", p.name), st.say, vec![], st.note);
            }
            g.overview("Welcome", "The architecture", big_picture.into(), Mode::Architecture, vec![], None);
        }
        None => g.overview("Welcome", &format!("Welcome to {}", p.name), format!("Welcome to **{}**.{} {}\n\n{}", p.name, goal, size, big_picture), Mode::Architecture, vec![], None),
    }
    let shown = mods.iter().take(opts.max_overview_items).count();
    for (i, m) in mods.iter().take(opts.max_overview_items).enumerate() {
        let title = format!("Module {}", module_display(&m.path));
        let lead = format!("{} `{}` module.", tour_lead(i, shown), module_display(&m.path));
        match doc_page(&module_display(&m.path), m.doc.as_ref()) {
            Some(stops) => {
                for (k, st) in stops.into_iter().enumerate() {
                    let text = if k == 0 { format!("{} {}", lead, st.say) } else { st.say };
                    g.overview_note("Welcome", &title, text, vec![m.path.clone()], st.note);
                }
            }
            None => {
                let what = module_sentence(p, m).replace("It has no module doc yet. ", "");
                g.overview("Welcome", &title, format!("{} {}", lead, what), Mode::Architecture, vec![m.path.clone()], None);
            }
        }
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
            let desc = type_sentence(p, it);
            g.overview("The data", &it.name.clone(), format!("{} {}", lead, desc), Mode::Types, vec![it.path.clone()], None);
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
