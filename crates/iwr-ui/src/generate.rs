//! The Generate page: record the codecast's voice with Kokoro through `iwr serve` (`/api/voice`).
//! Pick a language, a voice (previewed on click), the delivery and the parts, then follow the
//! recording; the job runs on the server, so it survives leaving the page or reloading it.

use crate::icons::Icon;
use crate::State;
use dioxus::prelude::*;
use iwr_core::voice::{self as v, PartStatus, VoiceJob, VoiceRequest, VoiceStatus};
use std::collections::HashSet;
use std::rc::Rc;
use std::time::Duration;

async fn fetch_status() -> Option<VoiceStatus> {
    let r = gloo_net::http::Request::get("/api/voice").send().await.ok()?;
    if !r.ok() {
        return None;
    }
    r.json().await.ok()
}

async fn fetch_job() -> Option<VoiceJob> {
    let r = gloo_net::http::Request::get("/api/voice/job").send().await.ok()?;
    if !r.ok() {
        return None;
    }
    r.json().await.ok()
}

async fn start(req: &VoiceRequest) -> Result<(), String> {
    post("/api/voice", req).await.map(|_| ())
}

/// POST JSON, answer the body (or the server's complaint).
async fn post<T: serde::Serialize>(url: &str, body: &T) -> Result<String, String> {
    let r = gloo_net::http::Request::post(url).json(body).map_err(|e| e.to_string())?.send().await.map_err(|e| e.to_string())?;
    let t = r.text().await.unwrap_or_default();
    if r.ok() {
        Ok(t)
    } else {
        Err(if t.is_empty() { r.status().to_string() } else { t })
    }
}

impl State {
    pub async fn refresh_voice_status(&mut self) {
        if let Some(s) = fetch_status().await {
            self.voice_status.set(Some(Rc::new(s)));
        }
    }

    /// Load the script again (new recordings), keeping the position.
    async fn reload_script(&mut self) {
        if self.script.peek().is_none() {
            return;
        }
        let Some((text, _, _, _)) = crate::player::fetch_script(true).await else { return };
        let (part, cue) = (*self.part_idx.peek(), *self.cue_idx.peek());
        self.load_script(iwr_core::script::parse(&text));
        let fits = self.script.peek().as_ref().and_then(|s| s.parts.get(part)).is_some_and(|p| cue < p.cues.len());
        if fits {
            self.part_idx.set(part);
            self.cue_idx.set(cue);
            self.apply_cue();
        }
    }
}

/// Fetch the recording status once the server is known, then follow a running job; when it ends,
/// refresh the status and reload the script so the new recordings play.
pub async fn watch(mut state: State) {
    let mut first = true;
    loop {
        let running = state.voice_job.peek().as_ref().is_some_and(|j| j.running);
        gloo_timers::future::sleep(Duration::from_millis(if running { 500 } else { 1500 })).await;
        if !*state.live.peek() {
            // a static export never gets a server
            if state.project.peek().is_some() {
                return;
            }
            continue;
        }
        if first {
            first = false;
            state.refresh_voice_status().await;
            state.voice_job.set(fetch_job().await.filter(|j| j.id > 0));
            continue;
        }
        if !running {
            continue;
        }
        if let Some(j) = fetch_job().await {
            let ended = !j.running;
            state.voice_job.set(Some(j));
            if ended {
                state.refresh_voice_status().await;
                state.reload_script().await;
            }
        }
    }
}

/// Parts that need a recording in a language by a voice: none yet (and no hand-made one), or
/// their text changed since.
fn needs(p: &PartStatus, code: &str, voice: &str) -> bool {
    let own = p.recordings.iter().any(|r| r.lang == code && r.state == "own");
    p.skip.is_none() && p.recording(code, voice).map_or(!own, |r| r.state == "stale")
}

/// For the top bar: English parts without an up-to-date recording, the running job's percent,
/// and the recordings whose text changed since (any language or voice).
pub fn badge(state: &State) -> (usize, Option<u32>, usize) {
    let running = state.voice_job.read().as_ref().filter(|j| j.running).map(|j| if j.total() > 0 { (j.done() * 100 / j.total()) as u32 } else { 0 });
    let todo = state.voice_status.read().as_ref().map(|s| s.parts.iter().filter(|p| p.skip.is_none() && !p.recorded_in("en")).count()).unwrap_or(0);
    let stale = state.voice_status.read().as_ref().map(|s| s.stale.iter().map(|g| g.parts.len()).sum()).unwrap_or(0);
    (todo, running, stale)
}

