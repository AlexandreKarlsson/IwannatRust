//! Markdown of guide text, notes and answers, drawn from the tree of `iwr_core::markdown`:
//! headings, nested and numbered lists, task lists, tables, quotes, admonitions, links, images,
//! rules, and code blocks coloured in the language of their fence (`iwr_core::highlight`), with
//! their title and a copy button; content tabs; a `mermaid` block drawn as a small diagram.
//! A page (a note of the side panel) glows what the codecast's `!` line names in it: a heading,
//! a list item, a table row or column, an admonition, a tab (which opens), a diagram node.
//! Inline code is shown as written and carries its spoken form (see `iwr_core::speech`).

use crate::i18n::t;
use crate::State;
use dioxus::prelude::*;
use iwr_core::highlight::{self, Lang};
use iwr_core::markdown::{self, Align, Block, Callout, Inline, Namer, Tab};
use std::cell::RefCell;
use iwr_core::speech::{self, Segment};

/// Text with its code: backtick spans and bare tokens that look like code are rendered as
/// written with their spoken form as a tooltip, or in parentheses after them when the viewer
/// asked for that in Settings ([`crate::State::speech_parens`]). A caption stays calm: a
/// backtick span that reads like a word is drawn like one. A `page` frames every backtick span.
fn speech_spans(s: &str, pron: &[(String, String)], parens: bool, page: bool) -> Vec<Element> {
    if page && s.contains('`') && s.matches('`').count() % 2 == 0 {
        let mut out = Vec::new();
        for (n, part) in s.split('`').enumerate() {
            if n % 2 == 0 {
                out.extend(speech_spans(part, pron, parens, false));
            } else if !part.trim().is_empty() {
                let say = speech::say_code(part, pron);
                let tip = t::md::spoken(&say);
                let same = say.eq_ignore_ascii_case(part.trim());
                out.push(rsx! {
                    code { class: if same { "" } else { "spk" }, title: "{tip}", "{part}" }
                    if parens && !same { span { class: "spk-say", " ({say})" } }
                });
            }
        }
        return out;
    }
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
    /// A page to read (a note of the side panel), not a caption: every backtick span is framed.
    page: bool,
    /// What the codecast highlights (`!`), by id; empty outside a page.
    hl: Vec<String>,
    /// Gives headings, list items and the rest their ids as they are drawn.
    namer: RefCell<Namer>,
    /// Ids a translation changed, (as shown, as the codecast names them).
    alias: Vec<(String, String)>,
    /// Where a relative picture is fetched from: the codecast directory URL.
    base: String,
}

impl Ctx {
    fn new(state: &State, page: bool, namer: Namer, alias: Vec<(String, String)>) -> Ctx {
        let hl = if page { state.hl_ids.read().clone() } else { vec![] };
        let base = state.audio_base.read().clone();
        Ctx { pron: state.pronounce.read().clone(), parens: *state.speech_parens.read(), page, hl, namer: RefCell::new(namer), alias, base }
    }

    /// Whether the codecast highlights the thing with this id.
    fn lit(&self, id: &str) -> bool {
        let id = self.alias.iter().find(|(shown, _)| shown == id).map_or(id, |(_, w)| w.as_str());
        !id.is_empty() && self.hl.iter().any(|h| h == id)
    }

    /// `class`, with ` on` when the thing with this id is highlighted.
    fn class(&self, class: &str, id: &str) -> String {
        if self.lit(id) { format!("{class} on") } else { class.to_string() }
    }
}

/// Tabs: a bar of names, one body shown. A click picks a tab; so does the codecast when it
/// highlights the tab or something inside it.
#[component]
fn TabSet(tabs: Vec<Tab>, namer: Namer, page: bool, alias: Vec<(String, String)>) -> Element {
    let state = use_context::<State>();
    let cx = Ctx::new(&state, page, namer, alias);
    let mut sel = use_signal(|| 0usize);
    // the first tab the codecast points into, with what it names in each
    let mut probe = cx.namer.borrow().clone();
    let lit: Vec<bool> = tabs
        .iter()
        .map(|t| {
            let mut ids = vec![markdown::slug(&t.title)];
            markdown::collect(&t.blocks, &mut probe, &mut ids);
            ids.iter().any(|id| cx.lit(id))
        })
        .collect();
    let forced = lit.iter().position(|l| *l);
    use_effect(use_reactive((&forced,), move |(forced,)| {
        if let Some(f) = forced {
            sel.set(f);
        }
    }));
    let cur = sel().min(tabs.len().saturating_sub(1));
    let bodies: Vec<Vec<Element>> = tabs.iter().map(|t| t.blocks.iter().map(|b| block(b, &cx)).collect()).collect();
    rsx! {
        div { class: "mdtabs",
            div { class: "mdtabs-bar",
                for (i, t) in tabs.iter().enumerate() {
                    button {
                        key: "{i}",
                        class: format!("mdtab{}{}", if i == cur { " cur" } else { "" }, if cx.lit(&markdown::slug(&t.title)) { " on" } else { "" }),
                        onclick: move |_| sel.set(i),
                        "{t.title}"
                    }
                }
            }
            for (i, body) in bodies.into_iter().enumerate() {
                div { key: "{i}", class: if i == cur { "mdtab-body" } else { "mdtab-body off" }, for x in body { {x} } }
            }
        }
    }
}

