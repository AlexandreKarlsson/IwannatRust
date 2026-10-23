//! First start and the tutorial. The tutorial is a codecast like any other
//! (`assets/tutorial/`, recorded once with Kokoro): its text never names the project, its
//! directive lines do through placeholders (`iwr_core::tutorial::fill`). The "have you used this
//! app before?" dialog starts it; the top bar's *?* button asks again.
//!
//! Also here: [`Spot`], the light a codecast's `+ search` / `+ player` line puts on a part of the
//! app (`iwr_core::script::UI_SPOTS`), the rest of the screen dimmed, and [`Zoom`], the
//! presenter's zoom of `+ zoom`.

use crate::State;
use dioxus::prelude::*;
use iwr_core::script::{self, PartFile, Voiced};

/// The voice the tutorial was recorded with (`iwr voice assets/tutorial --path examples/demo`).
const VOICE: &str = "af_heart";

const INDEX: &str = include_str!("../assets/tutorial/index.md");

/// Each part: file stem, script, `.times` of its recording, the recording.
const PARTS: &[(&str, &str, &str, Asset)] = &[
    ("01-welcome", include_str!("../assets/tutorial/01-welcome.md"), include_str!("../assets/tutorial/01-welcome.af_heart.times"), asset!("/assets/tutorial/01-welcome.af_heart.ogg")),
    ("02-the-player", include_str!("../assets/tutorial/02-the-player.md"), include_str!("../assets/tutorial/02-the-player.af_heart.times"), asset!("/assets/tutorial/02-the-player.af_heart.ogg")),
    ("03-files-and-folders", include_str!("../assets/tutorial/03-files-and-folders.md"), include_str!("../assets/tutorial/03-files-and-folders.af_heart.times"), asset!("/assets/tutorial/03-files-and-folders.af_heart.ogg")),
    ("04-reading-code", include_str!("../assets/tutorial/04-reading-code.md"), include_str!("../assets/tutorial/04-reading-code.af_heart.times"), asset!("/assets/tutorial/04-reading-code.af_heart.ogg")),
    ("05-the-views", include_str!("../assets/tutorial/05-the-views.md"), include_str!("../assets/tutorial/05-the-views.af_heart.times"), asset!("/assets/tutorial/05-the-views.af_heart.ogg")),
    ("06-sketches-and-notes", include_str!("../assets/tutorial/06-sketches-and-notes.md"), include_str!("../assets/tutorial/06-sketches-and-notes.af_heart.times"), asset!("/assets/tutorial/06-sketches-and-notes.af_heart.ogg")),
    ("07-around-the-app", include_str!("../assets/tutorial/07-around-the-app.md"), include_str!("../assets/tutorial/07-around-the-app.af_heart.times"), asset!("/assets/tutorial/07-around-the-app.af_heart.ogg")),
    ("08-your-own-codecast", include_str!("../assets/tutorial/08-your-own-codecast.md"), include_str!("../assets/tutorial/08-your-own-codecast.af_heart.times"), asset!("/assets/tutorial/08-your-own-codecast.af_heart.ogg")),
];

/// Cue start times of a `.times` file (`start-seconds text-hash` lines, `#` comments).
fn times(t: &str) -> Vec<f64> {
    t.lines().filter(|l| !l.trim_start().starts_with('#')).filter_map(|l| l.split_whitespace().next()?.parse().ok()).collect()
}

/// The tutorial codecast for `p`; recordings are fetched from `base` (the page's directory URL).
fn tutorial_script(p: &iwr_core::Project, base: &str) -> script::Script {
    let parts: Vec<PartFile> = PARTS
        .iter()
        .map(|(stem, text, t, audio)| {
            let audio = format!("{}{}", base, audio.to_string().trim_start_matches('/'));
            let times = times(t);
            PartFile { stem: stem.to_string(), text: iwr_core::tutorial::fill(p, text), audio: Some(audio.clone()), times: times.clone(), voices: vec![Voiced { lang: "en".into(), voice: VOICE.into(), audio, times }] }
        })
        .collect();
    script::parse(&script::assemble(INDEX, &parts))
}