fn clock(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

fn size(kb: u64) -> String {
    if kb >= 1024 { format!("{:.1} MB", kb as f64 / 1024.0) } else { format!("{kb} KB") }
}

/// Where a voice comes from, by the first letter of its id.
pub fn accent(id: &str) -> &'static str {
    match id.chars().next() {
        Some('a') => "American",
        Some('b') => "British",
        Some('j') => "Japanese",
        Some('f') => "French",
        Some('e') => "Spanish",
        Some('i') => "Italian",
        Some('p') => "Brazilian",
        _ => "",
    }
}

/// A hue per voice, for its avatar.
pub fn hue(id: &str) -> u32 {
    id.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32)) % 360
}

/// Play a voice's sample: the server synthesizes it (cached); answers when playback ends.
async fn play_preview(req: &VoiceRequest) -> Result<(), String> {
    let js = format!(
        r#"try {{
             if (window.__iwrPreview) {{ window.__iwrPreview.pause(); }}
             const r = await fetch("/api/voice/preview", {{ method: "POST", headers: {{ "Content-Type": "application/json" }}, body: {} }});
             if (!r.ok) {{ dioxus.send("err:" + await r.text()); }}
             else {{
               const a = new Audio(URL.createObjectURL(await r.blob()));
               window.__iwrPreview = a;
               a.onended = () => dioxus.send("ok:");
               a.onpause = () => dioxus.send("ok:");
               await a.play();
             }}
           }} catch (e) {{ dioxus.send("err:" + e); }}"#,
        serde_json::to_string(&serde_json::to_string(req).unwrap_or_default()).unwrap_or_default()
    );
    let mut ev = document::eval(&js);
    let msg = ev.recv::<String>().await.map_err(|e| format!("{e:?}"))?;
    match msg.strip_prefix("err:") {
        Some(e) => Err(e.to_string()),
        None => Ok(()),
    }
}

fn stop_preview() {
    document::eval("if (window.__iwrPreview) { window.__iwrPreview.pause(); }");
}

