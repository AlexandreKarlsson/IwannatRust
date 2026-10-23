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
    // `segments` trims its text: keep the spaces next to a **bold** run
    let lead = s.starts_with(char::is_whitespace).then(|| rsx! { " " });
    let trail = s.ends_with(char::is_whitespace).then(|| rsx! { " " });
    lead.into_iter()
        .chain(speech::segments(s, pron).into_iter()
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
        }))
        .chain(trail)
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
    // (indent level, `1.` marker of a numbered item, text)
    let mut bullets: Vec<(usize, Option<String>, String)> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let flush_para = |para: &mut Vec<String>, blocks: &mut Vec<Element>| {
        if !para.is_empty() {
            let t = para.join(" ");
            para.clear();
            let parts = inline(&t);
            blocks.push(rsx! { p { for x in parts { {x} } } });
        }
    };
    let flush_bullets = |bullets: &mut Vec<(usize, Option<String>, String)>, blocks: &mut Vec<Element>| {
        if !bullets.is_empty() {
            let items: Vec<(String, String, Vec<Element>)> = bullets
                .iter()
                .map(|(lvl, num, b)| (format!("margin-left:{}px", lvl * 16), num.clone().unwrap_or_else(|| "•".into()), inline(b)))
                .collect();
            bullets.clear();
            blocks.push(rsx! { ul { class: "mdl", for (style, mark, it) in items { li { style: "{style}", span { class: "lnum", "{mark}" } span { for x in it { {x} } } } } } });
        }
    };
    // `| a | b |` rows; the first is the header, `|---|` lines are skipped
    let flush_rows = |rows: &mut Vec<Vec<String>>, blocks: &mut Vec<Element>| {
        if !rows.is_empty() {
            let mut cells: Vec<Vec<Vec<Element>>> = rows.iter().map(|r| r.iter().map(|c| inline(c)).collect()).collect();
            rows.clear();
            let head = cells.remove(0);
            blocks.push(rsx! { table { class: "mdt",
                thead { tr { for c in head { th { for x in c { {x} } } } } }
                tbody { for r in cells { tr { for c in r { td { for x in c { {x} } } } } } }
            } });
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
        let indent = (line.len() - line.trim_start().len()) / 2;
        if !t.starts_with('|') {
            flush_rows(&mut rows, &mut blocks);
        }
        let numbered = t.split_once(". ").filter(|(n, _)| !n.is_empty() && n.len() <= 3 && n.chars().all(|c| c.is_ascii_digit()));
        if t.is_empty() {
            flush_para(&mut para, &mut blocks);
            flush_bullets(&mut bullets, &mut blocks);
        } else if t.starts_with('|') {
            flush_para(&mut para, &mut blocks);
            flush_bullets(&mut bullets, &mut blocks);
            if !t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) {
                rows.push(t.trim_matches('|').split('|').map(|c| c.trim().to_string()).collect());
            }
        } else if let Some(h) = t.strip_prefix("### ").or_else(|| t.strip_prefix("## ")).or_else(|| t.strip_prefix("# ")) {
            flush_para(&mut para, &mut blocks);
            flush_bullets(&mut bullets, &mut blocks);
            let parts = inline(h);
            blocks.push(rsx! { h4 { class: "mdh", for x in parts { {x} } } });
        } else if let Some(b) = t.strip_prefix("• ").or_else(|| t.strip_prefix("- ")).or_else(|| t.strip_prefix("* ")) {
            flush_para(&mut para, &mut blocks);
            bullets.push((indent, None, b.to_string()));
        } else if let Some((n, b)) = numbered {
            flush_para(&mut para, &mut blocks);
            bullets.push((indent, Some(format!("{}.", n)), b.to_string()));
        } else {
            flush_bullets(&mut bullets, &mut blocks);
            para.push(t.to_string());
        }
    }
    flush_rows(&mut rows, &mut blocks);
    if let Some(c) = code {
        let t = c.join("\n");
        blocks.push(rsx! { pre { "{t}" } });
    }
    flush_para(&mut para, &mut blocks);
    flush_bullets(&mut bullets, &mut blocks);
    rsx! { div { class: "md", for b in blocks { {b} } } }
}
