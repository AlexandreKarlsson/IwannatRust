//! Automatic tour of a program: a welcome and the architecture, the data types,
//! the call tree, then one part per function, walked statement by statement and
//! told in plain sentences. Each step tells the UI which view to show, what to
//! focus and what to say. `script::from_guide` turns the steps into a codecast.
//!
//! The tour stays on one function until it is fully explained; the functions it
//! calls are visited afterwards, in the order they were first mentioned (breadth
//! first), each one once. That keeps the camera still and the story linear.

use crate::model::*;
use crate::tr::{self, authored, t::guide as tg, Lang};
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
    /// The language the tour is told in; author text stays as written, marked for translation
    /// ([`crate::tr::authored`]).
    pub lang: Lang,
}

impl Default for GuideOptions {
    fn default() -> Self {
        Self { max_steps: 300, max_depth: 6, min_body_lines: 0, max_overview_items: 10, lang: Lang::En }
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

/// [`doc_sentence`] inside a sentence told in `lang` (marked for translation).
pub(crate) fn told_doc(doc: Option<&String>, lang: Lang) -> Option<String> {
    doc_sentence(doc).map(|d| authored(lang, &d))
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
/// What is said is the author's text: marked for translation when told in another language.
pub fn doc_page(title: &str, doc: Option<&String>, lang: Lang) -> Option<Vec<DocStop>> {
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
    let mut stops = vec![DocStop { note: crate::script::Note { title: Some(title.to_string()), text: doc.clone() }, say: authored(lang, &spoken_md(&sections[0].1.join("\n"))) }];
    for (h, body) in sections.iter().skip(1) {
        let h = h.clone().unwrap_or_default();
        let text = body.join("\n").trim().to_string();
        let say = authored(lang, format!("{}. {}", h.trim_end_matches(['.', ':']), spoken_md(&text)).trim());
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
pub fn module_sentence(p: &Project, m: &Module, lang: Lang) -> String {
    told_doc(m.doc.as_ref(), lang).unwrap_or_else(|| tr::sentences(lang, &[tg::module_no_doc(lang), &module_holds(p, m, lang)]))
}

/// What a module holds: "It holds 3 functions and 1 type, among them `a`, `b`…".
fn module_holds(p: &Project, m: &Module, lang: Lang) -> String {
    let fns = m.items.iter().filter(|i| p.item(**i).is_callable()).count();
    let tys = m.items.iter().filter(|i| p.item(**i).is_type()).count();
    let names: Vec<String> = m.items.iter().filter(|i| p.item(**i).kind != ItemKind::Method && p.item(**i).kind != ItemKind::Impl).take(4).map(|i| format!("`{}`", p.item(*i).name)).collect();
    let (f, t) = (tg::module_fns(lang, fns), tg::module_types(lang, tys));
    if names.is_empty() {
        tg::module_holds(lang, f, t)
    } else {
        tg::module_holds_among(lang, f, t, tr::join_and(lang, &names))
    }
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

/// The i-th of n modules of a tour: "First, the `a` module.", "Then the `b` module.", … "And
/// last, the `z` module.".
pub fn module_lead(lang: Lang, i: usize, n: usize, name: &str) -> String {
    match i {
        0 => tg::module_first(lang, name),
        _ if i + 1 == n => tg::module_last(lang, name),
        _ => [tg::module_then, tg::module_next, tg::module_plain][i % 3](lang, name),
    }
}

/// One sentence about a type: what kind it is, its fields / variants / implementors, then its doc.
pub fn type_sentence(p: &Project, it: &Item, lang: Lang) -> String {
    let name = &it.name;
    let desc = match &it.extra {
        ItemExtra::Struct(s) => {
            let names: Vec<String> = s.fields.iter().take(5).map(|f| format!("`{}`", f.name)).collect();
            let n = s.fields.len();
            if s.is_unit {
                tg::ty_unit(lang, name)
            } else if names.is_empty() || s.is_tuple {
                tg::ty_struct_count(lang, n, name)
            } else if n > 5 {
                tg::ty_struct_fields_more(lang, n, name, tr::join_and(lang, &names))
            } else {
                tg::ty_struct_fields(lang, n, name, tr::join_and(lang, &names))
            }
        }
        ItemExtra::Enum(e) => {
            let names = tr::join_and(lang, &e.variants.iter().take(6).map(|v| format!("`{}`", v.name)).collect::<Vec<_>>());
            let n = e.variants.len();
            match (it.name.ends_with("Error"), n > 6) {
                (true, true) => tg::ty_error_more(lang, n, name, names),
                (true, false) => tg::ty_error(lang, n, name, names),
                (false, true) => tg::ty_enum_more(lang, n, name, names),
                (false, false) => tg::ty_enum(lang, n, name, names),
            }
        }
        ItemExtra::Trait(t) => {
            let impls: Vec<String> = p.items.iter().filter(|o| matches!(&o.extra, ItemExtra::Impl(ii) if ii.trait_id == Some(it.id))).filter_map(|o| match &o.extra { ItemExtra::Impl(ii) => Some(format!("`{}`", ii.target)), _ => None }).collect();
            let methods: Vec<String> = t.methods.iter().take(4).map(|m| format!("`{}`", p.item(*m).name)).collect();
            match (methods.is_empty(), impls.is_empty()) {
                (true, true) => tg::ty_trait(lang, name),
                (false, true) => tg::ty_trait_methods(lang, name, tr::join_and(lang, &methods)),
                (true, false) => tg::ty_trait_impls(lang, name, tr::join_and(lang, &impls)),
                (false, false) => tg::ty_trait_methods_impls(lang, name, tr::join_and(lang, &methods), tr::join_and(lang, &impls)),
            }
        }
        ItemExtra::TypeAlias { target } => tg::ty_alias(lang, name, target),
        _ => tg::ty_other(lang, name),
    };
    tr::sentences(lang, &[&desc, &told_doc(it.doc.as_ref(), lang).unwrap_or_default()])
}

/// Every type, tests included, in tour order (the built-in tour's data chapter).
fn tour_types_all(p: &Project) -> Vec<&Item> {
    let mut ranked: Vec<&Item> = p.items.iter().filter(|i| i.is_type()).collect();
    ranked.sort_by_key(|i| type_rank(i));
    ranked
}

/// Traits first, then error enums, structs, other enums.
fn type_rank(i: &Item) -> u8 {
    match i.kind {
        ItemKind::Trait => 0,
        ItemKind::Enum if i.name.ends_with("Error") => 1,
        ItemKind::Struct => 2,
        ItemKind::Enum => 3,
        _ => 4,
    }
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

/// Sentence connector for the i-th of n steps inside a function.
fn connector(i: usize, n: usize, lang: Lang) -> &'static str {
    if n > 1 && i + 1 == n {
        return tg::conn_last(lang);
    }
    match i {
        0 => tg::conn_first(lang),
        _ => [tg::conn_then, tg::conn_next, tg::conn_after, tg::conn_then, tg::conn_now][i % 5](lang),
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
        let lang = self.opts.lang;
        let intro = match caller {
            None if f.is_main => tg::enter_main(lang, &name),
            None => tg::enter_root(lang, &name),
            Some(c) => {
                let cl = label(p, c);
                let e = p.calls.iter().find(|e| e.from == c && e.to == Some(id));
                match e {
                    Some(e) if e.in_loop => tg::enter_called_loop(lang, &name, &cl),
                    Some(e) if e.propagated => tg::enter_called_question(lang, &name, &cl),
                    Some(e) if e.in_branch => tg::enter_called_branch(lang, &name, &cl),
                    _ => tg::enter_called(lang, &name, &cl),
                }
            }
        };
        let intro = tr::sentences(lang, &[&intro, &told_doc(it.doc.as_ref(), lang).unwrap_or_default()]);
        let mut facts: Vec<String> = Vec::new();
        let params: Vec<String> = f.params.iter().filter(|a| a.name != "self").map(|a| format!("`{}`", a.name)).collect();
        let gives = match (&f.ret, f.returns_result, f.returns_option) {
            (None, _, _) => tg::enter_gives_nothing(lang).to_string(),
            (Some(_), true, _) => tg::enter_gives_result(lang, f.result_ok.as_deref().map(|o| format!("`{}`", o)).unwrap_or_else(|| tg::enter_a_value(lang).into()), f.result_err.as_deref().unwrap_or("_")),
            (Some(r), _, true) => tg::enter_gives_option(lang, r),
            (Some(r), _, _) => tg::enter_gives(lang, r),
        };
        facts.push(if params.is_empty() { tg::enter_gives_only(lang, gives) } else { tg::enter_takes_gives(lang, tr::join_and(lang, &params), gives) });
        if f.is_async {
            facts.push(tg::enter_async(lang).into());
        }
        if f.is_recursive {
            facts.push(tg::enter_recursive(lang).into());
        }
        let mut shape: Vec<String> = Vec::new();
        if f.loops > 0 {
            shape.push(tg::enter_loops(lang, f.loops));
        }
        if f.branches > 0 {
            shape.push(tg::enter_decisions(lang, f.branches));
        }
        if f.question_marks > 0 {
            shape.push(tg::enter_escapes(lang, f.question_marks));
        }
        if !shape.is_empty() {
            let single = shape.len() == 1 && [f.loops, f.branches, f.question_marks].iter().sum::<usize>() == 1;
            let list = tr::join_and(lang, &shape);
            facts.push(if single { tg::enter_inside_is(lang, list) } else { tg::enter_inside_are(lang, list) });
        }
        if let Some(o) = &f.owner_type_name {
            facts.push(tg::enter_method_of(lang, o));
        }
        let text = format!("{}\n\n{}", intro, facts.join(tr::gap(lang)));
        self.push(GuideStep {
            title: if caller.is_none() { tg::title_start_at(lang, &name) } else { tg::title_enter(lang, &name) },
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
            self.cfg_step(id, &part, cfg, *n, connector(i, n_body, lang), depth);
            if self.steps.len() >= self.opts.max_steps {
                break;
            }
        }

        // ---- leave
        let ret = match (&f.ret, f.returns_result, f.returns_option) {
            (None, _, _) => tg::leave_ret_nothing(lang).to_string(),
            (Some(_), true, _) => tg::leave_ret_result(lang).to_string(),
            (Some(r), _, true) => tg::leave_ret_option(lang, r),
            (Some(r), _, _) => tg::leave_ret(lang, r),
        };
        let next = match self.queue.front() {
            _ if self.single => String::new(),
            Some((nid, Some(c), _)) if *c == id => tg::leave_next_called(lang, label(p, *nid)),
            Some((nid, _, _)) => tg::leave_next(lang, label(p, *nid)),
            None if caller.is_none() => tg::leave_end_program(lang).to_string(),
            None => tg::leave_end_tour(lang).to_string(),
        };
        self.push(GuideStep {
            title: tg::title_leave(lang, &name),
            text: tg::leave_text(lang, &name, ret, next),
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
        let lang = self.opts.lang;
        // what we say about the callees of this node
        let mut call_notes: Vec<String> = Vec::new();
        let mut call_names: Vec<String> = Vec::new();
        for c in node.calls.clone() {
            let cl = label(p, c);
            call_names.push(format!("`{}`", cl));
            let ci = p.item(c);
            match self.mention(fn_id, c, depth) {
                "itself" => call_notes.push(tg::callee_itself(lang, &fname)),
                "seen" => call_notes.push(tg::callee_seen(lang, &cl)),
                "later" => call_notes.push(tr::sentences(lang, &[&tg::callee_later(lang, &cl), &told_doc(ci.doc.as_ref(), lang).unwrap_or_default()])),
                "trait" => {
                    let impls: Vec<String> = ci.fn_info().and_then(|cf| cf.trait_id).map(|tid| p.items.iter().filter(|o| o.fn_info().map(|of| of.trait_id == Some(tid) && of.impl_id.is_some() && o.name == ci.name).unwrap_or(false)).map(|o| format!("`{}`", label(p, o.id))).collect()).unwrap_or_default();
                    call_notes.push(if impls.is_empty() { tg::callee_trait(lang, &cl) } else { tg::callee_trait_impls(lang, &cl, tr::join_and(lang, &impls)) });
                }
                _ => {}
            }
        }
        let callees = tr::join_and(lang, &call_names);
        let notes = call_notes.join(tr::gap(lang));
        // a step's sentence, then what its calls lead to
        let with_notes = |s: String| tr::sentences(lang, &[&s, &notes]);
        let (title, text, kind) = match node.kind {
            CfgNodeKind::Exit | CfgNodeKind::Entry => return,
            CfgNodeKind::Io => (tg::title_io(lang).to_string(), tg::step_io(lang, conn, &short), StepKind::Statement),
            CfgNodeKind::Assign => (tg::title_assign(lang).to_string(), tg::step_assign(lang, conn, &short), StepKind::Statement),
            CfgNodeKind::Let if crate::views::group_title(node).is_some() => (crate::views::group_title(node).unwrap_or_default(), tg::step_definitions(lang, conn, &short), StepKind::Statement),
            CfgNodeKind::Block | CfgNodeKind::Let | CfgNodeKind::Log => (
                tg::title_statements(lang).to_string(),
                if node.detail.matches(';').count() > 1 { tg::step_statements(lang, conn, &short) } else { tg::step_statement(lang, conn, &short) },
                StepKind::Statement,
            ),
            CfgNodeKind::If => {
                let t = succ.iter().find(|e| e.kind == CfgEdgeKind::True).map(|e| crate::shorten(&cfg.nodes[e.to].label, 40));
                let f = succ.iter().find(|e| e.kind == CfgEdgeKind::False).map(|e| crate::shorten(&cfg.nodes[e.to].label, 40));
                let cond = crate::shorten(node.detail.trim_start_matches("if ").trim(), 60);
                let s = match (t, f) {
                    (Some(t), Some(f)) if f != "exit" => tg::step_if_both(lang, conn, &cond, t, f),
                    (Some(t), _) => tg::step_if_then(lang, conn, &cond, t),
                    _ => tg::step_if_plain(lang, conn, &cond),
                };
                (tg::title_decision(lang).to_string(), s, StepKind::Branch)
            }
            CfgNodeKind::Match => {
                let arms: Vec<String> = succ.iter().filter(|e| e.kind == CfgEdgeKind::Arm).map(|e| tg::step_arm(lang, e.label.clone().unwrap_or_default(), crate::shorten(&cfg.nodes[e.to].label, 36))).collect();
                let on = crate::shorten(node.detail.trim_start_matches("match ").trim(), 50);
                (tg::title_match_arms(lang, arms.len()), tg::step_match(lang, arms.len(), conn, on, tr::join_and(lang, &arms)), StepKind::Branch)
            }
            CfgNodeKind::Loop => {
                let has_break = cfg.edges.iter().any(|e| e.kind == CfgEdgeKind::Break);
                let s = tg::step_loop(lang, conn, &short);
                (tg::title_loop(lang).to_string(), if has_break { tr::sentences(lang, &[&s, tg::step_loop_break(lang)]) } else { s }, StepKind::Loop)
            }
            CfgNodeKind::Call => (tg::title_call(lang, &callees), format!("{}{}", code_block(&node.detail), with_notes(tg::step_call(lang, conn, &callees))), StepKind::Call),
            CfgNodeKind::Propagate => (
                tg::title_propagate(lang).to_string(),
                format!("{}{}", code_block(&node.detail), with_notes(tg::step_propagate(lang, conn, if callees.is_empty() { tg::step_this_expression(lang).to_string() } else { callees.clone() }, &fname))),
                StepKind::Error,
            ),
            CfgNodeKind::Panic => (tg::title_panic(lang).to_string(), with_notes(tg::step_panic(lang, &short)), StepKind::Error),
            CfgNodeKind::Halt => (tg::title_halt(lang).to_string(), tg::step_halt(lang, conn, &node.label), StepKind::Return),
            CfgNodeKind::Return | CfgNodeKind::ReturnErr => (tg::title_early_return(lang).to_string(), with_notes(tg::step_early_return(lang, conn, &short, &fname)), StepKind::Return),
            CfgNodeKind::Jump => (node.label.clone(), if node.label == "break" { tg::step_break(lang) } else { tg::step_continue(lang) }.to_string(), StepKind::Loop),
            CfgNodeKind::Await => (tg::title_await(lang).to_string(), with_notes(tg::step_await(lang, conn, &short)), StepKind::Statement),
        };
        // the author's own short comment says best what this step is for
        let (title, text) = match &node.note {
            Some(note) => {
                let end = if note.ends_with(['.', '!', '?']) { "" } else { "." };
                (authored(lang, note), tr::sentences(lang, &[&authored(lang, &format!("{}{}", note, end)), text.trim_end()]))
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
    let lang = opts.lang;
    let (part_welcome, part_data, part_calls) = (tg::part_welcome(lang), tg::part_data(lang), tg::part_calls(lang));
    // the crate doc as a page: shown from the welcome on, its sections read before the map
    let mut page = doc_page(&p.name, p.modules.first().and_then(|m| m.doc.as_ref()), lang).unwrap_or_default().into_iter();
    let first = page.next();
    let goal = match &first {
        Some(st) => Some(st.say.clone()).filter(|s| !s.is_empty()),
        None => p.modules.first().and_then(|m| told_doc(m.doc.as_ref(), lang)),
    }
    .unwrap_or_default();
    let n_files = p.files.iter().filter(|f| f.rust).count();
    let size = tg::intro_size(lang, tg::intro_files(lang, n_files), tg::intro_modules(lang, p.modules.len()), tg::intro_fns(lang, n_fn), tg::intro_types(lang, n_ty));
    let welcome = tr::sentences(lang, &[&tg::intro_welcome(lang, &p.name), &goal, &size]);
    let big_picture = tg::intro_big_picture(lang);
    match first {
        Some(st) => {
            g.overview_note(part_welcome, &tg::title_welcome(lang, &p.name), welcome, vec![], st.note);
            for st in page {
                g.overview_note(part_welcome, &tg::title_about(lang, &p.name), st.say, vec![], st.note);
            }
            g.overview(part_welcome, tg::title_architecture(lang), big_picture.into(), Mode::Architecture, vec![], None);
        }
        None => g.overview(part_welcome, &tg::title_welcome(lang, &p.name), format!("{}\n\n{}", welcome, big_picture), Mode::Architecture, vec![], None),
    }
    let shown = mods.iter().take(opts.max_overview_items).count();
    for (i, m) in mods.iter().take(opts.max_overview_items).enumerate() {
        let title = tg::title_module(lang, module_display(&m.path));
        let lead = module_lead(lang, i, shown, &module_display(&m.path));
        match doc_page(&module_display(&m.path), m.doc.as_ref(), lang) {
            Some(stops) => {
                for (k, st) in stops.into_iter().enumerate() {
                    let text = if k == 0 { tr::sentences(lang, &[&lead, &st.say]) } else { st.say };
                    g.overview_note(part_welcome, &title, text, vec![m.path.clone()], st.note);
                }
            }
            None => {
                let what = told_doc(m.doc.as_ref(), lang).unwrap_or_else(|| module_holds(p, m, lang));
                g.overview(part_welcome, &title, tr::sentences(lang, &[&lead, &what]), Mode::Architecture, vec![m.path.clone()], None);
            }
        }
    }

    // ---- data types
    if n_ty > 0 {
        g.overview(part_data, tg::title_data(lang), tg::intro_data(lang).into(), Mode::Types, vec![], None);
        let ranked = tour_types_all(p);
        let shown = ranked.iter().take(opts.max_overview_items).count();
        for (i, it) in ranked.iter().take(opts.max_overview_items).enumerate() {
            let lead = match i {
                0 => tg::type_lead_first(lang),
                _ if i + 1 == shown => tg::type_lead_last(lang),
                _ => [tg::type_lead_next, tg::type_lead_then, tg::type_lead_there][i % 3](lang),
            };
            let desc = type_sentence(p, it, lang);
            g.overview(part_data, &it.name.clone(), tr::led(lang, lead, &desc), Mode::Types, vec![it.path.clone()], None);
        }
    }

    // ---- call tree + functions
    if let Some(r) = root {
        let direct: Vec<String> = {
            let mut seen = HashSet::new();
            p.callees(r).iter().filter_map(|e| e.to).filter(|t| seen.insert(*t)).take(5).map(|t| format!("`{}`", label(p, t))).collect()
        };
        let text = if direct.is_empty() { tg::intro_call_tree(lang, label(p, r)) } else { tg::intro_call_tree_calls(lang, label(p, r), tr::join_and(lang, &direct)) };
        g.overview(part_calls, tg::title_call_tree(lang), text, Mode::CallTree, vec![p.item(r).path.clone()], Some(r));
        g.queued.insert(r);
        g.queue.push_back((r, None, 0));
        while let Some((id, caller, depth)) = g.queue.pop_front() {
            if g.steps.len() >= opts.max_steps {
                break;
            }
            g.walk_fn(id, caller, depth);
        }
    } else {
        g.overview(part_calls, tg::title_no_entry(lang), tg::intro_no_main(lang).into(), Mode::CallTree, vec![], None);
    }
    g.steps
}
