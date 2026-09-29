//! Tiny markdown subset renderer for guide text: fenced code, bullets, `code`, **bold**.

use dioxus::prelude::*;

fn inline(s: &str) -> Vec<Element> {
    let mut out = Vec::new();
    let mut rest = s;
    while !rest.is_empty() {
        let ci = rest.find('`');
        let bi = rest.find("**");
        match (ci, bi) {
            (Some(c), b) if b.map(|b| c < b).unwrap_or(true) => {
                if c > 0 {
                    let t = rest[..c].to_string();
                    out.push(rsx! { "{t}" });
                }
                if let Some(end) = rest[c + 1..].find('`') {
                    let code = rest[c + 1..c + 1 + end].to_string();
                    out.push(rsx! { code { "{code}" } });
                    rest = &rest[c + 2 + end..];
                } else {
                    let t = rest[c..].to_string();
                    out.push(rsx! { "{t}" });
                    rest = "";
                }
            }
            (_, Some(b)) => {
                if b > 0 {
                    let t = rest[..b].to_string();
                    out.push(rsx! { "{t}" });
                }
                if let Some(end) = rest[b + 2..].find("**") {
                    let bold = rest[b + 2..b + 2 + end].to_string();
                    out.push(rsx! { b { "{bold}" } });
                    rest = &rest[b + 4 + end..];
                } else {
                    let t = rest[b..].to_string();
                    out.push(rsx! { "{t}" });
                    rest = "";
                }
            }
            _ => {
                let t = rest.to_string();
                out.push(rsx! { "{t}" });
                rest = "";
            }
        }
    }
    out
}

#[component]
pub fn Markdown(text: String) -> Element {
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
