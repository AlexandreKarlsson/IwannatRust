//! "Code" view: the source as simple nested blocks, side by side with the real
//! source. Hovering a block highlights its lines and vice versa.

use crate::theme::node_color;
use crate::State;
use dioxus::prelude::*;
use iwr_core::model::{Block, BlockKind, Item, ItemExtra, ItemKind, Span};
use iwr_core::script::Resolved;
use iwr_core::views::{Mode, NodeKind};
use std::rc::Rc;

pub fn block_color(k: BlockKind) -> &'static str {
    match k {
        BlockKind::Fn => node_color(NodeKind::Function),
        BlockKind::Let => "#94a3b8",
        BlockKind::Stmt => "#64748b",
        BlockKind::Call => node_color(NodeKind::Call),
        BlockKind::Macro => "#64748b",
        BlockKind::If | BlockKind::ElseIf => node_color(NodeKind::If),
        BlockKind::Else => "#ca8a04",
        BlockKind::Match => node_color(NodeKind::Match),
        BlockKind::Arm => "#fb923c",
        BlockKind::Loop => node_color(NodeKind::Loop),
        BlockKind::Return => node_color(NodeKind::Return),
        BlockKind::Break => node_color(NodeKind::Jump),
        BlockKind::Continue => node_color(NodeKind::Jump),
        BlockKind::Propagate => node_color(NodeKind::Propagate),
        BlockKind::Panic => node_color(NodeKind::Panic),
        BlockKind::Await => node_color(NodeKind::Await),
        BlockKind::Unsafe => "#f87171",
        BlockKind::Closure => "#a78bfa",
    }
}

fn block_word(k: BlockKind) -> &'static str {
    match k {
        BlockKind::Fn => "fn",
        BlockKind::Let => "let",
        BlockKind::Stmt => "",
        BlockKind::Call => "call",
        BlockKind::Macro => "macro",
        BlockKind::If => "if",
        BlockKind::ElseIf => "else if",
        BlockKind::Else => "else",
        BlockKind::Match => "match",
        BlockKind::Arm => "arm",
        BlockKind::Loop => "loop",
        BlockKind::Return => "return",
        BlockKind::Break => "break",
        BlockKind::Continue => "continue",
        BlockKind::Propagate => "?",
        BlockKind::Panic => "panic",
        BlockKind::Await => "await",
        BlockKind::Unsafe => "unsafe",
        BlockKind::Closure => "closure",
    }
}

/// Strip the leading keyword from a label so the coloured tag carries it instead.
fn strip_label(k: BlockKind, label: &str) -> String {
    let prefixes: &[&str] = match k {
        BlockKind::If => &["if "],
        BlockKind::ElseIf => &["else if "],
        BlockKind::Match => &["match "],
        BlockKind::Loop => &["for ", "while ", "loop"],
        BlockKind::Return => &["return ", "return"],
        BlockKind::Let | BlockKind::Call | BlockKind::Propagate | BlockKind::Panic | BlockKind::Await => &["let "],
        _ => &[],
    };
    for p in prefixes {
        if let Some(r) = label.strip_prefix(p) {
            return r.to_string();
        }
    }
    label.to_string()
}

fn contains(outer: &Span, inner: &Span) -> bool {
    outer.file == inner.file && outer.line_start <= inner.line_start && outer.line_end >= inner.line_end
}

