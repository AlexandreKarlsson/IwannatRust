//! The prompt card of the Generate page: a few questions → a prompt to paste into an AI agent,
//! which then writes the codecast (`iwr_core::prompt` builds the text). The answers are kept in
//! this browser (`localStorage` `iwr-prompt`).

use crate::i18n::t;
use crate::icons::Icon;
use crate::settings::Toggle;
use crate::State;
use dioxus::prelude::*;
use iwr_core::prompt::{self as pr, Audience, Humour, Metaphor, PromptOptions, Task, Want};

/// A page's name in the interface language.
fn page_name(key: &str) -> &'static str {
    match key {
        "arch" => t::prompt::page_arch(),
        "flow" => t::prompt::page_flow(),
        "code" => t::prompt::page_code(),
        "calls" => t::prompt::page_calls(),
        "sequence" => t::prompt::page_sequence(),
        "types" => t::prompt::page_types(),
        "errors" => t::prompt::page_errors(),
        "branches" => t::prompt::page_branches(),
        "structure" => t::prompt::page_structure(),
        "diagram" => t::prompt::page_diagram(),
        "seqdiagram" => t::prompt::page_seqdiagram(),
        "chart" => t::prompt::page_chart(),
        "scene" => t::prompt::page_scene(),
        "bytes" => t::prompt::page_bytes(),
        "example" => t::prompt::page_example(),
        _ => t::prompt::page_tours(),
    }
}

fn save(o: &PromptOptions) {
    let json = serde_json::to_string(o).unwrap_or_default();
    document::eval(&format!("try {{ localStorage.setItem('iwr-prompt', {}); }} catch (e) {{}}", serde_json::to_string(&json).unwrap_or_default()));
}

/// A row of buttons, one per choice; `set` stores the clicked one.
fn seg<T: Copy + PartialEq + 'static>(mut opts: Signal<PromptOptions>, cur: T, items: Vec<(T, &'static str)>, set: fn(&mut PromptOptions, T)) -> Element {
    rsx! {
        div { class: "gen-quick pr-seg",
            for (v, label) in items {
                button { class: if v == cur { "sel" } else { "" }, onclick: move |_| { let mut o = opts.write(); set(&mut o, v); save(&o); }, "{label}" }
            }
        }
    }
}

