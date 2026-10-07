//! Colours and CSS. Everything visual lives here so it can be re-skinned.

use iwr_core::views::{EdgeKind, Mode, NodeKind};

pub fn node_color(k: NodeKind) -> &'static str {
    match k {
        NodeKind::Main => "#f59e0b",
        NodeKind::Function => "#60a5fa",
        NodeKind::Method => "#818cf8",
        NodeKind::Struct => "#34d399",
        NodeKind::Enum => "#2dd4bf",
        NodeKind::ErrorType => "#f87171",
        NodeKind::Trait => "#c084fc",
        NodeKind::TypeAlias => "#5eead4",
        NodeKind::Const => "#9ca3af",
        NodeKind::Module => "#94a3b8",
        NodeKind::Impl => "#a78bfa",
        NodeKind::External => "#64748b",
        NodeKind::Macro => "#64748b",
        NodeKind::Entry => "#22c55e",
        NodeKind::Exit => "#ef4444",
        NodeKind::Block => "#94a3b8",
        NodeKind::If => "#eab308",
        NodeKind::Match => "#f97316",
        NodeKind::Loop => "#06b6d4",
        NodeKind::Call => "#3b82f6",
        NodeKind::Return => "#ef4444",
        NodeKind::Jump => "#06b6d4",
        NodeKind::Propagate => "#f43f5e",
        NodeKind::Panic => "#dc2626",
        NodeKind::Await => "#a855f7",
        NodeKind::File => "#94a3b8",
        NodeKind::Note => "#94a3b8",
        NodeKind::Box => "#60a5fa",
        NodeKind::Round => "#34d399",
        NodeKind::Pill => "#c084fc",
        NodeKind::Diamond => "#eab308",
        NodeKind::Cylinder => "#2dd4bf",
        NodeKind::Group => "#94a3b8",
    }
}

pub fn node_kind_label(k: NodeKind) -> &'static str {
    match k {
        NodeKind::Main => "entry point",
        NodeKind::Function => "function",
        NodeKind::Method => "method",
        NodeKind::Struct => "struct",
        NodeKind::Enum => "enum",
        NodeKind::ErrorType => "error type",
        NodeKind::Trait => "trait",
        NodeKind::TypeAlias => "type alias",
        NodeKind::Const => "const",
        NodeKind::Module => "module",
        NodeKind::Impl => "impl block",
        NodeKind::External => "external",
        NodeKind::Macro => "macro",
        NodeKind::Entry => "entry",
        NodeKind::Exit => "exit",
        NodeKind::Block => "statements",
        NodeKind::If => "if",
        NodeKind::Match => "match",
        NodeKind::Loop => "loop",
        NodeKind::Call => "call",
        NodeKind::Return => "return",
        NodeKind::Jump => "break / continue",
        NodeKind::Propagate => "? propagation",
        NodeKind::Panic => "panic",
        NodeKind::Await => "await",
        NodeKind::File => "file",
        NodeKind::Note => "note",
        NodeKind::Box => "box",
        NodeKind::Round => "round",
        NodeKind::Pill => "pill",
        NodeKind::Diamond => "diamond",
        NodeKind::Cylinder => "store",
        NodeKind::Group => "group",
    }
}

/// (colour, dash array, width)
pub fn edge_style(k: EdgeKind) -> (&'static str, &'static str, f64) {
    match k {
        EdgeKind::Call => ("#60a5fa", "", 1.6),
        EdgeKind::CallInLoop => ("#06b6d4", "", 2.4),
        EdgeKind::CallInBranch => ("#eab308", "", 1.6),
        EdgeKind::Recursion => ("#f97316", "6 3", 2.2),
        EdgeKind::Propagate => ("#f43f5e", "5 3", 1.8),
        EdgeKind::Unwrap => ("#dc2626", "2 3", 1.8),
        EdgeKind::Await => ("#a855f7", "", 1.6),
        EdgeKind::Construct => ("#34d399", "1 3", 1.2),
        EdgeKind::Next => ("#94a3b8", "", 1.5),
        EdgeKind::True => ("#22c55e", "", 1.8),
        EdgeKind::False => ("#ef4444", "", 1.8),
        EdgeKind::Arm => ("#f97316", "", 1.6),
        EdgeKind::LoopBody => ("#06b6d4", "", 2.0),
        EdgeKind::LoopBack => ("#06b6d4", "6 3", 1.8),
        EdgeKind::Break => ("#ef4444", "6 3", 1.6),
        EdgeKind::Continue => ("#06b6d4", "2 3", 1.6),
        EdgeKind::Error => ("#f43f5e", "5 3", 1.6),
        EdgeKind::Return => ("#ef4444", "", 1.6),
        EdgeKind::Contains => ("#34d399", "", 1.4),
        EdgeKind::Implements => ("#c084fc", "6 3", 1.8),
        EdgeKind::Supertrait => ("#c084fc", "", 1.8),
        EdgeKind::Uses => ("#94a3b8", "2 3", 1.2),
        EdgeKind::Aliases => ("#5eead4", "6 3", 1.4),
        EdgeKind::Dependency => ("#94a3b8", "", 1.6),
        EdgeKind::Handles => ("#22c55e", "", 1.6),
        EdgeKind::Arrow | EdgeKind::Line => ("#94a3b8", "", 1.8),
        EdgeKind::Dotted | EdgeKind::DottedLine => ("#94a3b8", "3 4", 1.8),
        EdgeKind::Thick | EdgeKind::ThickLine => ("#94a3b8", "", 3.2),
    }
}

