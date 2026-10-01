//! Codecast scripts: spoken text with references to what to show and highlight.
//!
//! Markdown-compatible text format (see docs/codecast.md). One file (`codecast.md`) or a
//! `codecast/` directory with `index.md` plus one file per part (`01-welcome.md`, …; see [`assemble`]).
//!
//! ```text
//! # Title                     script title (first line)
//! audio: codecast.mp3        optional recorded voice for the whole script
//!
//! ## Part name                a part; users can play one part alone
//! audio: 01-welcome.mp3       optional recording of this part only ([t] times are then relative to it)
//! @ flow:crate::run           show <view>[:<ref>]  (sticky)
//! ! crate::run/b1 crate::run  highlight refs       (sticky; `!` alone clears)
//! = src/main.rs:49:18-44      underline code       (this cue only)
//! [9.4]                       start time in seconds (audio mode)
//! Spoken sentence.            every plain line is one cue; the directives above attach to it
//! ? What is a crate?          a question the listener can click (pauses the codecast)
//!   The answer, indented.     answer lines; may hold their own `@` `!` `=` directives
//! ```
//!
//! `glossary: off` in the header turns the built-in glossary questions off for this script
//! (see [`crate::glossary`]); `pronounce: Dioxus = dee ox us` fixes how TTS says a word
//! (see [`crate::speech`]).

use crate::model::*;
use crate::views::Mode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Script {
    pub title: String,
    pub audio: Option<String>,
    /// `glossary: off` → false: no automatic questions from the built-in glossary
    #[serde(default = "yes")]
    pub glossary: bool,
    /// `pronounce: written = spoken` header lines (see [`crate::speech`])
    #[serde(default)]
    pub pronounce: Vec<(String, String)>,
    pub parts: Vec<Part>,
}

fn yes() -> bool {
    true
}

