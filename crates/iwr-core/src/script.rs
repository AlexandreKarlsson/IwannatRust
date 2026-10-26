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
//! times: 0 4.2 9.8            start times of the part's cues in order (written by `iwr voice`); `[t]` wins
//! audio.ja: 01-welcome.ja.ogg recording of this part in another language (`iwr voice --lang ja`),
//! times.ja: 0 3.1 8.7         with its cue times; played when the codecast is played in that language
//! audio.en.am_adam: …         a recording by one Kokoro voice (`iwr voice` writes one per voice):
//! times.en.am_adam: …         the listener picks the voice in the player
//! Spoken sentence.            every plain line is one cue; the directives above attach to it
//! ? What is a crate?          a question the listener can click (pauses the codecast)
//!   The answer, indented.     answer lines; may hold their own `@` `!` `=` directives
//! ```
//!
//! `glossary: off` in the header turns the built-in glossary questions off for this script
//! (see [`crate::glossary`]); `pronounce: Dioxus = dee ox us` fixes how TTS says a word,
//! `spell: IP, CoAP` has words read letter by letter (see [`crate::speech`]).
//!
//! `plan: one line` under a part heading (or the description after its link in `index.md`) feeds
//! the plan page that `@ plan` shows: the parts in order, one line each.
//!
//! A ```` ```mermaid ```` fenced block inside a part is a diagram (see [`crate::diagram`]):
//! `@ diagram` shows it, `!` glows its nodes by id, `> alice bus` moves a node into a group
//! (or next to a node) from that cue on, `> alice` brings it back to the top level,
//! `> +copy` draws a node written `copy[…]:::hidden`, `> -alice` hides one.
//!
//! The **side panel** shows a second thing next to the view: `| bytes:Header` a
//! ```` ```bytes Header ```` block (a byte layout, see [`crate::bytes`]), `| md:Steps` a
//! ```` ```md Steps ```` block (a list, a table, notes), `| diagram:2` a mermaid block. Sticky
//! inside a part like `>` lines; `|` alone closes it. `!` refs may name its fields or nodes.
//! A ```` ```json Request body ```` block (also `csv`, `yaml`, `toml`, `xml`, `http`, `sql`, `sh`,
//! `text`) is an **example**: `| example:Request body` shows it, `!` glows its keys or columns.

use crate::diagram::{Diagram, Op};
use crate::model::*;
use crate::views::Mode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    /// `plan:` line of the header: the intro sentence of the plan page
    #[serde(default)]
    pub summary: Option<String>,
    pub parts: Vec<Part>,
}

fn yes() -> bool {
    true
}

impl Default for Script {
    fn default() -> Self {
        Script { title: String::new(), audio: None, glossary: true, pronounce: vec![], summary: None, parts: vec![] }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Part {
    pub name: String,
    /// Recording of this part alone (relative to the script's directory); overrides `Script::audio`.
    pub audio: Option<String>,
    pub cues: Vec<Cue>,
    /// ```` ```mermaid ```` blocks of the part, in order (`@ diagram`, `@ diagram:2`, `@ diagram:<title>`)
    #[serde(default)]
    pub diagrams: Vec<Diagram>,
    /// one line about the part for the plan page (`@ plan`): a `plan:` line under the heading, or
    /// the description after the part's link in `index.md`
    #[serde(default)]
    pub summary: Option<String>,
    /// ```` ```bytes ```` blocks of the part, in order (`| bytes`, `| bytes:2`, `| bytes:<title>`)
    #[serde(default)]
    pub layouts: Vec<crate::bytes::Layout>,
    /// ```` ```md ```` blocks of the part, in order (`| md`, `| md:2`, `| md:<title>`)
    #[serde(default)]
    pub notes: Vec<Note>,
    /// example blocks of the part (```` ```json Body ````, ```` ```csv ````…), in order (`| example:Body`)
    #[serde(default)]
    pub examples: Vec<Example>,
    /// recordings of this part in other languages or by other voices (`audio.ja:` + `times.ja:`,
    /// `audio.en.am_adam:` + `times.en.am_adam:` lines)
    #[serde(default)]
    pub voices: Vec<Voiced>,
}

/// A recording of a part in a language, by a voice: `audio.ja: 01-welcome.ja.ogg` and
/// `times.ja: 0 3.1 …` (the cues of the translated part, in order), or `audio.en.am_adam:` for
/// one Kokoro voice among several.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Voiced {
    pub lang: String,
    /// the Kokoro voice (`am_adam`), `""` when the line does not say
    #[serde(default)]
    pub voice: String,
    pub audio: String,
    pub times: Vec<f64>,
}

impl Voiced {
    /// `ja`, or `en.am_adam`: what follows `audio.` / `times.`
    pub fn key(&self) -> String {
        if self.voice.is_empty() { self.lang.clone() } else { format!("{}.{}", self.lang, self.voice) }
    }

    /// The `audio.KEY:` and `times.KEY:` lines.
    pub fn lines(&self) -> String {
        format!("audio.{k}: {}\ntimes.{k}: {}", self.audio, self.times.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(" "), k = self.key())
    }
}

impl Part {
    fn voiced(&mut self, key: &str) -> &mut Voiced {
        let (lang, voice) = key.split_once('.').unwrap_or((key, ""));
        if let Some(i) = self.voices.iter().position(|v| v.lang == lang && v.voice == voice) {
            return &mut self.voices[i];
        }
        self.voices.push(Voiced { lang: lang.to_string(), voice: voice.to_string(), ..Default::default() });
        self.voices.last_mut().unwrap()
    }
}

/// A ```` ```md Title ```` block: markdown shown in the side panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Note {
    pub title: Option<String>,
    pub text: String,
}

