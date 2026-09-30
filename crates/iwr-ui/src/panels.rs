//! Top bar, sidebar (item tree), details/source panel, guide bar.

use crate::State;
use dioxus::prelude::*;
use iwr_core::model::{ItemExtra, ItemKind, ModuleId, Project};
use iwr_core::views::{self, Mode, NodeKind};

#[component]
pub fn TopBar() -> Element {
    let mut state = use_context::<State>();
    let mode = *state.mode.read();
    let flags = *state.flags.read();
    let depth = *state.depth.read();
    let name = state.project.read().as_ref().map(|p| p.name.clone()).unwrap_or_default();
    let has_guide = state.script.read().is_some();
    let search = state.search.read().clone();
    let views_open = *state.views_open.read() || mode != Mode::Code;
    let theme_name = state.theme.read().clone();
    let show_source = *state.show_source.read();
    rsx! {
        div { class: "topbar",
            div { class: "brand", "IwannatRust" span { "{name}" } }
            div { class: "tabs",
                button { class: if mode == Mode::Code { "active" } else { "" }, title: "{Mode::Code.description()}", onclick: move |_| { state.set_mode(Mode::Code); state.views_open.set(false); }, "Code" }
                button { class: if views_open { "active" } else { "" }, title: "Show the analysis views", onclick: move |_| { let v = *state.views_open.read(); state.views_open.set(!v); },
                    if views_open { "Views ▾" } else { "Views ▸" }
                }
                if views_open {
                    for m in Mode::GRAPHS {
                        button { class: if m == mode { "active" } else { "" }, title: "{m.description()}", onclick: move |_| state.set_mode(m), "{m.label()}" }
                    }
                }
            }
            if mode == Mode::Code {
                button { class: if show_source { "active" } else { "" }, title: "Show the source next to the blocks (hover either side to highlight both)", onclick: move |_| { let v = *state.show_source.read(); state.show_source.set(!v); }, "Source ½" }
            }
            div { class: "spacer" }
            select { class: "theme-select", value: "{theme_name}", onchange: move |e| state.set_theme(&e.value()),
                option { value: "dark", "Dark" }
                option { value: "light", "Light" }
                option { value: "paper", "Paper" }
            }
            if mode == Mode::CallTree {
                div { class: "toggles",
                    "depth"
                    button { class: "small", onclick: move |_| { let d = *state.depth.read(); state.depth.set(d.saturating_sub(1).max(1)); }, "−" }
                    span { "{depth}" }
                    button { class: "small", onclick: move |_| { let d = *state.depth.read(); state.depth.set((d + 1).min(12)); }, "+" }
                }
            }
            if mode != Mode::Code {
            div { class: "toggles",
                label { input { r#type: "checkbox", checked: flags.external, onchange: move |e| { let mut f = *state.flags.read(); f.external = e.checked(); state.flags.set(f); } } "external" }
                label { input { r#type: "checkbox", checked: flags.macros, onchange: move |e| { let mut f = *state.flags.read(); f.macros = e.checked(); state.flags.set(f); } } "macros" }
                label { input { r#type: "checkbox", checked: flags.constructs, onchange: move |e| { let mut f = *state.flags.read(); f.constructs = e.checked(); state.flags.set(f); } } "constructors" }
                label { input { r#type: "checkbox", checked: flags.tests, onchange: move |e| { let mut f = *state.flags.read(); f.tests = e.checked(); state.flags.set(f); } } "tests" }
            }
            }
            input { r#type: "text", placeholder: "search items…", value: "{search}", oninput: move |e| state.search.set(e.value()) }
            button { class: if has_guide { "active" } else { "" }, title: "Play the built-in walkthrough (g). Load your own narration from the player bar.", onclick: move |_| { if has_guide { state.stop_script() } else { state.start_builtin_guide(None) } },
                if has_guide { "■ Stop narration" } else { "▶ Narrate" }
            }
        }
    }
}

fn kind_color(it: &iwr_core::model::Item) -> &'static str {
    use crate::theme::node_color;
    match it.kind {
        ItemKind::Function => node_color(if it.fn_info().map(|f| f.is_main).unwrap_or(false) { NodeKind::Main } else { NodeKind::Function }),
        ItemKind::Method => node_color(NodeKind::Method),
        ItemKind::Struct => node_color(NodeKind::Struct),
        ItemKind::Enum => node_color(NodeKind::Enum),
        ItemKind::Trait => node_color(NodeKind::Trait),
        ItemKind::TypeAlias => node_color(NodeKind::TypeAlias),
        ItemKind::Const | ItemKind::Static => node_color(NodeKind::Const),
        ItemKind::Module => node_color(NodeKind::Module),
        ItemKind::Impl => node_color(NodeKind::Impl),
    }
}

#[component]
pub fn Sidebar() -> Element {
    let state = use_context::<State>();
    let _scope = *state.scope.read();
    let project = state.project.read().clone();
    let Some(p) = project else {
        return rsx! { div { class: "sidebar", div { class: "status", "loading project…" } } };
    };
    let search = state.search.read().to_lowercase();
    let selected = *state.selected.read();
    let show_tests = state.flags.read().tests;
    let mut order: Vec<ModuleId> = Vec::new();
    fn visit(p: &Project, m: ModuleId, out: &mut Vec<ModuleId>) {
        out.push(m);
        for c in &p.module(m).children {
            visit(p, *c, out);
        }
    }
    visit(&p, 0, &mut order);
    rsx! {
        div { class: "sidebar",
            for m in order {
                {
                    let module = p.module(m);
                    let items: Vec<&iwr_core::model::Item> = module.items.iter().map(|i| p.item(*i))
                        .filter(|i| i.kind != ItemKind::Method)
                        .filter(|i| show_tests || !i.fn_info().map(|f| f.is_test).unwrap_or(false))
                        .filter(|i| search.is_empty() || i.name.to_lowercase().contains(&search) || matches!(&i.extra, ItemExtra::Impl(ii) if ii.methods.iter().any(|m| p.item(*m).name.to_lowercase().contains(&search))))
                        .collect();
                    let mpath = module.path.clone();
                    let scoped = *state.scope.read() == Some(m);
                    rsx! {
                        if !items.is_empty() {
                            div { class: if scoped { "mod scoped" } else { "mod" }, title: "click to scope the module views to this module",
                                onclick: move |_| { let mut st = state; st.set_scope(if scoped { None } else { Some(m) }); },
                                "{mpath}"
                            }
                            for it in items {
                                SidebarItem { id: it.id, selected, nested: false }
                                {
                                    let methods: Vec<usize> = match &it.extra {
                                        ItemExtra::Impl(ii) => ii.methods.clone(),
                                        ItemExtra::Trait(t) => t.methods.clone(),
                                        _ => vec![],
                                    };
                                    rsx! {
                                        for mid in methods {
                                            if search.is_empty() || p.item(mid).name.to_lowercase().contains(&search) {
                                                SidebarItem { id: mid, selected, nested: true }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SidebarItem(id: usize, selected: Option<usize>, nested: bool) -> Element {
    let mut state = use_context::<State>();
    let Some(p) = state.project.read().clone() else { return rsx! {} };
    let it = p.item(id);
    let color = kind_color(it);
    let label = if it.kind == ItemKind::Impl { it.name.clone() } else { it.name.clone() };
    let title = it.signature.clone();
    rsx! {
        div {
            class: format!("item{}{}", if selected == Some(id) { " sel" } else { "" }, if nested { " nested" } else { "" }),
            title: "{title}",
            onclick: move |_| state.select_item(id),
            ondoubleclick: move |_| {
                let callable = state.project.read().as_ref().map(|p| p.item(id).is_callable()).unwrap_or(false);
                if callable {
                    state.set_mode(Mode::ControlFlow);
                    state.set_root(Some(id));
                }
            },
            span { class: "k", style: "background:{color}", "{it.kind.label()}" }
            span { "{label}" }
        }
    }
}

#[component]
pub fn Details() -> Element {
    let mut state = use_context::<State>();
    let project = state.project.read().clone();
    let selected = *state.selected.read();
    let span = *state.selected_span.read();
    let Some(p) = project else { return rsx! { div { class: "details" } } };
    let info = selected.map(|id| views::item_info(&p, id));
    let item_span = selected.map(|id| p.item(id).span);
    let file = span.or(item_span).map(|s| s.file);
    let mode = *state.mode.read();
    let hl_refs = state.hl.read().clone();
    let code_mark = *state.code_mark.read();

    // scroll the highlighted line into view when the span changes
    use_effect(move || {
        if let Some(s) = *state.selected_span.read() {
            let line = s.line_start;
            document::eval(&format!("setTimeout(() => {{ const el = document.getElementById('L{}'); if (el) el.scrollIntoView({{block:'center'}}); }}, 30);", line));
        }
    });

    rsx! {
        div { class: "details",
            div { class: "info",
                if let Some(info) = &info {
                    h3 { "{info.title} " span { class: "pill", "{info.kind}" } }
                    div { class: "sig", "{info.signature}" }
                    if let Some(d) = &info.doc { div { class: "doc", "{d}" } } else { div { "{info.summary}" } }
                    div { for f in info.facts.iter() { span { class: "pill", "{f}" } } }
                    if !info.params.is_empty() {
                        h4 { if info.kind == "struct" { "fields" } else if info.kind == "enum" { "variants" } else if info.kind == "trait" { "methods" } else { "parameters" } }
                        table { for pa in info.params.iter() { tr { td { class: "n", "{pa.name}" } td { class: "ty", "{pa.ty}" } } } }
                    }
                    if let Some(r) = &info.ret { h4 { "returns" } div { class: "sig", "{r}" } }
                    if !info.calls.is_empty() {
                        h4 { "calls" }
                        div { class: "links", for (id, l) in info.calls.iter() { {let id = *id; rsx!{ a { onclick: move |_| state.select_item(id), "{l}" } }} } }
                    }
                    if !info.called_by.is_empty() {
                        h4 { "called by" }
                        div { class: "links", for (id, l) in info.called_by.iter() { {let id = *id; rsx!{ a { onclick: move |_| state.select_item(id), "{l}" } }} } }
                    }
                    if !info.related.is_empty() {
                        h4 { "related" }
                        div { class: "links", for (id, l) in info.related.iter() { {let id = *id; rsx!{ a { onclick: move |_| state.select_item(id), "{l}" } }} } }
                    }
                    if let Some(id) = selected {
                        if p.item(id).is_callable() && p.item(id).fn_info().map(|f| f.cfg.is_some()).unwrap_or(false) {
                            div { style: "margin-top:8px; display:flex; gap:4px",
                                button { class: "small", onclick: move |_| { state.set_mode(Mode::ControlFlow); state.set_root(Some(id)); }, "control flow" }
                                button { class: "small", onclick: move |_| { state.set_mode(Mode::BranchTree); state.set_root(Some(id)); }, "branches" }
                                button { class: "small", onclick: move |_| { state.set_mode(Mode::CallTree); state.set_root(Some(id)); }, "call tree" }
                                if mode != Mode::ControlFlow {
                                    button { class: "small", onclick: move |_| state.start_builtin_guide(Some(id)), "narrate from here" }
                                }
                            }
                        }
                    }
                } else {
                    div { class: "status", "Click a node or an item in the sidebar to see its details and source. Hover for a quick summary." }
                }
            }
            div { class: "source", onmouseleave: move |_| state.hover_span.set(None),
                if let Some(f) = file {
                    {
                        let sf = p.file(f);
                        let hl = span.filter(|s| s.file == f);
                        let fn_span = item_span.filter(|s| s.file == f);
                        let path = sf.path.clone();
                        rsx! {
                            div { class: "path", "{path}" }
                            for (i, line) in sf.content.lines().enumerate() {
                                {
                                    let ln = i + 1;
                                    let is_hl = hl.map(|s| s.contains_line(ln)).unwrap_or(false) || hl_refs.iter().any(|r| r.inner && r.span.file == f && r.span.contains_line(ln));
                                    let in_fn = fn_span.map(|s| s.contains_line(ln)).unwrap_or(false);
                                    let (pre, mid, post) = match code_mark.filter(|m| m.file == f && m.contains_line(ln)) {
                                        Some(m) if m.col_end > 1 && m.line_start == m.line_end => {
                                            let chars: Vec<char> = line.chars().collect();
                                            let a = m.col_start.saturating_sub(1).min(chars.len());
                                            let b = m.col_end.min(chars.len()).max(a);
                                            (chars[..a].iter().collect::<String>(), chars[a..b].iter().collect::<String>(), chars[b..].iter().collect::<String>())
                                        }
                                        Some(_) => (String::new(), line.to_string(), String::new()),
                                        None => (line.to_string(), String::new(), String::new()),
                                    };
                                    rsx! {
                                        div { class: format!("line{}{}", if is_hl { " hl" } else { "" }, if in_fn { " fn" } else { "" }), id: "L{ln}",
                                            onmouseenter: move |_| state.hover_span.set(Some(iwr_core::model::Span { file: f, line_start: ln, col_start: 1, line_end: ln, col_end: 1 })),
                                            span { class: "ln", "{ln}" }
                                            span { "{pre}" if !mid.is_empty() { span { class: "mark", "{mid}" } } "{post}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    div { class: "path", "source" }
                }
            }
        }
    }
}