/// Where a `+` spot is on screen (see `iwr_core::script::UI_SPOTS`).
pub fn selector(spot: &str) -> Option<&'static str> {
    Some(match spot {
        "brand" => ".topbar .brand",
        "panel-button" => ".topbar [aria-label^='Files and items']",
        "views" => ".topbar .tabs",
        "split" => ".topbar [aria-label^='Show the source']",
        "search" => ".topbar input",
        "codecast" => ".topbar [aria-label*='codecast (g)']",
        "tour" => ".topbar [aria-label^='Full tour']",
        "generate" => ".topbar .gen-btn",
        "help" => ".topbar [aria-label^='Help']",
        "settings" => ".topbar [aria-label^='Settings']",
        "sidebar-tabs" => ".sidebar .sbtabs",
        "player" => ".guide",
        "caption" => ".guide .body",
        "parts" => ".guide .parts",
        "voice" => ".guide [data-spot='voice']",
        "lang" => ".guide [data-spot='lang']",
        "plan-button" => ".guide [data-spot='plan']",
        "questions" => ".guide .qs",
        "side" => ".side",
        // panels, for `+ zoom files`
        "files" | "items" => ".sidebar",
        "view" => ".canvas-wrap, .code-view .blocks-pane, .plan",
        "source" => ".details, .code-view .source-pane, .side",
        _ => return None,
    })
}

impl State {
    /// Remember that this browser has been asked.
    fn mark_welcomed(&self) {
        document::eval("try { localStorage.setItem('iwr-welcomed', '1'); } catch (e) {}");
    }

    pub fn close_welcome(&mut self) {
        self.welcome_open.set(false);
        self.mark_welcomed();
    }

    /// Play the tutorial codecast, with its recording, every part in a row.
    pub fn start_tutorial(&mut self) {
        self.close_welcome();
        self.close_pages();
        let Some(p) = self.project.peek().clone() else { return };
        let mut state = *self;
        // not owned by the dialog, which closes now
        dioxus::core::spawn_forever(async move {
            // the recordings are page-relative assets: make them absolute, `audio:` names
            // without a scheme are looked up next to the project's codecast
            let base = document::eval("return new URL('.', document.baseURI).href;").await.ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_else(|| "/".into());
            state.stop_script();
            state.audio_url.set(None);
            state.load_script(tutorial_script(&p, &base));
            state.voice.set(crate::player::Voice::Audio);
            state.tour.set(true);
            state.playing.set(true);
        });
    }
}

/// The first-start dialog.
#[component]
pub fn Welcome() -> Element {
    let mut state = use_context::<State>();
    rsx! {
        div { class: "tut-backdrop dim",
            div { class: "tut-card welcome", role: "dialog", "aria-modal": "true",
                div { class: "tut-logo", crate::icons::Logo { size: 40 } }
                h2 { "Have you used IwannatRust before?" }
                p { "It draws a Rust project and can tell you about it. The tutorial is itself a codecast: it talks you through the app, on this project, in about five minutes." }
                div { class: "tut-actions col",
                    button { class: "tut-primary", onclick: move |_| state.start_tutorial(), "No, show me around" }
                    button { class: "tut-quiet", onclick: move |_| state.close_welcome(), "Yes, I know my way" }
                }
            }
        }
    }
}

/// The light on the part of the app a codecast's `+` line names: the rest of the screen dims, the
/// player stays readable, the part gets a ring (`.spot-on`).
#[component]
pub fn Spot(name: String) -> Element {
    let mut rect: Signal<Option<(f64, f64, f64, f64)>> = use_signal(|| None);
    let sel = selector(&name).unwrap_or("");
    // measure again and again: the app moves under the light (panels open, views change); a new
    // name mounts a new `Spot` (keyed by it)
    use_future(move || async move {
        loop {
            let js = format!(
                "document.querySelectorAll('.spot-on').forEach(e => {{ if (!e.matches({q:?})) e.classList.remove('spot-on'); }}); const el = document.querySelector({q:?}); if (!el) return []; el.classList.add('spot-on'); const r = el.getBoundingClientRect(); return r.width > 0 ? [r.x, r.y, r.width, r.height] : [];",
                q = sel
            );
            if let Ok(v) = document::eval(&js).await {
                let n: Vec<f64> = v.as_array().map(|a| a.iter().filter_map(|x| x.as_f64()).collect()).unwrap_or_default();
                let r = (n.len() == 4).then(|| (n[0], n[1], n[2], n[3]));
                if *rect.peek() != r {
                    rect.set(r);
                }
            }
            gloo_timers::future::sleep(std::time::Duration::from_millis(250)).await;
        }
    });
    use_drop(|| {
        document::eval("document.querySelectorAll('.spot-on').forEach(e => e.classList.remove('spot-on'));");
    });
    let Some((x, y, w, h)) = *rect.read() else { return rsx! {} };
    // four dim bands around the hole (a 9999px box-shadow is not painted reliably under the
    // raised player)
    let pad = 6.0;
    let (l, t, r, b) = (x - pad, y - pad, x + w + pad, y + h + pad);
    rsx! {
        div { class: "spot-dim", style: "left:0; top:0; right:0; height:{t.max(0.0):.0}px" }
        div { class: "spot-dim", style: "left:0; top:{b:.0}px; right:0; bottom:0" }
        div { class: "spot-dim", style: "left:0; top:{t:.0}px; width:{l.max(0.0):.0}px; height:{b - t:.0}px" }
        div { class: "spot-dim", style: "left:{r:.0}px; top:{t:.0}px; right:0; height:{b - t:.0}px" }
    }
}