impl Default for Script {
    fn default() -> Self {
        Script { title: String::new(), audio: None, glossary: true, pronounce: vec![], parts: vec![] }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Part {
    pub name: String,
    /// Recording of this part alone (relative to the script's directory); overrides `Script::audio`.
    pub audio: Option<String>,
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
    /// questions offered while this cue is shown (`?` lines)
    #[serde(default)]
    pub questions: Vec<Question>,
}

/// A question the listener can click. Answering pauses the codecast, shows and speaks the
/// answer (applying its directives), then resumes. Written in the script with `?`, or taken
/// from a glossary when the cue text mentions the term.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Question {
    pub ask: String,
    /// markdown; spoken with [`speakable`]
    pub answer: String,
    pub show: Option<String>,
    pub hl: Option<Vec<String>>,
    pub code: Option<String>,
    /// glossary term this came from (`None` for questions written in the script)
    #[serde(default)]
    pub term: Option<String>,
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
    let mut part = Part { name: "Codecast".into(), audio: None, cues: vec![] };
    let mut pending = Cue::default();
    let mut in_part = false;
    // a `?` block being read: it goes to the previous cue when no directive is pending for the
    // next one, else to the next cue
    let mut question: Option<(Question, bool)> = None;
    fn flush(q: &mut Option<(Question, bool)>, part: &mut Part, pending: &mut Cue) {
        if let Some((mut q, prev)) = q.take() {
            q.answer = q.answer.trim().to_string();
            match (prev, part.cues.last_mut()) {
                (true, Some(c)) => c.questions.push(q),
                _ => pending.questions.push(q),
            }
        }
    }
    for raw in text.lines() {
        let line = raw.trim_end();
        let t = line.trim_start();
        if t.is_empty() {
            continue;
        }
        let indented = line.starts_with(' ') || line.starts_with('\t');
        if let Some((q, _)) = question.as_mut() {
            if indented {
                // answer body; its own directives apply while the answer is shown
                if let Some(rest) = t.strip_prefix("@ ").or_else(|| t.strip_prefix('@').filter(|r| !r.starts_with(' '))) {
                    q.show = Some(rest.trim().to_string());
                } else if let Some(rest) = t.strip_prefix("! ").or_else(|| (t == "!").then_some("")) {
                    q.hl = Some(rest.split_whitespace().map(|r| r.to_string()).collect());
                } else if let Some(rest) = t.strip_prefix("= ") {
                    q.code = Some(rest.trim().to_string());
                } else if !t.starts_with("//") {
                    q.answer.push_str(t);
                    q.answer.push('\n');
                }
                continue;
            }
            flush(&mut question, &mut part, &mut pending);
        }
        if let Some(rest) = t.strip_prefix("? ").or_else(|| (t == "?").then_some("")) {
            let prev = pending == Cue::default() && !part.cues.is_empty();
            question = Some((Question { ask: rest.trim().to_string(), ..Default::default() }, prev));
            continue;
        }
        if let Some(rest) = t.strip_prefix("## ") {
            flush(&mut question, &mut part, &mut pending);
            if !part.cues.is_empty() || !s.parts.is_empty() {
                s.parts.push(std::mem::take(&mut part));
            }
            part = Part { name: rest.trim().to_string(), audio: None, cues: vec![] };
            in_part = true;
            continue;
        }
        if let Some(rest) = t.strip_prefix("# ") {
            // the first `#` heading is the title; later ones (e.g. one per part file) are ignored
            if s.title.is_empty() {
                s.title = rest.trim().to_string();
            }
            continue;
        }
        if let Some(rest) = t.strip_prefix("audio:") {
            let a = Some(rest.trim().to_string()).filter(|a| !a.is_empty());
            if in_part {
                part.audio = a;
            } else {
                s.audio = a;
            }
            continue;
        }
        if let Some(rest) = t.strip_prefix("glossary:") {
            if !in_part {
                s.glossary = !matches!(rest.trim().to_ascii_lowercase().as_str(), "off" | "no" | "false" | "none");
            }
            continue;
        }
        if t.starts_with("pronounce:") {
            s.pronounce.extend(crate::speech::parse_pronounce(t));
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
    flush(&mut question, &mut part, &mut pending);
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
    if !s.glossary {
        out.push_str("glossary: off\n");
    }
    for (a, b) in &s.pronounce {
        out.push_str(&format!("pronounce: {} = {}\n", a, b));
    }
    for p in &s.parts {
        out.push_str(&format!("\n## {}\n", p.name));
        if let Some(a) = &p.audio {
            out.push_str(&format!("audio: {}\n", a));
        }
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
            for q in &c.questions {
                out.push_str(&format!("? {}\n", q.ask));
                if let Some(v) = &q.show {
                    out.push_str(&format!("  @ {}\n", v));
                }
                if let Some(h) = &q.hl {
                    out.push_str(&format!("  ! {}\n", h.join(" ")));
                }
                if let Some(cd) = &q.code {
                    out.push_str(&format!("  = {}\n", cd));
                }
                for l in q.answer.lines() {
                    out.push_str(&format!("  {}\n", l));
                }
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

// ------------------------------------------------------------------ directory form

/// One part file of a `codecast/` directory.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PartFile {
    /// file name without extension, e.g. `01-welcome`
    pub stem: String,
    pub text: String,
    /// sibling recording with the same stem (`01-welcome.mp3`), if any
    pub audio: Option<String>,
}

/// Part name from a file stem: `01-how_it-runs` → `How it runs`.
pub fn part_name_from_stem(stem: &str) -> String {
    let rest = stem.trim_start_matches(|c: char| c.is_ascii_digit()).trim_start_matches(['-', '_', ' ', '.']);
    let rest = if rest.is_empty() { stem } else { rest };
    let words = rest.replace(['-', '_'], " ");
    let mut c = words.trim().chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => "Part".into(),
    }
}

/// Order the part files of a directory: the order listed in `index` (markdown links or list items
/// naming `.md` files), then the rest sorted by name. `index.md` itself is never a part.
pub fn order_parts(index: &str, stems: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in index.lines() {
        let t = line.trim();
        let Some(body) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| t.split_once(". ").filter(|(n, _)| n.chars().all(|c| c.is_ascii_digit())).map(|(_, b)| b)) else { continue };
        let target = match (body.rfind("](" ), body.ends_with(')')) {
            (Some(i), true) => &body[i + 2..body.len() - 1],
            _ => body.trim_matches('`'),
        };
        let stem = target.trim().trim_start_matches("./").trim_end_matches(".md").to_string();
        if stems.contains(&stem) && !out.contains(&stem) {
            out.push(stem);
        }
    }
    let mut rest: Vec<&String> = stems.iter().filter(|s| !out.contains(s) && s.as_str() != "index").collect();
    rest.sort();
    out.extend(rest.into_iter().cloned());
    out
}

/// Lines of `index.md` that are not the part list (title, `audio:`, intro cues).
fn index_body(index: &str, stems: &[String]) -> String {
    index
        .lines()
        .filter(|line| {
            let t = line.trim();
            let listed = |target: &str| stems.contains(&target.trim().trim_start_matches("./").trim_end_matches(".md").to_string());
            if let Some(body) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
                let target = match (body.rfind("](" ), body.ends_with(')')) {
                    (Some(i), true) => &body[i + 2..body.len() - 1],
                    _ => body.trim_matches('`'),
                };
                return !listed(target);
            }
            true
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Join `index.md` and the part files into one script text. Each part file becomes a `##` part:
/// its first `##` heading names it, otherwise the file name does; a sibling recording becomes its
/// `audio:` line unless the file sets one.
pub fn assemble(index: &str, parts: &[PartFile]) -> String {
    let stems: Vec<String> = parts.iter().map(|p| p.stem.clone()).collect();
    let order = order_parts(index, &stems);
    let mut out = index_body(index, &stems).trim_end().to_string();
    out.push('\n');
    for stem in order {
        let Some(pf) = parts.iter().find(|p| p.stem == stem) else { continue };
        let mut body: Vec<String> = pf.text.lines().map(|l| l.trim_end().to_string()).skip_while(|l| l.trim().is_empty() || l.trim_start().starts_with("# ")).collect();
        let has_heading = body.first().map(|l| l.trim_start().starts_with("## ")).unwrap_or(false);
        if !has_heading {
            body.insert(0, format!("## {}", part_name_from_stem(&pf.stem)));
        }
        if let Some(a) = &pf.audio {
            if !body.iter().any(|l| l.trim_start().starts_with("audio:")) {
                body.insert(1, format!("audio: {}", a));
            }
        }
        out.push('\n');
        out.push_str(&body.join("\n"));
        out.push('\n');
    }
    out
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
            for q in &c.questions {
                let at = format!("{} / `? {}`", at, q.ask);
                if q.ask.is_empty() {
                    bad.push(format!("{}: question without text", at));
                }
                if q.answer.trim().is_empty() {
                    bad.push(format!("{}: question without an answer (indent the answer lines)", at));
                }
                if let Some(show) = &q.show {
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
                for r in q.hl.iter().flatten() {
                    if resolve(p, r).is_none() {
                        bad.push(format!("{}: unresolved ref `{}` in `!`", at, r));
                    }
                }
                if let Some(r) = &q.code {
                    if resolve(p, r).is_none() {
                        bad.push(format!("{}: unresolved ref `{}` in `=`", at, r));
                    }
                }
            }
        }
    }
    bad
}

// ------------------------------------------------------------------ built-in guide as a script

/// Turn the generated tour into a script: one part per chapter (`GuideStep::part`).
pub fn from_guide(p: &Project, steps: &[crate::guide::GuideStep]) -> Script {
    let mut s = Script { title: format!("Tour of {}", p.name), ..Default::default() };
    let mut part = Part { name: String::new(), audio: None, cues: vec![] };
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
        part.cues.push(Cue { say, show: if last_show.as_deref() == Some(&show) { None } else { Some(show.clone()) }, hl: Some(hl), code: None, t: None, questions: vec![] });
        last_show = Some(show);
    }
    if !part.cues.is_empty() {
        s.parts.push(part);
    }
    s
}

/// Spoken form of a cue: markdown and code blocks stripped, code tokens said as words
/// ([`crate::speech::spoken`] with the built-in pronunciations). Players with a script's own
/// `pronounce:` list call `speech::spoken` directly.
pub fn speakable(say: &str) -> String {
    crate::speech::spoken(say, &crate::speech::builtin_pronounce())
}

/// Markdown stripped, code left as written (for matching glossary terms).
pub fn plain(say: &str) -> String {
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
