//! The codecast design editor on the Settings page: the author builds the codecast's
//! `codecast.toml` (theme, fonts, colours, voice, language) with a live preview, then saves it next
//! to the script (`codecast serve`) or downloads it (static export).

use crate::i18n::t;
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
        label { class: "upload", title: t::design::upload_tip(),
            {t::design::upload()}
            input { r#type: "file", accept: ".ttf,.otf,.woff,.woff2", style: "display:none", onchange: move |e| {
                let files = e.files();
                let mut status = status;
                spawn(async move {
                    let Some(f) = files.first() else { return };
                    let name = f.name();
                    status.set(t::design::uploading(&name));
                    match f.read_bytes().await {
                        Ok(bytes) => match upload_font(&name, &bytes).await {
                            Ok(path) => {
                                status.set(t::design::font_saved(&path));
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
    // recorded voices with a name (`codecast voice`), in English: what `rec_voice` can name
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
                    h3 { {t::design::title()} " " span { class: "badge", {t::design::badge()} } }
                    div { class: "csub", {t::design::sub()} }
                }
            }
            div { class: "cta",
                button { class: if open { "small" } else { "big" }, "aria-expanded": "{open}", onclick: move |_| { let o = *show.read(); show.set(!o); },
                    if open { {t::design::hide()} } else if configured { {t::design::edit()} } else { {t::design::create()} }
                }
                span { class: "info-tip", tabindex: "0", "aria-label": t::design::what_config(),
                    Icon { name: "info", size: 13 }
                    span { class: "tip", role: "tooltip",
                        b { "codecast.toml" }
                        {t::design::config_tip()}
                    }
                }
            }
            if open {
            h4 { {t::design::theme()} }
            crate::settings::ThemeGrid { value: theme_val, allow_none: true, onpick: move |v: String| edit(state, draft, |c| c.theme = opt(v)) }

            h4 { {t::design::fonts()} }
            div { class: "seg",
                for (r, l) in [("title", t::design::box_titles()), ("text", t::design::interface_text()), ("code", t::design::code())] {
                    button { class: if role == r { "sel" } else { "" }, onclick: move |_| font_role.set(r), "{l}" }
                }
            }
            if role == "code" {
                crate::settings::Row { title: t::design::code_font(), desc: t::design::code_font_desc().to_string(),
                    input { r#type: "text", placeholder: t::design::code_font_ph(), value: "{role_font}", onchange: move |e| edit(state, draft, |c| set_font(c, "code", opt(e.value()))) }
                }
            } else {
                crate::settings::FontGrid { value: role_font, sample: if role == "title" { "Parse the task list".to_string() } else { "The scheduler picks the heaviest task".to_string() }, viewer: true, // i18n: sample drawn in Latin-only fonts
                    onpick: move |v: String| edit(state, draft, |c| set_font(c, role, opt(v))) }
            }
            if live {
                UploadFont { role, draft, status }
            }

            h4 { {t::design::voice_lang()} }
            crate::settings::Row { title: t::design::language(),
                select { class: "theme-select", onchange: move |e| edit(state, draft, |c| c.lang = opt(e.value())),
                    option { value: "", selected: c.lang.is_none(), {t::design::viewers_choice()} }
                    for l in iwr_core::i18n::LANGS {
                        option { value: l.code, selected: c.lang.as_deref() == Some(l.code), "{l.name}" }
                    }
                }
            }
            crate::settings::Row { title: t::design::voice(),
                select { class: "theme-select", onchange: move |e| edit(state, draft, |c| c.voice = opt(e.value())),
                    option { value: "", selected: c.voice.is_none(), {t::design::viewers_choice()} }
                    option { value: "tts", selected: c.voice.as_deref() == Some("tts"), {t::design::voice_tts()} }
                    option { value: "audio", selected: c.voice.as_deref() == Some("audio"), {t::design::voice_audio()} }
                    option { value: "read", selected: c.voice.as_deref() == Some("read"), {t::design::voice_read()} }
                }
            }
            crate::settings::Row { title: t::design::browser_voice(), desc: t::design::browser_voice_desc().to_string(),
                input { r#type: "text", list: "iwr-voices", placeholder: t::design::voice_ph(), value: "{c.tts_voice.clone().unwrap_or_default()}",
                    // the list shows "Name (lang)": keep the name
                    onchange: move |e| edit(state, draft, |c| c.tts_voice = opt(e.value().rsplit_once(" (").map(|(n, _)| n.to_string()).unwrap_or(e.value()))) }
            }
            crate::settings::Row { title: t::design::second_voice(), desc: t::design::second_voice_desc().to_string(),
                input { r#type: "text", list: "iwr-voices", placeholder: t::design::voice_ph(), value: "{c.tts_voice2.clone().unwrap_or_default()}",
                    onchange: move |e| edit(state, draft, |c| c.tts_voice2 = opt(e.value().rsplit_once(" (").map(|(n, _)| n.to_string()).unwrap_or(e.value()))) }
                datalist { id: "iwr-voices",
                    for v in voices.read().iter() {
                        option { value: "{v}" }
                    }
                }
            }
            if !recorded.is_empty() {
                crate::settings::Row { title: t::design::rec_voice(), desc: t::design::rec_voice_desc().to_string(),
                    select { class: "theme-select", onchange: move |e| edit(state, draft, |c| c.rec_voice = opt(e.value())),
                        option { value: "", selected: c.rec_voice.is_none(), {t::design::rec_most()} }
                        for (id, n) in recorded.iter().cloned() {
                            option { value: "{id}", selected: c.rec_voice.as_deref() == Some(id.as_str()), {t::design::rec_named(n, iwr_core::voice::voice_label(&id).0)} }
                        }
                    }
                }
            }
            crate::settings::Row { title: t::design::speech_rate(),
                input { r#type: "number", min: "0.5", max: "2", step: "0.1", placeholder: t::design::rate_ph(), value: c.rate.map(|r| format!("{r:.1}")).unwrap_or_default(),
                    onchange: move |e| edit(state, draft, |c| c.rate = e.value().trim().parse::<f64>().ok().map(|r| r.clamp(0.5, 2.0))) }
            }
            crate::settings::Row { title: t::design::full_tour(),
                select { class: "theme-select", value: tri(c.tour), onchange: move |e| edit(state, draft, |c| c.tour = from_tri(&e.value())),
                    option { value: "", {t::design::viewers_choice()} }
                    option { value: "on", {t::design::on()} }
                    option { value: "off", {t::design::off()} }
                }
            }
            crate::settings::Row { title: t::design::glossary(),
                select { class: "theme-select", value: tri(c.glossary), onchange: move |e| edit(state, draft, |c| c.glossary = from_tri(&e.value())),
                    option { value: "", {t::design::viewers_choice()} }
                    option { value: "on", {t::design::on()} }
                    option { value: "off", {t::design::off()} }
                }
            }
            crate::settings::Row { title: t::design::lock(), desc: t::design::lock_desc().to_string(),
                crate::settings::Toggle { checked: c.lock, onchange: move |v| edit(state, draft, |c| c.lock = v) }
            }

            div { class: "cta",
                button { class: "small", "aria-expanded": "{colors_open}", onclick: move |_| { let o = *show_colors.read(); show_colors.set(!o); },
                    if colors_open { {t::design::colors_open()} } else { {t::design::colors_closed()} }
                    if n_colors > 0 { span { class: "sd", {t::design::colors_set(n_colors)} } }
                }
                span { class: "info-tip", tabindex: "0", "aria-label": t::design::what_colors(),
                    Icon { name: "info", size: 13 }
                    span { class: "tip", role: "tooltip", {t::design::colors_tip()} }
                }
                if colors_open && n_colors > 0 {
                    button { class: "small", onclick: move |_| edit(state, draft, |c| c.colors.clear()), {t::design::reset_colors()} }
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
                                    button { class: "small", title: t::design::color_reset_tip(), onclick: move |_| edit(state, draft, |c| { c.colors.remove(k); }), "✕" }
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
                                    status.set(t::design::saved(&path));
                                    if let Ok(json) = serde_json::to_string(&c) {
                                        document::eval(&format!("localStorage.setItem('iwr-config', {});", serde_json::to_string(&json).unwrap_or_default()));
                                    }
                                }
                                Err(e) => status.set(format!("⚠ {e}")),
                            }
                        });
                    }, {t::design::save()} }
                }
                button { class: "small", onclick: move |_| download(&draft.read()), {t::design::download()} }
                button { class: "small", title: t::design::clear_tip(), onclick: move |_| { draft.set(Config::default()); state.config.set(Default::default()); }, {t::design::clear()} }
                if !status_now.is_empty() {
                    span { class: if status_now.starts_with('⚠') { "sd warn" } else { "sd" }, "{status_now}" }
                }
            }
            if !live {
                div { class: "sd", {t::design::static_note()} }
            } else if !has_script {
                div { class: "sd", {t::design::no_script_note()} }
            }
            }
        }
    }
}
