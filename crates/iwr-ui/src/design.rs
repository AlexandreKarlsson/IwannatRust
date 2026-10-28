//! The codecast design editor on the Settings page: the author builds the codecast's
//! `codecast.toml` (theme, fonts, colours, voice, language) with a live preview, then saves it next
//! to the script (`iwr serve`) or downloads it (static export).

use crate::icons::Icon;
use crate::State;
use dioxus::prelude::*;
use iwr_core::config::{Config, COLORS};

/// Change the draft and preview it at once (theme, fonts, colours and settings all apply).
fn edit(mut state: State, mut draft: Signal<Config>, f: impl FnOnce(&mut Config)) {
    f(&mut draft.write());
    let c = draft.read().clone();
    state.apply_config(c, None);
}

/// `""` → None, else the trimmed text.
fn opt(v: String) -> Option<String> {
    let v = v.trim().to_string();
    (!v.is_empty()).then_some(v)
}

fn tri(v: Option<bool>) -> &'static str {
    match v {
        Some(true) => "on",
        Some(false) => "off",
        None => "",
    }
}

fn from_tri(v: &str) -> Option<bool> {
    match v {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    }
}

/// Upload a font file to the codecast directory (`fonts/NAME`); answers its relative path.
async fn upload_font(name: &str, bytes: &[u8]) -> Result<String, String> {
    let js = format!(
        r#"const b = Uint8Array.from(atob({}), c => c.charCodeAt(0));
           try {{ const r = await fetch("/api/font/" + encodeURIComponent({}), {{ method: "POST", body: b }});
                  dioxus.send((r.ok ? "ok:" : "err:") + await r.text()); }} catch (e) {{ dioxus.send("err:" + e); }}"#,
        serde_json::to_string(&crate::icons::base64(bytes)).unwrap_or_default(),
        serde_json::to_string(name).unwrap_or_default()
    );
    let mut ev = document::eval(&js);
    let msg = ev.recv::<String>().await.map_err(|e| format!("{e:?}"))?;
    match msg.strip_prefix("ok:") {
        Some(p) => Ok(p.to_string()),
        None => Err(msg.trim_start_matches("err:").to_string()),
    }
}

async fn save(c: &Config) -> Result<String, String> {
    let r = gloo_net::http::Request::post("/api/config").json(c).map_err(|e| e.to_string())?.send().await.map_err(|e| e.to_string())?;
    let text = r.text().await.unwrap_or_default();
    if r.ok() {
        Ok(text)
    } else {
        Err(if text.is_empty() { r.status().to_string() } else { text })
    }
}

fn download(c: &Config) {
    document::eval(&format!(
        r#"const a = document.createElement("a");
           a.href = URL.createObjectURL(new Blob([{}], {{ type: "application/toml" }}));
           a.download = "codecast.toml"; a.click(); setTimeout(() => URL.revokeObjectURL(a.href), 1000);"#,
        serde_json::to_string(&c.to_toml()).unwrap_or_default()
    ));
}

/// One `[fonts]` role: a font name or file path, and (with a server) a file to upload.
fn get_font(c: &Config, role: &str) -> Option<String> {
    match role {
        "title" => c.fonts.title.clone(),
        "text" => c.fonts.text.clone(),
        _ => c.fonts.code.clone(),
    }
}

fn set_font(c: &mut Config, role: &str, v: Option<String>) {
    match role {
        "title" => c.fonts.title = v,
        "text" => c.fonts.text = v,
        _ => c.fonts.code = v,
    }
}