/// A ```` ```json Title ```` (csv, yaml…) block: sample data shown in the side panel, a request
/// body sent, a file read, a row stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Example {
    pub lang: String,
    pub title: Option<String>,
    pub text: String,
}

/// Fence words read as examples.
const EXAMPLE_LANGS: [&str; 13] = ["json", "csv", "tsv", "yaml", "yml", "toml", "xml", "http", "sql", "sh", "text", "txt", "example"];

impl Example {
    /// `csv`/`tsv`: rows of cells, the first one the header.
    pub fn table(&self) -> Option<Vec<Vec<String>>> {
        let sep = match self.lang.as_str() {
            "csv" => ',',
            "tsv" => '\t',
            _ => return None,
        };
        Some(self.text.lines().filter(|l| !l.trim().is_empty()).map(|l| l.split(sep).map(|c| c.trim().trim_matches('"').to_string()).collect()).collect())
    }

    /// What `!` may glow: the columns of a table, the keys of the other formats
    /// (`"key":`, `key:`, `key =`, `<key`).
    pub fn ids(&self) -> Vec<String> {
        if let Some(t) = self.table() {
            return t.into_iter().next().unwrap_or_default();
        }
        let mut ids: Vec<String> = Vec::new();
        for l in self.text.lines() {
            if let Some(k) = line_key(l) {
                if !ids.iter().any(|i| i == k) {
                    ids.push(k.to_string());
                }
            }
        }
        ids
    }
}

/// The key a line of json, yaml, toml, xml or http headers names, if any.
pub fn line_key(l: &str) -> Option<&str> {
    let t = l.trim_start().trim_start_matches("- ");
    let word = |s: &str| s.char_indices().find(|(_, c)| !(c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))).map(|(i, _)| i).unwrap_or(s.len());
    let k = if let Some(r) = t.strip_prefix('"') {
        let e = r.find('"')?;
        r[e + 1..].trim_start().starts_with(':').then(|| &r[..e])?
    } else if let Some(r) = t.strip_prefix('<') {
        &r[..word(r)]
    } else {
        let n = word(t);
        let rest = t[n..].trim_start();
        (rest.starts_with(':') || rest.starts_with('=')).then(|| &t[..n])?
    };
    Some(k).filter(|k| !k.is_empty())
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
    /// `>` lines: what moves in the diagram from this cue on (see [`crate::diagram::Op`])
    #[serde(default)]
    pub ops: Vec<Op>,
    /// `| kind[:ref]`: what the side panel shows from this cue on; `Some("")` closes it
    #[serde(default)]
    pub side: Option<String>,
    /// `^` lines: a popup card shown with this cue only (a path, an endpoint, a command), never
    /// spoken nor translated
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pop: Vec<String>,
    /// `architecture_tour` line: a generated tour that [`expand_tours`] puts in place of this cue
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tour: Option<String>,
    /// `+ files` / `+ search`: the panel enlarged, or the part of the app lit up, from this cue
    /// on (see [`Focus`]); `Some("")` (`+` alone) gives everything back its normal look
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<String>,
}

/// A line that asks for a generated tour: `name_tour` or `name_tour:<ref>`.
fn tour_line(t: &str) -> bool {
    let name = t.split_once(':').map_or(t, |(n, _)| n);
    name.ends_with("_tour") && name.chars().all(|c| c.is_ascii_lowercase() || c == '_') && !t.contains(char::is_whitespace)
}

