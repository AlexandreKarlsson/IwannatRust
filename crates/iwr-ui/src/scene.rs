//! Scenes of a codecast (`@ scene`, `| scene`): an SVG drawn from [`iwr_core::scene::State`], the
//! scene as the markers fired so far left it. Objects are keyed by id: a new one fades in, a
//! moved one glides to its cell through a CSS transition, and the lines follow their ends.

use crate::State;
use dioxus::prelude::*;
use iwr_core::scene::{Scene, Shape, Stroke};

/// Colour of a shape: the ones of a diagram's nodes, so both read as one system.
fn color(shape: Shape, state: Option<&str>) -> &'static str {
    match (state, shape) {
        (Some("ok"), _) => "#34d399",
        (Some("warn"), _) => "#eab308",
        (Some("err"), _) => "#f87171",
        (_, Shape::Round) => "#34d399",
        (_, Shape::Pill) => "#c084fc",
        (_, Shape::Diamond) => "#eab308",
        (_, Shape::Store) => "#2dd4bf",
        (_, Shape::Zone) => "#94a3b8",
        _ => "#60a5fa",
    }
}

#[component]
pub fn SceneView(scene: Scene, #[props(default)] full: bool) -> Element {
    let state = use_context::<State>();
    let (part, cue, frac) = (*state.part_idx.read(), *state.cue_idx.read(), *state.cue_frac.read());
    let script = state.script.read().clone();
    let marks = script.as_ref().and_then(|s| s.parts.get(part)).map(|p| iwr_core::script::marks_at(p, cue, frac)).unwrap_or_default();
    let st = scene.state(marks);
    // a `!` marker of this cue that has fired, else the cue's `!` refs (a marker's glow ends with its cue)
    let fired = script.as_ref().and_then(|s| s.parts.get(part)).and_then(|p| p.cues.get(cue)).is_some_and(|c| c.marks.iter().any(|m| m.frac() <= frac && m.marks.iter().any(|m| matches!(m, iwr_core::chart::Mark::Glow(_)))));
    let lit: Vec<String> = st.glow.clone().filter(|_| fired).unwrap_or_else(|| state.hl_ids.read().clone());
    let on = |id: &str| lit.iter().any(|g| g == id);
    let sc = &scene;
    let (w, h) = sc.size();
    let (cw, ch) = sc.cell();
    let title = if full { sc.title.clone() } else { None };

    // zones behind, then the boxes, then icons and texts (what moves onto a box stays visible)
    let layer = |s: Shape| match s {
        Shape::Zone => 0,
        Shape::Icon | Shape::Text => 2,
        _ => 1,
    };
    let order: Vec<usize> = (0..3).flat_map(|k| (0..sc.objs.len()).filter(move |i| layer(sc.objs[*i].shape) == k)).filter(|i| st.shown[*i]).collect();
    let objs = order.into_iter().map(|i| {
        let o = &sc.objs[i];
        let (x, y, ow, oh) = sc.rect(st.at[i].0, st.at[i].1, o.w, o.h, o.shape);
        let s = st.state[i].as_deref();
        let col = color(o.shape, s);
        // the colour mixed into the background: a line behind a box does not show through
        let fill = if o.shape == Shape::Zone { col.to_string() } else { format!("color-mix(in srgb, {col} calc(var(--fill-op) * 100%), var(--bg))") };
        let cls = format!("obj {}{}{}", o.shape.name(), s.map(|s| format!(" s-{s}")).unwrap_or_default(), if on(&o.id) { " glow" } else { "" });
        let rows: Vec<String> = st.label[i].split("<br>").map(|l| l.trim().to_string()).collect();
        let n = rows.len() as f64;
        let big = (ow.min(oh) * 0.62).round();
        let ry = 7.0;
        rsx! {
            g { key: "{o.id}", class: "{cls}", style: "transform:translate({x}px,{y}px)",
                g { class: "in",
                    match o.shape {
                        Shape::Box => rsx! { rect { width: "{ow}", height: "{oh}", rx: "6", style: "fill:{fill}", stroke: col } },
                        Shape::Round => rsx! { rect { width: "{ow}", height: "{oh}", rx: "16", style: "fill:{fill}", stroke: col } },
                        Shape::Pill => rsx! { rect { width: "{ow}", height: "{oh}", rx: "{oh / 2.0}", style: "fill:{fill}", stroke: col } },
                        Shape::Zone => rsx! { rect { width: "{ow}", height: "{oh}", rx: "10", style: "fill:{fill}", stroke: col } },
                        Shape::Diamond => rsx! { polygon { points: "{ow / 2.0},0 {ow},{oh / 2.0} {ow / 2.0},{oh} 0,{oh / 2.0}", style: "fill:{fill}", stroke: col } },
                        Shape::Store => rsx! {
                            path { d: "M0 {ry} A{ow / 2.0} {ry} 0 0 1 {ow} {ry} V{oh - ry} A{ow / 2.0} {ry} 0 0 1 0 {oh - ry} Z", style: "fill:{fill}", stroke: col }
                            path { class: "lid", d: "M0 {ry} A{ow / 2.0} {ry} 0 0 0 {ow} {ry}", stroke: col }
                        },
                        Shape::Icon | Shape::Text => rsx! {},
                    }
                    if o.shape == Shape::Zone {
                        text { x: "12", y: "4", "{st.label[i]}" }
                    } else if o.shape == Shape::Icon {
                        text { x: "{ow / 2.0}", y: "{oh / 2.0 + big * 0.36}", "text-anchor": "middle", style: "font-size:{big}px", "{st.label[i]}" }
                    } else {
                        for (k, l) in rows.into_iter().enumerate() {
                            text { x: "{ow / 2.0}", y: "{oh / 2.0 + 5.0 + (k as f64 - (n - 1.0) / 2.0) * 18.0}", "text-anchor": "middle", "{l}" }
                        }
                    }
                }
            }
        }
    });

    let lines = sc.lines.iter().enumerate().filter(|(i, _)| st.line_shown[*i]).filter_map(|(i, l)| {
        let ((x1, y1), (x2, y2)) = sc.segment(&st, l)?;
        // a line between two glowing objects glows too
        let ends = |e: &iwr_core::scene::End| matches!(e, iwr_core::scene::End::Obj(id) if on(id));
        let glow = if on(&l.id) || (ends(&l.from) && ends(&l.to)) { " glow" } else { "" };
        let kind = match l.stroke {
            Stroke::Arrow => "",
            Stroke::Plain => " plain",
            Stroke::Dotted => " dotted",
            Stroke::Thick => " thick",
        };
        let deg = (y2 - y1).atan2(x2 - x1).to_degrees();
        let label = st.line_label[i].clone();
        Some(rsx! {
            g { key: "{l.id}", class: "ln{kind}{glow}",
                path { style: "d:path('M{x1} {y1} L{x2} {y2}')" }
                if l.stroke != Stroke::Plain {
                    path { class: "head", d: "M0 0 L-10 -5 L-10 5 Z", style: "transform:translate({x2}px,{y2}px) rotate({deg}deg)" }
                }
                if let Some(t) = label {
                    text { "text-anchor": "middle", style: "transform:translate({(x1 + x2) / 2.0}px,{(y1 + y2) / 2.0 - 6.0}px)", "{t}" }
                }
            }
        })
    });

    let notes = st.notes.iter().filter_map(|(id, t)| {
        let i = sc.obj(id).filter(|i| st.shown[*i])?;
        let o = &sc.objs[i];
        let (x, y, ow, oh) = sc.rect(st.at[i].0, st.at[i].1, o.w, o.h, o.shape);
        // above the object, below it on the first row
        let ty = if y > 20.0 { y - 8.0 } else { y + oh + 18.0 };
        Some(rsx! {
            g { key: "n{id}", class: "callout", style: "transform:translate({x + ow / 2.0}px,{ty}px)",
                text { "text-anchor": "middle", "{t}" }
            }
        })
    });

    rsx! {
        div { class: if full { "scene full" } else { "scene" },
            if let Some(t) = title {
                div { class: "chart-title", "{t}" }
            }
            div { class: "scene-fit",
                svg { view_box: "0 0 {w} {h}", preserve_aspect_ratio: "xMidYMid meet", style: "max-width:{w * 1.8}px; max-height:{h * 1.8}px",
                    if sc.ruled {
                        for c in 0..=sc.cols {
                            line { class: "cell", x1: "{c as f64 * cw}", x2: "{c as f64 * cw}", y1: "0", y2: "{h}" }
                        }
                        for r in 0..=sc.rows {
                            line { class: "cell", x1: "0", x2: "{w}", y1: "{r as f64 * ch}", y2: "{r as f64 * ch}" }
                        }
                    }
                    {lines}
                    {objs}
                    {notes}
                }
            }
        }
    }
}