#[component]
pub fn GeneratePage() -> Element {
    let mut state = use_context::<State>();
    // the status may be stale (parts edited since): refresh on opening
    use_hook(move || spawn(async move { state.refresh_voice_status().await }));
    let job = state.voice_job.read().clone();
    // the last job's settings, else US English with its default voice
    let last = job.as_ref().and_then(|j| j.request.clone());
    let mut req = use_signal(move || last.map(|r| VoiceRequest { parts: vec![], ..r }).unwrap_or_default());
    // ticked parts; `None` = the ones that need it in the picked language
    let mut picked = use_signal(|| None::<HashSet<String>>);
    // voice being previewed: (id, still loading?)
    let mut previewing = use_signal(|| None::<(String, bool)>);
    let mut message = use_signal(String::new);
    let show_log = use_signal(|| false);
    // the voice slot a click on a voice fills: 1 the main voice, 2 the second one
    let mut slot = use_signal(|| 1u8);
    // the voice being dragged onto a slot
    let mut dragging = use_signal(|| None::<&'static str>);
    // Firefox starts a drag only when it carries data
    use_hook(|| {
        document::eval(
            r#"if (!window.__iwrVoiceDrag) { window.__iwrVoiceDrag = true;
                 document.addEventListener("dragstart", (e) => { const v = e.target.closest && e.target.closest("[data-voice]");
                   if (v) { e.dataTransfer.setData("text/plain", v.dataset.voice); e.dataTransfer.effectAllowed = "copy"; } }, true); }"#,
        );
    });

    let status = state.voice_status.read().clone();
    let Some(status) = status else {
        return rsx! { div { class: "gen", div { class: "gen-wrap", div { class: "gen-loading", span { class: "gen-spin" } "Reading the codecast…" } } } };
    };
    let r = req.read().clone();
    let lang = v::voice_lang(&r.lang).copied().unwrap_or(v::VOICE_LANGS[0]);
    let code = lang.code;
    let parts = &status.parts;
    let recordable: Vec<&PartStatus> = parts.iter().filter(|p| p.skip.is_none()).collect();
    // a codecast with `sp:` lines is recorded by a pair of voices: `af_heart+am_fenrir`
    let voice2 = r.voice2.clone().unwrap_or_else(|| v::second_voice(&r.voice).to_string());
    let key = if status.second { v::pair(&r.voice, Some(&voice2)) } else { r.voice.clone() };
    let auto: HashSet<String> = parts.iter().filter(|p| needs(p, code, &key)).map(|p| p.stem.clone()).collect();
    let sel: HashSet<String> = picked.read().clone().unwrap_or_else(|| auto.clone());
    let n_sel = recordable.iter().filter(|p| sel.contains(&p.stem)).count();
    let cues_sel: usize = recordable.iter().filter(|p| sel.contains(&p.stem)).map(|p| p.cues).sum();
    // what the picked voice has recorded in the picked language
    let mine = |p: &PartStatus| p.recording(code, &key).cloned();
    let own = |p: &PartStatus| p.recordings.iter().any(|x| x.lang == code && x.state == "own");
    let count = |state: &str| parts.iter().filter(|p| mine(p).is_some_and(|x| x.state == state)).count();
    let (n_rec, n_stale) = (count("recorded"), count("stale"));
    let n_own = parts.iter().filter(|p| mine(p).is_none() && own(p)).count();
    let n_missing = recordable.iter().filter(|p| mine(p).is_none() && !own(p)).count();
    let total = parts.len().max(1);
    let audio: f64 = parts.iter().filter_map(mine).filter(|x| x.state == "recorded").filter_map(|x| x.duration).sum();
    let kb: u64 = parts.iter().filter_map(mine).map(|x| x.kb).sum();
    let running = job.as_ref().is_some_and(|j| j.running);
    let (voice_name, _) = v::voice_label(&key);

    let n_auto = auto.len();
    // record the ticked parts; `retranslate` / `fresh` skip the translation / cue clip caches
    let generate = move |retranslate: bool, fresh: bool| {
        let auto = auto.clone();
        move |_| {
            let mut r = VoiceRequest { retranslate, fresh, ..req.read().clone() };
            let sel = picked.read().clone();
            r.parts = sel.unwrap_or_else(|| auto.clone()).into_iter().collect();
            r.parts.sort();
            stop_preview();
            spawn(async move {
                message.set(String::new());
                match start(&r).await {
                    Ok(()) => {
                        picked.set(None);
                        state.voice_job.set(Some(VoiceJob { running: true, stage: "starting".into(), request: Some(VoiceRequest { retranslate: false, fresh: false, ..r.clone() }), ..Default::default() }));
                    }
                    Err(e) => message.set(e),
                }
            });
        }
    };

    rsx! {
        div { class: "gen",
            div { class: "gen-wrap",
                // ---- hero: what is there, in the picked language
                div { class: "gen-hero",
                    div { class: "gen-hero-text",
                        div { class: "gen-kicker", Icon { name: "voice", size: 14 } "Kokoro · offline text-to-speech" }
                        h2 { "Generate the voice" }
                        p { "Record every part of the codecast with a natural voice that runs on this machine. Each part becomes a small Opus file next to its script, played instead of the browser's voice — and spoken by TTS again as soon as its text changes." }
                    }
                    if status.problem.is_none() {
                        div { class: "gen-stats",
                            div { class: "gen-stat", b { "{n_rec}" span { "/{recordable.len()}" } } span { "by {voice_name}" } }
                            div { class: "gen-stat", b { "{clock(audio)}" } span { "of audio" } }
                            div { class: "gen-stat", b { "{size(kb)}" } span { "on disk" } }
                        }
                    }
                }
                if let Some(problem) = status.problem.clone() {
                    div { class: "gen-card gen-empty",
                        div { class: "gen-empty-icon", Icon { name: "voice", size: 34 } }
                        h3 { "Nothing to record yet" }
                        p { "{problem}" }
                        pre { class: "gen-tree", "codecast/\n├── index.md          title + the parts, in order\n├── 01-welcome.md     one ## part per file\n└── 02-pipeline.md" }
                        p { class: "gen-muted", "Then run iwr serve again, or record from the terminal with " code { "iwr voice codecast/" } "." }
                    }
                } else {
                    div { class: "gen-coverage", title: "{voice_name}: {n_rec} recorded · {n_stale} changed since · {n_missing} missing · {n_own} hand-made",
                        i { class: "ok", style: "flex:{n_rec}" }
                        i { class: "stale", style: "flex:{n_stale}" }
                        i { class: "own", style: "flex:{n_own}" }
                        i { class: "missing", style: "flex:{n_missing + total - total.min(n_rec + n_stale + n_own + n_missing)}" }
                    }
                    div { class: "gen-legend",
                        span { i { class: "ok" } "{n_rec} recorded by {voice_name}" }
                        span { i { class: "stale" } "{n_stale} changed since" }
                        span { i { class: "missing" } "{n_missing} missing" }
                        if n_own > 0 { span { i { class: "own" } "{n_own} hand-made" } }
                    }
                    if !status.stale.is_empty() || !status.orphans.is_empty() {
                        Outdated { status: status.clone(), running, message }
                    }
                    if !status.ready {
                        div { class: "gen-banner",
                            Icon { name: "info", size: 16 }
                            div {
                                b { "First run sets Kokoro up" }
                                span { " — a Python venv and the ~90 MB model, downloaded once into ~/.cache/iwr/kokoro. Needs python3 and curl; takes a minute or two." }
                            }
                        }
                    }

                    // ---- 1. language
                    section { class: "gen-card",
                        div { class: "gen-step", span { "1" } div { h3 { "Language" } p { "Other languages are translated offline from the English parts, and stay in sync with them." } p { class: "gen-warn", "⚠ The offline translation is rough for now. For a good one, translate with another tool (DeepL, an LLM…): " code { "iwr translate codecast/ --lang ja --pieces > ja.json" } ", fill in the values, save it as " code { "codecast/translations/ja.json" } "; those pieces win over the model." } } }
                        div { class: "gen-langs",
                            for l in v::VOICE_LANGS.iter().copied() {
                                { let done = parts.iter().filter(|p| p.recorded_in(l.code)).count();
                                  let n = recordable.len().max(1);
                                  let pct = done * 100 / n;
                                  rsx! {
                                    button { class: if l.arg == lang.arg { "gen-lang sel" } else { "gen-lang" }, disabled: running,
                                        onclick: move |_| { stop_preview(); picked.set(None); req.with_mut(|r| { r.lang = l.arg.into(); r.voice = l.default_voice.into(); r.voice2 = None; }); },
                                        span { class: "name", "{l.name}" }
                                        span { class: "bar", i { style: "width:{pct}%" } }
                                        span { class: "sub", "{done}/{recordable.len()} parts" }
                                    }
                                } }
                            }
                        }
                    }

                    // ---- 2. voice
                    section { class: "gen-card",
                        div { class: "gen-step", span { "2" } div { h3 { "Voices" } p { "Click a slot, then a voice, or drag a voice onto a slot. ▶ plays a sample. Every voice (or pair) keeps its own recordings: listeners pick one in the player." } } }
                        div { class: "gen-slots",
                            for n in [1u8, 2] {
                                { let (id, auto_pick) = if n == 1 { (r.voice.clone(), false) } else { (voice2.clone(), r.voice2.is_none()) };
                                  let (name, _) = v::voice_label(&id);
                                  let same = n == 2 && id == r.voice;
                                  let active = *slot.read() == n;
                                  let off = n == 2 && !status.second;
                                  let title = if n == 1 { "Main voice" } else { "Second voice" };
                                  let note = match (n, off, same, auto_pick) {
                                      (1, ..) => "says every line".to_string(),
                                      (_, true, _, _) => "says the sp: lines: this codecast has none yet".to_string(),
                                      (_, _, true, _) => "the sp: lines, same voice spoken higher".to_string(),
                                      (_, _, _, true) => "the sp: lines (picked for you)".to_string(),
                                      _ => "says the sp: lines".to_string(),
                                  };
                                  let class = format!("gen-slot{}{}", if active { " active" } else { "" }, if off { " off" } else { "" });
                                  rsx! {
                                    div { class: "{class}",
                                        onclick: move |_| if !running { slot.set(n) },
                                        ondragover: move |e| e.prevent_default(),
                                        ondrop: move |e| {
                                            e.prevent_default();
                                            if let Some(id) = dragging.take() {
                                                if !running {
                                                    req.with_mut(|r| if n == 1 { r.voice = id.into() } else { r.voice2 = Some(id.into()) });
                                                }
                                            }
                                        },
                                        span { class: "slot-n", "{n}" }
                                        div { class: "avatar", style: "--h:{hue(&id)}", "{name.chars().next().unwrap_or('?')}" }
                                        div { class: "who",
                                            div { class: "slot-title", "{title}" }
                                            div { class: "vname", "{name}" if same { " ↑" } }
                                            div { class: "vtags", "{note}" }
                                        }
                                        if n == 2 && !auto_pick {
                                            button { class: "slot-x", title: "pick for me", disabled: running,
                                                onclick: move |e| { e.stop_propagation(); req.with_mut(|r| r.voice2 = None); }, "✕" }
                                        }
                                    }
                                } }
                            }
                            button { class: "gen-swap", title: "swap the two voices", disabled: running,
                                onclick: { let v2 = voice2.clone(); move |_| { let v2 = v2.clone(); req.with_mut(|r| { let v1 = std::mem::replace(&mut r.voice, v2); r.voice2 = Some(v1); }); } },
                                "⇄" }
                        }
                        div { class: "gen-voices",
                            for id in v::voices_for(&lang) {
                                { let (name, female) = v::voice_label(id);
                                  let featured = v::FEATURED.contains(&id);
                                  let sel = r.voice == id;
                                  let sel2 = voice2 == id;
                                  let has = parts.iter().filter(|p| p.recordings.iter().any(|x| x.lang == code && x.state == "recorded" && x.voice.as_deref().is_some_and(|w| v::split_pair(w).0 == id))).count();
                                  let pv = previewing.read().clone().filter(|(p, _)| p == id);
                                  let h = hue(id);
                                  let initial = name.chars().next().unwrap_or('?');
                                  rsx! {
                                    div { class: if sel { "gen-voice sel" } else if sel2 { "gen-voice sel2" } else { "gen-voice" },
                                        draggable: !running, "data-voice": id,
                                        ondragstart: move |_| dragging.set(Some(id)),
                                        ondragend: move |_| dragging.set(None),
                                        onclick: move |_| if !running { req.with_mut(|r| if *slot.peek() == 1 { r.voice = id.into() } else { r.voice2 = Some(id.into()) }) },
                                        div { class: "avatar", style: "--h:{h}", "{initial}" }
                                        if sel || sel2 {
                                            span { class: "vslot", if sel && sel2 { "1·2" } else if sel { "1" } else { "2" } }
                                        }
                                        div { class: "who",
                                            div { class: "vname", "{name}" if featured { span { class: "star", title: "one of the best Kokoro voices", "★" } } }
                                            div { class: "vtags", if female { "♀ " } else { "♂ " } "{accent(id)}"
                                                if has > 0 { span { class: "vhas", title: "parts recorded with this voice", "● {has}/{recordable.len()}" } }
                                            }
                                        }
                                        button { class: match &pv { Some((_, true)) => "play loading", Some(_) => "play on", None => "play" }, title: "hear a sample",
                                            onclick: move |e| {
                                                e.stop_propagation();
                                                if previewing.read().as_ref().is_some_and(|(p, _)| p == id) {
                                                    stop_preview();
                                                    previewing.set(None);
                                                    return;
                                                }
                                                let pr = VoiceRequest { voice: id.into(), parts: vec![], ..req.read().clone() };
                                                previewing.set(Some((id.into(), true)));
                                                spawn(async move {
                                                    // flips to "playing" once the sample arrives
                                                    let mut ready = document::eval(r#"const t = setInterval(() => { if (window.__iwrPreview && !window.__iwrPreview.paused) { clearInterval(t); dioxus.send(1); } }, 100);"#);
                                                    spawn(async move {
                                                        if ready.recv::<u32>().await.is_ok() && previewing.peek().as_ref().is_some_and(|(p, l)| p == id && *l) {
                                                            previewing.set(Some((id.into(), false)));
                                                        }
                                                    });
                                                    let res = play_preview(&pr).await;
                                                    if previewing.peek().as_ref().is_some_and(|(p, _)| p == id) {
                                                        previewing.set(None);
                                                    }
                                                    if let Err(e) = res {
                                                        message.set(format!("preview: {e}"));
                                                    }
                                                });
                                            },
                                            match &pv { Some((_, true)) => rsx! { span { class: "gen-spin small" } }, Some(_) => rsx! { "■" }, None => rsx! { "▶" } }
                                        }
                                    }
                                } }
                            }
                        }
                    }

                    // ---- 3. delivery
                    section { class: "gen-card",
                        div { class: "gen-step", span { "3" } div { h3 { "Delivery" } p { "How it is spoken and stored." } } }
                        div { class: "gen-sliders",
                            label { class: "gen-slider",
                                div { span { "Speed" } b { "{r.speed:.2}×" } }
                                input { r#type: "range", min: "0.7", max: "1.4", step: "0.05", value: "{r.speed}", disabled: running, oninput: move |e| if let Ok(x) = e.value().parse() { req.with_mut(|r| r.speed = x) } }
                                div { class: "ends", span { "calm" } span { "brisk" } }
                            }
                            label { class: "gen-slider",
                                div { span { "Pause between cues" } b { "{r.gap:.1} s" } }
                                input { r#type: "range", min: "0", max: "1.5", step: "0.1", value: "{r.gap}", disabled: running, oninput: move |e| if let Ok(x) = e.value().parse() { req.with_mut(|r| r.gap = x) } }
                                div { class: "ends", span { "tight" } span { "airy" } }
                            }
                            label { class: "gen-slider",
                                div { span { "Compression" } b { "{r.level:.2}" } }
                                input { r#type: "range", min: "0.5", max: "1", step: "0.05", value: "{r.level}", disabled: running, oninput: move |e| if let Ok(x) = e.value().parse() { req.with_mut(|r| r.level = x) } }
                                div { class: "ends", span { "cleaner" } span { "smaller (0.95 ≈ 18 kbps)" } }
                            }
                        }
                    }

                    // ---- 4. parts
                    section { class: "gen-card",
                        div { class: "gen-step", span { "4" }
                            div { h3 { "Parts" } p { "Ticked parts are recorded, up to date or not. Unchanged cues come from the cache, instantly." } }
                            div { class: "gen-quick",
                                button { class: if picked.read().is_none() { "sel" } else { "" }, disabled: running, onclick: move |_| picked.set(None), "Needs it ({n_auto})" }
                                button { disabled: running, onclick: { let all: HashSet<String> = recordable.iter().map(|p| p.stem.clone()).collect(); move |_| picked.set(Some(all.clone())) }, "All" }
                                button { disabled: running, onclick: move |_| picked.set(Some(HashSet::new())), "None" }
                            }
                        }
                        div { class: "gen-parts",
                            for (i, p) in parts.iter().enumerate() {
                                { let rec = mine(p);
                                  let others: Vec<String> = p.recordings.iter().filter(|x| x.lang == code && x.state == "recorded").filter_map(|x| x.voice.clone()).filter(|x| *x != key).collect();
                                  let own = own(p);
                                  let stem = p.stem.clone();
                                  let sel = sel.clone();
                                  let on = sel.contains(&p.stem) && p.skip.is_none();
                                  let (pill, pill_class) = match (&p.skip, rec.as_ref().map(|r| r.state.as_str())) {
                                      (Some(_), _) => ("can't record", "skip"),
                                      (_, Some("recorded")) => ("recorded", "ok"),
                                      (_, Some("stale")) => ("text changed", "stale"),
                                      (_, _) if own => ("hand-made", "own"),
                                      (_, _) => ("missing", "missing"),
                                  };
                                  let meta = match &rec {
                                      Some(r) => {
                                          let who = r.voice.as_deref().map(|v| v::voice_label(v).0).unwrap_or_default();
                                          let d = r.duration.map(clock).unwrap_or_default();
                                          [who, d, size(r.kb)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")
                                      }
                                      None => p.skip.clone().unwrap_or_default(),
                                  };
                                  rsx! {
                                    label { class: if p.skip.is_some() { "gen-part off" } else if on { "gen-part on" } else { "gen-part" },
                                        input { r#type: "checkbox", checked: on, disabled: p.skip.is_some() || running,
                                            onchange: move |e| {
                                                let mut s = picked.read().clone().unwrap_or_else(|| sel.clone());
                                                if e.checked() { s.insert(stem.clone()); } else { s.remove(&stem); }
                                                picked.set(Some(s));
                                            } }
                                        span { class: "num", "{i + 1:02}" }
                                        div { class: "what",
                                            div { class: "pname", if p.name.is_empty() { "{p.stem}" } else { "{p.name}" } }
                                            div { class: "pmeta", code { "{p.stem}.md" } " · {p.cues} cues" if !meta.is_empty() { " · {meta}" } }
                                        }
                                        if !others.is_empty() {
                                            span { class: "gen-others", title: "also recorded by {others.iter().map(|o| v::voice_label(o).0).collect::<Vec<_>>().join(\", \")}",
                                                for o in others.iter() {
                                                    span { class: "avatar", style: "--h:{hue(o)}", "{v::voice_label(o).0.chars().next().unwrap_or('?')}" }
                                                }
                                            }
                                        }
                                        span { class: "gen-pill {pill_class}", "{pill}" }
                                    }
                                } }
                            }
                        }
                    }
                }
            }

            // ---- the action bar, or the running job
            if status.problem.is_none() {
                div { class: "gen-dock",
                    match job.clone() {
                        Some(j) if j.running || j.id > 0 => rsx! { Progress { job: j, show_log } },
                        _ => rsx! {},
                    }
                    if !running {
                        div { class: "gen-action",
                            div { class: "gen-summary",
                                b { {if n_sel == 0 { "Nothing ticked".to_string() } else { format!("{n_sel} part(s) · {cues_sel} cues") }} }
                                span { "{voice_name} · {lang.name} · {r.speed:.2}×" }
                                if !message.read().is_empty() { span { class: "gen-error", "{message}" } }
                            }
                            div { class: "gen-buttons",
                                if code != "en" {
                                    button { class: "gen-again", disabled: n_sel == 0, onclick: generate(true, false),
                                        title: "translate the ticked parts again (not from the cache), then record them",
                                        "↻ Retranslate"
                                    }
                                }
                                button { class: "gen-again", disabled: n_sel == 0, onclick: generate(false, true),
                                    title: "synthesize every cue of the ticked parts again (not from the cache)",
                                    "↻ Rerecord"
                                }
                                button { class: "gen-go", disabled: n_sel == 0, onclick: generate(false, false),
                                    Icon { name: "voice", size: 18 }
                                    if n_sel == 0 { "Generate" } else { "Generate {n_sel} part(s)" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The running (or last) job: a bar over every cue, the parts with their state, cancel / done.
#[component]
fn Progress(job: VoiceJob, show_log: Signal<bool>) -> Element {
    let mut state = use_context::<State>();
    let mut show_log = show_log;
    let (done, total) = (job.done(), job.total());
    let pct = if total > 0 { done as f64 * 100.0 / total as f64 } else { 0.0 };
    let indeterminate = job.running && total == 0;
    let eta = (job.running && done > 0 && done < total).then(|| job.elapsed / done as f64 * (total - done) as f64);
    let cur = job.current();
    let failed = job.error.is_some() && job.stage != "cancelled";
    let audio: f64 = job.parts.iter().filter_map(|p| p.duration).sum();
    let kb: u64 = job.parts.iter().map(|p| p.kb).sum();
    let n_done = job.parts.iter().filter(|p| p.duration.is_some()).count();
    let class = if job.running { "gen-job running" } else if failed { "gen-job failed" } else if job.stage == "cancelled" { "gen-job cancelled" } else { "gen-job done" };
    let title = if job.running {
        match cur.and_then(|i| job.parts.get(i)) {
            Some(p) if job.stage == "recording" => format!("Recording {}", p.stem),
            _ => job.stage.clone(),
        }
    } else if failed {
        "Recording failed".into()
    } else if job.stage == "cancelled" {
        format!("Cancelled — {n_done} part(s) kept")
    } else if job.parts.is_empty() {
        "Everything was already up to date".into()
    } else {
        format!("{n_done} part(s) recorded · {} of audio · {}", clock(audio), size(kb))
    };
    rsx! {
        div { class: "{class}",
            div { class: "gen-job-head",
                if job.running { span { class: "gen-spin" } } else if failed { span { class: "gen-badge bad", "!" } } else { span { class: "gen-badge good", "✓" } }
                div { class: "gen-job-title",
                    b { "{title}" }
                    span {
                        if total > 0 { "cue {done}/{total} · " }
                        "{clock(job.elapsed)} elapsed"
                        if let Some(e) = eta { " · about {clock(e)} left" }
                    }
                }
                if job.running {
                    b { class: "gen-pct", "{pct:.0}%" }
                    button { class: "gen-cancel", onclick: move |_| { spawn(async move { let _ = gloo_net::http::Request::post("/api/voice/cancel").send().await; }); }, "Cancel" }
                } else {
                    if !failed && n_done > 0 {
                        button { class: "gen-play", onclick: move |_| { state.close_pages(); state.start_tour(); }, Icon { name: "codecast", size: 14 } "Play the codecast" }
                    }
                    button { class: "gen-dismiss", title: "hide", onclick: move |_| state.voice_job.set(None), "✕" }
                }
            }
            div { class: if indeterminate { "gen-bar indeterminate" } else { "gen-bar" },
                i { style: "width:{pct}%" }
            }
            if !job.parts.is_empty() {
                div { class: "gen-chips",
                    for (i, p) in job.parts.iter().enumerate() {
                        { let st = if p.duration.is_some() { "done" } else if job.running && Some(i) == cur { "cur" } else { "wait" };
                          let w = if p.cues > 0 { p.done * 100 / p.cues } else { 0 };
                          rsx! {
                            div { class: "gen-chip {st}", title: "{p.done}/{p.cues} cues",
                                span { class: "mark", match st { "done" => "✓", "cur" => "●", _ => "○" } }
                                span { "{p.stem}" }
                                if let Some(d) = p.duration { em { "{clock(d)}" } }
                                if st == "cur" { i { style: "width:{w}%" } }
                            }
                        } }
                    }
                }
            }
            if let Some(e) = &job.error {
                if failed { pre { class: "gen-err", "{e}" } }
            }
            if !job.log.is_empty() {
                button { class: "gen-logbtn", onclick: move |_| { let v = *show_log.read(); show_log.set(!v); }, if *show_log.read() { "hide details ▾" } else { "details ({job.log.len()}) ▸" } }
                if *show_log.read() {
                    pre { class: "gen-log", for l in job.log.iter() { "{l}\n" } }
                }
            }
        }
    }
}

/// Recordings whose text changed since, by voice, and files of parts that are gone: record them
/// again (same voice and delivery) or delete them.
#[component]
fn Outdated(status: Rc<VoiceStatus>, running: bool, message: Signal<String>) -> Element {
    let mut state = use_context::<State>();
    let mut message = message;
    // the delete button asks once more: which group (`*` = all) is waiting for that second click
    let mut confirm = use_signal(|| None::<String>);
    let n_rec: usize = status.stale.iter().map(|g| g.parts.len()).sum();
    let n_orphans = status.orphans.iter().filter(|f| f.ends_with(".times")).count();
    let refresh = move |groups: Vec<String>| {
        spawn(async move {
            message.set(String::new());
            match post("/api/voice/refresh", &v::CleanRequest { groups }).await {
                Ok(_) => state.voice_job.set(Some(VoiceJob { running: true, stage: "starting".into(), ..Default::default() })),
                Err(e) => message.set(e),
            }
        });
    };
    let mut clean = move |groups: Vec<String>| {
        confirm.set(None);
        spawn(async move {
            match post("/api/voice/clean", &v::CleanRequest { groups }).await {
                Ok(_) => {
                    state.refresh_voice_status().await;
                    state.reload_script().await;
                }
                Err(e) => message.set(e),
            }
        });
    };
    let lang_name = |code: &str| v::VOICE_LANGS.iter().find(|l| l.code == code).map(|l| l.name.to_string()).unwrap_or_else(|| code.to_string());
    let all_confirm = confirm.read().as_deref() == Some("*");
    rsx! {
        section { class: "gen-card gen-outdated",
            div { class: "gen-out-head",
                span { class: "gen-out-icon", "↻" }
                div {
                    h3 { if n_rec > 0 { "{n_rec} recording(s) out of date: their text changed" } else { "Old recordings to clean up" } }
                    p {
                        if n_rec > 0 { "Those parts are spoken by the browser voice until they are recorded again. " }
                        if n_orphans > 0 { "{n_orphans} recording(s) belong to parts that no longer exist." }
                    }
                }
            }
            div { class: "gen-out-list",
                for g in status.stale.iter().cloned() {
                    { let key = g.key();
                      let (name, _) = v::voice_label(&g.voice);
                      let asking = confirm.read().as_deref() == Some(key.as_str());
                      let parts = g.parts.join(", ");
                      let (k1, k2, k3) = (key.clone(), key.clone(), key.clone());
                      rsx! {
                        div { class: "gen-out-row",
                            span { class: "avatar", style: "--h:{hue(&g.voice)}", "{name.chars().next().unwrap_or('?')}" }
                            div { class: "what",
                                b { "{name} · {lang_name(&g.lang)}" }
                                span { title: "{parts}", "{g.parts.len()} part(s): {parts}" }
                            }
                            button { class: "gen-mini go", disabled: running, onclick: move |_| refresh(vec![k1.clone()]), "↻ Regenerate" }
                            if asking {
                                button { class: "gen-mini danger", disabled: running, onclick: move |_| clean(vec![k2.clone()]), "Delete {g.parts.len()} file(s)?" }
                            } else {
                                button { class: "gen-mini", disabled: running, onclick: move |_| confirm.set(Some(k3.clone())), "Delete" }
                            }
                        }
                    } }
                }
                if n_orphans > 0 {
                    div { class: "gen-out-row",
                        span { class: "avatar trash", "🗑" }
                        div { class: "what",
                            b { "Parts that are gone" }
                            span { title: "{status.orphans.join(\", \")}", "{status.orphans.join(\", \")}" }
                        }
                        span { class: "gen-out-note", "deleted with either action below" }
                    }
                }
            }
            div { class: "gen-out-foot",
                if n_rec > 0 {
                    button { class: "gen-mini go big", disabled: running, onclick: move |_| refresh(vec![]), "↻ Regenerate all out-of-date" }
                }
                if all_confirm {
                    button { class: "gen-mini danger", disabled: running, onclick: move |_| clean(vec![]), "Yes, delete {n_rec + n_orphans} recording(s)" }
                    button { class: "gen-mini", onclick: move |_| confirm.set(None), "Keep them" }
                } else {
                    button { class: "gen-mini", disabled: running, onclick: move |_| confirm.set(Some("*".into())), if n_rec > 0 { "Delete all out-of-date" } else { "Delete them" } }
                }
            }
        }
    }
}