/// Inline pieces: emphasis, links and images around the text runs.
fn inlines(items: &[Inline], cx: &Ctx) -> Vec<Element> {
    let mut out = Vec::new();
    for i in items {
        match i {
            Inline::Text(t) => out.extend(speech_spans(t, &cx.pron, cx.parens, cx.page)),
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
                // a file next to the codecast (`img/x.png`) comes from the codecast directory
                let src = if src.contains("://") || src.starts_with('/') || src.starts_with('#') { src.clone() } else { format!("{}{}", cx.base, src) };
                rsx! { img { class: "mdi", src: "{src}", alt: "{alt}", title: "{alt}" } }
            }),
            Inline::Break => out.push(rsx! { br {} }),
        }
    }
    out
}

/// A code block, coloured when its fence names a language the highlighter knows, under its
/// title when it has one. A button, shown when the pointer is over the block, copies the code.
#[component]
fn CodeBlock(lang: String, title: String, text: String) -> Element {
    let mut copied = use_signal(|| false);
    let lines = highlight::code(Lang::of_fence(&lang), &text);
    let last = lines.len().saturating_sub(1);
    let copy = move |_| {
        let js = format!(
            r#"const t = {};
               try {{ await navigator.clipboard.writeText(t); return true; }} catch (e) {{
                 const a = document.createElement('textarea'); a.value = t; document.body.appendChild(a); a.select();
                 const ok = document.execCommand('copy'); a.remove(); return ok; }}"#,
            serde_json::to_string(&text).unwrap_or_default()
        );
        spawn(async move {
            let ok = document::eval(&js).await.ok().and_then(|v| v.as_bool()).unwrap_or(false);
            copied.set(ok);
        });
    };
    rsx! {
        div { class: "mdcode",
            if !title.is_empty() { div { class: "mdcode-title", "{title}" } }
            div { class: "mdcode-body",
                button {
                    class: if copied() { "mdcopy ok" } else { "mdcopy" },
                    title: if copied() { t::md::copied() } else { t::md::copy() },
                    onclick: copy,
                    onmouseleave: move |_| copied.set(false),
                }
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
    }
}

/// The title of an admonition that has none: the name of its kind.
fn callout_name(kind: Callout) -> String {
    match kind {
        Callout::Note => t::md::adm_note(),
        Callout::Abstract => t::md::adm_abstract(),
        Callout::Info => t::md::adm_info(),
        Callout::Tip => t::md::adm_tip(),
        Callout::Success => t::md::adm_success(),
        Callout::Question => t::md::adm_question(),
        Callout::Warning => t::md::adm_warning(),
        Callout::Failure => t::md::adm_failure(),
        Callout::Danger => t::md::adm_danger(),
        Callout::Bug => t::md::adm_bug(),
        Callout::Example => t::md::adm_example(),
        Callout::Quote => t::md::adm_quote(),
    }
    .to_string()
}

/// An admonition: a frame in the colour of its kind, a title line with its icon, the blocks
/// inside. One that folds is a `<details>`.
fn admonition(kind: Callout, title: &Option<Vec<Inline>>, fold: Option<bool>, blocks: &[Block], cx: &Ctx) -> Element {
    let id = title.as_ref().map(|t| markdown::slug(&markdown::plain(t))).unwrap_or_default();
    let class = cx.class(&format!("adm adm-{}", kind.class()), &id);
    let title: Vec<Element> = match title {
        Some(t) => inlines(t, cx),
        None => {
            let name = callout_name(kind);
            vec![rsx! { "{name}" }]
        }
    };
    let body: Vec<Element> = blocks.iter().map(|b| block(b, cx)).collect();
    match fold {
        Some(open) => rsx! {
            details { class: "{class}", open: open,
                // a folding one always has a line to click
                summary { class: "adm-title", for x in title { {x} } }
                div { class: "adm-body", for x in body { {x} } }
            }
        },
        None => rsx! {
            div { class: "{class}",
                if !title.is_empty() { p { class: "adm-title", for x in title { {x} } } }
                div { class: "adm-body", for x in body { {x} } }
            }
        },
    }
}

/// Whether a `mermaid` block is a diagram [`crate::side::MiniDiagram`] draws: a flowchart or
/// a sequence with something in it.
fn drawable(text: &str) -> bool {
    let d = iwr_core::diagram::parse(text);
    d.seq.is_some() || !d.nodes.is_empty()
}

/// One block, with the blocks inside it.
fn block(b: &Block, cx: &Ctx) -> Element {
    match b {
        Block::Para(p) => {
            // a picture alone in its paragraph is a figure: it grows with the panel
            let fig = matches!(p.as_slice(), [Inline::Image { src, .. }] if !src.is_empty());
            let k = inlines(p, cx);
            rsx! { p { class: if fig { "mdfig" } else { "" }, for x in k { {x} } } }
        }
        Block::Heading(level, h) => {
            let id = cx.namer.borrow_mut().heading(h);
            let class = cx.class(&format!("mdh mdh{level}"), &id);
            let k = inlines(h, cx);
            rsx! { h4 { class: "{class}", for x in k { {x} } } }
        }
        // a diagram: its nodes glow by id
        // (a kind of mermaid diagram the drawing does not know stays its code)
        Block::Code { lang, text, .. } if lang == "mermaid" && drawable(text) => rsx! { div { class: "mdmini", crate::side::MiniDiagram { diagram: iwr_core::diagram::parse(text) } } },
        Block::Tabs(tabs) => {
            // the tabs are drawn by their own component: it starts where the page is, and the
            // page goes on after them
            let namer = cx.namer.borrow().clone();
            markdown::collect(std::slice::from_ref(b), &mut cx.namer.borrow_mut(), &mut vec![]);
            rsx! { TabSet { tabs: tabs.clone(), namer, page: cx.page, alias: cx.alias.clone() } }
        }
        Block::Code { lang, title, text } => rsx! { CodeBlock { lang: lang.clone(), title: title.clone(), text: text.clone() } },
        Block::Admonition { kind, title, fold, blocks } => admonition(*kind, title, *fold, blocks, cx),
        Block::Quote(q) => {
            let k: Vec<Element> = q.iter().map(|b| block(b, cx)).collect();
            rsx! { blockquote { for x in k { {x} } } }
        }
        Block::Rule => rsx! { hr {} },
        Block::List { start, items } => {
            let key = cx.namer.borrow_mut().list();
            let lis: Vec<Element> = items
                .iter()
                .enumerate()
                .map(|(i, it)| {
                    let id = key.as_ref().map(|k| markdown::item_id(k, *start, i)).unwrap_or_default();
                    let k: Vec<Element> = it.blocks.iter().map(|b| block(b, cx)).collect();
                    match it.task {
                        Some(done) => rsx! { li { class: cx.class("task", &id), input { r#type: "checkbox", checked: done, disabled: true } div { for x in k { {x} } } } },
                        None => rsx! { li { class: cx.class("", &id), for x in k { {x} } } },
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
            // a row glows by its first cell, a column by its header
            let cols: Vec<&'static str> = head.iter().map(|c| if cx.lit(&markdown::slug(&markdown::plain(c))) { "on" } else { "" }).collect();
            let row = |r: &Vec<Vec<Inline>>| -> Vec<(&'static str, &'static str, Vec<Element>)> { r.iter().enumerate().map(|(n, c)| (style(n), cols.get(n).copied().unwrap_or(""), inlines(c, cx))).collect() };
            let lit: Vec<&'static str> = rows.iter().map(|r| if r.first().is_some_and(|c| cx.lit(&markdown::slug(&markdown::plain(c)))) { "on" } else { "" }).collect();
            let head = row(head);
            let rows: Vec<_> = rows.iter().map(row).zip(lit).collect();
            rsx! {
                // the frame scrolls a table wider than the panel
                div { class: "mdtw",
                    table { class: "mdt",
                        thead { tr { for (st, on, c) in head { th { class: "{on}", style: "{st}", for x in c { {x} } } } } }
                        tbody { for (r, lit) in rows { tr { class: "{lit}", for (st, on, c) in r { td { class: "{on}", style: "{st}", for x in c { {x} } } } } } }
                    }
                }
            }
        }
    }
}

/// A markdown text. `page`: it is a page to read, laid out with more room (see [`Ctx::page`]).
/// `alias`: the ids of a translated note, (as shown, as the codecast names them).
#[component]
pub fn Markdown(text: String, #[props(default)] page: bool, #[props(default)] alias: Vec<(String, String)>) -> Element {
    let state = use_context::<State>();
    let cx = Ctx::new(&state, page, Namer::default(), alias);
    // bring what the codecast highlights into sight, once it is drawn
    use_effect(move || {
        if page && !state.hl_ids.read().is_empty() {
            document::eval("setTimeout(() => { const e = document.querySelector('.side-note .on'); if (e) e.scrollIntoView({block:'nearest'}); }, 30);");
        }
    });
    let blocks: Vec<Element> = markdown::parse(&text).iter().map(|b| block(b, &cx)).collect();
    rsx! { div { class: "md", for b in blocks { {b} } } }
}
