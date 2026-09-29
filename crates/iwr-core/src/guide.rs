//! Automatic step-by-step walkthrough of a program, starting at `main()`.
//! Each step tells the UI which mode to show, which item / CFG node to focus,
//! and what to say about it.

use crate::model::*;
use crate::views::Mode;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuideStep {
    pub title: String,
    pub text: String,
    pub mode: Mode,
    /// Function (or type/module item) in focus.
    pub focus_item: Option<ItemId>,
    /// CFG node inside `focus_item` in focus (for ControlFlow / BranchTree).
    pub focus_cfg: Option<usize>,
    /// Other items to highlight (call chain so far, related types...).
    pub highlight_items: Vec<ItemId>,
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
}

impl Default for GuideOptions {
    fn default() -> Self {
        Self { max_steps: 300, max_depth: 6, min_body_lines: 0 }
    }
}

fn label(p: &Project, id: ItemId) -> String {
    let it = p.item(id);
    match it.fn_info().and_then(|f| f.owner_type_name.clone().or(f.trait_name.clone())) {
        Some(o) if it.kind == ItemKind::Method => format!("{}::{}", o, it.name),
        _ => it.name.clone(),
    }
}

struct Gen<'a> {
    p: &'a Project,
    opts: &'a GuideOptions,
    steps: Vec<GuideStep>,
    stack: Vec<ItemId>,
    /// functions fully walked already (their details are only shown once)
    walked: HashSet<ItemId>,
}

impl<'a> Gen<'a> {
    fn push(&mut self, mut s: GuideStep) {
        if self.steps.len() >= self.opts.max_steps {
            return;
        }
        s.stack = self.stack.clone();
        if s.highlight_items.is_empty() {
            s.highlight_items = self.stack.clone();
        }
        self.steps.push(s);
    }

    fn walk_fn(&mut self, id: ItemId) {
        let p = self.p;
        let it = p.item(id);
        let Some(f) = it.fn_info() else { return };
        let depth = self.stack.len();
        let already = self.walked.contains(&id);
        self.stack.push(id);
        let mut intro = format!("`{}`", it.signature);
        if let Some(d) = &it.doc {
            intro.push_str("\n\n");
            intro.push_str(d.lines().take(3).collect::<Vec<_>>().join(" ").as_str());
        }
        let mut facts = Vec::new();
        if f.returns_result {
            facts.push(format!("It can fail: returns `Result<{}, {}>`.", f.result_ok.as_deref().unwrap_or("_"), f.result_err.as_deref().unwrap_or("_")));
        } else if f.returns_option {
            facts.push("It may return nothing (`Option`).".into());
        }
        if f.is_async {
            facts.push("It is `async`: it runs when awaited.".into());
        }
        if f.is_recursive {
            facts.push("It calls itself (recursion).".into());
        }
        if f.loops > 0 {
            facts.push(format!("It contains {} loop(s).", f.loops));
        }
        if f.branches > 0 {
            facts.push(format!("It has {} branch point(s).", f.branches));
        }
        let owner = f.owner_type_name.clone();
        if let Some(o) = &owner {
            facts.push(format!("It is a method of `{}`.", o));
        }
        if !facts.is_empty() {
            intro.push_str("\n\n");
            intro.push_str(&facts.join(" "));
        }
        if already {
            intro.push_str("\n\n(Already walked through above; not repeating its body.)");
        }
        self.push(GuideStep {
            title: format!("{} {}()", if depth == 0 { "Start at" } else { "Enter" }, label(p, id)),
            text: intro,
            mode: Mode::ControlFlow,
            focus_item: Some(id),
            focus_cfg: Some(0),
            highlight_items: vec![],
            span: Some(it.span),
            stack: vec![],
            kind: StepKind::Enter,
        });
        if already || f.cfg.is_none() {
            self.stack.pop();
            return;
        }
        self.walked.insert(id);
        let cfg = f.cfg.as_ref().unwrap();
        // walk the CFG in a sensible order: DFS preferring the "main" successor
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
        for n in order {
            if n == 0 {
                continue;
            }
            self.cfg_step(id, cfg, n);
            if self.steps.len() >= self.opts.max_steps {
                break;
            }
        }
        // leave
        let ret = match (&f.ret, f.returns_result, f.returns_option) {
            (None, _, _) => "It returns nothing.".to_string(),
            (Some(r), true, _) => format!("It returns `{}`: either `Ok` with the value, or `Err` that the caller must handle.", r),
            (Some(r), _, true) => format!("It returns `{}`: `Some(value)` or `None`.", r),
            (Some(r), _, _) => format!("It returns `{}`.", r),
        };
        let back = if depth == 0 { "The program ends.".to_string() } else { format!("Back to `{}`.", label(p, self.stack[depth - 1])) };
        self.push(GuideStep {
            title: format!("Leave {}()", label(p, id)),
            text: format!("{} {}", ret, back),
            mode: Mode::ControlFlow,
            focus_item: Some(id),
            focus_cfg: Some(1),
            highlight_items: vec![],
            span: Some(Span { line_start: it.span.line_end, line_end: it.span.line_end, ..it.span }),
            stack: vec![],
            kind: StepKind::Leave,
        });
        self.stack.pop();
    }

