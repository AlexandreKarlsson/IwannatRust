//! Diagrams drawn by a codecast: a subset of mermaid flowcharts, written in a ```` ```mermaid ````
//! fenced block inside a part (so GitHub renders it too). The script shows one with `@ diagram`
//! and glows its nodes with `!`: node ids are the refs. Laid out with [`crate::layout::layered`],
//! groups (`subgraph`) nest.
//!
//! ```text
//! graph LR                        direction: LR / TD (TB) / RL / BT
//! a[Box] --> b(Round) --> c{Diamond}
//! d([Pill]) -.-> e[(Store)]       dotted; `==>` thick; `---` a line without arrow head
//! a -->|label| d                  edge label (also `a -- label --> d`)
//! subgraph bus[🚌 Bus]            a group; nodes mentioned inside belong to it
//!   p1[🧍 Alice]
//! end
//! %% comment
//! ```
//!
//! Shapes pick the colour (box blue, round green, pill purple, diamond yellow, store teal), so a
//! writer can tell kinds of things apart without a legend. Emoji in labels are plain text.

use crate::layout::{self, LayoutEdge, LayoutNode, LayoutOptions, Rect};
use crate::views::{EdgeKind, Graph, Mode, NodeKind, VEdge, VNode};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    /// `[text]` (also `[[text]]`, `>text]`, `[/text/]`)
    Box,
    /// `(text)`
    Round,
    /// `([text])`, `((text))`
    Pill,
    /// `{text}`, `{{text}}`
    Diamond,
    /// `[(text)]`
    Cylinder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Line {
    Solid,
    Dotted,
    Thick,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DNode {
    pub id: String,
    pub label: String,
    pub shape: Shape,
    /// a `subgraph`: drawn as a box behind its members
    pub group: bool,
    /// the group this node was mentioned in
    pub parent: Option<String>,
    /// `direction LR|TD` inside a group
    pub horizontal: Option<bool>,
    /// `id:::hidden`: not drawn until a `> +id` line shows it
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub line: Line,
    pub arrow: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Diagram {
    /// `title:` of the front matter (`@ diagram:<title>` picks it)
    pub title: Option<String>,
    /// `graph LR` / `RL`; `TD` / `TB` / `BT` are vertical
    pub horizontal: bool,
    pub nodes: Vec<DNode>,
    pub edges: Vec<DEdge>,
    /// lines the parser did not understand (`iwr check` reports them)
    pub errors: Vec<String>,
    /// the block as written (kept for [`crate::script::to_text`])
    pub source: String,
}

impl Diagram {
    pub fn node(&self, id: &str) -> Option<&DNode> {
        self.nodes.iter().find(|n| n.id == id)
    }
    pub fn has(&self, id: &str) -> bool {
        self.node(id).is_some()
    }
    /// Node ids, for error messages.
    pub fn ids(&self) -> Vec<&str> {
        self.nodes.iter().map(|n| n.id.as_str()).collect()
    }

    fn touch(&mut self, id: String, shape: Option<Shape>, label: Option<String>, group: Option<&String>) {
        if let Some(n) = self.nodes.iter_mut().find(|n| n.id == id) {
            if let Some(s) = shape {
                n.shape = s;
            }
            if let Some(l) = label {
                n.label = l;
            }
            if let Some(g) = group {
                if n.parent.is_none() && *g != n.id {
                    n.parent = Some(g.clone());
                }
            }
            return;
        }
        self.nodes.push(DNode { label: label.unwrap_or_else(|| id.clone()), id, shape: shape.unwrap_or(Shape::Box), group: false, parent: group.cloned(), horizontal: None, hidden: false });
    }
}

// ------------------------------------------------------------------ ops: `>` lines

/// What a `>` line of a cue changes in the diagram, from that cue on. The player replays the
/// ops of a part from its first cue, so stepping back and forth stays consistent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    /// `> a b`: `a` moves into group `b`, or next to node `b`; `> a`: back to the top level
    Move { node: String, to: Option<String> },
    /// `> +a`: draw a node that was hidden (`a:::hidden`, or `> -a`)
    Show(String),
    /// `> -a`: hide a node (and, for a group, its members)
    Hide(String),
    /// a `>` line as written that could not be read (`iwr check` reports it)
    Bad(String),
}

/// Parse the text after `>`: `node group`, `node other`, `node`, or `+a -b …`.
pub fn parse_ops(rest: &str) -> Vec<Op> {
    let words: Vec<&str> = rest.split_whitespace().collect();
    if !words.is_empty() && words.iter().all(|w| w.len() > 1 && (w.starts_with('+') || w.starts_with('-'))) {
        return words.iter().map(|w| if let Some(a) = w.strip_prefix('+') { Op::Show(a.to_string()) } else { Op::Hide(w[1..].to_string()) }).collect();
    }
    match words.as_slice() {
        [a] => vec![Op::Move { node: a.to_string(), to: None }],
        [a, b] => vec![Op::Move { node: a.to_string(), to: Some(b.to_string()) }],
        _ => vec![Op::Bad(rest.trim().to_string())],
    }
}

/// The text after `>` for an op (inverse of [`parse_ops`]).
pub fn op_text(op: &Op) -> String {
    match op {
        Op::Move { node, to: Some(t) } => format!("{} {}", node, t),
        Op::Move { node, to: None } => node.clone(),
        Op::Show(a) => format!("+{}", a),
        Op::Hide(a) => format!("-{}", a),
        Op::Bad(s) => s.clone(),
    }
}

// ------------------------------------------------------------------ parsing

const OTHER_DIAGRAMS: [&str; 12] = ["sequencediagram", "classdiagram", "statediagram", "erdiagram", "gantt", "pie", "journey", "gitgraph", "mindmap", "timeline", "quadrantchart", "xychart"];
const SKIPPED: [&str; 7] = ["classdef ", "class ", "style ", "linkstyle ", "click ", "acctitle", "accdescr"];

/// Parse a mermaid flowchart. Never fails: lines it cannot read go to [`Diagram::errors`].
pub fn parse(text: &str) -> Diagram {
    let mut d = Diagram { source: text.trim_end().to_string(), ..Default::default() };
    let mut body: Vec<(usize, &str)> = text.lines().enumerate().collect();
    // front matter: --- title: … ---
    if body.first().map(|(_, l)| l.trim() == "---").unwrap_or(false) {
        if let Some(end) = body.iter().skip(1).position(|(_, l)| l.trim() == "---") {
            for (_, l) in &body[1..=end] {
                if let Some(t) = l.trim().strip_prefix("title:") {
                    d.title = Some(t.trim().trim_matches('"').to_string()).filter(|t| !t.is_empty());
                }
            }
            body.drain(..=end + 1);
        }
    }
    let mut groups: Vec<String> = Vec::new();
    let mut header = false;
    for (ln, raw) in body {
        let line = raw.split("%%").next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        for stmt in split_statements(line) {
            let stmt = stmt.trim();
            if stmt.is_empty() {
                continue;
            }
            let lower = stmt.to_ascii_lowercase();
            if !header {
                header = true;
                if lower.starts_with("graph") || lower.starts_with("flowchart") {
                    let dir = stmt.split_whitespace().nth(1).unwrap_or("TD").to_ascii_uppercase();
                    d.horizontal = matches!(dir.as_str(), "LR" | "RL");
                    continue;
                }
                if let Some(k) = OTHER_DIAGRAMS.iter().find(|k| lower.starts_with(*k)) {
                    d.errors.push(format!("line {}: `{}` is not supported, only flowcharts (`graph LR` / `graph TD`)", ln + 1, k));
                    return d;
                }
                // no header: a vertical flowchart
            }
            if let Some(rest) = keyword(stmt, "subgraph") {
                match parse_subgraph(rest) {
                    Ok((id, label)) => {
                        let parent = groups.last().cloned();
                        match d.nodes.iter_mut().find(|n| n.id == id) {
                            Some(n) => {
                                n.group = true;
                                if let Some(l) = label {
                                    n.label = l;
                                }
                            }
                            None => d.nodes.push(DNode { label: label.unwrap_or_else(|| id.clone()), id: id.clone(), shape: Shape::Box, group: true, parent, horizontal: None, hidden: false }),
                        }
                        groups.push(id);
                    }
                    Err(e) => d.errors.push(format!("line {}: {}", ln + 1, e)),
                }
                continue;
            }
            if lower == "end" {
                if groups.pop().is_none() {
                    d.errors.push(format!("line {}: `end` without a subgraph", ln + 1));
                }
                continue;
            }
            if let Some(rest) = keyword(stmt, "direction") {
                let h = matches!(rest.trim().to_ascii_uppercase().as_str(), "LR" | "RL");
                match groups.last() {
                    Some(g) => {
                        if let Some(n) = d.nodes.iter_mut().find(|n| &n.id == g) {
                            n.horizontal = Some(h);
                        }
                    }
                    None => d.horizontal = h,
                }
                continue;
            }
            if SKIPPED.iter().any(|k| lower.starts_with(k)) {
                continue;
            }
            if let Err(e) = parse_statement(stmt, &mut d, groups.last()) {
                d.errors.push(format!("line {}: {} in `{}`", ln + 1, e, stmt));
            }
        }
    }
    if !groups.is_empty() {
        d.errors.push(format!("subgraph `{}` without `end`", groups.last().unwrap()));
    }
    d
}

/// `keyword rest` → `Some(rest)` when the statement starts with the keyword as a whole word.
fn keyword<'a>(stmt: &'a str, kw: &str) -> Option<&'a str> {
    let rest = stmt.strip_prefix(kw)?;
    if rest.is_empty() || rest.starts_with(|c: char| c.is_whitespace()) {
        Some(rest.trim_start())
    } else {
        None
    }
}

