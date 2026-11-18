//! Scenes of a codecast: a ```` ```scene Title ```` block places objects on a grid and joins them
//! with lines, where a diagram would lay them out itself. Position means something here: slots of
//! a buffer, machines on a network, cells of a matrix.
//!
//! ```text
//! grid: 8x3 lines                 optional: columns x rows; `lines` draws the cells, `square` makes them square
//! head  icon  1,1  "👉"           `id [shape] col,row [WxH] ["label"] [:::hidden] [:::off]`
//! a     1,2  "A"                  shape defaults to box; cells count from 1,1 at the top left
//! disk  store 7,2 2x1 "💾 Disk"   two cells wide
//! logo  3,1 2x2 ![Server](img/server.png)   an image (file next to the codecast), its caption under it
//! tok   icon 1,3 "🟡" :::replace  on another object it takes its place (default `:::under`: drawn under it)
//! a -->|flush| disk               a line between objects (or cells: `1,3 --- 8,3`), ids `l1`, `l2`…
//! ```
//!
//! The markers of [`crate::chart`] move it while the voice speaks: `{>head 2,1}` glides an object
//! to a cell (or onto another object), `{=a off}` greys it, `{=a "B"}` relabels it, `{+id}` /
//! `{-id}` show and hide, `{!id}` glows, `{=logo "![Down](img/down.png)"}` swaps an image, `{*led 250:#.}`
//! blinks it like a light ([`Blink`]). [`Scene::state`] folds the markers fired so far;
//! [`Scene::rect`] and [`Scene::segment`] say where things are drawn.

use crate::chart::{quoted, tokens, Mark};
use serde::{Deserialize, Serialize};

/// States an object can be in (`:::off`, `{=id warn}`); `on` goes back to none.
pub const STATES: [&str; 4] = ["off", "ok", "warn", "err"];

/// Largest grid: beyond it the labels are too small to read.
const MAX: (i32, i32) = (16, 16);
/// Columns of a `square` grid: its cells are narrower.
const MAX_SQUARE: i32 = 25;
/// Space left around an object inside its cells.
const PAD: f64 = 8.0;
/// Height of the strip an icon gets under the object it landed on, and the gap above it.
const STRIP: f64 = 24.0;
const GAP: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    #[default]
    Box,
    Round,
    Pill,
    Diamond,
    Store,
    /// no outline, the label drawn large: an emoji
    Icon,
    /// no outline, plain text
    Text,
    /// a dashed area behind other objects, titled at its top
    Zone,
    /// no outline, a picture (`![caption](file)`) filling its cells, the caption under it
    Image,
}

impl Shape {
    fn of(w: &str) -> Option<Shape> {
        Some(match w {
            "box" => Shape::Box,
            "round" => Shape::Round,
            "pill" => Shape::Pill,
            "diamond" => Shape::Diamond,
            "store" => Shape::Store,
            "icon" => Shape::Icon,
            "text" => Shape::Text,
            "zone" => Shape::Zone,
            "image" => Shape::Image,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Shape::Box => "box",
            Shape::Round => "round",
            Shape::Pill => "pill",
            Shape::Diamond => "diamond",
            Shape::Store => "store",
            Shape::Icon => "icon",
            Shape::Text => "text",
            Shape::Zone => "zone",
            Shape::Image => "image",
        }
    }

    /// Drawn with an outline its label has to fit in.
    fn framed(self) -> bool {
        !matches!(self, Shape::Icon | Shape::Text | Shape::Zone | Shape::Image)
    }
}

/// One object on the grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Obj {
    pub id: String,
    /// `<br>` breaks a line
    pub label: String,
    pub shape: Shape,
    /// top left cell, from 1; halves sit between two cells
    pub col: f64,
    pub row: f64,
    /// size in cells
    pub w: i32,
    pub h: i32,
    /// `:::hidden`: drawn from its `+id` marker on
    pub hidden: bool,
    /// one of [`STATES`]
    pub state: Option<String>,
    /// `![caption](file)`: the address of its picture (`label` is the caption)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub img: Option<String>,
    /// `:::replace` (an icon): landed on another object, it is drawn instead of it; else under it
    #[serde(default)]
    pub replace: bool,
}

/// How a light blinks: `{*id 250:#.}` (250 ms lit, 250 ms dark, for ever), `{*id 400:#. x3}`
/// (three blinks, then dark: the last step stays), `{*id 100:#.#.......}` (a double blink every second). One character
/// per step: `#` lit, `.` dark. Objects given a pattern by the same marker start together, so a
/// pattern over several lights is one timeline each, of the same length.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blink {
    /// length of one step
    pub step_ms: u32,
    /// one entry per step: lit or dark
    pub steps: Vec<bool>,
    /// played this many times, then the last step stays (`#`: lit, `.`: dark); `None`: for ever
    pub times: Option<u32>,
}

impl Blink {
    /// Shortest step: faster reads as a flicker on a screen.
    const MIN_STEP: u32 = 20;
    /// Longest step, and longest timeline.
    const MAX_STEP: u32 = 10_000;
    const MAX_STEPS: usize = 400;