    fn cfg_step(&mut self, fn_id: ItemId, cfg: &Cfg, n: usize) {
        let p = self.p;
        let node = &cfg.nodes[n];
        let succ: Vec<&CfgEdge> = cfg.succ(n).collect();
        let calls: Vec<ItemId> = node.calls.clone();
        let call_names: Vec<String> = calls.iter().map(|c| format!("`{}`", label(p, *c))).collect();
        let (title, mut text, kind) = match node.kind {
            CfgNodeKind::Exit | CfgNodeKind::Entry => return,
            CfgNodeKind::Block => (
                "Plain statements".to_string(),
                format!("```rust\n{}\n```\nStraight-line code: runs top to bottom, no branching.", node.detail),
                StepKind::Statement,
            ),
            CfgNodeKind::If => {
                let t = succ.iter().filter(|e| e.kind == CfgEdgeKind::True).map(|e| crate::shorten(&cfg.nodes[e.to].label, 40)).next();
                let f = succ.iter().filter(|e| e.kind == CfgEdgeKind::False).map(|e| crate::shorten(&cfg.nodes[e.to].label, 40)).next();
                let mut s = format!("Branch on `{}`.", node.detail.trim_start_matches("if "));
                if let Some(t) = t {
                    s.push_str(&format!("\n\n• **true** → {}", t));
                }
                if let Some(f) = f {
                    s.push_str(&format!("\n• **false** → {}", f));
                }
                ("Branch: if".to_string(), s, StepKind::Branch)
            }
            CfgNodeKind::Match => {
                let arms: Vec<String> = succ.iter().filter(|e| e.kind == CfgEdgeKind::Arm).map(|e| format!("• `{}` → {}", e.label.clone().unwrap_or_default(), crate::shorten(&cfg.nodes[e.to].label, 36))).collect();
                (
                    format!("Branch: match ({} arms)", arms.len()),
                    format!("`{}` chooses one arm depending on the value:\n\n{}", node.detail, arms.join("\n")),
                    StepKind::Branch,
                )
            }
            CfgNodeKind::Loop => {
                let breaks = cfg.edges.iter().filter(|e| e.kind == CfgEdgeKind::Break && succ.iter().any(|_| true)).count();
                let mut s = format!("`{}` repeats its body. Each iteration follows the **each iteration** edge; when it is done control continues after the loop.", node.detail);
                if breaks > 0 {
                    s.push_str(" A `break` inside exits early.");
                }
                ("Loop".to_string(), s, StepKind::Loop)
            }
            CfgNodeKind::Call => (
                format!("Call {}", call_names.join(", ")),
                format!("```rust\n{}\n```", node.detail),
                StepKind::Call,
            ),
            CfgNodeKind::Propagate => {
                let mut s = format!("```rust\n{}\n```\nThe `?` operator: if this produces `Err` (or `None`), `{}` stops **here** and returns that error to its caller. Otherwise the value is unwrapped and execution continues.", node.detail, label(p, fn_id));
                if !call_names.is_empty() {
                    s.push_str(&format!("\n\nThe fallible call is to {}.", call_names.join(", ")));
                }
                ("Error propagation with ?".to_string(), s, StepKind::Error)
            }
            CfgNodeKind::Panic => (
                "Possible panic".to_string(),
                format!("```rust\n{}\n```\nThis can abort the thread with a panic (unwrap/expect/panic!/assert). No error is returned to the caller; the program crashes unless it is caught.", node.detail),
                StepKind::Error,
            ),
            CfgNodeKind::Return => (
                "Early return".to_string(),
                format!("```rust\n{}\n```\n`{}` leaves here without running the rest of its body.", node.detail, label(p, fn_id)),
                StepKind::Return,
            ),
            CfgNodeKind::Jump => (
                node.label.clone(),
                if node.label == "break" { "Exits the enclosing loop immediately.".to_string() } else { "Skips to the next iteration of the enclosing loop.".to_string() },
                StepKind::Loop,
            ),
            CfgNodeKind::Await => (
                "Await".to_string(),
                format!("```rust\n{}\n```\nSuspends this async function until the awaited future completes; other tasks may run meanwhile.", node.detail),
                StepKind::Statement,
            ),
        };
        let mut span = node.span;
        span.file = node.span.file;
        self.push(GuideStep { title, text: String::new(), mode: Mode::ControlFlow, focus_item: Some(fn_id), focus_cfg: Some(n), highlight_items: vec![], span: Some(span), stack: vec![], kind });
        // dive into calls
        if !calls.is_empty() && self.stack.len() < self.opts.max_depth {
            let mut dive: Vec<ItemId> = Vec::new();
            for c in &calls {
                let ci = p.item(*c);
                let Some(cf) = ci.fn_info() else { continue };
                if self.stack.contains(c) {
                    text.push_str(&format!("\n\n↻ `{}` is already on the call stack: this is **recursion**. It will unwind when the base case is reached.", label(p, *c)));
                    continue;
                }
                if cf.cfg.is_none() {
                    text.push_str(&format!("\n\n`{}` is a trait method: which implementation runs depends on the concrete type.", label(p, *c)));
                    // show implementations
                    if let Some(tid) = cf.trait_id {
                        let impls: Vec<String> = p.items.iter().filter(|o| o.fn_info().map(|of| of.trait_id == Some(tid) && of.impl_id.is_some() && o.name == ci.name).unwrap_or(false)).map(|o| format!("`{}`", label(p, o.id))).collect();
                        if !impls.is_empty() {
                            text.push_str(&format!(" Implementations: {}.", impls.join(", ")));
                        }
                    }
                    continue;
                }
                if cf.body_lines < self.opts.min_body_lines {
                    continue;
                }
                dive.push(*c);
            }
            if let Some(last) = self.steps.last_mut() {
                last.text = text.clone();
            }
            for c in dive {
                self.walk_fn(c);
                let back = GuideStep {
                    title: format!("Back in {}()", label(p, fn_id)),
                    text: format!("`{}` returned. Execution continues after:\n```rust\n{}\n```", label(p, c), crate::shorten(&node.detail, 80)),
                    mode: Mode::ControlFlow,
                    focus_item: Some(fn_id),
                    focus_cfg: Some(n),
                    highlight_items: vec![],
                    span: Some(node.span),
                    stack: vec![],
                    kind: StepKind::Return,
                };
                self.push(back);
            }
        } else if let Some(last) = self.steps.last_mut() {
            last.text = text;
        }
    }
}

