//! Subtitles: the caption again, in a second language, under the one that plays (a talk in
//! Japanese followed in English, or the other way round).
//!
//! Only text that already exists is shown, never a translation made on the spot: the English
//! source, the author's `translations/<lang>.json` and the translation cache behind
//! `codecast serve` (`/api/subtitles/LANG`, which never runs the model), or the map a static
//! export ships (`translations/<lang>.json`). A language with none is offered greyed out, with
//! what to run to make it ([`how_to_make`]).

use crate::i18n::t;
use crate::State;
use dioxus::prelude::*;
use std::collections::HashMap;
use std::rc::Rc;

/// How much of the codecast's text exists in one language.
#[derive(Clone, Debug, PartialEq)]
pub struct Avail {
    pub code: &'static str,
    /// pieces (cue lines, part names, notes…) that have a translation
    pub have: usize,
    pub total: usize,
}

impl Avail {
    /// Some text exists: the language can be a subtitle (missing cues show none).
    pub fn ready(&self) -> bool {
        self.have > 0
    }
}

/// What to run so `lang` has text to show: a live server reads the cache and the author's
/// file, a static export only what `export --lang` baked in.
pub fn how_to_make(lang: &str, live: bool) -> String {
    if live {
        t::subtitles::how_live(lang)
    } else {
        t::subtitles::how_export(lang)
    }
}

/// The existing translations of `texts` into `lang`, `None` for a piece that has none; `None`
/// altogether when the language has no text at all.
async fn known(live: bool, lang: &str, texts: &[String]) -> Option<Vec<Option<String>>> {
    if live {
        let r = gloo_net::http::Request::post(&format!("/api/subtitles/{lang}")).json(texts).ok()?.send().await.ok()?;
        if !r.ok() {
            return None;
        }
        r.json::<Vec<Option<String>>>().await.ok()
    } else {
        let r = gloo_net::http::Request::get(&format!("{}translations/{lang}.json", crate::topics::static_base())).send().await.ok()?;
        if !r.ok() {
            return None;
        }
        // a static server answers unknown paths with the page itself: not JSON, no text
        let map: HashMap<String, String> = r.json().await.ok()?;
        Some(texts.iter().map(|t| map.get(t).cloned()).collect())
    }
}

impl State {
    /// The pieces a subtitle needs: the script's, protected as the translations' keys are.
    /// The script as told in `code` (see `State::told`), its names, its pieces to translate.
    fn sub_texts(&self, code: &str) -> Option<(Rc<iwr_core::script::Script>, std::collections::HashSet<String>, Vec<String>)> {
        let base = self.told(code)?;
        let p = self.project.peek().clone()?;
        let names = iwr_core::i18n::names(&p);
        let texts = iwr_core::i18n::texts(&base, &[], &names);
        Some((base, names, texts))
    }

    /// Find which languages have text for the loaded script (when it loads, and after a
    /// translation filled the cache), then build the chosen subtitle.
    pub fn check_subtitles(&mut self) {
        let mut state = *self;
        let Some(src) = self.script_src.peek().clone() else { return };
        // each language's own pieces: iwr-core tells some languages itself
        let per_lang: Vec<Vec<String>> = iwr_core::i18n::LANGS.iter().map(|l| self.sub_texts(l.code).map(|(_, _, t)| t).unwrap_or_default()).collect();
        let live = *self.live.peek();
        spawn(async move {
            let mut out = Vec::new();
            for (l, texts) in iwr_core::i18n::LANGS.iter().zip(per_lang) {
                let have = if l.code == "en" {
                    texts.len()
                } else {
                    known(live, l.code, &texts).await.map(|v| v.iter().filter(|t| t.is_some()).count()).unwrap_or(0)
                };
                out.push(Avail { code: l.code, have, total: texts.len() });
            }
            // another script loaded meanwhile: its own check wins
            if !state.script_src.peek().as_ref().is_some_and(|s| Rc::ptr_eq(s, &src)) {
                return;
            }
            state.sub_avail.set(Some(Rc::new(out)));
            state.build_subtitles();
        });
    }

    /// Show subtitles in `code` (`""`: none), remembered in the settings.
    pub fn set_sub_lang(&mut self, code: &str) {
        self.sub_lang.set(code.to_string());
        self.save_settings();
        self.build_subtitles();
    }

    /// Make `sub_script`: the source script with the existing translations put in.
    pub fn build_subtitles(&mut self) {
        let mut state = *self;
        let code = self.sub_lang.peek().clone();
        let Some(src) = self.script_src.peek().clone() else { return };
        let Some((base, names, texts)) = self.sub_texts(&code).filter(|_| !code.is_empty()) else {
            self.sub_script.set(None);
            return;
        };
        if code == "en" {
            self.sub_script.set(Some(src));
            return;
        }
        let live = *self.live.peek();
        spawn(async move {
            let map: HashMap<String, String> = match known(live, &code, &texts).await {
                Some(done) => texts.iter().cloned().zip(done).filter_map(|(k, v)| Some((k, v?))).collect(),
                None => HashMap::new(),
            };
            if *state.sub_lang.peek() != code || !state.script_src.peek().as_ref().is_some_and(|s| Rc::ptr_eq(s, &src)) {
                return;
            }
            let mut s = (*base).clone();
            iwr_core::i18n::apply(&mut s, &mut [], &names, &|t| map.get(t).cloned());
            state.sub_script.set(Some(Rc::new(s)));
        });
    }

