//! The loading page: what the viewer sees while `codecast serve` analyzes the project.
//!
//! The server is up before the analysis is done (big projects take a while), so the page polls
//! `/api/status` (`iwr_core::progress::Status`) every 200 ms and shows the stage, a bar, the file
//! or function being read, the counts so far and the time. A static export has no status: the
//! page shows an indeterminate bar while `project.json` downloads.

use crate::i18n::t;
use crate::{icons, State};
use dioxus::prelude::*;
use iwr_core::progress::{Stage, Status};

/// The stages the stepper shows, in pipeline order.
const STEPS: [Stage; 4] = [Stage::Reading, Stage::Parsing, Stage::Linking, Stage::Calls];

fn stage_label(stage: Stage) -> String {
    match stage {
        Stage::Idle | Stage::Reading => t::loading::stage_reading().to_string(),
        Stage::Parsing => t::loading::stage_parsing().to_string(),
        Stage::Linking => t::loading::stage_linking().to_string(),
        Stage::Calls => t::loading::stage_calls().to_string(),
        Stage::Done => t::loading::stage_done().to_string(),
        Stage::Failed => t::loading::stage_failed().to_string(),
    }
}

/// Where a stage stands in the pipeline (to tell done steps from coming ones).
fn rank(stage: Stage) -> usize {
    match stage {
        Stage::Idle => 0,
        Stage::Reading => 1,
        Stage::Parsing => 2,
        Stage::Linking => 3,
        Stage::Calls => 4,
        Stage::Done | Stage::Failed => 5,
    }
}

async fn fetch_status() -> Option<Status> {
    let resp = gloo_net::http::Request::get("/api/status").send().await.ok()?;
    if !resp.ok() {
        return None;
    }
    resp.json::<Status>().await.ok()
}

/// Full-page view shown while `State::project` is `None`.
#[component]
pub fn LoadingPage() -> Element {
    let state = use_context::<State>();
    let live = *state.live.read();
    let mut status: Signal<Option<Status>> = use_signal(|| None);
    // follow the analysis; the component goes away with the page once the project is in
    use_future(move || async move {
        loop {
            if *state.live.peek() {
                status.set(fetch_status().await);
            }
            gloo_timers::future::sleep(std::time::Duration::from_millis(200)).await;
        }
    });

    let st = status.read().clone();
    let progress = st.as_ref().map(|s| s.progress.clone()).unwrap_or_default();
    let stage = progress.stage;
    let failed = stage == Stage::Failed;
    let name = st.as_ref().map(|s| s.name.clone()).filter(|n| !n.is_empty());
    let path = st.as_ref().map(|s| s.path.clone()).unwrap_or_default();
    let title = match (&name, live, st.is_some()) {
        (Some(n), _, _) => t::loading::analyzing(n),
        (None, true, _) => t::loading::connecting().to_string(),
        (None, false, _) => t::loading::static_loading().to_string(),
    };
    // the bar: the stage's fraction when it knows its size, else a sliding shimmer. Reading
    // counts up (no total), so it stays indeterminate; the parse and calls stages fill the bar
    let frac = progress.fraction();
    let determinate = frac.is_some() && !failed;
    let width = frac.map(|f| format!("{:.1}%", f * 100.0)).unwrap_or_else(|| "100%".into());
    let secs = st.as_ref().map(|s| s.elapsed_ms as f64 / 1000.0).unwrap_or(0.0);
    let elapsed = t::loading::elapsed(format!("{secs:.1}"));
    let count = |n: usize| -> String {
        // thousands separated for readability (12 345)
        let s = n.to_string();
        let mut out = String::new();
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (s.len() - i) % 3 == 0 {
                out.push('\u{2009}');
            }
            out.push(c);
        }
        out
    };
    let steps_total = if progress.total > 0 { Some(t::loading::of(count(progress.done), count(progress.total))) } else { None };
    let current = progress.current.clone();
    let error = st.as_ref().and_then(|s| s.error.clone());
    let fns = progress.total.max(progress.done);
    rsx! {
        div { class: if failed { "loading failed" } else { "loading" },
            BuildingGraph { stage: rank(stage) }
            div { class: "card",
                div { class: "head",
                    span { class: "glow", icons::Logo { size: 44 } }
                    div {
                        div { class: "title", "{title}" }
                        if !path.is_empty() {
                            div { class: "path", "{path}" }
                        }
                    }
                }
                if live && st.is_some() {
                    ol { class: "steps",
                        for s in STEPS {
                            li { class: if rank(s) < rank(stage) { "done" } else if s == stage || (stage == Stage::Idle && s == Stage::Reading) { "now" } else { "" },
                                span { class: "dot" }
                                "{stage_label(s)}"
                            }
                        }
                    }
                }
                div { class: if determinate { "bar" } else { "bar indeterminate" },
                    div { class: "fill", style: "width:{width}" }
                }
                div { class: "line",
                    span { class: "stage", "{stage_label(stage)}" }
                    if let Some(of) = steps_total {
                        span { class: "of", "{of}" }
                    }
                    span { class: "spacer" }
                    if st.is_some() { span { class: "elapsed", "{elapsed}" } }
                }
                if !current.is_empty() && !failed {
                    // keyed so the fade-in plays again on every new name
                    div { class: "current", key: "{current}", code { "{current}" } }
                }
                if let Some(e) = error {
                    div { class: "error", pre { "{e}" } div { class: "hint", {t::loading::failed_hint()} } }
                } else if st.is_some() {
                    div { class: "counts",
                        div { class: "count", span { class: "n", "{count(progress.files)}" } span { class: "l", {t::loading::files()} } }
                        div { class: "count", span { class: "n", "{count(progress.items)}" } span { class: "l", {t::loading::items()} } }
                        if stage == Stage::Calls || stage == Stage::Done {
                            div { class: "count", span { class: "n", "{count(fns)}" } span { class: "l", {t::loading::functions()} } }
                        }
                        div { class: "count", span { class: "n", "{count(progress.calls)}" } span { class: "l", {t::loading::calls()} } }
                    }
                    div { class: "hint", {t::loading::hint()} }
                }
            }
        }
    }
}

