//! IwannatRust web UI (Dioxus). Fetches the analyzed project from the CLI
//! server, builds views with `iwr-core` and renders them as interactive SVG.

#![allow(non_snake_case)]

mod canvas;
mod code;
mod md;
mod panels;
mod theme;

use dioxus::prelude::*;
use iwr_core::guide::{GuideOptions, GuideStep};
use iwr_core::model::{ItemId, Project, Span};
use iwr_core::views::{self, Graph, Mode, ViewOptions};
use std::collections::HashSet;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Transform {
    pub tx: f64,
    pub ty: f64,
    pub k: f64,
}

impl Default for Transform {
    fn default() -> Self {
        Self { tx: 20.0, ty: 20.0, k: 1.0 }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Flags {
    pub external: bool,
    pub macros: bool,
    pub tests: bool,
    pub constructs: bool,
}

#[derive(Clone, Copy)]
pub struct State {
    pub project: Signal<Option<Rc<Project>>>,
    pub version: Signal<u64>,
    pub status: Signal<String>,
    pub mode: Signal<Mode>,
    pub root: Signal<Option<ItemId>>,
    pub scope: Signal<Option<usize>>,
    pub expanded: Signal<HashSet<String>>,
    pub collapsed: Signal<HashSet<String>>,
    pub depth: Signal<usize>,
    pub flags: Signal<Flags>,
    /// selected item + optional cfg node
    pub selected: Signal<Option<ItemId>>,
    pub selected_span: Signal<Option<Span>>,
    pub hovered: Signal<Option<(String, f64, f64)>>,
    pub view: Signal<Transform>,
    pub canvas_rect: Signal<(f64, f64, f64, f64)>,
    pub guide: Signal<Option<Rc<Vec<GuideStep>>>>,
    pub guide_idx: Signal<usize>,
    pub playing: Signal<bool>,
    pub search: Signal<String>,
    pub fit_request: Signal<u32>,
    /// Span under the mouse (blocks pane or source pane) for two-way highlighting.
    pub hover_span: Signal<Option<Span>>,
    /// Show the source pane next to the code blocks.
    pub show_source: Signal<bool>,
    /// Opened blocks in the Code view: "i<item>" for items, "i<item>/b<block>" for body blocks.
    pub open_blocks: Signal<HashSet<String>>,
    /// Currently displayed file in the Code view.
    pub code_file: Signal<Option<usize>>,
    pub theme: Signal<String>,
    /// Are the graph-view tabs revealed?
    pub views_open: Signal<bool>,
}

impl State {
    pub fn view_options(&self) -> ViewOptions {
        let f = *self.flags.read();
        ViewOptions {
            root: *self.root.read(),
            module: *self.scope.read(),
            expanded: self.expanded.read().clone(),
            collapsed: self.collapsed.read().clone(),
            depth: *self.depth.read(),
            show_external: f.external,
            show_macros: f.macros,
            show_tests: f.tests,
            show_constructs: f.constructs,
            max_nodes: 400,
        }
    }

    pub fn build_graph(&self) -> Graph {
        match self.project.read().as_ref() {
            Some(p) => views::build(p, *self.mode.read(), &self.view_options()),
            None => Graph::default(),
        }
    }

    pub fn set_mode(&mut self, m: Mode) {
        if *self.mode.read() != m {
            self.mode.set(m);
            self.hovered.set(None);
            self.hover_span.set(None);
            self.fit_request += 1;
        }
    }

    pub fn set_theme(&mut self, t: &str) {
        self.theme.set(t.to_string());
        document::eval(&format!("localStorage.setItem('iwr-theme', '{}');", t));
    }

    /// Show an item in the Code view: switch file, open its block, select it.
    pub fn open_in_code(&mut self, id: ItemId) {
        let Some(p) = self.project.read().clone() else { return };
        let it = p.item(id);
        self.code_file.set(Some(it.span.file));
        self.open_blocks.write().insert(format!("i{}", id));
        if let Some(f) = it.fn_info() {
            if let Some(impl_id) = f.impl_id {
                self.open_blocks.write().insert(format!("i{}", impl_id));
            }
        }
        self.selected.set(Some(id));
        self.selected_span.set(Some(it.span));
    }

    pub fn select_item(&mut self, id: ItemId) {
        let Some(p) = self.project.read().clone() else { return };
        let it = p.item(id);
        self.selected.set(Some(id));
        self.selected_span.set(Some(it.span));
        if *self.mode.read() == Mode::Code {
            self.open_in_code(id);
            return;
        }
        if it.is_callable() {
            let mode = *self.mode.read();
            if matches!(mode, Mode::CallTree | Mode::ControlFlow | Mode::BranchTree) {
                self.set_root(Some(id));
            }
        }
    }

    pub fn set_scope(&mut self, m: Option<usize>) {
        if *self.scope.read() != m {
            self.scope.set(m);
            self.expanded.write().clear();
            self.collapsed.write().clear();
            self.fit_request += 1;
        }
    }

    pub fn set_root(&mut self, r: Option<ItemId>) {
        if *self.root.read() != r {
            self.root.set(r);
            self.expanded.write().clear();
            self.collapsed.write().clear();
            self.fit_request += 1;
        }
    }

    pub fn toggle_expand(&mut self, id: &str, currently: bool) {
        if currently {
            self.expanded.write().remove(id);
            self.collapsed.write().insert(id.to_string());
        } else {
            self.collapsed.write().remove(id);
            self.expanded.write().insert(id.to_string());
        }
    }

    /// Center the view on a node rect (graph coordinates).
    pub fn center_on(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let (_, _, cw, ch) = *self.canvas_rect.read();
        let k = self.view.read().k;
        let cw = if cw > 0.0 { cw } else { 800.0 };
        let ch = if ch > 0.0 { ch } else { 600.0 };
        self.view.set(Transform { tx: cw / 2.0 - (x + w / 2.0) * k, ty: ch / 2.0 - (y + h / 2.0) * k, k });
    }

    pub fn fit(&mut self, g: &Graph) {
        let (_, _, cw, ch) = *self.canvas_rect.read();
        let cw = if cw > 0.0 { cw } else { 800.0 };
        let ch = if ch > 0.0 { ch } else { 600.0 };
        if g.width <= 0.0 || g.height <= 0.0 {
            self.view.set(Transform::default());
            return;
        }
        let k = ((cw - 40.0) / g.width).min((ch - 40.0) / g.height).clamp(0.15, 1.4);
        self.view.set(Transform { tx: (cw - g.width * k) / 2.0, ty: (ch - g.height * k) / 2.0 + 10.0, k });
    }

    // ---------------- guide

    pub fn start_guide(&mut self) {
        let Some(p) = self.project.read().clone() else { return };
        let steps = iwr_core::guide::build(&p, None, &GuideOptions::default());
        self.guide.set(Some(Rc::new(steps)));
        self.guide_idx.set(0);
        self.playing.set(false);
        self.apply_step(0);
    }

    pub fn stop_guide(&mut self) {
        self.guide.set(None);
        self.playing.set(false);
    }

    pub fn guide_step(&mut self, delta: i64) {
        let Some(g) = self.guide.read().clone() else { return };
        let cur = *self.guide_idx.read() as i64;
        let next = (cur + delta).clamp(0, g.len() as i64 - 1) as usize;
        if next as i64 == cur && delta > 0 {
            self.playing.set(false);
        }
        self.guide_idx.set(next);
        self.apply_step(next);
    }

    pub fn apply_step(&mut self, idx: usize) {
        let Some(g) = self.guide.read().clone() else { return };
        let Some(step) = g.get(idx) else { return };
        let step = step.clone();
        self.mode.set(step.mode);
        self.hovered.set(None);
        if let Some(f) = step.focus_item {
            if step.mode.needs_function() || step.mode == Mode::CallTree {
                self.set_root(Some(f));
            }
            self.selected.set(Some(f));
        }
        if let Some(s) = step.span {
            self.selected_span.set(Some(s));
        } else if let Some(f) = step.focus_item {
            if let Some(p) = self.project.read().clone() {
                self.selected_span.set(Some(p.item(f).span));
            }
        }
        // in the call tree expand the whole stack so the focus is visible
        if step.mode == Mode::CallTree {
            let mut id = String::new();
            for (i, it) in step.stack.iter().enumerate() {
                if i == 0 {
                    id = format!("i{}", it);
                } else {
                    id = format!("{}/i{}", id, it);
                }
                self.expanded.write().insert(id.clone());
            }
        }
        // center on the focused node
        let graph = self.build_graph();
        let focus = focus_nodes(&graph, &step);
        if let Some(n) = focus.first().and_then(|id| graph.node(id)) {
            let (x, y, w, h) = (n.x, n.y, n.w, n.h);
            // zoom so the whole graph fits when it can, otherwise a readable level centred on the focus
            self.fit(&graph);
            let k = self.view.read().k.clamp(0.8, 1.2);
            self.view.write().k = k;
            self.center_on(x, y, w, h);
        } else {
            self.fit(&graph);
        }
    }
}

/// Node ids a guide step should highlight in the current graph.
pub fn focus_nodes(g: &Graph, step: &GuideStep) -> Vec<String> {
    let mut out = Vec::new();
    match step.mode {
        Mode::ControlFlow | Mode::BranchTree => {
            if let Some(c) = step.focus_cfg {
                for n in &g.nodes {
                    if n.cfg_node == Some(c) && n.depth == 0 {
                        out.push(n.id.clone());
                    }
                }
            }
        }
        _ => {
            for n in &g.nodes {
                if let Some(it) = n.item {
                    if Some(it) == step.focus_item {
                        out.push(n.id.clone());
                    }
                }
            }
            // deepest stack entry first
            if let Some(last) = step.stack.last() {
                let mut ordered: Vec<String> = g.nodes.iter().filter(|n| n.item == Some(*last)).map(|n| n.id.clone()).collect();
                ordered.extend(out.into_iter());
                out = ordered;
            }
            for n in &g.nodes {
                if let Some(it) = n.item {
                    if step.highlight_items.contains(&it) && !out.contains(&n.id) {
                        out.push(n.id.clone());
                    }
                }
            }
        }
    }
    out
}

async fn fetch_project(url: &str) -> Result<Project, String> {
    let resp = gloo_net::http::Request::get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("server returned {}", resp.status()));
    }
    resp.json::<Project>().await.map_err(|e| e.to_string())
}

