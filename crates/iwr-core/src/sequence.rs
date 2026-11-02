//! Sequence diagrams: who calls whom, in time order, top to bottom. Two sources, one drawing:
//!
//! - a ```` ```mermaid ```` block that starts with `sequenceDiagram` (written by the author;
//!   [`parse`], shown with `@ diagram` like a flowchart);
//! - the project itself: `@ sequence:<fn>` walks the calls made from a function, in source
//!   order, one column per type or module ([`from_calls`]).
//!
//! ```text
//! sequenceDiagram
//! participant c as Client          a column (`actor` too); ids are the refs (`!c`)
//! c->>s: request                   solid arrow; `-->>` dotted reply; `->` / `-->` no head
//! s->>+db: query                   `+` starts an activation bar on the target, `-` ends the sender's
//! Note over s,db: cached           `Note left of` / `right of` / `over a,b`
//! loop every second                frames: loop / alt / else / opt / par / and / critical / break
//!   s-)c: ping                     async; `-x` lost
//! end
//! autonumber                       numbers the messages
//! ```
//!
//! Messages are refs too: `m1`, `m2`, … in order, so `!m3` glows the third arrow and
//! `> -m3` / `> +m3` hide and show it (its row stays, nothing jumps).
//!
//! The drawing is a [`Graph`] like the other views: participant boxes, activation bars (small
//! boxes carrying the called function, so a fn ref glows them), frames (containers), notes, and
//! edges for lifelines and messages.

use crate::diagram::Op;
use crate::model::{BlockKind, CallKind, ItemId, ItemKind, ModuleId, Project, Span};
use crate::views::{self, display_width, edge_label_width, EdgeKind, Graph, Mode, NodeKind, VEdge, VNode, ViewOptions};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A column of the diagram.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Participant {
    pub id: String,
    pub label: String,
    /// `actor`: drawn as a pill
    #[serde(default)]
    pub actor: bool,
    /// project item it stands for (a type), when built from the code
    #[serde(default)]
    pub item: Option<ItemId>,
    /// module it stands for (free functions), when built from the code
    #[serde(default)]
    pub module: Option<ModuleId>,
    /// box colour; `Box` for a hand-written one
    pub kind: NodeKind,
}

/// How a message arrow ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Head {
    Arrow,
    None,
    /// `-x`: lost message
    Cross,
    /// `-)`: async
    Async,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// `m1`, `m2`, … in order
    pub id: String,
    pub from: String,
    pub to: String,
    pub label: String,
    pub dotted: bool,
    pub head: Head,
    /// `+`: starts an activation bar on `to`
    pub activate: bool,
    /// `-`: ends the activation bar of `from`
    pub deactivate: bool,
    /// the function called (activation bars carry it)
    #[serde(default)]
    pub item: Option<ItemId>,
    /// where the call is written
    #[serde(default)]
    pub span: Option<Span>,
    /// colour of the arrow, when built from the code (same kinds as the call tree)
    #[serde(default)]
    pub kind: Option<EdgeKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteSide {
    Left,
    Right,
    Over,
}

/// One line of the diagram, in time order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Msg(Message),
    Note { id: String, side: NoteSide, over: Vec<String>, text: String },
    Activate(String),
    Deactivate(String),
    /// a frame starts: `loop`, `alt`, `opt`, … with its text
    Open { kind: String, label: String },
    /// `else` / `and` / `option`: the frame goes on under another label
    Else { label: String },
    Close,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Sequence {
    pub participants: Vec<Participant>,
    pub steps: Vec<Step>,
    /// `autonumber`: messages labelled `1.`, `2.`, …
    #[serde(default)]
    pub autonumber: bool,
}

impl Sequence {
    pub fn participant(&self, id: &str) -> Option<&Participant> {
        self.participants.iter().find(|p| p.id == id)
    }
    pub fn messages(&self) -> impl Iterator<Item = &Message> {
        self.steps.iter().filter_map(|s| if let Step::Msg(m) = s { Some(m) } else { None })
    }
    /// Every ref of the diagram: participants, messages, notes.
    pub fn ids(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.participants.iter().map(|p| p.id.as_str()).collect();
        for s in &self.steps {
            match s {
                Step::Msg(m) => v.push(&m.id),
                Step::Note { id, .. } => v.push(id),
                _ => {}
            }
        }
        v
    }
    fn touch(&mut self, id: &str) {
        if self.participant(id).is_none() {
            self.participants.push(Participant { id: id.to_string(), label: id.to_string(), actor: false, item: None, module: None, kind: NodeKind::Box });
        }
    }
    fn next_id(&self, prefix: &str) -> String {
        let n = self.steps.iter().filter(|s| match s {
            Step::Msg(_) => prefix == "m",
            Step::Note { .. } => prefix == "n",
            _ => false,
        });
        format!("{}{}", prefix, n.count() + 1)
    }
}

// ------------------------------------------------------------------ parse