    /// The words after `*id`: `<ms>:<#.…> [x<N>]`; empty is `None` (the light stops).
    pub fn parse(words: &str) -> Result<Option<Blink>, String> {
        let mut w = words.split_whitespace();
        let Some(first) = w.next() else { return Ok(None) };
        let form = "write `<ms>:<steps>` (`#` lit, `.` dark), then `x<N>` to stop after N times: `250:#.`, `400:#. x3`";
        let (ms, tl) = first.split_once(':').ok_or_else(|| format!("`{first}`: {form}"))?;
        let step_ms: u32 = ms.parse().map_err(|_| format!("`{ms}` is not a number of milliseconds: {form}"))?;
        if !(Self::MIN_STEP..=Self::MAX_STEP).contains(&step_ms) {
            return Err(format!("a step of {step_ms} ms: keep it between {} and {} ms", Self::MIN_STEP, Self::MAX_STEP));
        }
        if let Some(c) = tl.chars().find(|c| !matches!(c, '#' | '.')) {
            return Err(format!("`{c}` in `{tl}`: a step is `#` (lit) or `.` (dark)"));
        }
        if tl.is_empty() || tl.len() > Self::MAX_STEPS {
            return Err(format!("`{tl}`: between 1 and {} steps", Self::MAX_STEPS));
        }
        if !tl.contains('#') {
            return Err(format!("`{tl}` never lights: stop the light with `*id` alone"));
        }
        let times = match w.next() {
            None => None,
            Some(x) => Some(x.strip_prefix('x').and_then(|n| n.parse::<u32>().ok()).filter(|n| (1..=99).contains(n)).ok_or_else(|| format!("`{x}`: write `x3` to play it 3 times (1 to 99)"))?),
        };
        if let Some(x) = w.next() {
            return Err(format!("`{x}` after the pattern: {form}"));
        }
        Ok(Some(Blink { step_ms, steps: tl.chars().map(|c| c == '#').collect(), times }))
    }

    /// One round, in milliseconds.
    pub fn period_ms(&self) -> u32 {
        self.step_ms * self.steps.len() as u32
    }

    /// The rounds as runs of one look: lit or not, and where each starts, 0 to 1 of a round.
    pub fn runs(&self) -> Vec<(bool, f64)> {
        let n = self.steps.len() as f64;
        let mut out: Vec<(bool, f64)> = vec![];
        for (k, on) in self.steps.iter().enumerate() {
            if out.last().map(|r| r.0) != Some(*on) {
                out.push((*on, k as f64 / n));
            }
        }
        out
    }
}

