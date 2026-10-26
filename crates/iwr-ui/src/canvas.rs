//! SVG canvas: pan/zoom, nodes, edges, tooltip, legend.

use crate::theme;
use crate::{State, Transform};
use iwr_core::script::Resolved;
use dioxus::html::geometry::WheelDelta;
use dioxus::prelude::*;
use iwr_core::layout::{self, Seg};
use iwr_core::views::{EdgeKind, Graph, Mode, NodeKind, VEdge, VNode};
use std::rc::Rc;

/// Arrow head polygon at the end of the drawn edge, sized to the line's width.
fn arrow(segs: &[Seg], width: f64) -> Option<String> {
    let ((x2, y2), (ux, uy)) = layout::edge_end(segs)?;
    let s = 7.0 + width * 1.2;
    let p1 = (x2 - ux * s - uy * s * 0.55, y2 - uy * s + ux * s * 0.55);
    let p2 = (x2 - ux * s + uy * s * 0.55, y2 - uy * s - ux * s * 0.55);
    Some(format!("{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}", x2, y2, p1.0, p1.1, p2.0, p2.1))
}

/// Class of an edge's `g`, shared by the edge and its label (drawn later, above the boxes).
fn edge_class(edge: &VEdge, highlighted: bool, near: bool, dim: bool) -> String {
    format!("edge{}{}{}{}", if highlighted { " hi" } else { "" }, if near { " near" } else { "" }, if dim { " dim" } else { "" }, if edge.hub { " hub" } else { "" })
}