#[component]
pub fn CodeView() -> Element {
    let mut state = use_context::<State>();
    let project = state.project.read().clone();
    let Some(p) = project else { return rsx! { div { class: "code-view", div { class: "status", "loading…" } } } };
    // which file?
    let file = (*state.code_file.read())
        .or_else(|| (*state.selected.read()).map(|id| p.item(id).span.file))
        .or_else(|| (*state.root.read()).map(|r| p.item(r).span.file))
        .unwrap_or(0)
        .min(p.files.len().saturating_sub(1));
    let show_source = *state.show_source.read();
    let hover = *state.hover_span.read();
    let sel_span = *state.selected_span.read();
    let hl_refs = state.hl.read().clone();
    let code_mark = *state.code_mark.read();
    let sf = p.file(file);
    // top-level items of this file (methods are rendered under their impl)
    let mut items: Vec<&Item> = p.items.iter().filter(|i| i.span.file == file && i.kind != ItemKind::Method).collect();
    items.sort_by_key(|i| i.span.line_start);
    let n_files = p.files.len();
    let files: Vec<(usize, String)> = p.files.iter().map(|f| (f.id, f.path.clone())).collect();
    let module_doc = p.modules.iter().find(|m| m.file == Some(file)).and_then(|m| m.doc.clone());

    // scroll the source to the selection when it changes
    use_effect(move || {
        if let Some(s) = *state.selected_span.read() {
            document::eval(&format!("setTimeout(() => {{ const el = document.getElementById('S{}'); if (el) el.scrollIntoView({{block:'center'}}); }}, 30);", s.line_start));
        }
    });

    // hovering a source line → deepest block containing it
    let p2 = p.clone();
    let line_hover = move |ln: usize| {
        let mut best: Option<Span> = None;
        for it in p2.items.iter().filter(|i| i.span.file == file && i.span.contains_line(ln)) {
            if let Some(b) = it.fn_info().and_then(|f| f.blocks.as_ref()) {
                if let Some(d) = b.deepest_at(ln) {
                    best = Some(d.span);
                    break;
                }
            }
            if best.map(|b| !contains(&b, &it.span) || b.line_end - b.line_start > it.span.line_end - it.span.line_start).unwrap_or(true) {
                best = Some(it.span);
            }
        }
        state.hover_span.set(Some(best.unwrap_or(Span { file, line_start: ln, col_start: 1, line_end: ln, col_end: 1 })));
    };

    rsx! {
        div { class: "code-view",
            div { class: if show_source { "blocks-pane half" } else { "blocks-pane" }, onmouseleave: move |_| state.hover_span.set(None),
                div { class: "file-bar",
                    if n_files > 1 {
                        select { value: "{file}", onchange: move |e| { if let Ok(id) = e.value().parse::<usize>() { state.code_file.set(Some(id)); } },
                            for (id, path) in files.iter() {
                                option { value: "{id}", selected: *id == file, "{path}" }
                            }
                        }
                    } else {
                        span { class: "path", "{sf.path}" }
                    }
                    span { class: "status", "{items.len()} items · click a block to open it" }
                }
                if let Some(d) = module_doc {
                    div { class: "moddoc", "{d}" }
                }
                for it in items {
                    ItemBlock { id: it.id, hover, sel_span, hl: hl_refs.clone() }
                }
            }
            if show_source {
                div { class: "source-pane", onmouseleave: move |_| state.hover_span.set(None),
                    div { class: "path", "{sf.path}" }
                    for (i, line) in sf.content.lines().enumerate() {
                        {
                            let ln = i + 1;
                            let is_hover = hover.map(|h| h.file == file && h.contains_line(ln)).unwrap_or(false);
                            let is_sel = sel_span.map(|s| s.file == file && s.contains_line(ln)).unwrap_or(false);
                            let is_nar = hl_refs.iter().any(|r| r.span.file == file && r.inner && r.span.contains_line(ln));
                            let mark = code_mark.filter(|m| m.file == file && m.contains_line(ln));
                            let mut lh = line_hover.clone();
                            let (pre, mid, post) = match mark {
                                Some(m) if m.col_end > 1 && m.line_start == m.line_end => {
                                    let chars: Vec<char> = line.chars().collect();
                                    let a = (m.col_start.saturating_sub(1)).min(chars.len());
                                    let b = m.col_end.min(chars.len()).max(a);
                                    (chars[..a].iter().collect::<String>(), chars[a..b].iter().collect::<String>(), chars[b..].iter().collect::<String>())
                                }
                                Some(_) => (String::new(), line.to_string(), String::new()),
                                None => (line.to_string(), String::new(), String::new()),
                            };
                            rsx! {
                                div { class: format!("line{}{}{}", if is_hover { " hl" } else { "" }, if is_sel { " fn" } else { "" }, if is_nar { " nar" } else { "" }), id: "S{ln}",
                                    onmouseenter: move |_| lh(ln),
                                    span { class: "ln", "{ln}" }
                                    span { "{pre}" if !mid.is_empty() { span { class: "mark", "{mid}" } } "{post}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn item_kind_color(it: &Item) -> &'static str {
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
fn ItemBlock(id: usize, hover: Option<Span>, sel_span: Option<Span>, hl: Vec<Resolved>) -> Element {
    let mut state = use_context::<State>();
    let Some(p) = state.project.read().clone() else { return rsx! {} };
    let it = p.item(id);
    let key = format!("i{}", id);
    let open = state.open_blocks.read().contains(&key);
    let color = item_kind_color(it);
    let span = it.span;
    let is_hover = hover.map(|h| h == span || (!open && contains(&span, &h))).unwrap_or(false);
    let is_nar = hl.iter().any(|r| (!r.inner && r.item == Some(id)) || (r.inner && !open && contains(&span, &r.span)));
    let is_sel = *state.selected.read() == Some(id);
    let (children_items, has_body): (Vec<usize>, bool) = match &it.extra {
        ItemExtra::Impl(i) => (i.methods.clone(), true),
        ItemExtra::Trait(t) => (t.methods.clone(), true),
        ItemExtra::Fn(f) => (vec![], f.blocks.is_some()),
        _ => (vec![], false),
    };
    let chips: Vec<(String, String)> = match &it.extra {
        ItemExtra::Struct(s) => s.fields.iter().map(|f| (f.name.clone(), f.ty.clone())).collect(),
        ItemExtra::Enum(e) => e.variants.iter().map(|v| (v.name.clone(), v.fields.iter().map(|f| f.ty.clone()).collect::<Vec<_>>().join(", "))).collect(),
        _ => vec![],
    };
    let title = match it.kind {
        ItemKind::Impl => it.name.clone(),
        _ => it.name.clone(),
    };
    let sig_rest = it.signature.clone();
    let doc = it.doc.as_ref().and_then(|d| d.lines().next().map(|l| l.to_string()));
    let n_children = it.fn_info().and_then(|f| f.blocks.as_ref()).map(|b| b.children.len()).unwrap_or(children_items.len());
    let key2 = key.clone();
    let expandable = has_body && n_children > 0;
    let blocks = it.fn_info().and_then(|f| f.blocks.clone());
    let fn_badges: Vec<String> = it.fn_info().map(|f| {
        let mut b = Vec::new();
        if f.returns_result { b.push("Result".into()); }
        if f.returns_option { b.push("Option".into()); }
        if f.is_async { b.push("async".into()); }
        if f.is_recursive { b.push("↻ recursive".into()); }
        b
    }).unwrap_or_default();
    rsx! {
        div { class: format!("iblk{}{}{}{}", if is_hover { " hl" } else { "" }, if is_sel { " sel" } else { "" }, if open { " open" } else { "" }, if is_nar { " nar" } else { "" }), style: "--c: {color}",
            div { class: "ihead",
                onmouseenter: move |_| state.hover_span.set(Some(span)),
                onclick: move |_| {
                    state.selected.set(Some(id));
                    state.selected_span.set(Some(span));
                    if expandable {
                        let mut ob = state.open_blocks.write();
                        if !ob.remove(&key2) { ob.insert(key2.clone()); }
                    }
                },
                span { class: "tag", "{it.kind.label()}" }
                span { class: "name", "{title}" }
                span { class: "sig", "{sig_rest}" }
                for b in fn_badges.iter() { span { class: "chip fact", "{b}" } }
                if expandable {
                    span { class: "count", if open { "▾" } else { "▸ {n_children}" } }
                }
                if it.is_callable() {
                    span { class: "chip act", title: "control flow of this function", onclick: move |e| { e.stop_propagation(); state.set_mode(Mode::ControlFlow); state.set_root(Some(id)); }, "flow" }
                    span { class: "chip act", title: "call tree from this function", onclick: move |e| { e.stop_propagation(); state.set_mode(Mode::CallTree); state.set_root(Some(id)); }, "calls" }
                }
            }
            if let Some(d) = doc {
                div { class: "idoc", "{d}" }
            }
            if !chips.is_empty() {
                div { class: "chips",
                    for (n, t) in chips.iter() {
                        span { class: "chip var", title: "{t}", "{n}" }
                    }
                }
            }
            if open {
                div { class: "ibody",
                    if let Some(b) = blocks {
                        for c in b.children.iter() {
                            BlockView { item: id, block: Rc::new(c.clone()), hover, depth: 0, hl: hl.clone() }
                        }
                    }
                    for m in children_items.iter() {
                        ItemBlock { id: *m, hover, sel_span, hl: hl.clone() }
                    }
                }
            }
        }
    }
}

#[component]
fn BlockView(item: usize, block: Rc<Block>, hover: Option<Span>, depth: usize, hl: Vec<Resolved>) -> Element {
    let mut state = use_context::<State>();
    let key = format!("i{}/b{}", item, block.id);
    let compound = block.kind.is_compound() || !block.children.is_empty();
    let open = compound && state.open_blocks.read().contains(&key);
    let color = block_color(block.kind);
    let span = block.span;
    let is_hover = hover.map(|h| h == span || (!open && contains(&span, &h) && (h.line_start != span.line_start || h.line_end != span.line_end || true))).unwrap_or(false);
    let label = strip_label(block.kind, &block.label);
    let word = block_word(block.kind);
    let n = block.children.len();
    let key2 = key.clone();
    let project = state.project.read().clone();
    let calls: Vec<(usize, String)> = block.calls.iter().filter_map(|c| project.as_ref().map(|p| (*c, p.item(*c).name.clone()))).collect();
    let uses: Vec<String> = block.uses.iter().filter(|u| !block.defines.contains(u)).cloned().collect();
    let hover_is_child = !open && hover.map(|h| contains(&span, &h) && h != span).unwrap_or(false);
    let is_nar = hl.iter().any(|r| r.inner && r.span.file == span.file && (contains(&r.span, &span) || (!open && contains(&span, &r.span))));
    rsx! {
        div { class: format!("blk k-{}{}{}{}", word.replace(' ', "-").replace('?', "try"), if is_hover || hover_is_child { " hl" } else { "" }, if open { " open" } else { "" }, if is_nar { " nar" } else { "" }), style: "--c: {color}",
            div { class: "bhead",
                onmouseenter: move |e| { e.stop_propagation(); state.hover_span.set(Some(span)); },
                onclick: move |e| {
                    e.stop_propagation();
                    state.selected_span.set(Some(span));
                    if compound {
                        let mut ob = state.open_blocks.write();
                        if !ob.remove(&key2) { ob.insert(key2.clone()); }
                    }
                },
                if !word.is_empty() { span { class: "tag", "{word}" } }
                span { class: "label", "{label}" }
                for d in block.defines.iter() { span { class: "chip def", title: "defines {d}", "{d}" } }
                for u in uses.iter() { span { class: "chip var", title: "uses {u}", "{u}" } }
                for (cid, name) in calls.iter() {
                    { let cid = *cid; rsx! { span { class: "chip call", title: "open {name}", onclick: move |e| { e.stop_propagation(); state.open_in_code(cid); }, "→ {name}" } } }
                }
                if compound {
                    span { class: "count", if open { "▾" } else { "▸ {n}" } }
                }
            }
            if open {
                div { class: "bbody",
                    for c in block.children.iter() {
                        BlockView { item, block: Rc::new(c.clone()), hover, depth: depth + 1, hl: hl.clone() }
                    }
                }
            }
        }
    }
}