/// Split a line at `;` outside quotes and brackets.
fn split_statements(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut quoted, mut start) = (0i32, false, 0usize);
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '[' | '(' | '{' if !quoted => depth += 1,
            ']' | ')' | '}' if !quoted => depth -= 1,
            ';' if !quoted && depth <= 0 => {
                out.push(&line[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&line[start..]);
    out
}

fn is_id_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn parse_subgraph(rest: &str) -> Result<(String, Option<String>), String> {
    let rest = rest.trim();
    if rest.is_empty() {
        return Err("subgraph without a name".into());
    }
    if let Some(q) = rest.strip_prefix('"') {
        let title = q.trim_end_matches('"').to_string();
        return Ok((title.clone(), Some(title)));
    }
    let cs: Vec<char> = rest.chars().collect();
    let mut i = 0;
    let id = read_id(&cs, &mut i);
    skip_ws(&cs, &mut i);
    if id.is_empty() {
        return Err(format!("bad subgraph name `{}`", rest));
    }
    if i >= cs.len() {
        return Ok((id, None));
    }
    if cs[i] == '[' {
        let (_, label) = read_shape(&cs, &mut i)?;
        return Ok((id, Some(label)));
    }
    // `subgraph Two words`: the whole text is the title and the id
    Ok((rest.to_string(), Some(rest.to_string())))
}

fn skip_ws(cs: &[char], i: &mut usize) {
    while *i < cs.len() && cs[*i].is_whitespace() {
        *i += 1;
    }
}

fn read_id(cs: &[char], i: &mut usize) -> String {
    let start = *i;
    while *i < cs.len() && is_id_char(cs[*i]) {
        *i += 1;
    }
    cs[start..*i].iter().collect()
}

fn is_open(c: char) -> bool {
    matches!(c, '(' | '[' | '{' | '>')
}
fn is_close(c: char) -> bool {
    matches!(c, ')' | ']' | '}' | '/' | '\\')
}
fn mirror(c: char) -> char {
    match c {
        '(' => ')',
        '[' | '>' => ']',
        _ => '}',
    }
}

/// `[label]`, `(label)`, `{label}`, `((label))`, `[(label)]`, `["quoted (label)"]`, … at `cs[i]`.
fn read_shape(cs: &[char], i: &mut usize) -> Result<(Shape, String), String> {
    let start = *i;
    let mut open = String::new();
    while *i < cs.len() && (is_open(cs[*i]) || (!open.is_empty() && matches!(cs[*i], '/' | '\\'))) && open.len() < 3 {
        open.push(cs[*i]);
        *i += 1;
    }
    let want = mirror(open.chars().next().unwrap_or('['));
    let label: String;
    if *i < cs.len() && cs[*i] == '"' {
        *i += 1;
        let s = *i;
        while *i < cs.len() && cs[*i] != '"' {
            *i += 1;
        }
        if *i >= cs.len() {
            return Err("unclosed quote".into());
        }
        label = cs[s..*i].iter().collect();
        *i += 1;
        while *i < cs.len() && is_close(cs[*i]) {
            *i += 1;
        }
    } else {
        let s = *i;
        let mut j = s;
        while j < cs.len() && cs[j] != want {
            j += 1;
        }
        if j >= cs.len() {
            let open_s: String = cs[start..s].iter().collect();
            return Err(format!("unclosed `{}`", open_s));
        }
        let mut k = j;
        while k > s && is_close(cs[k - 1]) {
            k -= 1;
        }
        label = cs[s..k].iter().collect();
        *i = j + 1;
        while *i < cs.len() && is_close(cs[*i]) {
            *i += 1;
        }
    }
    let shape = match open.as_str() {
        "((" | "(((" | "([" => Shape::Pill,
        "[(" => Shape::Cylinder,
        "(" => Shape::Round,
        "{" | "{{" => Shape::Diamond,
        _ => Shape::Box,
    };
    Ok((shape, clean_label(&label)))
}

fn clean_label(s: &str) -> String {
    let s = s.replace("<br/>", "\n").replace("<br />", "\n").replace("<br>", "\n").replace("#quot;", "\"");
    s.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

/// An arrow at `cs[i]`: `-->`, `---`, `-.->`, `==>`, `--x`, `<-->`, `-- text -->`, `-->|text|`.
fn read_edge(cs: &[char], i: &mut usize) -> Result<(Line, bool, Option<String>), String> {
    let run = read_run(cs, i);
    if run.is_empty() {
        let near: String = cs[*i..].iter().take(12).collect();
        return Err(format!("expected an arrow or `&` before `{}`", near.trim()));
    }
    let core = run.trim_start_matches('<');
    let mut label = None;
    if core.trim_end_matches(['>', 'x', 'o']).len() < 2 {
        return Err(format!("arrow `{}` too short: use `-->`, `---`, `-.->` or `==>`", run));
    }
    let run = if matches!(core, "--" | "-." | "==") {
        // `-- text -->`: the label sits between two runs
        skip_ws(cs, i);
        let s = *i;
        let mut j = s;
        while j < cs.len() && !(matches!(cs[j], '-' | '.' | '=') && is_run_end(cs, j)) {
            j += 1;
        }
        if j >= cs.len() {
            return Err("unfinished arrow (`-- text -->`)".into());
        }
        label = Some(cs[s..j].iter().collect::<String>().trim().to_string()).filter(|l| !l.is_empty());
        *i = j;
        read_run(cs, i)
    } else {
        run
    };
    let line = if run.contains('.') {
        Line::Dotted
    } else if run.contains('=') {
        Line::Thick
    } else {
        Line::Solid
    };
    let arrow = run.ends_with('>') || run.ends_with('x') || run.ends_with('o') || run.starts_with('<');
    skip_ws(cs, i);
    if *i < cs.len() && cs[*i] == '|' {
        *i += 1;
        let s = *i;
        while *i < cs.len() && cs[*i] != '|' {
            *i += 1;
        }
        if *i >= cs.len() {
            return Err("unclosed `|label|`".into());
        }
        label = Some(cs[s..*i].iter().collect::<String>().trim().to_string()).filter(|l| !l.is_empty());
        *i += 1;
    }
    Ok((line, arrow, label))
}

/// Does an arrow run starting at `j` end an inline label (`-->`, `---`, `.->`, `.-`, `==>`, `===`)?
fn is_run_end(cs: &[char], j: usize) -> bool {
    let mut k = j;
    while k < cs.len() && matches!(cs[k], '-' | '.' | '=' | '>') {
        k += 1;
    }
    k - j >= 2 && (k >= cs.len() || cs[k].is_whitespace() || is_id_char(cs[k]) || cs[k] == '|')
}

fn read_run(cs: &[char], i: &mut usize) -> String {
    let mut run = String::new();
    if *i < cs.len() && cs[*i] == '<' {
        run.push('<');
        *i += 1;
    }
    if *i >= cs.len() || !matches!(cs[*i], '-' | '=' | '.') {
        if !run.is_empty() {
            *i -= 1;
        }
        return String::new();
    }
    while *i < cs.len() {
        let c = cs[*i];
        let next_ok = *i + 1 >= cs.len() || !is_id_char(cs[*i + 1]);
        if matches!(c, '-' | '=' | '.' | '>') || (matches!(c, 'x' | 'o') && next_ok && !run.is_empty()) {
            run.push(c);
            *i += 1;
            if matches!(c, '>' | 'x' | 'o') {
                break;
            }
        } else {
            break;
        }
    }
    run
}

/// `a[Label] --> b & c -->|yes| d{Q}`: nodes, `&` sets, arrows between sets.
fn parse_statement(stmt: &str, d: &mut Diagram, group: Option<&String>) -> Result<(), String> {
    let cs: Vec<char> = stmt.chars().collect();
    let mut i = 0;
    let mut prev: Vec<String> = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    let mut edge: Option<(Line, bool, Option<String>)> = None;
    loop {
        skip_ws(&cs, &mut i);
        if i >= cs.len() {
            if edge.is_some() {
                return Err("arrow without a target node".into());
            }
            break;
        }
        let id = read_id(&cs, &mut i);
        if id.is_empty() {
            let near: String = cs[i..].iter().take(12).collect();
            return Err(format!("expected a node id before `{}` (letters, digits and _ only)", near.trim()));
        }
        let mut shape = None;
        let mut label = None;
        if i < cs.len() && is_open(cs[i]) {
            let (s, l) = read_shape(&cs, &mut i)?;
            shape = Some(s);
            label = Some(l);
        }
        let mut hidden = false;
        if i + 2 < cs.len() && cs[i] == ':' && cs[i + 1] == ':' && cs[i + 2] == ':' {
            i += 3;
            // `:::hidden` is ours; other class names are ignored
            hidden = read_id(&cs, &mut i) == "hidden";
        }
        d.touch(id.clone(), shape, label, group);
        if hidden {
            if let Some(n) = d.nodes.iter_mut().find(|n| n.id == id) {
                n.hidden = true;
            }
        }
        if let Some((line, arrow, lbl)) = &edge {
            for f in &prev {
                d.edges.push(DEdge { from: f.clone(), to: id.clone(), label: lbl.clone(), line: *line, arrow: *arrow });
            }
        }
        cur.push(id);
        skip_ws(&cs, &mut i);
        if i >= cs.len() {
            break;
        }
        if cs[i] == '&' {
            i += 1;
            continue;
        }
        edge = Some(read_edge(&cs, &mut i)?);
        prev = std::mem::take(&mut cur);
    }
    Ok(())
}

// ------------------------------------------------------------------ layout

const FONT: f64 = 13.0;
const PAD: f64 = 12.0;
const TITLE: f64 = 28.0;
const MARGIN: f64 = 24.0;

/// Characters wider than a Latin letter (emoji, CJK) count double; joiners and variation
/// selectors count nothing.
fn display_width(s: &str) -> f64 {
    s.chars()
        .map(|c| match c as u32 {
            0x200D | 0xFE0E | 0xFE0F | 0x1F3FB..=0x1F3FF => 0.0,
            u if u >= 0x2600 => 2.0,
            _ => 1.0,
        })
        .sum()
}

fn node_size(n: &DNode) -> (f64, f64) {
    let lines = n.label.lines().count().max(1) as f64;
    let wmax = n.label.lines().map(display_width).fold(0.0, f64::max);
    let mut w = wmax * FONT * 0.62 + 30.0;
    let mut h = 18.0 + 16.0 * lines;
    match n.shape {
        Shape::Diamond => {
            w = w * 1.4 + 10.0;
            h *= 1.5;
        }
        Shape::Pill => w += 12.0,
        _ => {}
    }
    (w.clamp(56.0, 360.0), h)
}

fn kind_of(n: &DNode) -> NodeKind {
    if n.group {
        return NodeKind::Group;
    }
    match n.shape {
        Shape::Box => NodeKind::Box,
        Shape::Round => NodeKind::Round,
        Shape::Pill => NodeKind::Pill,
        Shape::Diamond => NodeKind::Diamond,
        Shape::Cylinder => NodeKind::Cylinder,
    }
}

fn edge_kind(e: &DEdge) -> EdgeKind {
    match (e.line, e.arrow) {
        (Line::Solid, true) => EdgeKind::Arrow,
        (Line::Solid, false) => EdgeKind::Line,
        (Line::Dotted, true) => EdgeKind::Dotted,
        (Line::Dotted, false) => EdgeKind::DottedLine,
        (Line::Thick, true) => EdgeKind::Thick,
        (Line::Thick, false) => EdgeKind::ThickLine,
    }
}

struct Ctx<'a> {
    d: &'a Diagram,
    /// effective parent of every node (`None` = top level)
    parent: HashMap<&'a str, Option<&'a str>>,
    /// node → the sibling it was moved next to (a layout-only edge keeps them adjacent)
    next_to: HashMap<&'a str, &'a str>,
    visible: HashSet<&'a str>,
    size: HashMap<&'a str, (f64, f64)>,
    /// position relative to the parent's inner origin
    local: HashMap<&'a str, (f64, f64)>,
    /// edge index → (group it was routed in, local points)
    routes: HashMap<usize, (Option<&'a str>, Vec<(f64, f64)>)>,
}

impl<'a> Ctx<'a> {
    fn is_group(&self, id: &str) -> bool {
        self.d.node(id).map(|n| n.group).unwrap_or(false)
    }
    /// The member of `group` that contains `x` (or `x` itself); `None` when `x` is not inside.
    fn lift(&self, x: &'a str, group: Option<&str>) -> Option<&'a str> {
        if !self.visible.contains(x) {
            return None;
        }
        let mut cur = x;
        loop {
            let p = self.parent.get(cur).copied().flatten();
            if p == group {
                return Some(cur);
            }
            cur = p?;
        }
    }
    fn kids(&self, group: Option<&str>) -> Vec<&'a DNode> {
        self.d.nodes.iter().filter(|n| self.visible.contains(n.id.as_str()) && self.parent.get(n.id.as_str()).copied().flatten() == group).collect()
    }
    fn place(&mut self, group: Option<&'a str>, horizontal: bool) -> (f64, f64) {
        let kids = self.kids(group);
        if kids.is_empty() {
            return (60.0, 10.0);
        }
        let mut lnodes = Vec::new();
        for k in &kids {
            let (w, h) = if k.group {
                let (iw, ih) = self.place(Some(k.id.as_str()), k.horizontal.unwrap_or(horizontal));
                let tw = display_width(&k.label) * FONT * 0.62 + 30.0;
                ((iw + 2.0 * PAD).max(tw), ih + TITLE + PAD)
            } else {
                node_size(k)
            };
            self.size.insert(k.id.as_str(), (w, h));
            lnodes.push(LayoutNode { id: k.id.clone(), w, h });
        }
        let mut ledges = Vec::new();
        let mut direct = Vec::new();
        for (ei, e) in self.d.edges.iter().enumerate() {
            let (Some(a), Some(b)) = (self.lift(&e.from, group), self.lift(&e.to, group)) else { continue };
            if a == b {
                continue;
            }
            ledges.push(LayoutEdge { from: a.to_string(), to: b.to_string(), reverse_rank: false });
            if a == e.from && b == e.to && !self.is_group(a) && !self.is_group(b) {
                direct.push(ei);
            }
        }
        for (n, t) in &self.next_to {
            if self.lift(n, group) == Some(*n) && self.lift(t, group) == Some(*t) {
                ledges.push(LayoutEdge { from: t.to_string(), to: n.to_string(), reverse_rank: false });
            }
        }
        let opts = LayoutOptions { layer_gap: 56.0, node_gap: 22.0, margin: 0.0, horizontal, max_per_layer: 6 };
        let res = layout::layered(&lnodes, &ledges, &opts);
        for k in &kids {
            if let Some(r) = res.rects.get(k.id.as_str()) {
                self.local.insert(k.id.as_str(), (r.x, r.y));
            }
        }
        let mut used: HashSet<usize> = HashSet::new();
        for ei in direct {
            let e = &self.d.edges[ei];
            if let Some((ri, r)) = res.edges.iter().enumerate().find(|(ri, r)| !used.contains(ri) && r.from == e.from && r.to == e.to) {
                used.insert(ri);
                self.routes.insert(ei, (group, r.points.clone()));
            }
        }
        (res.width, res.height)
    }
}

/// Lay the diagram out as a [`Graph`]: node ids are the mermaid ids, groups are containers.
/// `ops` are the `>` lines played so far (in order); names they mention that do not exist are
/// ignored (`iwr check` reports them).
pub fn graph(d: &Diagram, ops: &[Op]) -> Graph {
    let mut g = Graph { mode: Some(Mode::Diagram), ..Default::default() };
    if d.nodes.is_empty() {
        g.note = Some("Empty diagram.".into());
        return g;
    }
    // parents: only an existing group counts, and never a cycle
    let mut parent: HashMap<&str, Option<&str>> = HashMap::new();
    for n in &d.nodes {
        let p = n.parent.as_deref().filter(|p| d.node(p).map(|x| x.group && x.id != n.id).unwrap_or(false));
        parent.insert(&n.id, p);
    }
    // `>` moves: into a group, or next to a node (same parent, laid out right after it)
    let mut next_to: HashMap<&str, &str> = HashMap::new();
    for op in ops {
        let Op::Move { node, to } = op else { continue };
        let Some(n) = d.node(node) else { continue };
        let n = n.id.as_str();
        match to.as_deref() {
            None => {
                parent.insert(n, None);
                next_to.remove(n);
            }
            Some(t) => match d.node(t) {
                None => continue,
                Some(t) if t.group => {
                    parent.insert(n, Some(t.id.as_str()));
                    next_to.remove(n);
                }
                Some(t) => {
                    let p = parent.get(t.id.as_str()).copied().flatten();
                    parent.insert(n, p);
                    next_to.insert(n, t.id.as_str());
                }
            },
        }
    }
    for n in &d.nodes {
        let mut seen = vec![n.id.as_str()];
        let mut cur = parent[n.id.as_str()];
        while let Some(p) = cur {
            if seen.contains(&p) {
                parent.insert(&n.id, None);
                break;
            }
            seen.push(p);
            cur = parent[p];
        }
    }
    // hidden: `:::hidden` nodes, then `> +a` / `> -a` in order; a hidden group hides its members
    let mut hidden: HashSet<&str> = d.nodes.iter().filter(|n| n.hidden).map(|n| n.id.as_str()).collect();
    for op in ops {
        match op {
            Op::Show(a) => {
                if let Some(n) = d.node(a) {
                    hidden.remove(n.id.as_str());
                }
            }
            Op::Hide(a) => {
                if let Some(n) = d.node(a) {
                    hidden.insert(n.id.as_str());
                }
            }
            _ => {}
        }
    }
    let visible: HashSet<&str> = d
        .nodes
        .iter()
        .filter(|n| {
            let mut cur = Some(n.id.as_str());
            while let Some(c) = cur {
                if hidden.contains(c) {
                    return false;
                }
                cur = parent.get(c).copied().flatten();
            }
            true
        })
        .map(|n| n.id.as_str())
        .collect();
    let mut ctx = Ctx { d, parent, next_to, visible, size: HashMap::new(), local: HashMap::new(), routes: HashMap::new() };
    let (w, h) = ctx.place(None, d.horizontal);
    // absolute positions
    let mut abs: HashMap<&str, (f64, f64)> = HashMap::new();
    let mut origin: HashMap<Option<&str>, (f64, f64)> = HashMap::new();
    origin.insert(None, (MARGIN, MARGIN));
    let mut stack: Vec<Option<&str>> = vec![None];
    while let Some(grp) = stack.pop() {
        let (ox, oy) = origin[&grp];
        for k in ctx.kids(grp) {
            let (lx, ly) = ctx.local.get(k.id.as_str()).copied().unwrap_or((0.0, 0.0));
            abs.insert(&k.id, (ox + lx, oy + ly));
            if k.group {
                origin.insert(Some(&k.id), (ox + lx + PAD, oy + ly + TITLE));
                stack.push(Some(&k.id));
            }
        }
    }
    fn depth_of(parent: &HashMap<&str, Option<&str>>, id: &str) -> usize {
        let mut n = 0;
        let mut cur = parent.get(id).copied().flatten();
        while let Some(p) = cur {
            n += 1;
            cur = parent.get(p).copied().flatten();
        }
        n
    }
    let mut nodes: Vec<VNode> = Vec::new();
    for n in &d.nodes {
        if !ctx.visible.contains(n.id.as_str()) {
            continue;
        }
        let (x, y) = abs[n.id.as_str()];
        let (w, h) = ctx.size[n.id.as_str()];
        nodes.push(VNode {
            id: n.id.clone(),
            label: n.label.clone(),
            sublabel: String::new(),
            kind: kind_of(n),
            item: None,
            cfg_node: None,
            span: None,
            x,
            y,
            w,
            h,
            expandable: false,
            expanded: false,
            badges: vec![],
            depth: depth_of(&ctx.parent, &n.id),
            parent: ctx.parent[n.id.as_str()].map(|p| p.to_string()),
            container: n.group,
        });
    }
    // containers first (outer before inner), then the rest, so the UI draws them behind
    nodes.sort_by_key(|n| (!n.container, n.depth));
    let rect = |id: &str| -> Option<Rect> {
        let (x, y) = abs.get(id)?;
        let (w, h) = ctx.size.get(id)?;
        Some(Rect { x: *x, y: *y, w: *w, h: *h })
    };
    for (ei, e) in d.edges.iter().enumerate() {
        let (Some(ra), Some(rb)) = (rect(&e.from), rect(&e.to)) else { continue };
        let points = match ctx.routes.get(&ei) {
            Some((grp, pts)) => {
                let (ox, oy) = origin.get(grp).copied().unwrap_or((MARGIN, MARGIN));
                pts.iter().map(|p| (p.0 + ox, p.1 + oy)).collect()
            }
            None => {
                let a = border(ra, (rb.cx(), rb.cy()));
                let b = border(rb, (ra.cx(), ra.cy()));
                vec![a, ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0), b]
            }
        };
        g.edges.push(VEdge { id: format!("e{}", ei), from: e.from.clone(), to: e.to.clone(), kind: edge_kind(e), label: e.label.clone(), points, backward: false });
    }
    g.nodes = nodes;
    g.width = w + 2.0 * MARGIN;
    g.height = h + 2.0 * MARGIN;
    g
}

/// Where the segment from the centre of `r` towards `to` leaves the rectangle.
fn border(r: Rect, to: (f64, f64)) -> (f64, f64) {
    let (cx, cy) = (r.cx(), r.cy());
    let (dx, dy) = (to.0 - cx, to.1 - cy);
    if dx.abs() < 1e-6 && dy.abs() < 1e-6 {
        return (cx, cy);
    }
    let tx = if dx.abs() > 1e-6 { (r.w / 2.0) / dx.abs() } else { f64::INFINITY };
    let ty = if dy.abs() > 1e-6 { (r.h / 2.0) / dy.abs() } else { f64::INFINITY };
    let t = tx.min(ty);
    (cx + dx * t, cy + dy * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shapes_edges_and_groups() {
        let d = parse("---\ntitle: Bus\n---\ngraph LR\n  subgraph stop[🚏 Stop]\n    p1[🧍 Alice]\n    p2([Bob])\n  end\n  stop --> bus[(🚌 Bus)] -->|arrives| office{Office?}\n  p1 -. waits .-> p2\n  a & b --- c; d ==> e\n  %% comment\n  classDef x fill:#f00\n");
        assert_eq!(d.title.as_deref(), Some("Bus"));
        assert!(d.horizontal);
        assert!(d.errors.is_empty(), "{:?}", d.errors);
        let stop = d.node("stop").unwrap();
        assert!(stop.group && stop.label == "🚏 Stop");
        assert_eq!(d.node("p1").unwrap().parent.as_deref(), Some("stop"));
        assert_eq!(d.node("p2").unwrap().shape, Shape::Pill);
        assert_eq!(d.node("bus").unwrap().shape, Shape::Cylinder);
        assert_eq!(d.node("office").unwrap().shape, Shape::Diamond);
        assert_eq!(d.node("office").unwrap().label, "Office?");
        let e: Vec<(&str, &str, Option<&str>, Line, bool)> = d.edges.iter().map(|e| (e.from.as_str(), e.to.as_str(), e.label.as_deref(), e.line, e.arrow)).collect();
        assert_eq!(e[0], ("stop", "bus", None, Line::Solid, true));
        assert_eq!(e[1], ("bus", "office", Some("arrives"), Line::Solid, true));
        assert_eq!(e[2], ("p1", "p2", Some("waits"), Line::Dotted, true));
        assert_eq!(e[3], ("a", "c", None, Line::Solid, false));
        assert_eq!(e[4], ("b", "c", None, Line::Solid, false));
        assert_eq!(e[5], ("d", "e", None, Line::Thick, true));
    }

    #[test]
    fn reports_bad_lines_and_keeps_going() {
        let d = parse("graph TD\na --> b\nc -->\nd -> e\nend\n");
        assert_eq!(d.edges.len(), 1);
        assert_eq!(d.errors.len(), 3, "{:?}", d.errors);
        assert!(d.errors[0].starts_with("line 3:"));
        let d = parse("sequenceDiagram\nA->>B: hi\n");
        assert_eq!(d.errors.len(), 1);
        assert!(parse("graph LR\nsubgraph g\na\n").errors[0].contains("without `end`"));
    }

    #[test]
    fn layout_nests_groups_and_routes_edges() {
        let d = parse("graph LR\nsubgraph stop[Stop]\n p1[Alice]\n p2[Bob]\nend\nstop --> bus[Bus]\np1 --> bus\n");
        let g = graph(&d, &[]);
        assert_eq!(g.nodes.len(), 4);
        let stop = g.node("stop").unwrap();
        let p1 = g.node("p1").unwrap();
        let bus = g.node("bus").unwrap();
        assert!(stop.container && !p1.container);
        assert!(p1.x >= stop.x && p1.y >= stop.y && p1.x + p1.w <= stop.x + stop.w && p1.y + p1.h <= stop.y + stop.h, "member inside its group");
        assert!(bus.x > stop.x + stop.w, "left to right");
        assert_eq!(g.edges.len(), 2);
        assert!(g.edges.iter().all(|e| e.points.len() >= 2));
        assert_eq!(g.nodes[0].id, "stop", "containers first");
        assert!(g.width > bus.x + bus.w && g.height > 0.0);
    }

    fn inside(g: &Graph, n: &str, grp: &str) -> bool {
        let (n, c) = (g.node(n).unwrap(), g.node(grp).unwrap());
        n.x >= c.x && n.y >= c.y && n.x + n.w <= c.x + c.w && n.y + n.h <= c.y + c.h
    }

    #[test]
    fn moves_change_parents() {
        let d = parse("graph LR\nsubgraph stop[Stop]\n alice[Alice]\nend\nsubgraph bus[Bus]\nend\nstop --> bus --> office[Office]\n");
        assert!(inside(&graph(&d, &[]), "alice", "stop"));
        let ops = parse_ops("alice bus");
        assert_eq!(ops, vec![Op::Move { node: "alice".into(), to: Some("bus".into()) }]);
        let g1 = graph(&d, &ops);
        assert!(inside(&g1, "alice", "bus") && !inside(&g1, "alice", "stop"));
        assert_eq!(g1.node("alice").unwrap().parent.as_deref(), Some("bus"));
        // next to a plain node: same level, laid out after it
        let g2 = graph(&d, &parse_ops("alice office"));
        let (a, o) = (g2.node("alice").unwrap(), g2.node("office").unwrap());
        assert!(a.parent.is_none() && a.x > o.x);
        // back to the top level; unknown names are ignored
        let g3 = graph(&d, &[parse_ops("alice bus"), parse_ops("alice"), parse_ops("nobody bus")].concat());
        assert!(g3.node("alice").unwrap().parent.is_none());
        assert_eq!(parse_ops("a b c"), vec![Op::Bad("a b c".into())]);
        assert_eq!(parse_ops(&op_text(&ops[0])), ops);
    }

    #[test]
    fn hidden_nodes_show_and_hide() {
        let d = parse("graph LR\nsubgraph stop[Stop]\n alice[Alice]\nend\ncopy[Copy]:::hidden\nstop --> copy\n");
        assert!(d.node("copy").unwrap().hidden && !d.node("alice").unwrap().hidden);
        let g = graph(&d, &[]);
        assert!(g.node("copy").is_none() && g.edges.is_empty(), "hidden node and its edge are not drawn");
        let ops = parse_ops("+copy -alice");
        assert_eq!(ops, vec![Op::Show("copy".into()), Op::Hide("alice".into())]);
        let g = graph(&d, &ops);
        assert!(g.node("copy").is_some() && g.node("alice").is_none() && g.edges.len() == 1);
        // hiding a group hides its members; showing it again brings them back
        let g = graph(&d, &parse_ops("-stop"));
        assert!(g.node("alice").is_none() && g.node("stop").is_none());
        let g = graph(&d, &[parse_ops("-stop"), parse_ops("+stop")].concat());
        assert!(g.node("alice").is_some());
        assert_eq!(parse_ops(&op_text(&ops[1])), vec![ops[1].clone()]);
        assert!(matches!(parse_ops("+")[0], Op::Move { .. }), "a lone `+` is a node name, not a toggle");
    }
}