const FRAMES: [&str; 7] = ["loop", "alt", "opt", "par", "critical", "break", "rect"];
const DIVIDERS: [&str; 3] = ["else", "and", "option"];
/// Arrow tokens, longest first so `-->>` is not read as `->`.
const ARROWS: [(&str, bool, Head); 8] = [
    ("-->>", true, Head::Arrow),
    ("->>", false, Head::Arrow),
    ("--x", true, Head::Cross),
    ("-x", false, Head::Cross),
    ("--)", true, Head::Async),
    ("-)", false, Head::Async),
    ("-->", true, Head::None),
    ("->", false, Head::None),
];

/// Parse the lines of a `sequenceDiagram` block after its header. Never fails: lines it cannot
/// read go to `errors` (as `line N: …`, numbered as given). Returns the `title` line too.
pub fn parse(lines: &[(usize, &str)], errors: &mut Vec<String>) -> (Sequence, Option<String>) {
    let mut s = Sequence::default();
    let mut title = None;
    let mut depth = 0usize;
    for &(ln, raw) in lines {
        let line = raw.split("%%").next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        let word = lower.split_whitespace().next().unwrap_or("");
        let rest = line[word.len()..].trim();
        let bad = |e: &mut Vec<String>, msg: &str| e.push(format!("line {}: {} in `{}`", ln + 1, msg, line));
        match word {
            "title" => title = Some(rest.trim_start_matches(':').trim().to_string()).filter(|t| !t.is_empty()),
            "autonumber" => s.autonumber = true,
            "participant" | "actor" | "create" => {
                let rest = if word == "create" { rest.split_once(char::is_whitespace).map(|x| x.1.trim()).unwrap_or("") } else { rest };
                let actor = word == "actor" || lower.starts_with("create actor");
                let (id, label) = match rest.split_once(" as ") {
                    Some((i, l)) => (i.trim(), l.trim()),
                    None => (rest, rest),
                };
                if id.is_empty() {
                    bad(errors, "a participant needs a name");
                    continue;
                }
                s.touch(id);
                let p = s.participants.iter_mut().find(|p| p.id == id).unwrap();
                p.label = label.to_string();
                p.actor = actor;
                if actor {
                    p.kind = NodeKind::Pill;
                }
            }
            "activate" | "deactivate" if !rest.is_empty() => {
                s.touch(rest);
                s.steps.push(if word == "activate" { Step::Activate(rest.to_string()) } else { Step::Deactivate(rest.to_string()) });
            }
            "destroy" | "box" | "links" | "link" | "properties" | "details" => {}
            "note" => match parse_note(rest) {
                Some((side, over, text)) => {
                    for p in &over {
                        s.touch(p);
                    }
                    let id = s.next_id("n");
                    s.steps.push(Step::Note { id, side, over, text });
                }
                None => bad(errors, "a note reads `Note right of a: text`, `Note over a,b: text`"),
            },
            "end" => {
                if depth == 0 {
                    bad(errors, "`end` without a frame");
                } else {
                    depth -= 1;
                    s.steps.push(Step::Close);
                }
            }
            w if FRAMES.contains(&w) => {
                depth += 1;
                let label = if w == "rect" { String::new() } else { rest.to_string() };
                s.steps.push(Step::Open { kind: w.to_string(), label });
            }
            w if DIVIDERS.contains(&w) => {
                if depth == 0 {
                    bad(errors, "outside a frame");
                } else {
                    s.steps.push(Step::Else { label: rest.to_string() });
                }
            }
            _ => match parse_message(line) {
                Some(mut m) => {
                    s.touch(&m.from);
                    s.touch(&m.to);
                    m.id = s.next_id("m");
                    s.steps.push(Step::Msg(m));
                }
                None => bad(errors, "not a message (`a->>b: text`) nor a keyword"),
            },
        }
    }
    if depth > 0 {
        errors.push(format!("{} frame(s) without `end`", depth));
    }
    (s, title)
}

/// `right of a: text` / `left of a: text` / `over a,b: text`
fn parse_note(rest: &str) -> Option<(NoteSide, Vec<String>, String)> {
    let (head, text) = rest.split_once(':')?;
    let head = head.trim();
    let lower = head.to_ascii_lowercase();
    let (side, who) = if let Some(w) = lower.strip_prefix("right of") {
        (NoteSide::Right, &head[head.len() - w.len()..])
    } else if let Some(w) = lower.strip_prefix("left of") {
        (NoteSide::Left, &head[head.len() - w.len()..])
    } else if let Some(w) = lower.strip_prefix("over") {
        (NoteSide::Over, &head[head.len() - w.len()..])
    } else {
        return None;
    };
    let over: Vec<String> = who.split(',').map(|w| w.trim().to_string()).filter(|w| !w.is_empty()).collect();
    if over.is_empty() {
        return None;
    }
    Some((side, over, text.trim().to_string()))
}

