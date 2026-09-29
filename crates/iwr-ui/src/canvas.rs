//! SVG canvas: pan/zoom, nodes, edges, tooltip, legend.

use crate::theme;
use crate::{focus_nodes, State, Transform};
use dioxus::html::geometry::WheelDelta;
use dioxus::prelude::*;
use iwr_core::views::{EdgeKind, Graph, Mode, NodeKind, VEdge, VNode};
use std::rc::Rc;

fn path_d(points: &[(f64, f64)], backward: bool) -> String {
    if points.len() < 2 {
        return String::new();
    }
    if points.len() == 2 {
        let (x1, y1) = points[0];
        let (x2, y2) = points[1];
        let dir = if y2 >= y1 { 1.0 } else { -1.0 };
        let dy = (y2 - y1).abs().max(20.0) * 0.5;
        return format!("M{:.1},{:.1} C{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}", x1, y1, x1, y1 + dir * dy, x2, y2 - dir * dy, x2, y2);
    }
    if backward {
        // orthogonal side route: keep the corners
        let mut d = format!("M{:.1},{:.1}", points[0].0, points[0].1);
        for p in &points[1..] {
            d.push_str(&format!(" L{:.1},{:.1}", p.0, p.1));
        }
        return d;
    }
    if points.len() == 3 {
        let (x1, y1) = points[0];
        let (cx, cy) = points[1];
        let (x2, y2) = points[2];
        return format!("M{:.1},{:.1} Q{:.1},{:.1} {:.1},{:.1}", x1, y1, cx, cy, x2, y2);
    }
    // smooth spline through the waypoints (dummy-node routing): quadratic segments via midpoints
    let mut d = format!("M{:.1},{:.1}", points[0].0, points[0].1);
    let n = points.len();
    for i in 1..n - 1 {
        let (cx, cy) = points[i];
        let (nx, ny) = points[i + 1];
        let (mx, my) = if i + 1 == n - 1 { (nx, ny) } else { ((cx + nx) / 2.0, (cy + ny) / 2.0) };
        d.push_str(&format!(" Q{:.1},{:.1} {:.1},{:.1}", cx, cy, mx, my));
    }
    d
}

/// Arrow head polygon at the end of a path.
fn arrow(points: &[(f64, f64)]) -> Option<(String, f64, f64)> {
    if points.len() < 2 {
        return None;
    }
    let (x2, y2) = points[points.len() - 1];
    let (x1, y1) = if points.len() == 2 {
        // bezier: approximate the tangent at the end as vertical
        if y2 >= points[0].1 { (x2, y2 - 10.0) } else { (x2, y2 + 10.0) }
    } else if points.len() == 3 {
        points[1]
    } else {
        points[points.len() - 2]
    };
    let (dx, dy) = (x2 - x1, y2 - y1);
    let len = (dx * dx + dy * dy).sqrt().max(0.001);
    let (ux, uy) = (dx / len, dy / len);
    let s = 8.0;
    let p1 = (x2 - ux * s - uy * s * 0.55, y2 - uy * s + ux * s * 0.55);
    let p2 = (x2 - ux * s + uy * s * 0.55, y2 - uy * s - ux * s * 0.55);
    Some((format!("{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}", x2, y2, p1.0, p1.1, p2.0, p2.1), (x1 + x2) / 2.0, (y1 + y2) / 2.0))
}

fn mid(points: &[(f64, f64)]) -> (f64, f64) {
    match points.len() {
        0 => (0.0, 0.0),
        1 => points[0],
        2 => ((points[0].0 + points[1].0) / 2.0, (points[0].1 + points[1].1) / 2.0),
        3 => (0.25 * points[0].0 + 0.5 * points[1].0 + 0.25 * points[2].0, 0.25 * points[0].1 + 0.5 * points[1].1 + 0.25 * points[2].1),
        n => {
            let a = points[n / 2 - 1];
            let b = points[n / 2];
            ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0)
        }
    }
}