async fn fetch_version() -> Option<u64> {
    let resp = gloo_net::http::Request::get("/api/version").send().await.ok()?;
    resp.json::<u64>().await.ok()
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut state = use_context_provider(|| State {
        project: Signal::new(None),
        version: Signal::new(0),
        status: Signal::new("loading…".into()),
        mode: Signal::new(Mode::Code),
        root: Signal::new(None),
        scope: Signal::new(None),
        expanded: Signal::new(HashSet::new()),
        collapsed: Signal::new(HashSet::new()),
        depth: Signal::new(2),
        flags: Signal::new(Flags::default()),
        selected: Signal::new(None),
        selected_span: Signal::new(None),
        hovered: Signal::new(None),
        view: Signal::new(Transform::default()),
        canvas_rect: Signal::new((0.0, 0.0, 0.0, 0.0)),
        guide: Signal::new(None),
        guide_idx: Signal::new(0),
        playing: Signal::new(false),
        search: Signal::new(String::new()),
        fit_request: Signal::new(0),
        hover_span: Signal::new(None),
        show_source: Signal::new(true),
        open_blocks: Signal::new(HashSet::new()),
        code_file: Signal::new(None),
        theme: Signal::new("dark".into()),
        views_open: Signal::new(false),
    });

    // theme persistence
    use_future(move || async move {
        if let Ok(v) = document::eval("return localStorage.getItem('iwr-theme') || 'dark';").await {
            if let Some(t) = v.as_str() {
                if ["dark", "light", "paper"].contains(&t) {
                    state.theme.set(t.to_string());
                }
            }
        }
    });

    // initial load + live reload polling. Without a server (static export) load ./project.json once.
    use_future(move || async move {
        let mut first = true;
        let live = fetch_version().await.is_some();
        loop {
            let v = if live { fetch_version().await.unwrap_or(0) } else { 1 };
            if first || v != *state.version.read() {
                match fetch_project(if live { "/api/project" } else { "project.json" }).await {
                    Ok(p) => {
                        let root = views::default_root(&p);
                        state.project.set(Some(Rc::new(p)));
                        state.version.set(v);
                        if first || state.root.read().is_none() {
                            state.root.set(root);
                        }
                        state.status.set(if first { "ready".into() } else { "reloaded".into() });
                        state.fit_request += 1;
                        if !first {
                            // guide steps refer to old ids: rebuild if active
                            if state.guide.read().is_some() {
                                state.stop_guide();
                            }
                        }
                    }
                    Err(e) => state.status.set(format!("load failed: {}", e)),
                }
                first = false;
            }
            if !live {
                break;
            }
            gloo_timers::future::sleep(std::time::Duration::from_millis(1500)).await;
        }
    });

    // autoplay
    use_future(move || async move {
        loop {
            gloo_timers::future::sleep(std::time::Duration::from_millis(2600)).await;
            if *state.playing.read() {
                state.guide_step(1);
            }
        }
    });

    // keyboard: arrows for guide, f = fit, g = guide
    use_future(move || async move {
        let mut ev = document::eval(
            r#"window.addEventListener('keydown', e => {
                const tag = (document.activeElement && document.activeElement.tagName) || '';
                if (tag === 'INPUT' || tag === 'TEXTAREA') return;
                if (['ArrowLeft','ArrowRight',' ','f','g','Escape'].includes(e.key)) { e.preventDefault(); dioxus.send(e.key); }
            });"#,
        );
        loop {
            match ev.recv::<String>().await {
                Ok(k) => match k.as_str() {
                    "ArrowLeft" => state.guide_step(-1),
                    "ArrowRight" => state.guide_step(1),
                    " " => {
                        if state.guide.read().is_some() {
                            let p = *state.playing.read();
                            state.playing.set(!p);
                        }
                    }
                    "f" => state.fit_request += 1,
                    "g" => {
                        if state.guide.read().is_some() {
                            state.stop_guide()
                        } else {
                            state.start_guide()
                        }
                    }
                    "Escape" => state.stop_guide(),
                    _ => {}
                },
                Err(_) => break,
            }
        }
    });

    let has_guide = state.guide.read().is_some();
    let theme_name = state.theme.read().clone();
    let is_code = *state.mode.read() == Mode::Code;
    rsx! {
        style { {theme::CSS} }
        div { class: "app theme-{theme_name}",
            panels::TopBar {}
            div { class: "main",
                panels::Sidebar {}
                if is_code {
                    code::CodeView {}
                } else {
                    canvas::Canvas {}
                    panels::Details {}
                }
            }
            if has_guide {
                panels::GuideBar {}
            }
        }
    }
}
