//! Codecast player: plays a `Script` (parts → cues) on top of the visualizer.
//! Voice: browser TTS, a recorded audio file, or silent reading.

use crate::icons::Icon;
use crate::md::Markdown;
use crate::State;
use dioxus::prelude::*;
use iwr_core::model::Span;
use iwr_core::script::{self, Cue, Question, Resolved, Script};
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
        let mut pron = iwr_core::speech::builtin_pronounce();
        pron.extend(self.glossary_pronounce.read().iter().cloned());
        pron.extend(s.pronounce.iter().cloned());
        self.pronounce.set(Rc::new(pron));
        self.script.set(Some(Rc::new(s)));
        self.part_idx.set(0);
        self.cue_idx.set(0);
        self.playing.set(false);
        self.answering.set(None);
        self.asked.set(vec![]);
        self.tts_gen += 1;
        if self.effective_audio().is_some() {
            self.voice.set(Voice::Audio);
        }
        self.apply_cue();
    }

    /// Questions to offer for the current cue: the script's own, then glossary terms the cue
    /// mentions that were not answered yet.
    pub fn questions(&self) -> Vec<Question> {
        let Some(cue) = self.current_cue() else { return vec![] };
        let use_glossary = *self.glossary_on.read() && self.script.read().as_ref().map(|s| s.glossary).unwrap_or(true);
        let g = self.glossary.read().clone();
        let asked = self.asked.read().clone();
        let empty: Vec<iwr_core::glossary::Entry> = vec![];
        let own: Vec<Question> = cue.questions.iter().filter(|q| !asked.iter().any(|a| a == &q.ask)).cloned().collect();
        let cue = Cue { questions: own, ..cue };
        iwr_core::glossary::questions_for(&cue, if use_glossary { &g } else { &empty }, &asked, 4)
    }

    /// Answer a question: pause the codecast, show what the answer points at, speak it.
    pub fn ask(&mut self, q: Question) {
        let was_playing = *self.playing.read();
        self.resume_after_answer.set(was_playing);
        self.tts_gen += 1;
        if *self.voice.read() == Voice::Audio {
            document::eval("try { const a = document.getElementById('iwr-audio'); if (a) a.pause(); } catch (e) {}");
        }
        self.playing.set(false);
        document::eval("try { speechSynthesis.cancel(); } catch (e) {}");
        let project = self.project.read().clone();
        if let Some(p) = project {
            self.apply_directives(&p, q.show.as_deref(), q.hl.as_ref(), q.code.as_deref());
        }
        self.answering.set(Some(q));
    }

    /// Done with the answer: remember it, restore the cue, resume if the codecast was playing.
    pub fn finish_answer(&mut self) {
        let Some(q) = self.answering.read().clone() else { return };
        let key = q.term.clone().unwrap_or_else(|| q.ask.clone());
        if !self.asked.read().contains(&key) {
            self.asked.write().push(key);
        }
        self.answering.set(None);
        document::eval("try { speechSynthesis.cancel(); } catch (e) {}");
        self.tts_gen += 1;
        self.apply_cue();
        if *self.resume_after_answer.read() {
            self.resume_after_answer.set(false);
            if *self.voice.read() == Voice::Audio {
                document::eval("try { const a = document.getElementById('iwr-audio'); if (a) a.play().catch(() => {}); } catch (e) {}");
            } else {
                self.playing.set(true);
            }
        }
    }

    /// A relative `audio:` name resolved against the codecast directory URL.
    fn audio_src(&self, name: &str) -> String {
        if name.contains("://") || name.starts_with("data:") || name.starts_with('/') {
            name.to_string()
        } else {
            format!("{}{}", self.audio_base.read(), name)
        }
    }

    /// Does the current part have a recording of its own (`[t]` times relative to it)?
    pub fn part_has_audio(&self) -> bool {
        self.script.read().as_ref().and_then(|s| s.parts.get(*self.part_idx.read())).map(|p| p.audio.is_some()).unwrap_or(false)
    }

    /// The recording to play now: the part's own, else the script's, else the one given from outside.
    pub fn effective_audio(&self) -> Option<String> {
        let s = self.script.read().clone();
        if let Some(s) = &s {
            if let Some(a) = s.parts.get(*self.part_idx.read()).and_then(|p| p.audio.as_deref()) {
                return Some(self.audio_src(a));
            }
            if let Some(a) = &s.audio {
                return Some(self.audio_src(a));
            }
        }
        self.audio_url.read().clone()
    }

    /// In recording mode, move the player head to the current cue's `[t]`.
    pub fn seek_audio(&self) {
        if *self.voice.read() != Voice::Audio {
            return;
        }
        let Some(t) = self.current_cue().and_then(|c| c.t) else { return };
        document::eval(&format!("try {{ const a = document.getElementById('iwr-audio'); if (a) a.currentTime = {:.3}; }} catch (e) {{}}", t));
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
        self.hl_ids.set(vec![]);
        if matches!(*self.mode.read(), Mode::Diagram | Mode::Plan) {
            self.set_mode(Mode::Code);
        }
        self.diagram.set(None);
        self.diagram_ops.set(vec![]);
        self.code_mark.set(None);
        document::eval("try { speechSynthesis.cancel(); } catch (e) {}");
    }

    /// The diagram `@ diagram[:ref]` names in the current part.
    pub fn find_diagram(&self, r: Option<&str>) -> Option<iwr_core::diagram::Diagram> {
        let s = self.script.read().clone()?;
        let part = s.parts.get(*self.part_idx.read())?;
        script::find_diagram(part, r).cloned()
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
        self.seek_audio();
        true
    }

    /// Next cue; at the end of a part, the next part when the full tour is on.
    /// Returns false when there is nothing more to play.
    pub fn advance(&mut self) -> bool {
        if self.step(1) {
            return true;
        }
        let n = self.script.read().as_ref().map(|s| s.parts.len()).unwrap_or(0);
        let part = *self.part_idx.read();
        if *self.tour.read() && part + 1 < n {
            self.goto_part(part + 1);
            return true;
        }
        false
    }

    /// Play the whole codecast from its first cue, part after part.
    pub fn start_tour(&mut self) {
        if self.script.read().is_none() {
            self.start_builtin_guide(None);
        }
        self.tour.set(true);
        self.part_idx.set(0);
        self.cue_idx.set(0);
        self.tts_gen += 1;
        self.apply_cue();
        self.seek_audio();
        self.playing.set(true);
    }

    pub fn goto_part(&mut self, part: usize) {
        self.part_idx.set(part);
        self.cue_idx.set(0);
        self.tts_gen += 1;
        self.apply_cue();
        self.seek_audio();
    }

    /// Apply the current cue: view, root/scope, highlights, code mark, camera.
    pub fn apply_cue(&mut self) {
        let Some(p) = self.project.read().clone() else { return };
        let Some(cue) = self.current_cue() else { return };
        // `>` lines are sticky inside a part: replay them from its first cue
        let (part, cue_idx) = (*self.part_idx.read(), *self.cue_idx.read());
        let ops: Vec<iwr_core::diagram::Op> = self
            .script
            .read()
            .as_ref()
            .and_then(|s| s.parts.get(part))
            .map(|part| part.cues.iter().take(cue_idx + 1).flat_map(|c| c.ops.iter().cloned()).collect())
            .unwrap_or_default();
        self.diagram_ops.set(ops);
        self.apply_directives(&p, cue.show.as_deref(), cue.hl.as_ref(), cue.code.as_deref());
    }

    /// Apply `@` / `!` / `=` directives (of a cue or of an answer), then move the camera.
    pub fn apply_directives(&mut self, p: &Rc<iwr_core::model::Project>, show: Option<&str>, hl: Option<&Vec<String>>, code: Option<&str>) {
        self.hovered.set(None);
        self.hover_span.set(None);
        self.animate.set(true);
        if let Some(show) = show {
            let (mode, r) = script::parse_show(show);
            if let Some(m) = mode {
                if m != *self.mode.read() {
                    self.mode.set(m);
                    if !matches!(m, Mode::Code | Mode::Plan) {
                        self.views_open.set(true);
                    }
                }
                if m == Mode::Diagram {
                    let d = self.find_diagram(r);
                    self.diagram.set(d.map(Rc::new));
                } else if let Some(r) = r {
                    if let Some(res) = script::resolve(p, r) {
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
        if let Some(refs) = hl {
            self.hl_ids.set(refs.clone());
            // in a diagram the refs are node ids: don't select project items that share a name
            let in_diagram = *self.mode.read() == Mode::Diagram;
            let resolved: Vec<Resolved> = if in_diagram { vec![] } else { refs.iter().filter_map(|r| script::resolve(p, r)).collect() };
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
        self.code_mark.set(code.and_then(|c| script::resolve(p, c)).map(|r| r.span));
        if let Some(m) = *self.code_mark.read() {
            self.selected_span.set(Some(m));
        }
        // camera (the Code view scrolls instead; the plan page is a list). It replaces the
        // fit that a scope / root / mode change above asked for.
        let req = *self.fit_request.peek();
        self.fit_done.set(req);
        if !matches!(*self.mode.read(), Mode::Code | Mode::Plan) {
            let graph = self.build_graph();
            let hl = self.hl.read().clone();
            let ids = self.hl_ids.read().clone();
            let in_diagram = *self.mode.read() == Mode::Diagram;
            let focus = graph.nodes.iter().find(|n| if in_diagram { ids.contains(&n.id) } else { crate::canvas::hl_hit(n, &hl) });
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

/// Try to load a script from the server (`/api/script`) or the static export (`codecast.md`).
/// Returns (text, external audio url, base url for relative `audio:` names, project glossary text).
pub async fn fetch_script(live: bool) -> Option<(String, Option<String>, String, Option<String>)> {
    if live {
        let t = fetch_text("/api/script").await?;
        let audio = gloo_net::http::RequestBuilder::new("/api/audio").method(gloo_net::http::Method::HEAD).send().await.ok().filter(|r| r.ok()).map(|_| "/api/audio".to_string());
        let glossary = fetch_text("/api/glossary").await;
        Some((t, audio, "/api/codecast/".into(), glossary))
    } else {
        let t = match fetch_text("codecast.md").await {
            Some(t) => t,
            None => fetch_text("script.txt").await?,
        };
        let audio = fetch_text("audio.txt").await.map(|a| a.trim().to_string());
        let glossary = fetch_text("glossary.md").await;
        Some((t, audio, "codecast/".into(), glossary))
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
    let audio_url = state.effective_audio();
    let Some(part) = s.parts.get(part_idx) else { return rsx! {} };
    let Some(cue) = part.cues.get(cue_idx) else { return rsx! {} };
    let total = part.cues.len();
    let pct = (cue_idx + 1) as f64 / total.max(1) as f64 * 100.0;
    let title = s.title.clone();
    let say = cue.say.clone();
    let n_parts = s.parts.len();
    let tour = *state.tour.read();
    let mut show_load = use_signal(|| false);
    let mut paste = use_signal(String::new);
    let answering = state.answering.read().clone();
    let questions = if answering.is_none() { state.questions() } else { vec![] };

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
            if state.answering.read().is_none() {
                document::eval("try { speechSynthesis.cancel(); } catch (e) {}");
            }
            return;
        }
        let Some(cue) = state.current_cue() else { return };
        let text = iwr_core::speech::spoken(&cue.say, &state.pronounce.peek());
        let rate = *state.tts_rate.peek();
        let js = format!(
            r#"try {{ speechSynthesis.cancel(); }} catch (e) {{}}
               const u = new SpeechSynthesisUtterance({});
               u.rate = {rate:.2};
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
                    if *state.tts_gen.peek() == my_gen && *state.playing.peek() && !state.advance() {
                        state.playing.set(false);
                    }
                }
            }
        });
    });
    let _ = gen;

    // ---- Answer: speak it (unless reading silently); when it ends, resume the codecast
    use_effect(move || {
        let Some(q) = state.answering.read().clone() else { return };
        if *state.voice.peek() == Voice::Read {
            return;
        }
        let pron = state.pronounce.peek().clone();
        let text = format!("{} {}", iwr_core::speech::spoken(&q.ask, &pron), iwr_core::speech::spoken(&q.answer, &pron));
        let rate = *state.tts_rate.peek();
        let js = format!(
            r#"try {{ speechSynthesis.cancel(); }} catch (e) {{}}
               const u = new SpeechSynthesisUtterance({});
               u.rate = {rate:.2};
               u.onend = () => dioxus.send("end");
               u.onerror = (e) => dioxus.send("error:" + e.error);
               setTimeout(() => speechSynthesis.speak(u), 60);
               "#,
            serde_json::to_string(&text).unwrap_or_else(|_| "\"\"".into())
        );
        let my_gen = *state.tts_gen.peek();
        spawn(async move {
            let mut ev = document::eval(&js);
            if let Ok(msg) = ev.recv::<String>().await {
                if *state.tts_gen.peek() != my_gen || state.answering.peek().is_none() {
                    return;
                }
                if msg == "end" {
                    gloo_timers::future::sleep(std::time::Duration::from_millis(600)).await;
                    if *state.tts_gen.peek() == my_gen && state.answering.peek().is_some() {
                        state.finish_answer();
                    }
                }
            }
        });
    });

    // ---- Audio: follow the recording's time
    use_effect(move || {
        let voice = *state.voice.read();
        let url = state.effective_audio();
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
                    Ok(t) if t == -2.0 => state.playing.set(false),
                    Ok(t) if t == -3.0 => {
                        // end of a per-part recording: continue with the next part when touring
                        let n = state.script.peek().as_ref().map(|s| s.parts.len()).unwrap_or(0);
                        let part = *state.part_idx.peek();
                        if state.part_has_audio() && *state.tour.peek() && part + 1 < n {
                            state.goto_part(part + 1);
                            if state.part_has_audio() {
                                document::eval("setTimeout(() => { const a = document.getElementById('iwr-audio'); if (a) { a.currentTime = 0; a.play().catch(() => {}); } }, 150);");
                                continue;
                            }
                        }
                        state.playing.set(false);
                    }
                    Ok(t) => {
                        // cue with the largest start time <= t: inside this part when it has its own
                        // recording, else across every part that shares the script's recording
                        let Some(s) = state.script.read().clone() else { break };
                        let own = state.part_has_audio();
                        let cur_part = *state.part_idx.peek();
                        let mut best: Option<(usize, usize)> = None;
                        let mut best_t = -1.0;
                        for (pi, p) in s.parts.iter().enumerate() {
                            if (own && pi != cur_part) || (!own && p.audio.is_some()) {
                                continue;
                            }
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
                    button { class: if playing { "active" } else { "" }, onclick: move |_| {
                        let p = *state.playing.read();
                        state.tts_gen += 1;
                        if *state.voice.read() == Voice::Audio {
                            // the recording drives `playing` through its own events
                            document::eval(if p { "const a = document.getElementById('iwr-audio'); if (a) a.pause();" } else { "const a = document.getElementById('iwr-audio'); if (a) a.play().catch(() => {});" });
                        } else {
                            state.playing.set(!p);
                        }
                    }, if playing { "❚❚ pause" } else { "▶ play" } }
                    button { onclick: move |_| { state.tts_gen += 1; state.step(1); }, disabled: cue_idx + 1 >= total, "▶|" }
                }
                div { class: "status", "{cue_idx + 1} / {total} · part {part_idx + 1} / {n_parts}" }
                div { class: "progress", div { style: "width:{pct:.1}%" } }
                div { class: "row",
                    select { class: "theme-select", value: match voice { Voice::Tts => "tts", Voice::Audio => "audio", Voice::Read => "read" },
                        onchange: move |e| { state.tts_gen += 1; state.playing.set(false); state.voice.set(match e.value().as_str() { "audio" => Voice::Audio, "read" => Voice::Read, _ => Voice::Tts }); state.save_settings(); },
                        option { value: "tts", "🔊 voice (TTS)" }
                        if audio_url.is_some() { option { value: "audio", "🎧 recording" } }
                        option { value: "read", "📖 read" }
                    }
                }
                div { class: "row",
                    label { class: "status", style: "display:flex; gap:4px; align-items:center; cursor:pointer", title: "Full tour: continue with the next part when one ends",
                        input { r#type: "checkbox", checked: tour, onchange: move |e| { state.tour.set(e.checked()); state.save_settings(); } }
                        "full tour"
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
                if let Some(q) = answering.clone() {
                    div { class: "answer",
                        div { class: "q",
                            Icon { name: "question", size: 14 }
                            "{q.ask}"
                            if q.term.is_some() { span { class: "src", "glossary" } }
                            div { class: "spacer" }
                            button { class: "small", title: "back to the codecast (space / Esc)", onclick: move |_| state.finish_answer(), if *state.resume_after_answer.read() { "▶ continue" } else { "✓ got it" } }
                        }
                        Markdown { text: q.answer.clone() }
                    }
                } else if !questions.is_empty() {
                    div { class: "qs",
                        span { class: "lbl", "ask:" }
                        for q in questions.iter().cloned() {
                            { let own = q.term.is_none(); let ask = q.ask.clone(); rsx! {
                                button { class: if own { "own" } else { "" }, title: if own { "question from the script" } else { "from the Rust glossary" }, onclick: move |_| state.ask(q.clone()), "{ask}" }
                            } }
                        }
                    }
                }
                if show_load() {
                    div { class: "loader",
                        div { class: "status", "Load a codecast script (markdown, see docs/codecast.md). Paste it or pick a file; optional audio file for a recording." }
                        textarea { rows: 5, placeholder: "# Title\n## Part\n@ flow:crate::run\n! crate::run/b1\nSpoken sentence…", value: "{paste}", oninput: move |e| paste.set(e.value()) }
                        div { class: "row",
                            input { r#type: "file", accept: ".md,.txt,.script", onchange: move |e| {
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
                                            let b64 = crate::icons::base64(&bytes);
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
                    button { class: "small", title: "The plan: every part of this codecast, one line each; click one to jump there", onclick: move |_| { state.settings_open.set(false); state.set_mode(Mode::Plan); }, "plan" }
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

/// The plan page (`@ plan`, or the *plan* button): the codecast's parts in order, one line each,
/// the current one marked. Clicking a part plays it from its first cue.
#[component]
pub fn PlanPage() -> Element {
    let mut state = use_context::<State>();
    let Some(s) = state.script.read().clone() else {
        return rsx! { div { class: "plan", div { class: "wrap", p { class: "intro", "No codecast loaded." } } } };
    };
    let cur = *state.part_idx.read();
    let n_parts = s.parts.len();
    let total = s.cue_count();
    rsx! {
        div { class: "plan",
            div { class: "wrap",
                h2 { Icon { name: "plan".to_string(), size: 22 } "{s.title}" }
                p { class: "intro",
                    if let Some(i) = &s.summary { "{i} · " }
                    "{n_parts} parts, {total} cues. Click a part to play it; the full tour plays them all in a row."
                }
                ol {
                    for (i, p) in s.parts.iter().enumerate() {
                        {
                            let first = p.cues.first().map(|c| c.say.clone()).unwrap_or_default();
                            let summary = p.summary.clone().unwrap_or_else(|| iwr_core::shorten(&script::plain(&first), 140));
                            let nq: usize = p.cues.iter().map(|c| c.questions.len()).sum();
                            let mut meta = format!("{} cue{}", p.cues.len(), if p.cues.len() == 1 { "" } else { "s" });
                            if nq > 0 {
                                meta.push_str(&format!(" · {} question{}", nq, if nq == 1 { "" } else { "s" }));
                            }
                            if !p.diagrams.is_empty() {
                                meta.push_str(" · diagram");
                            }
                            let class = if i == cur { "cur" } else if i < cur { "done" } else { "" };
                            rsx! {
                                li { key: "{i}", class: "{class}",
                                    onclick: move |_| { state.tts_gen += 1; state.goto_part(i); },
                                    div { class: "num", "{i + 1}" }
                                    div {
                                        div { class: "name", "{p.name}" }
                                        div { class: "sum", "{summary}" }
                                    }
                                    div { class: "meta", "{meta}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