/// Upload a font file of the author's own for `role` (with a server only).
#[component]
fn UploadFont(role: &'static str, draft: Signal<Config>, status: Signal<String>) -> Element {
    let state = use_context::<State>();
    rsx! {
        label { class: "upload", title: "copy a font file (ttf, otf, woff, woff2) into codecast/fonts/ and use it",
            "＋ your own font file…"
            input { r#type: "file", accept: ".ttf,.otf,.woff,.woff2", style: "display:none", onchange: move |e| {
                let files = e.files();
                let mut status = status;
                spawn(async move {
                    let Some(f) = files.first() else { return };
                    let name = f.name();
                    status.set(format!("uploading {name}…"));
                    match f.read_bytes().await {
                        Ok(bytes) => match upload_font(&name, &bytes).await {
                            Ok(path) => {
                                status.set(format!("font saved as {path}"));
                                edit(state, draft, |c| set_font(c, role, Some(path)));
                            }
                            Err(e) => status.set(format!("⚠ {e}")),
                        },
                        Err(e) => status.set(format!("⚠ {e:?}")),
                    }
                });
            } }
        }
    }
}

#[component]
pub fn DesignEditor() -> Element {
    let mut state = use_context::<State>();
    // the draft starts from the codecast's current config
    let mut draft = use_signal(|| (**state.config.peek()).clone());
    let status = use_signal(String::new);
    // the config may arrive after the page opened (or be re-applied): follow it
    use_effect(move || {
        let c = (**state.config.read()).clone();
        if *draft.peek() != c {
            draft.set(c);
        }
    });
    // recorded voices with a name (`iwr voice`), in English: what `rec_voice` can name
    let recorded: Vec<(String, usize)> = state.script_src.read().as_ref().map(|s| iwr_core::i18n::recorded_voices(s, "en")).unwrap_or_default().into_iter().filter(|(v, _)| !v.is_empty()).collect();
    let mut voices = use_signal(Vec::<String>::new);
    use_future(move || async move {
        let mut ev = document::eval(
            r#"const send = () => { try { dioxus.send(speechSynthesis.getVoices().map(v => v.name + " (" + v.lang + ")")); } catch (e) { dioxus.send([]); } };
               try { if (speechSynthesis.getVoices().length) send(); else { speechSynthesis.addEventListener("voiceschanged", send, { once: true }); setTimeout(send, 2000); } } catch (e) { send(); }"#,
        );
        if let Ok(v) = ev.recv::<Vec<String>>().await {
            voices.set(v);
        }
    });
    let mut font_role = use_signal(|| "title");
    // folded until the author asks for it
    let mut show = use_signal(|| false);
    let open = *show.read();
    let mut show_colors = use_signal(|| false);
    let colors_open = *show_colors.read();
    let role = *font_role.read();
    let live = *state.live.read();
    let has_script = state.script.read().is_some();
    let c = draft.read().clone();
    let problems = c.problems();
    let status_now = status.read().clone();
    let theme_val = c.theme.clone().unwrap_or_default();
    let role_font = get_font(&c, role).unwrap_or_default();
    let configured = c != Config::default();
    let n_colors = c.colors.len();
    rsx! {
        section { id: "s-design", class: "card design",
            div { class: "chead",
                span { class: "cicon", Icon { name: "tour", size: 18 } }
                div {
                    h3 { "Codecast design " span { class: "badge", "author" } }
                    div { class: "csub", "The look and defaults viewers of this codecast start with." }
                }
            }
            div { class: "cta",
                button { class: if open { "small" } else { "big" }, "aria-expanded": "{open}", onclick: move |_| { let o = *show.read(); show.set(!o); },
                    if open { "Hide the configuration" } else if configured { "Edit the codecast configuration" } else { "Create the codecast configuration" }
                }
                span { class: "info-tip", tabindex: "0", "aria-label": "What is the codecast configuration?",
                    Icon { name: "info", size: 13 }
                    span { class: "tip", role: "tooltip",
                        b { "codecast.toml" }
                        " is a small file next to the script. It sets the theme, fonts, colours, voice and language viewers of this codecast start with; with lock on, they keep them. "
                        "Pick everything here with a live preview, then save it (iwr serve) or download it. Without the file, viewers keep their own settings."
                    }
                }
            }
            if open {
            h4 { "Theme" }
            crate::settings::ThemeGrid { value: theme_val, allow_none: true, onpick: move |v: String| edit(state, draft, |c| c.theme = opt(v)) }

            h4 { "Fonts" }
            div { class: "seg",
                for (r, l) in [("title", "Box titles"), ("text", "Interface text"), ("code", "Code")] {
                    button { class: if role == r { "sel" } else { "" }, onclick: move |_| font_role.set(r), "{l}" }
                }
            }
            if role == "code" {
                crate::settings::Row { title: "Code font", desc: "an installed font's name, or a file uploaded below (no bundled font is monospace)".to_string(),
                    input { r#type: "text", placeholder: "e.g. JetBrains Mono", value: "{role_font}", onchange: move |e| edit(state, draft, |c| set_font(c, "code", opt(e.value()))) }
                }
            } else {
                crate::settings::FontGrid { value: role_font, sample: if role == "title" { "Parse the task list".to_string() } else { "The scheduler picks the heaviest task".to_string() }, none_label: "the viewer's",
                    onpick: move |v: String| edit(state, draft, |c| set_font(c, role, opt(v))) }
            }
            if live {
                UploadFont { role, draft, status }
            }

            h4 { "Voice and language" }
            crate::settings::Row { title: "Language",
                select { class: "theme-select", onchange: move |e| edit(state, draft, |c| c.lang = opt(e.value())),
                    option { value: "", selected: c.lang.is_none(), "viewer's choice" }
                    for l in iwr_core::i18n::LANGS {
                        option { value: l.code, selected: c.lang.as_deref() == Some(l.code), "{l.name}" }
                    }
                }
            }
            crate::settings::Row { title: "Voice",
                select { class: "theme-select", onchange: move |e| edit(state, draft, |c| c.voice = opt(e.value())),
                    option { value: "", selected: c.voice.is_none(), "viewer's choice" }
                    option { value: "tts", selected: c.voice.as_deref() == Some("tts"), "🔊 spoken by the browser" }
                    option { value: "audio", selected: c.voice.as_deref() == Some("audio"), "🎧 the recording" }
                    option { value: "read", selected: c.voice.as_deref() == Some("read"), "📖 read silently" }
                }
            }
            crate::settings::Row { title: "Browser voice", desc: "picked when the viewer's browser has a voice with this in its name".to_string(),
                input { r#type: "text", list: "iwr-voices", placeholder: "part of a voice name", value: "{c.tts_voice.clone().unwrap_or_default()}",
                    // the list shows "Name (lang)": keep the name
                    onchange: move |e| edit(state, draft, |c| c.tts_voice = opt(e.value().rsplit_once(" (").map(|(n, _)| n.to_string()).unwrap_or(e.value()))) }
            }
            crate::settings::Row { title: "Second browser voice", desc: "says the sp: lines; without one, another voice of the language, else the same voice higher".to_string(),
                input { r#type: "text", list: "iwr-voices", placeholder: "part of a voice name", value: "{c.tts_voice2.clone().unwrap_or_default()}",
                    onchange: move |e| edit(state, draft, |c| c.tts_voice2 = opt(e.value().rsplit_once(" (").map(|(n, _)| n.to_string()).unwrap_or(e.value()))) }
                datalist { id: "iwr-voices",
                    for v in voices.read().iter() {
                        option { value: "{v}" }
                    }
                }
            }
            if !recorded.is_empty() {
                crate::settings::Row { title: "Recorded voice", desc: "the recording played first when several voices recorded the codecast (a pair: two voices)".to_string(),
                    select { class: "theme-select", onchange: move |e| edit(state, draft, |c| c.rec_voice = opt(e.value())),
                        option { value: "", selected: c.rec_voice.is_none(), "the one with the most parts" }
                        for (id, n) in recorded.iter().cloned() {
                            option { value: "{id}", selected: c.rec_voice.as_deref() == Some(id.as_str()), "{iwr_core::voice::voice_label(&id).0} ({n} part{plural(n)})" }
                        }
                    }
                }
            }
            crate::settings::Row { title: "Speech rate",
                input { r#type: "number", min: "0.5", max: "2", step: "0.1", placeholder: "viewer's", value: c.rate.map(|r| format!("{r:.1}")).unwrap_or_default(),
                    onchange: move |e| edit(state, draft, |c| c.rate = e.value().trim().parse::<f64>().ok().map(|r| r.clamp(0.5, 2.0))) }
            }
            crate::settings::Row { title: "Full tour",
                select { class: "theme-select", value: tri(c.tour), onchange: move |e| edit(state, draft, |c| c.tour = from_tri(&e.value())),
                    option { value: "", "viewer's choice" }
                    option { value: "on", "on" }
                    option { value: "off", "off" }
                }
            }
            crate::settings::Row { title: "Glossary questions",
                select { class: "theme-select", value: tri(c.glossary), onchange: move |e| edit(state, draft, |c| c.glossary = from_tri(&e.value())),
                    option { value: "", "viewer's choice" }
                    option { value: "on", "on" }
                    option { value: "off", "off" }
                }
            }
            crate::settings::Row { title: "Lock", desc: "viewers cannot change theme, fonts, language and voice".to_string(),
                crate::settings::Toggle { checked: c.lock, onchange: move |v| edit(state, draft, |c| c.lock = v) }
            }

            div { class: "cta",
                button { class: "small", "aria-expanded": "{colors_open}", onclick: move |_| { let o = *show_colors.read(); show_colors.set(!o); },
                    if colors_open { "▾ Custom colours" } else { "▸ Custom colours" }
                    if n_colors > 0 { span { class: "sd", " · {n_colors} set" } }
                }
                span { class: "info-tip", tabindex: "0", "aria-label": "What are custom colours?",
                    Icon { name: "info", size: 13 }
                    span { class: "tip", role: "tooltip", "Change single colours of the theme (accent, background, highlight…) for this codecast. Rarely needed: picking a theme is usually enough. ✕ on a colour goes back to the theme's." }
                }
                if colors_open && n_colors > 0 {
                    button { class: "small", onclick: move |_| edit(state, draft, |c| c.colors.clear()), "reset all colours" }
                }
            }
            if colors_open {
                div { class: "colors",
                    for k in COLORS.iter().copied() {
                        { let v = c.colors.get(k).cloned(); let set = v.is_some(); rsx! {
                            label { class: if set { "color set" } else { "color" },
                                input { r#type: "color", value: v.clone().filter(|v| v.len() == 7 && v.starts_with('#')).unwrap_or("#888888".into()),
                                    oninput: move |e| edit(state, draft, |c| { c.colors.insert(k.to_string(), e.value()); }) }
                                span { "{k}" }
                                if set {
                                    button { class: "small", title: "use the theme's colour", onclick: move |_| edit(state, draft, |c| { c.colors.remove(k); }), "✕" }
                                }
                            }
                        } }
                    }
                }
            }
            for p in problems.iter() {
                div { class: "sd warn", "⚠ {p}" }
            }
            div { class: "cta",
                if live && has_script {
                    button { class: "big", disabled: !problems.is_empty(), onclick: move |_| {
                        let c = draft.read().clone();
                        let mut status = status;
                        spawn(async move {
                            match save(&c).await {
                                Ok(path) => {
                                    status.set(format!("✓ saved {path}"));
                                    if let Ok(json) = serde_json::to_string(&c) {
                                        document::eval(&format!("localStorage.setItem('iwr-config', {});", serde_json::to_string(&json).unwrap_or_default()));
                                    }
                                }
                                Err(e) => status.set(format!("⚠ {e}")),
                            }
                        });
                    }, "Save codecast.toml" }
                }
                button { class: "small", onclick: move |_| download(&draft.read()), "download" }
                button { class: "small", title: "start again from an empty design", onclick: move |_| { draft.set(Config::default()); state.config.set(Default::default()); }, "clear" }
                if !status_now.is_empty() {
                    span { class: if status_now.starts_with('⚠') { "sd warn" } else { "sd" }, "{status_now}" }
                }
            }
            if !live {
                div { class: "sd", "Static site: download the file and put it next to the script (codecast/codecast.toml), with any font file of your own it names." }
            } else if !has_script {
                div { class: "sd", "No codecast next to this project yet: create codecast.md or codecast/ to save the design there, or download it." }
            }
            }
        }
    }
}

/// `s` after a count other than one.
fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}
