//! Charts of a codecast: a ```` ```chart Title ```` block (a line, bar, pie or stacked chart) and
//! the markers that move it while the voice speaks.
//!
//! ```text
//! line  x: version  y: seconds  unit: s     kind, then axis names and unit
//! series: debug release                     optional: several values per row
//! v1  42                                    `id value…` ["label"] [:::hidden]
//! v2  35  :::hidden                         waits for a `+v2` marker
//! ```
//!
//! Markers sit in braces inside a cue's text, `Then caching cut it {+v2 !v2} a bit.`: [`split_say`]
//! takes them out of the spoken text and remembers where they were, so the player fires them on
//! that word. [`Chart::state`] folds the markers fired so far into what is drawn.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    #[default]
    Line,
    Bar,
    Pie,
    Donut,
    /// stacked bars
    Stack,
    /// stacked bars, each one to 100 %
    StackPct,
}

impl Kind {
    fn of(w: &str) -> Option<Kind> {
        Some(match w {
            "line" => Kind::Line,
            "bar" | "bars" => Kind::Bar,
            "pie" => Kind::Pie,
            "donut" => Kind::Donut,
            "stack" => Kind::Stack,
            "stack%" => Kind::StackPct,
            _ => return None,
        })
    }

    pub fn round(self) -> bool {
        matches!(self, Kind::Pie | Kind::Donut)
    }
}

/// One row: a point, a bar, a slice (or a stacked bar with one value per series).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Row {
    pub id: String,
    pub label: String,
    pub values: Vec<f64>,
    /// `:::hidden`: drawn from its `+id` marker on
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Chart {
    pub title: Option<String>,
    /// the title as written: what `@ chart:<title>` names, also once `title` is translated
    pub name: Option<String>,
    pub kind: Kind,
    pub x: Option<String>,
    pub y: Option<String>,
    pub unit: Option<String>,
    pub series: Vec<String>,
    pub rows: Vec<Row>,
    pub errors: Vec<String>,
    /// the block as written (for `script::to_text`)
    pub source: String,
}

/// What a marker does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Mark {
    /// `+id`
    Show(String),
    /// `-id`
    Hide(String),
    /// `=id 18` (one value per series)
    Set(String, Vec<f64>),
    /// `!id`: glow these (the `!` tokens of one marker together); `!` alone clears
    Glow(Vec<String>),
    /// `~a>b`: a delta badge from a to b
    Delta(String, String),
    /// `--30 "target"`: a threshold line
    Rule(f64, Option<String>),
    /// `"text"@id`: a callout on a mark
    Note(String, String),
}

/// Markers of one `{}` in a cue: they fire `at` characters into its `of` characters of text (the
/// fraction holds when the text is translated).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CueMark {
    pub at: usize,
    pub of: usize,
    pub marks: Vec<Mark>,
}

impl CueMark {
    /// How far into the cue it fires, 0 to 1.
    pub fn frac(&self) -> f64 {
        if self.of == 0 { 0.0 } else { self.at as f64 / self.of as f64 }
    }
}

pub fn parse(src: &str, title: Option<&str>) -> Chart {
    let mut c = Chart { title: title.map(String::from), source: src.trim_end().to_string(), ..Default::default() };
    let mut head = false;
    for (n, line) in src.lines().enumerate() {
        let t = line.trim();
        if t.is_empty() || t.starts_with("//") {
            continue;
        }
        let toks = tokens(t);
        if !head {
            head = true;
            match Kind::of(&toks[0]) {
                Some(k) => c.kind = k,
                None => c.errors.push(format!("line {}: start with the kind: line, bar, pie, donut, stack or stack%", n + 1)),
            }
            for (k, v) in pairs(&toks[1..]) {
                match k.as_str() {
                    "x" => c.x = Some(v),
                    "y" => c.y = Some(v),
                    "unit" => c.unit = Some(v),
                    "title" => c.title = Some(v),
                    _ => c.errors.push(format!("line {}: unknown `{}:` (x, y, unit, title)", n + 1, k)),
                }
            }
            continue;
        }
        if let Some(r) = t.strip_prefix("series:") {
            c.series = r.split_whitespace().map(String::from).collect();
            continue;
        }
        let mut row = Row { id: toks[0].clone(), ..Default::default() };
        for tk in &toks[1..] {
            if tk == ":::hidden" {
                row.hidden = true;
            } else if let Some(l) = quoted(tk) {
                row.label = l.to_string();
            } else if let Ok(v) = tk.parse() {
                row.values.push(v);
            } else {
                c.errors.push(format!("line {}: `{}` is not a number", n + 1, tk));
            }
        }
        if row.label.is_empty() {
            row.label = row.id.clone();
        }
        let want = c.series.len().max(1);
        if row.values.len() != want {
            c.errors.push(format!("line {}: `{}` has {} value(s), expected {}", n + 1, row.id, row.values.len(), want));
            row.values.resize(want, 0.0);
        }
        if c.rows.iter().any(|r| r.id == row.id) {
            c.errors.push(format!("line {}: `{}` twice", n + 1, row.id));
        }
        c.rows.push(row);
    }
    c.name = c.title.clone();
    if c.rows.is_empty() {
        c.errors.push("no rows (`id value` per line)".into());
    }
    c
}