/// The backdrop: a small graph drawing itself, boxes lighting up and edges growing, on a loop.
/// `stage` (0..=5) sets how many boxes are lit so the picture advances with the analysis.
#[component]
fn BuildingGraph(stage: usize) -> Element {
    // boxes in three ranks, like the layered views draw them
    const BOXES: [(f64, f64, f64); 9] = [
        (300.0, 40.0, 96.0),
        (120.0, 150.0, 84.0),
        (300.0, 150.0, 110.0),
        (480.0, 150.0, 84.0),
        (40.0, 260.0, 84.0),
        (190.0, 260.0, 96.0),
        (340.0, 260.0, 84.0),
        (470.0, 260.0, 110.0),
        (620.0, 260.0, 84.0),
    ];
    const EDGES: [(usize, usize); 9] = [(0, 1), (0, 2), (0, 3), (1, 4), (1, 5), (2, 5), (2, 6), (3, 7), (3, 8)];
    let lit = (stage * 2).min(BOXES.len());
    rsx! {
        svg { class: "backdrop", view_box: "0 0 740 340", preserve_aspect_ratio: "xMidYMid meet",
            for (i, (a, b)) in EDGES.iter().enumerate() {
                {
                    let (ax, ay, aw) = BOXES[*a];
                    let (bx, by, bw) = BOXES[*b];
                    let (x1, y1, x2, y2) = (ax + aw / 2.0, ay + 34.0, bx + bw / 2.0, by);
                    let my = (y1 + y2) / 2.0;
                    let d = format!("M{x1:.0} {y1:.0} C{x1:.0} {my:.0} {x2:.0} {my:.0} {x2:.0} {y2:.0}");
                    rsx! { path { class: if *b < lit { "edge lit" } else { "edge" }, d: "{d}", style: "animation-delay:{i as f64 * 0.35}s" } }
                }
            }
            for (i, (x, y, w)) in BOXES.iter().enumerate() {
                rect { class: if i < lit { "box lit" } else { "box" }, x: "{x}", y: "{y}", width: "{w}", height: "34", rx: "7", style: "animation-delay:{i as f64 * 0.3}s" }
            }
        }
    }
}