#[component]
pub fn Canvas() -> Element {
    let mut state = use_context::<State>();
    let graph: Memo<Rc<Graph>> = use_memo(move || Rc::new(state.build_graph()));
    // label placement depends on the graph only, not on pan/zoom or hover
    let label_spots: Memo<Rc<Vec<Option<(f64, f64)>>>> = use_memo(move || Rc::new(graph.read().label_spots()));
    let mut dragging: Signal<Option<(f64, f64, f64, f64)>> = use_signal(|| None);
    let mut moved = use_signal(|| false);
    let mut svg_el: Signal<Option<Rc<MountedData>>> = use_signal(|| None);

    // fit when requested (mode/root change, load)
    let fit_req = *state.fit_request.read();
    use_effect(move || {
        let req = *state.fit_request.read();
        if req != *state.fit_done.peek() {
            state.fit_done.set(req);
            let g = graph.read().clone();
            state.fit(&g);
        }
    });
    let _ = fit_req;

    let refresh_rect = move || {
        if let Some(el) = svg_el.read().clone() {
            spawn(async move {
                if let Ok(r) = el.get_client_rect().await {
                    state.canvas_rect.set((r.origin.x, r.origin.y, r.size.width, r.size.height));
                }
            });
        }
    };

    // a cue enlarged or shrank a panel: once the resize animation is over, measure the canvas
    // again and aim the camera at the highlight with the new size
    use_effect(move || {
        let _ = *state.focus.read();
        if state.script.peek().is_none() {
            return;
        }
        spawn(async move {
            gloo_timers::future::sleep(std::time::Duration::from_millis(480)).await;
            // layout size, not the drawn one: a `+ zoom` may be scaling the screen
            if let Ok(v) = document::eval("const s = document.querySelector('.canvas-wrap svg'); return s ? [s.clientWidth, s.clientHeight] : [];").await {
                let n: Vec<f64> = v.as_array().map(|a| a.iter().filter_map(|x| x.as_f64()).collect()).unwrap_or_default();
                if let [w, h] = n[..] {
                    let (x, y, _, _) = *state.canvas_rect.peek();
                    state.canvas_rect.set((x, y, w, h));
                }
            }
            state.camera();
        });
    });

    let t = *state.view.read();
    let g = graph.read().clone();
    let mode = *state.mode.read();
    let selected = *state.selected.read();
    let sel_span = *state.selected_span.read();
    let hl_refs = state.hl.read().clone();
    let hl_ids = state.hl_ids.read().clone();
    let hi: Vec<String> = if mode == Mode::Diagram {
        g.nodes.iter().filter(|n| hl_ids.contains(&n.id)).map(|n| n.id.clone()).collect()
    } else if state.script.read().is_some() {
        g.nodes.iter().filter(|n| hl_hit(n, &hl_refs)).map(|n| n.id.clone()).collect()
    } else {
        vec![]
    };
    let hovered = state.hovered.read().clone();
    let hover_span = *state.hover_span.read();
    let is_drag = dragging.read().is_some();
    let transform = format!("translate({:.1},{:.1}) scale({:.3})", t.tx, t.ty, t.k);
    let animate = *state.animate.read();
    let legend = theme::legend(mode);
    let status = state.status.read().clone();
    let depth = *state.depth.read();
    let node_count = g.nodes.len();
    let root_label = state.root.read().and_then(|r| state.project.read().as_ref().map(|p| p.item(r).name.clone()));
    let scope_label = state.scope.read().and_then(|m| state.project.read().as_ref().map(|p| p.module(m).path.clone()));
    let hovered_id: Option<String> = hovered.as_ref().map(|h| h.0.clone());
    let selected_node_id: Option<String> = g.nodes.iter().find(|n| is_selected(n, selected, sel_span, mode)).map(|n| n.id.clone());
    let focus_id = hovered_id.clone().or(selected_node_id);
    let spots = label_spots.read().clone();
    let edge_classes: Vec<String> = g
        .edges
        .iter()
        .map(|e| {
            let highlighted = hi.contains(&e.from) && hi.contains(&e.to);
            let near = focus_id.as_ref().is_some_and(|f| &e.from == f || &e.to == f);
            // architecture: dependency arrows stay faint unless they touch the box in focus or a glowing one
            let dim = mode == Mode::Architecture && e.kind == EdgeKind::Dependency && !near && !hi.contains(&e.from) && !hi.contains(&e.to);
            edge_class(e, highlighted, near, dim)
        })
        .collect();

    rsx! {
        div { class: "canvas-wrap",
            svg {
                class: if is_drag { "dragging" } else { "" },
                onmounted: move |e| {
                    svg_el.set(Some(e.data()));
                    refresh_rect();
                },
                onmousedown: move |e| {
                    let c = e.data().client_coordinates();
                    let v = *state.view.read();
                    state.animate.set(false);
                    dragging.set(Some((c.x, c.y, v.tx, v.ty)));
                    moved.set(false);
                    refresh_rect();
                },
                onmousemove: move |e| {
                    if let Some((sx, sy, tx0, ty0)) = *dragging.read() {
                        let c = e.data().client_coordinates();
                        if (c.x - sx).abs() + (c.y - sy).abs() > 2.0 {
                            moved.set(true);
                        }
                        let k = state.view.read().k;
                        state.view.set(Transform { tx: tx0 + (c.x - sx), ty: ty0 + (c.y - sy), k });
                    }
                },
                onmouseup: move |_| dragging.set(None),
                onmouseleave: move |_| { dragging.set(None); state.hovered.set(None); },
                onwheel: move |e| {
                    e.prevent_default();
                    state.animate.set(false);
                    let dy = match e.data().delta() {
                        WheelDelta::Pixels(v) => v.y,
                        WheelDelta::Lines(v) => v.y * 30.0,
                        WheelDelta::Pages(v) => v.y * 300.0,
                    };
                    let c = e.data().client_coordinates();
                    let (rx, ry, _, _) = *state.canvas_rect.read();
                    let (px, py) = (c.x - rx, c.y - ry);
                    let v = *state.view.read();
                    let factor = (-dy / 400.0).exp().clamp(0.7, 1.4);
                    let k = (v.k * factor).clamp(0.1, 4.0);
                    let f = k / v.k;
                    state.view.set(Transform { tx: px - (px - v.tx) * f, ty: py - (py - v.ty) * f, k });
                },
                defs {
                    filter { id: "glow", x: "-20%", y: "-20%", width: "140%", height: "140%",
                        feGaussianBlur { "stdDeviation": "3", result: "blur" }
                        feMerge { feMergeNode { "in": "blur" } feMergeNode { "in": "SourceGraphic" } }
                    }
                }
                g { class: if animate { "cam anim" } else { "cam" }, transform: "{transform}",
                    // containers first
                    for n in g.nodes.iter().filter(|n| n.container) {
                        Node { key: "{n.id}", node: n.clone(), selected: false, highlighted: hi.contains(&n.id), hovered: false, mode, moved }
                    }
                    for (e, class) in g.edges.iter().zip(&edge_classes) {
                        Edge { key: "{e.id}", edge: e.clone(), segs: g.segments(e), class: class.clone() }
                    }
                    for n in g.nodes.iter().filter(|n| !n.container) {
                        Node {
                            key: "{n.id}",
                            node: n.clone(),
                            selected: is_selected(n, selected, sel_span, mode),
                            highlighted: hi.contains(&n.id) || line_hit(n, hover_span),
                            hovered: hovered_id.as_deref() == Some(n.id.as_str()),
                            mode,
                            moved,
                        }
                    }
                    // labels last: above the boxes, where they were placed not to cover any
                    for ((e, class), spot) in g.edges.iter().zip(&edge_classes).zip(spots.iter()) {
                        if let (Some(l), Some((x, y))) = (&e.label, spot) {
                            EdgeLabel { key: "l{e.id}", label: l.clone(), color: theme::edge_color(e.kind, &e.from), x: *x, y: *y, class: class.clone() }
                        }
                    }
                }
            }
            div { class: "hud",
                button { class: "small", onclick: move |_| { let g = graph.read().clone(); state.fit(&g); }, "Fit" }
                button { class: "small", onclick: move |_| { let mut v = *state.view.read(); v.k = (v.k * 1.25).min(4.0); state.view.set(v); }, "+" }
                button { class: "small", onclick: move |_| { let mut v = *state.view.read(); v.k = (v.k / 1.25).max(0.1); state.view.set(v); }, "−" }
                if let Some(r) = root_label {
                    span { class: "status", "root: " b { "{r}" } }
                }
                if mode == Mode::CallTree {
                    span { class: "depth", title: "how many call levels to open from the root",
                        "depth"
                        button { class: "small", onclick: move |_| { let d = *state.depth.read(); state.depth.set(d.saturating_sub(1).max(1)); state.save_settings(); }, "−" }
                        span { "{depth}" }
                        button { class: "small", onclick: move |_| { let d = *state.depth.read(); state.depth.set((d + 1).min(12)); state.save_settings(); }, "+" }
                    }
                }
                if let Some(sc) = scope_label {
                    span { class: "chip", title: "clear module scope", onclick: move |_| state.set_scope(None), "scope: {sc} ✕" }
                }

                span { class: "status", "{node_count} nodes · {status}" }
                span { class: "status", " · " span { class: "kbd", "f" } " fit  " span { class: "kbd", "g" } " codecast  " span { class: "kbd", "t" } " tour  " span { class: "kbd", "←" } span { class: "kbd", "→" } " steps" }
            }
            if mode == Mode::Structure {
                div { class: "hud hud2", title: "tick what the Structure view shows",
                    span { class: "status", "show" }
                    crate::settings::KindChips { compact: true }
                }
            }
            if let Some(note) = &g.note {
                div { class: "canvas-note", "{note}" }
            }
            if !legend.is_empty() {
                div { class: "legend",
                    for l in legend.iter() {
                        {let l_style = if l.dash.is_empty() { "solid" } else { "dashed" };
                        rsx! { div {
                            if l.is_edge {
                                span { class: "ln", style: "border-color: {l.color}; border-top-style: {l_style}" }
                            } else {
                                span { class: "sw", style: "background: {l.color}" }
                            }
                            "{l.label}"
                        } }}
                    }
                }
            }
            if let Some((id, x, y)) = hovered {
                Tooltip { node_id: id, x, y, graph: graph.read().clone() }
            }
        }
    }
}