/// Words of a line; a `"quoted label"` (with what is glued to it, `"text"@id`) is one.
fn tokens(t: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut q = false;
    for ch in t.chars() {
        if ch == '"' {
            q = !q;
        }
        if ch.is_whitespace() && !q {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn quoted(t: &str) -> Option<&str> {
    t.strip_prefix('"')?.strip_suffix('"')
}

/// `x: a b  y: c` → (x, "a b"), (y, "c").
fn pairs(toks: &[String]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = vec![];
    for t in toks {
        match t.strip_suffix(':') {
            Some(k) if !k.is_empty() => out.push((k.to_string(), String::new())),
            _ => match out.last_mut() {
                Some((_, v)) => {
                    if !v.is_empty() {
                        v.push(' ');
                    }
                    v.push_str(t.trim_matches('"'));
                }
                None => out.push((t.clone(), String::new())),
            },
        }
    }
    out
}

/// The markers of a `{}`, or `None` when it is not one (code, a placeholder…).
pub fn parse_marks(inner: &str) -> Option<Vec<Mark>> {
    let toks = tokens(inner);
    let mut out = vec![];
    let mut glow: Option<Vec<String>> = None;
    let mut i = 0;
    let num = |s: &str| s.parse::<f64>().ok();
    while i < toks.len() {
        let t = toks[i].as_str();
        i += 1;
        if let Some(v) = t.strip_prefix("--") {
            let label = toks.get(i).and_then(|l| quoted(l)).map(String::from);
            i += label.is_some() as usize;
            out.push(Mark::Rule(num(v)?, label));
        } else if let Some(id) = t.strip_prefix('+') {
            out.push(Mark::Show(id.to_string()));
        } else if let Some(id) = t.strip_prefix('-') {
            out.push(Mark::Hide(id.to_string()));
        } else if let Some(id) = t.strip_prefix('=') {
            let mut vs = vec![];
            while let Some(v) = toks.get(i).and_then(|v| num(v)) {
                vs.push(v);
                i += 1;
            }
            if vs.is_empty() {
                return None;
            }
            out.push(Mark::Set(id.to_string(), vs));
        } else if let Some(id) = t.strip_prefix('!') {
            let g = glow.get_or_insert_with(Vec::new);
            if !id.is_empty() {
                g.push(id.to_string());
            }
        } else if let Some((a, b)) = t.strip_prefix('~').and_then(|r| r.split_once('>')) {
            out.push(Mark::Delta(a.to_string(), b.to_string()));
        } else if let Some((text, id)) = t.rsplit_once("\"@") {
            out.push(Mark::Note(id.to_string(), text.strip_prefix('"')?.to_string()));
        } else {
            return None;
        }
        if out.iter().any(|m| matches!(m, Mark::Show(s) | Mark::Hide(s) | Mark::Set(s, _) | Mark::Note(s, _) if s.is_empty())) {
            return None;
        }
    }
    out.extend(glow.map(Mark::Glow));
    (!out.is_empty()).then_some(out)
}

/// Take the `{}` markers out of a cue's text: the text to speak and show, and where each one sat.
/// Braces inside backticks and ones that hold no markers stay in the text.
pub fn split_say(raw: &str) -> (String, Vec<CueMark>) {
    let mut text = String::new();
    let mut found: Vec<(usize, Vec<Mark>)> = vec![];
    let mut rest = raw;
    let mut code = false;
    while let Some(i) = rest.find(['{', '`']) {
        let (before, from) = rest.split_at(i);
        text.push_str(before);
        if let Some(r) = from.strip_prefix('`') {
            code = !code;
            text.push('`');
            rest = r;
            continue;
        }
        match from.find('}').filter(|_| !code).and_then(|e| parse_marks(&from[1..e]).map(|m| (e, m))) {
            Some((e, m)) => {
                // `word {+a} word` keeps one space, `word {+a}.` none
                if text.ends_with(' ') {
                    text.pop();
                }
                found.push((text.chars().count(), m));
                rest = &from[e + 1..];
            }
            None => {
                text.push('{');
                rest = &from[1..];
            }
        }
    }
    text.push_str(rest);
    let lead = text.len() - text.trim_start().len();
    let text = text.trim().to_string();
    let of = text.chars().count();
    (text, found.into_iter().map(|(at, marks)| CueMark { at: at.saturating_sub(lead).min(of), of, marks }).collect())
}

/// The cue text with its markers back in place (inverse of [`split_say`]).
pub fn join_say(say: &str, marks: &[CueMark]) -> String {
    let chars: Vec<char> = say.chars().collect();
    let mut out = String::new();
    let mut k = 0;
    for m in marks {
        let at = m.at.min(chars.len());
        out.extend(&chars[k.min(at)..at]);
        k = at;
        if !(out.is_empty() || out.ends_with(' ')) {
            out.push(' ');
        }
        out.push_str(&format!("{{{}}}", marks_text(&m.marks)));
        if chars.get(at).is_some_and(|c| c.is_alphanumeric() || *c == '`') {
            out.push(' ');
        }
    }
    out.extend(&chars[k.min(chars.len())..]);
    out
}

fn fmt(v: f64) -> String {
    v.to_string()
}

pub fn marks_text(ms: &[Mark]) -> String {
    ms.iter()
        .map(|m| match m {
            Mark::Show(id) => format!("+{id}"),
            Mark::Hide(id) => format!("-{id}"),
            Mark::Set(id, vs) => format!("={id} {}", vs.iter().map(|v| fmt(*v)).collect::<Vec<_>>().join(" ")),
            Mark::Glow(ids) if ids.is_empty() => "!".into(),
            Mark::Glow(ids) => ids.iter().map(|i| format!("!{i}")).collect::<Vec<_>>().join(" "),
            Mark::Delta(a, b) => format!("~{a}>{b}"),
            Mark::Rule(v, l) => format!("--{}{}", fmt(*v), l.as_ref().map(|l| format!(" \"{l}\"")).unwrap_or_default()),
            Mark::Note(id, t) => format!("\"{t}\"@{id}"),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// What is drawn after some markers fired.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct State {
    /// per row
    pub shown: Vec<bool>,
    /// per row, one value per series
    pub values: Vec<Vec<f64>>,
    /// set by the last `!` marker; `None` until one fired (the cue's `!` refs glow then)
    pub glow: Option<Vec<String>>,
    pub deltas: Vec<(String, String)>,
    pub rules: Vec<(f64, Option<String>)>,
    pub notes: Vec<(String, String)>,
}

impl Chart {
    pub fn ids(&self) -> Vec<&str> {
        self.rows.iter().map(|r| r.id.as_str()).collect()
    }

    pub fn row(&self, id: &str) -> Option<usize> {
        self.rows.iter().position(|r| r.id == id)
    }

    /// The chart as written, then each marker in order; markers on ids of other charts are skipped.
    pub fn state<'a>(&self, marks: impl IntoIterator<Item = &'a Mark>) -> State {
        let mut s = State { shown: self.rows.iter().map(|r| !r.hidden).collect(), values: self.rows.iter().map(|r| r.values.clone()).collect(), ..Default::default() };
        let has = |id: &str| self.row(id).is_some();
        for m in marks {
            match m {
                Mark::Show(id) | Mark::Hide(id) => {
                    if let Some(i) = self.row(id) {
                        s.shown[i] = matches!(m, Mark::Show(_));
                    }
                }
                Mark::Set(id, vs) => {
                    if let Some(i) = self.row(id) {
                        for (k, v) in vs.iter().enumerate().take(s.values[i].len()) {
                            s.values[i][k] = *v;
                        }
                    }
                }
                Mark::Glow(ids) => {
                    if ids.is_empty() || ids.iter().any(|i| has(i)) {
                        s.glow = Some(ids.iter().filter(|i| has(i)).cloned().collect());
                    }
                }
                Mark::Delta(a, b) => {
                    if has(a) && has(b) && !s.deltas.contains(&(a.clone(), b.clone())) {
                        s.deltas.push((a.clone(), b.clone()));
                    }
                }
                Mark::Rule(v, l) => s.rules.push((*v, l.clone())),
                Mark::Note(id, t) => {
                    if has(id) {
                        s.notes.retain(|(i, _)| i != id);
                        s.notes.push((id.clone(), t.clone()));
                    }
                }
            }
        }
        s
    }

    /// Ids a marker names that are not rows of this chart.
    pub fn unknown<'a>(&self, m: &'a Mark) -> Vec<&'a str> {
        let ids: Vec<&str> = match m {
            Mark::Show(i) | Mark::Hide(i) | Mark::Set(i, _) | Mark::Note(i, _) => vec![i],
            Mark::Glow(v) => v.iter().map(String::as_str).collect(),
            Mark::Delta(a, b) => vec![a, b],
            Mark::Rule(..) => vec![],
        };
        ids.into_iter().filter(|i| self.row(i).is_none()).collect()
    }

    /// The value axis: 0 to a round number above the largest value shown (stacks: the largest
    /// sum), rules included, and the step between its ticks.
    pub fn range(&self, s: &State) -> (f64, f64, f64) {
        if self.kind == Kind::StackPct {
            return (0.0, 100.0, 25.0);
        }
        let vals = s.values.iter().zip(&s.shown).filter(|(_, sh)| **sh).map(|(v, _)| if self.kind == Kind::Stack { v.iter().sum() } else { v.iter().cloned().fold(f64::MIN, f64::max) });
        let all: Vec<f64> = vals.chain(s.rules.iter().map(|r| r.0)).collect();
        let lo = all.iter().cloned().fold(0.0, f64::min);
        let hi = all.iter().cloned().fold(0.0, f64::max);
        let step = nice((hi - lo) / 4.0);
        (step * (lo / step).floor(), step * (hi / step).ceil().max(1.0), step)
    }
}

/// A round step at or above `v` (1, 2, 5 × 10ⁿ).
pub fn nice(v: f64) -> f64 {
    if v <= 0.0 {
        return 1.0;
    }
    let p = 10f64.powf(v.log10().floor());
    [1.0, 2.0, 5.0, 10.0].iter().map(|m| m * p).find(|x| *x >= v * 0.999).unwrap_or(10.0 * p)
}

/// `-71%` from a to b.
pub fn delta_text(a: f64, b: f64) -> String {
    if a == 0.0 {
        return format!("{:+}", b - a);
    }
    let pct = (b - a) / a.abs() * 100.0;
    format!("{}{:.0}%", if pct >= 0.0 { "+" } else { "−" }, pct.abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block() {
        let c = parse("line x: version y: build time unit: s\nv1 42\nv2 35 \"version 2\" :::hidden", Some("Build"));
        assert!(c.errors.is_empty(), "{:?}", c.errors);
        assert_eq!((c.kind, c.y.as_deref(), c.unit.as_deref()), (Kind::Line, Some("build time"), Some("s")));
        assert_eq!(c.rows[1].label, "version 2");
        assert!(c.rows[1].hidden);
        let s = parse("stack\nseries: a b\nx 1", None);
        assert_eq!(s.errors.len(), 1);
    }

    #[test]
    fn markers() {
        let (t, m) = split_say("Caching cut it {+v2 !v2} a bit, `fmt {x}` {not one} {=v1 18 --30 \"goal\" \"fast\"@v3 ~v1>v3}.");
        assert_eq!(t, "Caching cut it a bit, `fmt {x}` {not one}.");
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].at, "Caching cut it".chars().count());
        assert_eq!(m[0].marks, vec![Mark::Show("v2".into()), Mark::Glow(vec!["v2".into()])]);
        assert_eq!(m[1].marks[1], Mark::Rule(30.0, Some("goal".into())));
        assert_eq!(m[1].marks[2], Mark::Note("v3".into(), "fast".into()));
        assert_eq!(join_say(&t, &m), "Caching cut it {+v2 !v2} a bit, `fmt {x}` {not one} {=v1 18 --30 \"goal\" \"fast\"@v3 ~v1>v3}.");
        let (t, m) = split_say("{+a} Starts here.");
        assert_eq!((t.as_str(), m[0].at), ("Starts here.", 0));
    }

    #[test]
    fn fold() {
        let c = parse("bar\na 10\nb 20 :::hidden", None);
        let ms = [Mark::Show("b".into()), Mark::Set("a".into(), vec![40.0]), Mark::Glow(vec!["b".into()]), Mark::Show("zz".into())];
        let s = c.state(&ms);
        assert_eq!(s.shown, vec![true, true]);
        assert_eq!(s.values[0], vec![40.0]);
        assert_eq!(s.glow, Some(vec!["b".to_string()]));
        assert_eq!(c.range(&s), (0.0, 40.0, 10.0));
        assert_eq!(delta_text(42.0, 12.0), "−71%");
    }
}