#[component]
pub fn Canvas() -> Element {
    let mut state = use_context::<State>();
    let graph: Memo<Rc<Graph>> = use_memo(move || Rc::new(state.build_graph()));
    let mut dragging: Signal<Option<(f64, f64, f64, f64)>> = use_signal(|| None);
    let mut moved = use_signal(|| false);
    let mut svg_el: Signal<Option<Rc<MountedData>>> = use_signal(|| None);

    // fit when requested (mode/root change, load)
    let fit_req = *state.fit_request.read();
    let mut last_fit = use_signal(|| 0u32);
    use_effect(move || {
        let req = *state.fit_request.read();
        if req != *last_fit.peek() {
            last_fit.set(req);
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

    let t = *state.view.read();
    let g = graph.read().clone();
    let mode = *state.mode.read();
    let selected = *state.selected.read();
    let sel_span = *state.selected_span.read();
    let guide = state.guide.read().clone();
    let gidx = *state.guide_idx.read();
    let hi: Vec<String> = guide.as_ref().and_then(|g2| g2.get(gidx)).map(|s| focus_nodes(&g, s)).unwrap_or_default();
    let hovered = state.hovered.read().clone();
    let hover_span = *state.hover_span.read();
    let is_drag = dragging.read().is_some();
    let transform = format!("translate({:.1},{:.1}) scale({:.3})", t.tx, t.ty, t.k);
    let legend = theme::legend(mode);
    let status = state.status.read().clone();
    let node_count = g.nodes.len();
    let root_label = state.root.read().and_then(|r| state.project.read().as_ref().map(|p| p.item(r).name.clone()));
    let scope_label = state.scope.read().and_then(|m| state.project.read().as_ref().map(|p| p.module(m).path.clone()));
    let hovered_id: Option<String> = hovered.as_ref().map(|h| h.0.clone());
    let selected_node_id: Option<String> = g.nodes.iter().find(|n| is_selected(n, selected, sel_span, mode)).map(|n| n.id.clone());
    let focus_id = hovered_id.clone().or(selected_node_id);

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
                g { transform: "{transform}",
                    // containers first
                    for n in g.nodes.iter().filter(|n| n.container) {
                        Node { key: "{n.id}", node: n.clone(), selected: false, highlighted: hi.contains(&n.id), mode, moved }
                    }
                    for e in g.edges.iter() {
                        Edge {
                            key: "{e.id}",
                            edge: e.clone(),
                            highlighted: hi.contains(&e.from) && hi.contains(&e.to),
                            near: focus_id.as_ref().map(|f| &e.from == f || &e.to == f).unwrap_or(false),
                            dim: mode == Mode::Architecture && e.kind == EdgeKind::Dependency && focus_id.as_ref().map(|f| &e.from != f && &e.to != f).unwrap_or(false),
                        }
                    }
                    for n in g.nodes.iter().filter(|n| !n.container) {
                        Node {
                            key: "{n.id}",
                            node: n.clone(),
                            selected: is_selected(n, selected, sel_span, mode),
                            highlighted: hi.contains(&n.id) || line_hit(n, hover_span),
                            mode,
                            moved,
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
                if let Some(sc) = scope_label {
                    span { class: "chip", title: "clear module scope", onclick: move |_| state.set_scope(None), "scope: {sc} ✕" }
                }
                span { class: "status", "{node_count} nodes · {status}" }
                span { class: "status", " · " span { class: "kbd", "f" } " fit  " span { class: "kbd", "g" } " guide  " span { class: "kbd", "←" } span { class: "kbd", "→" } " steps" }
            }
            if let Some(note) = &g.note {
                div { class: "note", "{note}" }
            }
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
            if let Some((id, x, y)) = hovered {
                Tooltip { node_id: id, x, y, graph: graph.read().clone() }
            }
        }
    }
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
fn Node(node: VNode, selected: bool, highlighted: bool, mode: Mode, moved: Signal<bool>) -> Element {
    let mut state = use_context::<State>();
    let color = theme::node_color(node.kind);
    let n = node.clone();
    let id = node.id.clone();
    let id2 = node.id.clone();
    let id3 = node.id.clone();
    let expanded = node.expanded;
    let lines: Vec<String> = node.label.lines().map(|s| s.to_string()).collect();
    let multi = lines.len() > 1;
    let sub_y = 20.0 + 15.0 * (lines.len().max(1) as f64 - 1.0) + 15.0;
    let container = node.container;
    let fill_op = if container { 0.10 } else if node.kind == NodeKind::Block { 0.18 } else { 0.28 };
    let rx = match node.kind {
        NodeKind::Entry | NodeKind::Exit => node.h / 2.0,
        NodeKind::If | NodeKind::Match => 4.0,
        NodeKind::Loop => 14.0,
        _ => 8.0,
    };
    let class = format!("node {}{}", if selected { "sel " } else { "" }, if highlighted { "hi" } else { "" });
    let badge = node.badges.first().cloned();
    let exp_y = if container { 12.0 } else { node.h / 2.0 };
    let badge_x = node.w - if node.expandable { 16.0 } else { 8.0 };
    let title_y = if container { 17.0 } else if multi { 18.0 } else if node.sublabel.is_empty() { node.h / 2.0 + 4.5 } else { 20.0 };
    rsx! {
        g {
            class: "{class}",
            transform: "translate({node.x:.1},{node.y:.1})",
            onclick: move |e| {
                e.stop_propagation();
                if *moved.read() { return; }
                if let Some(item) = n.item {
                    let mut sel_item = item;
                    // for cfg nodes select the enclosing function (root) but keep the statement span
                    if n.cfg_node.is_some() {
                        if let Some(r) = *state.root.read() { sel_item = r; }
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
                if node.cfg_node.is_some() {
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
            rect { class: "body", width: "{node.w:.1}", height: "{node.h:.1}", rx: "{rx}", fill: "{color}", "fill-opacity": "{fill_op}", stroke: "{color}", "stroke-width": "1.6" }
            if container {
                rect { width: "{node.w:.1}", height: "24", rx: "{rx}", fill: "{color}", "fill-opacity": "0.25" }
            }
            text { x: "12", y: "{title_y:.1}", class: if container { "cont" } else { "" },
                if multi {
                    for (i, l) in lines.iter().enumerate() {
                        tspan { x: "12", dy: if i == 0 { "0" } else { "15" }, "{l}" }
                    }
                } else {
                    "{node.label}"
                }
            }
            if !node.sublabel.is_empty() {
                text { x: "12", y: "{sub_y:.1}", class: "sub",
                    for (i, l) in node.sublabel.lines().enumerate() {
                        tspan { x: "12", dy: if i == 0 { "0" } else { "15" }, "{l}" }
                    }
                }
            }
            if let Some(b) = badge {
                if !container {
                    text { x: "{badge_x:.1}", y: "13", class: "sub", "text-anchor": "end", "font-size": "9", fill: "{color}", "{b}" }
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

#[component]
fn Edge(edge: VEdge, highlighted: bool, near: bool, dim: bool) -> Element {
    let (color, dash, width) = theme::edge_style(edge.kind);
    let d = path_d(&edge.points, edge.backward);
    let head = arrow(&edge.points);
    let (mx, my) = mid(&edge.points);
    let label_w = edge.label.as_ref().map(|l| l.chars().count() as f64 * 6.0 + 8.0).unwrap_or(0.0);
    rsx! {
        g { class: format!("edge{}{}{}", if highlighted { " hi" } else { "" }, if near { " near" } else { "" }, if dim { " dim" } else { "" }),
            path { d: "{d}", fill: "none", stroke: "{color}", "stroke-width": "{width}", "stroke-dasharray": "{dash}", "stroke-linejoin": "round", "stroke-linecap": "round", opacity: "0.9" }
            if let Some((pts, _, _)) = head {
                polygon { points: "{pts}", fill: "{color}" }
            }
            if let Some(l) = &edge.label {
                rect { class: "edge-label-bg", x: "{mx - label_w / 2.0:.1}", y: "{my - 8.0:.1}", width: "{label_w:.1}", height: "15", rx: "4" }
                text { class: "edge-label", x: "{mx:.1}", y: "{my + 3.5:.1}", "text-anchor": "middle", fill: "{color}", "{l}" }
            }
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
                div { class: "sig", "{info.signature}" }
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
                        div { class: "sig", "{info.signature}" }
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
                }
            }
            if !node.badges.is_empty() && node.cfg_node.is_none() && info.is_none() {
                div { for b in node.badges.iter() { span { class: "pill", "{b}" } } }
            }
            div { class: "doc", style: "margin-top:4px; font-size:10px",
                if node.expandable { "click ⊕ to expand · " }
                "click: show source · double-click: "
                if node.cfg_node.is_some() { "open callee" } else { "expand / open in control flow" }
            }
        }
    }
}