/// One end of a line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum End {
    Obj(String),
    /// the middle of a cell
    Cell(f64, f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Stroke {
    /// `-->`
    #[default]
    Arrow,
    /// `---`
    Plain,
    /// `-.->`
    Dotted,
    /// `==>`
    Thick,
}

const ARROWS: [(&str, Stroke); 4] = [("-.->", Stroke::Dotted), ("==>", Stroke::Thick), ("-->", Stroke::Arrow), ("---", Stroke::Plain)];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Line {
    /// `l1`, `l2`… in the order written
    pub id: String,
    pub from: End,
    pub to: End,
    pub stroke: Stroke,
    pub label: Option<String>,
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Scene {
    pub title: Option<String>,
    /// the title as written: what `@ scene:<title>` names, also once `title` is translated
    pub name: Option<String>,
    pub cols: i32,
    pub rows: i32,
    /// `lines`: the cells are drawn
    pub ruled: bool,
    /// `square`: square cells (a board, a matrix) instead of wide ones
    pub square: bool,
    pub objs: Vec<Obj>,
    pub lines: Vec<Line>,
    pub errors: Vec<String>,
    /// the block as written (for `script::to_text`)
    pub source: String,
}

/// `![caption](file)` taken out of an object line: the line without it, the caption and the file.
fn take_image(t: &str) -> (String, Option<(String, String)>) {
    let Some(i) = t.find("![") else { return (t.to_string(), None) };
    let Some(end) = t[i..].find("](").and_then(|j| t[i + j..].find(')').map(|k| i + j + k + 1)) else { return (t.to_string(), None) };
    match crate::markdown::image(&t[i..end]) {
        Some(im) => (format!("{} {}", &t[..i], &t[end..]), Some(im)),
        None => (t.to_string(), None),
    }
}

/// `3,2` (or `2.5,1`: between two cells).
fn cell(t: &str) -> Option<(f64, f64)> {
    let (c, r) = t.split_once(',')?;
    let ok = |s: &str| !s.is_empty() && s.chars().all(|ch| ch.is_ascii_digit() || ch == '.');
    (ok(c) && ok(r)).then(|| Some((c.parse().ok()?, r.parse().ok()?)))?
}

/// `2x1`: a size in cells.
fn dims(t: &str) -> Option<(i32, i32)> {
    let (w, h) = t.split_once('x')?;
    let n = |s: &str| s.parse::<i32>().ok().filter(|v| *v > 0);
    Some((n(w)?, n(h)?))
}

fn ident(t: &str) -> bool {
    !t.is_empty() && t.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// The first arrow of a line outside quotes and `|labels|`: where it starts and which one.
fn find_arrow(t: &str) -> Option<(usize, &'static str, Stroke)> {
    let (mut q, mut bar, mut img) = (false, false, false);
    for (i, ch) in t.char_indices() {
        match ch {
            '"' => q = !q,
            // `![caption](file)`: a `--` in a file name is no arrow
            '!' if !q && !bar && t[i..].starts_with("![") => img = true,
            ')' if img => img = false,
            _ if img => {}
            '|' if !q => bar = !bar,
            _ if !q && !bar => {
                if let Some((a, s)) = ARROWS.iter().find(|(a, _)| t[i..].starts_with(a)) {
                    return Some((i, a, *s));
                }
            }
            _ => {}
        }
    }
    None
}

pub fn parse(src: &str, title: Option<&str>) -> Scene {
    let mut s = Scene { title: title.map(String::from), source: src.trim_end().to_string(), ..Default::default() };
    let mut grid_set = false;
    for (n, line) in src.lines().enumerate() {
        let t = line.trim();
        let n = n + 1;
        if t.is_empty() || t.starts_with("//") || t.starts_with("%%") {
            continue;
        }
        if let Some(r) = t.strip_prefix("title:") {
            s.title = Some(r.trim().to_string());
            continue;
        }
        // The grid line: its size and options
        if let Some(r) = t.strip_prefix("grid").filter(|r| r.starts_with([':', ' '])) {
            for w in r.trim_start_matches(':').split_whitespace() {
                match (w, dims(w)) {
                    (_, Some((c, r))) => {
                        (s.cols, s.rows) = (c, r);
                        grid_set = true;
                    }
                    ("lines", _) => s.ruled = true,
                    ("square", _) => s.square = true,
                    _ => s.errors.push(format!("line {n}: unknown `{w}` after `grid:` (a size like `8x3`, `lines`, `square`)")),
                }
            }
            continue;
        }
        // An arrow in it: lines between things
        if find_arrow(t).is_some() {
            s.read_lines(t, n);
            continue;
        }
        // Anything else is an object, its id first; `![caption](file)` makes it a picture
        let (t, img) = take_image(t);
        let toks = tokens(&t);
        let mut o = Obj { id: toks[0].clone(), w: 1, h: 1, ..Default::default() };
        if !ident(&o.id) {
            s.errors.push(format!("line {n}: `{}` is not an id (letters, digits, `_`); write `id [shape] col,row [WxH] \"label\"`", o.id));
            continue;
        }
        let (mut placed, mut label, mut shaped, mut land) = (false, None, false, None);
        // Then its words in any order: state, label, shape, cell, size
        for tk in &toks[1..] {
            if let Some(c) = tk.strip_prefix(":::") {
                match c {
                    "hidden" => o.hidden = true,
                    "on" => o.state = None,
                    "replace" | "under" => {
                        o.replace = c == "replace";
                        land = Some(c);
                    }
                    c if STATES.contains(&c) => o.state = Some(c.to_string()),
                    _ => s.errors.push(format!("line {n}: unknown `:::{c}` (hidden, replace, under, {})", STATES.join(", "))),
                }
            } else if let Some(l) = quoted(tk) {
                label = Some(l.to_string());
            } else if let Some(sh) = Shape::of(tk) {
                o.shape = sh;
                shaped = true;
            } else if let Some((c, r)) = cell(tk) {
                (o.col, o.row) = (c, r);
                placed = true;
            } else if let Some((w, h)) = dims(tk) {
                (o.w, o.h) = (w, h);
            } else {
                s.errors.push(format!("line {n}: cannot read `{tk}` (shapes: box round pill diamond store icon text zone image; a cell `3,2`; a size `2x1`; a \"label\"; a picture `![caption](file)`)"));
            }
        }
        if !placed {
            s.errors.push(format!("line {n}: `{}` has no cell (`col,row`, from 1,1 at the top left)", o.id));
            continue;
        }
        if let Some(w) = land.filter(|_| o.shape != Shape::Icon) {
            s.errors.push(format!("line {n}: `:::{w}` says how an icon lands on another object: `{}` is a {}", o.id, o.shape.name()));
        }
        match img {
            Some((alt, src)) => {
                if !shaped {
                    o.shape = Shape::Image;
                }
                if !crate::markdown::safe_url(&src) {
                    s.errors.push(format!("line {n}: `{src}` is not an image address the page may load (a file next to the codecast, or `https://…`)"));
                }
                o.img = Some(src);
                // the caption: `![…]`, else a "label"
                o.label = if alt.is_empty() { label.unwrap_or_default() } else { alt };
            }
            None if o.shape == Shape::Image => {
                s.errors.push(format!("line {n}: `{}` is an `image` without a picture: write `![caption](file)`", o.id));
                continue;
            }
            None => o.label = label.unwrap_or_else(|| o.id.clone()),
        }
        if s.obj(&o.id).is_some() {
            s.errors.push(format!("line {n}: `{}` twice", o.id));
        }
        s.objs.push(o);
    }
    s.name = s.title.clone();
    if !grid_set {
        // no `grid:` line: the grid is what the objects and lines take
        let cells = s.lines.iter().flat_map(|l| [&l.from, &l.to]).filter_map(|e| match e {
            End::Cell(c, r) => Some((*c, *r)),
            End::Obj(_) => None,
        });
        let far: Vec<(f64, f64)> = s.objs.iter().map(|o| (o.col + o.w as f64 - 1.0, o.row + o.h as f64 - 1.0)).chain(cells).collect();
        s.cols = far.iter().map(|f| f.0.ceil() as i32).max().unwrap_or(1).max(1);
        s.rows = far.iter().map(|f| f.1.ceil() as i32).max().unwrap_or(1).max(1);
    }
    s.verify();
    s
}

impl Scene {
    /// `a -->|label| b --> 3,2 :::hidden`: one line per arrow.
    fn read_lines(&mut self, t: &str, n: usize) {
        let (t, hidden) = match t.strip_suffix(":::hidden") {
            Some(t) => (t.trim_end(), true),
            None => (t, false),
        };
        let end = |w: &str| -> Option<End> {
            let w = w.trim();
            cell(w).map(|(c, r)| End::Cell(c, r)).or_else(|| ident(w).then(|| End::Obj(w.to_string())))
        };
        let Some((i, a, mut stroke)) = find_arrow(t) else { return };
        let mut from = end(&t[..i]);
        let mut from_text = t[..i].trim();
        let mut rest = t[i + a.len()..].trim_start();
        loop {
            let mut label = None;
            if let Some(r) = rest.strip_prefix('|') {
                match r.split_once('|') {
                    Some((l, r)) => {
                        label = Some(l.trim().trim_matches('"').to_string()).filter(|l| !l.is_empty());
                        rest = r.trim_start();
                    }
                    None => {
                        self.errors.push(format!("line {n}: `|` of a line label is not closed (`a -->|label| b`)"));
                        return;
                    }
                }
            }
            let next = find_arrow(rest);
            let to_text = rest[..next.map_or(rest.len(), |x| x.0)].trim();
            let to = end(to_text);
            match (&from, &to) {
                (Some(f), Some(t)) => {
                    let id = format!("l{}", self.lines.len() + 1);
                    self.lines.push(Line { id, from: f.clone(), to: t.clone(), stroke, label, hidden });
                }
                _ => {
                    let bad = if from.is_none() { from_text } else { to_text };
                    self.errors.push(format!("line {n}: `{bad}` is not an end of a line (an object id, or a cell `3,2`)"));
                }
            }
            let Some((i, a, s)) = next else { return };
            (from, from_text, stroke) = (to, to_text, s);
            rest = rest[i + a.len()..].trim_start();
        }
    }

    /// What a block must hold to be drawn well: everything on the grid, known ids, nothing on
    /// top of something else, labels that fit their box.
    fn verify(&mut self) {
        let mut errs = vec![];
        let (cols, rows) = (self.cols, self.rows);
        if cols > (if self.square { MAX_SQUARE } else { MAX.0 }) || rows > MAX.1 {
            errs.push(format!("grid {cols}x{rows} is too large to read (at most {}x{}, {MAX_SQUARE}x{} with `square`)", MAX.0, MAX.1, MAX.1));
        }
        if self.objs.is_empty() {
            errs.push("no objects (`id [shape] col,row \"label\"` per line)".to_string());
        }
        let on_grid = |c: f64, r: f64, w: i32, h: i32| c >= 1.0 && r >= 1.0 && c + w as f64 - 1.0 <= cols as f64 && r + h as f64 - 1.0 <= rows as f64;
        // Every object sits on the grid, and its label fits its box
        for o in &self.objs {
            if !on_grid(o.col, o.row, o.w, o.h) {
                errs.push(format!("`{}` at {},{} ({}x{}) is outside the grid {cols}x{rows}", o.id, o.col, o.row, o.w, o.h));
            }
            if self.lines.iter().any(|l| l.id == o.id) {
                errs.push(format!("`{}` is also the id of a line (lines are `l1`, `l2`… in order): rename the object", o.id));
            }
            if o.shape == Shape::Image {
                let (_, _, w, _) = self.rect(o.col, o.row, o.w, o.h, o.shape);
                if crate::views::display_width(&o.label) * 6.8 > w + 16.0 {
                    errs.push(format!("`{}`: the caption \"{}\" is wider than its picture: shorten it or widen the object (`{}x{}`)", o.id, o.label, o.w + 1, o.h));
                }
            }
            if !o.shape.framed() {
                continue;
            }
            let (_, _, w, h) = self.rect(o.col, o.row, o.w, o.h, o.shape);
            let room = if o.shape == Shape::Diamond { w * 0.7 } else { w };
            let rows_of: Vec<&str> = o.label.split("<br>").collect();
            let need = rows_of.iter().map(|l| crate::views::display_width(l.trim())).fold(0.0, f64::max) * 7.6 + 16.0;
            // a picture in a box leaves one line under it for the label
            let tall = if o.img.is_some() { rows_of.len() > 1 || h < 52.0 } else { rows_of.len() as f64 * 18.0 > h - 4.0 };
            if need > room || tall {
                errs.push(format!("`{}`: the label \"{}\" does not fit its {}x{} cell(s): shorten it, break it with `<br>`, or widen the object (`{}x{}`)", o.id, o.label, o.w, o.h, o.w + 1, o.h));
            }
        }
        // No two objects on the same cells
        for (i, a) in self.objs.iter().enumerate() {
            for b in &self.objs[i + 1..] {
                let apart = a.col + a.w as f64 <= b.col || b.col + b.w as f64 <= a.col || a.row + a.h as f64 <= b.row || b.row + b.h as f64 <= a.row;
                // an icon may start on an object: it sits under it, or replaces it
                let lands = (a.shape == Shape::Icon) != (b.shape == Shape::Icon);
                if !apart && !lands && !a.hidden && !b.hidden && a.shape != Shape::Zone && b.shape != Shape::Zone {
                    errs.push(format!("`{}` and `{}` overlap: move one, make one `:::hidden`, or make the one behind a `zone`", a.id, b.id));
                }
            }
        }
        // Every line ends on an object or on a cell of the grid
        for l in &self.lines {
            for e in [&l.from, &l.to] {
                match e {
                    End::Obj(id) if self.obj(id).is_none() => errs.push(format!("line `{}`: no object `{id}` (objects: {})", l.id, self.objs.iter().map(|o| o.id.as_str()).collect::<Vec<_>>().join(" "))),
                    End::Cell(c, r) if !on_grid(*c, *r, 1, 1) => errs.push(format!("line `{}`: cell {c},{r} is outside the grid {cols}x{rows}", l.id)),
                    _ => {}
                }
            }
        }
        // A line straight through a third object reads as two lines
        let st = self.state([]);
        for l in self.lines.iter().filter(|l| !l.hidden) {
            let Some((a, b)) = self.segment(&st, l) else { continue };
            let is_end = |id: &str| [&l.from, &l.to].into_iter().any(|e| matches!(e, End::Obj(x) if x == id));
            for o in self.objs.iter().filter(|o| !o.hidden && o.shape != Shape::Zone && !is_end(&o.id)) {
                if crosses(a, b, self.rect(o.col, o.row, o.w, o.h, o.shape)) {
                    errs.push(format!("line `{}` runs through `{}`: move one of them, or bend the line through free cells (`a --- 2,3 --- 7,3 --> b`)", l.id, o.id));
                }
            }
        }
        self.errors.extend(errs);
    }

    pub fn obj(&self, id: &str) -> Option<usize> {
        self.objs.iter().position(|o| o.id == id)
    }

    pub fn line(&self, id: &str) -> Option<usize> {
        self.lines.iter().position(|l| l.id == id)
    }

    pub fn has(&self, id: &str) -> bool {
        self.obj(id).is_some() || self.line(id).is_some()
    }

    /// What `!` may glow: objects, then lines.
    pub fn ids(&self) -> Vec<&str> {
        self.objs.iter().map(|o| o.id.as_str()).chain(self.lines.iter().map(|l| l.id.as_str())).collect()
    }

    /// Width and height of a cell, in drawing units.
    pub fn cell(&self) -> (f64, f64) {
        if self.square { (76.0, 76.0) } else { (120.0, 76.0) }
    }

    /// Size of the whole drawing.
    pub fn size(&self) -> (f64, f64) {
        let (cw, ch) = self.cell();
        (self.cols.max(1) as f64 * cw, self.rows.max(1) as f64 * ch)
    }

    /// Where an object of `w`x`h` cells at `col`,`row` is drawn: x, y, width, height. A zone
    /// fills its cells, so the objects inside it stay inside its border.
    pub fn rect(&self, col: f64, row: f64, w: i32, h: i32, shape: Shape) -> (f64, f64, f64, f64) {
        let (cw, ch) = self.cell();
        let pad = if shape == Shape::Zone { 2.0 } else { PAD };
        ((col - 1.0) * cw + pad, (row - 1.0) * ch + pad, w as f64 * cw - 2.0 * pad, h as f64 * ch - 2.0 * pad)
    }

    /// The scene as written, then each marker in order; markers on other ids are skipped.
    pub fn state<'a>(&self, marks: impl IntoIterator<Item = &'a Mark>) -> State {
        let mut s = State {
            shown: self.objs.iter().map(|o| !o.hidden).collect(),
            at: self.objs.iter().map(|o| (o.col, o.row)).collect(),
            state: self.objs.iter().map(|o| o.state.clone()).collect(),
            label: self.objs.iter().map(|o| o.label.clone()).collect(),
            blink: vec![None; self.objs.len()],
            img: self.objs.iter().map(|o| o.img.clone()).collect(),
            line_shown: self.lines.iter().map(|l| !l.hidden).collect(),
            line_label: self.lines.iter().map(|l| l.label.clone()).collect(),
            ..Default::default()
        };
        // Each marker fired so far changes it, in order
        for (n, m) in marks.into_iter().enumerate() {
            match m {
                Mark::Show(id) | Mark::Hide(id) => {
                    let on = matches!(m, Mark::Show(_));
                    if let Some(i) = self.obj(id) {
                        s.shown[i] = on;
                    } else if let Some(i) = self.line(id) {
                        s.line_shown[i] = on;
                    }
                }
                Mark::Move(id, to) => {
                    if let (Some(i), Some(at)) = (self.obj(id), self.target(&s, id, to)) {
                        s.at[i] = at;
                    }
                }
                Mark::State(id, st) => {
                    if let Some(i) = self.obj(id) {
                        s.state[i] = Some(st.clone()).filter(|st| st != "on");
                    }
                }
                Mark::Label(id, text) => {
                    if let Some(i) = self.obj(id) {
                        // `{=id "![caption](file)"}` swaps the picture of a picture
                        match crate::markdown::image(text).filter(|_| s.img[i].is_some()) {
                            Some((alt, src)) => {
                                s.img[i] = Some(src).filter(|u| crate::markdown::safe_url(u));
                                s.label[i] = alt;
                            }
                            None => s.label[i] = text.clone(),
                        }
                    } else if let Some(i) = self.line(id) {
                        s.line_label[i] = Some(text.clone());
                    }
                }
                Mark::Glow(ids) if ids.is_empty() || ids.iter().any(|i| self.has(i)) => {
                    s.glow = Some(ids.iter().filter(|i| self.has(i)).cloned().collect());
                }
                Mark::Blink(id, p) => {
                    if let (Some(i), Ok(b)) = (self.obj(id), Blink::parse(p)) {
                        // which marker set it: the same pattern asked again starts again
                        s.blink[i] = b.map(|b| (b, n));
                    }
                }
                Mark::Note(id, t) if self.obj(id).is_some() => {
                    s.notes.retain(|(i, _)| i != id);
                    s.notes.push((id.clone(), t.clone()));
                }
                _ => {}
            }
        }
        s.landed = self.landings(&s);
        s
    }

    /// Per object: for an icon whose centre sits on another object (not an icon, not a zone),
    /// that object. One icon per object: a second one stays where it is, on top.
    fn landings(&self, s: &State) -> Vec<Option<usize>> {
        let mut landed = vec![None; self.objs.len()];
        for (j, ic) in self.objs.iter().enumerate().filter(|(j, o)| o.shape == Shape::Icon && s.shown[*j]) {
            let (cx, cy) = (s.at[j].0 + ic.w as f64 / 2.0, s.at[j].1 + ic.h as f64 / 2.0);
            landed[j] = (0..self.objs.len()).find(|&i| {
                let (o, (c, r)) = (&self.objs[i], s.at[i]);
                let under = matches!(o.shape, Shape::Icon | Shape::Zone) || !s.shown[i] || landed.contains(&Some(i));
                !under && cx > c && cx < c + o.w as f64 && cy > r && cy < r + o.h as f64
            });
        }
        landed
    }

    /// The icon that landed on object `i`.
    pub fn holder(&self, s: &State, i: usize) -> Option<usize> {
        s.landed.iter().position(|l| *l == Some(i))
    }

    /// Is object `i` drawn: shown, and not replaced by an icon (`:::replace`) that landed on it.
    pub fn drawn(&self, s: &State, i: usize) -> bool {
        s.shown[i] && !self.holder(s, i).is_some_and(|j| self.objs[j].replace)
    }

    /// Where object `i` is drawn now: x, y, width, height. An icon that landed on an object takes
    /// its place (`:::replace`) or a strip under it, and the object gives that strip up.
    pub fn place(&self, s: &State, i: usize) -> (f64, f64, f64, f64) {
        let at = |i: usize| {
            let o = &self.objs[i];
            self.rect(s.at[i].0, s.at[i].1, o.w, o.h, o.shape)
        };
        if let Some(t) = s.landed[i] {
            let (x, y, w, h) = at(t);
            return if self.objs[i].replace { (x, y, w, h) } else { (x, y + h - STRIP, w, STRIP) };
        }
        let (x, y, w, h) = at(i);
        match self.holder(s, i) {
            Some(j) if !self.objs[j].replace => (x, y, w, h - STRIP - GAP),
            _ => (x, y, w, h),
        }
    }

    /// Where `{>id to}` puts `id`: a cell, or centred on the object `to` where it is now.
    fn target(&self, s: &State, id: &str, to: &str) -> Option<(f64, f64)> {
        if let Some(c) = cell(to) {
            return Some(c);
        }
        let (o, t) = (&self.objs[self.obj(id)?], self.obj(to)?);
        let (tc, tr) = s.at[t];
        Some((tc + (self.objs[t].w - o.w) as f64 / 2.0, tr + (self.objs[t].h - o.h) as f64 / 2.0))
    }

    /// Ids a marker names that are not things of this scene.
    pub fn unknown<'a>(&self, m: &'a Mark) -> Vec<&'a str> {
        let (any, objs): (Vec<&str>, Vec<&str>) = match m {
            Mark::Show(i) | Mark::Hide(i) | Mark::Label(i, _) => (vec![i], vec![]),
            Mark::Glow(v) => (v.iter().map(String::as_str).collect(), vec![]),
            Mark::Note(i, _) | Mark::State(i, _) | Mark::Blink(i, _) => (vec![], vec![i]),
            Mark::Move(i, to) => (vec![], if cell(to).is_some() { vec![i] } else { vec![i, to] }),
            // chart markers
            Mark::Set(i, _) => (vec![], vec![i]),
            Mark::Delta(a, b) | Mark::Span(a, b, _) => (vec![], vec![a, b]),
            Mark::Series(n, _) => (vec![], vec![n]),
            Mark::Rule(..) | Mark::ClearRules | Mark::ClearSpans => (vec![], vec![]),
        };
        any.into_iter().filter(|i| !self.has(i)).chain(objs.into_iter().filter(|i| self.obj(i).is_none())).collect()
    }

    /// What is wrong with a marker on an object of this scene: a move off the grid.
    pub fn mark_error(&self, m: &Mark) -> Option<String> {
        if let Mark::Blink(id, p) = m {
            self.obj(id)?;
            return Blink::parse(p).err().map(|e| format!("`{{*{id} {p}}}`: {e}"));
        }
        if let Mark::Label(id, text) = m {
            let (o, (_, src)) = (&self.objs[self.obj(id)?], crate::markdown::image(text)?);
            return if o.img.is_none() {
                Some(format!("`{{={id} \"{text}\"}}`: `{id}` has no picture to swap (write it `![caption](file)` in the scene)"))
            } else {
                (!crate::markdown::safe_url(&src)).then(|| format!("`{{={id} \"{text}\"}}`: `{src}` is not an image address the page may load"))
            };
        }
        let Mark::Move(id, to) = m else { return None };
        let (o, (c, r)) = (&self.objs[self.obj(id)?], cell(to)?);
        let inside = c >= 1.0 && r >= 1.0 && c + o.w as f64 - 1.0 <= self.cols as f64 && r + o.h as f64 - 1.0 <= self.rows as f64;
        (!inside).then(|| format!("`{{>{id} {to}}}` moves `{id}` ({}x{}) outside the grid {}x{}", o.w, o.h, self.cols, self.rows))
    }

    /// The two ends of a line as drawn, on the borders of what it joins; `None` when an end is
    /// hidden or both are in the same place.
    pub fn segment(&self, s: &State, l: &Line) -> Option<((f64, f64), (f64, f64))> {
        // centre, half width, half height, diamond
        let place = |e: &End| -> Option<((f64, f64), f64, f64, bool)> {
            match e {
                End::Cell(c, r) => {
                    let (x, y, w, h) = self.rect(*c, *r, 1, 1, Shape::Box);
                    Some(((x + w / 2.0, y + h / 2.0), 0.0, 0.0, false))
                }
                End::Obj(id) => {
                    let i = self.obj(id).filter(|i| s.shown[*i])?;
                    let o = &self.objs[i];
                    let (x, y, w, h) = self.place(s, i);
                    // a glyph is smaller than its cell (an icon in place of the object too)
                    let k = if o.shape == Shape::Icon || !self.drawn(s, i) { 0.3 } else { 0.5 };
                    Some(((x + w / 2.0, y + h / 2.0), w * k + 4.0, h * k + 4.0, o.shape == Shape::Diamond))
                }
            }
        };
        let (a, b) = (place(&l.from)?, place(&l.to)?);
        let (dx, dy) = (b.0 .0 - a.0 .0, b.0 .1 - a.0 .1);
        // how far along the way from one centre to the other its border is, 0 to 1
        let out = |(_, hw, hh, diamond): &((f64, f64), f64, f64, bool)| -> f64 {
            if *hw == 0.0 {
                0.0
            } else if *diamond {
                1.0 / (dx.abs() / hw + dy.abs() / hh)
            } else {
                (hw / dx.abs()).min(hh / dy.abs())
            }
        };
        let (ta, tb) = (out(&a), out(&b));
        if ta + tb >= 1.0 {
            return None;
        }
        Some(((a.0 .0 + dx * ta, a.0 .1 + dy * ta), (b.0 .0 - dx * tb, b.0 .1 - dy * tb)))
    }
}

