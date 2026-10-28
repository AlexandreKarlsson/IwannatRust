//! Charts of a codecast (`@ chart`, `| chart`): an SVG drawn from [`iwr_core::chart::State`], the
//! chart as the markers fired so far left it. Marks are keyed by row id so a new one mounts with
//! its entry animation (a line draws to it, a bar grows, a slice sweeps) and a changed one glides
//! there through CSS transitions.

use crate::State;
use dioxus::prelude::*;
use iwr_core::chart::{delta_text, Chart, Kind};

const L: f64 = 56.0;
const R: f64 = 24.0;
const T: f64 = 36.0;
const B: f64 = 44.0;

fn num(v: f64) -> String {
    if v.fract() == 0.0 || v.abs() >= 100.0 { format!("{:.0}", v) } else { format!("{:.1}", v) }
}

#[component]
pub fn ChartView(chart: Chart, #[props(default)] full: bool) -> Element {
    let state = use_context::<State>();
    let (part, cue, frac) = (*state.part_idx.read(), *state.cue_idx.read(), *state.cue_frac.read());
    let script = state.script.read().clone();
    let marks = script.as_ref().and_then(|s| s.parts.get(part)).map(|p| iwr_core::script::marks_at(p, cue, frac)).unwrap_or_default();
    let st = chart.state(marks);
    // the last `!` marker, else the cue's `!` refs
    let lit: Vec<String> = st.glow.clone().unwrap_or_else(|| state.hl_ids.read().clone());
    let c = &chart;
    let glow = |id: &str| if lit.iter().any(|g| g == id) { " glow" } else { "" };
    // the side panel is narrow: a smaller canvas keeps the text readable
    let (w, h) = if full { (760.0, 400.0) } else { (420.0, 340.0) };
    let color = |i: usize| crate::side::PALETTE[i % crate::side::PALETTE.len()];
    let unit = c.unit.clone().unwrap_or_default();
    let title = if full { c.title.clone() } else { None };
    let n = c.rows.len().max(1) as f64;
    let (lo, hi, step) = c.range(&st);
    let y_of = |v: f64| T + (h - T - B) * (1.0 - (v - lo) / (hi - lo).max(1e-9));
    let slot = (w - L - R) / n;
    let x_of = |i: usize| L + slot * (i as f64 + 0.5);
    let ns = c.series.len().max(1);
    // top of a row's mark: where badges and callouts sit
    let top = |i: usize| -> (f64, f64) {
        let v = &st.values[i];
        let y = match c.kind {
            Kind::Stack => y_of(v.iter().sum()),
            Kind::StackPct => T,
            _ => y_of(v.iter().cloned().fold(f64::MIN, f64::max)),
        };
        (x_of(i), y)
    };

    let body = if c.kind.round() {
        // slices: one circle stroke each, its dash the slice, its offset where it starts
        let total: f64 = st.values.iter().zip(&st.shown).filter(|(_, s)| **s).map(|(v, _)| v[0].max(0.0)).sum::<f64>().max(1e-9);
        let (cx, cy, rad) = (w * 0.36, (h + T - B) / 2.0 + 6.0, (h - T - B) / 2.0);
        let (r, sw) = if c.kind == Kind::Donut { (rad * 0.78, rad * 0.44) } else { (rad / 2.0, rad) };
        let circ = std::f64::consts::TAU * r;
        let mut start = 0.0;
        let slices: Vec<(usize, f64, f64)> = (0..c.rows.len())
            .map(|i| {
                let len = if st.shown[i] { st.values[i][0].max(0.0) / total * circ } else { 0.0 };
                let s = start;
                start += len;
                (i, s, len)
            })
            .collect();
        rsx! {
            g { transform: "rotate(-90 {cx} {cy})",
                for (i, s, len) in slices.iter().cloned() {
                    circle { key: "{c.rows[i].id}", class: "slice{glow(&c.rows[i].id)}", cx: "{cx}", cy: "{cy}", r: "{r}", fill: "none", stroke: color(i), "stroke-width": "{sw}",
                        style: "stroke-dasharray:{len} {circ};stroke-dashoffset:{-s}" }
                }
            }
            if c.kind == Kind::Donut {
                text { class: "chart-total", x: "{cx}", y: "{cy + 8.0}", "text-anchor": "middle", "{num(total)}{unit}" }
            }
            for (k, i) in (0..c.rows.len()).filter(|i| st.shown[*i]).enumerate() {
                g { key: "{c.rows[i].id}", class: "legend{glow(&c.rows[i].id)}", style: "transform:translate({w * 0.68}px,{T + 8.0 + k as f64 * 26.0}px)",
                    rect { width: "14", height: "14", rx: "3", fill: color(i) }
                    text { x: "22", y: "12", "{c.rows[i].label}" }
                    text { class: "val", x: "{w * 0.3}", y: "12", "text-anchor": "end", {format!("{}%", num(st.values[i][0].max(0.0) / total * 100.0))} }
                }
            }
        }
    } else {
        let ticks: Vec<f64> = (0..=((hi - lo) / step).round() as usize).map(|k| lo + step * k as f64).collect();
        let shown: Vec<usize> = (0..c.rows.len()).filter(|i| st.shown[*i]).collect();
        let bw = slot * 0.62 / if c.kind == Kind::Bar { ns as f64 } else { 1.0 };
        rsx! {
            for v in ticks.iter().cloned() {
                line { class: "grid", x1: "{L}", x2: "{w - R}", style: "transform:translateY({y_of(v)}px)" }
                text { class: "tick", x: "{L - 8.0}", "text-anchor": "end", style: "transform:translateY({y_of(v) + 4.0}px)", "{num(v)}" }
            }
            for i in 0..c.rows.len() {
                text { class: if st.shown[i] { "xlab" } else { "xlab off" }, x: "{x_of(i)}", y: "{h - B + 18.0}", "text-anchor": "middle", "{c.rows[i].label}" }
            }
            match c.kind {
                Kind::Line => rsx! {
                    for s in 0..ns {
                        for w in shown.windows(2) {
                            path { key: "{s}-{c.rows[w[0]].id}-{c.rows[w[1]].id}", class: "seg", stroke: color(s), "pathLength": "1",
                                style: "d:path('M{x_of(w[0])} {y_of(st.values[w[0]][s])} L{x_of(w[1])} {y_of(st.values[w[1]][s])}')" }
                        }
                        for i in shown.iter().cloned() {
                            g { key: "{s}-{c.rows[i].id}", class: "pt{glow(&c.rows[i].id)}", style: "transform:translate({x_of(i)}px,{y_of(st.values[i][s])}px)",
                                circle { r: "6", fill: color(s) }
                                text { class: "val", y: "-12", "text-anchor": "middle", "{num(st.values[i][s])}" }
                            }
                        }
                    }
                },
                Kind::Bar => rsx! {
                    for i in shown.iter().cloned() {
                        for s in 0..ns {
                            g { key: "{c.rows[i].id}-{s}", class: "bar{glow(&c.rows[i].id)}",
                                rect { x: "{x_of(i) - bw * ns as f64 / 2.0 + bw * s as f64}", width: "{bw * 0.92}", fill: color(s),
                                    style: "y:{y_of(st.values[i][s].max(lo).max(0.0))}px;height:{(y_of(lo.max(0.0).min(st.values[i][s])) - y_of(st.values[i][s].max(0.0))).abs()}px" }
                                text { class: "val", x: "{x_of(i) - bw * ns as f64 / 2.0 + bw * (s as f64 + 0.46)}", "text-anchor": "middle", style: "transform:translateY({y_of(st.values[i][s]) - 8.0}px)", "{num(st.values[i][s])}" }
                            }
                        }
                    }
                },
                _ => {
                    // stacks: each series on top of the previous ones; `stack%` scales each bar to 100
                    let segs: Vec<(usize, usize, f64, f64)> = shown
                        .iter()
                        .flat_map(|&i| {
                            let v = &st.values[i];
                            let sum: f64 = v.iter().sum::<f64>().max(1e-9);
                            let k = if c.kind == Kind::StackPct { 100.0 / sum } else { 1.0 };
                            let mut acc = 0.0;
                            v.iter().enumerate().map(move |(s, x)| {
                                acc += x * k;
                                (i, s, acc - x * k, acc)
                            }).collect::<Vec<_>>()
                        })
                        .collect();
                    rsx! {
                        for (i, s, a, b) in segs.into_iter() {
                            g { key: "{c.rows[i].id}-{s}", class: "bar{glow(&c.rows[i].id)}",
                                rect { x: "{x_of(i) - bw / 2.0}", width: "{bw}", fill: color(s), style: "y:{y_of(b)}px;height:{(y_of(a) - y_of(b)).max(0.0)}px" }
                            }
                        }
                    }
                }
            }
            for (k, (v, l)) in st.rules.iter().enumerate() {
                g { key: "rule{k}", class: "rule", style: "transform:translateY({y_of(*v)}px)",
                    line { x1: "{L}", x2: "{w - R}" }
                    text { x: "{w - R}", y: "-6", "text-anchor": "end", {l.clone().unwrap_or_else(|| num(*v))} }
                }
            }
            if c.series.len() > 1 {
                for (s, name) in c.series.iter().enumerate() {
                    g { class: "legend", style: "transform:translate({w - R - 110.0 * (c.series.len() - s) as f64}px,{T - 28.0}px)",
                        rect { width: "12", height: "12", rx: "3", fill: color(s) }
                        text { x: "18", y: "11", "{name}" }
                    }
                }
            }
            if let Some(y) = &c.y {
                text { class: "axis", x: "{L - 8.0}", y: "{T - 16.0}", "{y}" if !unit.is_empty() { " ({unit})" } }
            }
            if let Some(x) = &c.x {
                text { class: "axis", x: "{w - R}", y: "{h - 6.0}", "text-anchor": "end", "{x}" }
            }
        }
    };
    let deltas: Vec<(String, (f64, f64), (f64, f64), String)> = if c.kind.round() {
        vec![]
    } else {
        st.deltas.iter().filter_map(|(a, b)| {
            let (ia, ib) = (c.row(a)?, c.row(b)?);
            Some((format!("{a}>{b}"), top(ia), top(ib), delta_text(st.values[ia][0], st.values[ib][0])))
        }).collect()
    };
    let notes: Vec<(String, (f64, f64), String)> = st.notes.iter().filter_map(|(id, t)| {
        let i = c.row(id)?;
        let p = if c.kind.round() { (w * 0.68 + 40.0, T + 8.0 + (0..i).filter(|k| st.shown[*k]).count() as f64 * 26.0 + 30.0) } else { top(i) };
        st.shown[i].then(|| (id.clone(), p, t.clone()))
    }).collect();
    rsx! {
        div { class: if full { "chart full" } else { "chart" },
            if let Some(t) = title {
                div { class: "chart-title", "{t}" }
            }
            svg { view_box: "0 0 {w} {h}", preserve_aspect_ratio: "xMidYMid meet",
                {body}
                for (k, a, b, t) in deltas.into_iter() {
                    g { key: "d{k}", class: "delta",
                        path { d: "M{a.0} {a.1 - 22.0} L{a.0} {a.1.min(b.1) - 30.0} L{b.0} {a.1.min(b.1) - 30.0} L{b.0} {b.1 - 22.0}", "pathLength": "1" }
                        g { style: "transform:translate({(a.0 + b.0) / 2.0}px,{a.1.min(b.1) - 30.0}px)",
                            rect { x: "-30", y: "-12", width: "60", height: "24", rx: "12" }
                            text { y: "5", "text-anchor": "middle", "{t}" }
                        }
                    }
                }
                for (id, p, t) in notes.into_iter() {
                    g { key: "n{id}", class: "callout", style: "transform:translate({p.0}px,{p.1 - 44.0}px)",
                        text { "text-anchor": "middle", "{t}" }
                    }
                }
            }
        }
    }
}