/// Is this node covered by one of the cue's highlight refs?
pub fn hl_hit(n: &VNode, refs: &[Resolved]) -> bool {
    if n.container {
        return false;
    }
    refs.iter().any(|r| {
        if let Some(m) = r.module {
            return n.id == format!("m{}", m);
        }
        if r.inner {
            // a folded loop stands for everything inside it
            let folded = n.kind == NodeKind::Loop && n.expandable && !n.expanded;
            match n.span {
                Some(s) if folded && s.contains_lines(&r.span) => true,
                Some(s) => n.cfg_node.is_some() && r.span.contains_lines(&s) && n.kind != NodeKind::Entry && n.kind != NodeKind::Exit,
                None => false,
            }
        } else {
            n.item == r.item && r.item.is_some() && n.cfg_node.is_none()
        }
    })
}

/// Does a hovered source line fall inside this (statement-level) node?
fn line_hit(n: &VNode, hs: Option<iwr_core::model::Span>) -> bool {
    match (n.span, hs) {
        (Some(s), Some(h)) => n.cfg_node.is_some() && s.file == h.file && s.contains_line(h.line_start) && n.kind != NodeKind::Entry && n.kind != NodeKind::Exit,
        _ => false,
    }
}

fn is_selected(n: &VNode, selected: Option<usize>, sel_span: Option<iwr_core::model::Span>, mode: Mode) -> bool {
    match mode {
        Mode::ControlFlow | Mode::BranchTree => match (n.span, sel_span) {
            (Some(a), Some(b)) => n.cfg_node.is_some() && a == b,
            _ => false,
        },
        _ => n.item.is_some() && n.item == selected && n.cfg_node.is_none(),
    }
}

