//! Settings page: theme, analysis view toggles, code view, codecast voice and tour,
//! layout, keyboard reference. Everything here is remembered in `localStorage`.

use crate::icons::Icon;
use crate::player::Voice;
use crate::theme::THEMES;
use crate::State;
use dioxus::prelude::*;
use iwr_core::config::Config;
use iwr_core::model::ItemKind;

/// Kinds the Structure view can hide, in display order.
pub const KINDS: [ItemKind; 9] = [ItemKind::Function, ItemKind::Method, ItemKind::Struct, ItemKind::Enum, ItemKind::Trait, ItemKind::TypeAlias, ItemKind::Const, ItemKind::Static, ItemKind::Impl];

fn kind_from_label(l: &str) -> Option<ItemKind> {
    KINDS.iter().copied().find(|k| k.label() == l)
}

/// Settings that survive a reload (besides the theme and the panel sizes, stored separately).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Saved {
    pub external: bool,
    pub macros: bool,
    pub constructs: bool,
    pub tests: bool,
    pub depth: usize,
    pub voice: String,
    pub rate: f64,
    pub read_delay: f64,
    pub tour: bool,
    pub show_source: bool,
    /// labels of the item kinds hidden in the Structure view
    #[serde(default)]
    pub hidden_kinds: Vec<String>,
    /// offer questions from the built-in Rust glossary
    #[serde(default = "yes")]
    pub glossary: bool,
    /// caption shows how code is spoken, in parentheses
    #[serde(default)]
    pub speech_parens: bool,
    /// language the codecast plays in
    #[serde(default = "en")]
    pub lang: String,
    /// font of the box titles: a bundled font's id, `""` = the theme's
    #[serde(default)]
    pub title_font: String,
    /// font of the interface text
    #[serde(default)]
    pub text_font: String,
    /// the recorded voice to play, when a codecast has several
    #[serde(default)]
    pub rec_voice: String,
    /// browser voices picked by name: the main one, the one of the `sp:` lines
    #[serde(default)]
    pub tts_voice: String,
    #[serde(default)]
    pub tts_voice2: String,
}

async fn fetch_config(url: &str) -> Option<Config> {
    let r = gloo_net::http::Request::get(url).send().await.ok()?;
    if !r.ok() {
        return None;
    }
    // a static server answers unknown paths with the page itself: not JSON, ignored
    r.json::<Config>().await.ok()
}

fn en() -> String {
    "en".into()
}

fn yes() -> bool {
    true
}

impl State {
    pub fn save_settings(&self) {
        let f = *self.flags.read();
        let s = Saved {
            external: f.external,
            macros: f.macros,
            constructs: f.constructs,
            tests: f.tests,
            depth: *self.depth.read(),
            voice: match *self.voice.read() { Voice::Read => "read".into(), _ => "tts".into() },
            rate: *self.tts_rate.read(),
            read_delay: *self.read_delay.read(),
            tour: *self.tour.read(),
            show_source: *self.show_source.read(),
            hidden_kinds: KINDS.iter().filter(|k| self.hidden_kinds.read().contains(k)).map(|k| k.label().to_string()).collect(),
            glossary: *self.glossary_on.read(),
            speech_parens: *self.speech_parens.read(),
            lang: self.lang.read().clone(),
            title_font: self.font_title.read().clone(),
            text_font: self.font_text.read().clone(),
            rec_voice: self.rec_voice.read().clone(),
            tts_voice: self.tts_voice.read().clone(),
            tts_voice2: self.tts_voice2.read().clone(),
        };
        if let Ok(json) = serde_json::to_string(&s) {
            document::eval(&format!("localStorage.setItem('iwr-settings', {});", serde_json::to_string(&json).unwrap_or_default()));
        }
    }

