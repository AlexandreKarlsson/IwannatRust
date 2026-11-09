//! The Generate page: record the codecast's voice with Kokoro through `codecast serve` (`/api/voice`).
//! Pick a language, a voice (previewed on click), the delivery and the parts, then follow the
//! recording; the job runs on the server, so it survives leaving the page or reloading it.

use crate::i18n::t;
use crate::icons::Icon;
use crate::subtitles::Cmd;
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

    /// Load the script again (new recordings): the copy the codecast button plays, and the player
    /// when it shows the project's codecast (not the tutorial or a guide), keeping the position.
    async fn reload_script(&mut self) {
        let Some((text, audio, base, _)) = crate::player::fetch_script(true).await else { return };
        let s = iwr_core::script::parse(&text);
        let old = self.own_script.peek().as_ref().map(|(o, _, _)| o.title.clone());
        self.own_script.set(Some((Rc::new(s.clone()), audio.clone(), base.clone())));
        let shows_own = self.script_src.peek().as_ref().is_some_and(|p| Some(&p.title) == old.as_ref());
        if self.script.peek().is_none() || !shows_own {
            return;
        }
        let (part, cue) = (*self.part_idx.peek(), *self.cue_idx.peek());
        self.audio_url.set(audio);
        self.audio_base.set(base);
        self.load_script(s);
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

/// `m:ss`, or `h:mm:ss` from an hour on.
pub fn clock(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

/// A length from [`iwr_core::script::Part::duration`]: `1:23`, or `≈ 1:23` when guessed from the words.
pub fn length((secs, measured): (f64, bool)) -> String {
    if measured { clock(secs) } else { format!("≈ {}", clock(secs)) }
}

fn size(kb: u64) -> String {
    if kb >= 1024 { format!("{:.1} MB", kb as f64 / 1024.0) } else { format!("{kb} KB") }
}

/// Where a voice comes from, by the first letter of its id.
pub fn accent(id: &str) -> &'static str {
    match id.chars().next() {
        Some('a') => t::generate::accent_american(),
        Some('b') => t::generate::accent_british(),
        Some('j') => t::generate::accent_japanese(),
        Some('f') => t::generate::accent_french(),
        Some('e') => t::generate::accent_spanish(),
        Some('i') => t::generate::accent_italian(),
        Some('p') => t::generate::accent_brazilian(),
        _ => "",
    }
}

/// A recording language's name in the interface language (`VOICE_LANGS` names English ones in
/// English; the others in their own language, kept as they are).
fn voice_lang_name(l: &v::VoiceLang) -> &'static str {
    match l.arg {
        "en-us" => t::generate::voice_lang_en_us(),
        "en-gb" => t::generate::voice_lang_en_gb(),
        _ => l.name,
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
        return rsx! { div { class: "gen", div { class: "gen-wrap", div { class: "gen-loading", span { class: "gen-spin" } {t::generate::reading()} } } } };
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
                        div { class: "gen-kicker", Icon { name: "voice", size: 14 } {t::generate::kicker()} }
                        h2 { {t::generate::title()} }
                        p { {t::generate::intro()} }
                    }
                    if status.problem.is_none() {
                        div { class: "gen-stats",
                            div { class: "gen-stat", b { "{n_rec}" span { "/{recordable.len()}" } } span { {t::generate::stat_by(&voice_name)} } }
                            div { class: "gen-stat", b { "{clock(audio)}" } span { {t::generate::stat_audio()} } }
                            div { class: "gen-stat", b { "{size(kb)}" } span { {t::generate::stat_disk()} } }
                        }
                    }
                }
                if let Some(problem) = status.problem.clone() {
                    div { class: "gen-card gen-empty",
                        div { class: "gen-empty-icon", Icon { name: "voice", size: 34 } }
                        h3 { {t::generate::empty_title()} }
                        p { {match problem {
                            iwr_core::voice::VoiceProblem::NoCodecast => t::generate::problem_no_codecast().to_string(),
                            iwr_core::voice::VoiceProblem::SingleFile { path } => t::generate::problem_single_file(path),
                        }} }
                        pre { class: "gen-tree", {t::generate::empty_tree()} }
                        p { class: "gen-muted", Cmd { text: t::generate::empty_then().to_string() } }
                    }
                } else {
                    div { class: "gen-coverage", title: t::generate::coverage_tip(&voice_name, n_rec, n_stale, n_missing, n_own),
                        i { class: "ok", style: "flex:{n_rec}" }
                        i { class: "stale", style: "flex:{n_stale}" }
                        i { class: "own", style: "flex:{n_own}" }
                        i { class: "missing", style: "flex:{n_missing + total - total.min(n_rec + n_stale + n_own + n_missing)}" }
                    }
                    div { class: "gen-legend",
                        span { i { class: "ok" } {t::generate::coverage_recorded(n_rec, &voice_name)} }
                        span { i { class: "stale" } {t::generate::coverage_stale(n_stale)} }
                        span { i { class: "missing" } {t::generate::coverage_missing(n_missing)} }
                        if n_own > 0 { span { i { class: "own" } {t::generate::coverage_own(n_own)} } }
                    }
                    if !status.stale.is_empty() || !status.orphans.is_empty() {
                        Outdated { status: status.clone(), running, message }
                    }
                    if !status.sets.is_empty() {
                        Sets { status: status.clone(), running, current: format!("{}/{}", code, key), req, picked, message }
                    }
                    if !status.ready {
                        div { class: "gen-banner",
                            Icon { name: "info", size: 16 }
                            div {
                                b { {t::generate::banner_title()} }
                                span { {t::generate::banner_text()} }
                            }
                        }
                    }

                    // ---- 1. language
                    section { class: "gen-card",
                        div { class: "gen-step", span { "1" } div { h3 { {t::generate::language_title()} } p { {t::generate::language_text()} } p { class: "gen-warn", Cmd { text: t::generate::language_warn().to_string() } } } }
                        div { class: "gen-langs",
                            for l in v::VOICE_LANGS.iter().copied() {
                                { let done = parts.iter().filter(|p| p.recorded_in(l.code)).count();
                                  let n = recordable.len().max(1);
                                  let pct = done * 100 / n;
                                  rsx! {
                                    button { class: if l.arg == lang.arg { "gen-lang sel" } else { "gen-lang" }, disabled: running,
                                        onclick: move |_| { stop_preview(); picked.set(None); req.with_mut(|r| { r.lang = l.arg.into(); r.voice = l.default_voice.into(); r.voice2 = None; }); },
                                        span { class: "name", {voice_lang_name(&l)} }
                                        span { class: "bar", i { style: "width:{pct}%" } }
                                        span { class: "sub", {t::generate::language_done(recordable.len(), done)} }
                                    }
                                } }
                            }
                        }
                    }

                    // ---- 2. voice
                    section { class: "gen-card",
                        div { class: "gen-step", span { "2" } div { h3 { {t::generate::voices_title()} } p { {t::generate::voices_text()} } } }
                        div { class: "gen-slots",
                            for n in [1u8, 2] {
                                { let (id, auto_pick) = if n == 1 { (r.voice.clone(), false) } else { (voice2.clone(), r.voice2.is_none()) };
                                  let (name, _) = v::voice_label(&id);
                                  let same = n == 2 && id == r.voice;
                                  let active = *slot.read() == n;
                                  let off = n == 2 && !status.second;
                                  let title = if n == 1 { t::generate::voices_main() } else { t::generate::voices_second() };
                                  let note = match (n, off, same, auto_pick) {
                                      (1, ..) => t::generate::voices_note_main(),
                                      (_, true, _, _) => t::generate::voices_note_none(),
                                      (_, _, true, _) => t::generate::voices_note_same(),
                                      (_, _, _, true) => t::generate::voices_note_auto(),
                                      _ => t::generate::voices_note_second(),
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
                                            button { class: "slot-x", title: t::generate::voices_pick_for_me(), disabled: running,
                                                onclick: move |e| { e.stop_propagation(); req.with_mut(|r| r.voice2 = None); }, "✕" }
                                        }
                                    }
                                } }
                            }
                            button { class: "gen-swap", title: t::generate::voices_swap(), disabled: running,
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
                                            div { class: "vname", "{name}" if featured { span { class: "star", title: t::generate::voices_best(), "★" } } }
                                            div { class: "vtags", if female { "♀ " } else { "♂ " } "{accent(id)}"
                                                if has > 0 { span { class: "vhas", title: t::generate::voices_has(), "● {has}/{recordable.len()}" } }
                                            }
                                        }
                                        button { class: match &pv { Some((_, true)) => "play loading", Some(_) => "play on", None => "play" }, title: t::generate::voices_sample(),
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
                                                        message.set(t::generate::voices_preview_error(e));
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
                        div { class: "gen-step", span { "3" } div { h3 { {t::generate::delivery_title()} } p { {t::generate::delivery_text()} } } }
                        div { class: "gen-sliders",
                            label { class: "gen-slider",
                                div { span { {t::generate::delivery_speed()} } b { "{r.speed:.2}×" } }
                                input { r#type: "range", min: "0.7", max: "1.4", step: "0.05", value: "{r.speed}", disabled: running, oninput: move |e| if let Ok(x) = e.value().parse() { req.with_mut(|r| r.speed = x) } }
                                div { class: "ends", span { {t::generate::delivery_calm()} } span { {t::generate::delivery_brisk()} } }
                            }
                            label { class: "gen-slider",
                                div { span { {t::generate::delivery_gap()} } b { "{r.gap:.1} s" } }
                                input { r#type: "range", min: "0", max: "1.5", step: "0.1", value: "{r.gap}", disabled: running, oninput: move |e| if let Ok(x) = e.value().parse() { req.with_mut(|r| r.gap = x) } }
                                div { class: "ends", span { {t::generate::delivery_tight()} } span { {t::generate::delivery_airy()} } }
                            }
                            label { class: "gen-slider",
                                div { span { {t::generate::delivery_compression()} } b { "{r.level:.2}" } }
                                input { r#type: "range", min: "0.5", max: "1", step: "0.05", value: "{r.level}", disabled: running, oninput: move |e| if let Ok(x) = e.value().parse() { req.with_mut(|r| r.level = x) } }
                                div { class: "ends", span { {t::generate::delivery_cleaner()} } span { {t::generate::delivery_smaller()} } }
                            }
                        }
                    }

                    // ---- 4. parts
                    section { class: "gen-card",
                        div { class: "gen-step", span { "4" }
                            div { h3 { {t::generate::pick_title()} } p { {t::generate::pick_text()} } }
                            div { class: "gen-quick",
                                button { class: if picked.read().is_none() { "sel" } else { "" }, disabled: running, onclick: move |_| picked.set(None), {t::generate::pick_needs(n_auto)} }
                                button { disabled: running, onclick: { let all: HashSet<String> = recordable.iter().map(|p| p.stem.clone()).collect(); move |_| picked.set(Some(all.clone())) }, {t::generate::pick_all()} }
                                button { disabled: running, onclick: move |_| picked.set(Some(HashSet::new())), {t::generate::pick_none()} }
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
                                      (Some(_), _) => (t::generate::pill_skip(), "skip"),
                                      (_, Some("recorded")) => (t::generate::pill_recorded(), "ok"),
                                      (_, Some("stale")) => (t::generate::pill_stale(), "stale"),
                                      (_, _) if own => (t::generate::pill_own(), "own"),
                                      (_, _) => (t::generate::pill_missing(), "missing"),
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
                                            div { class: "pmeta", code { "{p.stem}.md" } " · {t::generate::cues(p.cues)}" if !meta.is_empty() { " · {meta}" } }
                                        }
                                        if !others.is_empty() {
                                            span { class: "gen-others", title: t::generate::pick_also_by(others.iter().map(|o| v::voice_label(o).0).collect::<Vec<_>>().join(", ")),
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
                                b { {if n_sel == 0 { t::generate::action_nothing().to_string() } else { format!("{} · {}", t::generate::parts(n_sel), t::generate::cues(cues_sel)) }} }
                                span { "{voice_name} · {voice_lang_name(&lang)} · {r.speed:.2}×" }
                                if !message.read().is_empty() { span { class: "gen-error", "{message}" } }
                            }
                            div { class: "gen-buttons",
                                if code != "en" {
                                    button { class: "gen-again", disabled: n_sel == 0, onclick: generate(true, false),
                                        title: t::generate::action_retranslate_tip(),
                                        {t::generate::action_retranslate()}
                                    }
                                }
                                button { class: "gen-again", disabled: n_sel == 0, onclick: generate(false, true),
                                    title: t::generate::action_rerecord_tip(),
                                    {t::generate::action_rerecord()}
                                }
                                button { class: "gen-go", disabled: n_sel == 0, onclick: generate(false, false),
                                    Icon { name: "voice", size: 18 }
                                    if n_sel == 0 { {t::generate::action_generate()} } else { {t::generate::action_generate_n(n_sel)} }
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
            Some(p) if job.stage == "recording" => t::generate::job_recording(&p.stem),
            // the stage the page sets itself; the server's own stages show as it writes them
            _ if job.stage == "starting" => t::generate::job_starting().to_string(),
            _ => job.stage.clone(),
        }
    } else if failed {
        t::generate::job_failed().into()
    } else if job.stage == "cancelled" {
        t::generate::job_cancelled(n_done)
    } else if job.parts.is_empty() {
        t::generate::job_up_to_date().into()
    } else {
        t::generate::job_done(n_done, clock(audio), size(kb))
    };
    rsx! {
        div { class: "{class}",
            div { class: "gen-job-head",
                if job.running { span { class: "gen-spin" } } else if failed { span { class: "gen-badge bad", "!" } } else { span { class: "gen-badge good", "✓" } }
                div { class: "gen-job-title",
                    b { "{title}" }
                    span {
                        if total > 0 { {t::generate::job_cue(done, total)} }
                        {t::generate::job_elapsed(clock(job.elapsed))}
                        if let Some(e) = eta { {t::generate::job_left(clock(e))} }
                    }
                }
                if job.running {
                    b { class: "gen-pct", "{pct:.0}%" }
                    button { class: "gen-cancel", onclick: move |_| { spawn(async move { let _ = gloo_net::http::Request::post("/api/voice/cancel").send().await; }); }, {t::generate::job_cancel()} }
                } else {
                    if !failed && n_done > 0 {
                        button { class: "gen-play", onclick: move |_| { state.close_pages(); state.start_tour(); }, Icon { name: "codecast", size: 14 } {t::generate::job_play()} }
                    }
                    button { class: "gen-dismiss", title: t::generate::job_hide(), onclick: move |_| state.voice_job.set(None), "✕" }
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
                            div { class: "gen-chip {st}", title: t::generate::job_chip(p.cues, p.done),
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
                button { class: "gen-logbtn", onclick: move |_| { let v = *show_log.read(); show_log.set(!v); }, if *show_log.read() { {t::generate::job_hide_details()} } else { {t::generate::job_details(job.log.len())} } }
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
    let lang_name = |code: &str| v::VOICE_LANGS.iter().find(|l| l.code == code).map(|l| voice_lang_name(l).to_string()).unwrap_or_else(|| code.to_string());
    let all_confirm = confirm.read().as_deref() == Some("*");
    rsx! {
        section { class: "gen-card gen-outdated",
            div { class: "gen-out-head",
                span { class: "gen-out-icon", "↻" }
                div {
                    h3 { if n_rec > 0 { {t::generate::outdated_title(n_rec)} } else { {t::generate::outdated_clean_title()} } }
                    p {
                        if n_rec > 0 { {t::generate::outdated_browser()} }
                        if n_orphans > 0 { {t::generate::outdated_orphans(n_orphans)} }
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
                                span { title: "{parts}", {t::generate::outdated_group(g.parts.len(), &parts)} }
                            }
                            button { class: "gen-mini go", disabled: running, onclick: move |_| refresh(vec![k1.clone()]), {t::generate::outdated_regenerate()} }
                            if asking {
                                button { class: "gen-mini danger", disabled: running, onclick: move |_| clean(vec![k2.clone()]), {t::generate::outdated_delete_files(g.parts.len())} }
                            } else {
                                button { class: "gen-mini", disabled: running, onclick: move |_| confirm.set(Some(k3.clone())), {t::generate::outdated_delete()} }
                            }
                        }
                    } }
                }
                if n_orphans > 0 {
                    div { class: "gen-out-row",
                        span { class: "avatar trash", "🗑" }
                        div { class: "what",
                            b { {t::generate::outdated_gone()} }
                            span { title: "{status.orphans.join(\", \")}", "{status.orphans.join(\", \")}" }
                        }
                        span { class: "gen-out-note", {t::generate::outdated_gone_note()} }
                    }
                }
            }
            div { class: "gen-out-foot",
                if n_rec > 0 {
                    button { class: "gen-mini go big", disabled: running, onclick: move |_| refresh(vec![]), {t::generate::outdated_regenerate_all()} }
                }
                if all_confirm {
                    button { class: "gen-mini danger", disabled: running, onclick: move |_| clean(vec![]), {t::generate::outdated_confirm_all(n_rec + n_orphans)} }
                    button { class: "gen-mini", onclick: move |_| confirm.set(None), {t::generate::outdated_keep()} }
                } else {
                    button { class: "gen-mini", disabled: running, onclick: move |_| confirm.set(Some("*".into())), if n_rec > 0 { {t::generate::outdated_delete_all()} } else { {t::generate::outdated_delete_them()} } }
                }
            }
        }
    }
}

/// Every voice (or pair) that has recordings, per language: pick it in the form, record what it
/// lacks (changed and missing parts) in one click, or delete all its files.
#[component]
fn Sets(status: Rc<VoiceStatus>, running: bool, current: String, req: Signal<VoiceRequest>, picked: Signal<Option<HashSet<String>>>, message: Signal<String>) -> Element {
    let mut state = use_context::<State>();
    let (mut req, mut picked, mut message) = (req, picked, message);
    // the delete button asks once more: the set waiting for that second click
    let mut confirm = use_signal(|| None::<String>);
    let complete = move |groups: Vec<String>| {
        spawn(async move {
            message.set(String::new());
            match post("/api/voice/complete", &v::CleanRequest { groups }).await {
                Ok(_) => state.voice_job.set(Some(VoiceJob { running: true, stage: "starting".into(), ..Default::default() })),
                Err(e) => message.set(e),
            }
        });
    };
    let mut remove = move |key: String| {
        confirm.set(None);
        spawn(async move {
            match post("/api/voice/drop", &v::CleanRequest { groups: vec![key] }).await {
                Ok(_) => {
                    state.refresh_voice_status().await;
                    state.reload_script().await;
                }
                Err(e) => message.set(e),
            }
        });
    };
    let lang_name = |code: &str| v::VOICE_LANGS.iter().find(|l| l.code == code).map(|l| voice_lang_name(l).to_string()).unwrap_or_else(|| code.to_string());
    let n_todo = status.sets.iter().filter(|g| !g.todo().is_empty()).count();
    let total = status.parts.iter().filter(|p| p.skip.is_none()).count();
    rsx! {
        section { class: "gen-card gen-sets",
            div { class: "gen-out-head",
                span { class: "gen-out-icon", Icon { name: "voice", size: 14 } }
                div {
                    h3 { {t::generate::sets_title()} }
                    p { {t::generate::sets_text()} }
                }
            }
            div { class: "gen-out-list",
                for g in status.sets.iter().cloned() {
                    { let key = g.key();
                      let (name, _) = v::voice_label(&g.voice);
                      let todo = g.todo().len();
                      let asking = confirm.read().as_deref() == Some(key.as_str());
                      let mut detail = vec![t::generate::sets_recorded(g.recorded.len(), total)];
                      if !g.stale.is_empty() { detail.push(t::generate::sets_changed(g.stale.len())); }
                      if !g.missing.is_empty() { detail.push(t::generate::sets_missing(g.missing.len())); }
                      detail.push(format!("{:.2}×", g.speed));
                      let detail = detail.join(" · ");
                      let lacking = g.todo().join(", ");
                      let cur = key == current;
                      let (k1, k2, k3) = (key.clone(), key.clone(), key.clone());
                      let pick = g.clone();
                      rsx! {
                        div { class: if cur { "gen-out-row cur" } else { "gen-out-row" },
                            span { class: "avatar", style: "--h:{hue(&g.voice)}", "{name.chars().next().unwrap_or('?')}" }
                            div { class: "what",
                                b { "{name} · {lang_name(&g.lang)}" }
                                span { title: "{lacking}", "{detail}" }
                            }
                            if !cur {
                                button { class: "gen-mini", disabled: running, title: t::generate::sets_select_tip(),
                                    onclick: move |_| {
                                        let (first, second) = v::split_pair(&pick.voice);
                                        picked.set(None);
                                        req.with_mut(|r| {
                                            r.lang = pick.arg.clone();
                                            r.voice = first.into();
                                            r.voice2 = second.map(str::to_string);
                                            r.speed = pick.speed;
                                            r.gap = pick.gap;
                                            r.level = pick.level;
                                        });
                                    },
                                    {t::generate::sets_select()}
                                }
                            }
                            button { class: "gen-mini go", disabled: running || todo == 0, title: if todo == 0 { t::generate::sets_nothing().to_string() } else { t::generate::sets_record(&lacking) },
                                onclick: move |_| complete(vec![k1.clone()]),
                                if todo == 0 { {t::generate::sets_complete()} } else { {t::generate::sets_complete_n(todo)} }
                            }
                            if asking {
                                button { class: "gen-mini danger", disabled: running, onclick: move |_| remove(k2.clone()), {t::generate::sets_delete_n(g.files.len() / 2)} }
                            } else {
                                button { class: "gen-mini", disabled: running, onclick: move |_| confirm.set(Some(k3.clone())), {t::generate::outdated_delete()} }
                            }
                        }
                    } }
                }
            }
            if n_todo > 1 {
                div { class: "gen-out-foot",
                    button { class: "gen-mini go big", disabled: running, onclick: move |_| complete(status.sets.iter().filter(|g| !g.todo().is_empty()).map(|g| g.key()).collect()), {t::generate::sets_complete_all(n_todo)} }
                }
            }
        }
    }
}
