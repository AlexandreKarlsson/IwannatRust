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
    pub fn load_script(&mut self, mut s: Script) {
        // generated tours (`architecture_tour`) need the project
        if let Some(p) = self.project.read().clone() {
            script::expand_tours(&p, &mut s);
        }
        let mut pron = iwr_core::speech::builtin_pronounce();
        pron.extend(self.glossary_pronounce.read().iter().cloned());
        pron.extend(s.pronounce.iter().cloned());
        self.pronounce.set(Rc::new(pron));
        let s = Rc::new(s);
        self.script_src.set(Some(s.clone()));
        self.glossary_tr.set(None);
        self.script.set(Some(Rc::new(self.with_voice(&s, "en"))));
        self.part_idx.set(0);
        self.cue_idx.set(0);
        self.playing.set(false);
        self.answering.set(None);
        self.asked.set(vec![]);
        self.tts_gen += 1;
        // a recording plays unless the codecast's config asks for TTS or silent reading
        if self.has_recording() && !matches!(self.config.peek().voice.as_deref(), Some("tts" | "read")) {
            self.voice.set(Voice::Audio);
        }
        self.apply_cue();
        if self.lang.peek().as_str() != "en" {
            self.translate();
        }
    }

    /// The recorded voice that plays in `lang`: the listener's pick when it recorded this
    /// language, else the voice that recorded the most parts (`""`: none named).
    pub fn recording_voice(&self, s: &Script, lang: &str) -> String {
        let voices = iwr_core::i18n::recorded_voices(s, lang);
        let pick = self.rec_voice.peek().clone();
        if voices.iter().any(|(v, _)| *v == pick) {
            return pick;
        }
        voices.into_iter().map(|(v, _)| v).next().unwrap_or_default()
    }

    /// A script whose parts play the recordings of the chosen voice in `lang`.
    fn with_voice(&self, s: &Script, lang: &str) -> Script {
        let mut s = s.clone();
        let voice = self.recording_voice(&s, lang);
        iwr_core::i18n::use_voices(&mut s, lang, &voice);
        s
    }

    /// Play the recordings of another voice (switches to recording mode), from the same cue.
    pub fn set_rec_voice(&mut self, voice: &str) {
        self.rec_voice.set(voice.to_string());
        self.save_settings();
        self.tts_gen += 1;
        self.playing.set(false);
        let lang = self.lang.peek().clone();
        // English starts again from the script as written; a translation keeps its text
        let base = if lang == "en" { self.script_src.peek().clone() } else { self.script.peek().clone() };
        if let Some(base) = base {
            self.script.set(Some(Rc::new(self.with_voice(&base, &lang))));
        }
        self.voice.set(Voice::Audio);
        self.apply_cue();
    }

    /// Play the codecast in another language (`en`: as written).
    pub fn set_lang(&mut self, code: &str) {
        if self.lang.peek().as_str() == code {
            return;
        }
        self.lang.set(code.to_string());
        self.tts_gen += 1;
        self.playing.set(false);
        document::eval("try { speechSynthesis.cancel(); } catch (e) {}");
        // recordings are in English: a translated codecast is spoken by TTS
        if code != "en" && *self.voice.peek() == Voice::Audio {
            self.voice.set(Voice::Tts);
        }
        if code == "en" {
            self.lang_info.set(String::new());
            self.glossary_tr.set(None);
            let src = self.script_src.peek().clone();
            if let Some(src) = src {
                self.script.set(Some(Rc::new(self.with_voice(&src, "en"))));
                if self.has_recording() {
                    self.voice.set(Voice::Audio);
                }
                self.apply_cue();
            }
        } else {
            // until the translation arrives: the English text, without its English recordings
            let src = self.script_src.peek().clone();
            if let Some(src) = src {
                let mut s = (*src).clone();
                iwr_core::i18n::use_voices(&mut s, "", "");
                self.script.set(Some(Rc::new(s)));
                self.translate();
            }
        }
    }

    /// Translate the English script and the glossary into `lang`: by the server's offline model
    /// (`/api/translate`), or from the map a static export ships (`translations/<lang>.json`).
    /// The script stays in English until the translation arrives; the position is kept.
    fn translate(&mut self) {
        let mut state = *self;
        let lang = self.lang.peek().clone();
        let Some(src) = self.script_src.peek().clone() else { return };
        let Some(p) = self.project.peek().clone() else { return };
        let glossary = self.glossary.peek().clone();
        let name = iwr_core::i18n::lang(&lang).map(|l| l.name).unwrap_or(&lang).to_string();
        let live = *self.live.peek();
        spawn(async move {
            let names = iwr_core::i18n::names(&p);
            let texts = iwr_core::i18n::texts(&src, &glossary, &names);
            state.lang_info.set(format!("translating {} pieces to {name}…", texts.len()));
            let map: Result<std::collections::HashMap<String, String>, String> = if live {
                fetch_translations(&lang, &texts).await.map(|done| texts.iter().cloned().zip(done).collect())
            } else {
                match fetch_text(&format!("translations/{lang}.json")).await {
                    Some(t) => serde_json::from_str(&t).map_err(|e| e.to_string()),
                    None => Err(format!("this site has no {name} translation (export it with --lang {lang})")),
                }
            };
            // the language or the script changed meanwhile: that one wins
            if *state.lang.peek() != lang || !state.script_src.peek().as_ref().is_some_and(|s| Rc::ptr_eq(s, &src)) {
                return;
            }
            match map {
                Ok(map) => {
                    let mut s = (*src).clone();
                    let mut g = (*glossary).clone();
                    iwr_core::i18n::apply(&mut s, &mut g, &names, &|t| map.get(t).cloned());
                    // parts recorded in this language (`iwr voice --lang`) play their recording
                    let voice = state.recording_voice(&s, &lang);
                    iwr_core::i18n::use_voices(&mut s, &lang, &voice);
                    state.glossary_tr.set(Some(Rc::new(g)));
                    state.script.set(Some(Rc::new(s)));
                    state.lang_info.set(String::new());
                    if state.has_recording() && *state.voice.peek() == Voice::Tts {
                        state.voice.set(Voice::Audio);
                    }
                    state.apply_cue();
                }
                Err(e) => state.lang_info.set(format!("⚠ no {name}: {e}. Playing in English.")),
            }
        });
    }

    /// Questions to offer for the current cue: the script's own, then glossary terms the cue
    /// mentions that were not answered yet.
    pub fn questions(&self) -> Vec<Question> {
        let Some(cue) = self.current_cue() else { return vec![] };
        let use_glossary = *self.glossary_on.read() && self.script.read().as_ref().map(|s| s.glossary).unwrap_or(true);
        let g = self.glossary_tr.read().clone().unwrap_or_else(|| self.glossary.read().clone());
        let asked = self.asked.read().clone();
        let empty: Vec<iwr_core::glossary::Entry> = vec![];
        let own: Vec<Question> = cue.questions.iter().filter(|q| !asked.iter().any(|a| a == &q.ask)).cloned().collect();
        // glossary terms are found in the English text, whatever the language played
        let say = self.script_src.read().as_ref().and_then(|s| s.parts.get(*self.part_idx.read())?.cues.get(*self.cue_idx.read()).map(|c| c.say.clone())).unwrap_or(cue.say);
        let cue = Cue { questions: own, say, ..Default::default() };
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
            if let Some(sd) = &q.side {
                let side = self.script.read().as_ref().and_then(|s| s.parts.get(*self.part_idx.read())).and_then(|part| script::find_side(part, sd).ok().flatten());
                self.side.set(side.map(Rc::new));
            }
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
            if *self.voice.read() == Voice::Audio && !self.uses_tts() {
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

    /// The recording to play now: the part's own, else the script's, else the one given from outside
    /// (English only). A translated script carries the recordings of its language (`audio.ja:`).
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
        self.audio_url.read().clone().filter(|_| self.lang.read().as_str() == "en")
    }

    /// Is there any recording: the script's, a part's, or one given from outside?
    pub fn has_recording(&self) -> bool {
        self.lang.read().as_str() == "en" && self.audio_url.read().is_some() || self.script.read().as_ref().is_some_and(|s| s.audio.is_some() || s.parts.iter().any(|p| p.audio.is_some()))
    }

    /// Is the current cue spoken by browser TTS: TTS mode, or recording mode on a part without a
    /// recording (none made, or dropped by the loader because the text changed since).
    pub fn uses_tts(&self) -> bool {
        match *self.voice.read() {
            Voice::Tts => true,
            Voice::Audio => self.effective_audio().is_none(),
            Voice::Read => false,
        }
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
        self.side.set(None);
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
        // `|` lines are sticky inside a part too
        let side = self.script.read().as_ref().and_then(|s| s.parts.get(part)).and_then(|part| script::side_at(part, cue_idx));
        self.side.set(side.map(Rc::new));
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
            // in a diagram the refs are node ids, and the side panel's fields or nodes are its
            // own: don't select project items that share a name
            let in_diagram = *self.mode.read() == Mode::Diagram;
            let side_ids = self.side.read().as_ref().map(|s| s.ids()).unwrap_or_default();
            let resolved: Vec<Resolved> = if in_diagram { vec![] } else { refs.iter().filter(|r| !side_ids.contains(r)).filter_map(|r| script::resolve(p, r)).collect() };
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

/// The server's offline translation of `texts` into `lang` (the first call for a language sets up
/// its model: minutes, once).
async fn fetch_translations(lang: &str, texts: &[String]) -> Result<Vec<String>, String> {
    let r = gloo_net::http::Request::post(&format!("/api/translate/{lang}")).json(texts).map_err(|e| e.to_string())?.send().await.map_err(|e| e.to_string())?;
    if !r.ok() {
        return Err(r.text().await.unwrap_or_else(|_| r.status().to_string()));
    }
    r.json::<Vec<String>>().await.map_err(|e| e.to_string())
}

/// JS that sets the utterance `u` to speak `lang`, with a voice for it when the browser has one
/// (English keeps the browser's default voice). `name`: the codecast's `tts_voice`, picked first
/// when a voice of that language has it in its name.
pub fn voice_js(lang: &str, name: Option<&str>) -> String {
    let Some(l) = iwr_core::i18n::lang(lang) else { return String::new() };
    let name = serde_json::to_string(&name.unwrap_or_default().to_lowercase()).unwrap_or_else(|_| "\"\"".into());
    if l.code == "en" && name == "\"\"" {
        return String::new();
    }
    format!(
        r#"{set_lang}
           try {{ const vs = speechSynthesis.getVoices().filter(v => v.lang.replace("_", "-").toLowerCase().startsWith("{code}"));
                  const v = ({name} && vs.find(v => v.name.toLowerCase().includes({name}))) || ("{code}" != "en" && vs[0]); if (v) u.voice = v; }} catch (e) {{}}"#,
        set_lang = if l.code == "en" { String::new() } else { format!("u.lang = \"{}\";", l.speech) },
        code = l.code
    )
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
    // an answer being shown brings its own popup, the cue's comes back after it
    let pop = match &*state.answering.read() { Some(q) => q.pop.clone(), None => cue.pop.clone() };
    // the cue whose popup the viewer closed; the next cue's popup shows again
    let mut pop_closed = use_signal(|| None::<(usize, usize)>);
    let n_parts = s.parts.len();
    let tour = *state.tour.read();
    let tts_info = state.tts_info.read().clone();
    let lang = state.lang.read().clone();
    let lang_info = state.lang_info.read().clone();
    let locked = state.locked();
    // voices the browser offers, checked as soon as TTS is selected (None = not known yet)
    let mut tts_voices = use_signal(|| None::<usize>);
    use_effect(move || {
        if !state.uses_tts() {
            return;
        }
        spawn(async move {
            // Chrome fills the list asynchronously: wait for `voiceschanged`, at most 2 s
            let mut ev = document::eval(
                r#"const n = () => { try { return speechSynthesis.getVoices().length; } catch (e) { return 0; } };
                   if (n() > 0) { dioxus.send(n()); } else {
                     let done = false;
                     const fin = () => { if (!done) { done = true; dioxus.send(n()); } };
                     try { speechSynthesis.addEventListener("voiceschanged", fin); } catch (e) {}
                     setTimeout(fin, 2000);
                   }"#,
            );
            if let Ok(n) = ev.recv::<usize>().await {
                tts_voices.set(Some(n));
            }
        });
    });
    let tts_now = state.uses_tts();
    let has_recording = state.has_recording();
    let no_voice = tts_now && *tts_voices.read() == Some(0);
    let mut show_load = use_signal(|| false);
    let mut paste = use_signal(String::new);
    let answering = state.answering.read().clone();
    let questions = if answering.is_none() { state.questions() } else { vec![] };

    // ---- TTS: speak the current cue while playing; advance on end
    let gen = *state.tts_gen.read();
    use_effect(move || {
        let playing = *state.playing.read();
        let _ = *state.cue_idx.read();
        let _ = *state.part_idx.read();
        let _gen = *state.tts_gen.read();
        if !state.uses_tts() {
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
        // A browser without a speech engine (e.g. snap Chromium, no voices) errors or "ends"
        // at once; advancing on that would race through the whole codecast in silence.
        let words = text.split_whitespace().count();
        let min_ms = (words as f64 * 60_000.0 / (400.0 * rate.max(0.1))) as u64;
        let js = format!(
            r#"try {{ speechSynthesis.cancel(); }} catch (e) {{}}
               let vs = [];
               try {{ vs = speechSynthesis.getVoices(); }} catch (e) {{}}
               const def = vs.find(v => v.default) || vs[0];
               dioxus.send("voices:" + vs.length + ":" + (def ? def.name + " (" + def.lang + ")" : "none"));
               const u = new SpeechSynthesisUtterance({});
               u.rate = {rate:.2};
               {voice}
               const t0 = performance.now();
               u.onstart = () => dioxus.send("start");
               u.onend = () => dioxus.send("end:" + Math.round(performance.now() - t0));
               u.onerror = (e) => dioxus.send("error:" + e.error);
               try {{ speechSynthesis.resume(); speechSynthesis.speak(u); }} catch (e) {{ dioxus.send("error:" + e); }}
               "#,
            serde_json::to_string(&text).unwrap_or_else(|_| "\"\"".into()),
            voice = voice_js(&state.lang.peek(), state.config.peek().tts_voice.as_deref())
        );
        let my_gen = *state.tts_gen.peek();
        let cue_no = *state.cue_idx.peek() + 1;
        spawn(async move {
            let mut ev = document::eval(&js);
            let mut voices = String::from("?");
            state.tts_info.set(format!("cue {cue_no}: asking browser to speak ({words} words)…"));
            while let Ok(msg) = ev.recv::<String>().await {
                if *state.tts_gen.peek() != my_gen {
                    return;
                }
                if let Some(v) = msg.strip_prefix("voices:") {
                    let (n, name) = v.split_once(':').unwrap_or((v, ""));
                    voices = if n == "0" { "0 voices".into() } else { format!("{n} voices, default {name}") };
                    state.tts_info.set(format!("cue {cue_no}: speak requested · {voices} · waiting for start…"));
                    continue;
                }
                if msg == "start" {
                    state.tts_info.set(format!("cue {cue_no}: speaking · {voices}"));
                    continue;
                }
                if !*state.playing.peek() {
                    return;
                }
                let ms = msg.strip_prefix("end:").and_then(|ms| ms.parse::<u64>().ok());
                let too_fast = ms.is_some_and(|ms| words >= 4 && ms < min_ms);
                let failed = msg.starts_with("error:") && !msg.ends_with("interrupted") && !msg.ends_with("canceled");
                if too_fast || failed {
                    state.playing.set(false);
                    let why = if failed { msg.clone() } else { format!("ended after {} ms, too fast to have spoken {words} words", ms.unwrap_or(0)) };
                    state.tts_info.set(format!(
                        "⚠ TTS failed on cue {cue_no}: {why} · {voices}. The browser has no working speech voice: try another browser (Chrome/Firefox, not snap Chromium) or the 📖 read voice."
                    ));
                    return;
                }
                state.tts_info.set(match ms {
                    Some(ms) => format!("cue {cue_no}: done in {:.1} s · {voices}", ms as f64 / 1000.0),
                    None => format!("cue {cue_no}: {msg} · {voices}"),
                });
                gloo_timers::future::sleep(std::time::Duration::from_millis(350)).await;
                if *state.tts_gen.peek() == my_gen && *state.playing.peek() && !state.advance() {
                    state.playing.set(false);
                }
                return;
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
               {voice}
               u.onend = () => dioxus.send("end");
               u.onerror = (e) => dioxus.send("error:" + e.error);
               setTimeout(() => speechSynthesis.speak(u), 60);
               "#,
            serde_json::to_string(&text).unwrap_or_else(|_| "\"\"".into()),
            voice = voice_js(&state.lang.peek(), state.config.peek().tts_voice.as_deref())
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
            // wait for the element, then stream currentTime; keep playing when the tour comes
            // here from a part spoken by TTS
            let play = if *state.playing.peek() { "a.play().catch(() => {});" } else { "" };
            let mut ev = document::eval(&format!(
                r#"const wait = () => new Promise(r => {{ const f = () => {{ const a = document.getElementById('iwr-audio'); if (a) r(a); else setTimeout(f, 100); }}; f(); }});
                   const a = await wait();
                   {play}
                   a.ontimeupdate = () => dioxus.send(a.currentTime);
                   a.onplay = () => dioxus.send(-1);
                   a.onpause = () => dioxus.send(-2);
                   a.onended = () => dioxus.send(-3);"#
            ));
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
                            if state.uses_tts() {
                                // no recording for that part: TTS speaks it
                                state.playing.set(true);
                                break;
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
            if !pop.is_empty() && pop_closed() != Some((part_idx, cue_idx)) {
                div { class: "pop", key: "{part_idx}-{cue_idx}-{answering.is_some()}", title: "shown with this sentence, not spoken",
                    button { class: "x", title: "close", onclick: move |_| pop_closed.set(Some((part_idx, cue_idx))), "✕" }
                    for l in pop.iter().cloned() {
                        div { class: "pop-row",
                            // a row with backticks is markdown; a bare row is a path, endpoint or command
                            if l.contains('`') { Markdown { text: l } } else { code { "{l}" } }
                        }
                    }
                }
            }
            div { class: "ctl",
                div { class: "row",
                    button { onclick: move |_| { state.tts_gen += 1; state.step(-1); }, disabled: cue_idx == 0, "◀" }
                    button { class: if playing { "active" } else { "" }, onclick: move |_| {
                        let p = *state.playing.read();
                        state.tts_gen += 1;
                        if *state.voice.read() == Voice::Audio && !state.uses_tts() {
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
                if no_voice {
                    div { class: "status", style: "white-space:normal; overflow-wrap:anywhere; color:#e5484d; font-weight:600",
                        "⚠ This browser has no text-to-speech voice: the codecast cannot be spoken. Open it in another browser (Google Chrome or Firefox, not snap Chromium) or pick 📖 read."
                    }
                }
                if tts_now && !tts_info.is_empty() && !(no_voice && tts_info.starts_with('⚠')) {
                    div { class: "status", style: if tts_info.starts_with('⚠') { "white-space:normal; overflow-wrap:anywhere; color:#e5484d; font-weight:600" } else { "white-space:normal; overflow-wrap:anywhere" },
                        title: "browser text-to-speech status", "🔊 {tts_info}" }
                }
                div { class: "row",
                    select { class: "theme-select", disabled: locked, value: match voice { Voice::Tts => "tts", Voice::Audio => "audio", Voice::Read => "read" },
                        onchange: move |e| { state.tts_gen += 1; state.playing.set(false); state.voice.set(match e.value().as_str() { "audio" => Voice::Audio, "read" => Voice::Read, _ => Voice::Tts }); state.save_settings(); },
                        // `selected` per option: the recording option comes and goes (language, loading)
                        option { value: "tts", selected: voice == Voice::Tts, "🔊 voice (TTS)" }
                        if has_recording { option { value: "audio", selected: voice == Voice::Audio, "🎧 recording" } }
                        option { value: "read", selected: voice == Voice::Read, "📖 read" }
                    }
                    select { class: "theme-select", disabled: locked, value: "{lang}", title: "Language: the codecast is translated when played (offline), recordings are English only",
                        onchange: move |e| { state.set_lang(&e.value()); state.save_settings(); },
                        for l in iwr_core::i18n::LANGS {
                            option { value: l.code, "{l.name}" }
                        }
                    }
                }
                VoicePicker {}
                if !lang_info.is_empty() {
                    div { class: "status", style: if lang_info.starts_with('⚠') { "white-space:normal; overflow-wrap:anywhere; color:#e5484d; font-weight:600" } else { "white-space:normal; overflow-wrap:anywhere" }, "🌐 {lang_info}" }
                }
                div { class: "row",
                    label { class: "status", style: "display:flex; gap:4px; align-items:center; cursor:pointer", title: "Full tour: continue with the next part when one ends",
                        input { r#type: "checkbox", checked: tour, onchange: move |e| { state.tour.set(e.checked()); state.save_settings(); } }
                        "full tour"
                    }
                }
                div { class: "row",
                    button { class: "small", title: "The plan: every part of this codecast, one line each; click one to jump there", onclick: move |_| { state.close_pages(); state.set_mode(Mode::Plan); }, "plan" }
                    button { class: "small", onclick: move |_| { let v = *show_load.read(); show_load.set(!v); }, "load…" }
                    button { class: "small", onclick: move |_| state.stop_script(), "✕ close" }
                }
            }
            div { class: "body",
                div { class: "parts",
                    span { class: "status cast-title", "{title} · " }
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

/// A voice's round avatar: its initial on a colour of its own (`♪` for a recording not by a Kokoro voice).
#[component]
fn Avatar(id: String, #[props(default = 26)] size: u32) -> Element {
    let (name, _) = iwr_core::voice::voice_label(&id);
    let letter = if id.is_empty() { "♪".to_string() } else { name.chars().next().map(String::from).unwrap_or_default() };
    let h = crate::generate::hue(&id);
    rsx! { span { class: "vavatar", style: "--h:{h}; width:{size}px; height:{size}px; font-size:{size * 45 / 100}px", "{letter}" } }
}

/// The recorded voice that plays, as a chip; a click lists every voice that recorded this
/// codecast in the current language, with how many parts each covers.
#[component]
fn VoicePicker() -> Element {
    let mut state = use_context::<State>();
    let mut open = use_signal(|| false);
    let Some(s) = state.script.read().clone() else { return rsx! {} };
    let lang = state.lang.read().clone();
    let _ = state.rec_voice.read();
    let voices = iwr_core::i18n::recorded_voices(&s, &lang);
    // one recording with no voice named: nothing to pick
    if voices.is_empty() || (voices.len() == 1 && voices[0].0.is_empty()) {
        return rsx! {};
    }
    let cur = state.recording_voice(&s, &lang);
    let audio_on = *state.voice.read() == Voice::Audio;
    let n = s.parts.len();
    let label = |id: &str| if id.is_empty() { "Original".to_string() } else { iwr_core::voice::voice_label(id).0 };
    let cur_name = label(&cur);
    rsx! {
        div { class: "vpick",
            button { class: if audio_on { "vchip on" } else { "vchip" }, title: "Pick the recorded voice ({voices.len()} recorded)",
                onclick: move |_| { let v = *open.read(); open.set(!v); },
                Avatar { id: cur.clone() }
                span { class: "vlabel",
                    small { if audio_on { "recorded voice" } else { "recording off" } }
                    b { "{cur_name}" }
                }
                span { class: "caret", if *open.read() { "▴" } else { "▾" } }
            }
            if *open.read() {
                div { class: "vpop-shade", onclick: move |_| open.set(false) }
                div { class: "vpop",
                    div { class: "vpop-head", span { "Recorded voices" } em { "{voices.len()}" } }
                    for (id, count) in voices.iter().cloned() {
                        { let sel = audio_on && id == cur;
                          let name = label(&id);
                          let (_, female) = iwr_core::voice::voice_label(&id);
                          let about = if id.is_empty() { "the codecast's own recording".to_string() } else { format!("{} {}", if female { "♀" } else { "♂" }, crate::generate::accent(&id)) };
                          let featured = iwr_core::voice::FEATURED.contains(&id.as_str());
                          let pct = count * 100 / n.max(1);
                          rsx! {
                            button { class: if sel { "vopt sel" } else { "vopt" },
                                onclick: move |_| { state.set_rec_voice(&id); open.set(false); },
                                Avatar { id: id.clone(), size: 34 }
                                span { class: "vtext",
                                    b { "{name}" if featured { span { class: "star", "★" } } }
                                    span { "{about}" }
                                    span { class: "vcover", i { style: "width:{pct}%" } }
                                }
                                span { class: "vcount", "{count}/{n}" small { "parts" } }
                                span { class: "vcheck", if sel { "✓" } }
                            }
                        } }
                    }
                    div { class: "vpop-foot",
                        button { class: if *state.voice.read() == Voice::Tts { "sel" } else { "" }, onclick: move |_| { state.tts_gen += 1; state.playing.set(false); state.voice.set(Voice::Tts); state.save_settings(); open.set(false); }, "🔊 browser voice" }
                        button { class: if *state.voice.read() == Voice::Read { "sel" } else { "" }, onclick: move |_| { state.tts_gen += 1; state.playing.set(false); state.voice.set(Voice::Read); state.save_settings(); open.set(false); }, "📖 read" }
                    }
                }
            }
        }
    }
}