#[component]
fn Node(node: VNode, selected: bool, highlighted: bool, hovered: bool, mode: Mode, moved: Signal<bool>) -> Element {
    let mut state = use_context::<State>();
    let color = theme::node_color(node.kind);
    let n = node.clone();
    let id = node.id.clone();
    let id2 = node.id.clone();
    let id3 = node.id.clone();
    // a folded module box (a crate in a big workspace): double-click opens it as the module scope
    let folded_module = (node.kind == NodeKind::Module && node.badges.iter().any(|b| b.ends_with(" inside"))).then(|| node.id.strip_prefix('m').and_then(|x| x.parse::<usize>().ok())).flatten();
    let expanded = node.expanded;
    let lines: Vec<String> = node.label.lines().map(|s| s.to_string()).collect();
    let multi = lines.len() > 1;
    let container = node.container;
    // details (signature, fields, badges) only when the node is the focus
    let active = !container && (selected || highlighted || hovered);
    let detail_lines: Vec<String> = if active {
        let mut v: Vec<String> = node.sublabel.lines().filter(|l| !l.is_empty()).map(|s| s.to_string()).collect();
        if !node.badges.is_empty() {
            v.push(node.badges.join(" · "));
        }
        v
    } else {
        vec![]
    };
    let detail_w = detail_lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f64 * 7.8 + 16.0;
    let detail_w = detail_w.max(node.w);
    let detail_h = 6.0 + 16.0 * detail_lines.len() as f64;
    let fill_op = if container { 0.10 } else if node.kind == NodeKind::Block { 0.18 } else { 0.28 };
    let rx = match node.kind {
        NodeKind::Entry | NodeKind::Exit | NodeKind::Pill => node.h / 2.0,
        NodeKind::If | NodeKind::Match => 4.0,
        NodeKind::Loop | NodeKind::Round => 14.0,
        _ => 8.0,
    };
    let class = format!("node {}{}", if selected { "sel " } else { "" }, if highlighted { "hi" } else { "" });
    let exp_y = if container { 12.0 } else { node.h / 2.0 };
    let title_y = if container { 17.0 } else if multi { 18.0 } else { node.h / 2.0 + 4.5 };
    // diagram nodes: text centred, diamonds drawn as diamonds
    // control-flow entry: the function name as the graph's title
    let fn_title = node.kind == NodeKind::Entry && node.cfg_node.is_some();
    let title_y = if fn_title { node.h / 2.0 + 4.5 } else { title_y };
    // a caption (module file under its doc label) takes the bottom line of the box
    let has_caption = !node.caption.is_empty() && !container;
    let title_y = if has_caption && !multi { (node.h - iwr_core::views::CAPTION_H) / 2.0 + 4.5 } else { title_y };
    let folded = node.kind == NodeKind::Loop && node.expandable && !node.expanded;
    let centered = (mode == Mode::Diagram && !container) || fn_title;
    let tx = if centered { node.w / 2.0 } else { 12.0 };
    let anchor = if centered { "middle" } else { "start" };
    let diamond = node.kind == NodeKind::Diamond;
    let poly = format!("{:.1},0 {:.1},{:.1} {:.1},{:.1} 0,{:.1}", node.w / 2.0, node.w, node.h / 2.0, node.w / 2.0, node.h, node.h / 2.0);
    rsx! {
        g {
            class: "{class}",
            transform: "translate({node.x:.1},{node.y:.1})",
            onclick: move |e| {
                e.stop_propagation();
                if *moved.read() { return; }
                if let Some(item) = n.item {
                    let mut sel_item = item;
                    // for cfg nodes select the enclosing function (root, or the unfolded callee) but keep the statement span
                    if n.cfg_node.is_some() {
                        if let Some(r) = iwr_core::views::cfg_owner(&n.id).or(*state.root.read()) { sel_item = r; }
                    }
                    let mode_now = *state.mode.read();
                    if n.cfg_node.is_some() {
                        state.selected.set(Some(sel_item));
                        state.selected_span.set(n.span);
                        if mode_now == Mode::BranchTree || mode_now == Mode::ControlFlow {
                            // clicking a call node jumps into the callee in the call tree on double click only
                        }
                    } else if mode_now == Mode::Structure || mode_now == Mode::Architecture || mode_now == Mode::Types || mode_now == Mode::ErrorFlow {
                        state.selected.set(Some(item));
                        state.selected_span.set(n.span);
                    } else {
                        state.selected.set(Some(item));
                        state.selected_span.set(n.span);
                    }
                } else {
                    state.selected_span.set(n.span);
                }
            },
            ondoubleclick: move |e| {
                e.stop_propagation();
                // double click: dive into the function (cfg) or toggle expansion
                let mode_now = *state.mode.read();
                let item = node.item;
                if let Some(m) = folded_module {
                    state.set_scope(Some(m));
                } else if node.cfg_node.is_some() {
                    if let Some(item) = item {
                        if state.project.read().as_ref().map(|p| p.item(item).is_callable()).unwrap_or(false) && node.item != *state.root.read() {
                            state.set_root(Some(item));
                            state.selected.set(Some(item));
                        }
                    }
                } else if node.expandable {
                    state.toggle_expand(&id3, expanded);
                } else if let Some(item) = item {
                    if state.project.read().as_ref().map(|p| p.item(item).is_callable()).unwrap_or(false) {
                        if mode_now != Mode::ControlFlow { state.set_mode(Mode::ControlFlow); }
                        state.set_root(Some(item));
                    }
                }
            },
            onmouseenter: move |e| {
                let c = e.data().client_coordinates();
                state.hovered.set(Some((id.clone(), c.x, c.y)));
            },
            onmousemove: move |e| {
                if state.hovered.read().is_some() {
                    let c = e.data().client_coordinates();
                    state.hovered.set(Some((id2.clone(), c.x, c.y)));
                }
            },
            onmouseleave: move |_| state.hovered.set(None),
            if folded {
                // a stack of cards: the loop body is folded inside
                rect { x: "5", y: "5", width: "{node.w:.1}", height: "{node.h:.1}", rx: "{rx}", fill: "var(--bg)", stroke: "{color}", "stroke-width": "1.2", "stroke-opacity": "0.7" }
            }
            // boxes are opaque: the tint sits on the page colour so arrows behind never show through the text
            if !container {
                if diamond {
                    polygon { points: "{poly}", fill: "var(--bg)" }
                } else {
                    rect { width: "{node.w:.1}", height: "{node.h:.1}", rx: "{rx}", fill: "var(--bg)" }
                }
            }
            if diamond {
                polygon { class: "body", points: "{poly}", fill: "{color}", "fill-opacity": "{fill_op}", stroke: "{color}", "stroke-width": "1.6", "stroke-linejoin": "round" }
            } else {
                rect { class: "body", width: "{node.w:.1}", height: "{node.h:.1}", rx: "{rx}", fill: "{color}", "fill-opacity": "{fill_op}", stroke: "{color}", "stroke-width": "1.6" }
            }
            if container {
                rect { width: "{node.w:.1}", height: "24", rx: "{rx}", fill: "{color}", "fill-opacity": "0.25" }
            }
            text { x: "{tx:.1}", y: "{title_y:.1}", "text-anchor": "{anchor}", class: if container { "title cont" } else if fn_title { "title fn-title" } else { "title" },
                if multi {
                    for (i, l) in lines.iter().enumerate() {
                        tspan { x: "{tx:.1}", dy: if i == 0 { "0" } else { "15" }, "{l}" }
                    }
                } else {
                    "{node.label}"
                }
            }
            if has_caption {
                text { x: "{tx:.1}", y: "{node.h - 9.0:.1}", "text-anchor": "{anchor}", class: "cap", "{node.caption}" }
            }
            // hub chips: the hub's own on its top-right border, the hubs it uses along the bottom right
            for (i, c) in node.chips.iter().filter(|c| c.node != node.id).enumerate() {
                Chip { chip: c.clone(), x: node.w - 12.0 - 18.0 * i as f64, y: node.h - 11.0, r: 7.5 }
            }
            for c in node.chips.iter().filter(|c| c.node == node.id) {
                Chip { chip: c.clone(), x: node.w - 4.0, y: 2.0, r: 10.0 }
            }
            if !detail_lines.is_empty() {
                g { class: "detail",
                    rect { x: "0", y: "{node.h + 3.0:.1}", width: "{detail_w:.1}", height: "{detail_h:.1}", rx: "5", fill: "var(--panel)", stroke: "{color}", "stroke-width": "1", "fill-opacity": "0.96" }
                    text { x: "8", y: "{node.h + 17.0:.1}", class: "sub",
                        for (i, l) in detail_lines.iter().enumerate() {
                            tspan { x: "8", dy: if i == 0 { "0" } else { "16" }, "{l}" }
                        }
                    }
                }
            }
            if node.expandable {
                g { class: "expander", transform: "translate({node.w - 1.0:.1},{exp_y:.1})",
                    onclick: move |e| { e.stop_propagation(); state.toggle_expand(&node.id, expanded); },
                    ondoubleclick: move |e| e.stop_propagation(),
                    circle { r: "9" }
                    text { x: "0", y: "4.5", "text-anchor": "middle", if expanded { "−" } else { "+" } }
                }
            }
        }
    }
}

