//! Codecast scripts: spoken text with references to what to show and highlight.
//!
//! Plain-text format (see docs/codecast.md):
//!
//! ```text
//! # Title                     script title (first line)
//! audio: codecast.mp3        optional recorded voice
//!
//! ## Part name                a part; users can play one part alone
//! @ flow:crate::run           show <view>[:<ref>]  (sticky)
//! ! crate::run/b1 crate::run  highlight refs       (sticky; `!` alone clears)
//! = src/main.rs:49:18-44      underline code       (this cue only)
//! [9.4]                       start time in seconds (audio mode)
//! Spoken sentence.            every plain line is one cue; the directives above attach to it
//! ```

use crate::model::*;
use crate::views::Mode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Script {
    pub title: String,
    pub audio: Option<String>,
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Part {
    pub name: String,
    pub cues: Vec<Cue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Cue {
    pub say: String,
    /// `view[:ref]`
    pub show: Option<String>,
    /// highlight refs; `Some(vec![])` clears, `None` keeps the previous highlight
    pub hl: Option<Vec<String>>,
    /// code ref to underline
    pub code: Option<String>,
    pub t: Option<f64>,
}

impl Script {
    pub fn cue_count(&self) -> usize {
        self.parts.iter().map(|p| p.cues.len()).sum()
    }
}

// ------------------------------------------------------------------ text format

/// Parse the text format. Never fails: unknown lines become spoken text.
pub fn parse(text: &str) -> Script {
    let mut s = Script::default();
    let mut part = Part { name: "Codecast".into(), cues: vec![] };
    let mut pending = Cue::default();
    for raw in text.lines() {
        let line = raw.trim_end();
        let t = line.trim_start();
        if t.is_empty() {
            continue;
        }
        if let Some(rest) = t.strip_prefix("## ") {
            if !part.cues.is_empty() || !s.parts.is_empty() {
                s.parts.push(std::mem::take(&mut part));
            }
            part = Part { name: rest.trim().to_string(), cues: vec![] };
            continue;
        }
        if let Some(rest) = t.strip_prefix("# ") {
            if s.title.is_empty() {
                s.title = rest.trim().to_string();
                continue;
            }
        }
        if let Some(rest) = t.strip_prefix("audio:") {
            s.audio = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = t.strip_prefix('@') {
            pending.show = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = t.strip_prefix('!') {
            pending.hl = Some(rest.split_whitespace().map(|r| r.to_string()).collect());
            continue;
        }
        if let Some(rest) = t.strip_prefix('=') {
            pending.code = Some(rest.trim().to_string());
            continue;
        }
        if t.starts_with('[') {
            if let Some(end) = t.find(']') {
                if let Ok(v) = t[1..end].trim().parse::<f64>() {
                    pending.t = Some(v);
                    let rest = t[end + 1..].trim();
                    if rest.is_empty() {
                        continue;
                    }
                    pending.say = rest.to_string();
                    part.cues.push(std::mem::take(&mut pending));
                    continue;
                }
            }
        }
        if t.starts_with("//") {
            continue;
        }
        pending.say = t.to_string();
        part.cues.push(std::mem::take(&mut pending));
    }
    if !part.cues.is_empty() || s.parts.is_empty() {
        s.parts.push(part);
    }
    if s.title.is_empty() {
        s.title = "Codecast".into();
    }
    s
}

pub fn to_text(s: &Script) -> String {
    let mut out = format!("# {}\n", s.title);
    if let Some(a) = &s.audio {
        out.push_str(&format!("audio: {}\n", a));
    }
    for p in &s.parts {
        out.push_str(&format!("\n## {}\n", p.name));
        for c in &p.cues {
            if let Some(v) = &c.show {
                out.push_str(&format!("@ {}\n", v));
            }
            if let Some(h) = &c.hl {
                out.push_str(&format!("! {}\n", h.join(" ")));
            }
            if let Some(cd) = &c.code {
                out.push_str(&format!("= {}\n", cd));
            }
            let say = flatten(&c.say);
            match c.t {
                Some(t) => out.push_str(&format!("[{}] {}\n", t, say)),
                None => out.push_str(&format!("{}\n", say)),
            }
        }
    }
    out
}

/// One-line form of a cue text: fenced code blocks become inline code, newlines become spaces.
fn flatten(say: &str) -> String {
    let mut out = String::new();
    let mut in_code = false;
    let mut code = String::new();
    for line in say.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            if in_code {
                let c: String = code.split_whitespace().collect::<Vec<_>>().join(" ");
                if !c.is_empty() {
                    out.push_str(&format!("`{}` ", c));
                }
                code.clear();
            }
            in_code = !in_code;
            continue;
        }
        if in_code {
            code.push_str(line);
            code.push(' ');
        } else if !t.is_empty() {
            out.push_str(t);
            out.push(' ');
        }
    }
    out.trim().to_string()
}

// ------------------------------------------------------------------ refs

/// What a ref points at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Resolved {
    pub span: Span,
    pub item: Option<ItemId>,
    /// set when the ref names a module
    pub module: Option<ModuleId>,
    /// true for block / line refs (highlight what is inside the span), false for whole items
    pub inner: bool,
}

pub fn view_of(name: &str) -> Option<Mode> {
    Some(match name {
        "code" => Mode::Code,
        "calls" | "calltree" => Mode::CallTree,
        "flow" | "cfg" => Mode::ControlFlow,
        "arch" | "architecture" => Mode::Architecture,
        "branches" | "branch" => Mode::BranchTree,
        "structure" => Mode::Structure,
        "types" => Mode::Types,
        "errors" | "error" => Mode::ErrorFlow,
        _ => return None,
    })
}

pub fn view_name(m: Mode) -> &'static str {
    match m {
        Mode::Code => "code",
        Mode::CallTree => "calls",
        Mode::ControlFlow => "flow",
        Mode::Architecture => "arch",
        Mode::BranchTree => "branches",
        Mode::Structure => "structure",
        Mode::Types => "types",
        Mode::ErrorFlow => "errors",
    }
}