/// Replace every tour cue (`architecture_tour`, `flow_tour:crate::run`, …) by the cues it stands
/// for ([`crate::tours`]). Tours that fail are dropped ([`check`] reports them). Directives
/// written with the tour line and its questions go to the first generated cue.
pub fn expand_tours(p: &Project, s: &mut Script) {
    for part in &mut s.parts {
        if part.cues.iter().all(|c| c.tour.is_none()) {
            continue;
        }
        let mut cues = Vec::new();
        // a tour that ends on one of its notes: the next written cue closes the panel
        let mut close = false;
        for mut c in std::mem::take(&mut part.cues) {
            let Some(line) = c.tour.as_deref() else {
                if std::mem::take(&mut close) && c.side.is_none() {
                    c.side = Some(String::new());
                }
                cues.push(c);
                continue;
            };
            let crate::tours::Tour { cues: mut gen, notes } = crate::tours::generate(p, line).unwrap_or_default();
            for n in notes {
                if !part.notes.contains(&n) {
                    part.notes.push(n);
                }
            }
            close = gen.iter().rev().find_map(|g| g.side.as_deref()).is_some_and(|s| !s.is_empty());
            if let Some(first) = gen.first_mut() {
                first.show = c.show.or(first.show.take());
                first.hl = c.hl.or(first.hl.take());
                first.code = c.code.or(first.code.take());
                first.side = c.side.or(first.side.take());
                first.focus = c.focus.or(first.focus.take());
                first.ops.extend(c.ops);
                first.questions.extend(c.questions);
            }
            cues.extend(gen);
        }
        part.cues = cues;
    }
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
    /// `|` line of the answer: the side panel while it is shown
    #[serde(default)]
    pub side: Option<String>,
    /// `^` lines of the answer: its popup card, never spoken
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pop: Vec<String>,
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
    let mut part = Part { name: "Codecast".into(), ..Default::default() };
    let mut pending = Cue::default();
    let mut in_part = false;
    // `times:` of the current part, given to its cues when the part ends
    let mut times: Vec<f64> = Vec::new();
    // a `?` block being read: it goes to the previous cue when no directive is pending for the
    // next one, else to the next cue
    let mut question: Option<(Question, bool)> = None;
    // a ```mermaid / ```bytes / ```md block being read: (kind, title, lines)
    let mut fence: Option<(Fence, Option<String>, Vec<String>)> = None;
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
        if let Some((_, _, buf)) = fence.as_mut() {
            if t.starts_with("```") {
                close_fence(fence.take(), &mut part);
            } else {
                buf.push(line.to_string());
            }
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
                } else if let Some(rest) = t.strip_prefix("| ").or_else(|| (t == "|").then_some("")) {
                    q.side = Some(rest.trim().to_string());
                } else if let Some(rest) = t.strip_prefix("^ ") {
                    q.pop.push(rest.trim().to_string());
                } else if !t.starts_with("//") {
                    q.answer.push_str(t);
                    q.answer.push('\n');
                }
                continue;
            }
            flush(&mut question, &mut part, &mut pending);
        }
        if let Some(info) = t.strip_prefix("```") {
            let (word, title) = info.trim().split_once(char::is_whitespace).map(|(a, b)| (a, Some(b.trim().to_string()))).unwrap_or((info.trim(), None));
            let kind = match word {
                "mermaid" => Some(Fence::Mermaid),
                "bytes" => Some(Fence::Bytes),
                "md" | "markdown" => Some(Fence::Md),
                w if EXAMPLE_LANGS.contains(&w) => Some(Fence::Example(w.to_string())),
                _ => None,
            };
            if let Some(k) = kind {
                fence = Some((k, title.filter(|t| !t.is_empty()), vec![]));
                continue;
            }
        }
        if let Some(rest) = t.strip_prefix("? ").or_else(|| (t == "?").then_some("")) {
            let prev = pending == Cue::default() && !part.cues.is_empty();
            question = Some((Question { ask: rest.trim().to_string(), ..Default::default() }, prev));
            continue;
        }
        if let Some(rest) = t.strip_prefix("## ") {
            flush(&mut question, &mut part, &mut pending);
            apply_times(&mut part, &mut times);
            if !part.cues.is_empty() || !s.parts.is_empty() {
                s.parts.push(std::mem::take(&mut part));
            }
            part = Part { name: rest.trim().to_string(), ..Default::default() };
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
        if let Some(rest) = t.strip_prefix("times:") {
            times = rest.split_whitespace().filter_map(|v| v.parse().ok()).collect();
            continue;
        }
        // `audio.ja:` / `times.ja:`: the part in another language; `audio.en.am_adam:` by one voice
        if let Some((key, rest)) = t.split_once(':').filter(|(k, _)| in_part && (k.starts_with("audio.") || k.starts_with("times.")) && !k.contains(' ')) {
            let (what, key) = key.split_once('.').unwrap_or_default();
            let v = part.voiced(key);
            if what == "audio" {
                v.audio = rest.trim().to_string();
            } else {
                v.times = rest.split_whitespace().filter_map(|v| v.parse().ok()).collect();
            }
            continue;
        }
        if let Some(rest) = t.strip_prefix("plan:") {
            let v = Some(rest.trim().to_string()).filter(|v| !v.is_empty());
            if in_part {
                part.summary = v;
            } else {
                s.summary = v;
            }
            continue;
        }
        if let Some(rest) = t.strip_prefix("glossary:") {
            if !in_part {
                s.glossary = !matches!(rest.trim().to_ascii_lowercase().as_str(), "off" | "no" | "false" | "none");
            }
            continue;
        }
        if t.starts_with("pronounce:") || t.starts_with("spell:") {
            s.pronounce.extend(crate::speech::parse_pronounce(t));
            continue;
        }
        if let Some(rest) = t.strip_prefix('@') {
            pending.show = Some(rest.trim().to_string());
            continue;
        }
        // `+ files`, `+ search`; any other `+ …` line is a cue
        if let Some(rest) = t.strip_prefix('+').filter(|r| focus_of(r).is_some()) {
            pending.focus = Some(rest.trim().to_string());
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
        if let Some(rest) = t.strip_prefix('>') {
            pending.ops.extend(crate::diagram::parse_ops(rest));
            continue;
        }
        if let Some(rest) = t.strip_prefix("^ ") {
            pending.pop.push(rest.trim().to_string());
            continue;
        }
        // `| bytes:Header`; a markdown table row (`| a | b |`) is not a directive
        if let Some(rest) = t.strip_prefix('|').filter(|r| !r.contains('|')) {
            pending.side = Some(rest.trim().to_string());
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
        if tour_line(t) {
            pending.tour = Some(t.to_string());
            part.cues.push(std::mem::take(&mut pending));
            continue;
        }
        pending.say = t.to_string();
        part.cues.push(std::mem::take(&mut pending));
    }
    // unclosed block: keep what was read, the parser reports what is wrong with it
    close_fence(fence.take(), &mut part);
    flush(&mut question, &mut part, &mut pending);
    apply_times(&mut part, &mut times);
    if !part.cues.is_empty() || s.parts.is_empty() {
        s.parts.push(part);
    }
    if s.title.is_empty() {
        s.title = "Codecast".into();
    }
    s
}

/// Give a part's `times:` to its cues in order; a cue's own `[t]` wins.
fn apply_times(part: &mut Part, times: &mut Vec<f64>) {
    for (c, t) in part.cues.iter_mut().zip(times.drain(..)) {
        c.t = c.t.or(Some(t));
    }
}

#[derive(Clone)]
enum Fence {
    Mermaid,
    Bytes,
    Md,
    Example(String),
}

fn close_fence(f: Option<(Fence, Option<String>, Vec<String>)>, part: &mut Part) {
    let Some((kind, title, lines)) = f else { return };
    match kind {
        Fence::Mermaid => part.diagrams.push(crate::diagram::parse(&lines.join("\n"))),
        Fence::Bytes => part.layouts.push(crate::bytes::parse(&lines.join("\n"), title.as_deref())),
        Fence::Md => {
            let text = dedent(&lines);
            part.notes.push(Note { title, text });
        }
        Fence::Example(lang) => part.examples.push(Example { lang, title, text: dedent(&lines) }),
    }
}

/// Drop the common indentation, keep nested indents.
fn dedent(lines: &[String]) -> String {
    let ind = lines.iter().filter(|l| !l.trim().is_empty()).map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
    lines.iter().map(|l| l.get(ind..).unwrap_or("").to_string()).collect::<Vec<_>>().join("\n")
}

pub fn to_text(s: &Script) -> String {
    let mut out = format!("# {}\n", s.title);
    if let Some(a) = &s.audio {
        out.push_str(&format!("audio: {}\n", a));
    }
    if let Some(p) = &s.summary {
        out.push_str(&format!("plan: {}\n", p));
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
        for v in &p.voices {
            out.push_str(&v.lines());
            out.push('\n');
        }
        if let Some(pl) = &p.summary {
            out.push_str(&format!("plan: {}\n", pl));
        }
        for d in &p.diagrams {
            out.push_str(&format!("```mermaid\n{}\n```\n", d.source));
        }
        for l in &p.layouts {
            out.push_str(&format!("```bytes{}\n{}\n```\n", l.title.as_ref().filter(|_| !l.source.contains("title:")).map(|t| format!(" {}", t)).unwrap_or_default(), l.source));
        }
        for n in &p.notes {
            out.push_str(&format!("```md{}\n{}\n```\n", n.title.as_ref().map(|t| format!(" {}", t)).unwrap_or_default(), n.text));
        }
        for e in &p.examples {
            out.push_str(&format!("```{}{}\n{}\n```\n", e.lang, e.title.as_ref().map(|t| format!(" {}", t)).unwrap_or_default(), e.text));
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
            for op in &c.ops {
                out.push_str(&format!("> {}\n", crate::diagram::op_text(op)));
            }
            if let Some(sd) = &c.side {
                out.push_str(&format!("| {}\n", sd).replace("| \n", "|\n"));
            }
            for l in &c.pop {
                out.push_str(&format!("^ {}\n", l));
            }
            if let Some(f) = &c.focus {
                out.push_str(&format!("+ {}\n", f).replace("+ \n", "+\n"));
            }
            let say = flatten(&c.say);
            match (&c.tour, c.t) {
                (Some(tour), _) => out.push_str(&format!("{}\n", tour)),
                (None, Some(t)) => out.push_str(&format!("[{}] {}\n", t, say)),
                (None, None) => out.push_str(&format!("{}\n", say)),
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
                if let Some(sd) = &q.side {
                    out.push_str(&format!("  | {}\n", sd).replace("| \n", "|\n"));
                }
                for l in &q.pop {
                    out.push_str(&format!("  ^ {}\n", l));
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
    /// cue start times of that recording (sibling `01-welcome.times`, written by `iwr voice`)
    pub times: Vec<f64>,
    /// recordings in other languages or by other voices (`01-welcome.ja.ogg` + `01-welcome.ja.times`,
    /// `01-welcome.am_adam.ogg` + `.times`)
    pub voices: Vec<Voiced>,
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

/// The part list of `index.md`, in order, as (stem, description): `- [Part](NN-file.md) — one line
/// about it` (also `- NN-file.md …`, `` - `NN-file.md` ``, `1. …`). Only entries naming an existing
/// part file count; the description feeds the plan page (`@ plan`).
pub fn index_parts(index: &str, stems: &[String]) -> Vec<(String, Option<String>)> {
    let mut out: Vec<(String, Option<String>)> = Vec::new();
    for line in index.lines() {
        let Some(body) = list_item(line.trim()) else { continue };
        let (target, rest) = link_target(body);
        let stem = target.trim_start_matches("./").trim_end_matches(".md").to_string();
        if stems.contains(&stem) && !out.iter().any(|(s, _)| *s == stem) {
            out.push((stem, link_desc(&rest)));
        }
    }
    out
}

/// Order the part files of a directory: the order listed in `index` (markdown links or list items
/// naming `.md` files), then the rest sorted by name. `index.md` itself is never a part.
pub fn order_parts(index: &str, stems: &[String]) -> Vec<String> {
    let mut out: Vec<String> = index_parts(index, stems).into_iter().map(|(s, _)| s).collect();
    let mut rest: Vec<&String> = stems.iter().filter(|s| !out.contains(s) && s.as_str() != "index").collect();
    rest.sort();
    out.extend(rest.into_iter().cloned());
    out
}

/// `- item`, `* item`, `1. item` → `item`.
fn list_item(t: &str) -> Option<&str> {
    t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| t.split_once(". ").filter(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())).map(|(_, b)| b))
}

/// `[Part](NN-file.md) — description` → (`NN-file.md`, ` — description`); also `` `NN-file.md` ``
/// and `NN-file.md description`.
fn link_target(body: &str) -> (String, String) {
    let body = body.trim();
    if let Some(i) = body.find("](") {
        let after = &body[i + 2..];
        if let Some(j) = after.find(')') {
            return (after[..j].trim().to_string(), after[j + 1..].to_string());
        }
    }
    let clean = |a: &str| a.trim_end_matches(':').trim_matches('`').to_string();
    match body.split_once(char::is_whitespace) {
        Some((a, b)) => (clean(a), b.to_string()),
        None => (clean(body), String::new()),
    }
}

/// The description after a part link: ` — about…`, ` - about…`, `: about…`.
fn link_desc(rest: &str) -> Option<String> {
    let d = rest.trim().trim_start_matches(['—', '–', '-', ':', '·']).trim();
    (!d.is_empty()).then(|| d.to_string())
}

/// Lines of `index.md` that are not the part list (title, `audio:`, `plan:`, intro cues).
fn index_body(index: &str, stems: &[String]) -> String {
    index
        .lines()
        .filter(|line| {
            let t = line.trim();
            match t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
                Some(body) => {
                    let (target, _) = link_target(body);
                    !stems.contains(&target.trim_start_matches("./").trim_end_matches(".md").to_string())
                }
                None => true,
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Join `index.md` and the part files into one script text. Each part file becomes a `##` part:
/// its first `##` heading names it, otherwise the file name does; a sibling recording becomes its
/// `audio:` line unless the file sets one, with its cue times as a `times:` line.
pub fn assemble(index: &str, parts: &[PartFile]) -> String {
    let stems: Vec<String> = parts.iter().map(|p| p.stem.clone()).collect();
    let order = order_parts(index, &stems);
    let descs: HashMap<String, String> = index_parts(index, &stems).into_iter().filter_map(|(s, d)| d.map(|d| (s, d))).collect();
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
                if !pf.times.is_empty() {
                    body.insert(2, format!("times: {}", pf.times.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(" ")));
                }
            }
        }
        for (i, v) in pf.voices.iter().enumerate() {
            body.insert(1 + i, v.lines());
        }
        if let Some(d) = descs.get(&stem) {
            if !body.iter().any(|l| l.trim_start().starts_with("plan:")) {
                body.insert(1, format!("plan: {}", d));
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
        "diagram" | "sketch" => Mode::Diagram,
        "plan" | "outline" => Mode::Plan,
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
        Mode::Diagram => "diagram",
        Mode::Plan => "plan",
    }
}

/// The diagram `@ diagram[:ref]` points at in a part: the first one, the n-th (`:2`), or by
/// front-matter title (`:Bus`).
pub fn find_diagram<'a>(part: &'a Part, r: Option<&str>) -> Option<&'a Diagram> {
    find_by(&part.diagrams, r, |d| d.title.as_deref())
}

/// `@ <kind>[:ref]` / `| <kind>[:ref]` lookup: the first block, the n-th (`:2`), or by title.
fn find_by<'a, T>(items: &'a [T], r: Option<&str>, title: impl Fn(&T) -> Option<&str>) -> Option<&'a T> {
    match r.map(str::trim).filter(|r| !r.is_empty()) {
        None => items.first(),
        Some(r) => match r.parse::<usize>() {
            Ok(n) => items.get(n.checked_sub(1)?),
            Err(_) => items.iter().find(|x| title(x).map(|t| t.eq_ignore_ascii_case(r)).unwrap_or(false)),
        },
    }
}

/// What the side panel shows (`| bytes:Header`, `| md:Steps`, `| diagram:2`).
#[derive(Debug, Clone, PartialEq)]
pub enum Side {
    Bytes(crate::bytes::Layout),
    Note(Note),
    Diagram(Diagram),
    Example(Example),
}

impl Side {
    /// The refs `!` may glow in it: field ids, node ids.
    pub fn ids(&self) -> Vec<String> {
        match self {
            Side::Bytes(l) => l.ids().into_iter().map(String::from).collect(),
            Side::Diagram(d) => d.ids().into_iter().map(String::from).collect(),
            Side::Example(e) => e.ids(),
            Side::Note(_) => vec![],
        }
    }
}

/// Resolve a `|` line in a part. `Ok(None)` closes the panel (`|` alone).
pub fn find_side(part: &Part, side: &str) -> Result<Option<Side>, String> {
    let side = side.trim();
    if side.is_empty() {
        return Ok(None);
    }
    let (kind, r) = match side.split_once(':') {
        Some((k, r)) => (k.trim(), Some(r.trim())),
        None => (side, None),
    };
    let missing = |what: &str, block: &str| match r {
        Some(r) => format!("no {} `{}` in this part (`| {}:2`, or the block's title)", what, r, kind),
        None => format!("`| {}` but this part has no {} block", kind, block),
    };
    match kind {
        "bytes" => find_by(&part.layouts, r, |l| l.title.as_deref()).map(|l| Some(Side::Bytes(l.clone()))).ok_or_else(|| missing("byte layout", "```bytes")),
        "md" | "markdown" | "note" | "list" => find_by(&part.notes, r, |n| n.title.as_deref()).map(|n| Some(Side::Note(n.clone()))).ok_or_else(|| missing("```md block", "```md")),
        "diagram" | "sketch" => find_by(&part.diagrams, r, |d| d.title.as_deref()).map(|d| Some(Side::Diagram(d.clone()))).ok_or_else(|| missing("diagram", "```mermaid")),
        "example" | "ex" | "data" => find_by(&part.examples, r, |e| e.title.as_deref()).map(|e| Some(Side::Example(e.clone()))).ok_or_else(|| missing("example", "```json (csv, yaml…)")),
        // `| json:Body`: the examples of that format
        k if EXAMPLE_LANGS.contains(&k) => {
            let of: Vec<Example> = part.examples.iter().filter(|e| e.lang == k).cloned().collect();
            find_by(&of, r, |e| e.title.as_deref()).map(|e| Some(Side::Example(e.clone()))).ok_or_else(|| missing(&format!("{} example", k), &format!("```{}", k)))
        }
        _ => Err(format!("unknown side panel `| {}` (bytes, md, diagram, example)", side)),
    }
}

/// The side panel at cue `cue` of a part: the last `|` line from the part's first cue on.
pub fn side_at(part: &Part, cue: usize) -> Option<Side> {
    let s = part.cues.iter().take(cue + 1).rev().find_map(|c| c.side.as_deref())?;
    find_side(part, s).ok().flatten()
}

/// A panel of the app a cue can enlarge (`+ files`): the others shrink while it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    /// the file tree (folders and files)
    Files,
    /// the item list of the sidebar
    Items,
    /// the view in the middle: a graph, the code blocks, a diagram
    View,
    /// the source: the details panel next to a graph, the source pane of the Code view
    Source,
}

impl Panel {
    pub fn name(self) -> &'static str {
        match self {
            Panel::Files => "files",
            Panel::Items => "items",
            Panel::View => "view",
            Panel::Source => "source",
        }
    }
}