/// A hub chip: coloured dot with the hub's initials; hovering it focuses the hub (its arrows show).
#[component]
fn Chip(chip: iwr_core::views::Chip, x: f64, y: f64, r: f64) -> Element {
    let mut state = use_context::<State>();
    let color = theme::hub_color(chip.hue);
    let fs = if r > 9.0 { 9.5 } else { 8.0 };
    let id = chip.node.clone();
    rsx! {
        g { class: "chip-dot", transform: "translate({x:.1},{y:.1})",
            onmouseenter: move |e| {
                let c = e.data().client_coordinates();
                state.hovered.set(Some((id.clone(), c.x, c.y)));
            },
            onmouseleave: move |_| state.hovered.set(None),
            circle { r: "{r}", fill: "{color}", stroke: "var(--bg)", "stroke-width": "1.5" }
            text { x: "0", y: "{fs * 0.36:.1}", "text-anchor": "middle", "font-size": "{fs}", "{chip.text}" }
        }
    }
}

#[component]
fn Edge(edge: VEdge, segs: Vec<Seg>, class: String) -> Element {
    let (_, dash, width) = theme::edge_style(edge.kind);
    let color = theme::edge_color(edge.kind, &edge.from);
    // counted edges (architecture) get thicker with use, up to about twice the base width
    let width = if edge.weight > 1 { width + (edge.weight as f64).log2().min(4.0) * 0.45 } else { width };
    let head = arrow(&segs, width);
    let tail = if edge.both {
        let rev: Vec<Seg> = segs.iter().rev().map(|s| match *s {
            Seg::Line(a, b) => Seg::Line(b, a),
            Seg::Quad(a, c, b) => Seg::Quad(b, c, a),
            Seg::Cubic(a, c1, c2, b) => Seg::Cubic(b, c2, c1, a),
        }).collect();
        arrow(&rev, width)
    } else {
        None
    };
    // the line stops inside the head, so its round cap never blunts the tip
    let mut line = segs.clone();
    let back = 4.0 + width;
    if head.is_some() {
        if let Some(Seg::Line(a, b)) = line.last_mut() {
            let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            if len > back * 2.0 {
                *b = (b.0 - (b.0 - a.0) / len * back, b.1 - (b.1 - a.1) / len * back);
            }
        }
    }
    let d = layout::edge_path(&line);
    rsx! {
        g { class: "{class}",
            path { d: "{d}", fill: "none", stroke: "{color}", "stroke-width": "{width}", "stroke-dasharray": "{dash}", "stroke-linejoin": "round", "stroke-linecap": "round", opacity: "0.95" }
            if let Some(pts) = head {
                polygon { points: "{pts}", fill: "{color}" }
            }
            if let Some(pts) = tail {
                polygon { points: "{pts}", fill: "{color}" }
            }
        }
    }
}