/// `a->>+b: text`
fn parse_message(line: &str) -> Option<Message> {
    let (lhs, text) = match line.split_once(':') {
        Some((l, t)) => (l, t.trim()),
        None => (line, ""),
    };
    let (at, (tok, dotted, head)) = ARROWS.iter().filter_map(|a| lhs.find(a.0).map(|i| (i, *a))).min_by_key(|(i, a)| (*i, std::cmp::Reverse(a.0.len())))?;
    let from = lhs[..at].trim();
    let mut to = lhs[at + tok.len()..].trim();
    let (mut activate, mut deactivate) = (false, false);
    if let Some(t) = to.strip_prefix('+') {
        activate = true;
        to = t.trim();
    } else if let Some(t) = to.strip_prefix('-') {
        deactivate = true;
        to = t.trim();
    }
    if from.is_empty() || to.is_empty() {
        return None;
    }
    Some(Message { id: String::new(), from: from.to_string(), to: to.to_string(), label: text.to_string(), dotted, head, activate, deactivate, item: None, span: None, kind: None })
}

/// Back to mermaid text (for a generated diagram: `codecast` can print it for the author).
pub fn to_text(s: &Sequence) -> String {
    let mut out = String::from("sequenceDiagram\n");
    if s.autonumber {
        out.push_str("    autonumber\n");
    }
    for p in &s.participants {
        let kw = if p.actor { "actor" } else { "participant" };
        if p.label == p.id {
            out.push_str(&format!("    {} {}\n", kw, p.id));
        } else {
            out.push_str(&format!("    {} {} as {}\n", kw, p.id, p.label));
        }
    }
    let mut depth = 1;
    for st in &s.steps {
        let pad = |d: usize| "    ".repeat(d);
        match st {
            Step::Msg(m) => {
                let tok = ARROWS.iter().find(|a| a.1 == m.dotted && a.2 == m.head).map(|a| a.0).unwrap_or("->>");
                let sign = if m.activate { "+" } else if m.deactivate { "-" } else { "" };
                out.push_str(&format!("{}{}{}{}{}: {}\n", pad(depth), m.from, tok, sign, m.to, m.label));
            }
            Step::Note { side, over, text, .. } => {
                let w = match side {
                    NoteSide::Left => "left of",
                    NoteSide::Right => "right of",
                    NoteSide::Over => "over",
                };
                out.push_str(&format!("{}Note {} {}: {}\n", pad(depth), w, over.join(","), text));
            }
            Step::Activate(p) => out.push_str(&format!("{}activate {}\n", pad(depth), p)),
            Step::Deactivate(p) => out.push_str(&format!("{}deactivate {}\n", pad(depth), p)),
            Step::Open { kind, label } => {
                out.push_str(&format!("{}{} {}\n", pad(depth), kind, label).replace(" \n", "\n"));
                depth += 1;
            }
            Step::Else { label } => out.push_str(&format!("{}else {}\n", pad(depth - 1), label).replace(" \n", "\n")),
            Step::Close => {
                depth = depth.saturating_sub(1).max(1);
                out.push_str(&format!("{}end\n", pad(depth)));
            }
        }
    }
    out
}

// ------------------------------------------------------------------ from the code

/// Most messages a generated diagram shows; past that it stops and says so.
const MAX_MESSAGES: usize = 120;

/// The sequence of calls made from `opts.root` (else the entry point): one column per type
/// (methods) or module (free functions), calls in source order, each callee's own calls nested
/// under it down to `opts.depth` levels. A callee is opened once; later calls to it are single
/// arrows. Loops and branches around calls become frames. Returns a note when it had to stop.
pub fn from_calls(p: &Project, opts: &ViewOptions) -> (Sequence, Option<String>) {
    let mut s = Sequence::default();
    let Some(root) = opts.root.or_else(|| views::default_root(p)) else {
        return (s, Some("No functions found.".into()));
    };
    s.participants.push(Participant { id: "caller".into(), label: "caller".into(), actor: true, item: None, module: None, kind: NodeKind::Pill });
    let mut w = Walk { p, opts, s, opened: HashSet::new(), stack: vec![], cut: false };
    let rp = w.participant(root);
    let it = p.item(root);
    w.message("caller", &rp, format!("{}()", it.name), Some(root), Some(it.span), Some(EdgeKind::Call));
    w.walk(root, &rp, 1);
    w.s.steps.push(Step::Deactivate(rp));
    let note = w.cut.then(|| format!("Cut at {} messages: lower the depth or start from a deeper function.", MAX_MESSAGES));
    (w.s, note)
}

struct Walk<'a> {
    p: &'a Project,
    opts: &'a ViewOptions,
    s: Sequence,
    opened: HashSet<ItemId>,
    stack: Vec<ItemId>,
    cut: bool,
}

