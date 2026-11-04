//! The interface language: buttons, menus, settings, tooltips. Separate from the language a
//! codecast plays in (`State::lang`): a Japanese viewer can follow an English codecast with
//! Japanese buttons.
//!
//! Texts live in `crates/iwr-ui/i18n/<lang>/<module>.toml`; the build turns each key into a
//! function, `t::settings::title()`, that answers in the current language. Reading it inside a
//! component subscribes the component, so switching language redraws everything at once.
//!
//! The language is picked once at start, before the first frame: `?ui=ja` in the URL (not
//! saved), else the viewer's choice saved in this browser, else the browser's languages, else
//! English.

use dioxus::prelude::*;

include!(concat!(env!("OUT_DIR"), "/i18n.rs"));

/// localStorage key of the viewer's choice.
const KEY: &str = "iwr-ui-lang";

/// The current interface language; components that read it redraw when it changes.
static LANG: GlobalSignal<Lang> = Signal::global(detect);

/// The current interface language (subscribes the calling component).
pub fn lang() -> Lang {
    *LANG.read()
}

/// Switch the interface language, remember it in this browser and tell the page (`<html lang>`
/// picks the right Japanese glyphs and the screen reader's voice).
pub fn set(l: Lang) {
    *LANG.write() = l;
    if let Some(w) = web_sys::window() {
        if let Ok(Some(s)) = w.local_storage() {
            let _ = s.set_item(KEY, l.code());
        }
    }
    mark_page(l);
}

/// Put the language on `<html lang>`.
fn mark_page(l: Lang) {
    if let Some(el) = web_sys::window().and_then(|w| w.document()).and_then(|d| d.document_element()) {
        let _ = el.set_attribute("lang", l.code());
    }
}

/// The language to start in (see the module doc).
fn detect() -> Lang {
    let l = detect_from_browser().unwrap_or_default();
    mark_page(l);
    l
}

fn detect_from_browser() -> Option<Lang> {
    let w = web_sys::window()?;
    let from_url = w.location().search().ok().and_then(|q| web_sys::UrlSearchParams::new_with_str(&q).ok()).and_then(|p| p.get("ui"));
    if let Some(l) = from_url.as_deref().and_then(Lang::from_code) {
        return Some(l);
    }
    let saved = w.local_storage().ok().flatten().and_then(|s| s.get_item(KEY).ok().flatten());
    if let Some(l) = saved.as_deref().and_then(Lang::from_code) {
        return Some(l);
    }
    let nav = w.navigator();
    nav.languages().iter().filter_map(|v| v.as_string()).chain(nav.language()).find_map(|c| Lang::from_code(&c))
}

/// The same language for `iwr-core`'s texts (view names, graph notes, generated tours).
pub fn core_lang() -> iwr_core::tr::Lang {
    iwr_core::tr::Lang::from_code(lang().code()).unwrap_or_default()
}

/// The interface language picker: a select listing every language by its own name.
#[component]
pub fn LangPicker(#[props(default)] compact: bool) -> Element {
    let now = lang();
    rsx! {
        select { class: if compact { "lang-pick compact" } else { "theme-select" },
            "aria-label": t::settings::ui_lang(),
            title: t::settings::ui_lang(),
            onchange: move |e| { if let Some(l) = Lang::from_code(&e.value()) { set(l) } },
            for l in Lang::ALL.iter().copied() {
                option { value: l.code(), selected: l == now, "{l.name()}" }
            }
        }
    }
}
