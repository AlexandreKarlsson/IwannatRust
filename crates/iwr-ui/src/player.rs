//! Codecast player: plays a `Script` (parts → cues) on top of the visualizer.
//! Voice: browser TTS, a recorded audio file, or silent reading.

use crate::md::Markdown;
use crate::State;
use dioxus::prelude::*;
use iwr_core::model::Span;
use iwr_core::script::{self, Cue, Resolved, Script};
use iwr_core::views::Mode;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Voice {
    Tts,
    Audio,
    Read,
}

/// Player state (all signals live in `State`; these helpers act on them).
impl State {
    pub fn load_script(&mut self, s: Script) {
        let has_audio = s.audio.is_some() || *self.audio_url.read() != None;
        if let Some(a) = &s.audio {
            if self.audio_url.read().is_none() {
                self.audio_url.set(Some(a.clone()));
            }
        }
        self.script.set(Some(Rc::new(s)));
        self.part_idx.set(0);
        self.cue_idx.set(0);
        self.playing.set(false);
        self.tts_gen += 1;
        if has_audio {
            self.voice.set(Voice::Audio);
        }
        self.apply_cue();
    }

    pub fn start_builtin_guide(&mut self, root: Option<usize>) {
        let Some(p) = self.project.read().clone() else { return };
        let steps = iwr_core::guide::build(&p, root, &iwr_core::guide::GuideOptions::default());
        let s = script::from_guide(&p, &steps);
        self.audio_url.set(None);
        self.voice.set(Voice::Tts);
        self.load_script(s);
    }

    pub fn stop_script(&mut self) {
        self.script.set(None);
        self.playing.set(false);
        self.tts_gen += 1;
        self.hl.set(vec![]);
        self.code_mark.set(None);
        document::eval("try { speechSynthesis.cancel(); } catch (e) {}");
    }

    pub fn current_cue(&self) -> Option<Cue> {
        let s = self.script.read().clone()?;
        s.parts.get(*self.part_idx.read())?.cues.get(*self.cue_idx.read()).cloned()
    }

    /// Move by `delta` cues inside the current part. Returns false at the part boundary.
    pub fn step(&mut self, delta: i64) -> bool {
        let Some(s) = self.script.read().clone() else { return false };
        let part = *self.part_idx.read();
        let n = s.parts.get(part).map(|p| p.cues.len()).unwrap_or(0) as i64;
        let cur = *self.cue_idx.read() as i64;
        let next = cur + delta;
        if next < 0 || next >= n {
            return false;
        }
        self.cue_idx.set(next as usize);
        self.apply_cue();
        true
    }

    pub fn goto_part(&mut self, part: usize) {
        self.part_idx.set(part);
        self.cue_idx.set(0);
        self.tts_gen += 1;
        self.apply_cue();
    }