impl Walk<'_> {
    /// The column of a function: its type for a method, its module for a free function.
    fn participant(&mut self, f: ItemId) -> String {
        let it = self.p.item(f);
        let info = it.fn_info();
        let (id, label, item, module, kind) = match (info.and_then(|i| i.owner_type), info.and_then(|i| i.owner_type_name.clone().or(i.trait_name.clone()))) {
            (Some(t), _) => (format!("t{}", t), self.p.item(t).name.clone(), Some(t), None, views::kind_of_item(self.p.item(t))),
            (None, Some(name)) => (format!("n{}", name), name, None, None, NodeKind::Trait),
            (None, None) => {
                let m = self.p.module(it.module);
                let label = if m.parent.is_none() { m.path.clone() } else { m.name.clone() };
                (format!("m{}", m.id), label, None, Some(m.id), NodeKind::Module)
            }
        };
        if self.s.participant(&id).is_none() {
            self.s.participants.push(Participant { id: id.clone(), label, actor: false, item, module, kind });
        }
        id
    }

    fn message(&mut self, from: &str, to: &str, label: String, item: Option<ItemId>, span: Option<Span>, kind: Option<EdgeKind>) {
        let id = self.s.next_id("m");
        self.s.steps.push(Step::Msg(Message { id, from: from.into(), to: to.into(), label, dotted: false, head: Head::Arrow, activate: true, deactivate: false, item, span, kind }));
    }

    fn walk(&mut self, f: ItemId, me: &str, depth: usize) {
        let p = self.p;
        let opts = self.opts;
        let mut calls: Vec<_> = p
            .callees(f)
            .into_iter()
            .filter(|c| c.kind != CallKind::Macro && (c.kind != CallKind::Construct || opts.show_constructs))
            .filter(|c| c.to.is_some_and(|t| opts.show_tests || !p.item(t).fn_info().is_some_and(|i| i.is_test)))
            .collect();
        calls.sort_by_key(|c| (c.span.line_start, c.span.col_start));
        self.stack.push(f);
        let blocks = p.item(f).fn_info().and_then(|i| i.blocks.as_ref());
        // frames open around the current call: (block id) per level
        let mut open: Vec<usize> = vec![];
        for c in calls {
            if self.s.messages().count() >= MAX_MESSAGES {
                self.cut = true;
                break;
            }
            let t = c.to.unwrap();
            // frames: the loops and branches around this call
            let chain = blocks.map(|b| frames_around(b, &c.span)).unwrap_or_default();
            let keep = open.iter().zip(&chain).take_while(|(a, b)| **a == b.0).count();
            for _ in keep..open.len() {
                self.s.steps.push(Step::Close);
            }
            open.truncate(keep);
            for (id, kind, label) in &chain[keep..] {
                self.s.steps.push(Step::Open { kind: kind.to_string(), label: label.clone() });
                open.push(*id);
            }
            let target = p.item(t);
            let to = self.participant(t);
            let recursive = c.is_recursive || self.stack.contains(&t);
            let mut label = format!("{}()", target.name);
            if c.is_await {
                label.push_str(".await");
            }
            if c.propagated {
                label.push('?');
            }
            let kind = if recursive {
                EdgeKind::Recursion
            } else if c.propagated {
                EdgeKind::Propagate
            } else if c.unwrapped {
                EdgeKind::Unwrap
            } else if c.kind == CallKind::Construct {
                EdgeKind::Construct
            } else if c.is_await {
                EdgeKind::Await
            } else {
                EdgeKind::Call
            };
            self.message(me, &to, label, Some(t), Some(c.span), Some(kind));
            if !recursive && depth < opts.depth.max(1) && self.opened.insert(t) {
                self.walk(t, &to, depth + 1);
            }
            self.s.steps.push(Step::Deactivate(to));
        }
        for _ in &open {
            self.s.steps.push(Step::Close);
        }
        self.stack.pop();
    }
}

/// The loops and branches of a body that hold a call, outermost first: (block id, frame kind,
/// title: the `//` comment above it, else its code).
fn frames_around(b: &crate::model::Block, at: &Span) -> Vec<(usize, &'static str, String)> {
    let mut out = vec![];
    let mut cur = b;
    // an `if` block's span runs over its `else` branches too: the latest sibling that holds the
    // call is the branch it is in
    while let Some(child) = cur.children.iter().filter(|ch| holds(&ch.span, at) && ch.kind.is_compound()).max_by_key(|ch| (ch.span.line_start, ch.span.col_start)) {
        let frame = match child.kind {
            BlockKind::Loop => Some("loop"),
            BlockKind::If | BlockKind::ElseIf | BlockKind::Else => Some("alt"),
            BlockKind::Arm => Some("alt"),
            _ => None,
        };
        if let Some(k) = frame {
            // a `//` comment above the loop or branch says what it is for: it is the title
            let title = child.note.clone().unwrap_or_else(|| child.label.trim_end_matches('{').trim().to_string());
            out.push((child.id, k, crate::shorten(&title, 44)));
        }
        cur = child;
    }
    out
}

/// Does span `a` hold the start of `b`?
fn holds(a: &Span, b: &Span) -> bool {
    a.file == b.file && (a.line_start, a.col_start) <= (b.line_start, b.col_start) && (b.line_start, b.col_start) < (a.line_end, a.col_end)
}

// ------------------------------------------------------------------ layout