pub const CSS: &str = r#"
.scene { display:flex; flex-direction:column; width:100%; height:100%; min-height:0; }
.scene.full { padding:24px 32px; box-sizing:border-box; background:var(--bg); flex:1; min-width:0; }
.scene-fit { flex:1; min-height:0; display:flex; align-items:center; justify-content:center; }
.scene svg { width:100%; height:100%; overflow:visible; }
.scene text { fill:var(--text); font-size:14px; }
.scene .cell { stroke:var(--grid); stroke-width:1; }
.scene .obj { transition:transform .7s ease, filter .4s, opacity .4s; }
.scene .obj .in { animation:scene-pop .45s ease-out both; transform-box:fill-box; transform-origin:center; }
.scene .obj rect, .scene .obj polygon, .scene .obj path { stroke-width:1.6; transition:fill .4s, stroke .4s; }
.scene .obj.zone rect { fill-opacity:.07; }
.scene .obj .lid { fill:none; }
.scene .obj.zone rect { stroke-dasharray:6 5; stroke-width:1.2; }
.scene .obj.zone text { fill:var(--muted); font-size:11px; font-weight:600; paint-order:stroke; stroke:var(--bg); stroke-width:6px; stroke-linejoin:round; }
.scene .obj.text text { font-size:15px; }
.scene .obj.s-off { filter:grayscale(1); opacity:.4; }
.scene .obj.glow rect, .scene .obj.glow polygon, .scene .obj.glow path, .scene .obj.icon.glow text { filter:drop-shadow(0 0 6px var(--hi)) drop-shadow(0 0 2px var(--hi)); }
.scene .obj.glow rect, .scene .obj.glow polygon, .scene .obj.glow path { stroke:var(--hi); stroke-width:2.4; }
.scene .obj.text.glow text { fill:var(--hi); }
.scene .ln { animation:scene-fade .5s ease-out both; }
.scene .ln path { fill:none; stroke:var(--muted); stroke-width:1.8; stroke-linecap:round; transition:d .7s ease, transform .7s ease, stroke .3s; }
.scene .ln .head { fill:var(--muted); stroke-width:1; stroke-linejoin:round; }
.scene .ln.dotted path:first-child { stroke-dasharray:2 6; }
.scene .ln.thick path:first-child { stroke-width:3.4; }
.scene .ln text { fill:var(--muted); font-size:12px; paint-order:stroke; stroke:var(--bg); stroke-width:5px; stroke-linejoin:round; transition:transform .7s ease; }
.scene .ln.glow path { stroke:var(--hi); }
.scene .ln.glow .head, .scene .ln.glow text { fill:var(--hi); }
.scene .callout { transition:transform .7s ease; animation:scene-fade .4s ease-out .2s both; }
.scene .callout text { fill:var(--accent); font-weight:600; font-size:13px; paint-order:stroke; stroke:var(--bg); stroke-width:5px; stroke-linejoin:round; }
@keyframes scene-pop { from { transform:scale(.4); opacity:0; } }
@keyframes scene-fade { from { opacity:0; } }
@media (prefers-reduced-motion: reduce) { .scene .obj, .scene .ln path, .scene .ln text, .scene .callout { transition:none; } .scene .obj .in, .scene .ln, .scene .callout { animation:none; } }
"#;