    /// Apply the current cue: view, root/scope, highlights, code mark, camera.
    pub fn apply_cue(&mut self) {
        let Some(p) = self.project.read().clone() else { return };
        let Some(cue) = self.current_cue() else { return };
        self.hovered.set(None);
        self.hover_span.set(None);
        self.animate.set(true);
        if let Some(show) = &cue.show {
            let (mode, r) = script::parse_show(show);
            if let Some(m) = mode {
                if m != *self.mode.read() {
                    self.mode.set(m);
                    if m != Mode::Code {
                        self.views_open.set(true);
                    }
                }
                if let Some(r) = r {
                    if let Some(res) = script::resolve(&p, r) {
                        if let Some(mid) = res.module {
                            self.set_scope(Some(mid));
                        } else if res.item.is_none() {
                            // a file: open it in the Code view
                            self.code_file.set(Some(res.span.file));
                            self.selected.set(None);
                        } else if let Some(item) = res.item {
                            match m {
                                Mode::Code => self.open_in_code(item),
                                Mode::CallTree | Mode::ControlFlow | Mode::BranchTree => {
                                    self.set_root(Some(item));
                                    self.selected.set(Some(item));
                                }
                                _ => {
                                    self.set_scope(Some(p.item(item).module));
                                    self.selected.set(Some(item));
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Some(refs) = &cue.hl {
            let resolved: Vec<Resolved> = refs.iter().filter_map(|r| script::resolve(&p, r)).collect();
            // in the Code view open the blocks that contain the highlight
            if *self.mode.read() == Mode::Code {
                for r in &resolved {
                    if r.item.is_none() && r.module.is_none() {
                        self.code_file.set(Some(r.span.file));
                    }
                    if let Some(item) = r.item {
                        self.code_file.set(Some(r.span.file));
                        let mut ob = self.open_blocks.write();
                        ob.insert(format!("i{}", item));
                        if let Some(f) = p.item(item).fn_info() {
                            if let Some(impl_id) = f.impl_id {
                                ob.insert(format!("i{}", impl_id));
                            }
                            if let Some(root) = &f.blocks {
                                open_path(root, &r.span, item, &mut ob);
                            }
                        }
                    }
                }
            }
            if let Some(first) = resolved.first() {
                self.selected_span.set(Some(first.span));
                if let Some(item) = first.item {
                    self.selected.set(Some(item));
                }
            }
            self.hl.set(resolved);
        }
        self.code_mark.set(cue.code.as_deref().and_then(|c| script::resolve(&p, c)).map(|r| r.span));
        if let Some(m) = *self.code_mark.read() {
            self.selected_span.set(Some(m));
        }
        // camera
        if *self.mode.read() != Mode::Code {
            let graph = self.build_graph();
            let hl = self.hl.read().clone();
            let focus = graph.nodes.iter().find(|n| crate::canvas::hl_hit(n, &hl));
            if let Some(n) = focus {
                let (x, y, w, h) = (n.x, n.y, n.w, n.h);
                self.fit(&graph);
                let k = self.view.read().k.clamp(0.8, 1.2);
                self.view.write().k = k;
                self.center_on(x, y, w, h);
            } else {
                self.fit(&graph);
            }
        }
    }
}

/// Open every block on the path to the blocks inside `span`.
fn open_path(b: &iwr_core::model::Block, span: &Span, item: usize, ob: &mut std::collections::HashSet<String>) {
    for c in &b.children {
        let inside = c.span.file == span.file && c.span.line_start <= span.line_end && c.span.line_end >= span.line_start;
        if inside {
            let contains_whole = c.span.line_start <= span.line_start && c.span.line_end >= span.line_end;
            // open ancestors (blocks that contain the target), not the target itself
            if contains_whole && !(c.span.line_start == span.line_start && c.span.line_end == span.line_end) {
                ob.insert(format!("i{}/b{}", item, c.id));
            }
            open_path(c, span, item, ob);
        }
    }
}

async fn fetch_text(url: &str) -> Option<String> {
    let r = gloo_net::http::Request::get(url).send().await.ok()?;
    if !r.ok() {
        return None;
    }
    r.text().await.ok()
}

/// Try to load a script from the server (`/api/script`) or the static export (`script.txt`).
pub async fn fetch_script(live: bool) -> Option<(String, Option<String>)> {
    if live {
        let t = fetch_text("/api/script").await?;
        let audio = gloo_net::http::RequestBuilder::new("/api/audio").method(gloo_net::http::Method::HEAD).send().await.ok().filter(|r| r.ok()).map(|_| "/api/audio".to_string());
        Some((t, audio))
    } else {
        let t = fetch_text("script.txt").await?;
        let audio = fetch_text("audio.txt").await.map(|a| a.trim().to_string());
        Some((t, audio))
    }
}

#[component]
pub fn PlayerBar() -> Element {
    let mut state = use_context::<State>();
    let Some(s) = state.script.read().clone() else { return rsx! {} };
    let part_idx = *state.part_idx.read();
    let cue_idx = *state.cue_idx.read();
    let playing = *state.playing.read();
    let voice = *state.voice.read();
    let audio_url = state.audio_url.read().clone();
    let Some(part) = s.parts.get(part_idx) else { return rsx! {} };
    let Some(cue) = part.cues.get(cue_idx) else { return rsx! {} };
    let total = part.cues.len();
    let pct = (cue_idx + 1) as f64 / total.max(1) as f64 * 100.0;
    let title = s.title.clone();
    let say = cue.say.clone();
    let n_parts = s.parts.len();
    let mut show_load = use_signal(|| false);
    let mut paste = use_signal(String::new);

    // ---- TTS: speak the current cue while playing; advance on end
    let gen = *state.tts_gen.read();
    use_effect(move || {
        let playing = *state.playing.read();
        let voice = *state.voice.read();
        let _ = *state.cue_idx.read();
        let _ = *state.part_idx.read();
        let _gen = *state.tts_gen.read();
        if voice != Voice::Tts {
            return;
        }
        if !playing {
            document::eval("try { speechSynthesis.cancel(); } catch (e) {}");
            return;
        }
        let Some(cue) = state.current_cue() else { return };
        let text = script::speakable(&cue.say);
        let js = format!(
            r#"try {{ speechSynthesis.cancel(); }} catch (e) {{}}
               const u = new SpeechSynthesisUtterance({});
               u.rate = 1.0;
               u.onend = () => dioxus.send("end");
               u.onerror = (e) => dioxus.send("error:" + e.error);
               speechSynthesis.speak(u);
               "#,
            serde_json::to_string(&text).unwrap_or_else(|_| "\"\"".into())
        );
        let my_gen = *state.tts_gen.peek();
        spawn(async move {
            let mut ev = document::eval(&js);
            if let Ok(msg) = ev.recv::<String>().await {
                if *state.tts_gen.peek() != my_gen || !*state.playing.peek() {
                    return;
                }
                if msg == "end" || msg.starts_with("error") {
                    gloo_timers::future::sleep(std::time::Duration::from_millis(350)).await;
                    if *state.tts_gen.peek() == my_gen && *state.playing.peek() && !state.step(1) {
                        state.playing.set(false);
                    }
                }
            }
        });
    });
    let _ = gen;

    // ---- Audio: follow the recording's time
    use_effect(move || {
        let voice = *state.voice.read();
        let url = state.audio_url.read().clone();
        if voice != Voice::Audio || url.is_none() {
            return;
        }
        spawn(async move {
            // wait for the element, then stream currentTime
            let mut ev = document::eval(
                r#"const wait = () => new Promise(r => { const f = () => { const a = document.getElementById('iwr-audio'); if (a) r(a); else setTimeout(f, 100); }; f(); });
                   const a = await wait();
                   a.ontimeupdate = () => dioxus.send(a.currentTime);
                   a.onplay = () => dioxus.send(-1);
                   a.onpause = () => dioxus.send(-2);
                   a.onended = () => dioxus.send(-3);"#,
            );
            loop {
                match ev.recv::<f64>().await {
                    Ok(t) if t == -1.0 => state.playing.set(true),
                    Ok(t) if t == -2.0 || t == -3.0 => state.playing.set(false),
                    Ok(t) => {
                        // cue with the largest start time <= t, across all parts
                        let Some(s) = state.script.read().clone() else { break };
                        let mut best: Option<(usize, usize)> = None;
                        let mut best_t = -1.0;
                        for (pi, p) in s.parts.iter().enumerate() {
                            for (ci, c) in p.cues.iter().enumerate() {
                                if let Some(ct) = c.t {
                                    if ct <= t && ct >= best_t {
                                        best_t = ct;
                                        best = Some((pi, ci));
                                    }
                                }
                            }
                        }
                        if let Some((pi, ci)) = best {
                            if pi != *state.part_idx.peek() || ci != *state.cue_idx.peek() {
                                state.part_idx.set(pi);
                                state.cue_idx.set(ci);
                                state.apply_cue();
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    });

    rsx! {
        div { class: "guide",
            div { class: "ctl",
                div { class: "row",
                    button { onclick: move |_| { state.tts_gen += 1; state.step(-1); }, disabled: cue_idx == 0, "◀" }
                    button { class: if playing { "active" } else { "" }, onclick: move |_| { let p = *state.playing.read(); state.tts_gen += 1; state.playing.set(!p); }, if playing { "❚❚ pause" } else { "▶ play" } }
                    button { onclick: move |_| { state.tts_gen += 1; state.step(1); }, disabled: cue_idx + 1 >= total, "▶|" }
                }
                div { class: "status", "{cue_idx + 1} / {total}" }
                div { class: "progress", div { style: "width:{pct:.1}%" } }
                div { class: "row",
                    select { class: "theme-select", value: match voice { Voice::Tts => "tts", Voice::Audio => "audio", Voice::Read => "read" },
                        onchange: move |e| { state.tts_gen += 1; state.playing.set(false); state.voice.set(match e.value().as_str() { "audio" => Voice::Audio, "read" => Voice::Read, _ => Voice::Tts }); },
                        option { value: "tts", "🔊 voice (TTS)" }
                        if audio_url.is_some() { option { value: "audio", "🎧 recording" } }
                        option { value: "read", "📖 read" }
                    }
                }
                div { class: "row",
                    button { class: "small", onclick: move |_| { let v = *show_load.read(); show_load.set(!v); }, "load…" }
                    button { class: "small", onclick: move |_| state.stop_script(), "✕ close" }
                }
            }
            div { class: "body",
                div { class: "parts",
                    span { class: "status", "{title} · " }
                    for (i, p) in s.parts.iter().enumerate() {
                        { let name = p.name.clone(); let n = p.cues.len(); rsx! {
                            button { class: if i == part_idx { "small active" } else { "small" }, title: "{n} cues", onclick: move |_| state.goto_part(i), "{name}" }
                        } }
                    }
                    if n_parts > 1 && cue_idx + 1 >= total && part_idx + 1 < n_parts {
                        button { class: "small", onclick: move |_| state.goto_part(part_idx + 1), "next part ▶" }
                    }
                }
                if voice == Voice::Audio {
                    if let Some(u) = &audio_url {
                        audio { id: "iwr-audio", controls: true, src: "{u}", style: "width:100%; height:32px; margin:4px 0" }
                    }
                }
                Markdown { text: say }
                if show_load() {
                    div { class: "loader",
                        div { class: "status", "Load a codecast script (text format, see docs/codecast.md). Paste it or pick a file; optional audio file for a recording." }
                        textarea { rows: 5, placeholder: "# Title\n## Part\n@ flow:crate::run\n! crate::run/b1\nSpoken sentence…", value: "{paste}", oninput: move |e| paste.set(e.value()) }
                        div { class: "row",
                            input { r#type: "file", accept: ".txt,.md,.script", onchange: move |e| {
                                let files = e.files();
                                spawn(async move {
                                    if let Some(f) = files.first() {
                                        if let Ok(t) = f.read_string().await { paste.set(t); }
                                    }
                                });
                            } }
                            input { r#type: "file", accept: "audio/*", onchange: move |e| {
                                let files = e.files();
                                spawn(async move {
                                    if let Some(f) = files.first() {
                                        if let Ok(bytes) = f.read_bytes().await {
                                            // hand the bytes to the browser as an object URL
                                            let b64 = base64_encode(&bytes);
                                            let mime = if f.name().ends_with(".wav") { "audio/wav" } else if f.name().ends_with(".ogg") { "audio/ogg" } else { "audio/mpeg" };
                                            state.audio_url.set(Some(format!("data:{};base64,{}", mime, b64)));
                                            state.voice.set(Voice::Audio);
                                        }
                                    }
                                });
                            } }
                            button { class: "small", onclick: move |_| {
                                let t = paste.read().clone();
                                if !t.trim().is_empty() {
                                    state.load_script(script::parse(&t));
                                    show_load.set(false);
                                }
                            }, "use script" }
                        }
                    }
                }
            }
        }
    }
}

fn base64_encode(bytes: &[u8]) -> String {
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