/// A `+` line's panel: `Some(Some(_))`, `Some(None)` for normal sizes (`+` alone, `+ none`),
/// `None` when the word is not a panel.
pub fn panel_of(s: &str) -> Option<Option<Panel>> {
    Some(Some(match s.trim() {
        "" | "none" | "off" | "normal" => return Some(None),
        "files" | "folders" | "tree" => Panel::Files,
        "items" => Panel::Items,
        "view" | "graph" | "canvas" | "blocks" | "diagram" => Panel::View,
        "source" | "code" => Panel::Source,
        _ => return None,
    }))
}

/// Parts of the app a `+` line can light up (the rest of the screen dims, the player stays
/// readable). The UI knows where each one is.
pub const UI_SPOTS: &[(&str, &str)] = &[
    ("brand", "the app's name, top left"),
    ("panel-button", "the button that shows or hides the sidebar"),
    ("views", "the Code tab and the views"),
    ("split", "the button that shows the source next to the blocks"),
    ("search", "the search box"),
    ("codecast", "the button that starts or stops a codecast"),
    ("tour", "the full tour button"),
    ("generate", "the Generate page button (iwr serve only)"),
    ("help", "the help button: the tutorial"),
    ("settings", "the settings button"),
    ("sidebar-tabs", "the files / items tabs of the sidebar"),
    ("player", "the player bar"),
    ("caption", "the text of the codecast"),
    ("parts", "the parts of the codecast"),
    ("voice", "the voice picker"),
    ("lang", "the language picker"),
    ("plan-button", "the plan button of the player"),
    ("questions", "the questions offered under the text"),
    ("side", "the side panel of a codecast (`|` lines)"),
];