pub fn edge_kind_label(k: EdgeKind) -> &'static str {
    match k {
        EdgeKind::Call => "call",
        EdgeKind::CallInLoop => "call inside loop",
        EdgeKind::CallInBranch => "conditional call",
        EdgeKind::Recursion => "recursion",
        EdgeKind::Propagate => "error propagated (?)",
        EdgeKind::Unwrap => "unwrap / expect",
        EdgeKind::Await => "await",
        EdgeKind::Construct => "constructs",
        EdgeKind::Next => "next",
        EdgeKind::True => "true",
        EdgeKind::False => "false / done",
        EdgeKind::Arm => "match arm",
        EdgeKind::LoopBody => "loop body",
        EdgeKind::LoopBack => "next iteration",
        EdgeKind::Break => "break",
        EdgeKind::Continue => "continue",
        EdgeKind::Error => "error exit",
        EdgeKind::Return => "return",
        EdgeKind::Contains => "contains",
        EdgeKind::Implements => "implements",
        EdgeKind::Supertrait => "supertrait",
        EdgeKind::Uses => "used by",
        EdgeKind::Aliases => "alias / From",
        EdgeKind::Dependency => "depends on",
        EdgeKind::Handles => "error handled",
        EdgeKind::Arrow | EdgeKind::Line => "link",
        EdgeKind::Dotted | EdgeKind::DottedLine => "dotted link",
        EdgeKind::Thick | EdgeKind::ThickLine => "strong link",
    }
}

pub struct LegendEntry {
    pub color: &'static str,
    pub label: &'static str,
    pub dash: &'static str,
    pub is_edge: bool,
}

fn n(k: NodeKind) -> LegendEntry {
    LegendEntry { color: node_color(k), label: node_kind_label(k), dash: "", is_edge: false }
}
fn e(k: EdgeKind) -> LegendEntry {
    let (c, d, _) = edge_style(k);
    LegendEntry { color: c, label: edge_kind_label(k), dash: d, is_edge: true }
}

pub fn legend(mode: Mode) -> Vec<LegendEntry> {
    match mode {
        Mode::Code => vec![],
        Mode::CallTree => vec![n(NodeKind::Main), n(NodeKind::Function), n(NodeKind::Method), n(NodeKind::External), e(EdgeKind::Call), e(EdgeKind::CallInLoop), e(EdgeKind::CallInBranch), e(EdgeKind::Recursion), e(EdgeKind::Propagate), e(EdgeKind::Unwrap)],
        Mode::ControlFlow | Mode::BranchTree => vec![n(NodeKind::Entry), n(NodeKind::Block), n(NodeKind::If), n(NodeKind::Match), n(NodeKind::Loop), n(NodeKind::Call), n(NodeKind::Propagate), n(NodeKind::Panic), n(NodeKind::Return), n(NodeKind::Exit), e(EdgeKind::True), e(EdgeKind::False), e(EdgeKind::Arm), e(EdgeKind::LoopBody), e(EdgeKind::LoopBack), e(EdgeKind::Error)],
        Mode::Architecture => vec![n(NodeKind::Module), n(NodeKind::Function), n(NodeKind::Struct), n(NodeKind::Trait), n(NodeKind::External), e(EdgeKind::Contains), e(EdgeKind::Dependency)],
        Mode::Structure => vec![n(NodeKind::Module), n(NodeKind::Function), n(NodeKind::Method), n(NodeKind::Impl), n(NodeKind::Struct), n(NodeKind::Enum), n(NodeKind::Trait), n(NodeKind::Const)],
        Mode::Types => vec![n(NodeKind::Struct), n(NodeKind::Enum), n(NodeKind::ErrorType), n(NodeKind::Trait), n(NodeKind::TypeAlias), n(NodeKind::Function), e(EdgeKind::Contains), e(EdgeKind::Implements), e(EdgeKind::Supertrait), e(EdgeKind::Uses)],
        Mode::ErrorFlow => vec![n(NodeKind::Function), n(NodeKind::Panic), n(NodeKind::ErrorType), e(EdgeKind::Propagate), e(EdgeKind::Unwrap), e(EdgeKind::Handles), e(EdgeKind::Error), e(EdgeKind::Aliases)],
        // a sketch explains itself: no legend
        Mode::Diagram => vec![],
    }
}