pub const CSS: &str = r#"
.chart { display:flex; flex-direction:column; width:100%; height:100%; min-height:0; }
.chart.full { padding:24px 32px; box-sizing:border-box; background:var(--bg); flex:1; }
.chart-title { font-family:var(--font-title, inherit); font-size:22px; font-weight:600; color:var(--text); margin-bottom:8px; }
.chart svg { flex:1; width:100%; min-height:0; overflow:visible; }
.chart text { fill:var(--text); font-size:14px; }
.chart .tick, .chart .axis, .chart .xlab { fill:var(--muted); font-size:13px; }
.chart .xlab { transition:opacity .4s; }
.chart .xlab.off { opacity:.25; }
.chart .val { font-size:13px; font-weight:600; }
.chart-total { font-size:26px !important; font-weight:700; }
.chart .grid { stroke:var(--grid); stroke-width:1; transition:transform .7s ease; }
.chart .tick { transition:transform .7s ease; }
.chart .seg { fill:none; stroke-width:3; stroke-linecap:round; stroke-dasharray:1; animation:chart-draw .7s ease-out both; transition:d .7s ease; }
.chart .pt { transition:transform .7s ease; }
.chart .pt circle, .chart .bar rect, .chart .slice { transition:y .7s ease, height .7s ease, stroke-dasharray .8s ease, stroke-dashoffset .8s ease, filter .3s; }
.chart .pt > * { animation:chart-pop .45s ease-out .35s both; transform-box:fill-box; transform-origin:center; }
.chart .bar rect { animation:chart-grow .7s ease-out both; transform-box:fill-box; transform-origin:bottom; }
.chart .bar text, .chart .legend { transition:transform .7s ease; animation:chart-fade .5s ease-out .3s both; }
.chart .glow circle, .chart .glow rect, .chart .slice.glow { filter:drop-shadow(0 0 6px var(--hi)) drop-shadow(0 0 2px var(--hi)); }
.chart .glow text { fill:var(--hi); }
.chart .rule { transition:transform .7s ease; animation:chart-fade .5s both; }
.chart .rule line { stroke:var(--hi); stroke-width:1.5; stroke-dasharray:6 5; }
.chart .rule text { fill:var(--hi); font-size:12px; }
.chart .delta path { fill:none; stroke:var(--muted); stroke-width:1.5; stroke-dasharray:1; animation:chart-draw .6s ease-out both; }
.chart .delta rect { fill:var(--accent); }
.chart .delta text { fill:var(--bg); font-weight:700; font-size:13px; }
.chart .delta g, .chart .callout { animation:chart-fade .4s ease-out .4s both; }
.chart .callout text { fill:var(--accent); font-weight:600; font-size:14px; }
@keyframes chart-draw { from { stroke-dashoffset:1; } to { stroke-dashoffset:0; } }
@keyframes chart-pop { from { transform:scale(0); } }
@keyframes chart-grow { from { transform:scaleY(0); } }
@keyframes chart-fade { from { opacity:0; } }
"#;
