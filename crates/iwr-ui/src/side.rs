//! Side panel: what a codecast's `|` lines show next to the view — a byte layout (```` ```bytes ````),
//! markdown (```` ```md ````: a list, a table, notes), a small diagram (```` ```mermaid ````) or an
//! example (```` ```json ````, ```` ```csv ````…). `!` refs glow byte fields, diagram nodes, example
//! keys and columns by id.

use crate::md::Markdown;
use crate::State;
use dioxus::prelude::*;
use iwr_core::bytes::{self, Layout, Size};
use iwr_core::script::Side;

/// Field colours, in order; padding (`_`) is grey.
const PALETTE: [&str; 8] = ["#60a5fa", "#34d399", "#fbbf24", "#f472b6", "#a78bfa", "#2dd4bf", "#fb923c", "#f87171"];
const PADDING: &str = "#9ca3af";

#[component]
pub fn SidePanel() -> Element {
    let mut state = use_context::<State>();
    let Some(side) = state.side.read().clone() else { return rsx! {} };
    let dw = *state.details_w.read();
    let (kind, title) = match side.as_ref() {
        Side::Bytes(l) => ("bytes", l.title.clone()),
        Side::Note(n) => ("notes", n.title.clone()),
        Side::Diagram(d) => ("diagram", d.title.clone()),
        Side::Example(e) => (e.lang.as_str(), e.title.clone()),
    };
    rsx! {
        div { class: "side", style: "width:{dw}px",
            div { class: "side-head",
                span { class: "kind", "{kind}" }
                span { class: "title", {title.unwrap_or_default()} }
                div { class: "spacer" }
                button { class: "small", title: "close the side panel (the next `|` line opens it again)", onclick: move |_| state.side.set(None), "✕" }
            }
            div { class: "side-body",
                match side.as_ref() {
                    Side::Bytes(l) => rsx! { BytesCard { layout: l.clone() } },
                    Side::Note(n) => rsx! { div { class: "side-note", Markdown { text: n.text.clone() } } },
                    Side::Diagram(d) => rsx! { MiniDiagram { diagram: d.clone() } },
                    Side::Example(e) => rsx! { ExampleCard { example: e.clone() } },
                }
            }
        }
    }
}

/// One cell of the byte strip.
struct Cell {
    field: Option<usize>,
    byte: Option<u8>,
    /// first cell of its field on its row: carries the field's name
    first: bool,
    /// a variable-size field without a sample: drawn as a few dotted cells
    open_size: bool,
}

fn color(l: &Layout, i: usize) -> &'static str {
    if l.fields[i].is_padding() {
        return PADDING;
    }
    let n = l.fields[..i].iter().filter(|f| !f.is_padding()).count();
    PALETTE[n % PALETTE.len()]
}

