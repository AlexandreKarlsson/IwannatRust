//! The bundled fonts (`assets/fonts/*.woff2`, catalogue in `iwr_core::config::FONTS`) and the
//! CSS that puts the chosen ones on box titles, text and code.

use dioxus::prelude::*;
use iwr_core::config::{is_font_file, is_safe_font_name, is_safe_path};

/// Font file of every bundled font, by id.
const FILES: &[(&str, Asset)] = &[
    ("akieir", asset!("/assets/fonts/akieir.woff2")),
    ("anak-bocil", asset!("/assets/fonts/anak-bocil.woff2")),
    ("apoca", asset!("/assets/fonts/apoca.woff2")),
    ("asal-usul", asset!("/assets/fonts/asal-usul.woff2")),
    ("brimful", asset!("/assets/fonts/brimful.woff2")),
    ("catways", asset!("/assets/fonts/catways.woff2")),
    ("cozy-autumns", asset!("/assets/fonts/cozy-autumns.woff2")),
    ("digitalt", asset!("/assets/fonts/digitalt.woff2")),
    ("lost-tumbler", asset!("/assets/fonts/lost-tumbler.woff2")),
    ("north-kids", asset!("/assets/fonts/north-kids.woff2")),
    ("outline-style", asset!("/assets/fonts/outline-style.woff2")),
    ("pasti", asset!("/assets/fonts/pasti.woff2")),
    ("quango", asset!("/assets/fonts/quango.woff2")),
    ("rubintek", asset!("/assets/fonts/rubintek.woff2")),
    ("slow-play", asset!("/assets/fonts/slow-play.woff2")),
    ("super-shiny", asset!("/assets/fonts/super-shiny.woff2")),
    ("thesead", asset!("/assets/fonts/thesead.woff2")),
    ("zerove", asset!("/assets/fonts/zerove.woff2")),
    ("library-3am", asset!("/assets/fonts/library-3am.woff2")),
    ("aicon", asset!("/assets/fonts/aicon.woff2")),
    ("rahull", asset!("/assets/fonts/rahull.woff2")),
    ("hey-august", asset!("/assets/fonts/hey-august.woff2")),
    ("mocaroni", asset!("/assets/fonts/mocaroni.woff2")),
    ("montserrat", asset!("/assets/fonts/montserrat.woff2")),
    ("josefin-sans", asset!("/assets/fonts/josefin-sans.woff2")),
    ("zt-otez", asset!("/assets/fonts/zt-otez.woff2")),
    ("glametrix", asset!("/assets/fonts/glametrix.woff2")),
    ("timeburner", asset!("/assets/fonts/timeburner.woff2")),
    ("ovaline", asset!("/assets/fonts/ovaline.woff2")),
    ("steiner", asset!("/assets/fonts/steiner.woff2")),
    ("cambai", asset!("/assets/fonts/cambai.woff2")),
];

/// URL of a bundled font, relative to the page so it works from a static export under any sub-path.
pub fn url(id: &str) -> Option<String> {
    FILES.iter().find(|(i, _)| *i == id).map(|(_, a)| a.to_string().trim_start_matches('/').to_string())
}

/// CSS family of a bundled font (its `@font-face` is in `faces()`).
pub fn family(id: &str) -> String {
    format!("iwr-f-{id}")
}

/// `@font-face` for every bundled font; the browser only downloads the ones in use. Wide fonts
/// carry their `size-adjust` (`Font::size`).
pub fn faces() -> String {
    FILES
        .iter()
        .filter_map(|(id, _)| {
            let size = iwr_core::config::font(id).map_or(100, |f| f.size);
            Some(format!("@font-face {{ font-family:\"{}\"; src:url(\"{}\") format(\"woff2\"); font-display:swap; size-adjust:{size}%; }}\n", family(id), url(id)?))
        })
        .collect()
}

/// The CSS family (and its `@font-face` if it is a file) for a `[fonts]` value: a bundled font's
/// id, a file next to the script (under `base`), or an installed font's name. None if unsafe.
pub fn resolve(role: &str, v: &str, base: &str) -> Option<(String, String)> {
    if iwr_core::config::font(v).is_some() {
        Some((family(v), String::new()))
    } else if is_font_file(v) && is_safe_path(v) {
        Some((format!("iwr-{role}"), format!("@font-face {{ font-family:\"iwr-{role}\"; src:url(\"{base}{v}\"); font-display:swap; }}\n")))
    } else if !is_font_file(v) && is_safe_font_name(v) {
        Some((v.to_string(), String::new()))
    } else {
        None
    }
}