    pub fn apply_settings(&mut self, s: Saved) {
        self.flags.set(crate::Flags { external: s.external, macros: s.macros, tests: s.tests, constructs: s.constructs });
        self.depth.set(s.depth.clamp(1, 12));
        if s.voice == "read" {
            self.voice.set(Voice::Read);
        }
        self.tts_rate.set(s.rate.clamp(0.5, 2.0));
        self.read_delay.set(s.read_delay.clamp(1.0, 15.0));
        self.tour.set(s.tour);
        self.show_source.set(s.show_source);
        self.hidden_kinds.set(s.hidden_kinds.iter().filter_map(|l| kind_from_label(l)).collect());
        self.glossary_on.set(s.glossary);
        self.speech_parens.set(s.speech_parens);
        if iwr_core::i18n::lang(&s.lang).is_some() {
            self.set_lang(&s.lang);
        }
        self.font_title.set(s.title_font);
        self.font_text.set(s.text_font);
        self.rec_voice.set(s.rec_voice);
        self.tts_voice.set(s.tts_voice);
        self.tts_voice2.set(s.tts_voice2);
    }

    /// Fetch the codecast's `codecast.toml` (as JSON: `/api/config` from `iwr serve`, `codecast.json`
    /// in an export) and apply it.
    pub async fn load_config(&mut self) {
        let (c, base) = match fetch_config("/api/config").await {
            Some(c) => (c, "/api/codecast/"),
            None => match fetch_config("codecast.json").await {
                Some(c) => (c, "codecast/"),
                None => return,
            },
        };
        let prev = match document::eval("return localStorage.getItem('iwr-config') || '';").await {
            Ok(v) => v.as_str().and_then(|t| serde_json::from_str::<Config>(t).ok()),
            Err(_) => None,
        };
        if let Ok(json) = serde_json::to_string(&c) {
            document::eval(&format!("localStorage.setItem('iwr-config', {});", serde_json::to_string(&json).unwrap_or_default()));
        }
        self.config_base.set(base.into());
        self.apply_config(c, prev);
    }

    /// Apply the author's settings. A value is applied when it is locked, new, or changed since the
    /// last time this browser saw the config; otherwise the viewer's own choice stays.
    pub fn apply_config(&mut self, c: Config, prev: Option<Config>) {
        let lock = c.lock;
        let fresh = |new: bool, same: bool| new && (lock || !same);
        let p = prev.unwrap_or_default();
        if let Some(t) = c.theme.as_deref().filter(|t| crate::theme::is_theme(t)) {
            if fresh(true, p.theme == c.theme) {
                self.set_theme(t);
            }
        }
        if let Some(l) = c.lang.as_deref().filter(|l| iwr_core::i18n::lang(l).is_some()) {
            if fresh(true, p.lang == c.lang) {
                self.set_lang(l);
            }
        }
        if fresh(c.voice.is_some(), p.voice == c.voice) {
            match c.voice.as_deref() {
                Some("read") => self.voice.set(Voice::Read),
                Some("tts") => self.voice.set(Voice::Tts),
                // without a recording (none, or not loaded yet) `load_script` switches later
                Some("audio") if self.has_recording() => self.voice.set(Voice::Audio),
                _ => {}
            }
        }
        if let Some(r) = c.rate.filter(|_| fresh(true, p.rate == c.rate)) {
            self.tts_rate.set(r.clamp(0.5, 2.0));
        }
        if let Some(t) = c.tour.filter(|_| fresh(true, p.tour == c.tour)) {
            self.tour.set(t);
        }
        if let Some(g) = c.glossary.filter(|_| fresh(true, p.glossary == c.glossary)) {
            self.glossary_on.set(g);
        }
        if let Some(v) = c.tts_voice.clone().filter(|_| fresh(true, p.tts_voice == c.tts_voice)) {
            self.tts_voice.set(v);
        }
        if let Some(v) = c.tts_voice2.clone().filter(|_| fresh(true, p.tts_voice2 == c.tts_voice2)) {
            self.tts_voice2.set(v);
        }
        if let Some(v) = c.rec_voice.clone().filter(|_| fresh(true, p.rec_voice == c.rec_voice)) {
            // a recording playing already switches to it
            if self.script.peek().is_some() && *self.voice.peek() == Voice::Audio && *self.rec_voice.peek() != v {
                self.set_rec_voice(&v);
            } else {
                self.rec_voice.set(v);
            }
        }
        if let Some(f) = c.fonts.title.clone().filter(|_| fresh(true, p.fonts.title == c.fonts.title)) {
            self.font_title.set(f);
        }
        if let Some(f) = c.fonts.text.clone().filter(|_| fresh(true, p.fonts.text == c.fonts.text)) {
            self.font_text.set(f);
        }
        self.config.set(std::rc::Rc::new(c));
        self.save_settings();
    }