/// (id, label, dark?) of every theme; the id is the CSS class suffix.
pub const THEMES: &[(&str, &str, bool)] = &[
    ("dark", "Dark", true),
    ("light", "Light", false),
    ("paper", "Paper", false),
    ("midnight", "Midnight", true),
    ("forest", "Forest", true),
    ("nord", "Nord", true),
    ("solar", "Solar", false),
    ("dusk", "Dusk", true),
];

pub fn is_theme(id: &str) -> bool {
    THEMES.iter().any(|(t, _, _)| *t == id)
}

pub const CSS: &str = r#"
:root, .theme-dark { --bg:#0b1020; --panel:#111827; --panel2:#0f172a; --panel3:#1f2937; --border:#1f2937; --text:#e5e7eb; --muted:#9ca3af; --accent:#60a5fa; --hi:#fbbf24; --hl-bg:#3b2f0b; --sel-bg:#0f1a2e; --code:#93c5fd; --grid:#1f2937; --fill-op:0.28; --icon-filter:invert(.88); --icon-active:none; }
.theme-light { --bg:#f8fafc; --panel:#ffffff; --panel2:#f1f5f9; --panel3:#e2e8f0; --border:#cbd5e1; --text:#0f172a; --muted:#64748b; --accent:#2563eb; --hi:#b45309; --hl-bg:#fef3c7; --sel-bg:#e0f2fe; --code:#1d4ed8; --grid:#e2e8f0; --fill-op:0.18; --icon-filter:invert(.2); --icon-active:invert(1); }
.theme-paper { --bg:#f5efe0; --panel:#fbf7ec; --panel2:#f1eadb; --panel3:#e6dcc5; --border:#d6c9a8; --text:#2b2416; --muted:#7a6d52; --accent:#8a4b08; --hi:#9a3412; --hl-bg:#fde68a; --sel-bg:#e9dfc2; --code:#7c2d12; --grid:#e3d8bd; --fill-op:0.2; --icon-filter:invert(.25) sepia(.5); --icon-active:invert(1); }
.theme-midnight { --bg:#0a0a14; --panel:#12121f; --panel2:#0e0e1a; --panel3:#1c1c30; --border:#26263d; --text:#ececf6; --muted:#8f8fb0; --accent:#a78bfa; --hi:#f0abfc; --hl-bg:#3b1d4a; --sel-bg:#1e1540; --code:#c4b5fd; --grid:#1a1a2e; --fill-op:0.3; --icon-filter:invert(.9); --icon-active:none; }
.theme-forest { --bg:#0c1a12; --panel:#112419; --panel2:#0e1f15; --panel3:#1a3324; --border:#234231; --text:#e3f2e8; --muted:#8fb39d; --accent:#4ade80; --hi:#fbbf24; --hl-bg:#3a3410; --sel-bg:#15301f; --code:#86efac; --grid:#16281d; --fill-op:0.28; --icon-filter:invert(.9); --icon-active:none; }
.theme-nord { --bg:#2e3440; --panel:#3b4252; --panel2:#353b4a; --panel3:#434c5e; --border:#4c566a; --text:#eceff4; --muted:#a3adc2; --accent:#88c0d0; --hi:#ebcb8b; --hl-bg:#4a4330; --sel-bg:#3f4a60; --code:#8fbcbb; --grid:#3b4252; --fill-op:0.3; --icon-filter:invert(.92); --icon-active:none; }
.theme-solar { --bg:#fdf6e3; --panel:#fffbf0; --panel2:#f7efd9; --panel3:#eee8d5; --border:#d9d2b8; --text:#073642; --muted:#657b83; --accent:#268bd2; --hi:#b58900; --hl-bg:#f5e6a8; --sel-bg:#e3eef5; --code:#2aa198; --grid:#ece5cf; --fill-op:0.2; --icon-filter:invert(.15) sepia(.4); --icon-active:invert(1); }
.theme-dusk { --bg:#1a1016; --panel:#241720; --panel2:#1f1319; --panel3:#33202c; --border:#3f2a37; --text:#f5e8ef; --muted:#b48ca2; --accent:#fb7185; --hi:#fcd34d; --hl-bg:#4a3414; --sel-bg:#3a1f30; --code:#fda4af; --grid:#2a1a24; --fill-op:0.3; --icon-filter:invert(.9); --icon-active:none; }
.app { color:var(--text); background:var(--bg); }
* { box-sizing: border-box; }
html, body { margin:0; height:100%; background:var(--bg); color:var(--text); font: 13px/1.45 system-ui, -apple-system, Segoe UI, Roboto, sans-serif; overflow:hidden; }
button { background:var(--panel3); color:var(--text); border:1px solid var(--border); border-radius:6px; padding:4px 9px; cursor:pointer; font-size:12px; }
button:hover { filter:brightness(1.15); }
button.active { background:var(--accent); color:var(--panel); border-color:var(--accent); font-weight:600; }
button.small { padding:2px 6px; font-size:11px; }
input[type=text], select { background:var(--panel2); color:var(--text); border:1px solid var(--border); border-radius:6px; padding:4px 8px; font-size:12px; }
code, pre { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }
code { background:var(--panel3); padding:1px 4px; border-radius:4px; font-size:12px; }
.app { display:flex; flex-direction:column; height:100vh; }
.topbar { display:flex; align-items:center; gap:8px; padding:6px 10px; border-bottom:1px solid var(--border); background:var(--panel); flex-wrap:wrap; position:relative; z-index:20; }
.topbar .brand { font-weight:700; color:var(--accent); margin-right:8px; letter-spacing:.3px; }
.topbar .brand span { color:var(--muted); font-weight:400; font-size:11px; margin-left:6px; }
.tabs { display:flex; gap:4px; flex-wrap:wrap; }
.spacer { flex:1; }
.toggles { display:flex; gap:8px; align-items:center; color:var(--muted); font-size:12px; }
.toggles label { display:flex; gap:3px; align-items:center; cursor:pointer; }
.main { display:flex; flex:1; min-height:0; }
.sidebar { width:250px; flex:none; border-right:1px solid var(--border); background:var(--panel); overflow:auto; padding:6px; }
.resizer { flex:none; width:6px; cursor:col-resize; background:transparent; transition: background .15s; position:relative; z-index:2; margin:0 -3px; }
.resizer:hover, .app.dragging .resizer { background:var(--accent); opacity:.6; }
.app.dragging { user-select:none; cursor:col-resize; }
.app.dragging iframe, .app.dragging svg { pointer-events:none; }
.cam.anim { transition: transform .6s cubic-bezier(.22,.61,.36,1); }
.cam.anim .node { transition: transform .6s cubic-bezier(.22,.61,.36,1); }
.node .body { transition: stroke-width .12s, stroke .35s, filter .35s; }
.cam.anim .node rect { transition: stroke-width .12s, stroke .35s, filter .35s, width .6s cubic-bezier(.22,.61,.36,1), height .6s cubic-bezier(.22,.61,.36,1); }
.edge path { transition: stroke .35s, stroke-width .35s, opacity .35s; }
.sbtabs { display:flex; gap:4px; margin:2px 0 6px; }
.ftree .frow { display:flex; align-items:center; gap:5px; padding:2px 6px; border-radius:4px; cursor:pointer; white-space:nowrap; font-size:12px; }
.ftree .frow:hover { background:var(--panel3); }
.ftree .frow.sel { background:var(--sel-bg); box-shadow: inset 3px 0 0 var(--accent); color:var(--text); font-weight:600; }
.ftree .frow.nar { box-shadow: inset 3px 0 0 var(--hi); background:var(--hl-bg); }
.ftree .frow.sel.nar { box-shadow: inset 3px 0 0 var(--hi), 0 0 0 1px var(--hi); }
.ftree .frow { transition: background .3s, box-shadow .3s; }
.ftree .frow.dir { color:var(--muted); }
.ftree .frow.other { color:var(--muted); }
.ftree .fic { font-size:11px; width:28px; flex:none; }
.ftree .fcount { margin-left:auto; color:var(--muted); font-size:10px; }
.node .detail text { font-size:11px; }
.sidebar .mod { margin:6px 0 2px; color:var(--muted); font-size:11px; text-transform:uppercase; letter-spacing:.5px; cursor:pointer; border-radius:4px; padding:1px 4px; }
.sidebar .mod:hover { background:var(--panel3); color:var(--text); }
.sidebar .mod.scoped { background:var(--hl-bg); color:var(--hi); }
.edge.dim { opacity:.18; }
.edge.near path { stroke-width:3 !important; opacity:1; }
.hud .chip { background:var(--hl-bg); color:var(--hi); border-radius:999px; padding:1px 8px; font-size:11px; cursor:pointer; }
.kinds { display:inline-flex; gap:4px; flex-wrap:wrap; align-items:center; }
.kinds .kind { display:inline-flex; align-items:center; gap:3px; border:1px solid var(--border); border-radius:999px; padding:0 7px 0 4px; font-size:11px; line-height:18px; cursor:pointer; color:var(--text); background:var(--panel3); user-select:none; font-family: ui-monospace, Menlo, Consolas, monospace; }
.kinds .kind.off { color:var(--muted); background:transparent; text-decoration:line-through; }
.kinds .kind input { margin:0; width:11px; height:11px; accent-color:var(--accent); }
.kinds .kind:hover { border-color:var(--accent); }
.theme-select { font-size:12px; padding:3px 6px; border-radius:6px; }

/* ---- Code view: blocks + source */
.code-view { display:flex; flex:1; min-width:0; min-height:0; }
.blocks-pane { flex:1; min-width:0; overflow:auto; padding:10px 14px 40px; }
.blocks-pane.half { flex:0 0 50%; border-right:1px solid var(--border); }
.source-pane { flex:1; min-width:0; overflow:auto; font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; font-size:12px; background:var(--panel2); }
.file-bar { display:flex; align-items:center; gap:10px; margin-bottom:8px; }
.file-bar .path { font-family: ui-monospace, Menlo, Consolas, monospace; color:var(--muted); }
.moddoc { color:var(--muted); font-style:italic; margin:0 0 10px 2px; white-space:pre-wrap; }
.iblk { border-left:4px solid var(--c); background:color-mix(in srgb, var(--c) 10%, var(--panel)); border-radius:8px; margin:6px 0; transition: box-shadow .35s; }
.iblk.hl { box-shadow: 0 0 0 2px var(--hi); }
.iblk.sel { box-shadow: inset 0 0 0 1px var(--accent); }
.iblk.sel.hl { box-shadow: inset 0 0 0 1px var(--accent), 0 0 0 2px var(--hi); }
.ihead { display:flex; align-items:center; gap:8px; padding:7px 10px; cursor:pointer; flex-wrap:wrap; }
.ihead:hover { background:color-mix(in srgb, var(--c) 18%, var(--panel)); border-radius:0 8px 8px 0; }
.ihead .tag, .bhead .tag { background:var(--c); color:var(--panel); font-size:10px; font-weight:700; padding:1px 6px; border-radius:4px; text-transform:uppercase; letter-spacing:.4px; flex:none; }
.ihead .name { font-weight:700; font-size:14px; }
.ihead .sig { color:var(--muted); font-family: ui-monospace, Menlo, Consolas, monospace; font-size:11px; }
.ihead .count, .bhead .count { margin-left:auto; color:var(--muted); font-size:11px; flex:none; }
.idoc { color:var(--muted); padding:0 10px 6px 34px; font-size:12px; }
.chips { padding:0 10px 8px 34px; display:flex; gap:4px; flex-wrap:wrap; }
.ibody { padding:2px 10px 8px 22px; }
.chip { display:inline-block; border-radius:999px; padding:0 7px; font-size:11px; line-height:17px; border:1px solid transparent; }
.chip.var { background:var(--panel3); color:var(--text); font-family: ui-monospace, Menlo, Consolas, monospace; }
.chip.def { background:var(--hl-bg); color:var(--hi); font-family: ui-monospace, Menlo, Consolas, monospace; border-color:var(--hi); }
.chip.def::before { content:"＋ "; font-size:9px; }
.chip.call { background:color-mix(in srgb, #3b82f6 25%, var(--panel)); color:var(--accent); cursor:pointer; font-family: ui-monospace, Menlo, Consolas, monospace; }
.chip.call:hover { filter:brightness(1.2); text-decoration:underline; }
.chip.fact { background:var(--panel3); color:var(--muted); }
.chip.act { background:transparent; border-color:var(--border); color:var(--muted); cursor:pointer; }
.chip.act:hover { color:var(--accent); border-color:var(--accent); }
.blk { border-left:4px solid var(--c); background:color-mix(in srgb, var(--c) 9%, var(--panel)); border-radius:6px; margin:4px 0; transition: box-shadow .35s; }
.blk.hl { box-shadow: 0 0 0 2px var(--hi); }
.bhead { display:flex; align-items:center; gap:6px; padding:5px 8px; cursor:pointer; flex-wrap:wrap; font-family: ui-monospace, Menlo, Consolas, monospace; font-size:12px; }
.bhead:hover { background:color-mix(in srgb, var(--c) 20%, var(--panel)); border-radius:0 6px 6px 0; }
.bhead .label { white-space:pre-wrap; }
.blk.k-loop > .bhead, .blk.k-if > .bhead, .blk.k-else-if > .bhead, .blk.k-else > .bhead, .blk.k-match > .bhead { font-family: system-ui, sans-serif; font-weight:600; }
.blk.k-arm { border-left-style:dashed; }
.blk.k-closure { border-left-style:dotted; }
.bbody { padding:2px 6px 6px 14px; }
.blk.k-return .label, .blk.k-panic .label, .blk.k-try .label { font-weight:600; }
.sidebar .item { padding:2px 6px; border-radius:4px; cursor:pointer; display:flex; gap:6px; align-items:center; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
.sidebar .item:hover { background:var(--panel3); }
.sidebar .item.sel { background:var(--sel-bg); outline:1px solid var(--accent); }
.sidebar .item .k { font-size:10px; color:#0b1020; padding:0 4px; border-radius:3px; font-weight:700; min-width:34px; text-align:center; }
.sidebar .item.nested { padding-left:22px; }
.canvas-wrap { flex:1; position:relative; min-width:0; background: radial-gradient(var(--grid) 1px, transparent 1px) 0 0/22px 22px; }
.canvas-wrap svg { width:100%; height:100%; display:block; cursor:grab; user-select:none; }
.canvas-wrap svg.dragging { cursor:grabbing; }
.node { cursor:pointer; }
.node:hover .body { stroke-width:2.5; filter: brightness(1.15); }
.node.sel .body { stroke-width:3; stroke:var(--text) !important; }
.node.hi .body { stroke:var(--hi) !important; stroke-width:3.5; filter: drop-shadow(0 0 8px rgba(251,191,36,.8)); }
.node.dim { opacity:.35; }
.node text { fill:var(--text); font-size:13px; pointer-events:none; }
.node text.sub { fill:var(--muted); font-size:11px; font-family: ui-monospace, Menlo, Consolas, monospace; }
.node text.cont { font-weight:700; }
.node .expander circle { fill:var(--bg); stroke:var(--muted); stroke-width:1.2; }
.node .expander:hover circle { stroke:var(--text); fill:var(--panel3); }
.node .expander text { fill:var(--text); font-size:13px; font-weight:700; }
.edge-label { font-size:10px; fill:var(--text); pointer-events:none; }
.edge-label-bg { fill:var(--bg); fill-opacity:.85; }
.edge.hi path { stroke:var(--hi) !important; stroke-width:3 !important; }
.hud { position:absolute; left:10px; top:10px; display:flex; gap:6px; align-items:center; white-space:nowrap; }
.hud.hud2 { top:auto; bottom:10px; background:var(--panel); opacity:.95; border:1px solid var(--border); border-radius:8px; padding:3px 8px; }
.legend { position:absolute; right:10px; bottom:10px; background:var(--panel); opacity:.95; border:1px solid var(--border); border-radius:8px; padding:6px 10px; font-size:11px; max-width:260px; display:grid; grid-template-columns:1fr 1fr; gap:2px 12px; }
.legend div { display:flex; align-items:center; gap:6px; white-space:nowrap; }
.legend .sw { width:12px; height:12px; border-radius:3px; display:inline-block; }
.legend .ln { width:18px; height:0; border-top:2px solid; display:inline-block; }
.note { position:absolute; left:50%; top:40%; transform:translate(-50%,-50%); color:var(--muted); background:var(--panel); padding:10px 16px; border-radius:8px; border:1px solid var(--border); }
.tooltip { position:fixed; z-index:50; background:var(--panel); border:1px solid var(--border); border-radius:8px; padding:8px 10px; max-width:420px; pointer-events:none; box-shadow:0 8px 24px rgba(0,0,0,.5); font-size:12px; }
.tooltip .t { font-weight:700; margin-bottom:2px; }
.tooltip .sig { color:var(--code); font-family: ui-monospace, Menlo, Consolas, monospace; white-space:pre-wrap; margin:3px 0; }
.tooltip .doc { color:var(--muted); white-space:pre-wrap; margin-top:4px; }
.tooltip pre { margin:4px 0 0; background:var(--panel2); padding:6px; border-radius:6px; max-height:160px; overflow:hidden; font-size:11px; }
.pill { display:inline-block; background:var(--panel3); border-radius:999px; padding:0 7px; font-size:11px; margin:2px 3px 0 0; color:var(--text); }
.details { width:380px; flex:none; border-left:1px solid var(--border); background:var(--panel); display:flex; flex-direction:column; min-height:0; }
.details .info { padding:10px; border-bottom:1px solid var(--border); overflow:auto; max-height:45%; }
.details h3 { margin:0 0 4px; font-size:14px; }
.details .sig { color:var(--code); font-family: ui-monospace, Menlo, Consolas, monospace; white-space:pre-wrap; font-size:12px; margin:4px 0; }
.details .doc { color:var(--text); opacity:.85; white-space:pre-wrap; margin:6px 0; font-size:12px; }
.details table { border-collapse:collapse; font-size:12px; margin:4px 0; }
.details td { padding:1px 8px 1px 0; vertical-align:top; }
.details td.n { color:var(--hi); font-family: ui-monospace, Menlo, Consolas, monospace; }
.details td.ty { color:var(--code); font-family: ui-monospace, Menlo, Consolas, monospace; }
.details .links a { display:inline-block; margin:2px 6px 0 0; color:var(--accent); cursor:pointer; text-decoration:underline dotted; }
.details h4 { margin:8px 0 2px; font-size:11px; text-transform:uppercase; letter-spacing:.5px; color:var(--muted); }
.source { flex:1; overflow:auto; font-family: ui-monospace, Menlo, Consolas, monospace; font-size:12px; background:var(--panel2); }
.source .path, .source-pane .path { position:sticky; top:0; background:var(--panel); padding:4px 10px; color:var(--muted); border-bottom:1px solid var(--border); font-size:11px; z-index:1; }
.source .line, .source-pane .line { display:flex; white-space:pre; }
.source .line .ln, .source-pane .line .ln { width:44px; text-align:right; padding-right:8px; color:var(--muted); opacity:.6; user-select:none; flex:none; }
.source .line.hl, .source-pane .line.hl { background:var(--hl-bg); }
.source .line.hl .ln, .source-pane .line.hl .ln { color:var(--hi); opacity:1; }
.source .line.fn, .source-pane .line.fn { background:var(--sel-bg); }
.source-pane .line.fn.hl { background:var(--hl-bg); }
.guide { border-top:1px solid var(--border); background:var(--panel); padding:8px 12px; display:flex; gap:12px; align-items:flex-start; max-height:34vh; }
.guide .ctl { display:flex; flex-direction:column; gap:6px; min-width:150px; }
.guide .ctl .row { display:flex; gap:4px; }
.guide .body { flex:1; overflow:auto; max-height:30vh; }
.guide .body h3 { margin:0 0 4px; font-size:14px; color:var(--hi); }
.guide .body pre { background:var(--panel2); padding:6px 8px; border-radius:6px; margin:4px 0; font-size:12px; white-space:pre-wrap; }
.guide .stack { color:var(--muted); font-size:11px; margin-top:4px; }
.guide .stack span { color:#cbd5e1; }
.md p { margin:3px 0; }
.iblk.nar, .blk.nar { box-shadow: 0 0 0 2px var(--hi), 0 0 12px color-mix(in srgb, var(--hi) 60%, transparent); }
.source-pane .line.nar, .source .line.nar { background:var(--hl-bg); }
.source-pane .line, .source .line { transition: background .3s; }
.source .mark, .source-pane .mark { text-decoration: underline 2px var(--hi); text-underline-offset:3px; background:color-mix(in srgb, var(--hi) 35%, transparent); border-radius:2px; }
.guide .parts { display:flex; gap:4px; flex-wrap:wrap; align-items:center; margin-bottom:6px; }
.guide .qs { display:flex; gap:6px; flex-wrap:wrap; align-items:center; margin-top:8px; animation: qs-in .35s ease-out; }
.guide .qs .lbl { color:var(--muted); font-size:11px; }
.guide .qs button { border-radius:999px; padding:3px 10px; font-size:12px; border:1px solid var(--border); background:var(--panel2); color:var(--text); }
.guide .qs button:hover { border-color:var(--hi); color:var(--hi); }
.guide .qs button.own { border-color:color-mix(in srgb, var(--hi) 60%, var(--border)); }
@keyframes qs-in { from { opacity:0; transform:translateY(4px); } to { opacity:1; transform:none; } }
.guide .answer { margin-top:6px; border-left:3px solid var(--hi); background:var(--panel2); padding:8px 12px; border-radius:0 8px 8px 0; }
.guide .answer .q { font-weight:600; color:var(--hi); margin-bottom:4px; display:flex; gap:8px; align-items:center; }
.guide .answer .q .spacer { flex:1; }
.guide .answer .src { color:var(--muted); font-size:11px; font-weight:400; }
.spk { text-decoration: underline dotted color-mix(in srgb, var(--muted) 70%, transparent); text-underline-offset:3px; cursor:help; }
.spk-say { color:var(--muted); font-style:italic; }
.guide .loader { margin-top:8px; border-top:1px solid var(--border); padding-top:6px; }
.guide .loader textarea { width:100%; background:var(--panel2); color:var(--text); border:1px solid var(--border); border-radius:6px; font: 12px ui-monospace, Menlo, Consolas, monospace; padding:6px; }
.guide .loader .row { display:flex; gap:8px; align-items:center; margin-top:4px; }
.guide .loader input[type=file] { font-size:11px; color:var(--muted); }
.md ul { margin:3px 0 3px 18px; padding:0; }
.progress { height:3px; background:var(--panel3); border-radius:2px; overflow:hidden; }
.progress div { height:100%; background:var(--hi); }
.status { color:var(--muted); font-size:11px; }
.kbd { border:1px solid var(--border); border-radius:4px; padding:0 4px; font-size:10px; color:var(--muted); }
/* ---- icons, icon buttons with hover labels */
.logo { display:inline-block; flex:none; vertical-align:middle; -webkit-mask-size:contain; mask-size:contain; -webkit-mask-repeat:no-repeat; mask-repeat:no-repeat; -webkit-mask-position:center; mask-position:center; }
.topbar .brand { display:flex; align-items:center; gap:7px; }
.ico { display:inline-block; vertical-align:middle; filter:var(--icon-filter); pointer-events:none; }
.ibtn { display:inline-flex; align-items:center; gap:5px; position:relative; padding:4px 7px; }
.ibtn.active .ico { filter:var(--icon-active); }
.ibtn[data-tip]::after { content:attr(data-tip); position:absolute; top:calc(100% + 7px); left:50%; transform:translateX(-50%) translateY(-4px); background:var(--panel3); color:var(--text); border:1px solid var(--border); padding:3px 9px; border-radius:6px; font-size:11px; font-weight:400; white-space:nowrap; z-index:80; pointer-events:none; box-shadow:0 6px 16px rgba(0,0,0,.35); opacity:0; transition:opacity .15s .3s, transform .15s .3s; }
.ibtn[data-tip]:hover::after { opacity:1; transform:translateX(-50%) translateY(0); }
.ibtn.tip-left[data-tip]::after { left:auto; right:0; transform:none; }
.topbar .tabs .ibtn { padding:4px 6px; }
.topbar .tabs .ibtn.view .ico { width:18px; height:18px; }
.topbar .sep { width:1px; height:20px; background:var(--border); margin:0 2px; }
.hud .depth { display:flex; gap:3px; align-items:center; color:var(--muted); font-size:11px; }

/* ---- settings page */
.settings { flex:1; min-height:0; overflow:auto; padding:18px 28px 60px; background:var(--bg); }
.settings .wrap { max-width:920px; margin:0 auto; }
.settings h2 { display:flex; align-items:center; gap:10px; margin:0 0 14px; font-size:20px; }
.settings h2 .spacer { flex:1; }
.settings section { background:var(--panel); border:1px solid var(--border); border-radius:10px; padding:14px 18px; margin-bottom:14px; }
.settings h3 { margin:0 0 10px; font-size:12px; text-transform:uppercase; letter-spacing:.6px; color:var(--muted); display:flex; gap:8px; align-items:center; }
.settings .row { display:flex; gap:14px; align-items:center; flex-wrap:wrap; margin:6px 0; }
.settings label.opt { display:flex; gap:6px; align-items:center; cursor:pointer; font-size:12px; }
.settings .hint { color:var(--muted); font-size:12px; margin:2px 0 8px; }
.settings input[type=range] { width:170px; accent-color:var(--accent); }
.settings .val { font-family: ui-monospace, Menlo, Consolas, monospace; font-size:12px; min-width:40px; }
.themes { display:grid; grid-template-columns:repeat(auto-fill, minmax(150px, 1fr)); gap:10px; }
.tcard { border:2px solid var(--border); border-radius:10px; overflow:hidden; cursor:pointer; background:var(--bg); color:var(--text); transition:transform .12s, border-color .2s; }
.tcard:hover { transform:translateY(-2px); }
.tcard.sel { border-color:var(--accent); box-shadow:0 0 0 2px color-mix(in srgb, var(--accent) 40%, transparent); }
.tcard .prev { display:flex; gap:4px; padding:12px 10px; background:var(--bg); }
.tcard .prev i { display:block; height:16px; border-radius:4px; flex:1; }
.tcard .name { padding:6px 10px; font-size:12px; background:var(--panel); border-top:1px solid var(--border); display:flex; justify-content:space-between; align-items:center; }
.settings table.keys { border-collapse:collapse; font-size:12px; }
.settings table.keys td { padding:3px 14px 3px 0; }
.settings table.keys .kbd { font-size:11px; padding:1px 6px; }
"#;
