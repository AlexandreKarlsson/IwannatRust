//! IwannatRust web UI (Dioxus). Fetches the analyzed project from the CLI
//! server, builds views with `iwr-core` and renders them as interactive SVG.

#![allow(non_snake_case)]

mod canvas;
mod code;
mod icons;
mod md;
mod panels;
mod player;
mod settings;
mod theme;

use dioxus::prelude::*;
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
    /// item kinds hidden in the Structure view
    pub hidden_kinds: Signal<HashSet<iwr_core::model::ItemKind>>,
    /// selected item + optional cfg node
    pub selected: Signal<Option<ItemId>>,
    pub selected_span: Signal<Option<Span>>,
    pub hovered: Signal<Option<(String, f64, f64)>>,
    pub view: Signal<Transform>,
    pub canvas_rect: Signal<(f64, f64, f64, f64)>,
    // ---- codecast player
    pub script: Signal<Option<Rc<iwr_core::script::Script>>>,
    pub part_idx: Signal<usize>,
    pub cue_idx: Signal<usize>,
    pub playing: Signal<bool>,
    pub voice: Signal<player::Voice>,
    /// recording given from outside the script (`--audio` or a picked file); `audio:` lines win over it
    pub audio_url: Signal<Option<String>>,
    /// where relative `audio:` names resolve (`/api/codecast/` live, `codecast/` in an export)
    pub audio_base: Signal<String>,
    /// bumps cancel in-flight speech
    pub tts_gen: Signal<u32>,
    /// resolved highlight refs of the current cue
    pub hl: Signal<Vec<iwr_core::script::Resolved>>,
    /// the `!` refs as written: diagram nodes are glowed by id
    pub hl_ids: Signal<Vec<String>>,
    /// the diagram shown by `@ diagram` (from the current part)
    pub diagram: Signal<Option<Rc<iwr_core::diagram::Diagram>>>,
    /// `>` ops of the current part up to the current cue, replayed on the diagram
    pub diagram_ops: Signal<Vec<iwr_core::diagram::Op>>,
    /// code span to underline (may have columns)
    pub code_mark: Signal<Option<Span>>,
    /// question being answered (the codecast is paused meanwhile)
    pub answering: Signal<Option<iwr_core::script::Question>>,
    /// was the codecast playing when the question was clicked? resume after the answer
    pub resume_after_answer: Signal<bool>,
    /// glossary terms (and script questions) already answered this session
    pub asked: Signal<Vec<String>>,
    /// built-in glossary merged with the project's own (`glossary.md` next to the codecast)
    pub glossary: Signal<Rc<Vec<iwr_core::glossary::Entry>>>,
    /// offer glossary questions at all (setting)
    pub glossary_on: Signal<bool>,
    /// how code is spoken: built-in + project glossary `pronounce:` + the script's own
    pub pronounce: Signal<Rc<Vec<(String, String)>>>,
    /// `pronounce:` lines of the project glossary (kept to re-merge when a script loads)
    pub glossary_pronounce: Signal<Vec<(String, String)>>,
    /// caption shows the spoken form in parentheses after code (setting, off by default)
    pub speech_parens: Signal<bool>,
    pub search: Signal<String>,
    pub fit_request: Signal<u32>,
    /// last `fit_request` handled (by the canvas fit, or by a codecast cue that placed the camera itself)
    pub fit_done: Signal<u32>,
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
    // ---- layout
    pub files_tab: Signal<bool>,
    pub closed_dirs: Signal<HashSet<String>>,
    pub sidebar_open: Signal<bool>,
    pub sidebar_w: Signal<f64>,
    pub details_w: Signal<f64>,
    /// blocks pane share of the code view, 0.2..0.8
    pub split: Signal<f64>,
    /// active drag: (which, start x, start value)
    pub drag: Signal<Option<(u8, f64, f64)>>,
    /// animate camera / scroll (set by the codecast player, cleared by manual pan/zoom)
    pub animate: Signal<bool>,
    // ---- settings
    pub settings_open: Signal<bool>,
    /// full tour: continue with the next part when one ends
    pub tour: Signal<bool>,
    pub tts_rate: Signal<f64>,
    /// what browser TTS is doing (voices, events), shown in the player
    pub tts_info: Signal<String>,
    /// seconds per cue in silent reading mode
    pub read_delay: Signal<f64>,
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
            hidden_kinds: self.hidden_kinds.read().clone(),
        }
    }

    pub fn build_graph(&self) -> Graph {
        if *self.mode.read() == Mode::Diagram {
            return match self.diagram.read().as_ref() {
                Some(d) => iwr_core::diagram::graph(d, &self.diagram_ops.read()),
                None => Graph { note: Some("No diagram here: a codecast draws one with a ```mermaid block and `@ diagram`.".into()), ..Default::default() },
            };
        }
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

    pub fn save_layout(&self) {
        document::eval(&format!(
            "localStorage.setItem('iwr-layout', '{},{},{},{}');",
            if *self.sidebar_open.read() { 1 } else { 0 },
            *self.sidebar_w.read(),
            *self.details_w.read(),
            *self.split.read()
        ));
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

    /// Smooth scroll an element into view (codecast) or jump (manual).
    pub fn scroll_to(&self, id: &str) {
        let behavior = if *self.animate.read() { "smooth" } else { "auto" };
        document::eval(&format!("setTimeout(() => {{ const el = document.getElementById('{}'); if (el) el.scrollIntoView({{block:'center', behavior:'{}'}}); }}, 30);", id, behavior));
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
        hidden_kinds: Signal::new(HashSet::new()),
        selected: Signal::new(None),
        selected_span: Signal::new(None),
        hovered: Signal::new(None),
        view: Signal::new(Transform::default()),
        canvas_rect: Signal::new((0.0, 0.0, 0.0, 0.0)),
        script: Signal::new(None),
        part_idx: Signal::new(0),
        cue_idx: Signal::new(0),
        playing: Signal::new(false),
        voice: Signal::new(player::Voice::Tts),
        audio_url: Signal::new(None),
        audio_base: Signal::new("/api/codecast/".into()),
        tts_gen: Signal::new(0),
        hl: Signal::new(vec![]),
        hl_ids: Signal::new(vec![]),
        diagram: Signal::new(None),
        diagram_ops: Signal::new(vec![]),
        code_mark: Signal::new(None),
        answering: Signal::new(None),
        resume_after_answer: Signal::new(false),
        asked: Signal::new(vec![]),
        glossary: Signal::new(Rc::new(iwr_core::glossary::builtin())),
        glossary_on: Signal::new(true),
        pronounce: Signal::new(Rc::new(iwr_core::speech::builtin_pronounce())),
        glossary_pronounce: Signal::new(vec![]),
        speech_parens: Signal::new(false),
        search: Signal::new(String::new()),
        fit_request: Signal::new(0),
        fit_done: Signal::new(0),
        hover_span: Signal::new(None),
        show_source: Signal::new(true),
        open_blocks: Signal::new(HashSet::new()),
        code_file: Signal::new(None),
        theme: Signal::new("dark".into()),
        views_open: Signal::new(false),
        files_tab: Signal::new(true),
        closed_dirs: Signal::new(HashSet::new()),
        sidebar_open: Signal::new(true),
        sidebar_w: Signal::new(250.0),
        details_w: Signal::new(380.0),
        split: Signal::new(0.5),
        drag: Signal::new(None),
        animate: Signal::new(false),
        settings_open: Signal::new(false),
        tour: Signal::new(true),
        tts_rate: Signal::new(1.0),
        tts_info: Signal::new(String::new()),
        read_delay: Signal::new(4.0),
    });

    // layout persistence
    use_future(move || async move {
        if let Ok(v) = document::eval("return localStorage.getItem('iwr-layout') || '';").await {
            if let Some(t) = v.as_str() {
                let parts: Vec<f64> = t.split(',').filter_map(|x| x.parse().ok()).collect();
                if parts.len() == 4 {
                    state.sidebar_w.set(parts[1].clamp(140.0, 600.0));
                    state.details_w.set(parts[2].clamp(240.0, 900.0));
                    state.split.set(parts[3].clamp(0.2, 0.8));
                }
            }
        }
    });

    // theme persistence
    use_future(move || async move {
        if let Ok(v) = document::eval("return localStorage.getItem('iwr-theme') || 'dark';").await {
            if let Some(t) = v.as_str() {
                if theme::is_theme(t) {
                    state.theme.set(t.to_string());
                }
            }
        }
    });

    // settings persistence
    use_future(move || async move {
        if let Ok(v) = document::eval("return localStorage.getItem('iwr-settings') || '';").await {
            if let Some(t) = v.as_str() {
                if let Ok(s) = serde_json::from_str::<settings::Saved>(t) {
                    state.apply_settings(s);
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
                        // a playing codecast keeps its camera: the cue is re-applied below
                        if first || state.script.read().is_none() {
                            state.fit_request += 1;
                        }
                        if first {
                            if let Some((text, audio, base, glossary)) = player::fetch_script(live).await {
                                state.audio_url.set(audio);
                                state.audio_base.set(base);
                                if let Some(g) = glossary {
                                    let own = iwr_core::glossary::parse(&g);
                                    state.glossary.set(Rc::new(iwr_core::glossary::merge(&[iwr_core::glossary::builtin(), own])));
                                    state.glossary_pronounce.set(iwr_core::speech::parse_pronounce(&g));
                                }
                                state.load_script(iwr_core::script::parse(&text));
                            }
                        } else if state.script.read().is_some() {
                            // refs are re-resolved on the next cue; just re-apply
                            state.apply_cue();
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

    // autoplay in silent reading mode (voice modes advance themselves)
    use_future(move || async move {
        loop {
            let ms = (*state.read_delay.peek() * 1000.0).clamp(1000.0, 20000.0) as u64;
            gloo_timers::future::sleep(std::time::Duration::from_millis(ms)).await;
            if *state.playing.read() && *state.voice.read() == player::Voice::Read && !state.advance() {
                state.playing.set(false);
            }
        }
    });

    // keyboard: arrows for guide, f = fit, g = guide
    use_future(move || async move {
        let mut ev = document::eval(
            r#"window.addEventListener('keydown', e => {
                const tag = (document.activeElement && document.activeElement.tagName) || '';
                if (tag === 'INPUT' || tag === 'TEXTAREA') return;
                if (['ArrowLeft','ArrowRight',' ','f','g','t',',','Escape'].includes(e.key)) { e.preventDefault(); dioxus.send(e.key); }
            });"#,
        );
        loop {
            match ev.recv::<String>().await {
                Ok(k) => match k.as_str() {
                    "ArrowLeft" => { state.tts_gen += 1; state.step(-1); }
                    "ArrowRight" => { state.tts_gen += 1; state.step(1); }
                    " " => {
                        if state.answering.read().is_some() {
                            state.finish_answer();
                        } else if state.script.read().is_some() {
                            let p = *state.playing.read();
                            state.tts_gen += 1;
                            state.playing.set(!p);
                        }
                    }
                    "f" => state.fit_request += 1,
                    "g" => {
                        if state.script.read().is_some() {
                            state.stop_script()
                        } else {
                            state.start_builtin_guide(None)
                        }
                    }
                    "t" => { state.settings_open.set(false); state.start_tour() }
                    "," => { let v = *state.settings_open.read(); state.settings_open.set(!v) }
                    "Escape" => {
                        if *state.settings_open.read() {
                            state.settings_open.set(false)
                        } else if state.answering.read().is_some() {
                            state.finish_answer()
                        } else {
                            state.stop_script()
                        }
                    }
                    _ => {}
                },
                Err(_) => break,
            }
        }
    });

    let has_guide = state.script.read().is_some();
    let theme_name = state.theme.read().clone();
    let is_code = *state.mode.read() == Mode::Code;
    let is_plan = *state.mode.read() == Mode::Plan;
    let sidebar_open = *state.sidebar_open.read();
    let dragging = state.drag.read().is_some();
    let settings_open = *state.settings_open.read();
    rsx! {
        document::Link { rel: "icon", r#type: "image/png", href: icons::uri("favicon") }
        style { {theme::CSS} }
        div { class: format!("app theme-{}{}", theme_name, if dragging { " dragging" } else { "" }),
            panels::TopBar {}
            if settings_open {
                settings::SettingsPage {}
            } else {
            div { class: "main",
                onmousemove: move |e| {
                    if let Some((which, x0, v0)) = *state.drag.read() {
                        let x = e.data().client_coordinates().x;
                        match which {
                            0 => state.sidebar_w.set((v0 + (x - x0)).clamp(140.0, 600.0)),
                            1 => state.details_w.set((v0 - (x - x0)).clamp(240.0, 900.0)),
                            _ => {
                                let (_, _, cw, _) = *state.canvas_rect.read();
                                let total = if cw > 0.0 { cw } else { 1000.0 };
                                state.split.set((v0 + (x - x0) / total).clamp(0.2, 0.8));
                            }
                        }
                    }
                },
                onmouseup: move |_| { if state.drag.read().is_some() { state.drag.set(None); state.save_layout(); } },
                onmouseleave: move |_| { if state.drag.read().is_some() { state.drag.set(None); state.save_layout(); } },
                if sidebar_open {
                    panels::Sidebar {}
                    div { class: "resizer", onmousedown: move |e| { e.prevent_default(); state.drag.set(Some((0, e.data().client_coordinates().x, *state.sidebar_w.read()))); } }
                }
                if is_code {
                    code::CodeView {}
                } else if is_plan {
                    player::PlanPage {}
                } else {
                    canvas::Canvas {}
                    div { class: "resizer", onmousedown: move |e| { e.prevent_default(); state.drag.set(Some((1, e.data().client_coordinates().x, *state.details_w.read()))); } }
                    panels::Details {}
                }
            }
            }
            if has_guide {
                player::PlayerBar {}
            }
        }
    }
}
