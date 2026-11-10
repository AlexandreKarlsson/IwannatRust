//! IwannatRust web UI (Dioxus). Fetches the analyzed project from the CLI
//! server, builds views with `iwr-core` and renders them as interactive SVG.

#![allow(non_snake_case)]

mod canvas;
mod castmap;
mod chart;
mod code;
mod design;
mod fonts;
mod generate;
mod i18n;
mod icons;
mod loading;
mod md;
mod panels;
mod player;
mod prompt;
mod settings;
mod side;
mod source;
mod subtitles;
mod theme;
mod tutorial;

use crate::i18n::t;
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

/// Default file tree width: a share of the window width.
pub const SIDEBAR_W: f64 = 0.10;
/// Default details / side panel width: a share of the window width (the graph gets the rest).
pub const DETAILS_W: f64 = 0.45;

/// Keep the file tree between 6% and 40% of the window.
pub fn clamp_sidebar(f: f64) -> f64 {
    f.clamp(0.06, 0.4)
}

/// Keep the details panel between 15% and 70% of the window.
pub fn clamp_details(f: f64) -> f64 {
    f.clamp(0.15, 0.7)
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
    /// the Kokoro voice whose recordings play (`""`: the one that recorded the most parts)
    pub rec_voice: Signal<String>,
    /// the browser voices the viewer picked (part of a name; `""`: the codecast's, else automatic):
    /// the main one, and the one of the `sp:` lines
    pub tts_voice: Signal<String>,
    pub tts_voice2: Signal<String>,
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
    /// folders highlighted by the current cue (`! src/storage/`), as the file tree names them
    pub hl_dirs: Signal<Vec<String>>,
    /// the panel the current cue enlarges (`+ files`); `None`: every panel at its own size
    pub focus: Signal<Option<iwr_core::script::Panel>>,
    /// the part of the app the current cue lights up (`+ search`, see `iwr_core::script::UI_SPOTS`)
    pub spot: Signal<Option<&'static str>>,
    /// what the screen zooms onto (`+ zoom`): `""` the highlight, else a panel or a spot name
    pub zoom: Signal<Option<&'static str>>,
    /// the codecast found next to the project, with its recording settings (`audio_url`,
    /// `audio_base`): what the codecast button plays again after another one (the tutorial)
    pub own_script: Signal<Option<(Rc<iwr_core::script::Script>, Option<String>, String)>>,
    /// the diagram shown by `@ diagram` (from the current part)
    pub diagram: Signal<Option<Rc<iwr_core::diagram::Diagram>>>,
    /// the chart shown by `@ chart` (from the current part)
    pub chart: Signal<Option<Rc<iwr_core::chart::Chart>>>,
    /// how far the voice is into the current cue, 0 to 1: chart markers up to there have fired
    pub cue_frac: Signal<f64>,
    /// `>` ops of the current part up to the current cue, replayed on the diagram
    pub diagram_ops: Signal<Vec<iwr_core::diagram::Op>>,
    /// what the side panel shows (`|` lines of a codecast), next to the view
    pub side: Signal<Option<Rc<iwr_core::script::Side>>>,
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
    /// file tree width, as a share of the window width (0.06..0.4)
    pub sidebar_w: Signal<f64>,
    /// details / side panel width, as a share of the window width (0.15..0.7)
    pub details_w: Signal<f64>,
    /// window width in px, to turn mouse drags into shares
    pub win_w: Signal<f64>,
    /// blocks pane share of the code view, 0.2..0.8
    pub split: Signal<f64>,
    /// active drag: (which, start x, start value)
    pub drag: Signal<Option<(u8, f64, f64)>>,
    /// animate camera / scroll (set by the codecast player, cleared by manual pan/zoom)
    pub animate: Signal<bool>,
    // ---- first start
    /// the "have you used this app before?" dialog
    pub welcome_open: Signal<bool>,
    /// video render mode (`?render=N`): only the stage and the caption, time fed by `codecast video`
    pub render: Signal<bool>,
    // ---- settings
    pub settings_open: Signal<bool>,
    /// the cast map (jump to another folder's codecast) is shown over the page
    pub map_open: Signal<bool>,
    /// the codecasts around the served project, from `/api/casts` (`codecast serve` only)
    pub casts: Signal<Option<Rc<iwr_core::casts::CastMap>>>,
    /// the Generate page (record the codecast's voice with Kokoro)
    pub generate_open: Signal<bool>,
    /// every part and its recordings, from `/api/voice` (`codecast serve` only)
    pub voice_status: Signal<Option<Rc<iwr_core::voice::VoiceStatus>>>,
    /// the server's running (or last) recording
    pub voice_job: Signal<Option<iwr_core::voice::VoiceJob>>,
    /// full tour: continue with the next part when one ends
    pub tour: Signal<bool>,
    pub tts_rate: Signal<f64>,
    /// what browser TTS is doing (voices, events), shown in the player
    pub tts_info: Signal<String>,
    /// seconds per cue in silent reading mode
    pub read_delay: Signal<f64>,
    // ---- language
    /// is an `codecast serve` behind the page (else a static export)?
    pub live: Signal<bool>,
    /// language the codecast plays in (`en` = as written; see `iwr_core::i18n::LANGS`)
    pub lang: Signal<String>,
    /// the loaded script in English, tours expanded: what every translation starts from
    pub script_src: Signal<Option<Rc<iwr_core::script::Script>>>,
    /// where the loaded codecast comes from (a written script, the built-in tour), to tell it again
    /// in a language iwr-core speaks itself (see `State::told`)
    pub source: Signal<Option<Rc<iwr_core::script::Source>>>,
    /// the glossary with its questions and answers translated (same entries, same order)
    pub glossary_tr: Signal<Option<Rc<Vec<iwr_core::glossary::Entry>>>>,
    /// translation progress or failure, shown in the player
    pub lang_info: Signal<String>,
    // ---- subtitles (`subtitles.rs`)
    /// language of the subtitle under the caption (`""`: off)
    pub sub_lang: Signal<String>,
    /// the script in the subtitle language: same parts and cues as `script`
    pub sub_script: Signal<Option<Rc<iwr_core::script::Script>>>,
    /// per language: pieces whose translation already exists, of how many (`None`: not checked yet)
    pub sub_avail: Signal<Option<Rc<Vec<crate::subtitles::Avail>>>>,
    /// the codecast's `codecast.toml`: fonts, colours, default settings
    pub config: Signal<Rc<iwr_core::config::Config>>,
    /// font of the box titles and headings: a bundled font's id, a codecast font file, or `""`
    pub font_title: Signal<String>,
    /// font of the interface text (same values)
    pub font_text: Signal<String>,
    /// where its font files are fetched from (`/api/codecast/`, or `codecast/` in an export)
    pub config_base: Signal<String>,
    /// `codecast.toml` was fetched and applied (or there is none): render mode waits for it
    pub config_loaded: Signal<bool>,
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
            lang: i18n::core_lang(),
            hidden_kinds: self.hidden_kinds.read().clone(),
        }
    }

    pub fn build_graph(&self) -> Graph {
        if *self.mode.read() == Mode::Diagram {
            return match self.diagram.read().as_ref() {
                Some(d) => {
                    let mut g = iwr_core::diagram::graph(d, &self.diagram_ops.read());
                    // core notes an empty diagram in English: say it in the interface language
                    if g.nodes.is_empty() && g.note.is_some() {
                        let l = i18n::core_lang();
                        let note = if d.seq.is_some() { iwr_core::tr::t::views::note_empty_sequence(l) } else { iwr_core::tr::t::views::note_empty_diagram(l) };
                        g.note = Some(note.into());
                    }
                    g
                }
                None => Graph { note: Some(t::app::no_diagram().into()), ..Default::default() },
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

    /// Leave the Settings or Generate page for the views.
    pub fn close_pages(&mut self) {
        self.settings_open.set(false);
        self.generate_open.set(false);
        self.map_open.set(false);
    }

    pub fn save_layout(&self) {
        document::eval(&format!(
            "localStorage.setItem('iwr-layout2', '{},{},{},{}');",
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
            if matches!(mode, Mode::CallTree | Mode::Sequence | Mode::ControlFlow | Mode::BranchTree) {
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
        return Err(t::app::server_returned(resp.status()));
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
        status: Signal::new(t::app::loading().into()),
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
        rec_voice: Signal::new(String::new()),
        tts_voice: Signal::new(String::new()),
        tts_voice2: Signal::new(String::new()),
        audio_url: Signal::new(None),
        audio_base: Signal::new("/api/codecast/".into()),
        config: Signal::new(Rc::new(Default::default())),
        config_base: Signal::new("/api/codecast/".into()),
        config_loaded: Signal::new(false),
        font_title: Signal::new(String::new()),
        font_text: Signal::new(String::new()),
        tts_gen: Signal::new(0),
        hl: Signal::new(vec![]),
        hl_ids: Signal::new(vec![]),
        hl_dirs: Signal::new(vec![]),
        focus: Signal::new(None),
        spot: Signal::new(None),
        zoom: Signal::new(None),
        own_script: Signal::new(None),
        welcome_open: Signal::new(false),
        render: Signal::new(false),
        diagram: Signal::new(None),
        diagram_ops: Signal::new(vec![]),
        chart: Signal::new(None),
        cue_frac: Signal::new(1.0),
        side: Signal::new(None),
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
        sidebar_w: Signal::new(SIDEBAR_W),
        details_w: Signal::new(DETAILS_W),
        win_w: Signal::new(1600.0),
        split: Signal::new(0.5),
        drag: Signal::new(None),
        animate: Signal::new(false),
        settings_open: Signal::new(false),
        map_open: Signal::new(false),
        casts: Signal::new(None),
        generate_open: Signal::new(false),
        voice_status: Signal::new(None),
        voice_job: Signal::new(None),
        tour: Signal::new(true),
        tts_rate: Signal::new(1.0),
        live: Signal::new(false),
        lang: Signal::new("en".into()),
        script_src: Signal::new(None),
        source: Signal::new(None),
        glossary_tr: Signal::new(None),
        lang_info: Signal::new(String::new()),
        sub_lang: Signal::new(String::new()),
        sub_script: Signal::new(None),
        sub_avail: Signal::new(None),
        tts_info: Signal::new(String::new()),
        read_delay: Signal::new(4.0),
    });

    // layout persistence
    use_future(move || async move {
        if let Ok(v) = document::eval("return localStorage.getItem('iwr-layout2') || '';").await {
            if let Some(t) = v.as_str() {
                let parts: Vec<f64> = t.split(',').filter_map(|x| x.parse().ok()).collect();
                if parts.len() == 4 {
                    state.sidebar_w.set(clamp_sidebar(parts[1]));
                    state.details_w.set(clamp_details(parts[2]));
                    state.split.set(parts[3].clamp(0.2, 0.8));
                }
            }
        }
    });
    // window width, kept current, so drags in px become shares of the window
    use_future(move || async move {
        let mut ev = document::eval(
            "dioxus.send(window.innerWidth); window.addEventListener('resize', () => dioxus.send(window.innerWidth));",
        );
        while let Ok(w) = ev.recv::<f64>().await {
            if w > 0.0 {
                state.win_w.set(w);
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

    // first start: ask whether the app is new to the viewer (once per browser)
    use_future(move || async move {
        if let Ok(v) = document::eval("try { return (new URLSearchParams(location.search).has('render') ? 'x' : localStorage.getItem('iwr-welcomed') || ''); } catch (e) { return 'x'; }").await {
            if v.as_str() == Some("") {
                state.welcome_open.set(true);
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
        // `?lang=ja` in the URL plays the codecast in that language (not saved)
        if let Ok(v) = document::eval("return new URLSearchParams(location.search).get('lang') || '';").await {
            if let Some(l) = v.as_str().filter(|l| iwr_core::i18n::lang(l).is_some()) {
                state.set_lang(l);
            }
        }
        // `?page=generate` / `?page=settings` / `?page=map` open that page
        if let Ok(v) = document::eval("return new URLSearchParams(location.search).get('page') || '';").await {
            match v.as_str() {
                Some("generate") => state.generate_open.set(true),
                Some("settings") => state.settings_open.set(true),
                Some("map") => state.open_map(),
                _ => {}
            }
        }
        // the codecast's own defaults come after the viewer's saved settings
        state.load_config().await;
    });

    // initial load + live reload polling. Without a server (static export) load ./project.json once.
    use_future(move || async move {
        let mut first = true;
        let live = fetch_version().await.is_some();
        state.live.set(live);
        if live {
            spawn(async move { state.load_casts().await });
        }
        loop {
            let v = if live { fetch_version().await.unwrap_or(0) } else { 1 };
            // version 0: the server is still analyzing (the loading page follows `/api/status`)
            if live && v == 0 {
                // back to 0 after a project was shown: the server now serves another folder
                // (the cast map, maybe in another tab). Start over on a fresh page
                if !first {
                    document::eval("location.reload();");
                    break;
                }
                gloo_timers::future::sleep(std::time::Duration::from_millis(300)).await;
                continue;
            }
            if first || v != *state.version.read() {
                match fetch_project(if live { "/api/project" } else { "project.json" }).await {
                    Ok(p) => {
                        // another folder's project (the jump was missed between two polls)
                        if state.project.peek().as_ref().is_some_and(|old| old.root != p.root) {
                            document::eval("location.reload();");
                            break;
                        }
                        let root = views::default_root(&p);
                        // the file tree opens with every folder closed
                        if first {
                            state.closed_dirs.set(p.files.iter().flat_map(|f| f.path.match_indices('/').map(|(i, _)| f.path[..i].to_string()).collect::<Vec<_>>()).collect());
                        }
                        state.project.set(Some(Rc::new(p)));
                        state.version.set(v);
                        if first || state.root.read().is_none() {
                            state.root.set(root);
                        }
                        state.status.set(if first { t::app::ready().into() } else { t::app::reloaded().into() });
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
                                let s = iwr_core::script::parse(&text);
                                state.own_script.set(Some((Rc::new(s.clone()), state.audio_url.peek().clone(), state.audio_base.peek().clone())));
                                state.load_script(s);
                                // `?page=plan`: where a jump from the cast map lands
                                if let Ok(v) = document::eval("return new URLSearchParams(location.search).get('page') || '';").await {
                                    if v.as_str() == Some("plan") {
                                        state.set_mode(Mode::Plan);
                                    }
                                }
                                // `?render=N&voice=V`: `codecast video` films part N with voice V
                                if let Ok(v) = document::eval("const q = new URLSearchParams(location.search); return [q.get('render') || '', q.get('voice') || '', q.get('sub') || ''];").await {
                                    let get = |i: usize| v.get(i).and_then(|x| x.as_str()).unwrap_or("").to_string();
                                    if let Ok(n) = get(0).parse::<usize>() {
                                        state.start_render(n.saturating_sub(1), &get(1), &get(2));
                                    }
                                }
                            }
                        } else if state.script.read().is_some() {
                            // refs are re-resolved on the next cue; just re-apply
                            state.apply_cue();
                        }
                    }
                    Err(e) => state.status.set(t::app::load_failed(e)),
                }
                first = false;
            }
            if !live {
                break;
            }
            gloo_timers::future::sleep(std::time::Duration::from_millis(1500)).await;
        }
    });

    // render mode: tell `codecast video` when the part can be filmed (translated, its recording
    // and subtitles in), or why it cannot
    use_effect(move || {
        if !*state.render.read() {
            return;
        }
        let lang = state.lang.read().clone();
        let info = state.lang_info.read().clone();
        let translated = lang == "en" || state.glossary_tr.read().is_some();
        let subs = state.sub_lang.read().is_empty() || state.sub_script.read().is_some();
        let ready = translated && subs && state.part_has_audio() && *state.config_loaded.read();
        let err = if info.starts_with('⚠') { info } else { String::new() };
        document::eval(&format!("window.__codecastReady = {ready}; window.__codecastError = {};", serde_json::to_string(&err).unwrap_or_default()));
    });

    // the Generate page's recording: status once, then follow a running job (it outlives the page)
    use_future(move || async move { generate::watch(state).await });

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
                            state.start_codecast()
                        }
                    }
                    "t" => { state.close_pages(); state.start_tour() }
                    "," => { let v = *state.settings_open.read(); state.close_pages(); state.settings_open.set(!v) }
                    "Escape" => {
                        if *state.map_open.read() {
                            state.map_open.set(false)
                        } else if *state.welcome_open.read() {
                            state.close_welcome()
                        } else if *state.settings_open.read() || *state.generate_open.read() {
                            state.close_pages()
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
    let chart = (*state.mode.read() == Mode::Chart).then(|| state.chart.read().clone());
    let is_graph = !is_code && !is_plan && chart.is_none();
    let is_diagram = *state.mode.read() == Mode::Diagram;
    let sidebar_open = *state.sidebar_open.read();
    let dragging = state.drag.read().is_some();
    let settings_open = *state.settings_open.read();
    let generate_open = *state.generate_open.read();
    let has_side = state.side.read().is_some();
    let focus = *state.focus.read();
    // during a codecast an empty details panel (nothing selected, no source line, not enlarged by
    // `+ source`) stays closed: the view takes its width
    let details_empty = has_guide && state.selected.read().is_none() && state.selected_span.read().is_none()
        && focus != Some(iwr_core::script::Panel::Source);
    let focus_cls = focus.map(|f| format!(" focus-{}", f.name())).unwrap_or_default();
    // a cue that enlarges the sidebar shows it even when the viewer closed it
    let spot = *state.spot.read();
    let render = *state.render.read();
    // render mode without subtitles: no caption band, the stage fills the video
    let sub_lang_empty = state.sub_lang.read().is_empty();
    let sidebar_open = (sidebar_open && !render) || matches!(focus, Some(iwr_core::script::Panel::Files | iwr_core::script::Panel::Items)) || spot == Some("sidebar-tabs");
    let welcome_open = *state.welcome_open.read();
    let loaded = state.project.read().is_some();
    rsx! {
        document::Link { rel: "icon", r#type: "image/png", href: icons::uri("favicon") }
        style { {theme::CSS} }
        style { {chart::CSS} }
        style { {castmap::CSS} }
        style { {fonts::faces()} }
        style { {theme::custom_css(&state.font_title.read(), &state.font_text.read(), &state.config.read(), &state.config_base.read())} }
        div { class: format!("app theme-{}{}{}{}{}", theme_name, if dragging { " dragging" } else { "" }, if spot.is_some() && has_guide { " spotting" } else { "" }, if render { " render" } else { "" }, if render && sub_lang_empty { " nocaption" } else { "" }),
            panels::TopBar {}
            if !loaded {
                loading::LoadingPage {}
            } else if settings_open {
                settings::SettingsPage {}
            } else if generate_open {
                generate::GeneratePage {}
            } else {
            div { class: "main{focus_cls}",
                onmousemove: move |e| {
                    if let Some((which, x0, v0)) = *state.drag.read() {
                        let x = e.data().client_coordinates().x;
                        match which {
                            0 => { let ww = *state.win_w.read(); state.sidebar_w.set(clamp_sidebar(v0 + (x - x0) / ww)) }
                            1 => { let ww = *state.win_w.read(); state.details_w.set(clamp_details(v0 - (x - x0) / ww)) }
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
                } else if let Some(c) = chart.clone() {
                    match c {
                        Some(c) => rsx! { chart::ChartView { chart: (*c).clone(), full: true } },
                        None => rsx! { div { class: "chart full", {t::app::no_chart()} } },
                    }
                } else {
                    canvas::Canvas {}
                    // the side panel takes the details' place; a codecast's own diagram has no
                    // item to detail, so without a side panel it gets the whole width (like a chart);
                    // same for an empty details panel during a codecast
                    if has_side || (!is_diagram && !details_empty) {
                        div { class: "resizer", onmousedown: move |e| { e.prevent_default(); state.drag.set(Some((1, e.data().client_coordinates().x, *state.details_w.read()))); } }
                    }
                    if has_side { side::SidePanel {} } else if !is_diagram && !details_empty { panels::Details {} }
                }
                if has_side && (is_code || is_plan || chart.is_some()) {
                    div { class: "resizer", onmousedown: move |e| { e.prevent_default(); state.drag.set(Some((1, e.data().client_coordinates().x, *state.details_w.read()))); } }
                    side::SidePanel {}
                }
            }
            }
            if has_guide {
                player::PlayerBar {}
            }
            // the first-start dialog waits for the project (the loading page shows meanwhile)
            if welcome_open && loaded {
                tutorial::Welcome {}
            }
            if *state.map_open.read() {
                castmap::CastMapPage {}
            }
            if let Some(name) = spot.filter(|_| has_guide) {
                tutorial::Spot { key: "{name}", name: name.to_string() }
            }
            // a graph frames its highlight with the camera (`State::camera`), the rest through `Zoom`
            if let Some(on) = state.zoom.read().filter(|on| has_guide && !settings_open && !generate_open && !(on.is_empty() && is_graph)) {
                tutorial::Zoom { key: "z-{on}", on: on.to_string() }
            }
        }
    }
}
