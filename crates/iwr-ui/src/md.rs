//! Markdown of guide text, notes and answers, drawn from the tree of `iwr_core::markdown`:
//! headings, nested and numbered lists, task lists, tables, quotes, links, images, rules, and
//! code blocks coloured in the language of their fence (`iwr_core::highlight`).
//! Inline code is shown as written and carries its spoken form (see `iwr_core::speech`).

use crate::i18n::t;
use crate::State;
use dioxus::prelude::*;
use iwr_core::highlight::{self, Lang};
use iwr_core::markdown::{self, Align, Block, Inline};
use iwr_core::speech::{self, Segment};

/// Text with its code: backtick spans and bare tokens that look like code are rendered as
/// written with their spoken form as a tooltip, or in parentheses after them when the viewer
/// asked for that in Settings ([`crate::State::speech_parens`]).
fn speech_spans(s: &str, pron: &[(String, String)], parens: bool) -> Vec<Element> {
    // `segments` trims its text: keep the spaces next to a **bold** run
    let lead = s.starts_with(char::is_whitespace).then(|| rsx! { " " });
    let trail = s.ends_with(char::is_whitespace).then(|| rsx! { " " });
    lead.into_iter()
        .chain(speech::segments(s, pron).into_iter()
        .map(|seg| match seg {
            Segment::Text(t) => rsx! { "{t}" },
            Segment::Code { code, say } => {
                let tip = t::md::spoken(&say);
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

/// What the pieces of a text need to know to be drawn.
struct Ctx {
    /// `pronounce:` overrides of the codecast.
    pron: std::rc::Rc<Vec<(String, String)>>,
    /// Spoken forms in parentheses instead of tooltips.
    parens: bool,
}

/// Inline pieces: emphasis, links and images around the text runs.
fn inlines(items: &[Inline], cx: &Ctx) -> Vec<Element> {
    let mut out = Vec::new();
    for i in items {
        match i {
            Inline::Text(t) => out.extend(speech_spans(t, &cx.pron, cx.parens)),
            Inline::Code(c) => out.push(rsx! { code { "{c}" } }),
            Inline::Strong(k) => {
                let k = inlines(k, cx);
                out.push(rsx! { b { for x in k { {x} } } });
            }
            Inline::Em(k) => {
                let k = inlines(k, cx);
                out.push(rsx! { i { for x in k { {x} } } });
            }
            Inline::Strike(k) => {
                let k = inlines(k, cx);
                out.push(rsx! { del { for x in k { {x} } } });
            }
            Inline::Link { href, text } => {
                let k = inlines(text, cx);
                // an address the page must not follow leaves its text
                out.push(if href.is_empty() {
                    rsx! { span { for x in k { {x} } } }
                } else {
                    rsx! { a { href: "{href}", target: "_blank", rel: "noopener noreferrer", for x in k { {x} } } }
                });
            }
            Inline::Image { src, alt } => out.push(if src.is_empty() {
                rsx! { span { "{alt}" } }
            } else {
                rsx! { img { class: "mdi", src: "{src}", alt: "{alt}", title: "{alt}" } }
            }),
            Inline::Break => out.push(rsx! { br {} }),
        }
    }
    out
}

/// A code block, coloured when its fence names a language the highlighter knows.
fn code_block(lang: &str, text: &str) -> Element {
    let lines = highlight::code(Lang::of_fence(lang), text);
    let last = lines.len().saturating_sub(1);
    rsx! {
        pre { class: "mdc",
            for (n, line) in lines.into_iter().enumerate() {
                for (tok, t) in line {
                    span { class: tok.class(), "{t}" }
                }
                if n < last { "\n" }
            }
        }
    }
}

/// One block, with the blocks inside it.
fn block(b: &Block, cx: &Ctx) -> Element {
    match b {
        Block::Para(p) => {
            let k = inlines(p, cx);
            rsx! { p { for x in k { {x} } } }
        }
        Block::Heading(level, h) => {
            let k = inlines(h, cx);
            rsx! { h4 { class: "mdh mdh{level}", for x in k { {x} } } }
        }
        Block::Code { lang, text } => code_block(lang, text),
        Block::Quote(q) => {
            let k: Vec<Element> = q.iter().map(|b| block(b, cx)).collect();
            rsx! { blockquote { for x in k { {x} } } }
        }
        Block::Rule => rsx! { hr {} },
        Block::List { start, items } => {
            let lis: Vec<Element> = items
                .iter()
                .map(|it| {
                    let k: Vec<Element> = it.blocks.iter().map(|b| block(b, cx)).collect();
                    match it.task {
                        Some(done) => rsx! { li { class: "task", input { r#type: "checkbox", checked: done, disabled: true } div { for x in k { {x} } } } },
                        None => rsx! { li { for x in k { {x} } } },
                    }
                })
                .collect();
            match start {
                Some(n) => rsx! { ol { start: "{n}", for x in lis { {x} } } },
                None => rsx! { ul { for x in lis { {x} } } },
            }
        }
        Block::Table { align, head, rows } => {
            let style = |n: usize| match align.get(n) {
                Some(Align::Center) => "text-align:center",
                Some(Align::Right) => "text-align:right",
                _ => "",
            };
            let row = |r: &Vec<Vec<Inline>>| -> Vec<(&'static str, Vec<Element>)> { r.iter().enumerate().map(|(n, c)| (style(n), inlines(c, cx))).collect() };
            let head = row(head);
            let rows: Vec<_> = rows.iter().map(row).collect();
            rsx! {
                table { class: "mdt",
                    thead { tr { for (st, c) in head { th { style: "{st}", for x in c { {x} } } } } }
                    tbody { for r in rows { tr { for (st, c) in r { td { style: "{st}", for x in c { {x} } } } } } }
                }
            }
        }
    }
}

#[component]
pub fn Markdown(text: String) -> Element {
    let state = use_context::<State>();
    let cx = Ctx { pron: state.pronounce.read().clone(), parens: *state.speech_parens.read() };
    let blocks: Vec<Element> = markdown::parse(&text).iter().map(|b| block(b, &cx)).collect();
    rsx! { div { class: "md", for b in blocks { {b} } } }
}