/// An edge's label, drawn above the boxes at the spot `Graph::label_spots` found for it.
#[component]
fn EdgeLabel(label: String, color: String, x: f64, y: f64, class: String) -> Element {
    let w = iwr_core::views::edge_label_width(&label);
    rsx! {
        g { class: "{class}",
            rect { class: "edge-label-bg", x: "{x - w / 2.0:.1}", y: "{y - 8.0:.1}", width: "{w:.1}", height: "15", rx: "4", stroke: "{color}", "stroke-opacity": "0.35" }
            text { class: "edge-label", x: "{x:.1}", y: "{y + 3.5:.1}", "text-anchor": "middle", fill: "{color}", "{label}" }
        }
    }
}

#[component]
fn Tooltip(node_id: String, x: f64, y: f64, graph: Rc<Graph>) -> Element {
    let state = use_context::<State>();
    let Some(node) = graph.node(&node_id) else { return rsx! {} };
    let project = state.project.read().clone();
    let (_, _, cw, _) = *state.canvas_rect.read();
    let left = if cw > 0.0 && x > cw * 0.6 { x - 440.0 } else { x + 16.0 };
    let top = y + 14.0;
    let kind = theme::node_kind_label(node.kind);
    let color = theme::node_color(node.kind);
    let info = node.item.and_then(|id| project.as_ref().map(|p| iwr_core::views::item_info(p, id)));
    let src = match (&project, node.span) {
        (Some(p), Some(s)) if node.cfg_node.is_some() && node.kind != NodeKind::Entry && node.kind != NodeKind::Exit => Some(p.source_of(&s)),
        _ => None,
    };
    let src = src.map(|s| {
        let lines: Vec<&str> = s.lines().take(8).collect();
        let n = s.lines().count();
        let mut t = lines.join("\n");
        if n > 8 {
            t.push_str("\n…");
        }
        t
    });
    rsx! {
        div { class: "tooltip", style: "left:{left:.0}px; top:{top:.0}px",
            div { class: "t", span { class: "pill", style: "background:{color}; color:#0b1020", "{kind}" } " {node.label.lines().next().unwrap_or(\"\")}" }
            if let (Some(info), true) = (&info, node.cfg_node.is_none() || node.kind == NodeKind::Entry) {
                div { class: "sig", crate::source::RustText { text: info.signature.clone() } }
                if let Some(d) = &info.doc {
                    div { class: "doc", "{d}" }
                } else {
                    div { "{info.summary}" }
                }
                if !info.params.is_empty() {
                    div { style: "margin-top:4px",
                        for p in info.params.iter().take(6) {
                            div { code { "{p.name}" } ": " span { style: "color:#93c5fd", "{p.ty}" } }
                        }
                    }
                }
                if let Some(r) = &info.ret {
                    div { "returns " span { style: "color:#93c5fd", "{r}" } }
                }
                div { style: "margin-top:4px",
                    for f in info.facts.iter() { span { class: "pill", "{f}" } }
                }
                if !info.calls.is_empty() {
                    div { class: "doc", "calls: " {info.calls.iter().map(|c| c.1.clone()).collect::<Vec<_>>().join(", ")} }
                }
                if !info.called_by.is_empty() {
                    div { class: "doc", "called by: " {info.called_by.iter().map(|c| c.1.clone()).collect::<Vec<_>>().join(", ")} }
                }
                div { class: "doc", "{info.location}" }
            } else {
                if let Some(s) = src {
                    pre { "{s}" }
                }
                if let Some(info) = &info {
                    if node.kind == NodeKind::Call || node.kind == NodeKind::Propagate {
                        div { class: "doc", "→ " b { "{info.title}" } ": {info.summary}" }
                        div { class: "sig", crate::source::RustText { text: info.signature.clone() } }
                    }
                }
                if node.kind == NodeKind::Propagate {
                    div { class: "doc", "On Err/None this function returns early to its caller." }
                }
                if node.kind == NodeKind::Panic {
                    div { class: "doc", "May abort with a panic instead of returning an error." }
                }
                if node.kind == NodeKind::Loop {
                    div { class: "doc", "Body repeats; exits on the 'done' edge or a break." }
                    if node.expandable && !node.expanded {
                        div { class: "doc", "Folded: click ⊕ to show the steps inside." }
                    }
                }
            }
            if !node.badges.is_empty() && node.cfg_node.is_none() && info.is_none() {
                div { for b in node.badges.iter() { span { class: "pill", "{b}" } } }
            }
            div { class: "doc", style: "margin-top:4px; font-size:10px",
                if node.expandable && node.cfg_node.is_some() && node.kind != NodeKind::Loop { if node.expanded { "click ⊖ to fold the callee · " } else { "click ⊕ to unfold the callee here · " } }
                else if node.expandable { "click ⊕ to expand · " }
                "click: show source · double-click: "
                if node.cfg_node.is_some() { "open callee" } else if node.badges.iter().any(|b| b.ends_with(" inside")) { "open the modules inside" } else { "expand / open in control flow" }
            }
        }
    }
}
