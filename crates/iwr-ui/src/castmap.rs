//! The cast map: jump to the codecast of a sub-folder or of a parent folder.
//!
//! `codecast serve` scans the folders around the served project (`GET /api/casts`,
//! `iwr_core::casts::CastMap`); this page draws them as a tree read left to right, a lit box per
//! codecast, the folders in between dimmed. A click asks the server to serve that folder
//! (`POST /api/root`), then reloads the page: the loading page shows the new analysis, the
//! codecast opens on its plan. `codecast serve` only: a static export has one codecast.

use crate::i18n::t;
use crate::icons::Icon;
use crate::State;
use dioxus::prelude::*;
use iwr_core::casts::{CastMap, CastNode};
use std::rc::Rc;

/// Width of a column of the tree and height of a row, in px; a box is `BOX_W` × `BOX_H`.
const COL: f64 = 250.0;
const ROW: f64 = 78.0;
const BOX_W: f64 = 206.0;
const BOX_H: f64 = 58.0;

impl State {
    /// Fetch the cast map (the top bar shows its button once there is somewhere to jump).
    pub async fn load_casts(&mut self) {
        let Ok(resp) = gloo_net::http::Request::get("/api/casts").send().await else { return };
        if !resp.ok() {
            return;
        }
        if let Ok(m) = resp.json::<CastMap>().await {
            self.casts.set(Some(Rc::new(m)));
        }
    }

    /// Is there a codecast to jump to, other than the one served?
    pub fn has_other_casts(&self) -> bool {
        self.casts.read().as_ref().is_some_and(|m| m.nodes.iter().any(|n| n.cast.is_some() && !n.current))
    }

    pub fn open_map(&mut self) {
        self.close_pages();
        self.map_open.set(true);
        let mut state = *self;
        // rescan: codecasts may have been written since
        spawn(async move { state.load_casts().await });
    }
}

/// Ask the server to serve the folder at `path` of the map; `Err` is the server's reason
/// (`recording`, `analyzing`, or a message).
async fn jump(path: &str) -> Result<(), String> {
    let r = gloo_net::http::Request::post("/api/root").json(&serde_json::json!({ "path": path })).map_err(|e| e.to_string())?.send().await.map_err(|e| e.to_string())?;
    if r.ok() {
        Ok(())
    } else {
        let t = r.text().await.unwrap_or_default();
        Err(if t.is_empty() { r.status().to_string() } else { t })
    }
}

/// What a codecast's box says under its title: parts, guessed length, recorded languages.
fn meta(n: &CastNode) -> String {
    let Some(c) = &n.cast else { return String::new() };
    let mut out = format!("{} · ≈ {}", t::castmap::parts(c.parts), crate::generate::clock(c.secs));
    if !c.langs.is_empty() {
        out.push_str(" · ");
        out.push_str(&c.langs.join(" "));
    }
    out
}

