//! Scenes of a codecast: a ```` ```scene Title ```` block places objects on a grid and joins them
//! with lines, where a diagram would lay them out itself. Position means something here: slots of
//! a buffer, machines on a network, cells of a matrix.
//!
//! ```text
//! grid: 8x3 lines                 optional: columns x rows; `lines` draws the cells, `square` makes them square
//! head  icon  1,1  "👉"           `id [shape] col,row [WxH] ["label"] [:::hidden] [:::off]`
//! a     1,2  "A"                  shape defaults to box; cells count from 1,1 at the top left
//! disk  store 7,2 2x1 "💾 Disk"   two cells wide
//! a -->|flush| disk               a line between objects (or cells: `1,3 --- 8,3`), ids `l1`, `l2`…
//! ```
//!
//! The markers of [`crate::chart`] move it while the voice speaks: `{>head 2,1}` glides an object
//! to a cell (or onto another object), `{=a off}` greys it, `{=a "B"}` relabels it, `{+id}` /
//! `{-id}` show and hide, `{!id}` glows. [`Scene::state`] folds the markers fired so far;
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
        }
    }

    /// Drawn with an outline its label has to fit in.
    fn framed(self) -> bool {
        !matches!(self, Shape::Icon | Shape::Text | Shape::Zone)
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
    let (mut q, mut bar) = (false, false);
    for (i, ch) in t.char_indices() {
        match ch {
            '"' => q = !q,
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
        // Anything else is an object, its id first
        let toks = tokens(t);
        let mut o = Obj { id: toks[0].clone(), w: 1, h: 1, ..Default::default() };
        if !ident(&o.id) {
            s.errors.push(format!("line {n}: `{}` is not an id (letters, digits, `_`); write `id [shape] col,row [WxH] \"label\"`", o.id));
            continue;
        }
        let (mut placed, mut label) = (false, None);
        // Then its words in any order: state, label, shape, cell, size
        for tk in &toks[1..] {
            if let Some(c) = tk.strip_prefix(":::") {
                match c {
                    "hidden" => o.hidden = true,
                    "on" => o.state = None,
                    c if STATES.contains(&c) => o.state = Some(c.to_string()),
                    _ => s.errors.push(format!("line {n}: unknown `:::{c}` (hidden, {})", STATES.join(", "))),
                }
            } else if let Some(l) = quoted(tk) {
                label = Some(l.to_string());
            } else if let Some(sh) = Shape::of(tk) {
                o.shape = sh;
            } else if let Some((c, r)) = cell(tk) {
                (o.col, o.row) = (c, r);
                placed = true;
            } else if let Some((w, h)) = dims(tk) {
                (o.w, o.h) = (w, h);
            } else {
                s.errors.push(format!("line {n}: cannot read `{tk}` (shapes: box round pill diamond store icon text zone; a cell `3,2`; a size `2x1`; a \"label\")"));
            }
        }
        if !placed {
            s.errors.push(format!("line {n}: `{}` has no cell (`col,row`, from 1,1 at the top left)", o.id));
            continue;
        }
        o.label = label.unwrap_or_else(|| o.id.clone());
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
            if !o.shape.framed() {
                continue;
            }
            let (_, _, w, h) = self.rect(o.col, o.row, o.w, o.h, o.shape);
            let room = if o.shape == Shape::Diamond { w * 0.7 } else { w };
            let rows_of: Vec<&str> = o.label.split("<br>").collect();
            let need = rows_of.iter().map(|l| crate::views::display_width(l.trim())).fold(0.0, f64::max) * 7.6 + 16.0;
            if need > room || rows_of.len() as f64 * 18.0 > h - 4.0 {
                errs.push(format!("`{}`: the label \"{}\" does not fit its {}x{} cell(s): shorten it, break it with `<br>`, or widen the object (`{}x{}`)", o.id, o.label, o.w, o.h, o.w + 1, o.h));
            }
        }
        // No two objects on the same cells
        for (i, a) in self.objs.iter().enumerate() {
            for b in &self.objs[i + 1..] {
                let apart = a.col + a.w as f64 <= b.col || b.col + b.w as f64 <= a.col || a.row + a.h as f64 <= b.row || b.row + b.h as f64 <= a.row;
                if !apart && !a.hidden && !b.hidden && a.shape != Shape::Zone && b.shape != Shape::Zone {
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
            line_shown: self.lines.iter().map(|l| !l.hidden).collect(),
            line_label: self.lines.iter().map(|l| l.label.clone()).collect(),
            ..Default::default()
        };
        // Each marker fired so far changes it, in order
        for m in marks {
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
                        s.label[i] = text.clone();
                    } else if let Some(i) = self.line(id) {
                        s.line_label[i] = Some(text.clone());
                    }
                }
                Mark::Glow(ids) if ids.is_empty() || ids.iter().any(|i| self.has(i)) => {
                    s.glow = Some(ids.iter().filter(|i| self.has(i)).cloned().collect());
                }
                Mark::Note(id, t) if self.obj(id).is_some() => {
                    s.notes.retain(|(i, _)| i != id);
                    s.notes.push((id.clone(), t.clone()));
                }
                _ => {}
            }
        }
        s
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
            Mark::Note(i, _) | Mark::State(i, _) => (vec![], vec![i]),
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
                    let (x, y, w, h) = self.rect(s.at[i].0, s.at[i].1, o.w, o.h, o.shape);
                    // a glyph is smaller than its cell
                    let k = if o.shape == Shape::Icon { 0.3 } else { 0.5 };
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
    /// per line; a line is drawn when its two ends are too
    pub line_shown: Vec<bool>,
    pub line_label: Vec<Option<String>>,
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