/// Split `view[:ref]`.
pub fn parse_show(s: &str) -> (Option<Mode>, Option<&str>) {
    match s.split_once(':') {
        Some((v, r)) => (view_of(v.trim()), Some(r.trim()).filter(|r| !r.is_empty())),
        None => (view_of(s.trim()), None),
    }
}

fn find_item(p: &Project, path: &str) -> Option<ItemId> {
    let path = path.trim();
    if let Some(id) = p.find_by_path(path) {
        return Some(id);
    }
    // tolerate missing `crate::` prefix and `Type::method` / bare names
    let with = format!("crate::{}", path);
    if let Some(id) = p.find_by_path(&with) {
        return Some(id);
    }
    let mut cands = p.items.iter().filter(|i| i.path.ends_with(&format!("::{}", path)));
    let first = cands.next()?;
    if cands.next().is_none() {
        Some(first.id)
    } else {
        None
    }
}

/// Resolve a ref against the project.
pub fn resolve(p: &Project, r: &str) -> Option<Resolved> {
    let r = r.trim();
    // whole file (any file of the repository)
    if !r.contains(':') && !r.contains("::") {
        if let Some(f) = p.files.iter().find(|f| f.path == r || f.path.ends_with(&format!("/{}", r))) {
            let n = f.content.lines().count().max(1);
            return Some(Resolved { span: Span { file: f.id, line_start: 1, col_start: 1, line_end: n, col_end: 1 }, item: None, module: None, inner: false });
        }
    }
    if r.ends_with(".rs") && !r.contains(':') {
        let f = p.files.iter().find(|f| f.path == r || f.path.ends_with(&format!("/{}", r)))?;
        let n = f.content.lines().count().max(1);
        return Some(Resolved { span: Span { file: f.id, line_start: 1, col_start: 1, line_end: n, col_end: 1 }, item: None, module: None, inner: false });
    }
    // file:line[-line][:c1-c2]
    if let Some((file, rest)) = r.split_once(':') {
        if file.ends_with(".rs") {
            let f = p.files.iter().find(|f| f.path == file || f.path.ends_with(&format!("/{}", file)))?;
            let mut parts = rest.split(':');
            let lines = parts.next()?;
            let (l1, l2) = match lines.split_once('-') {
                Some((a, b)) => (a.parse().ok()?, b.parse().ok()?),
                None => {
                    let l: usize = lines.parse().ok()?;
                    (l, l)
                }
            };
            let (c1, c2) = match parts.next().and_then(|c| c.split_once('-')) {
                Some((a, b)) => (a.parse().unwrap_or(1), b.parse().unwrap_or(1)),
                None => (1, 1),
            };
            let cols = c2 > 1;
            return Some(Resolved {
                span: Span { file: f.id, line_start: l1, col_start: c1, line_end: l2, col_end: if cols { c2 } else { 1 } },
                item: p.items.iter().filter(|i| i.span.file == f.id && i.span.contains_line(l1) && i.is_callable()).map(|i| i.id).next(),
                module: None,
                inner: true,
            });
        }
    }
    // item[/b<n>]
    let (path, block) = match r.rsplit_once("/b") {
        Some((pth, n)) if n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty() => (pth, n.parse::<usize>().ok()),
        _ => (r, None),
    };
    let Some(id) = find_item(p, path) else {
        // a module?
        let m = p.modules.iter().find(|m| m.path == r || m.path == format!("crate::{}", r) || (r != "crate" && m.path.ends_with(&format!("::{}", r))))?;
        let span = m.span.or_else(|| m.file.map(|f| Span { file: f, line_start: 1, col_start: 1, line_end: p.files[f].content.lines().count().max(1), col_end: 1 }))?;
        return Some(Resolved { span, item: None, module: Some(m.id), inner: false });
    };
    let it = p.item(id);
    match block {
        Some(b) => {
            let root = it.fn_info()?.blocks.as_ref()?;
            fn find(b: &Block, id: usize) -> Option<&Block> {
                if b.id == id {
                    return Some(b);
                }
                b.children.iter().find_map(|c| find(c, id))
            }
            let blk = find(root, b)?;
            Some(Resolved { span: blk.span, item: Some(id), module: None, inner: true })
        }
        None => Some(Resolved { span: it.span, item: Some(id), module: None, inner: false }),
    }
}