#[component]
fn BytesCard(layout: Layout) -> Element {
    let state = use_context::<State>();
    let mut hover = use_signal(|| None::<usize>);
    let l = layout;
    let placed = bytes::place(&l);
    let hl = state.hl_ids.read().clone();
    let lit: Vec<bool> = l.fields.iter().map(|f| !f.is_padding() && hl.contains(&f.id)).collect();
    let any_lit = lit.iter().any(|x| *x);

    // the strip: one cell per byte, fields of unknown size as three dotted cells
    let mut cells: Vec<Cell> = Vec::new();
    let mut known = true;
    for p in &placed {
        let (n, open_size) = match p.len {
            Some(n) => (n, false),
            None => (3, true),
        };
        for k in 0..n {
            let byte = p.offset.filter(|_| !open_size).and_then(|o| l.sample.get(o + k)).copied();
            cells.push(Cell { field: Some(p.field), byte, first: k == 0, open_size });
        }
        known &= !open_size;
    }
    if known {
        let used: usize = placed.iter().filter_map(|p| p.len).sum();
        for b in l.sample.iter().skip(used) {
            cells.push(Cell { field: None, byte: Some(*b), first: false, open_size: false });
        }
    }
    let row = l.row.max(1);
    let n_rows = cells.len().div_ceil(row);
    // offsets are exact up to the first field of unknown size
    let first_unsized = cells.iter().position(|c| c.open_size).unwrap_or(usize::MAX);
    let h = *hover.read();

    rsx! {
        div { class: "bytes",
            div { class: "strip", style: "--row:{row}",
                for r in 0..n_rows {
                    div { key: "{r}", class: "brow",
                        div { class: "off", if r * row <= first_unsized { {format!("{:04x}", r * row)} } else { "…" } }
                        for (k, c) in cells.iter().enumerate().skip(r * row).take(row) {
                            {
                                let f = c.field;
                                let col = f.map(|i| color(&l, i)).unwrap_or(PADDING);
                                let on = f.map(|i| lit[i] || h == Some(i)).unwrap_or(false);
                                let dim = (any_lit || h.is_some()) && !on;
                                let first = c.first || (k % row == 0 && f.is_some());
                                let name = f.filter(|_| first).map(|i| l.fields[i].id.clone()).unwrap_or_default();
                                let text = if c.open_size { "··".to_string() } else { c.byte.map(|b| format!("{:02x}", b)).unwrap_or_default() };
                                let class = format!("cell{}{}{}{}", if on { " on" } else { "" }, if dim { " dim" } else { "" }, if c.open_size { " var" } else { "" }, if f.is_none() { " extra" } else { "" });
                                rsx! {
                                    div { key: "{k}", class: "{class}", style: "--c:{col}",
                                        onmouseenter: move |_| hover.set(f),
                                        onmouseleave: move |_| hover.set(None),
                                        if !name.is_empty() { span { class: "n", "{name}" } }
                                        span { class: "b", "{text}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            table { class: "fields",
                thead { tr { th {} th { "field" } th { "at" } th { "size" } if !l.sample.is_empty() { th { "value" } } th {} } }
                tbody {
                    for p in placed.iter() {
                        {
                            let i = p.field;
                            let f = &l.fields[i];
                            let on = lit[i] || h == Some(i);
                            let dim = (any_lit || h.is_some()) && !on;
                            let col = color(&l, i);
                            let at = p.offset.map(|o| o.to_string()).unwrap_or_else(|| "?".into());
                            let size = match (&f.size, p.len) {
                                (_, Some(n)) if !matches!(f.size, Size::Fixed(_)) => format!("{} ({})", n, size_word(&f.size)),
                                (_, Some(n)) => n.to_string(),
                                (s, None) => size_word(s),
                            };
                            let id = if f.is_padding() { "—".to_string() } else { f.id.clone() };
                            let value = p.value.clone().unwrap_or_default();
                            let desc = f.desc.clone();
                            rsx! {
                                tr { key: "{i}", class: format!("{}{}", if on { "on" } else { "" }, if dim { " dim" } else { "" }),
                                    onmouseenter: move |_| hover.set(Some(i)),
                                    onmouseleave: move |_| hover.set(None),
                                    td { span { class: "sw", style: "background:{col}" } }
                                    td { class: "id", "{id}" }
                                    td { class: "num", "{at}" }
                                    td { class: "num", "{size}" }
                                    if !l.sample.is_empty() { td { class: "val", "{value}" } }
                                    td { class: "desc", Markdown { text: desc } }
                                }
                            }
                        }
                    }
                }
            }
            if !l.errors.is_empty() {
                div { class: "errs", for e in l.errors.iter() { div { "⚠ {e}" } } }
            }
        }
    }
}

/// Sample data: a csv as a table, anything else as text; `!` glows columns or the lines of keys.
#[component]
fn ExampleCard(example: iwr_core::script::Example) -> Element {
    let state = use_context::<State>();
    let hl = state.hl_ids.read().clone();
    if let Some(rows) = example.table() {
        let head = rows.first().cloned().unwrap_or_default();
        let lit: Vec<bool> = head.iter().map(|h| hl.contains(h)).collect();
        let any = lit.iter().any(|x| *x);
        let cls = move |i: usize| if lit.get(i).copied().unwrap_or(false) { "on" } else if any { "dim" } else { "" };
        return rsx! {
            table { class: "example-table",
                thead { tr { for (i, h) in head.iter().enumerate() { th { key: "{i}", class: cls(i), "{h}" } } } }
                tbody {
                    for (r, row) in rows.iter().enumerate().skip(1) {
                        tr { key: "{r}", for (i, c) in row.iter().enumerate() { td { key: "{i}", class: cls(i), "{c}" } } }
                    }
                }
            }
        };
    }
    let any = example.text.lines().any(|l| iwr_core::script::line_key(l).is_some_and(|k| hl.iter().any(|h| h == k)));
    rsx! {
        pre { class: "example",
            for (i, l) in example.text.lines().enumerate() {
                {
                    let on = iwr_core::script::line_key(l).is_some_and(|k| hl.iter().any(|h| h == k));
                    rsx! { div { key: "{i}", class: if on { "on" } else if any { "dim" } else { "" }, "{l}" } }
                }
            }
        }
    }
}

fn size_word(s: &Size) -> String {
    match s {
        Size::Fixed(n) => n.to_string(),
        Size::Field(f) => f.clone(),
        Size::Rest => "rest".into(),
    }
}

/// A diagram drawn small and still (no camera, no moves): boxes, labels, straight edges.
#[component]
fn MiniDiagram(diagram: iwr_core::diagram::Diagram) -> Element {
    let state = use_context::<State>();
    let g = iwr_core::diagram::graph(&diagram, &[]);
    let hl = state.hl_ids.read().clone();
    let any = g.nodes.iter().any(|n| hl.contains(&n.id));
    let (w, h) = (g.width.max(10.0) + 20.0, g.height.max(10.0) + 20.0);
    let center = |id: &str| g.nodes.iter().find(|n| n.id == id).map(|n| (n.x + n.w / 2.0, n.y + n.h / 2.0));
    let mut groups: Vec<&iwr_core::views::VNode> = g.nodes.iter().filter(|n| n.container).collect();
    groups.sort_by_key(|n| n.depth);
    let boxes: Vec<&iwr_core::views::VNode> = g.nodes.iter().filter(|n| !n.container).collect();
    rsx! {
        svg { class: "mini", view_box: "-10 -10 {w} {h}", preserve_aspect_ratio: "xMidYMin meet",
            for n in groups.iter() { {mini_node(n, &hl, any)} }
            for e in g.edges.iter() {
                {
                    let pts: Vec<(f64, f64)> = if e.points.len() >= 2 { e.points.clone() } else { [center(&e.from), center(&e.to)].into_iter().flatten().collect() };
                    let d = pts.iter().map(|(x, y)| format!("{:.1},{:.1}", x, y)).collect::<Vec<_>>().join(" ");
                    let on = hl.contains(&e.from) && hl.contains(&e.to);
                    rsx! { polyline { key: "{e.id}", points: "{d}", fill: "none", stroke: if on { "var(--hi)" } else { "var(--muted)" }, stroke_width: "1.4" } }
                }
            }
            for n in boxes.iter() { {mini_node(n, &hl, any)} }
        }
    }
}

fn mini_node(n: &iwr_core::views::VNode, hl: &[String], any: bool) -> Element {
    let on = hl.contains(&n.id);
    let col = crate::theme::node_color(n.kind);
    let op = if any && !on && !n.container { 0.35 } else { 1.0 };
    let (tx, ty) = (n.x + n.w / 2.0, if n.container { n.y + 14.0 } else { n.y + n.h / 2.0 + 4.0 });
    rsx! {
        g { key: "{n.id}", opacity: "{op}",
            rect { x: "{n.x}", y: "{n.y}", width: "{n.w}", height: "{n.h}", rx: "8",
                fill: if n.container { "var(--panel3)" } else { col }, fill_opacity: if n.container { "0.5" } else { "var(--fill-op)" },
                stroke: if on { "var(--hi)" } else { col }, stroke_width: if on { "3" } else { "1.2" } }
            text { x: "{tx}", y: "{ty}", text_anchor: "middle", font_size: "12", fill: "var(--text)", "{n.label}" }
        }
    }
}