    /// Does the codecast fix theme, fonts, language and voice (`lock = true`)?
    pub fn locked(&self) -> bool {
        self.config.read().lock
    }

    pub fn toggle_kind(&mut self, k: ItemKind) {
        let mut h = self.hidden_kinds.write();
        if !h.remove(&k) {
            h.insert(k);
        }
        drop(h);
        self.save_settings();
    }

    pub fn reset_layout(&mut self) {
        self.sidebar_w.set(250.0);
        self.details_w.set(380.0);
        self.split.set(0.5);
        self.sidebar_open.set(true);
        self.save_layout();
    }
}

/// One checkbox chip per item kind: ticked = shown in the Structure view. `all` adds a reset.
#[component]
pub fn KindChips(compact: bool) -> Element {
    let mut state = use_context::<State>();
    let hidden = state.hidden_kinds.read().clone();
    let any_hidden = !hidden.is_empty();
    rsx! {
        span { class: if compact { "kinds compact" } else { "kinds" }, title: "which kinds of items the Structure view shows",
            for k in KINDS.iter().copied() {
                label { class: if hidden.contains(&k) { "kind off" } else { "kind" },
                    input { r#type: "checkbox", checked: !hidden.contains(&k), onchange: move |_| state.toggle_kind(k) }
                    "{k.label()}"
                }
            }
            if any_hidden {
                button { class: "small", title: "show every kind", onclick: move |_| { state.hidden_kinds.set(Default::default()); state.save_settings(); }, "all" }
            }
            button { class: "small", title: "functions and methods only", onclick: move |_| { state.hidden_kinds.set(KINDS.iter().copied().filter(|k| !matches!(k, ItemKind::Function | ItemKind::Method)).collect()); state.save_settings(); }, "fns only" }
        }
    }
}

/// A setting: its name and one line about it on the left, the control on the right.
#[component]
pub fn Row(title: String, #[props(default)] desc: String, children: Element) -> Element {
    rsx! {
        div { class: "srow",
            div { class: "sl",
                div { class: "st", "{title}" }
                if !desc.is_empty() {
                    div { class: "sd", "{desc}" }
                }
            }
            div { class: "sc", {children} }
        }
    }
}

/// An on/off switch.
#[component]
pub fn Toggle(checked: bool, #[props(default = false)] disabled: bool, onchange: EventHandler<bool>) -> Element {
    rsx! {
        label { class: if disabled { "switch off" } else { "switch" },
            input { r#type: "checkbox", checked, disabled, onchange: move |e| onchange.call(e.checked()) }
            span { class: "knob" }
        }
    }
}

/// A card per theme: a small picture of the interface in its colours.
#[component]
pub fn ThemeGrid(value: String, #[props(default = false)] locked: bool, #[props(default = false)] allow_none: bool, onpick: EventHandler<String>) -> Element {
    let mut filter = use_signal(|| 0u8); // 0 all, 1 dark, 2 light
    let f = *filter.read();
    rsx! {
        div { class: "chips",
            for (i, l) in [(0u8, "All"), (1, "Dark"), (2, "Light")] {
                button { class: if f == i { "chip-btn sel" } else { "chip-btn" }, onclick: move |_| filter.set(i), "{l}" }
            }
            span { class: "count", "{THEMES.len()} themes" }
        }
        div { class: if locked { "themes locked" } else { "themes" },
            if allow_none {
                div { class: if value.is_empty() { "tcard none sel" } else { "tcard none" }, onclick: move |_| if !locked { onpick.call(String::new()) },
                    div { class: "mock", div { class: "none-text", "viewer's choice" } }
                    div { class: "name", span { "None" } }
                }
            }
            for (id, label, dark) in THEMES.iter().copied().filter(|t| f == 0 || (f == 1) == t.2) {
                div { class: format!("tcard theme-{}{}", id, if value == id { " sel" } else { "" }), title: "{label}", onclick: move |_| if !locked { onpick.call(id.to_string()) },
                    div { class: "mock",
                        div { class: "mbar", i { class: "dot" } i { class: "pill" } i { class: "pill short" } }
                        div { class: "mgraph",
                            div { class: "mbox a" }
                            div { class: "mline" }
                            div { class: "mbox b" }
                        }
                        div { class: "mcode", i { class: "c1" } i { class: "c2" } i { class: "c3" } }
                    }
                    div { class: "name",
                        span { "{label}" }
                        Icon { name: if dark { "dark".to_string() } else { "light".to_string() }, size: 12 }
                    }
                }
            }
        }
    }
}

/// A card per bundled font, drawn in that font; `""` = the theme's own font.
#[component]
pub fn FontGrid(value: String, sample: String, #[props(default = false)] locked: bool, #[props(default = "the theme's")] none_label: &'static str, onpick: EventHandler<String>) -> Element {
    use iwr_core::config::{FontKind, FONTS};
    let mut kind = use_signal(|| None::<FontKind>);
    let k = *kind.read();
    let custom = !value.is_empty() && iwr_core::config::font(&value).is_none();
    rsx! {
        div { class: "chips",
            button { class: if k.is_none() { "chip-btn sel" } else { "chip-btn" }, onclick: move |_| kind.set(None), "All" }
            for fk in [FontKind::Display, FontKind::Script, FontKind::Text] {
                button { class: if k == Some(fk) { "chip-btn sel" } else { "chip-btn" }, onclick: move |_| kind.set(Some(fk)), "{fk.label()}" }
            }
            span { class: "count", "{FONTS.len()} fonts" }
        }
        div { class: if locked { "fonts locked" } else { "fonts" },
            div { class: if value.is_empty() { "fcard sel" } else { "fcard" }, onclick: move |_| if !locked { onpick.call(String::new()) },
                div { class: "aa", "Aa" }
                div { class: "fname", "Default" }
                div { class: "fsample", "{none_label} font" }
            }
            if custom {
                div { class: "fcard sel",
                    div { class: "aa", "Aa" }
                    div { class: "fname", "Custom" }
                    div { class: "fsample", "{value}" }
                }
            }
            for f in FONTS.iter().filter(|f| k.is_none() || Some(f.kind) == k) {
                { let id = f.id; let fam = crate::fonts::family(id); rsx! {
                    div { class: if value == id { "fcard sel" } else { "fcard" }, style: "font-family:\"{fam}\", system-ui", title: "{f.name} · {f.kind.label()}", onclick: move |_| if !locked { onpick.call(id.to_string()) },
                        div { class: "aa", "Aa" }
                        div { class: "fname", "{f.name}" }
                        div { class: "fsample", "{sample}" }
                    }
                } }
            }
        }
    }
}

/// Sections of the page: (anchor, icon, title).
const SECTIONS: [(&str, &str, &str); 7] = [
    ("look", "palette", "Appearance"),
    ("cast", "codecast", "Codecast"),
    ("design", "tour", "Codecast design"),
    ("views", "calls", "Analysis views"),
    ("code", "code", "Code view"),
    ("layout", "panel", "Layout"),
    ("keys", "info", "Keyboard"),
];

#[component]
fn Head(icon: &'static str, title: &'static str, sub: String) -> Element {
    rsx! {
        div { class: "chead",
            span { class: "cicon", Icon { name: icon, size: 18 } }
            div {
                h3 { "{title}" }
                div { class: "csub", "{sub}" }
            }
        }
    }
}

#[component]
pub fn SettingsPage() -> Element {
    let mut state = use_context::<State>();
    let theme = state.theme.read().clone();
    let flags = *state.flags.read();
    let depth = *state.depth.read();
    let voice = *state.voice.read();
    let rate = *state.tts_rate.read();
    let delay = *state.read_delay.read();
    let tour = *state.tour.read();
    let show_source = *state.show_source.read();
    let sidebar_open = *state.sidebar_open.read();
    let has_script = state.script.read().is_some();
    let glossary_on = *state.glossary_on.read();
    let speech_parens = *state.speech_parens.read();
    let lang = state.lang.read().clone();
    let n_terms = state.glossary.read().len();
    let n_asked = state.asked.read().len();
    let locked = state.locked();
    // the browser voices of the language, for the two pickers
    let mut tts_list = use_signal(Vec::<(String, String)>::new);
    use_effect(move || {
        let lang = state.lang.read().clone();
        spawn(async move { tts_list.set(crate::player::browser_voices(&lang).await) });
    });
    let (own1, own2) = (state.tts_voice.read().clone(), state.tts_voice2.read().clone());
    let cfg = state.config.read().clone();
    let auto = |c: &Option<String>| match c {
        Some(n) => format!("the codecast's ({n})"),
        None => "automatic".to_string(),
    };
    let (auto1, auto2) = (auto(&cfg.tts_voice), auto(&cfg.tts_voice2));
    // recorded voices of the codecast, in this language
    let recorded = state.script_src.read().as_ref().map(|s| iwr_core::i18n::recorded_voices(s, &lang)).unwrap_or_default();
    let rec_now = state.script_src.read().as_ref().map(|s| state.recording_voice(s, &lang)).unwrap_or_default();
    let has_second = state.script_src.read().as_ref().is_some_and(|s| s.has_second_voice());
    let font_title = state.font_title.read().clone();
    let font_text = state.font_text.read().clone();
    let mut font_role = use_signal(|| 0u8); // 0 box titles, 1 text
    let role = *font_role.read();
    let mut active = use_signal(|| "look");
    // the side menu follows the section in view
    use_future(move || async move {
        let mut ev = document::eval(
            r#"const seen = new Map();
               const io = new IntersectionObserver(es => {
                 es.forEach(e => seen.set(e.target.id, e.isIntersecting ? e.boundingClientRect.top : null));
                 const top = [...seen].filter(([, t]) => t !== null).sort((a, b) => a[1] - b[1])[0];
                 if (top) dioxus.send(top[0].slice(2));
               }, { root: document.querySelector(".settings"), rootMargin: "0px 0px -55% 0px" });
               document.querySelectorAll(".settings section[id^='s-']").forEach(s => io.observe(s));
               await new Promise(() => {});"#,
        );
        while let Ok(id) = ev.recv::<String>().await {
            if let Some((s, _, _)) = SECTIONS.iter().find(|(s, _, _)| *s == id) {
                active.set(s);
            }
        }
    });
    let mut set_flag = move |f: fn(&mut crate::Flags, bool), v: bool| {
        let mut fl = *state.flags.read();
        f(&mut fl, v);
        state.flags.set(fl);
        state.save_settings();
    };
    rsx! {
        div { class: "settings",
            div { class: "shell",
                nav { class: "snav",
                    div { class: "sbrand", crate::icons::Logo { size: 24 } span { "Settings" } }
                    for (id, icon, label) in SECTIONS {
                        button { class: if *active.read() == id { "snav-item sel" } else { "snav-item" },
                            onclick: move |_| {
                                active.set(id);
                                document::eval(&format!("document.getElementById('s-{id}')?.scrollIntoView({{behavior:'smooth', block:'start'}});"));
                            },
                            Icon { name: icon, size: 15 }
                            span { "{label}" }
                        }
                    }
                    div { class: "spacer" }
                    button { class: "snav-back", title: "Back to the code (Esc)", onclick: move |_| state.close_pages(), Icon { name: "back", size: 15 } span { "Back" } }
                }
                div { class: "scontent",
                    div { class: "shero",
                        h2 { "Make it yours" }
                        p { "Colours, fonts, voice and views. Everything is remembered in this browser." }
                        if locked {
                            div { class: "lockline", "🔒 This codecast sets its own theme, fonts, language and voice." }
                        }
                    }

                    section { id: "s-look", class: "card",
                        Head { icon: "palette", title: "Appearance", sub: "The colours of the whole interface, graphs included, and the fonts.".to_string() }
                        h4 { "Theme" }
                        ThemeGrid { value: theme.clone(), locked, onpick: move |id: String| state.set_theme(&id) }
                        h4 { "Fonts" }
                        div { class: "seg",
                            button { class: if role == 0 { "sel" } else { "" }, onclick: move |_| font_role.set(0), "Box titles" }
                            button { class: if role == 1 { "sel" } else { "" }, onclick: move |_| font_role.set(1), "Interface text" }
                        }
                        if role == 0 {
                            FontGrid { value: font_title.clone(), sample: "Parse the task list".to_string(), locked, onpick: move |v: String| { state.font_title.set(v); state.save_settings(); } }
                        } else {
                            FontGrid { value: font_text.clone(), sample: "The scheduler picks the heaviest task".to_string(), locked, onpick: move |v: String| { state.font_text.set(v); state.save_settings(); } }
                        }
                    }

                    section { id: "s-cast", class: "card",
                        Head { icon: "codecast", title: "Codecast", sub: "How the spoken tour plays.".to_string() }
                        Row { title: "Voice", desc: if locked { "set by this codecast".to_string() } else { "a recording, when the codecast ships one, is picked automatically".to_string() },
                            select { class: "theme-select", disabled: locked, value: if voice == Voice::Read { "read" } else { "tts" },
                                onchange: move |e| { state.tts_gen += 1; state.playing.set(false); state.voice.set(if e.value() == "read" { Voice::Read } else { Voice::Tts }); state.save_settings(); },
                                option { value: "tts", "🔊 spoken by the browser" }
                                option { value: "read", "📖 read silently, advance on a timer" }
                            }
                        }
                        for second in [false, true] {
                            { let (title, desc, own, auto) = if second {
                                  ("Second browser voice", if has_second { "says the sp: lines and asks the questions you click" } else { "asks the questions you click (and says the sp: lines, in codecasts that have them)" }, own2.clone(), auto2.clone())
                              } else {
                                  ("Browser voice", "speaks the codecast when there is no recording", own1.clone(), auto1.clone())
                              };
                              let list = tts_list.read().clone();
                              let lang = lang.clone();
                              rsx! {
                                Row { title: title, desc: desc.to_string(),
                                    select { class: "theme-select", disabled: locked, value: "{own}",
                                        onchange: move |e| {
                                            let v = e.value();
                                            if second { state.tts_voice2.set(v); } else { state.tts_voice.set(v); }
                                            state.save_settings();
                                            state.tts_gen += 1;
                                        },
                                        option { value: "", "{auto}" }
                                        for (name, l) in list.iter().cloned() {
                                            option { value: "{name}", selected: name == own, "{name} ({l})" }
                                        }
                                    }
                                    button { class: "small", title: "hear it", onclick: move |_| crate::player::speak_sample(&lang, &state.tts_names(), *state.tts_rate.peek(), second), "▶" }
                                }
                            } }
                        }
                        if recorded.len() > 1 || recorded.first().is_some_and(|(v, _)| !v.is_empty()) {
                            Row { title: "Recorded voice", desc: "the recording that plays; a pair (A & B) is a codecast with two voices".to_string(),
                                select { class: "theme-select", disabled: locked, value: "{rec_now}",
                                    onchange: move |e| state.set_rec_voice(&e.value()),
                                    for (id, n) in recorded.iter().cloned() {
                                        option { value: "{id}", selected: id == rec_now,
                                            if id.is_empty() { "Original ({n} part{plural(n)})" } else { "{iwr_core::voice::voice_label(&id).0} ({n} part{plural(n)})" } }
                                    }
                                }
                            }
                        }
                        Row { title: "Language", desc: "translated when played, offline (iwr serve), spoken by a browser voice of that language; recordings and code stay English. ⚠ The offline translation is rough for now: for a good one, translate with another tool (iwr translate --pieces, see the docs)".to_string(),
                            select { class: "theme-select", disabled: locked, value: "{lang}",
                                onchange: move |e| { state.set_lang(&e.value()); state.save_settings(); },
                                for l in iwr_core::i18n::LANGS {
                                    option { value: l.code, "{l.name}" }
                                }
                            }
                        }
                        Row { title: "Speech rate",
                            input { r#type: "range", min: "0.6", max: "1.6", step: "0.1", value: "{rate}", oninput: move |e| { if let Ok(v) = e.value().parse::<f64>() { state.tts_rate.set(v); state.save_settings(); } } }
                            span { class: "val", "{rate:.1}×" }
                        }
                        Row { title: "Reading time per cue", desc: "silent mode only".to_string(),
                            input { r#type: "range", min: "2", max: "12", step: "1", value: "{delay}", oninput: move |e| { if let Ok(v) = e.value().parse::<f64>() { state.read_delay.set(v); state.save_settings(); } } }
                            span { class: "val", "{delay:.0} s" }
                        }
                        Row { title: "Full tour", desc: "when a part ends, continue with the next one".to_string(),
                            Toggle { checked: tour, onchange: move |v| { state.tour.set(v); state.save_settings(); } }
                        }
                        Row { title: "Show how code is spoken", desc: "\"crate::parser::parse_all\" → \"parse all\" in parentheses; hover a dotted token to hear it. Fix a word with `pronounce: written = spoken`.".to_string(),
                            Toggle { checked: speech_parens, onchange: move |v| { state.speech_parens.set(v); state.save_settings(); } }
                        }
                        Row { title: "Glossary questions", desc: format!("offer \"what is a…?\" when a cue says a Rust term ({n_terms} terms, each once); script questions always show"),
                            if n_asked > 0 {
                                button { class: "small", onclick: move |_| state.asked.set(vec![]), "forget the {n_asked} answered" }
                            }
                            Toggle { checked: glossary_on, onchange: move |v| { state.glossary_on.set(v); state.save_settings(); } }
                        }
                        div { class: "cta",
                            button { class: "big", onclick: move |_| { state.close_pages(); state.start_tour(); }, Icon { name: "tour", size: 16 } "Start the full tour" }
                            span { class: "sd", if has_script { "the loaded codecast, from its first part" } else { "no script loaded: the built-in tour of the project" } }
                        }
                    }

                    crate::design::DesignEditor {}

                    section { id: "s-views", class: "card",
                        Head { icon: "calls", title: "Analysis views", sub: "What the graph views include. Off by default to keep the diagrams small.".to_string() }
                        Row { title: "External crates and calls", Toggle { checked: flags.external, onchange: move |v| set_flag(|f, v| f.external = v, v) } }
                        Row { title: "Macro invocations", Toggle { checked: flags.macros, onchange: move |v| set_flag(|f, v| f.macros = v, v) } }
                        Row { title: "Constructors", Toggle { checked: flags.constructs, onchange: move |v| set_flag(|f, v| f.constructs = v, v) } }
                        Row { title: "Tests", Toggle { checked: flags.tests, onchange: move |v| set_flag(|f, v| f.tests = v, v) } }
                        Row { title: "Call tree depth", desc: "levels opened from the root function".to_string(),
                            div { class: "stepper",
                                button { onclick: move |_| { let d = *state.depth.read(); state.depth.set(d.saturating_sub(1).max(1)); state.save_settings(); }, "−" }
                                span { "{depth}" }
                                button { onclick: move |_| { let d = *state.depth.read(); state.depth.set((d + 1).min(12)); state.save_settings(); }, "+" }
                            }
                        }
                        div { class: "srow block",
                            div { class: "sl", div { class: "st", "Structure shows" } div { class: "sd", "untick a kind to hide it; hiding impl puts the methods in their module" } }
                            KindChips { compact: false }
                        }
                    }

                    section { id: "s-code", class: "card",
                        Head { icon: "code", title: "Code view", sub: "The blocks of a function, and its source.".to_string() }
                        Row { title: "Show the source next to the blocks", desc: "hover either side to highlight both".to_string(),
                            Toggle { checked: show_source, onchange: move |v| { state.show_source.set(v); state.save_settings(); } }
                        }
                    }

                    section { id: "s-layout", class: "card",
                        Head { icon: "panel", title: "Layout", sub: "Panel edges are draggable; sizes are remembered.".to_string() }
                        Row { title: "Files / items panel", Toggle { checked: sidebar_open, onchange: move |v| { state.sidebar_open.set(v); state.save_layout(); } } }
                        Row { title: "Panel sizes", button { class: "small", onclick: move |_| state.reset_layout(), "reset" } }
                    }

                    section { id: "s-keys", class: "card",
                        Head { icon: "info", title: "Keyboard", sub: "Shortcuts everywhere outside text fields.".to_string() }
                        div { class: "keys",
                            for (k, what) in [("← →", "previous / next cue"), ("space", "play / pause"), ("g", "start / stop the codecast"), ("t", "start the full tour"), ("f", "fit the graph to the view"), (",", "open / close this page"), ("Esc", "close the player or this page")] {
                                div { class: "key",
                                    span { for part in k.split(' ') { span { class: "kbd", "{part}" } } }
                                    span { "{what}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// `s` after a count other than one.
fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}