    /// The subtitle of the current cue: none when off, in the language that plays, or when it
    /// would repeat the caption.
    pub fn subtitle(&self) -> Option<String> {
        let code = self.sub_lang.read().clone();
        if code.is_empty() || code == *self.lang.read() {
            return None;
        }
        let (p, c) = (*self.part_idx.read(), *self.cue_idx.read());
        let say = |s: &Option<Rc<iwr_core::script::Script>>| s.as_ref().and_then(|s| s.parts.get(p)?.cues.get(c).map(|c| c.say.clone()));
        let sub = say(&self.sub_script.read())?;
        // a cue with no translation, or a caption not translated yet: the same text twice
        if say(&self.script.read()).is_some_and(|shown| shown == sub) {
            return None;
        }
        Some(sub)
    }
}

/// Text with `code` spans, as plain spans and `code` elements (no spoken forms: commands).
#[component]
pub(crate) fn Cmd(text: String) -> Element {
    rsx! {
        for (i, t) in text.split('`').enumerate() {
            if i % 2 == 1 { code { "{t}" } } else { span { "{t}" } }
        }
    }
}

/// The subtitle language menu: off, then every other language; one without text is greyed
/// out (its tooltip says how to make it). `full`: the Settings row, else the player's chip.
#[component]
pub fn SubPicker(full: bool) -> Element {
    let mut state = use_context::<State>();
    let cur = state.sub_lang.read().clone();
    let lang = state.lang.read().clone();
    let live = *state.live.read();
    let avail = state.sub_avail.read().clone();
    let Some(avail) = avail else {
        return if full { rsx! { span { class: "sd", {t::subtitles::load_first()} } } } else { rsx! {} };
    };
    let name = |c: &str| iwr_core::i18n::lang(c).map(|l| l.name).unwrap_or(c).to_string();
    let any = avail.iter().any(|a| a.ready() && a.code != lang);
    let tip = if any {
        t::subtitles::tip().to_string()
    } else {
        t::subtitles::tip_none(how_to_make("ja", live).replace('`', ""))
    };
    rsx! {
        select { class: if full { "theme-select" } else { "theme-select sub-pick" }, "data-spot": "subtitles", value: "{cur}", title: "{tip}",
            onchange: move |e| state.set_sub_lang(&e.value()),
            option { value: "", selected: cur.is_empty(), if full { {t::subtitles::off()} } else { {t::subtitles::none()} } }
            for a in avail.iter().filter(|a| a.code != lang || a.code == cur).cloned() {
                { let label = match (a.ready(), a.have < a.total) {
                      (false, _) => t::subtitles::no_text(name(a.code)),
                      (true, true) => t::subtitles::partial(a.total, name(a.code), a.have),
                      (true, false) => name(a.code),
                  };
                  let label = if full { label } else { format!("💬 {label}") };
                  rsx! { option { value: a.code, selected: cur == a.code, disabled: !a.ready(), title: if a.ready() { String::new() } else { how_to_make(a.code, live).replace('`', "") }, "{label}" } }
                }
            }
        }
    }
}

/// Under the Settings row: what the chosen subtitle lacks, and how to add a language.
#[component]
pub fn SubHelp() -> Element {
    let state = use_context::<State>();
    let cur = state.sub_lang.read().clone();
    let lang = state.lang.read().clone();
    let live = *state.live.read();
    let Some(avail) = state.sub_avail.read().clone() else { return rsx! {} };
    let name = |c: &str| iwr_core::i18n::lang(c).map(|l| l.name).unwrap_or(c).to_string();
    let chosen = avail.iter().find(|a| a.code == cur).cloned();
    let missing: Vec<&'static str> = avail.iter().filter(|a| !a.ready()).map(|a| a.code).collect();
    rsx! {
        div { class: "sub-help",
            if !cur.is_empty() && cur == lang {
                p { {t::subtitles::same_lang(name(&cur))} }
            }
            if let Some(a) = chosen.filter(|a| a.ready() && a.have < a.total) {
                p { {t::subtitles::partial_help(a.total, name(a.code), a.have)} Cmd { text: how_to_make(a.code, live) } }
            }
            if !missing.is_empty() {
                details {
                    summary { {t::subtitles::missing(missing.iter().map(|c| name(c)).collect::<Vec<_>>().join(", "))} }
                    for c in missing.iter().copied() {
                        p { b { {t::subtitles::lang_label(name(c))} } Cmd { text: how_to_make(c, live) } }
                    }
                }
            }
        }
    }
}
