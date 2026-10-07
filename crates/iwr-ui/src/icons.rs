//! Icons: small PNG silhouettes embedded in the wasm and served as data URIs, so
//! they work both from `iwr serve` and from a static export under any sub-path.
//! Black on transparent; themes recolour them with the `--icon-filter` CSS variable.

use dioxus::prelude::*;
use iwr_core::views::Mode;
use std::collections::HashMap;
use std::sync::OnceLock;

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        const RAW: &[(&str, &[u8])] = &[$(($name, include_bytes!(concat!("../assets/icons/", $name, ".png")))),*];
    };
}

icons!["code", "calls", "flow", "arch", "branches", "structure", "types", "errors", "diagram", "plan", "settings", "palette", "back", "info", "dark", "light", "panel", "split", "codecast", "tour", "fit", "check", "prev", "next", "question", "home", "logo", "favicon"];

fn table() -> &'static HashMap<&'static str, String> {
    static T: OnceLock<HashMap<&'static str, String>> = OnceLock::new();
    T.get_or_init(|| RAW.iter().map(|(n, b)| (*n, format!("data:image/png;base64,{}", base64(b)))).collect())
}

/// Data URI of an icon (empty string for an unknown name).
pub fn uri(name: &str) -> &'static str {
    table().get(name).map(|s| s.as_str()).unwrap_or("")
}

pub fn for_mode(m: Mode) -> &'static str {
    match m {
        Mode::Code => "code",
        Mode::CallTree => "calls",
        Mode::ControlFlow => "flow",
        Mode::Architecture => "arch",
        Mode::BranchTree => "branches",
        Mode::Structure => "structure",
        Mode::Types => "types",
        Mode::ErrorFlow => "errors",
        Mode::Diagram => "diagram",
        Mode::Plan => "plan",
    }
}

/// The IwannatRust logo, painted in a CSS colour (default: the theme accent) through a mask.
#[component]
pub fn Logo(#[props(default = 22)] size: u32, #[props(default = "var(--accent)".to_string())] color: String) -> Element {
    let src = uri("logo");
    rsx! { span { class: "logo", style: "width:{size}px; height:{size}px; background:{color}; -webkit-mask-image:url({src}); mask-image:url({src});" } }
}

/// An icon image. `size` in px (default 16).
#[component]
pub fn Icon(name: String, #[props(default = 16)] size: u32) -> Element {
    let src = uri(&name);
    rsx! { img { class: "ico", src: "{src}", width: "{size}", height: "{size}", alt: "", draggable: false } }
}

/// Icon button with a hover label (CSS tooltip through `data-tip`).
#[component]
pub fn IconButton(name: String, tip: String, #[props(default = false)] active: bool, #[props(default)] text: String, onclick: EventHandler<MouseEvent>) -> Element {
    let src = uri(&name);
    rsx! {
        button { class: if active { "ibtn active" } else { "ibtn" }, "data-tip": "{tip}", "aria-label": "{tip}", onclick: move |e| onclick.call(e),
            img { class: "ico", src: "{src}", width: "16", height: "16", alt: "", draggable: false }
            if !text.is_empty() { span { "{text}" } }
        }
    }
}

pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len() * 4 / 3 + 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}