/// Full-window overlay drawn over whatever is shown (the loading page included).
#[component]
pub fn CastMapPage() -> Element {
    let mut state = use_context::<State>();
    let mut problem: Signal<Option<String>> = use_signal(|| None);
    let mut going: Signal<Option<String>> = use_signal(|| None);
    let map = state.casts.read().clone();
    let body = match map {
        None => rsx! { div { class: "cm-empty", {t::castmap::scanning()} } },
        Some(m) => {
            let places = m.layout();
            let cols = places.iter().map(|p| p.col).max().unwrap_or(0) as f64 + 1.0;
            let rows = places.iter().map(|p| p.row).fold(0.0, f64::max) + 1.0;
            let (w, h) = (cols * COL - (COL - BOX_W), rows * ROW - (ROW - BOX_H));
            let at = |i: usize| (places[i].col as f64 * COL, places[i].row * ROW);
            let busy = going.read().is_some();
            rsx! {
                div { class: "cm-tree", style: "width:{w}px; height:{h}px",
                    svg { width: "{w}", height: "{h}", view_box: "0 0 {w} {h}",
                        for (i, n) in m.nodes.iter().enumerate() {
                            if let Some(p) = n.parent {
                                {
                                    let (px, py) = at(p);
                                    let (x, y) = at(i);
                                    let (x1, y1, x2, y2) = (px + BOX_W, py + BOX_H / 2.0, x, y + BOX_H / 2.0);
                                    let mx = (x1 + x2) / 2.0;
                                    let d = format!("M{x1:.0} {y1:.0} C{mx:.0} {y1:.0} {mx:.0} {y2:.0} {x2:.0} {y2:.0}");
                                    rsx! { path { class: if n.cast.is_some() || n.current { "cm-edge lit" } else { "cm-edge" }, d: "{d}", style: "animation-delay:{i as f64 * 0.04}s" } }
                                }
                            }
                        }
                    }
                    for (i, n) in m.nodes.iter().enumerate() {
                        {
                            let (x, y) = at(i);
                            let pos = format!("left:{x:.0}px; top:{y:.0}px; animation-delay:{:.2}s", i as f64 * 0.04);
                            let folder = if n.path.is_empty() { n.name.clone() } else { format!("{}/", n.name) };
                            let path = n.path.clone();
                            let is_going = going.read().as_deref() == Some(n.path.as_str());
                            match &n.cast {
                                Some(c) if n.current => rsx! {
                                    div { class: "cm-node cast current", style: "{pos}", title: "{n.path}",
                                        span { class: "cm-here", {t::castmap::here()} }
                                        span { class: "cm-folder", "{folder}" }
                                        span { class: "cm-title", "{c.title}" }
                                        span { class: "cm-meta", "{meta(n)}" }
                                    }
                                },
                                Some(c) => rsx! {
                                    button { class: if is_going { "cm-node cast going" } else { "cm-node cast" }, style: "{pos}", title: t::castmap::open_tip(&c.title), disabled: busy,
                                        onclick: move |_| {
                                            let path = path.clone();
                                            going.set(Some(path.clone()));
                                            problem.set(None);
                                            spawn(async move {
                                                match jump(&path).await {
                                                    // a fresh page: the loading page follows the analysis, then the plan
                                                    Ok(()) => { document::eval("const q = new URLSearchParams(location.search); q.delete('render'); q.set('page', 'plan'); location.search = q.toString();"); }
                                                    Err(e) => {
                                                        going.set(None);
                                                        problem.set(Some(match e.as_str() {
                                                            "recording" => t::castmap::busy_recording().to_string(),
                                                            "analyzing" => t::castmap::busy_analyzing().to_string(),
                                                            _ => t::castmap::failed(e),
                                                        }));
                                                    }
                                                }
                                            });
                                        },
                                        span { class: "cm-folder", "{folder}" }
                                        span { class: "cm-title", "{c.title}" }
                                        span { class: "cm-meta", "{meta(n)}" }
                                    }
                                },
                                None => rsx! {
                                    div { class: if n.current { "cm-node way current" } else { "cm-node way" }, style: "{pos}", title: "{n.path}",
                                        if n.current { span { class: "cm-here", {t::castmap::here()} } }
                                        span { class: "cm-folder", "{folder}" }
                                        if n.current { span { class: "cm-meta", {t::castmap::no_cast()} } }
                                    }
                                },
                            }
                        }
                    }
                }
                if m.partial {
                    div { class: "cm-note", {t::castmap::partial()} }
                }
            }
        }
    };
    let top = state.casts.read().as_ref().map(|m| m.top.clone()).unwrap_or_default();
    rsx! {
        div { class: "castmap", onclick: move |_| state.map_open.set(false),
            div { class: "cm-card", onclick: move |e| e.stop_propagation(),
                div { class: "cm-head",
                    span { class: "cm-ico", Icon { name: "map", size: 22 } }
                    div {
                        div { class: "cm-h", {t::castmap::title()} }
                        div { class: "cm-top", "{top}" }
                    }
                    span { class: "spacer" }
                    button { class: "cm-close", title: t::castmap::close(), "aria-label": t::castmap::close(), onclick: move |_| state.map_open.set(false), "✕" }
                }
                div { class: "cm-scroll", {body} }
                if let Some(p) = problem.read().clone() {
                    div { class: "cm-problem", "{p}" }
                }
                div { class: "cm-hint", {t::castmap::hint()} }
            }
        }
    }
}