/// What a `+` line puts forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// a panel grows, the others shrink
    Panel(Panel),
    /// a part of the app ([`UI_SPOTS`]) is lit up
    Spot(&'static str),
    /// the screen zooms like a presenter would: onto what the cue highlights (`+ zoom`), or onto a
    /// panel or a part of the app (`+ zoom files`, `+ zoom search`; the name as [`Panel::name`]
    /// or in [`UI_SPOTS`])
    Zoom(Option<&'static str>),
}

/// A `+` line's focus: `Some(Some(_))`, `Some(None)` for the normal look (`+` alone), `None` when
/// the word is neither a panel nor a spot.
pub fn focus_of(s: &str) -> Option<Option<Focus>> {
    let s = s.trim();
    if s == "zoom" {
        return Some(Some(Focus::Zoom(None)));
    }
    if let Some(on) = s.strip_prefix("zoom ") {
        return match focus_of(on)?? {
            Focus::Panel(p) => Some(Some(Focus::Zoom(Some(p.name())))),
            Focus::Spot(n) => Some(Some(Focus::Zoom(Some(n)))),
            Focus::Zoom(_) => None,
        };
    }
    panel_of(s).map(|p| p.map(Focus::Panel)).or_else(|| UI_SPOTS.iter().find(|(n, _)| *n == s.trim()).map(|(n, _)| Some(Focus::Spot(n))))
}

/// The focus at cue `cue` of a part: the last `+` line from the part's first cue on.
pub fn focus_at(part: &Part, cue: usize) -> Option<Focus> {
    part.cues.iter().take(cue + 1).rev().find_map(|c| c.focus.as_deref()).and_then(focus_of).flatten()
}

/// A folder ref (`src/storage`, `src/storage/`, `storage/`): the directory as the file tree
/// names it (`src/storage`). An exact path wins, else a unique suffix. Callers try [`resolve`]
/// first: a file or a module of that name wins over the folder (end the ref with `/`).
pub fn resolve_dir(p: &Project, r: &str) -> Option<String> {
    let r = r.trim().trim_end_matches('/');
    if r.is_empty() || r.contains(':') {
        return None;
    }
    let mut dirs: Vec<&str> = p.files.iter().flat_map(|f| f.path.match_indices('/').map(|(i, _)| &f.path[..i])).collect();
    dirs.sort();
    dirs.dedup();
    if dirs.contains(&r) {
        return Some(r.to_string());
    }
    let tail = format!("/{}", r);
    let mut cands = dirs.into_iter().filter(|d| d.ends_with(&tail));
    let first = cands.next()?;
    cands.next().is_none().then(|| first.to_string())
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
    // file:line[-line][:c1-c2]: a Rust file, or any other file of the repository (Terraform, YAML…)
    if let Some((file, rest)) = r.split_once(':') {
        let text_file = !file.ends_with(".rs") && rest.starts_with(|c: char| c.is_ascii_digit());
        let find = || p.files.iter().find(|f| f.path == file || f.path.ends_with(&format!("/{}", file)));
        if file.ends_with(".rs") || (text_file && find().is_some()) {
            let f = find()?;
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
        let m = p.modules.iter().find(|m| m.path == r || m.path == format!("crate::{}", r) || (r != "crate" && m.path.ends_with(&format!("::{}", r)))).or_else(|| {
            // a crate named after its package, asked for by its folder (`config-server` holding `gw-config-server`)
            let root = |f: &str| [format!("{r}/src/main.rs"), format!("{r}/src/lib.rs")].iter().any(|x| f == x || f.ends_with(&format!("/{x}")));
            p.modules.iter().find(|m| !m.path.contains("::") && m.file.is_some_and(|f| root(&p.files[f].path)))
        })?;
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

/// What `@` has selected so far while reading a script in order (the view is sticky, like in
/// the player).
#[derive(Clone)]
struct Track<'a> {
    mode: Option<Mode>,
    diagram: Option<&'a Diagram>,
    /// refs of what the side panel shows (`|`)
    side: Vec<String>,
}

impl<'a> Track<'a> {
    /// Apply an `@` line; reports an unknown view, ref or diagram.
    fn show(&mut self, p: &Project, part: &'a Part, at: &str, show: &str, bad: &mut Vec<String>) {
        let (v, r) = parse_show(show);
        match v {
            None => bad.push(format!("{}: unknown view in `@ {}`", at, show)),
            Some(Mode::Diagram) => {
                self.mode = Some(Mode::Diagram);
                self.diagram = find_diagram(part, r);
                if self.diagram.is_none() {
                    bad.push(match r {
                        Some(r) => format!("{}: no diagram `{}` in this part (`@ diagram:2`, or the block's `title:`)", at, r),
                        None => format!("{}: `@ diagram` but this part has no ```mermaid block", at),
                    });
                }
            }
            Some(Mode::Plan) => {
                self.mode = Some(Mode::Plan);
                self.diagram = None;
                if r.is_some() {
                    bad.push(format!("{}: `@ plan` takes no ref", at));
                }
            }
            Some(m) => {
                self.mode = Some(m);
                self.diagram = None;
                if let Some(r) = r {
                    if resolve(p, r).is_none() {
                        bad.push(format!("{}: unresolved ref `{}` in `@`", at, r));
                    }
                }
            }
        }
    }

    /// Apply a `|` line; reports an unknown panel or block.
    fn side(&mut self, part: &'a Part, at: &str, side: &str, bad: &mut Vec<String>) {
        match find_side(part, side) {
            Ok(s) => self.side = s.map(|s| s.ids()).unwrap_or_default(),
            Err(e) => {
                self.side.clear();
                bad.push(format!("{}: {}", at, e));
            }
        }
    }

    /// Check `!` refs: side panel ids, diagram node ids while a diagram is shown, project refs
    /// otherwise.
    fn hl(&self, p: &Project, at: &str, refs: &[String], bad: &mut Vec<String>) {
        for r in refs {
            if self.side.contains(r) {
                continue;
            }
            match (self.mode, self.diagram) {
                (Some(Mode::Diagram), Some(d)) => {
                    if !d.has(r) {
                        bad.push(format!("{}: `{}` is not a node of the diagram (nodes: {})", at, r, d.ids().join(" ")));
                    }
                }
                (Some(Mode::Diagram), None) => {}
                _ => {
                    if resolve(p, r).is_none() && resolve_dir(p, r).is_none() {
                        bad.push(format!("{}: unresolved ref `{}` in `!`", at, r));
                    }
                }
            }
        }
    }

    /// Check `>` ops against the diagram shown, else the part's first one.
    fn ops(&self, part: &'a Part, at: &str, ops: &[Op], bad: &mut Vec<String>) {
        let d = match (self.mode, self.diagram) {
            (Some(Mode::Diagram), Some(d)) => d,
            (Some(Mode::Diagram), None) => return, // the `@ diagram` error was reported
            _ => match part.diagrams.first() {
                Some(d) => d,
                None => {
                    bad.push(format!("{}: `>` but this part has no diagram to move things in", at));
                    return;
                }
            },
        };
        for op in ops {
            match op {
                Op::Bad(s) => bad.push(format!("{}: cannot read `> {}` (one move per line: `> node group`, `> node other`, `> node`)", at, s)),
                Op::Move { node, to } => {
                    for r in std::iter::once(node).chain(to.iter()) {
                        if !d.has(r) {
                            bad.push(format!("{}: `{}` in `>` is not a node of the diagram (nodes: {})", at, r, d.ids().join(" ")));
                        }
                    }
                }
                Op::Show(a) | Op::Hide(a) => {
                    if !d.has(a) {
                        bad.push(format!("{}: `{}` in `>` is not a node of the diagram (nodes: {})", at, a, d.ids().join(" ")));
                    }
                }
            }
        }
    }
}

/// All refs in a script that do not resolve, with their location.
pub fn check(p: &Project, s: &Script) -> Vec<String> {
    let mut bad = Vec::new();
    for part in &s.parts {
        for c in &part.cues {
            if let Some(Err(e)) = c.tour.as_deref().map(|t| crate::tours::generate(p, t)) {
                bad.push(format!("{}: {}", part.name, e));
            }
        }
    }
    // refs are checked on the script as it will play
    let mut expanded = s.clone();
    expand_tours(p, &mut expanded);
    let s = &expanded;
    let mut t = Track { mode: None, diagram: None, side: vec![] };
    for part in &s.parts {
        for (i, d) in part.diagrams.iter().enumerate() {
            for e in &d.errors {
                bad.push(format!("{} / diagram {}: {}", part.name, i + 1, e));
            }
        }
        for (i, l) in part.layouts.iter().enumerate() {
            for e in &l.errors {
                bad.push(format!("{} / bytes {}: {}", part.name, i + 1, e));
            }
        }
        // the side panel is per part
        t.side.clear();
        for (i, c) in part.cues.iter().enumerate() {
            let at = format!("{} / cue {}", part.name, i + 1);
            if let Some(show) = &c.show {
                t.show(p, part, &at, show, &mut bad);
            }
            if let Some(sd) = &c.side {
                t.side(part, &at, sd, &mut bad);
            }
            if let Some(refs) = &c.hl {
                t.hl(p, &at, refs, &mut bad);
            }
            if !c.ops.is_empty() {
                t.ops(part, &at, &c.ops, &mut bad);
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
                // an answer's directives apply while it is shown, then the cue comes back
                let mut qt = t.clone();
                if let Some(show) = &q.show {
                    qt.show(p, part, &at, show, &mut bad);
                }
                if let Some(sd) = &q.side {
                    qt.side(part, &at, sd, &mut bad);
                }
                if let Some(refs) = &q.hl {
                    qt.hl(p, &at, refs, &mut bad);
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
    let mut part = Part::default();
    let mut last_show: Option<String> = None;
    for st in steps {
        if st.part != part.name {
            if !part.cues.is_empty() {
                s.parts.push(std::mem::take(&mut part));
            }
            part.name = st.part.clone();
        }
        let mut cue = step_cue(p, st, &mut last_show);
        attach_note(&mut part.notes, &part.cues, &mut cue, st.note.clone());
        part.cues.push(cue);
    }
    if !part.cues.is_empty() {
        s.parts.push(part);
    }
    s
}

/// Show `note` in the side panel from `cue` on (added to `notes`, opened by title); without a
/// note, close the panel a note of the previous cues left open.
pub(crate) fn attach_note(notes: &mut Vec<Note>, before: &[Cue], cue: &mut Cue, note: Option<Note>) {
    match note {
        Some(n) => {
            cue.side = Some(format!("md:{}", n.title.clone().unwrap_or_default()));
            if !notes.contains(&n) {
                notes.push(n);
            }
        }
        None if cue.side.is_none() && before.iter().rev().find_map(|c| c.side.as_deref()).is_some_and(|s| s.starts_with("md:")) => cue.side = Some(String::new()),
        None => {}
    }
}

/// One guide step as a cue: its view (only when it changes), its glow, its text.
pub fn step_cue(p: &Project, st: &crate::guide::GuideStep, last_show: &mut Option<String>) -> Cue {
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
    let cue = Cue { say, show: if last_show.as_deref() == Some(&show) { None } else { Some(show.clone()) }, hl: Some(hl), ..Default::default() };
    *last_show = Some(show);
    cue
}

/// Spoken form of a cue: markdown and code blocks stripped, code tokens said as words
/// ([`crate::speech::spoken`] with the built-in pronunciations). Players with a script's own
/// `pronounce:` list call `speech::spoken` directly.
pub fn speakable(say: &str) -> String {
    crate::speech::spoken(say, &crate::speech::builtin_pronounce())
}

/// Words in spoken text that a voice stumbles on and that belong in a `^` popup instead: hyphenated
/// words (`gw-config-server`, `Wi-Fi`), paths (`/srv/gw`, `src/main.rs`) and HTTP requests
/// (`GET /config`). For `iwr check`. Code blocks are skipped.
pub fn hard_to_say(say: &str) -> Vec<String> {
    const VERBS: [&str; 7] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];
    let text = plain(say);
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut out: Vec<String> = Vec::new();
    for (i, w) in words.iter().enumerate() {
        let w = w.trim_matches(|c: char| matches!(c, ',' | '.' | ';' | ':' | '!' | '?' | '(' | ')' | '"' | '\'' | '«' | '»'));
        let b = w.as_bytes();
        let hyphen = (1..b.len().saturating_sub(1)).any(|k| b[k] == b'-' && b[k - 1].is_ascii_alphanumeric() && b[k + 1].is_ascii_alphanumeric());
        let path = w.len() > 1 && w.contains('/') && w.split('/').filter(|s| !s.is_empty()).count() >= if w.starts_with('/') { 1 } else { 2 } && !w.contains("://");
        let verb = VERBS.contains(&w) && words.get(i + 1).is_some_and(|n| n.starts_with('/'));
        let hit = if verb { format!("{} {}", w, words[i + 1].trim_end_matches([',', '.', ';', ':'])) } else { w.to_string() };
        if (hyphen || path || verb) && !out.iter().any(|o| o == &hit || o.ends_with(&format!(" {}", w))) {
            out.push(hit);
        }
    }
    out
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