/// A labelled text field bound to one string of the options.
fn text(mut opts: Signal<PromptOptions>, label: &'static str, ph: &'static str, value: String, set: fn(&mut PromptOptions, String)) -> Element {
    rsx! {
        label { class: "pr-row",
            span { "{label}" }
            input { r#type: "text", placeholder: ph, value, oninput: move |e| { let mut o = opts.write(); set(&mut o, e.value()); save(&o); } }
        }
    }
}

/// The card: collapsed to a button, open by default when there is no codecast yet.
#[component]
pub fn PromptCard(has_codecast: bool) -> Element {
    let state = use_context::<State>();
    let mut open = use_signal(move || !has_codecast);
    let mut opts = use_signal(move || PromptOptions { task: if has_codecast { Task::Improve } else { Task::New }, ..Default::default() });
    // 0 nothing yet, 1 copied, 2 the browser refused
    let mut copied = use_signal(|| 0u8);
    use_hook(move || {
        spawn(async move {
            if let Ok(v) = document::eval("try { return localStorage.getItem('iwr-prompt') || ''; } catch (e) { return ''; }").await {
                if let Some(o) = v.as_str().and_then(|s| serde_json::from_str::<PromptOptions>(s).ok()) {
                    opts.set(o);
                }
            }
        })
    });
    let path = state.project.read().as_ref().map(|p| p.root.clone()).unwrap_or_default();
    let o = opts.read().clone();
    let prompt = pr::build(&path, &o);
    let copy = {
        let prompt = prompt.clone();
        move |_| {
            let js = format!(
                r#"const t = {};
                   try {{ await navigator.clipboard.writeText(t); return 1; }} catch (e) {{
                     const a = document.createElement('textarea'); a.value = t; document.body.appendChild(a); a.select();
                     const ok = document.execCommand('copy'); a.remove(); return ok ? 1 : 2; }}"#,
                serde_json::to_string(&prompt).unwrap_or_default()
            );
            spawn(async move {
                let r = document::eval(&js).await.ok().and_then(|v| v.as_u64()).unwrap_or(2);
                copied.set(r as u8);
            });
        }
    };

    rsx! {
        section { class: "gen-card pr-card",
            div { class: "gen-step",
                span { Icon { name: "codecast", size: 15 } }
                div {
                    div { class: "gen-kicker", {t::prompt::kicker()} }
                    h3 { {t::prompt::title()} }
                    p { {t::prompt::intro()} }
                }
                button { class: "pr-toggle", onclick: move |_| { let v = *open.read(); open.set(!v) },
                    if *open.read() { {t::prompt::close()} } else { {t::prompt::open()} }
                }
            }
            if *open.read() {
                div { class: "pr-body",
                    div { class: "pr-form",
                        div { class: "pr-row", span { {t::prompt::task_label()} }
                            {seg(opts, o.task, vec![(Task::New, t::prompt::task_new()), (Task::Improve, t::prompt::task_improve()), (Task::AddPart, t::prompt::task_add())], |o, v| o.task = v)}
                        }
                        {text(opts, t::prompt::field_subject(), t::prompt::field_subject_ph(), o.subject.clone(), |o, v| o.subject = v)}
                        div { class: "pr-row", span { {t::prompt::field_audience()} }
                            {seg(opts, o.audience, vec![(Audience::Newcomer, t::prompt::audience_newcomer()), (Audience::RustBeginner, t::prompt::audience_beginner()), (Audience::Expert, t::prompt::audience_expert()), (Audience::Child, t::prompt::audience_child())], |o, v| o.audience = v)}
                        }
                        {text(opts, t::prompt::field_example(), t::prompt::field_example_ph(), o.example.clone(), |o, v| o.example = v)}
                        label { class: "pr-row", span { {t::prompt::field_parts()} }
                            div { class: "pr-parts",
                                input { r#type: "range", min: "0", max: "12", value: "{o.parts}",
                                    oninput: move |e| { let mut o = opts.write(); o.parts = e.value().parse().unwrap_or(0); save(&o); } }
                                b { if o.parts == 0 { {t::prompt::field_parts_auto()} } else { "{o.parts}" } }
                            }
                        }
                        div { class: "pr-row", span { {t::prompt::field_humour()} }
                            {seg(opts, o.humour, vec![(Humour::None, t::prompt::humour_none()), (Humour::Light, t::prompt::humour_light()), (Humour::Playful, t::prompt::humour_playful())], |o, v| o.humour = v)}
                        }
                        div { class: "pr-row", span { {t::prompt::field_metaphor()} }
                            {seg(opts, o.metaphor, vec![(Metaphor::None, t::prompt::metaphor_none()), (Metaphor::Ai, t::prompt::metaphor_ai()), (Metaphor::Mine, t::prompt::metaphor_mine())], |o, v| o.metaphor = v)}
                        }
                        if o.metaphor == Metaphor::Mine {
                            {text(opts, "", t::prompt::field_metaphor_ph(), o.metaphor_text.clone(), |o, v| o.metaphor_text = v)}
                        }
                        div { class: "pr-row pr-top", span { {t::prompt::field_pages()} }
                            div {
                                div { class: "pr-chips",
                                    for (key, _) in pr::PAGES.iter().copied() {
                                        { let w = o.want(key);
                                          let (cls, tag) = match w {
                                              Want::Free => ("", ""),
                                              Want::Prefer => ("prefer", t::prompt::want_prefer()),
                                              Want::Must => ("must", t::prompt::want_must()),
                                              Want::Avoid => ("avoid", t::prompt::want_avoid()),
                                          };
                                          rsx! { button { class: "pr-chip {cls}", onclick: move |_| { let mut o = opts.write(); o.set_want(key, w.next()); save(&o); },
                                              "{page_name(key)}" if !tag.is_empty() { em { "{tag}" } } } } }
                                    }
                                }
                                p { class: "gen-muted pr-help", {t::prompt::field_pages_help()} }
                            }
                        }
                        div { class: "pr-switches",
                            label { Toggle { checked: o.plan_first, onchange: move |v| { let mut o = opts.write(); o.plan_first = v; save(&o); } } {t::prompt::switch_plan_first()} }
                            label { Toggle { checked: o.edit_code, onchange: move |v| { let mut o = opts.write(); o.edit_code = v; save(&o); } } {t::prompt::switch_edit_code()} }
                            label { Toggle { checked: o.two_voices, onchange: move |v| { let mut o = opts.write(); o.two_voices = v; save(&o); } } {t::prompt::switch_two_voices()} }
                            label { Toggle { checked: o.questions, onchange: move |v| { let mut o = opts.write(); o.questions = v; save(&o); } } {t::prompt::switch_questions()} }
                        }
                        {text(opts, t::prompt::field_notes(), t::prompt::field_notes_ph(), o.notes.clone(), |o, v| o.notes = v)}
                        p { class: "gen-muted pr-help", {t::prompt::help()} }
                    }
                    div { class: "pr-out",
                        div { class: "pr-out-head",
                            b { {t::prompt::preview()} }
                            button { class: "pr-reset", onclick: move |_| { let o = PromptOptions { task: if has_codecast { Task::Improve } else { Task::New }, ..Default::default() }; save(&o); opts.set(o); copied.set(0); }, {t::prompt::reset()} }
                        }
                        pre { class: "pr-pre", "{prompt}" }
                        button { class: "gen-go pr-copy", onclick: copy, Icon { name: "check", size: 14 } {t::prompt::copy()} }
                        match *copied.read() {
                            1 => rsx! { p { class: "pr-note ok", {t::prompt::copied()} } },
                            2 => rsx! { p { class: "pr-note", {t::prompt::copy_failed()} } },
                            _ => rsx! {},
                        }
                    }
                }
            }
        }
    }
}