const MARGIN: f64 = 24.0;
const HEAD_H: f64 = 38.0;
const ROW: f64 = 34.0;
const ACT_W: f64 = 10.0;
/// sideways step of a nested activation bar on the same lifeline
const NEST: f64 = 5.0;
const SELF_W: f64 = 34.0;
const SELF_H: f64 = 18.0;
const FRAME_TITLE: f64 = 28.0;
const NOTE_H: f64 = 30.0;

fn text_w(s: &str) -> f64 {
    display_width(s) * 7.4 + 28.0
}

fn vnode(id: String, label: String, kind: NodeKind, (x, y, w, h): (f64, f64, f64, f64)) -> VNode {
    VNode { id, label, sublabel: String::new(), kind, item: None, cfg_node: None, span: None, x, y, w, h, expandable: false, expanded: false, badges: vec![], depth: 0, parent: None, container: false, caption: String::new(), chips: vec![] }
}

fn vedge(id: String, from: &str, to: &str, kind: EdgeKind, label: Option<String>, points: Vec<(f64, f64)>, backward: bool) -> VEdge {
    VEdge { id, from: from.into(), to: to.into(), kind, label, points, backward, ..Default::default() }
}

/// Lay the sequence out as a [`Graph`]. `ops` hide or show participants, messages and notes
/// (`> -m3`, `> +m3`); hidden things keep their place.
pub fn graph(s: &Sequence, ops: &[Op], mode: Mode) -> Graph {
    let mut g = Graph { mode: Some(mode), ..Default::default() };
    if s.participants.is_empty() {
        g.note = Some("Empty sequence diagram.".into());
        return g;
    }
    let mut hidden: HashSet<&str> = HashSet::new();
    for op in ops {
        match op {
            Op::Hide(id) => {
                hidden.insert(id.as_str());
            }
            Op::Show(id) => {
                hidden.remove(id.as_str());
            }
            _ => {}
        }
    }
    let col: HashMap<&str, usize> = s.participants.iter().enumerate().map(|(i, p)| (p.id.as_str(), i)).collect();
    let n = s.participants.len();
    let widths: Vec<f64> = s.participants.iter().map(|p| text_w(&p.label).max(84.0)).collect();
    let label_of = |m: &Message, k: usize| if s.autonumber { format!("{}. {}", k, m.label) } else { m.label.clone() };

    // columns: each gap wide enough for the boxes, the labels between them, self-loops, notes
    let mut need: Vec<Vec<(usize, f64)>> = vec![vec![]; n]; // need[j] = (i, gap) with i < j
    for i in 1..n {
        need[i].push((i - 1, (widths[i - 1] + widths[i]) / 2.0 + 28.0));
    }
    let mut k = 0;
    for st in &s.steps {
        match st {
            Step::Msg(m) => {
                k += 1;
                let (a, b) = (col[m.from.as_str()], col[m.to.as_str()]);
                let w = if m.label.is_empty() { 0.0 } else { edge_label_width(&label_of(m, k)) };
                if a == b {
                    if a + 1 < n {
                        need[a + 1].push((a, SELF_W + w + 30.0));
                    }
                } else {
                    let (lo, hi) = (a.min(b), a.max(b));
                    need[hi].push((lo, w + 44.0));
                }
            }
            Step::Note { side, over, text, .. } => {
                let w = text_w(text);
                let c = col[over[0].as_str()];
                match side {
                    NoteSide::Right if c + 1 < n => need[c + 1].push((c, w + 24.0)),
                    NoteSide::Left if c > 0 => need[c].push((c - 1, w + 24.0)),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    let mut cx = vec![0.0f64; n];
    // room on the left for a note left of the first column
    let left_room = s.steps.iter().filter_map(|st| match st {
        Step::Note { side: NoteSide::Left, over, text, .. } if col[over[0].as_str()] == 0 => Some(text_w(text) + 16.0 - widths[0] / 2.0),
        _ => None,
    });
    cx[0] = MARGIN + widths[0] / 2.0 + left_room.fold(0.0, f64::max).max(0.0);
    for j in 1..n {
        cx[j] = need[j].iter().map(|&(i, gap)| cx[i] + gap).fold(cx[j - 1], f64::max);
    }

    // participant boxes
    for (i, p) in s.participants.iter().enumerate() {
        if hidden.contains(p.id.as_str()) {
            continue;
        }
        let mut v = vnode(p.id.clone(), p.label.clone(), p.kind, (cx[i] - widths[i] / 2.0, MARGIN, widths[i], HEAD_H));
        v.item = p.item;
        g.nodes.push(v);
    }

    // rows
    let mut y = MARGIN + HEAD_H + 8.0;
    // open activation bars per column: (start y, message id, item)
    let mut bars: Vec<Vec<(f64, String, Option<ItemId>, bool)>> = vec![vec![]; n];
    let mut bar_count = 0usize;
    // open frames; an `else` starts a new section of the same frame (same `gid`)
    struct Frame {
        gid: usize,
        kind: String,
        /// word in the frame's corner tab: the kind, or `else` for a later section
        tag: String,
        label: String,
        top: f64,
        lo: usize,
        hi: usize,
        self_loop: bool,
    }
    let mut frames: Vec<Frame> = vec![];
    let mut gids = 0usize;
    let mut boxes: Vec<(Frame, f64, usize)> = vec![]; // finished sections: frame, bottom, depth
    let touch = |frames: &mut Vec<Frame>, a: usize, b: usize, selfl: bool| {
        for f in frames.iter_mut() {
            f.lo = f.lo.min(a.min(b));
            f.hi = f.hi.max(a.max(b));
            f.self_loop |= selfl;
        }
    };
    let bar_x = |bars: &Vec<Vec<(f64, String, Option<ItemId>, bool)>>, c: usize, right: bool| -> f64 {
        let lvl = bars[c].len();
        if lvl == 0 {
            cx[c]
        } else {
            let left = cx[c] - ACT_W / 2.0 + (lvl - 1) as f64 * NEST;
            if right { left + ACT_W } else { left }
        }
    };
    let close_bar = |g: &mut Graph, bars: &mut Vec<Vec<(f64, String, Option<ItemId>, bool)>>, c: usize, end: f64, count: &mut usize| {
        if let Some((top, msg, item, hide)) = bars[c].pop() {
            let lvl = bars[c].len();
            *count += 1;
            if hide {
                return;
            }
            let h = (end - top).max(16.0);
            let mut v = vnode(format!("a{}", msg), String::new(), NodeKind::Block, (cx[c] - ACT_W / 2.0 + lvl as f64 * NEST, top, ACT_W, h));
            v.item = item;
            v.kind = NodeKind::Function;
            v.depth = lvl;
            // a nested bar sits on its outer one, a little to the right
            v.parent = bars[c].last().map(|b| format!("a{}", b.1));
            g.nodes.push(v);
        }
    };
    let mut k = 0;
    let mut last_msg_y = y;
    for st in &s.steps {
        match st {
            Step::Msg(m) => {
                k += 1;
                y += ROW;
                let (a, b) = (col[m.from.as_str()], col[m.to.as_str()]);
                let hide = hidden.contains(m.id.as_str()) || hidden.contains(m.from.as_str()) || hidden.contains(m.to.as_str());
                touch(&mut frames, a, b, a == b);
                let label = Some(label_of(m, k)).filter(|l| !l.is_empty());
                let kind = m.kind.unwrap_or(match (m.dotted, m.head) {
                    (false, Head::None) => EdgeKind::Line,
                    (true, Head::None) => EdgeKind::DottedLine,
                    (true, _) => EdgeKind::Dotted,
                    (false, _) => EdgeKind::Arrow,
                });
                if a == b {
                    let x0 = bar_x(&bars, a, true);
                    if m.activate {
                        bars[b].push((y + SELF_H - 6.0, m.id.clone(), m.item, hide));
                    }
                    let x1 = bar_x(&bars, b, true);
                    let to_id = if m.activate { format!("a{}", m.id) } else { m.to.clone() };
                    if !hide {
                        let pts = vec![(x0, y), (x0 + SELF_W, y), (x0 + SELF_W, y + SELF_H), (x1, y + SELF_H)];
                        g.edges.push(vedge(m.id.clone(), &m.from, &to_id, kind, label, pts, true));
                    }
                    y += SELF_H;
                } else {
                    let right = b > a;
                    let x0 = bar_x(&bars, a, right);
                    if m.activate {
                        bars[b].push((y - 4.0, m.id.clone(), m.item, hide));
                    }
                    let x1 = bar_x(&bars, b, !right);
                    let to_id = if m.activate { format!("a{}", m.id) } else { m.to.clone() };
                    if !hide {
                        g.edges.push(vedge(m.id.clone(), &m.from, &to_id, kind, label, vec![(x0, y), (x1, y)], false));
                    }
                }
                if m.deactivate {
                    close_bar(&mut g, &mut bars, a, y + 4.0, &mut bar_count);
                }
                last_msg_y = y;
            }
            Step::Activate(p) => {
                let c = col[p.as_str()];
                let key = format!("{}{}", p, bar_count + bars[c].len());
                bars[c].push((y + 4.0, key, None, hidden.contains(p.as_str())));
            }
            Step::Deactivate(p) => {
                let c = col[p.as_str()];
                close_bar(&mut g, &mut bars, c, (y + 6.0).max(last_msg_y + 10.0), &mut bar_count);
            }
            Step::Note { id, side, over, text } => {
                y += 10.0;
                let cs: Vec<usize> = over.iter().map(|o| col[o.as_str()]).collect();
                let (lo, hi) = (*cs.iter().min().unwrap(), *cs.iter().max().unwrap());
                touch(&mut frames, lo, hi, false);
                let w = text_w(text);
                let x = match side {
                    NoteSide::Right => cx[lo] + 10.0,
                    NoteSide::Left => cx[lo] - 10.0 - w,
                    NoteSide::Over if lo == hi => cx[lo] - w / 2.0,
                    NoteSide::Over => {
                        let ww = (cx[hi] - cx[lo] + 40.0).max(w);
                        let mid = (cx[lo] + cx[hi]) / 2.0;
                        if !hidden.contains(id.as_str()) {
                            g.nodes.push(vnode(id.clone(), text.clone(), NodeKind::Note, (mid - ww / 2.0, y, ww, NOTE_H)));
                        }
                        y += NOTE_H;
                        continue;
                    }
                };
                if !hidden.contains(id.as_str()) {
                    g.nodes.push(vnode(id.clone(), text.clone(), NodeKind::Note, (x, y, w, NOTE_H)));
                }
                y += NOTE_H;
            }
            Step::Open { kind, label } => {
                y += FRAME_TITLE;
                let first = col.len();
                gids += 1;
                frames.push(Frame { gid: gids, kind: kind.clone(), tag: kind.clone(), label: label.clone(), top: y - FRAME_TITLE + 6.0, lo: first, hi: 0, self_loop: false });
            }
            Step::Else { label } => {
                y += 14.0;
                if let Some(f) = frames.pop() {
                    let next = Frame { gid: f.gid, kind: f.kind.clone(), tag: "else".into(), label: label.clone(), top: y, lo: f.lo, hi: f.hi, self_loop: f.self_loop };
                    let d = frames.len();
                    boxes.push((f, y - 4.0, d));
                    frames.push(next);
                }
                y += FRAME_TITLE - 14.0;
            }
            Step::Close => {
                y += 14.0;
                if let Some(f) = frames.pop() {
                    let d = frames.len();
                    // the parent frame covers what this one covered
                    if let Some(p) = frames.last_mut() {
                        if f.lo <= f.hi {
                            p.lo = p.lo.min(f.lo);
                            p.hi = p.hi.max(f.hi);
                        }
                        p.self_loop |= f.self_loop;
                    }
                    // the sections already finished of this frame share its width
                    for (b, _, _) in boxes.iter_mut().filter(|(b, _, _)| b.gid == f.gid) {
                        b.lo = b.lo.min(f.lo);
                        b.hi = b.hi.max(f.hi);
                        b.self_loop |= f.self_loop;
                    }
                    boxes.push((f, y - 4.0, d));
                }
            }
        }
    }
    for c in 0..n {
        while !bars[c].is_empty() {
            close_bar(&mut g, &mut bars, c, y + 10.0, &mut bar_count);
        }
    }
    let bottom = y + ROW * 0.6;

    // frames: as wide as the columns they touch, nested ones a bit narrower
    for (f, fb, d) in boxes {
        let (lo, hi) = if f.lo <= f.hi { (f.lo, f.hi) } else { (0, 0) };
        let pad = 34.0 - d as f64 * 8.0;
        let x0 = cx[lo] - pad - 26.0;
        let mut x1 = cx[hi] + pad + 26.0;
        if f.self_loop {
            x1 = x1.max(cx[hi] + SELF_W + 30.0);
        }
        let kind = match f.kind.as_str() {
            "loop" => NodeKind::Loop,
            "alt" | "opt" => NodeKind::If,
            "par" => NodeKind::Await,
            "critical" | "break" => NodeKind::Panic,
            _ => NodeKind::Group,
        };
        // the title after the corner tab; the tab (kind word) in `sublabel`, empty for a `rect`
        let mut v = vnode(format!("f{}", g.nodes.len()), f.label.clone(), kind, (x0, f.top, x1 - x0, fb - f.top));
        v.sublabel = if f.kind == "rect" { String::new() } else { f.tag.clone() };
        v.container = true;
        v.depth = d;
        g.nodes.push(v);
    }

    // lifelines, behind everything
    let mut lines = vec![];
    for (i, p) in s.participants.iter().enumerate() {
        if hidden.contains(p.id.as_str()) {
            continue;
        }
        lines.push(vedge(format!("life:{}", p.id), &p.id, &p.id, EdgeKind::Lifeline, None, vec![(cx[i], MARGIN + HEAD_H), (cx[i], bottom)], false));
    }
    lines.append(&mut g.edges);
    g.edges = lines;

    // containers first (outer before inner), then the rest, so the UI draws them behind
    g.nodes.sort_by_key(|n| (!n.container, n.depth));
    let right = g.nodes.iter().map(|n| n.x + n.w).fold(0.0, f64::max).max(g.edges.iter().flat_map(|e| e.points.iter().map(|p| p.0)).fold(0.0, f64::max));
    g.width = right + MARGIN;
    g.height = bottom + MARGIN;
    g
}

/// The `Mode::Sequence` view: [`from_calls`] laid out.
pub fn view(p: &Project, opts: &ViewOptions) -> Graph {
    let (s, note) = from_calls(p, opts);
    let mut g = graph(&s, &[], Mode::Sequence);
    if g.note.is_none() {
        g.note = note;
    }
    // activation bars carry the function they stand for: its kind, its signature on hover
    for n in g.nodes.iter_mut() {
        if let Some(item) = n.item.filter(|_| n.id.starts_with('a')) {
            let it = p.item(item);
            n.kind = if it.kind == ItemKind::Method { NodeKind::Method } else { NodeKind::Function };
            n.span = Some(it.span);
        }
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seq(src: &str) -> (Sequence, Vec<String>) {
        let lines: Vec<(usize, &str)> = src.lines().enumerate().collect();
        let mut errs = vec![];
        let (s, _) = parse(&lines, &mut errs);
        (s, errs)
    }

    #[test]
    fn parses_messages_notes_frames() {
        let (s, errs) = seq("participant c as Client\nactor u\nc->>+s: hello\ns-->>-c: hi\nNote over c,s: done\nloop every 1s\n  s-)c: ping\nelse never\n  c-xs: lost\nend\nc->s\n");
        assert!(errs.is_empty(), "{:?}", errs);
        assert_eq!(s.participants.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["c", "u", "s"]);
        assert_eq!(s.participant("c").unwrap().label, "Client");
        assert!(s.participant("u").unwrap().actor);
        let ms: Vec<&Message> = s.messages().collect();
        assert_eq!(ms.len(), 5);
        assert!(ms[0].activate && !ms[0].dotted && ms[0].head == Head::Arrow);
        assert!(ms[1].deactivate && ms[1].dotted);
        assert_eq!(ms[2].head, Head::Async);
        assert_eq!(ms[3].head, Head::Cross);
        assert_eq!(ms[4].head, Head::None);
        assert_eq!(s.ids(), ["c", "u", "s", "m1", "m2", "n1", "m3", "m4", "m5"]);
    }

    #[test]
    fn reports_bad_lines() {
        let (_, errs) = seq("hello there\nend\nloop x\n");
        assert_eq!(errs.len(), 3, "{:?}", errs);
    }

    #[test]
    fn lays_out_left_to_right_top_to_bottom() {
        let (s, _) = seq("a->>+b: one\nb->>b: self\nb-->>-a: back\n");
        let g = graph(&s, &[], Mode::Diagram);
        let (a, b) = (g.node("a").unwrap(), g.node("b").unwrap());
        assert!(a.x + a.w < b.x);
        let y = |id: &str| g.edges.iter().find(|e| e.id == id).unwrap().points[0].1;
        assert!(y("m1") < y("m2") && y("m2") < y("m3"));
        // one activation bar on b, from the first message to the last
        let bar = g.node("am1").unwrap();
        assert!(bar.y <= y("m1") && bar.y + bar.h >= y("m3") - 1.0);
        // lifelines for both
        assert_eq!(g.edges.iter().filter(|e| e.kind == EdgeKind::Lifeline).count(), 2);
        // hidden message: no edge, same rows
        let g2 = graph(&s, &[Op::Hide("m2".into())], Mode::Diagram);
        assert!(g2.edges.iter().all(|e| e.id != "m2"));
        assert_eq!(g2.edges.iter().find(|e| e.id == "m3").unwrap().points[0].1, y("m3"));
    }

    #[test]
    fn round_trips_text() {
        let (s, _) = seq("participant a as Alice\na->>+b: hi\nloop again\nb-->>a: yo\nend\n");
        let t = to_text(&s);
        let lines: Vec<(usize, &str)> = t.lines().skip(1).enumerate().collect();
        let mut errs = vec![];
        let (s2, _) = parse(&lines, &mut errs);
        assert!(errs.is_empty(), "{:?}", errs);
        assert_eq!(s, s2);
    }
}

#[cfg(test)]
mod project_tests {
    use super::*;

    #[test]
    fn builds_from_the_demo_crate() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/demo");
        let p = crate::analyze_path(&root).unwrap();
        let opts = ViewOptions { depth: 3, ..Default::default() };
        let (s, note) = from_calls(&p, &opts);
        eprintln!("{}", to_text(&s));
        assert!(note.is_none());
        assert!(s.messages().count() > 2);
        let g = view(&p, &opts);
        assert!(g.nodes.iter().any(|n| n.id.starts_with('a') && n.item.is_some()));
        // every message arrow goes down the page in order
        let ys: Vec<f64> = g.edges.iter().filter(|e| e.id.starts_with('m')).map(|e| e.points[0].1).collect();
        assert!(ys.windows(2).all(|w| w[0] < w[1]), "{:?}", ys);
    }
}

#[cfg(test)]
mod frame_tests {
    use super::*;

    #[test]
    fn a_comment_above_a_loop_titles_its_frame() {
        let src = "fn main() {\n    // send every task\n    for t in 0..3 {\n        send(t);\n    }\n    if true {\n        send(9);\n    }\n}\nfn send(_t: u32) {}\n";
        let p = crate::analyze_source(src);
        let (s, _) = from_calls(&p, &ViewOptions::default());
        let opens: Vec<(&str, &str)> = s.steps.iter().filter_map(|st| if let Step::Open { kind, label } = st { Some((kind.as_str(), label.as_str())) } else { None }).collect();
        assert_eq!(opens, [("loop", "send every task"), ("alt", "if true")]);
        let g = graph(&s, &[], Mode::Sequence);
        let f = g.nodes.iter().find(|n| n.container && n.sublabel == "loop").unwrap();
        assert_eq!(f.label, "send every task");
    }
}
