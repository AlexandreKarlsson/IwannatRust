//! Topics: the other codecasts about the same code, kept in sub-folders of `codecast/`.
//!
//! `codecast serve` lists them (`GET /api/topics`, `iwr_core::casts::Topics`), a static export
//! ships `topics.json` and one folder per topic (`topics/<dir>/`, picked by `?topic=<dir>`).
//! The plan page offers them under the parts (alone when there is no general codecast), and a
//! card proposes them when the codecast ends. Opening one asks the server to play it
//! (`POST /api/topic`: same project, nothing is analyzed again) and reloads the page on its plan.

use crate::i18n::t;
use crate::State;
use dioxus::prelude::*;
use iwr_core::casts::{CastInfo, Topics};
use std::rc::Rc;

/// The topic a static export's page plays, from `?topic=<dir>`; empty for the general codecast
/// (and for a name that could leave the `topics/` folder).
pub fn url_topic() -> String {
    let from_url = web_sys::window().and_then(|w| w.location().search().ok()).and_then(|q| web_sys::UrlSearchParams::new_with_str(&q).ok()).and_then(|p| p.get("topic"));
    from_url.filter(|d| !d.is_empty() && !d.starts_with('.') && !d.chars().any(|c| "/\\?#%\"'".contains(c) || c.is_control())).unwrap_or_default()
}

/// Where a static export keeps the files of the codecast that plays: `""` for the general
/// one, `topics/<dir>/` for a topic.
pub fn static_base() -> String {
    let t = url_topic();
    if t.is_empty() { String::new() } else { format!("topics/{t}/") }
}

impl State {
    /// Fetch the topics of the codecast (`/api/topics` live, `topics.json` in an export).
    pub async fn load_topics(&mut self, live: bool) {
        if let Some(t) = fetch_topics(live).await {
            self.topics.set(Some(Rc::new(t)));
        }
    }

    /// Does the server now play another topic than this page (changed in another tab)?
    pub async fn topic_changed(&self) -> bool {
        let Some(now) = fetch_topics(true).await else { return false };
        let mine = self.topics.peek().as_ref().map(|t| t.current.clone()).unwrap_or_default();
        now.current != mine
    }

    pub fn has_topics(&self) -> bool {
        self.topics.read().as_ref().is_some_and(|t| !t.topics.is_empty())
    }

    /// Is the project's own codecast the one loaded (not the tutorial or the built-in tour)?
    pub fn plays_own(&self) -> bool {
        let own = self.own_script.read();
        let src = self.script_src.read();
        match (own.as_ref(), src.as_ref()) {
            (Some((own, _, _)), Some(s)) => own.title == s.title && own.parts.len() == s.parts.len(),
            _ => false,
        }
    }
}

async fn fetch_topics(live: bool) -> Option<Topics> {
    let resp = gloo_net::http::Request::get(if live { "/api/topics" } else { "topics.json" }).send().await.ok()?;
    if !resp.ok() {
        return None;
    }
    // a static server may answer an unknown path with the page itself: not JSON, no topics
    let mut t = resp.json::<Topics>().await.ok()?;
    if !live {
        let cur = url_topic();
        t.current = if t.topics.iter().any(|x| x.dir == cur) { cur } else { String::new() };
    }
    Some(t)
}

/// Play the topic `dir` (`""`: the general codecast), then reload the page on its plan; `Err`
/// is the server's reason (`recording`, or a message).
pub async fn open(live: bool, dir: &str) -> Result<(), String> {
    if live {
        let r = gloo_net::http::Request::post("/api/topic").json(&serde_json::json!({ "dir": dir })).map_err(|e| e.to_string())?.send().await.map_err(|e| e.to_string())?;
        if !r.ok() {
            let t = r.text().await.unwrap_or_default();
            return Err(if t.is_empty() { r.status().to_string() } else { t });
        }
    }
    // an export keeps the topic in the URL; a server remembers it
    let topic = if live || dir.is_empty() { "q.delete('topic');".to_string() } else { format!("q.set('topic', {});", serde_json::to_string(dir).unwrap_or_default()) };
    document::eval(&format!("const q = new URLSearchParams(location.search); q.delete('render'); q.set('page', 'plan'); {topic} location.search = q.toString();"));
    Ok(())
}

/// What a codecast's card says under its name: parts, guessed length, recorded languages.
pub fn meta(c: &CastInfo) -> String {
    let mut out = format!("{} · ≈ {}", t::castmap::parts(c.parts), crate::generate::clock(c.secs));
    if !c.langs.is_empty() {
        out.push_str(" · ");
        out.push_str(&c.langs.join(" "));
    }
    out
}