/// What `+ zoom` zooms onto, by priority: the glowing nodes of a graph, the highlighted blocks and
/// lines of the Code view, the highlighted files and folders.
const HIGHLIGHTS: &[&str] = &[".node.hi", ".blk.nar", ".iblk.nar", ".source-pane .line.nar", ".details .line.hl", ".ftree .frow.nar"];

/// The presenter's zoom: the middle of the screen (`.main`) is scaled and moved so that what the
/// cue talks about fills it, the top bar and the player untouched. `on` is `""` for the highlight,
/// else a panel or a spot ([`selector`]). Followed every 250 ms (views move under it).
#[component]
pub fn Zoom(on: String) -> Element {
    let sels: Vec<&str> = if on.is_empty() { HIGHLIGHTS.to_vec() } else { selector(&on).into_iter().collect() };
    let sels = serde_json::to_string(&sels).unwrap_or_else(|_| "[]".into());
    use_future(move || {
        let sels = sels.clone();
        async move {
            loop {
                let js = format!(
                    r#"const main = document.querySelector('.main'); if (main) {{
                    // measured with the transform it has right now (it may be animating)
                    const t = getComputedStyle(main).transform;
                    const k0 = t && t !== 'none' ? new DOMMatrix(t).a : 1;
                    const m = main.getBoundingClientRect();
                    const mw = m.width / k0, mh = m.height / k0;
                    let els = [];
                    for (const s of {sels}) {{ els = [...document.querySelectorAll(s)].filter(e => main.contains(e) && e.getBoundingClientRect().width > 0); if (els.length) break; }}
                    let x1 = 1e9, y1 = 1e9, x2 = -1e9, y2 = -1e9;
                    for (const e of els) {{ const r = e.getBoundingClientRect(); x1 = Math.min(x1, (r.left - m.left) / k0); y1 = Math.min(y1, (r.top - m.top) / k0); x2 = Math.max(x2, (r.right - m.left) / k0); y2 = Math.max(y2, (r.bottom - m.top) / k0); }}
                    let k = 1, tx = 0, ty = 0;
                    if (els.length) {{
                        x1 = Math.max(0, x1); y1 = Math.max(0, y1); x2 = Math.min(mw, x2); y2 = Math.min(mh, y2);
                        // a tall target (a whole panel): its top half
                        const w = Math.max(40, x2 - x1), h = Math.max(30, Math.min(y2 - y1, mh / 2));
                        k = Math.max(1, Math.min(2.2, 0.85 * mw / w, 0.85 * mh / h));
                        tx = Math.min(0, Math.max(mw - k * mw, mw / 2 - k * (x1 + x2) / 2));
                        ty = Math.min(0, Math.max(mh - k * mh, mh / 2 - k * (y1 + h / 2)));
                    }}
                    // the target it is going to
                    const kt = parseFloat(main.dataset.k || '1'), tx0 = parseFloat(main.dataset.tx || '0'), ty0 = parseFloat(main.dataset.ty || '0');
                    if (Math.abs(k - kt) > 0.02 || Math.abs(tx - tx0) > 2 || Math.abs(ty - ty0) > 2) {{
                        main.dataset.k = k; main.dataset.tx = tx; main.dataset.ty = ty;
                        main.classList.add('zoomed');
                        main.style.transform = 'translate(' + tx + 'px,' + ty + 'px) scale(' + k + ')';
                    }}
                }}"#
                );
                document::eval(&js);
                gloo_timers::future::sleep(std::time::Duration::from_millis(250)).await;
            }
        }
    });
    use_drop(|| {
        document::eval("const m = document.querySelector('.main'); if (m) { m.style.transform = ''; delete m.dataset.k; delete m.dataset.tx; delete m.dataset.ty; setTimeout(() => m.classList.remove('zoomed'), 700); }");
    });
    rsx! {}
}