pub const CSS: &str = r#"
.castmap { position:fixed; inset:0; z-index:900; display:flex; align-items:center; justify-content:center; background:color-mix(in srgb, var(--bg) 70%, transparent); backdrop-filter: blur(6px); animation: cm-fade .18s ease-out; }
.cm-card { display:flex; flex-direction:column; max-width:calc(100vw - 48px); max-height:calc(100vh - 48px); min-width:min(520px, calc(100vw - 48px)); background:var(--panel); border:1px solid var(--border); border-radius:16px; box-shadow: 0 30px 80px -30px rgba(0,0,0,.7), 0 0 0 1px color-mix(in srgb, var(--accent) 15%, transparent); animation: cm-pop .22s ease-out; }
.cm-head { display:flex; align-items:center; gap:14px; padding:16px 18px 12px; border-bottom:1px solid var(--border); }
.cm-head .spacer { flex:1; }
.cm-ico { display:inline-flex; padding:8px; border-radius:11px; background:color-mix(in srgb, var(--accent) 14%, transparent); }
.cm-h { font-size:16px; font-weight:700; }
.cm-top { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; font-size:11px; color:var(--muted); margin-top:2px; word-break:break-all; }
.cm-close { background:none; border:none; color:var(--muted); font-size:15px; padding:6px 9px; border-radius:8px; cursor:pointer; }
.cm-close:hover { background:var(--panel3); color:var(--text); }
.cm-scroll { overflow:auto; padding:26px 28px; background: radial-gradient(ellipse at 30% 30%, color-mix(in srgb, var(--accent) 7%, var(--panel2)), var(--panel2) 70%); }
.cm-tree { position:relative; margin:0 auto; }
.cm-tree svg { position:absolute; inset:0; pointer-events:none; }
.cm-edge { fill:none; stroke:var(--border); stroke-width:1.5; stroke-dasharray:400; stroke-dashoffset:400; animation: cm-draw .5s ease-out forwards; }
.cm-edge.lit { stroke:color-mix(in srgb, var(--accent) 60%, var(--border)); }
.cm-node { position:absolute; box-sizing:border-box; width:206px; height:58px; display:flex; flex-direction:column; justify-content:center; gap:1px; padding:6px 12px; border-radius:11px; text-align:left; font:inherit; color:var(--text); overflow:hidden; opacity:0; animation: cm-in .3s ease-out forwards; }
.cm-node span { white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
.cm-node.way { background:transparent; border:1px dashed var(--border); color:var(--muted); }
.cm-node.cast { background:color-mix(in srgb, var(--accent) 12%, var(--panel)); border:1px solid color-mix(in srgb, var(--accent) 55%, var(--border)); box-shadow: 0 0 18px -8px var(--accent); }
button.cm-node.cast { cursor:pointer; transition: transform .12s, box-shadow .12s, background .12s; }
button.cm-node.cast:hover:not(:disabled) { transform: translateY(-2px); background:color-mix(in srgb, var(--accent) 22%, var(--panel)); box-shadow: 0 6px 24px -8px var(--accent); }
button.cm-node.cast:disabled { cursor:default; opacity:.55; }
.cm-node.cast.going { opacity:1 !important; animation: cm-pulse .9s ease-in-out infinite; }
.cm-node.current { border-color:var(--hi); box-shadow: 0 0 0 2px color-mix(in srgb, var(--hi) 45%, transparent); overflow:visible; }
.cm-here { position:absolute; top:-9px; right:10px; font-size:9.5px; font-weight:700; letter-spacing:.3px; padding:1px 7px; border-radius:99px; background:var(--hi); color:var(--bg); }
.cm-folder { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; font-size:10.5px; color:var(--muted); }
.cm-node.way .cm-folder { font-size:12px; }
.cm-title { font-size:13px; font-weight:650; }
.cm-meta { font-size:10.5px; color:var(--muted); font-variant-numeric: tabular-nums; }
.cm-empty, .cm-note { color:var(--muted); font-size:12px; padding:8px 0; }
.cm-problem { margin:0 18px; margin-top:12px; border:1px solid color-mix(in srgb, #ef4444 50%, var(--border)); background:color-mix(in srgb, #ef4444 10%, var(--panel2)); border-radius:10px; padding:8px 12px; font-size:12px; }
.cm-hint { padding:10px 18px 14px; font-size:11px; color:var(--muted); }
@keyframes cm-fade { from { opacity:0; } to { opacity:1; } }
@keyframes cm-pop { from { opacity:0; transform: scale(.97) translateY(8px); } to { opacity:1; transform:none; } }
@keyframes cm-in { from { opacity:0; transform: translateX(-8px); } to { opacity:1; transform:none; } }
@keyframes cm-draw { to { stroke-dashoffset:0; } }
@keyframes cm-pulse { 50% { box-shadow: 0 0 28px -4px var(--accent); } }
@media (prefers-reduced-motion: reduce) { .castmap * { animation-duration:.01s !important; } }
"#;