/// Build the guide starting at `root` (defaults to `main`).
pub fn build(p: &Project, root: Option<ItemId>, opts: &GuideOptions) -> Vec<GuideStep> {
    let mut g = Gen { p, opts, steps: vec![], stack: vec![], walked: HashSet::new() };
    let root = root.or(p.main).or_else(|| crate::views::default_root(p));
    // overview steps
    let n_fn = p.functions().count();
    let n_ty = p.items.iter().filter(|i| i.is_type()).count();
    let mods: Vec<String> = p.modules.iter().filter(|m| m.path != "crate").map(|m| format!("`{}`", m.path.trim_start_matches("crate::"))).collect();
    g.push(GuideStep {
        title: format!("Overview of {}", p.name),
        text: format!(
            "{} source file(s), {} module(s), {} function(s), {} type(s).{}\n\nThe architecture view shows modules and the dependencies between them. Next we look at the types, then follow execution from the entry point.",
            p.files.len(),
            p.modules.len(),
            n_fn,
            n_ty,
            if mods.is_empty() { String::new() } else { format!("\n\nModules: {}.", mods.join(", ")) }
        ),
        mode: Mode::Architecture,
        focus_item: None,
        focus_cfg: None,
        highlight_items: vec![],
        span: None,
        stack: vec![],
        kind: StepKind::Overview,
    });
    if n_ty > 0 {
        let traits: Vec<String> = p.items.iter().filter(|i| i.kind == ItemKind::Trait).map(|i| format!("`{}`", i.name)).collect();
        let errs: Vec<String> = p.items.iter().filter(|i| i.kind == ItemKind::Enum && i.name.ends_with("Error")).map(|i| format!("`{}`", i.name)).collect();
        let mut t = "Structs and enums hold the data; traits describe shared behaviour. Arrows show which struct contains which type and which types implement which traits.".to_string();
        if !traits.is_empty() {
            t.push_str(&format!("\n\nTraits: {}.", traits.join(", ")));
        }
        if !errs.is_empty() {
            t.push_str(&format!("\n\nError types: {}. See the error-flow view for how they travel.", errs.join(", ")));
        }
        g.push(GuideStep { title: "Data types".into(), text: t, mode: Mode::Types, focus_item: None, focus_cfg: None, highlight_items: p.items.iter().filter(|i| i.is_type()).map(|i| i.id).collect(), span: None, stack: vec![], kind: StepKind::Overview });
    }
    if let Some(r) = root {
        g.push(GuideStep {
            title: "Call tree".into(),
            text: format!("Starting from `{}`, this is who calls whom. Now we follow that path step by step.", label(p, r)),
            mode: Mode::CallTree,
            focus_item: Some(r),
            focus_cfg: None,
            highlight_items: vec![r],
            span: Some(p.item(r).span),
            stack: vec![],
            kind: StepKind::Overview,
        });
        g.walk_fn(r);
    } else {
        g.push(GuideStep { title: "No entry point".into(), text: "No `fn main` found. Pick a function in the sidebar to walk through it.".into(), mode: Mode::CallTree, focus_item: None, focus_cfg: None, highlight_items: vec![], span: None, stack: vec![], kind: StepKind::Overview });
    }
    g.steps
}