/// All refs in a script that do not resolve, with their location.
pub fn check(p: &Project, s: &Script) -> Vec<String> {
    let mut bad = Vec::new();
    for part in &s.parts {
        for (i, c) in part.cues.iter().enumerate() {
            let at = format!("{} / cue {}", part.name, i + 1);
            if let Some(show) = &c.show {
                let (v, r) = parse_show(show);
                if v.is_none() {
                    bad.push(format!("{}: unknown view in `@ {}`", at, show));
                }
                if let Some(r) = r {
                    if resolve(p, r).is_none() {
                        bad.push(format!("{}: unresolved ref `{}` in `@`", at, r));
                    }
                }
            }
            for r in c.hl.iter().flatten() {
                if resolve(p, r).is_none() {
                    bad.push(format!("{}: unresolved ref `{}` in `!`", at, r));
                }
            }
            if let Some(r) = &c.code {
                if resolve(p, r).is_none() {
                    bad.push(format!("{}: unresolved ref `{}` in `=`", at, r));
                }
            }
        }
    }
    bad
}

// ------------------------------------------------------------------ built-in guide as a script

/// Turn the generated tour into a script: one part per chapter (`GuideStep::part`).
pub fn from_guide(p: &Project, steps: &[crate::guide::GuideStep]) -> Script {
    let mut s = Script { title: format!("Tour of {}", p.name), audio: None, parts: vec![] };
    let mut part = Part { name: String::new(), cues: vec![] };
    let mut last_show: Option<String> = None;
    for st in steps {
        if st.part != part.name {
            if !part.cues.is_empty() {
                s.parts.push(std::mem::take(&mut part));
            }
            part.name = st.part.clone();
        }
        let show = format!("{}{}", view_name(st.mode), st.focus_item.map(|f| format!(":{}", p.item(f).path)).unwrap_or_default());
        let mut hl: Vec<String> = st.refs.clone();
        if hl.is_empty() {
            if let Some(sp) = st.span {
                let f = &p.files[sp.file].path;
                hl.push(if sp.line_start == sp.line_end { format!("{}:{}", f, sp.line_start) } else { format!("{}:{}-{}", f, sp.line_start, sp.line_end) });
            } else {
                for h in &st.highlight_items {
                    hl.push(p.item(*h).path.clone());
                }
            }
        }
        let say = if st.text.trim().is_empty() { st.title.clone() } else { st.text.clone() };
        part.cues.push(Cue { say, show: if last_show.as_deref() == Some(&show) { None } else { Some(show.clone()) }, hl: Some(hl), code: None, t: None });
        last_show = Some(show);
    }
    if !part.cues.is_empty() {
        s.parts.push(part);
    }
    s
}

/// Spoken form of a cue: markdown and code blocks stripped.
pub fn speakable(say: &str) -> String {
    let mut out = String::new();
    let mut in_code = false;
    for line in say.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        let t = t.trim_start_matches("• ").trim_start_matches("- ");
        let cleaned: String = t.replace("**", "").replace('`', "");
        if !cleaned.is_empty() {
            out.push_str(&cleaned);
            out.push(' ');
        }
    }
    out.trim().to_string()
}