/// One card per codecast to go to from the one that plays: the general one (from a topic),
/// then the other topics. Draws nothing when there is nowhere to go.
#[component]
pub fn TopicCards() -> Element {
    let state = use_context::<State>();
    let mut problem: Signal<Option<String>> = use_signal(|| None);
    let mut going: Signal<Option<String>> = use_signal(|| None);
    let Some(topics) = state.topics.read().clone() else { return rsx! {} };
    let live = *state.live.read();
    // (dir, name, small line, tooltip)
    let mut cards: Vec<(String, String, String, String)> = Vec::new();
    if topics.general && !topics.current.is_empty() {
        let name = if topics.title.trim().is_empty() { t::topics::general().to_string() } else { topics.title.clone() };
        cards.push((String::new(), name, t::topics::general().to_string(), t::topics::open_general_tip().to_string()));
    }
    for tp in topics.topics.iter().filter(|tp| tp.dir != topics.current) {
        cards.push((tp.dir.clone(), tp.label.clone(), meta(&tp.info), t::topics::open_tip(&tp.label)));
    }
    if cards.is_empty() {
        return rsx! {};
    }
    let busy = going.read().is_some();
    rsx! {
        div { class: "topic-cards",
            for (dir, name, small, tip) in cards {
                {
                    let is_going = going.read().as_deref() == Some(dir.as_str());
                    let key = if dir.is_empty() { "general".to_string() } else { dir.clone() };
                    rsx! {
                        button { key: "{key}", class: if is_going { "topic-card going" } else if dir.is_empty() { "topic-card general" } else { "topic-card" }, title: "{tip}", disabled: busy,
                            onclick: move |_| {
                                let dir = dir.clone();
                                going.set(Some(dir.clone()));
                                problem.set(None);
                                spawn(async move {
                                    if let Err(e) = open(live, &dir).await {
                                        going.set(None);
                                        problem.set(Some(if e == "recording" { t::castmap::busy_recording().to_string() } else { t::castmap::failed(e) }));
                                    }
                                });
                            },
                            span { class: "topic-name", "{name}" }
                            span { class: "topic-meta", "{small}" }
                        }
                    }
                }
            }
        }
        if let Some(p) = problem.read().clone() {
            div { class: "topic-problem", "{p}" }
        }
    }
}

/// The card shown when the project's codecast has played to its end: where to go next.
#[component]
pub fn EndCard() -> Element {
    let mut state = use_context::<State>();
    rsx! {
        // no height of its own: the card hangs above the player bar that follows
        div { class: "topic-end-anchor",
            div { class: "topic-end",
                div { class: "topic-end-head",
                    span { {t::topics::ended()} }
                    button { class: "topic-end-close", title: t::topics::dismiss(), "aria-label": t::topics::dismiss(), onclick: move |_| state.ended.set(false), "✕" }
                }
                TopicCards {}
            }
        }
    }
}

pub const CSS: &str = r#"
.topic-cards { display:grid; grid-template-columns: repeat(auto-fill, minmax(210px, 1fr)); gap:10px; }
.topic-card { display:flex; flex-direction:column; gap:3px; padding:11px 14px; border-radius:11px; text-align:left; font:inherit; color:var(--text); cursor:pointer; background:color-mix(in srgb, var(--accent) 10%, var(--panel)); border:1px solid color-mix(in srgb, var(--accent) 50%, var(--border)); transition: transform .12s, box-shadow .12s, background .12s; }
.topic-card:hover:not(:disabled) { transform: translateY(-2px); background:color-mix(in srgb, var(--accent) 20%, var(--panel)); box-shadow: 0 6px 22px -10px var(--accent); }
.topic-card:disabled { cursor:default; opacity:.55; }
.topic-card.general { background:var(--panel); border-style:dashed; }
.topic-card.going { opacity:1; animation: cm-pulse .9s ease-in-out infinite; }
.topic-name { font-weight:700; font-size:14px; }
.topic-meta { font-size:11px; color:var(--muted); font-variant-numeric: tabular-nums; }
.topic-problem { margin-top:10px; border:1px solid color-mix(in srgb, #ef4444 50%, var(--border)); background:color-mix(in srgb, #ef4444 10%, var(--panel2)); border-radius:10px; padding:8px 12px; font-size:12px; }
.plan h3.topics-h { margin:26px 0 4px; font-size:15px; }
.plan .topics-intro { color:var(--muted); margin:0 0 12px; font-size:12px; }
.topic-end-anchor { position:relative; height:0; flex:none; z-index:500; }
.topic-end { position:absolute; left:50%; bottom:14px; transform:translateX(-50%); width:min(720px, calc(100vw - 48px)); max-height:50vh; overflow:auto; padding:14px 16px 16px; border-radius:14px; background:var(--panel); border:1px solid color-mix(in srgb, var(--accent) 45%, var(--border)); box-shadow: 0 24px 60px -24px rgba(0,0,0,.7); animation: topic-up .25s ease-out; }
.topic-end-head { display:flex; align-items:center; gap:10px; margin-bottom:10px; font-size:13px; font-weight:650; }
.topic-end-head span { flex:1; }
.topic-end-close { background:none; border:none; color:var(--muted); font-size:14px; padding:4px 8px; border-radius:8px; cursor:pointer; }
.topic-end-close:hover { background:var(--panel3); color:var(--text); }
.app.render .topic-end { display:none; }
@keyframes topic-up { from { opacity:0; transform:translate(-50%, 10px); } to { opacity:1; transform:translateX(-50%); } }
@media (prefers-reduced-motion: reduce) { .topic-end, .topic-card { animation-duration:.01s !important; transition:none; } }
"#;
