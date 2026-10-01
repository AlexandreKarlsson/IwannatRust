//! Tiny markdown subset renderer for guide text: fenced code, bullets, `code`, **bold**.
//! Code is shown as written and carries its spoken form (see `iwr_core::speech`).

use crate::State;
use dioxus::prelude::*;
use iwr_core::speech::{self, Segment};

/// Inline markdown: `**bold**`, then code. Code (backtick spans and bare tokens that look like
/// code) is rendered as written with its spoken form as a tooltip, or in parentheses after it
/// when the viewer asked for that in Settings ([`crate::State::speech_parens`]).
fn inline(s: &str, pron: &[(String, String)], parens: bool) -> Vec<Element> {
    let mut out = Vec::new();
    let mut rest = s;
    while !rest.is_empty() {
        match rest.find("**") {
            Some(b) => {
                if b > 0 {
                    out.extend(speech_spans(&rest[..b], pron, parens));
                }
                if let Some(end) = rest[b + 2..].find("**") {
                    let bold = rest[b + 2..b + 2 + end].to_string();
                    out.push(rsx! { b { "{bold}" } });
                    rest = &rest[b + 4 + end..];
                } else {
                    out.extend(speech_spans(&rest[b..], pron, parens));
                    rest = "";
                }
            }
            None => {
                out.extend(speech_spans(rest, pron, parens));
                rest = "";
            }
        }
    }
    out
}

fn speech_spans(s: &str, pron: &[(String, String)], parens: bool) -> Vec<Element> {
    speech::segments(s, pron)
        .into_iter()
        .map(|seg| match seg {
            Segment::Text(t) => rsx! { "{t}" },
            Segment::Code { code, say } => {
                let tip = format!("spoken: {}", say);
                let same = say.eq_ignore_ascii_case(code.trim());
                let backtick_like = code.contains(' ') || code.contains("::") || code.contains('(') || code.contains('<') || code.starts_with('&') || code.starts_with('#');
                rsx! {
                    if backtick_like { code { class: if same { "" } else { "spk" }, title: "{tip}", "{code}" } } else { span { class: if same { "" } else { "spk" }, title: "{tip}", "{code}" } }
                    if parens && !same { span { class: "spk-say", " ({say})" } }
                }
            }
        })
        .collect()
}

#[component]
pub fn Markdown(text: String) -> Element {
    let state = use_context::<State>();
    let pron = state.pronounce.read().clone();
    let parens = *state.speech_parens.read();
    let inline = |t: &str| inline(t, &pron, parens);
    let mut blocks: Vec<Element> = Vec::new();
    let mut code: Option<Vec<String>> = None;
    let mut bullets: Vec<String> = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let flush_para = |para: &mut Vec<String>, blocks: &mut Vec<Element>| {
        if !para.is_empty() {
            let t = para.join(" ");
            para.clear();
            let parts = inline(&t);
            blocks.push(rsx! { p { for x in parts { {x} } } });
        }
    };
    let flush_bullets = |bullets: &mut Vec<String>, blocks: &mut Vec<Element>| {
        if !bullets.is_empty() {
            let items: Vec<Vec<Element>> = bullets.iter().map(|b| inline(b)).collect();
            bullets.clear();
            blocks.push(rsx! { ul { for it in items { li { for x in it { {x} } } } } });
        }
    };
    for line in text.lines() {
        if let Some(c) = code.as_mut() {
            if line.trim_start().starts_with("```") {
                let t = c.join("\n");
                code = None;
                blocks.push(rsx! { pre { "{t}" } });
            } else {
                c.push(line.to_string());
            }
            continue;
        }
        if line.trim_start().starts_with("```") {
            flush_para(&mut para, &mut blocks);
            flush_bullets(&mut bullets, &mut blocks);
            code = Some(Vec::new());
            continue;
        }
        let t = line.trim();
        if t.is_empty() {
            flush_para(&mut para, &mut blocks);
            flush_bullets(&mut bullets, &mut blocks);
        } else if let Some(b) = t.strip_prefix("• ").or_else(|| t.strip_prefix("- ")) {
            flush_para(&mut para, &mut blocks);
            bullets.push(b.to_string());
        } else {
            flush_bullets(&mut bullets, &mut blocks);
            para.push(t.to_string());
        }
    }
    if let Some(c) = code {
        let t = c.join("\n");
        blocks.push(rsx! { pre { "{t}" } });
    }
    flush_para(&mut para, &mut blocks);
    flush_bullets(&mut bullets, &mut blocks);
    rsx! { div { class: "md", for b in blocks { {b} } } }
}
