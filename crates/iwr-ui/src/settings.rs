//! Settings page: theme, analysis view toggles, code view, codecast voice and tour,
//! layout, keyboard reference. Everything here is remembered in `localStorage`.

use crate::icons::{Icon, IconButton};
use crate::player::Voice;
use crate::theme::THEMES;
use crate::State;
use dioxus::prelude::*;

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
    }

    pub fn reset_layout(&mut self) {
        self.sidebar_w.set(250.0);
        self.details_w.set(380.0);
        self.split.set(0.5);
        self.sidebar_open.set(true);
        self.save_layout();
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
    rsx! {
        div { class: "settings",
            div { class: "wrap",
                h2 {
                    crate::icons::Logo { size: 26 }
                    "Settings"
                    div { class: "spacer" }
                    IconButton { name: "back", tip: "Back to the code (Esc)".to_string(), text: "back", onclick: move |_| state.settings_open.set(false) }
                }

                section {
                    h3 { Icon { name: "palette" } "Theme" }
                    div { class: "hint", "Colours of the whole interface, graphs included. Remembered in this browser." }
                    div { class: "themes",
                        for (id, label, dark) in THEMES.iter().copied() {
                            div { class: format!("tcard theme-{}{}", id, if theme == id { " sel" } else { "" }), onclick: move |_| state.set_theme(id),
                                div { class: "prev",
                                    i { style: "background:var(--accent)" }
                                    i { style: "background:var(--hi)" }
                                    i { style: "background:var(--code)" }
                                    i { style: "background:var(--panel3)" }
                                }
                                div { class: "name",
                                    span { "{label}" }
                                    Icon { name: if dark { "dark".to_string() } else { "light".to_string() }, size: 13 }
                                }
                            }
                        }
                    }
                }

                section {
                    h3 { Icon { name: "calls" } "Analysis views" }
                    div { class: "hint", "What the graph views include. Off by default to keep the diagrams small." }
                    div { class: "row",
                        label { class: "opt", input { r#type: "checkbox", checked: flags.external, onchange: move |e| { let mut f = *state.flags.read(); f.external = e.checked(); state.flags.set(f); state.save_settings(); } } "external crates and calls" }
                        label { class: "opt", input { r#type: "checkbox", checked: flags.macros, onchange: move |e| { let mut f = *state.flags.read(); f.macros = e.checked(); state.flags.set(f); state.save_settings(); } } "macro invocations" }
                        label { class: "opt", input { r#type: "checkbox", checked: flags.constructs, onchange: move |e| { let mut f = *state.flags.read(); f.constructs = e.checked(); state.flags.set(f); state.save_settings(); } } "constructors" }
                        label { class: "opt", input { r#type: "checkbox", checked: flags.tests, onchange: move |e| { let mut f = *state.flags.read(); f.tests = e.checked(); state.flags.set(f); state.save_settings(); } } "test functions" }
                    }
                    div { class: "row",
                        span { "Call tree depth" }
                        button { class: "small", onclick: move |_| { let d = *state.depth.read(); state.depth.set(d.saturating_sub(1).max(1)); state.save_settings(); }, "−" }
                        span { class: "val", "{depth}" }
                        button { class: "small", onclick: move |_| { let d = *state.depth.read(); state.depth.set((d + 1).min(12)); state.save_settings(); }, "+" }
                        span { class: "hint", "levels opened from the root function" }
                    }
                }

                section {
                    h3 { Icon { name: "code" } "Code view" }
                    div { class: "row",
                        label { class: "opt", input { r#type: "checkbox", checked: show_source, onchange: move |e| { state.show_source.set(e.checked()); state.save_settings(); } } "show the source next to the blocks (hover either side to highlight both)" }
                    }
                }

                section {
                    h3 { Icon { name: "codecast" } "Codecast" }
                    div { class: "row",
                        span { "Voice" }
                        select { class: "theme-select", value: if voice == Voice::Read { "read" } else { "tts" },
                            onchange: move |e| { state.tts_gen += 1; state.playing.set(false); state.voice.set(if e.value() == "read" { Voice::Read } else { Voice::Tts }); state.save_settings(); },
                            option { value: "tts", "🔊 spoken by the browser" }
                            option { value: "read", "📖 read silently, advance on a timer" }
                        }
                        span { class: "hint", "a recording, when a script ships one, is picked automatically" }
                    }
                    div { class: "row",
                        span { "Speech rate" }
                        input { r#type: "range", min: "0.6", max: "1.6", step: "0.1", value: "{rate}", oninput: move |e| { if let Ok(v) = e.value().parse::<f64>() { state.tts_rate.set(v); state.save_settings(); } } }
                        span { class: "val", "{rate:.1}×" }
                    }
                    div { class: "row",
                        span { "Reading time per cue" }
                        input { r#type: "range", min: "2", max: "12", step: "1", value: "{delay}", oninput: move |e| { if let Ok(v) = e.value().parse::<f64>() { state.read_delay.set(v); state.save_settings(); } } }
                        span { class: "val", "{delay:.0} s" }
                        span { class: "hint", "silent mode only" }
                    }
                    div { class: "row",
                        label { class: "opt", input { r#type: "checkbox", checked: tour, onchange: move |e| { state.tour.set(e.checked()); state.save_settings(); } } "full tour: when a part ends, continue with the next one" }
                    }
                    div { class: "row",
                        IconButton { name: "tour", tip: "Play every part of the codecast in a row (t)".to_string(), text: "start the full tour", onclick: move |_| { state.settings_open.set(false); state.start_tour(); } }
                        if has_script {
                            span { class: "hint", "the loaded script will be played from its first part" }
                        } else {
                            span { class: "hint", "no script loaded: the built-in tour of the project will be played" }
                        }
                    }
                }

                section {
                    h3 { Icon { name: "panel" } "Layout" }
                    div { class: "row",
                        label { class: "opt", input { r#type: "checkbox", checked: sidebar_open, onchange: move |e| { state.sidebar_open.set(e.checked()); state.save_layout(); } } "show the files / items panel" }
                        button { class: "small", onclick: move |_| state.reset_layout(), "reset panel sizes" }
                        span { class: "hint", "panel edges are draggable; sizes are remembered" }
                    }
                }

                section {
                    h3 { Icon { name: "info" } "Keyboard" }
                    table { class: "keys",
                        tr { td { span { class: "kbd", "←" } " " span { class: "kbd", "→" } } td { "previous / next cue" } }
                        tr { td { span { class: "kbd", "space" } } td { "play / pause" } }
                        tr { td { span { class: "kbd", "g" } } td { "start / stop the codecast" } }
                        tr { td { span { class: "kbd", "t" } } td { "start the full tour" } }
                        tr { td { span { class: "kbd", "f" } } td { "fit the graph to the view" } }
                        tr { td { span { class: "kbd", "," } } td { "open / close this page" } }
                        tr { td { span { class: "kbd", "Esc" } } td { "close the player or this page" } }
                    }
                }
            }
        }
    }
}