/// Does the segment from `a` to `b` enter the rectangle (x, y, width, height)?
fn crosses(a: (f64, f64), b: (f64, f64), (x, y, w, h): (f64, f64, f64, f64)) -> bool {
    // clip the segment to the rectangle, one side at a time
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for (p, q) in [(-dx, a.0 - x), (dx, x + w - a.0), (-dy, a.1 - y), (dy, y + h - a.1)] {
        if p == 0.0 {
            if q <= 0.0 {
                return false;
            }
        } else if p < 0.0 {
            t0 = t0.max(q / p);
        } else {
            t1 = t1.min(q / p);
        }
    }
    t0 < t1
}

/// What is drawn after some markers fired.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct State {
    /// per object
    pub shown: Vec<bool>,
    /// per object: its top left cell
    pub at: Vec<(f64, f64)>,
    /// per object: one of [`STATES`]
    pub state: Vec<Option<String>>,
    pub label: Vec<String>,
    /// per object: its blinking, and the index of the marker that set it
    pub blink: Vec<Option<(Blink, usize)>>,
    /// per object: its picture's address, for a picture
    pub img: Vec<Option<String>>,
    /// per line; a line is drawn when its two ends are too
    pub line_shown: Vec<bool>,
    pub line_label: Vec<Option<String>>,
    /// per object: for an icon, the object it landed on ([`Scene::place`])
    pub landed: Vec<Option<usize>>,
    /// set by the last `!` marker; `None` until one fired (the cue's `!` refs glow then)
    pub glow: Option<Vec<String>>,
    /// callouts: object id, text
    pub notes: Vec<(String, String)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::split_say;

    const RING: &str = "grid: 8x3 lines\nhead icon 1,1 \"👉\"\na 1,2 \"A\"\nb 2,2 \"B\" :::off\nspare 3,2 :::hidden\ndisk store 7,2 2x1 \"💾 Disk\"\nb -->|flush| disk\n1,3 --- 8,3\n";

    #[test]
    fn pictures() {
        let s = parse("grid: 6x3\nlogo 1,1 2x2 ![Server](img/a--b.png)\nbox round 4,1 2x2 ![](img/db.svg) \"Db\"\nlogo --> box\n", None);
        assert!(s.errors.is_empty(), "{:?}", s.errors);
        assert_eq!((s.objs[0].shape, s.objs[0].label.as_str(), s.objs[0].img.as_deref()), (Shape::Image, "Server", Some("img/a--b.png")));
        assert_eq!((s.objs[1].shape, s.objs[1].label.as_str(), s.objs[1].img.as_deref()), (Shape::Round, "Db", Some("img/db.svg")));
        assert_eq!(s.lines.len(), 1);
        let swap = Mark::Label("logo".into(), "![Down](img/down.png)".into());
        let st = s.state([&swap]);
        assert_eq!((st.label[0].as_str(), st.img[0].as_deref()), ("Down", Some("img/down.png")));
        assert_eq!(s.mark_error(&swap), None);
        assert!(s.mark_error(&Mark::Label("logo".into(), "![x](javascript:alert(1))".into())).is_some());
        let bad = parse("a image 1,1\nb 2,1 ![x](javascript:x)\n", None);
        assert_eq!(bad.errors.len(), 2, "{:?}", bad.errors);
    }

    #[test]
    fn block() {
        let s = parse(RING, Some("Ring"));
        assert!(s.errors.is_empty(), "{:?}", s.errors);
        assert_eq!((s.cols, s.rows, s.ruled, s.square), (8, 3, true, false));
        assert_eq!(s.ids(), vec!["head", "a", "b", "spare", "disk", "l1", "l2"]);
        assert_eq!((s.objs[0].shape, s.objs[1].shape, s.objs[1].label.as_str()), (Shape::Icon, Shape::Box, "A"));
        assert_eq!((s.objs[2].state.as_deref(), s.objs[3].hidden, s.objs[3].label.as_str()), (Some("off"), true, "spare"));
        assert_eq!((s.objs[4].w, s.objs[4].h), (2, 1));
        assert_eq!((s.lines[0].label.as_deref(), s.lines[0].stroke, s.lines[1].stroke), (Some("flush"), Stroke::Arrow, Stroke::Plain));
        assert_eq!(s.lines[1].from, End::Cell(1.0, 3.0));
        // without a `grid:` line the grid is what the objects take; a chain is one line per arrow
        let s = parse("a 1,1\nb 3,1 \"x --- y\"\nc 2,2 2x2\na --> b ==>|go on| c :::hidden", None);
        assert!(s.errors.is_empty(), "{:?}", s.errors);
        assert_eq!((s.cols, s.rows, s.lines.len()), (3, 3, 2));
        assert_eq!((s.lines[1].stroke, s.lines[1].label.as_deref(), s.lines[1].hidden), (Stroke::Thick, Some("go on"), true));
    }

    #[test]
    fn errors() {
        let e = |src: &str| parse(src, None).errors;
        assert!(e("grid: 2x2\na 3,1")[0].contains("outside the grid"));
        assert!(e("a 1,1\nb 1,1")[0].contains("overlap"));
        // a zone holds other objects, a hidden one waits where another one is
        assert!(e("z zone 1,1 2x2 \"Zone\"\na 1,1\nb 1,1 :::hidden").is_empty());
        assert!(e("a 1,1 \"a label much too long\"")[0].contains("does not fit"));
        assert!(e("a 1,1 2x1 \"a label<br>on two lines\"").is_empty());
        assert!(e("a 1,1\na --> zz")[0].contains("no object `zz`"));
        assert!(e("a 1,1\na 2,1")[0].contains("twice"));
        assert!(e("a box")[0].contains("no cell"));
        assert!(e("a 1,1 blob :::big").len() == 2);
        assert!(e("grid: 2x2")[0].contains("no objects"));
        assert!(e("l1 1,1\nb 2,1\nl1 --> b")[0].contains("id of a line"));
        // a line through a third object; bent through free cells it passes
        assert!(e("a 1,1\nb 2,1\nc 3,1\na --> c")[0].contains("runs through `b`"));
        assert!(e("a 1,1\nb 2,1\nc 3,1\na --- 1,2 --- 3,2 --> c").is_empty());
    }

    #[test]
    fn fold() {
        let s = parse(RING, None);
        let (text, m) = split_say("The head moves {>head 2,1 =b on !b} on, {+spare =a \"Z\" -l1 \"full\"@disk} then {>head disk =a err}.");
        assert_eq!(text, "The head moves on, then.");
        let ms: Vec<&Mark> = m.iter().flat_map(|m| &m.marks).collect();
        let st = s.state(ms[..3].iter().copied());
        assert_eq!((st.at[0], st.state[2].clone(), st.glow.clone()), ((2.0, 1.0), None, Some(vec!["b".to_string()])));
        assert_eq!((st.shown[3], st.line_shown[0]), (false, true));
        let st = s.state(ms.iter().copied());
        assert_eq!((st.shown[3], st.label[1].as_str(), st.line_shown[0], st.notes.len()), (true, "Z", false, 1));
        // onto a wider object: centred on it
        assert_eq!((st.at[0], st.state[1].as_deref()), ((7.5, 2.0), Some("err")));
        assert!(s.unknown(ms[0]).is_empty() && s.unknown(&Mark::Move("head".into(), "zz".into())) == vec!["zz"]);
        assert_eq!(s.unknown(&Mark::Note("l1".into(), "x".into())), vec!["l1"]);
        assert!(s.mark_error(&Mark::Move("disk".into(), "8,2".into())).is_some() && s.mark_error(ms[0]).is_none());
    }

    #[test]
    fn blinking() {
        let s = parse(RING, None);
        let (text, m) = split_say("It blinks {*a 250:#. *b 100:#.#....... x3} twice, {*a} then stops.");
        assert_eq!(text, "It blinks twice, then stops.");
        let ms: Vec<&Mark> = m.iter().flat_map(|m| &m.marks).collect();
        assert_eq!(crate::chart::marks_text(&m[0].marks), "*a 250:#. *b 100:#.#....... x3");
        let st = s.state(ms[..2].iter().copied());
        let (a, b) = (st.blink[1].clone().unwrap(), st.blink[2].clone().unwrap());
        assert_eq!((a.0.step_ms, a.0.period_ms(), a.0.times, a.1), (250, 500, None, 0));
        assert_eq!((b.0.period_ms(), b.0.times, b.0.runs()), (1000, Some(3), vec![(true, 0.0), (false, 0.1), (true, 0.2), (false, 0.3)]));
        // `*a` alone stops it; the other keeps blinking
        let st = s.state(ms.iter().copied());
        assert!(st.blink[1].is_none() && st.blink[2].is_some());
        // a bad pattern is reported, and does not blink
        for bad in ["fast", "5:#.", "250:#o", "250:...", "250:#. x0", "250:#. x2 y"] {
            let mk = Mark::Blink("a".into(), bad.into());
            assert!(s.mark_error(&mk).is_some(), "{bad}");
            assert!(s.state([&mk]).blink[1].is_none());
        }
        assert!(s.mark_error(ms[0]).is_none() && s.unknown(&Mark::Blink("zz".into(), String::new())) == vec!["zz"]);
        // a typo after the id is kept for the check, not spoken
        let (text, m) = split_say("Look {*a quick} here.");
        assert_eq!((text.as_str(), m[0].marks[0].clone()), ("Look here.", Mark::Blink("a".into(), "quick".into())));
    }

    #[test]
    fn landing() {
        let s = parse("grid: 4x2\na 1,1 \"A\"\nb 2,1 2x1 \"B\"\ntok icon 1,2 \"🟡\"\nme icon 4,2 \"👉\" :::replace\nz zone 1,2 2x1\nme --- 4,1", None);
        assert!(s.errors.is_empty(), "{:?}", s.errors);
        // in a zone, on no object: drawn in its own cell
        let st = s.state([]);
        assert_eq!((st.landed[2], st.landed[3]), (None, None));
        assert_eq!(s.place(&st, 2), s.rect(1.0, 2.0, 1, 1, Shape::Icon));
        // `under` (the default): the box gives up a strip at its bottom, the icon is drawn there
        let (onto_a, onto_b) = (Mark::Move("tok".into(), "a".into()), Mark::Move("me".into(), "b".into()));
        let st = s.state([&onto_a, &onto_b]);
        assert_eq!((st.landed[2], st.landed[3]), (Some(0), Some(1)));
        let (x, y, w, h) = s.rect(1.0, 1.0, 1, 1, Shape::Box);
        assert_eq!(s.place(&st, 0), (x, y, w, h - STRIP - GAP));
        assert_eq!(s.place(&st, 2), (x, y + h - STRIP, w, STRIP));
        assert!(s.drawn(&st, 0));
        // `replace`: the object is not drawn, the icon takes its whole place, its lines end on it
        assert!(!s.drawn(&st, 1) && st.shown[1]);
        assert_eq!(s.place(&st, 3), s.rect(2.0, 1.0, 2, 1, Shape::Box));
        let ((x1, _), _) = s.segment(&st, &s.lines[0]).unwrap();
        let (bx, _, bw, _) = s.rect(2.0, 1.0, 2, 1, Shape::Box);
        assert!(x1 < bx + bw - 20.0, "ends on the glyph, inside the box it replaced");
        // a second icon on the same object stays on top
        let st = s.state([&onto_a, &Mark::Move("me".into(), "a".into())]);
        assert_eq!((st.landed[2], st.landed[3]), (Some(0), None));
        // only an icon says how it lands; it may start on an object
        assert!(parse("a 1,1\nb 2,1 :::replace", None).errors[0].contains("`:::replace` says how an icon lands"));
        assert!(parse("a 1,1\nt icon 1,1 \"x\"", None).errors.is_empty());
        assert!(parse("t icon 1,1\nu icon 1,1", None).errors[0].contains("overlap"));
    }

    #[test]
    fn segments() {
        let s = parse("a 1,1\nb 3,1\nd diamond 1,3\na --> b\na --> d\na --> a", None);
        let st = s.state([]);
        let ((x1, y1), (x2, y2)) = s.segment(&st, &s.lines[0]).unwrap();
        // from the right border of `a` to the left border of `b`, a small gap kept
        assert_eq!((x1.round(), y1, x2.round(), y2), (116.0, 38.0, 244.0, 38.0));
        let ((_, y1), (_, y2)) = s.segment(&st, &s.lines[1]).unwrap();
        assert_eq!((y1.round(), y2.round()), (72.0, 156.0));
        assert!(s.segment(&st, &s.lines[2]).is_none());
        assert!(s.segment(&s.state([&Mark::Hide("b".into())]), &s.lines[0]).is_none());
    }
}
