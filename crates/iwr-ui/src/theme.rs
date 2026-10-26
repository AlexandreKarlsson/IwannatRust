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
        NodeKind::Return => "#10b981",
        NodeKind::ReturnErr => "#ef4444",
        NodeKind::Halt => "#be123c",
        NodeKind::Let => "#38bdf8",
        NodeKind::Assign => "#f472b6",
        NodeKind::Io => "#a3e635",
        NodeKind::Log => "#64748b",
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
        NodeKind::ReturnErr => "return error",
        NodeKind::Halt => "program stops",
        NodeKind::Let => "new variable",
        NodeKind::Assign => "changes state",
        NodeKind::Io => "input / output",
        NodeKind::Log => "log",
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
/// Hub chip colours (architecture): distinct, readable on light and dark backgrounds.
pub const HUB_COLORS: [&str; 8] = ["#f472b6", "#fb923c", "#a3e635", "#22d3ee", "#c084fc", "#facc15", "#34d399", "#f87171"];

pub fn hub_color(hue: usize) -> &'static str {
    HUB_COLORS[hue % HUB_COLORS.len()]
}

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
        Mode::ControlFlow | Mode::BranchTree => vec![n(NodeKind::Entry), n(NodeKind::Block), n(NodeKind::Let), n(NodeKind::Assign), n(NodeKind::Io), n(NodeKind::If), n(NodeKind::Match), n(NodeKind::Loop), n(NodeKind::Call), n(NodeKind::Propagate), n(NodeKind::Panic), n(NodeKind::Return), n(NodeKind::ReturnErr), n(NodeKind::Halt), n(NodeKind::Exit), e(EdgeKind::True), e(EdgeKind::False), e(EdgeKind::Arm), e(EdgeKind::LoopBody), e(EdgeKind::LoopBack), e(EdgeKind::Error)],
        Mode::Architecture => vec![n(NodeKind::Module), n(NodeKind::Function), n(NodeKind::Struct), n(NodeKind::Trait), n(NodeKind::External), e(EdgeKind::Contains), e(EdgeKind::Dependency)],
        Mode::Structure => vec![n(NodeKind::Module), n(NodeKind::Function), n(NodeKind::Method), n(NodeKind::Impl), n(NodeKind::Struct), n(NodeKind::Enum), n(NodeKind::Trait), n(NodeKind::Const)],
        Mode::Types => vec![n(NodeKind::Struct), n(NodeKind::Enum), n(NodeKind::ErrorType), n(NodeKind::Trait), n(NodeKind::TypeAlias), n(NodeKind::Function), e(EdgeKind::Contains), e(EdgeKind::Implements), e(EdgeKind::Supertrait), e(EdgeKind::Uses)],
        Mode::ErrorFlow => vec![n(NodeKind::Function), n(NodeKind::Panic), n(NodeKind::ErrorType), e(EdgeKind::Propagate), e(EdgeKind::Unwrap), e(EdgeKind::Handles), e(EdgeKind::Error), e(EdgeKind::Aliases)],
        // a sketch explains itself, and the plan page is not a graph: no legend
        Mode::Diagram | Mode::Plan => vec![],
    }
}

pub use iwr_core::config::THEMES;

pub fn is_theme(id: &str) -> bool {
    iwr_core::config::is_theme(id)
}

/// Where each `[fonts]` role of `codecast.toml` applies.
const FONT_TARGETS: [(&str, &str, &str); 3] = [
    ("title", ".node text.title, .settings .shero h2, .settings .chead h3, .plan h2, .plan .name, .side-head .title, .cast-title, .tut-card h2", "system-ui, sans-serif"),
    ("text", "&, button, input, select, .bhead .label.note", "system-ui, sans-serif"),
    ("code", "code, pre, .source-pane, .ihead .sig, .bhead, .chip.var, .chip.def, .chip.call, .node text.sub, .node text.cap", "ui-monospace, Menlo, Consolas, monospace"),
];

/// CSS for the chosen fonts (box titles and text: the viewer's, which a `codecast.toml` sets by
/// default; code: the config's) and the config's colour overrides, on top of the theme.
/// Unsafe values are left out.
pub fn custom_css(title: &str, text: &str, c: &iwr_core::config::Config, base: &str) -> String {
    use iwr_core::config::{is_safe_color, COLORS};
    let mut css = String::new();
    let code = c.fonts.code.clone().unwrap_or_default();
    for (role, v) in [("title", title), ("text", text), ("code", code.as_str())] {
        if v.is_empty() {
            continue;
        }
        let Some((_, sel, fallback)) = FONT_TARGETS.iter().find(|(r, _, _)| *r == role) else { continue };
        let Some((family, face)) = crate::fonts::resolve(role, v, base) else { continue };
        css.push_str(&face);
        let sel = sel.split(", ").map(|s| if s == "&" { ".app.app".to_string() } else { format!(".app.app {s}") }).collect::<Vec<_>>().join(", ");
        css.push_str(&format!("{sel} {{ font-family:\"{family}\", {fallback}; }}\n"));
    }
    let vars: String = c.colors.iter().filter(|(k, v)| COLORS.contains(&k.as_str()) && is_safe_color(v)).map(|(k, v)| format!(" --{k}:{v};")).collect();
    if !vars.is_empty() {
        css.push_str(&format!(".app.app {{{vars} }}\n"));
    }
    css
}

pub const CSS: &str = r#"
:root, .theme-dark { --bg:#0b1020; --panel:#111827; --panel2:#0f172a; --panel3:#1f2937; --border:#1f2937; --text:#e5e7eb; --muted:#9ca3af; --accent:#60a5fa; --hi:#fbbf24; --hl-bg:#3b2f0b; --sel-bg:#0f1a2e; --code:#93c5fd; --grid:#1f2937; --fill-op:0.28; --icon-filter:invert(.88); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
.theme-light { --bg:#f8fafc; --panel:#ffffff; --panel2:#f1f5f9; --panel3:#e2e8f0; --border:#cbd5e1; --text:#0f172a; --muted:#64748b; --accent:#2563eb; --hi:#b45309; --hl-bg:#fef3c7; --sel-bg:#e0f2fe; --code:#1d4ed8; --grid:#e2e8f0; --fill-op:0.18; --icon-filter:invert(.2); --icon-active:invert(1); --hl-line:var(--hl-bg); }
.theme-paper { --bg:#f5efe0; --panel:#fbf7ec; --panel2:#f1eadb; --panel3:#e6dcc5; --border:#d6c9a8; --text:#2b2416; --muted:#7a6d52; --accent:#8a4b08; --hi:#9a3412; --hl-bg:#fde68a; --sel-bg:#e9dfc2; --code:#7c2d12; --grid:#e3d8bd; --fill-op:0.2; --icon-filter:invert(.25) sepia(.5); --icon-active:invert(1); --hl-line:var(--hl-bg); }
.theme-midnight { --bg:#0a0a14; --panel:#12121f; --panel2:#0e0e1a; --panel3:#1c1c30; --border:#26263d; --text:#ececf6; --muted:#8f8fb0; --accent:#a78bfa; --hi:#f0abfc; --hl-bg:#3b1d4a; --sel-bg:#1e1540; --code:#c4b5fd; --grid:#1a1a2e; --fill-op:0.3; --icon-filter:invert(.9); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
.theme-forest { --bg:#0c1a12; --panel:#112419; --panel2:#0e1f15; --panel3:#1a3324; --border:#234231; --text:#e3f2e8; --muted:#8fb39d; --accent:#4ade80; --hi:#fbbf24; --hl-bg:#3a3410; --sel-bg:#15301f; --code:#86efac; --grid:#16281d; --fill-op:0.28; --icon-filter:invert(.9); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
.theme-nord { --bg:#2e3440; --panel:#3b4252; --panel2:#353b4a; --panel3:#434c5e; --border:#4c566a; --text:#eceff4; --muted:#a3adc2; --accent:#88c0d0; --hi:#ebcb8b; --hl-bg:#4a4330; --sel-bg:#3f4a60; --code:#8fbcbb; --grid:#3b4252; --fill-op:0.3; --icon-filter:invert(.92); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
.theme-solar { --bg:#fdf6e3; --panel:#fffbf0; --panel2:#f7efd9; --panel3:#eee8d5; --border:#d9d2b8; --text:#073642; --muted:#657b83; --accent:#268bd2; --hi:#b58900; --hl-bg:#f5e6a8; --sel-bg:#e3eef5; --code:#2aa198; --grid:#ece5cf; --fill-op:0.2; --icon-filter:invert(.15) sepia(.4); --icon-active:invert(1); --hl-line:var(--hl-bg); }
.theme-dusk { --bg:#1a1016; --panel:#241720; --panel2:#1f1319; --panel3:#33202c; --border:#3f2a37; --text:#f5e8ef; --muted:#b48ca2; --accent:#fb7185; --hi:#fcd34d; --hl-bg:#4a3414; --sel-bg:#3a1f30; --code:#fda4af; --grid:#2a1a24; --fill-op:0.3; --icon-filter:invert(.9); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
.theme-dracula { --bg:#282a36; --panel:#2f3241; --panel2:#2b2d3a; --panel3:#44475a; --border:#4b4f66; --text:#f8f8f2; --muted:#a4a9c9; --accent:#bd93f9; --hi:#f1fa8c; --hl-bg:#4a4a2c; --sel-bg:#3b3560; --code:#ff79c6; --grid:#323544; --fill-op:0.3; --icon-filter:invert(.9); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
.theme-monokai { --bg:#272822; --panel:#2f302a; --panel2:#2b2c26; --panel3:#3e3d32; --border:#49483e; --text:#f8f8f2; --muted:#a59f85; --accent:#a6e22e; --hi:#fd971f; --hl-bg:#4a3a1c; --sel-bg:#3a3d2c; --code:#66d9ef; --grid:#33342c; --fill-op:0.3; --icon-filter:invert(.9); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
.theme-gruvbox { --bg:#282828; --panel:#32302f; --panel2:#2d2b2a; --panel3:#3c3836; --border:#504945; --text:#ebdbb2; --muted:#a89984; --accent:#fabd2f; --hi:#fe8019; --hl-bg:#4a3518; --sel-bg:#3a3a2a; --code:#8ec07c; --grid:#32302f; --fill-op:0.3; --icon-filter:invert(.9); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
.theme-gruvbox-light { --bg:#fbf1c7; --panel:#fdf6d8; --panel2:#f2e5bc; --panel3:#ebdbb2; --border:#d5c4a1; --text:#3c3836; --muted:#7c6f64; --accent:#af3a03; --hi:#b57614; --hl-bg:#f9e2a3; --sel-bg:#efe0b0; --code:#076678; --grid:#ebdbb2; --fill-op:0.2; --icon-filter:invert(.2); --icon-active:invert(1); --hl-line:var(--hl-bg); }
.theme-catppuccin { --bg:#1e1e2e; --panel:#252537; --panel2:#212131; --panel3:#313244; --border:#45475a; --text:#cdd6f4; --muted:#a6adc8; --accent:#cba6f7; --hi:#f9e2af; --hl-bg:#45402f; --sel-bg:#2f2d48; --code:#89dceb; --grid:#28283a; --fill-op:0.3; --icon-filter:invert(.9); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
.theme-latte { --bg:#eff1f5; --panel:#ffffff; --panel2:#e6e9ef; --panel3:#dce0e8; --border:#ccd0da; --text:#4c4f69; --muted:#6c6f85; --accent:#8839ef; --hi:#df8e1d; --hl-bg:#f6e3bb; --sel-bg:#e4dcf5; --code:#1e66f5; --grid:#e6e9ef; --fill-op:0.2; --icon-filter:invert(.2); --icon-active:invert(1); --hl-line:var(--hl-bg); }
.theme-tokyo { --bg:#1a1b26; --panel:#1f2335; --panel2:#1c1e2d; --panel3:#292e42; --border:#3b4261; --text:#c0caf5; --muted:#8790b5; --accent:#7aa2f7; --hi:#e0af68; --hl-bg:#3d3424; --sel-bg:#283457; --code:#7dcfff; --grid:#232433; --fill-op:0.3; --icon-filter:invert(.9); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
.theme-onedark { --bg:#282c34; --panel:#2f343e; --panel2:#2b2f38; --panel3:#3a3f4b; --border:#454b58; --text:#dcdfe4; --muted:#9aa1ad; --accent:#61afef; --hi:#e5c07b; --hl-bg:#4a4130; --sel-bg:#2f3a4e; --code:#c678dd; --grid:#30343d; --fill-op:0.3; --icon-filter:invert(.9); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
.theme-github { --bg:#f6f8fa; --panel:#ffffff; --panel2:#f0f3f6; --panel3:#e6eaef; --border:#d0d7de; --text:#1f2328; --muted:#59636e; --accent:#0969da; --hi:#9a6700; --hl-bg:#fff1c2; --sel-bg:#ddf4ff; --code:#8250df; --grid:#eaeef2; --fill-op:0.2; --icon-filter:invert(.2); --icon-active:invert(1); --hl-line:var(--hl-bg); }
.theme-github-dark { --bg:#0d1117; --panel:#161b22; --panel2:#11161d; --panel3:#21262d; --border:#30363d; --text:#e6edf3; --muted:#8d96a0; --accent:#58a6ff; --hi:#d29922; --hl-bg:#3b2e0f; --sel-bg:#132339; --code:#d2a8ff; --grid:#161b22; --fill-op:0.3; --icon-filter:invert(.9); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
.theme-rosepine { --bg:#191724; --panel:#1f1d2e; --panel2:#1c1a29; --panel3:#26233a; --border:#393552; --text:#e0def4; --muted:#908caa; --accent:#ebbcba; --hi:#f6c177; --hl-bg:#433826; --sel-bg:#2d2a45; --code:#c4a7e7; --grid:#21202e; --fill-op:0.3; --icon-filter:invert(.9); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
.theme-dawn { --bg:#faf4ed; --panel:#fffaf3; --panel2:#f2e9e1; --panel3:#ebe0d6; --border:#dfd3c8; --text:#575279; --muted:#797593; --accent:#d7827e; --hi:#ea9d34; --hl-bg:#f9e3c1; --sel-bg:#efe2e9; --code:#907aa9; --grid:#f2e9e1; --fill-op:0.2; --icon-filter:invert(.2); --icon-active:invert(1); --hl-line:var(--hl-bg); }
.theme-everforest { --bg:#2d353b; --panel:#343f44; --panel2:#313b40; --panel3:#3d484d; --border:#475258; --text:#d3c6aa; --muted:#9da9a0; --accent:#a7c080; --hi:#dbbc7f; --hl-bg:#4a4535; --sel-bg:#3a4a42; --code:#83c092; --grid:#343f44; --fill-op:0.3; --icon-filter:invert(.9); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
.theme-kanagawa { --bg:#1f1f28; --panel:#25252f; --panel2:#22222b; --panel3:#2a2a37; --border:#363646; --text:#dcd7ba; --muted:#a09d8a; --accent:#7e9cd8; --hi:#e6c384; --hl-bg:#45402c; --sel-bg:#2d3449; --code:#957fb8; --grid:#26262f; --fill-op:0.3; --icon-filter:invert(.9); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
.theme-ayu { --bg:#fafafa; --panel:#ffffff; --panel2:#f3f4f5; --panel3:#e7e8e9; --border:#d8d9da; --text:#5c6166; --muted:#8a9199; --accent:#ff9940; --hi:#e6ba7e; --hl-bg:#fdf0d5; --sel-bg:#e9f0f7; --code:#399ee6; --grid:#eeeff0; --fill-op:0.2; --icon-filter:invert(.2); --icon-active:invert(1); --hl-line:var(--hl-bg); }
.theme-synthwave { --bg:#1b1031; --panel:#24173e; --panel2:#201437; --panel3:#2f1f4f; --border:#432e6b; --text:#f8e9ff; --muted:#b59ad6; --accent:#ff6ec7; --hi:#fede5d; --hl-bg:#4a2c3c; --sel-bg:#3a1f5a; --code:#36f9f6; --grid:#24173e; --fill-op:0.3; --icon-filter:invert(.9); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
.theme-ocean { --bg:#0b1d2a; --panel:#0f2636; --panel2:#0d2230; --panel3:#163447; --border:#1f4258; --text:#dff2fb; --muted:#89aebf; --accent:#38bdf8; --hi:#facc15; --hl-bg:#3d3814; --sel-bg:#103650; --code:#5eead4; --grid:#112a3a; --fill-op:0.3; --icon-filter:invert(.9); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
.theme-sakura { --bg:#fff5f8; --panel:#ffffff; --panel2:#fdecf1; --panel3:#f9dde6; --border:#f1c6d3; --text:#4a2b36; --muted:#8f6474; --accent:#d6336c; --hi:#c2410c; --hl-bg:#ffe4d1; --sel-bg:#fde2ec; --code:#a61e4d; --grid:#fbe6ed; --fill-op:0.2; --icon-filter:invert(.2); --icon-active:invert(1); --hl-line:var(--hl-bg); }
.theme-mint { --bg:#f1fbf6; --panel:#ffffff; --panel2:#e6f6ee; --panel3:#d6efe2; --border:#bfe3d0; --text:#173a2c; --muted:#5b7f6f; --accent:#0f9d6c; --hi:#b45309; --hl-bg:#fdecc8; --sel-bg:#d9f3e7; --code:#047857; --grid:#e3f4eb; --fill-op:0.2; --icon-filter:invert(.2); --icon-active:invert(1); --hl-line:var(--hl-bg); }
.theme-desert { --bg:#f7ecdf; --panel:#fdf6ee; --panel2:#f1e3d2; --panel3:#e8d4bd; --border:#d8bf9f; --text:#3d2a1a; --muted:#86684b; --accent:#c2410c; --hi:#9a3412; --hl-bg:#fcd9b0; --sel-bg:#f0dcc5; --code:#9a5b13; --grid:#eedfcc; --fill-op:0.2; --icon-filter:invert(.2); --icon-active:invert(1); --hl-line:var(--hl-bg); }
.theme-coffee { --bg:#1c1410; --panel:#251b15; --panel2:#211813; --panel3:#33261d; --border:#463427; --text:#f1e3d3; --muted:#b69a80; --accent:#d9a066; --hi:#f2c14e; --hl-bg:#4a3818; --sel-bg:#3b2a1f; --code:#e6b98a; --grid:#261c16; --fill-op:0.3; --icon-filter:invert(.9); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
.theme-matrix { --bg:#050a06; --panel:#0a140c; --panel2:#08100a; --panel3:#102014; --border:#173020; --text:#c8f7d0; --muted:#6fae7c; --accent:#22c55e; --hi:#a3e635; --hl-bg:#24380f; --sel-bg:#0f2a17; --code:#4ade80; --grid:#0b170e; --fill-op:0.3; --icon-filter:invert(.9); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
.theme-cyber { --bg:#0a0a12; --panel:#12121e; --panel2:#0e0e18; --panel3:#1c1c2e; --border:#2c2c46; --text:#f0f0ff; --muted:#9a9ac0; --accent:#fcee0a; --hi:#00f0ff; --hl-bg:#3a3a10; --sel-bg:#262050; --code:#ff2a6d; --grid:#141422; --fill-op:0.3; --icon-filter:invert(.9); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
.theme-contrast { --bg:#000000; --panel:#0a0a0a; --panel2:#050505; --panel3:#1a1a1a; --border:#5a5a5a; --text:#ffffff; --muted:#d0d0d0; --accent:#ffd400; --hi:#00e5ff; --hl-bg:#3a3300; --sel-bg:#1a2a3a; --code:#7cc4ff; --grid:#151515; --fill-op:0.3; --icon-filter:invert(.9); --icon-active:none; --hl-line:color-mix(in srgb, var(--hi) 30%, var(--panel2)); }
/* syntax colours: one set for dark themes, one for light ones */
:root, .theme-dark, .theme-midnight, .theme-forest, .theme-nord, .theme-dusk { --t-kw:#c678dd; --t-bi:#d19a66; --t-ty:#e5c07b; --t-fn:#61afef; --t-mac:#56b6c2; --t-str:#98c379; --t-num:#d19a66; --t-lt:#e06c75; --t-com:#7f848e; --t-doc:#9bb38a; --t-attr:#a0a8b8; --t-var:#e06c75; }
.theme-light, .theme-paper, .theme-solar { --t-kw:#a626a4; --t-bi:#986801; --t-ty:#b07600; --t-fn:#4078f2; --t-mac:#0184bc; --t-str:#50a14f; --t-num:#986801; --t-lt:#e45649; --t-com:#8b8c92; --t-doc:#5f7f4f; --t-attr:#696c77; --t-var:#e45649; }
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
.app { display:flex; flex-direction:column; height:100vh; overflow:clip; }
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
@keyframes node-in { from { opacity:0; } to { opacity:1; } }
.cam.anim .node, .cam.anim .edge { animation: node-in .5s ease-out; }
.edge path { transition: stroke .35s, stroke-width .35s, opacity .35s; }
.sbtabs { display:flex; gap:4px; margin:2px 0 6px; }
.ftree .frow { display:flex; align-items:center; gap:5px; padding:2px 6px; border-radius:4px; cursor:pointer; white-space:nowrap; font-size:12px; }
.ftree .frow:hover { background:var(--panel3); }
.ftree .frow.sel { background:var(--sel-bg); box-shadow: inset 3px 0 0 var(--accent); color:var(--text); font-weight:600; }
.ftree .frow.nar { box-shadow: inset 3px 0 0 var(--hi); background:var(--hl-bg); }
.ftree .frow.sel.nar { box-shadow: inset 3px 0 0 var(--hi), 0 0 0 1px var(--hi); }
.ftree .frow { transition: background .3s, box-shadow .3s; }
.ftree .frow.dir { color:var(--muted); }
.ftree .frow.dir.nar { color:var(--hi); font-weight:600; }
/* `+ panel` in a codecast: that panel grows, the others shrink (over the sizes set by hand) */
.main .sidebar, .main .details, .main .side { transition: width .45s ease; }
.main .blocks-pane.half { transition: flex-basis .45s ease; }
.ftree .frow, .sidebar .item { transition: font-size .3s, padding .3s, background .3s, box-shadow .3s; }
.main.focus-files .sidebar, .main.focus-items .sidebar { width:max(40vw, 340px) !important; }
.main.focus-files .ftree .frow { font-size:16px; padding:4px 8px; }
.main.focus-files .ftree .fic { font-size:15px; width:36px; }
.main.focus-files .ftree .fcount { font-size:12px; }
.main.focus-items .sidebar .item { font-size:15px; padding:4px 8px; }
.main.focus-view .sidebar, .main.focus-source .sidebar { width:150px !important; }
.main.focus-view .details, .main.focus-view .side { width:260px !important; }
.main.focus-view .blocks-pane.half { flex-basis:72% !important; }
.main.focus-source .details { width:max(55vw, 420px) !important; }
.main.focus-source .details .source { font-size:1.12em; }
.main.focus-source .blocks-pane.half { flex-basis:30% !important; }
.main.focus-source .source-pane { font-size:1.08em; }
/* first start: the welcome dialog and the tutorial */
.tut-backdrop { position:fixed; inset:0; z-index:80; }
.tut-backdrop.dim { background:rgba(0,0,0,.55); display:flex; align-items:center; justify-content:center; }
.tut-card { background:var(--panel); color:var(--text); border:1px solid var(--border); border-radius:14px; box-shadow:0 14px 40px rgba(0,0,0,.45); }
.tut-card h2 { margin:8px 0; font-size:20px; }
.tut-card.welcome { position:static; width:min(440px, calc(100vw - 32px)); text-align:center; padding:24px 24px 18px; }
.tut-card.welcome p { color:var(--muted); margin:0 0 16px; }
/* `+ search` in a codecast: that part of the app in the light, the rest dimmed, the player readable */
/* `+ zoom`: the presenter's zoom of the middle of the screen */
.main { transform-origin:0 0; transition: transform .6s ease; }
.main.zoomed { position:relative; z-index:0; }
.spot-dim { position:fixed; z-index:60; background:rgba(0,0,0,.55); pointer-events:none; animation: spot-in .3s ease; }
@keyframes spot-in { from { opacity:0; } }
.app.spotting .guide { position:relative; z-index:61; }
.spot-on { outline:2px solid var(--accent) !important; outline-offset:3px; border-radius:6px; animation: spot-pulse 1.6s ease-in-out infinite; }
@keyframes spot-pulse { 50% { outline-offset:7px; } }
.tut-actions { display:flex; gap:8px; align-items:center; margin-top:12px; }
.tut-actions .spacer { flex:1; }
.tut-actions.col { flex-direction:column; align-items:stretch; }
.tut-actions button { cursor:pointer; padding:6px 12px; border-radius:8px; border:1px solid var(--border); background:var(--panel2); color:var(--text); }
.tut-actions button:hover { border-color:var(--accent); }
.tut-actions .tut-primary { background:var(--accent); border-color:var(--accent); color:var(--bg); font-weight:600; }
.tut-actions .tut-quiet { background:transparent; border-color:transparent; color:var(--muted); }
.ftree .frow.other { color:var(--muted); }
.ftree .fic { font-size:11px; width:28px; flex:none; }
.ftree .fcount { margin-left:auto; color:var(--muted); font-size:10px; }
.node .detail text { font-size:13px; }
.sidebar .mod { margin:6px 0 2px; color:var(--muted); font-size:11px; text-transform:uppercase; letter-spacing:.5px; cursor:pointer; border-radius:4px; padding:1px 4px; }
.sidebar .mod:hover { background:var(--panel3); color:var(--text); }
.sidebar .mod.scoped { background:var(--hl-bg); color:var(--hi); }
.edge.dim { opacity:.18; }
.edge.hub.dim { opacity:0; }
.node .chip-dot { pointer-events:all; cursor:pointer; }
.node .chip-dot text { fill:#0b1020; font-weight:800; pointer-events:none; font-family: system-ui, sans-serif; }
.node .chip-dot:hover circle { stroke:var(--text); }
.edge.dim .edge-label, .edge.dim .edge-label-bg { display:none; }
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
.iblk.hl { box-shadow: 0 0 0 2px var(--hi), 0 0 14px color-mix(in srgb, var(--hi) 45%, transparent); background:color-mix(in srgb, var(--hi) 12%, var(--panel)); }
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
.blk.hl { box-shadow: 0 0 0 2px var(--hi), 0 0 12px color-mix(in srgb, var(--hi) 45%, transparent); background:color-mix(in srgb, var(--hi) 16%, var(--panel)); }
.bhead { display:flex; align-items:center; gap:6px; padding:5px 8px; cursor:pointer; flex-wrap:wrap; font-family: ui-monospace, Menlo, Consolas, monospace; font-size:12px; }
.bhead:hover { background:color-mix(in srgb, var(--c) 20%, var(--panel)); border-radius:0 6px 6px 0; }
.bhead .label { white-space:pre-wrap; }
.bhead .label.note { font-family: system-ui, sans-serif; font-weight:600; font-size:13.5px; color:var(--text); }
/* commented blocks and items: the comment is what you read; the rest is small, details on hover */
.blk.has-note > .bhead .tag, .iblk.has-note > .ihead .tag { font-size:8.5px; padding:0 4px; opacity:.8; }
.blk.has-note > .bhead .chip, .iblk.has-note > .ihead .chip, .iblk.has-note > .ihead .sig { display:none; }
.blk.has-note > .bhead:hover .chip, .iblk.has-note > .ihead:hover .chip { display:inline-block; }
.iblk.has-note > .ihead:hover .sig { display:inline; }
.ihead .note { font-weight:700; font-size:14.5px; }
.ihead .name.small { font-size:11px; font-weight:500; color:var(--muted); font-family: ui-monospace, Menlo, Consolas, monospace; }
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
.node text.sub { fill:var(--muted); font-size:13px; font-family: ui-monospace, Menlo, Consolas, monospace; }
.node text.cont { font-weight:700; }
.node text.cap { fill:var(--muted); font-size:13px; font-family: ui-monospace, Menlo, Consolas, monospace; }
.node.hi text.cap, .node.sel text.cap, .node:hover text.cap { fill:var(--text); }
.node text.fn-title { font-weight:700; }
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
.canvas-note { position:absolute; left:50%; top:40%; transform:translate(-50%,-50%); color:var(--muted); background:var(--panel); padding:10px 16px; border-radius:8px; border:1px solid var(--border); }
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
.source .line.hl, .source-pane .line.hl { background:var(--hl-line); box-shadow: inset 3px 0 0 var(--hi); }
.source .line.hl .ln, .source-pane .line.hl .ln { color:var(--hi); opacity:1; }
.source .line.fn, .source-pane .line.fn { background:var(--sel-bg); }
.source-pane .line.fn.hl { background:var(--hl-line); }
.guide { border-top:1px solid var(--border); background:var(--panel); padding:8px 12px; display:flex; gap:12px; align-items:flex-start; max-height:34vh; position:relative; }
.guide .pop { position:absolute; bottom:calc(100% + 10px); right:16px; z-index:20; max-width:min(560px, calc(100vw - 32px)); background:var(--panel2); border:1px solid var(--border); border-left:3px solid var(--hi); border-radius:8px; padding:8px 30px 8px 12px; box-shadow:0 6px 24px rgba(0,0,0,.35); animation: qs-in .3s ease-out; }
.guide .pop .pop-row { margin:2px 0; overflow-wrap:anywhere; }
.guide .pop .pop-row > code { font: 13px ui-monospace, Menlo, Consolas, monospace; color:var(--code); }
.guide .pop .pop-row .md p { margin:0; }
.guide .pop .x { position:absolute; top:4px; right:4px; border:none; background:none; color:var(--muted); cursor:pointer; font-size:12px; padding:2px 6px; }
.guide .pop .x:hover { color:var(--hi); }
.guide .ctl { display:flex; flex-direction:column; gap:6px; min-width:150px; }
.guide .ctl .row { display:flex; gap:4px; }
.guide .body { flex:1; overflow:auto; max-height:30vh; }
.guide .body h3 { margin:0 0 4px; font-size:14px; color:var(--hi); }
.guide .body pre { background:var(--panel2); padding:6px 8px; border-radius:6px; margin:4px 0; font-size:12px; white-space:pre-wrap; }
.guide .stack { color:var(--muted); font-size:11px; margin-top:4px; }
.guide .stack span { color:#cbd5e1; }
.md p { margin:3px 0; }
.iblk.nar, .blk.nar { box-shadow: 0 0 0 2px var(--hi), 0 0 12px color-mix(in srgb, var(--hi) 60%, transparent); background:color-mix(in srgb, var(--hi) 14%, var(--panel)); }
.source-pane .line.nar, .source .line.nar { background:var(--hl-line); box-shadow: inset 3px 0 0 var(--hi); }
.source-pane .line, .source .line { transition: background .3s; }
.source .mark, .source-pane .mark { text-decoration: underline 2px var(--hi); text-underline-offset:3px; background:color-mix(in srgb, var(--hi) 35%, transparent); border-radius:2px; }
.source .gut, .source-pane .gut { width:3px; flex:none; margin-right:8px; background:var(--ic); box-shadow: 3px 0 0 color-mix(in srgb, var(--bc) 70%, transparent); }
.source .code, .source-pane .code { padding-left:3px; flex:1; background:color-mix(in srgb, var(--ic) 7%, transparent); }
.t-kw { color:var(--t-kw); } .t-bi { color:var(--t-bi); } .t-ty { color:var(--t-ty); } .t-fn { color:var(--t-fn); } .t-mac { color:var(--t-mac); }
.t-str { color:var(--t-str); } .t-num { color:var(--t-num); } .t-lt { color:var(--t-lt); } .t-attr { color:var(--t-attr); } .t-var { color:var(--t-var); }
.t-com { color:var(--t-com); font-style:italic; } .t-doc { color:var(--t-doc); font-style:italic; }
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

/* ---- plan page: the codecast's itinerary (`@ plan`) */
.settings { flex:1; min-height:0; overflow:auto; background:radial-gradient(1200px 500px at 70% -10%, color-mix(in srgb, var(--accent) 14%, transparent), transparent 70%), var(--bg); scroll-behavior:smooth; }
.settings .shell { display:flex; max-width:1180px; margin:0 auto; gap:28px; padding:22px 28px 80px; align-items:flex-start; }
.settings .snav { position:sticky; top:16px; width:200px; flex:none; display:flex; flex-direction:column; gap:2px; background:color-mix(in srgb, var(--panel) 85%, transparent); border:1px solid var(--border); border-radius:14px; padding:12px 10px; backdrop-filter:blur(8px); min-height:420px; }
.settings .sbrand { display:flex; align-items:center; gap:9px; padding:4px 8px 12px; font-weight:800; font-size:15px; border-bottom:1px solid var(--border); margin-bottom:8px; }
.settings .snav-item, .settings .snav-back { display:flex; align-items:center; gap:10px; background:none; border:none; border-radius:9px; padding:8px 10px; font-size:13px; color:var(--muted); text-align:left; width:100%; }
.settings .snav-item:hover, .settings .snav-back:hover { background:var(--panel3); color:var(--text); }
.settings .snav-item.sel { background:color-mix(in srgb, var(--accent) 18%, transparent); color:var(--text); font-weight:600; box-shadow: inset 3px 0 0 var(--accent); }
.settings .snav .spacer { flex:1; min-height:20px; }
.settings .snav-back { border:1px solid var(--border); justify-content:center; color:var(--text); }
.settings .scontent { flex:1; min-width:0; }
.settings .shero { padding:6px 4px 18px; }
.settings .shero h2 { margin:0; font-size:30px; font-weight:800; letter-spacing:-.4px; background:linear-gradient(90deg, var(--text), var(--accent)); -webkit-background-clip:text; background-clip:text; color:transparent; }
.settings .shero p { margin:6px 0 0; color:var(--muted); font-size:14px; }
.settings .lockline { margin-top:10px; display:inline-block; background:var(--hl-bg); color:var(--hi); border-radius:999px; padding:4px 12px; font-size:12px; font-weight:600; }
.settings .card { background:var(--panel); border:1px solid var(--border); border-radius:16px; padding:20px 22px; margin-bottom:18px; box-shadow:0 1px 0 color-mix(in srgb, var(--text) 4%, transparent), 0 10px 30px -18px rgba(0,0,0,.45); scroll-margin-top:16px; }
.settings .chead { display:flex; gap:14px; align-items:flex-start; margin-bottom:6px; }
.settings .cicon { width:38px; height:38px; flex:none; border-radius:11px; display:flex; align-items:center; justify-content:center; background:color-mix(in srgb, var(--accent) 20%, var(--panel3)); }
.settings .chead h3 { margin:2px 0 2px; font-size:17px; font-weight:700; display:flex; align-items:center; gap:8px; }
.settings .csub { color:var(--muted); font-size:12.5px; }
.settings .badge { font-size:10px; text-transform:uppercase; letter-spacing:.6px; background:var(--accent); color:var(--panel); border-radius:999px; padding:1px 8px; font-weight:700; }
.settings h4 { margin:20px 0 10px; font-size:11px; text-transform:uppercase; letter-spacing:.8px; color:var(--muted); font-weight:700; display:flex; gap:8px; align-items:baseline; }
.settings .srow { display:flex; align-items:center; gap:18px; padding:11px 0; border-top:1px solid color-mix(in srgb, var(--border) 70%, transparent); }
.settings .chead + .srow { border-top:none; }
.settings .srow.block { flex-direction:column; align-items:stretch; gap:8px; }
.settings .sl { flex:1; min-width:0; }
.settings .st { font-size:13.5px; font-weight:600; }
.settings .sd { color:var(--muted); font-size:12px; margin-top:2px; }
.settings .sd.warn { color:#e5484d; font-weight:600; }
.settings .sc { display:flex; align-items:center; gap:10px; flex:none; }
.settings .val { font-family: ui-monospace, Menlo, Consolas, monospace; font-size:12px; min-width:40px; }
.settings input[type=range] { width:180px; accent-color:var(--accent); }
.settings input[type=text], .settings input[type=number], .settings select { min-height:30px; border-radius:8px; }
.settings input[type=text] { min-width:240px; }
.settings input[type=number] { width:80px; background:var(--panel2); color:var(--text); border:1px solid var(--border); padding:3px 8px; }
.switch { position:relative; display:inline-block; width:40px; height:22px; flex:none; cursor:pointer; }
.switch input { opacity:0; width:0; height:0; position:absolute; }
.switch .knob { position:absolute; inset:0; background:var(--panel3); border:1px solid var(--border); border-radius:999px; transition:background .2s, border-color .2s; }
.switch .knob::after { content:""; position:absolute; left:2px; top:2px; width:16px; height:16px; border-radius:50%; background:var(--muted); transition:transform .2s, background .2s; }
.switch input:checked + .knob { background:var(--accent); border-color:var(--accent); }
.switch input:checked + .knob::after { transform:translateX(18px); background:var(--panel); }
.switch input:focus-visible + .knob { outline:2px solid var(--accent); outline-offset:2px; }
.switch.off { opacity:.5; cursor:default; }
.settings .stepper { display:inline-flex; align-items:center; border:1px solid var(--border); border-radius:8px; overflow:hidden; }
.settings .stepper button { border:none; border-radius:0; padding:4px 11px; font-size:14px; }
.settings .stepper span { min-width:34px; text-align:center; font-weight:700; font-family: ui-monospace, Menlo, Consolas, monospace; }
.settings .cta { display:flex; align-items:center; gap:12px; margin-top:16px; flex-wrap:wrap; }
.settings button.big { display:inline-flex; align-items:center; gap:8px; background:var(--accent); color:var(--panel); border:none; border-radius:10px; padding:9px 18px; font-size:13px; font-weight:700; box-shadow:0 6px 18px -8px var(--accent); }
.settings button.big:hover { filter:brightness(1.08); }
.settings button.big:disabled { opacity:.5; cursor:default; }
.settings .chips { padding:0; display:flex; gap:6px; align-items:center; margin-bottom:10px; flex-wrap:wrap; }
.settings .chip-btn { border-radius:999px; padding:3px 12px; font-size:12px; background:var(--panel2); }
.settings .chip-btn.sel { background:var(--text); color:var(--panel); border-color:var(--text); font-weight:600; }
.settings .chips .count { margin-left:auto; color:var(--muted); font-size:11px; }
.settings .seg { display:inline-flex; border:1px solid var(--border); border-radius:10px; padding:3px; background:var(--panel2); margin-bottom:12px; gap:2px; }
.settings .seg button { border:none; background:none; border-radius:8px; padding:5px 14px; font-size:12.5px; color:var(--muted); }
.settings .seg button.sel { background:var(--panel); color:var(--text); font-weight:600; box-shadow:0 1px 4px rgba(0,0,0,.25); }
.themes { display:grid; grid-template-columns:repeat(auto-fill, minmax(148px, 1fr)); gap:12px; }
.tcard { border:2px solid var(--border); border-radius:12px; overflow:hidden; cursor:pointer; background:var(--bg); color:var(--text); transition:transform .15s, border-color .2s, box-shadow .2s; }
.tcard:hover { transform:translateY(-3px); box-shadow:0 10px 24px -14px rgba(0,0,0,.6); }
.tcard.sel { border-color:var(--accent); box-shadow:0 0 0 3px color-mix(in srgb, var(--accent) 35%, transparent); }
.tcard .mock { height:84px; background:var(--bg); display:flex; flex-direction:column; }
.tcard .mbar { height:14px; background:var(--panel); border-bottom:1px solid var(--border); display:flex; gap:4px; align-items:center; padding:0 6px; }
.tcard .mbar i { display:block; height:5px; border-radius:3px; background:var(--panel3); width:22px; }
.tcard .mbar i.dot { width:6px; height:6px; border-radius:50%; background:var(--accent); }
.tcard .mbar i.short { width:12px; }
.tcard .mgraph { flex:1; position:relative; background-image:radial-gradient(var(--grid) 1px, transparent 1px); background-size:8px 8px; }
.tcard .mbox { position:absolute; height:16px; border-radius:4px; }
.tcard .mbox.a { left:12px; top:8px; width:52px; background:color-mix(in srgb, var(--accent) 30%, var(--bg)); border:1.5px solid var(--accent); }
.tcard .mbox.b { right:12px; top:28px; width:44px; background:color-mix(in srgb, var(--hi) 30%, var(--bg)); border:1.5px solid var(--hi); box-shadow:0 0 8px color-mix(in srgb, var(--hi) 60%, transparent); }
.tcard .mline { position:absolute; left:62px; top:22px; width:40px; height:12px; border-top:1.5px solid var(--muted); border-right:1.5px solid var(--muted); border-top-right-radius:8px; }
.tcard .mcode { height:20px; background:var(--panel2); border-top:1px solid var(--border); display:flex; gap:4px; align-items:center; padding:0 6px; }
.tcard .mcode i { display:block; height:4px; border-radius:2px; }
.tcard .mcode .c1 { width:18px; background:var(--code); } .tcard .mcode .c2 { width:34px; background:var(--text); opacity:.7; } .tcard .mcode .c3 { width:20px; background:var(--muted); }
.tcard .name { padding:7px 10px; font-size:12px; font-weight:600; background:var(--panel); border-top:1px solid var(--border); display:flex; justify-content:space-between; align-items:center; }
.tcard.none .mock { align-items:center; justify-content:center; background:repeating-linear-gradient(45deg, var(--panel2) 0 8px, var(--panel) 8px 16px); }
.tcard.none .none-text { color:var(--muted); font-size:12px; }
.themes.locked .tcard, .fonts.locked .fcard { cursor:default; opacity:.45; transform:none; }
.themes.locked .tcard.sel, .fonts.locked .fcard.sel { opacity:1; }
.fonts { display:grid; grid-template-columns:repeat(auto-fill, minmax(160px, 1fr)); gap:10px; max-height:430px; overflow:auto; padding:2px; }
.fcard { border:1.5px solid var(--border); border-radius:12px; padding:10px 12px 11px; cursor:pointer; background:var(--panel2); transition:transform .15s, border-color .2s, background .2s; min-width:0; }
.fcard:hover { transform:translateY(-2px); border-color:color-mix(in srgb, var(--accent) 60%, var(--border)); }
.fcard.sel { border-color:var(--accent); background:color-mix(in srgb, var(--accent) 12%, var(--panel2)); box-shadow:0 0 0 2px color-mix(in srgb, var(--accent) 30%, transparent); }
.fcard .aa { font-size:30px; line-height:1.1; color:var(--accent); }
.fcard .fname { font-size:15px; margin-top:4px; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
.fcard .fsample { font-size:12px; color:var(--muted); margin-top:3px; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
.design .upload { display:inline-flex; gap:6px; align-items:center; cursor:pointer; font-size:12px; border:1px dashed var(--border); border-radius:9px; padding:6px 12px; margin-top:10px; color:var(--muted); }
.design .upload:hover { border-color:var(--accent); color:var(--text); }
.design .colors { display:grid; grid-template-columns:repeat(auto-fill, minmax(140px, 1fr)); gap:8px; }
.design .color { display:flex; gap:8px; align-items:center; font-size:12px; color:var(--muted); font-family: ui-monospace, Menlo, Consolas, monospace; background:var(--panel2); border:1px solid var(--border); border-radius:9px; padding:5px 8px; }
.design .color.set { color:var(--text); font-weight:600; border-color:var(--accent); }
.design .color input[type=color] { width:26px; height:22px; padding:0; border:1px solid var(--border); border-radius:6px; background:none; cursor:pointer; }
.design .color:not(.set) input[type=color] { opacity:.4; }
.design .color button { margin-left:auto; }
.settings .keys { display:grid; grid-template-columns:repeat(auto-fill, minmax(230px, 1fr)); gap:8px; margin-top:10px; }
.settings .key { display:flex; gap:10px; align-items:center; background:var(--panel2); border:1px solid var(--border); border-radius:9px; padding:7px 10px; font-size:12.5px; }
.settings .key > span:first-child { display:flex; gap:3px; min-width:64px; }
.settings .key .kbd { font-size:11px; padding:1px 7px; background:var(--panel); color:var(--text); border-bottom-width:2px; }
@media (max-width: 820px) { .settings .shell { flex-direction:column; padding:14px 16px 60px; } .settings .snav { position:static; width:auto; min-height:0; flex-direction:row; flex-wrap:wrap; } .settings .snav .spacer, .settings .sbrand { display:none; } .settings .srow { flex-wrap:wrap; } }
.info-tip { position:relative; display:inline-flex; align-items:center; justify-content:center; width:24px; height:24px; border-radius:50%; border:1px solid var(--border); background:var(--panel2); cursor:help; outline:none; }
.info-tip:hover, .info-tip:focus-visible { border-color:var(--accent); }
.info-tip .tip { display:none; position:absolute; left:32px; top:50%; transform:translateY(-50%); width:320px; z-index:20; background:var(--panel); color:var(--text); border:1px solid var(--border); border-radius:10px; padding:10px 12px; font-size:12px; line-height:1.5; box-shadow:0 12px 30px -10px rgba(0,0,0,.5); cursor:default; }
.info-tip .tip b { font-family: ui-monospace, Menlo, Consolas, monospace; color:var(--accent); }
.info-tip:hover .tip, .info-tip:focus .tip, .info-tip:focus-within .tip { display:block; }
@media (max-width: 820px) { .info-tip .tip { left:auto; right:0; top:30px; transform:none; width:260px; } }
.plan { flex:1; min-height:0; overflow:auto; padding:22px 32px 60px; background:var(--bg); }
.plan .wrap { max-width:820px; margin:0 auto; }
.plan h2 { margin:0 0 4px; font-size:22px; display:flex; align-items:center; gap:10px; }
.plan .intro { color:var(--muted); margin:0 0 16px; font-size:13px; }
.plan ol { list-style:none; margin:0; padding:0; }
.plan li { display:flex; gap:14px; align-items:flex-start; padding:10px 14px; border:1px solid var(--border); border-radius:10px; margin-bottom:8px; background:var(--panel); cursor:pointer; transition: border-color .2s, transform .12s, opacity .2s; }
.plan li:hover { border-color:var(--accent); transform:translateX(2px); }
.plan li.cur { border-color:var(--hi); box-shadow: 0 0 0 1px var(--hi), 0 0 14px color-mix(in srgb, var(--hi) 35%, transparent); }
.plan li.done { opacity:.7; }
.plan .num { flex:none; width:28px; height:28px; border-radius:999px; background:var(--panel3); color:var(--text); display:flex; align-items:center; justify-content:center; font-weight:700; font-size:12px; }
.plan li.cur .num { background:var(--hi); color:var(--panel); }
.plan .name { font-weight:700; font-size:14px; }
.plan .sum { color:var(--muted); font-size:12px; margin-top:2px; }
.plan .meta { margin-left:auto; flex:none; color:var(--muted); font-size:11px; white-space:nowrap; padding-top:2px; }

/* ---- settings page */
/* side panel: `|` lines of a codecast (byte layouts, notes, small diagrams) */
.side { width:380px; flex:none; border-left:1px solid var(--border); background:var(--panel); display:flex; flex-direction:column; min-height:0; min-width:0; }
.side-head { display:flex; align-items:center; gap:8px; padding:8px 10px; border-bottom:1px solid var(--border); }
.side-head .kind { font-size:10px; text-transform:uppercase; letter-spacing:.5px; color:var(--muted); border:1px solid var(--border); border-radius:999px; padding:0 7px; }
.side-head .title { font-weight:700; font-size:14px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.side-head .spacer { flex:1; }
.side-body { flex:1; min-height:0; overflow:auto; padding:10px 12px 24px; }
.side-note { font-size:13px; }
.example { margin:0; font-family: ui-monospace, Menlo, Consolas, monospace; font-size:12px; line-height:1.5; white-space:pre; overflow-x:auto; }
.example div, .example-table td, .example-table th { transition: opacity .25s, background .25s; }
.example .on, .example-table .on { background:color-mix(in srgb, var(--hi) 22%, transparent); }
.example .dim, .example-table .dim { opacity:.45; }
.example-table { border-collapse:collapse; font-size:12px; font-family: ui-monospace, Menlo, Consolas, monospace; }
.example-table td, .example-table th { border:1px solid var(--border); padding:2px 6px; text-align:left; white-space:nowrap; }
.example-table th { background:var(--panel3); }
.md table.mdt { border-collapse:collapse; margin:6px 0; font-size:12px; } .md table.mdt td, .md table.mdt th { border:1px solid var(--border); padding:2px 6px; text-align:left; vertical-align:top; } .md table.mdt th { background:var(--panel3); }
.md h4.mdh { margin:10px 0 4px; font-size:13px; } .md ul.mdl { list-style:none; margin-left:2px; } .md ul.mdl li { display:flex; gap:6px; } .md .lnum { flex:none; min-width:12px; color:var(--muted); font-variant-numeric:tabular-nums; }
.bytes .strip { display:flex; flex-direction:column; gap:3px; margin-bottom:12px; }
.bytes .brow { display:grid; grid-template-columns: 38px repeat(var(--row), minmax(0, 1fr)); gap:2px; align-items:end; }
.bytes .off { font-family: ui-monospace, Menlo, Consolas, monospace; font-size:10px; color:var(--muted); padding-bottom:5px; }
.bytes .cell { position:relative; padding-top:13px; transition: opacity .25s, transform .2s; cursor:default; }
.bytes .cell .b { display:block; text-align:center; font-family: ui-monospace, Menlo, Consolas, monospace; font-size:12px; line-height:22px; border-radius:4px; background:color-mix(in srgb, var(--c) 30%, var(--panel)); border:1px solid color-mix(in srgb, var(--c) 70%, transparent); color:var(--text); min-height:24px; }
.bytes .cell .n { position:absolute; top:0; left:1px; font-size:10px; line-height:12px; color:var(--c); font-weight:700; white-space:nowrap; z-index:1; pointer-events:none; }
.bytes .cell.var .b { border-style:dashed; color:var(--muted); }
.bytes .cell.extra .b { background:transparent; border-style:dotted; color:var(--muted); }
.bytes .cell.on .b { box-shadow: 0 0 0 2px var(--hi), 0 0 10px color-mix(in srgb, var(--hi) 45%, transparent); font-weight:700; }
.bytes .cell.on { transform: translateY(-1px); }
.bytes .dim { opacity:.35; }
.bytes .fields { border-collapse:collapse; width:100%; font-size:12px; }
.bytes .fields th { text-align:left; font-size:10px; text-transform:uppercase; letter-spacing:.4px; color:var(--muted); font-weight:600; padding:2px 6px 4px 0; }
.bytes .fields td { padding:3px 6px 3px 0; vertical-align:top; border-top:1px solid var(--border); transition: opacity .25s; }
.bytes .fields tr.on td { background:var(--hl-bg); }
.bytes .fields .sw { display:inline-block; width:10px; height:10px; border-radius:3px; margin-top:3px; }
.bytes .fields .id { font-family: ui-monospace, Menlo, Consolas, monospace; color:var(--hi); white-space:nowrap; }
.bytes .fields .num { font-family: ui-monospace, Menlo, Consolas, monospace; color:var(--muted); white-space:nowrap; }
.bytes .fields .val { font-family: ui-monospace, Menlo, Consolas, monospace; color:var(--code); overflow-wrap:anywhere; }
.bytes .fields .desc p { margin:0; }
.bytes .errs { margin-top:10px; color:#e5484d; font-size:12px; }
.side svg.mini { width:100%; height:auto; max-height:70vh; }
@media (max-width: 900px) {
  .side { position:fixed; right:8px; bottom:8px; width:min(92vw, 420px) !important; max-height:55vh; z-index:30; border:1px solid var(--border); border-radius:10px; box-shadow:0 8px 30px rgba(0,0,0,.35); }
}
/* ---- generate page: record the codecast with Kokoro */
.gen-btn { position:relative; }
.gen-btn .gen-dot { min-width:15px; height:15px; padding:0 4px; border-radius:999px; background:var(--hi); color:var(--panel); font-size:9px; font-weight:800; line-height:15px; text-align:center; }
.gen-btn .gen-ring { width:13px; height:13px; border-radius:50%; background:conic-gradient(var(--hi) calc(var(--p) * 1%), color-mix(in srgb, var(--hi) 30%, transparent) 0); -webkit-mask:radial-gradient(circle, transparent 3px, #000 3.5px); mask:radial-gradient(circle, transparent 3px, #000 3.5px); animation:gen-pulse 1.6s ease-in-out infinite; }
@keyframes gen-pulse { 50% { opacity:.55; } }
@keyframes gen-spin { to { transform:rotate(360deg); } }
@keyframes gen-stripes { from { background-position:0 0; } to { background-position:20px 0; } }
@keyframes gen-slide { from { left:-40%; } to { left:100%; } }
@keyframes gen-rise { from { opacity:0; transform:translateY(10px); } to { opacity:1; transform:none; } }
.gen { flex:1; min-height:0; overflow:auto; background:
  radial-gradient(1200px 500px at 10% -10%, color-mix(in srgb, var(--accent) 14%, transparent), transparent 60%),
  radial-gradient(900px 420px at 110% 0%, color-mix(in srgb, var(--hi) 10%, transparent), transparent 60%), var(--bg); display:flex; flex-direction:column; }
.gen-wrap { width:100%; max-width:980px; margin:0 auto; padding:28px 28px 24px; box-sizing:border-box; flex:1; }
.gen-loading { display:flex; gap:10px; align-items:center; color:var(--muted); padding:60px 0; justify-content:center; }
.gen-spin { width:18px; height:18px; border-radius:50%; border:2.5px solid color-mix(in srgb, var(--accent) 25%, transparent); border-top-color:var(--accent); animation:gen-spin .8s linear infinite; flex:none; display:inline-block; box-sizing:border-box; }
.gen-spin.small { width:13px; height:13px; border-width:2px; }
.gen-hero { display:flex; gap:24px; align-items:flex-end; justify-content:space-between; flex-wrap:wrap; margin-bottom:18px; animation:gen-rise .35s ease both; }
.gen-hero-text { flex:1 1 420px; }
.gen-kicker { display:inline-flex; gap:6px; align-items:center; font-size:11px; font-weight:700; letter-spacing:.6px; text-transform:uppercase; color:var(--accent); background:color-mix(in srgb, var(--accent) 12%, transparent); border:1px solid color-mix(in srgb, var(--accent) 30%, transparent); padding:3px 10px; border-radius:999px; }
.gen-hero h2 { font-size:30px; line-height:1.15; margin:12px 0 8px; letter-spacing:-.4px; background:linear-gradient(90deg, var(--text), color-mix(in srgb, var(--accent) 80%, var(--text))); -webkit-background-clip:text; background-clip:text; color:transparent; }
.gen-hero p { margin:0; color:var(--muted); font-size:13.5px; line-height:1.55; max-width:600px; }
.gen-stats { display:flex; gap:10px; }
.gen-stat { min-width:92px; padding:12px 14px; border-radius:14px; background:color-mix(in srgb, var(--panel) 85%, transparent); border:1px solid var(--border); backdrop-filter:blur(8px); display:flex; flex-direction:column; gap:2px; }
.gen-stat b { font-size:20px; font-variant-numeric:tabular-nums; }
.gen-stat b span { font-size:13px; color:var(--muted); font-weight:600; }
.gen-stat > span { font-size:11px; color:var(--muted); }
.gen-coverage { display:flex; height:8px; border-radius:999px; overflow:hidden; background:var(--panel3); gap:2px; }
.gen-coverage i { display:block; min-width:0; transition:flex .5s ease; }
.gen-coverage i.ok, .gen-legend i.ok { background:#22c55e; }
.gen-coverage i.stale, .gen-legend i.stale { background:#f59e0b; }
.gen-coverage i.own, .gen-legend i.own { background:#a78bfa; }
.gen-coverage i.missing, .gen-legend i.missing { background:var(--panel3); }
.gen-legend { display:flex; gap:16px; flex-wrap:wrap; margin:8px 0 18px; font-size:11.5px; color:var(--muted); }
.gen-legend span { display:inline-flex; gap:6px; align-items:center; }
.gen-legend i { width:8px; height:8px; border-radius:3px; display:inline-block; border:1px solid var(--border); }
.gen-banner { display:flex; gap:12px; align-items:flex-start; padding:12px 16px; border-radius:12px; margin-bottom:16px; font-size:12.5px; line-height:1.5; background:color-mix(in srgb, var(--accent) 10%, var(--panel)); border:1px solid color-mix(in srgb, var(--accent) 35%, transparent); }
.gen-banner .ico { margin-top:2px; }
.gen-card { background:color-mix(in srgb, var(--panel) 92%, transparent); border:1px solid var(--border); border-radius:16px; padding:18px 20px; margin-bottom:14px; box-shadow:0 1px 0 color-mix(in srgb, var(--text) 4%, transparent), 0 10px 30px -18px rgba(0,0,0,.45); animation:gen-rise .35s ease both; }
.gen-card:nth-of-type(2) { animation-delay:.04s; } .gen-card:nth-of-type(3) { animation-delay:.08s; } .gen-card:nth-of-type(4) { animation-delay:.12s; }
.gen-step { display:flex; gap:14px; align-items:flex-start; margin-bottom:14px; }
.gen-step > span { flex:none; width:28px; height:28px; border-radius:9px; display:flex; align-items:center; justify-content:center; font-weight:800; font-size:13px; color:#fff; background:linear-gradient(135deg, var(--accent), color-mix(in srgb, var(--accent) 55%, var(--hi))); box-shadow:0 4px 12px -4px color-mix(in srgb, var(--accent) 70%, transparent); }
.gen-step h3 { margin:0; font-size:15px; }
.gen-step p { margin:2px 0 0; font-size:12px; color:var(--muted); }
.gen-step p.gen-warn { margin-top:6px; padding:6px 9px; border-radius:7px; color:var(--text); background:color-mix(in srgb, var(--hi) 14%, transparent); border:1px solid color-mix(in srgb, var(--hi) 35%, transparent); }
.gen-step p.gen-warn code { font-size:11px; }
.gen-step > div:nth-child(2) { flex:1; }
.gen-langs { display:grid; grid-template-columns:repeat(auto-fill, minmax(128px, 1fr)); gap:8px; }
.gen-lang { display:flex; flex-direction:column; align-items:flex-start; gap:6px; padding:10px 12px; border-radius:12px; border:1.5px solid var(--border); background:var(--panel2); color:var(--text); cursor:pointer; text-align:left; transition:border-color .15s, transform .12s, background .2s; }
.gen-lang:hover:not(:disabled) { transform:translateY(-1px); border-color:color-mix(in srgb, var(--accent) 60%, var(--border)); }
.gen-lang.sel { border-color:var(--accent); background:color-mix(in srgb, var(--accent) 12%, var(--panel2)); box-shadow:0 0 0 3px color-mix(in srgb, var(--accent) 18%, transparent); }
.gen-lang:disabled { cursor:default; opacity:.6; }
.gen-lang .name { font-weight:700; font-size:13px; }
.gen-lang .bar { width:100%; height:4px; border-radius:999px; background:var(--panel3); overflow:hidden; }
.gen-lang .bar i { display:block; height:100%; background:#22c55e; border-radius:999px; transition:width .4s; }
.gen-lang .sub { font-size:10.5px; color:var(--muted); }
.gen-voices { display:grid; grid-template-columns:repeat(auto-fill, minmax(190px, 1fr)); gap:8px; }
.gen-voice { display:flex; gap:10px; align-items:center; padding:9px 10px; border-radius:12px; border:1.5px solid var(--border); background:var(--panel2); cursor:pointer; transition:border-color .15s, transform .12s, background .2s; }
.gen-voice:hover { transform:translateY(-1px); border-color:color-mix(in srgb, var(--accent) 60%, var(--border)); }
.gen-voice.sel { border-color:var(--accent); background:color-mix(in srgb, var(--accent) 12%, var(--panel2)); box-shadow:0 0 0 3px color-mix(in srgb, var(--accent) 18%, transparent); }
.gen-voice .avatar { flex:none; width:36px; height:36px; border-radius:50%; display:flex; align-items:center; justify-content:center; font-weight:800; color:#fff; font-size:15px; background:linear-gradient(135deg, hsl(var(--h) 75% 58%), hsl(calc(var(--h) + 50) 70% 45%)); box-shadow:inset 0 -2px 6px rgba(0,0,0,.2); }
.gen-voice .who { flex:1; min-width:0; }
.gen-voice .vname { font-weight:700; font-size:13px; display:flex; gap:5px; align-items:center; }
.gen-voice .star { color:#f59e0b; font-size:11px; }
.gen-voice .vtags { font-size:11px; color:var(--muted); }
.gen-voice .play { flex:none; width:30px; height:30px; border-radius:50%; border:1px solid var(--border); background:var(--panel); color:var(--text); cursor:pointer; display:flex; align-items:center; justify-content:center; font-size:11px; padding:0; transition:background .15s, transform .12s; }
.gen-voice .play:hover { transform:scale(1.08); background:var(--accent); color:#fff; border-color:transparent; }
.gen-voice .play.on { background:var(--accent); color:#fff; border-color:transparent; box-shadow:0 0 0 4px color-mix(in srgb, var(--accent) 25%, transparent); animation:gen-pulse 1.2s ease-in-out infinite; }
.gen-sliders { display:grid; grid-template-columns:repeat(auto-fit, minmax(220px, 1fr)); gap:18px 26px; }
.gen-slider { display:flex; flex-direction:column; gap:8px; font-size:12.5px; }
.gen-slider > div:first-child { display:flex; justify-content:space-between; align-items:baseline; }
.gen-slider b { font-family:ui-monospace, Menlo, Consolas, monospace; font-size:12px; color:var(--accent); background:color-mix(in srgb, var(--accent) 12%, transparent); padding:1px 7px; border-radius:6px; }
.gen-slider input[type=range] { width:100%; accent-color:var(--accent); }
.gen-slider .ends { display:flex; justify-content:space-between; font-size:10.5px; color:var(--muted); }
.gen-check { display:flex; gap:8px; align-items:center; margin-top:14px; font-size:12.5px; cursor:pointer; }
.gen-check input { accent-color:var(--accent); }
.gen-quick { display:flex; gap:4px; background:var(--panel2); border:1px solid var(--border); padding:3px; border-radius:10px; }
.gen-quick button { border:none; background:none; color:var(--muted); padding:4px 10px; border-radius:7px; font-size:12px; cursor:pointer; }
.gen-quick button.sel { background:var(--panel); color:var(--text); box-shadow:0 1px 4px rgba(0,0,0,.2); }
.gen-parts { display:flex; flex-direction:column; gap:4px; }
.gen-part { display:flex; gap:12px; align-items:center; padding:9px 12px; border-radius:10px; border:1px solid transparent; cursor:pointer; transition:background .15s, border-color .15s; }
.gen-part:hover { background:var(--panel2); }
.gen-part.on { background:color-mix(in srgb, var(--accent) 8%, transparent); border-color:color-mix(in srgb, var(--accent) 30%, transparent); }
.gen-part.off { opacity:.55; cursor:default; }
.gen-part input { accent-color:var(--accent); width:15px; height:15px; }
.gen-part .num { font-family:ui-monospace, Menlo, Consolas, monospace; color:var(--muted); font-size:11px; }
.gen-part .what { flex:1; min-width:0; }
.gen-part .pname { font-weight:600; font-size:13px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.gen-part .pmeta { font-size:11px; color:var(--muted); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.gen-part .pmeta code { font-size:10.5px; }
.gen-pill { flex:none; font-size:10.5px; font-weight:700; padding:2px 9px; border-radius:999px; border:1px solid; }
.gen-pill.ok { color:#16a34a; border-color:color-mix(in srgb, #22c55e 45%, transparent); background:color-mix(in srgb, #22c55e 12%, transparent); }
.gen-pill.stale { color:#d97706; border-color:color-mix(in srgb, #f59e0b 45%, transparent); background:color-mix(in srgb, #f59e0b 12%, transparent); }
.gen-pill.own { color:#8b5cf6; border-color:color-mix(in srgb, #a78bfa 45%, transparent); background:color-mix(in srgb, #a78bfa 12%, transparent); }
.gen-pill.missing, .gen-pill.skip { color:var(--muted); border-color:var(--border); background:var(--panel2); }
.gen-empty { text-align:center; padding:36px 24px; }
.gen-empty-icon { width:68px; height:68px; margin:0 auto 12px; border-radius:20px; display:flex; align-items:center; justify-content:center; background:color-mix(in srgb, var(--accent) 14%, transparent); }
.gen-empty h3 { margin:0 0 6px; }
.gen-empty p { color:var(--muted); font-size:13px; max-width:520px; margin:6px auto; }
.gen-tree { display:inline-block; text-align:left; font-size:12px; background:var(--panel2); border:1px solid var(--border); border-radius:10px; padding:12px 16px; margin:10px 0; }
.gen-muted { color:var(--muted); }
.gen-dock { position:sticky; bottom:0; z-index:5; padding:12px 28px 16px; background:linear-gradient(to top, var(--bg) 70%, transparent); }
.gen-dock > div { max-width:924px; margin:0 auto; }
.gen-action { display:flex; gap:16px; align-items:center; justify-content:space-between; padding:12px 14px 12px 20px; border-radius:16px; background:color-mix(in srgb, var(--panel) 90%, transparent); border:1px solid var(--border); backdrop-filter:blur(12px); box-shadow:0 18px 40px -20px rgba(0,0,0,.6); }
.gen-summary { display:flex; flex-direction:column; gap:2px; min-width:0; }
.gen-summary b { font-size:14px; }
.gen-summary span { font-size:12px; color:var(--muted); }
.gen-summary .gen-error { color:#e5484d; }
.gen-go { display:inline-flex; gap:10px; align-items:center; border:none; cursor:pointer; padding:12px 22px; border-radius:12px; font-size:14px; font-weight:700; color:#fff;
  background:linear-gradient(135deg, var(--accent), color-mix(in srgb, var(--accent) 50%, #a855f7)); box-shadow:0 10px 24px -10px color-mix(in srgb, var(--accent) 90%, transparent), inset 0 1px 0 rgba(255,255,255,.25); transition:transform .12s, box-shadow .2s, filter .2s; }
.gen-go .ico { filter:invert(1); }
.gen-go:hover:not(:disabled) { transform:translateY(-1px); filter:brightness(1.08); box-shadow:0 14px 30px -10px color-mix(in srgb, var(--accent) 90%, transparent); }
.gen-go:disabled { opacity:.45; cursor:default; box-shadow:none; }
.gen-buttons { display:flex; gap:8px; align-items:center; flex-wrap:wrap; justify-content:flex-end; }
.gen-again { border:1px solid var(--border); background:var(--panel); color:var(--text); border-radius:12px; padding:11px 14px; font-size:13px; font-weight:600; cursor:pointer; white-space:nowrap; transition:border-color .15s; }
.gen-again:hover:not(:disabled) { border-color:var(--accent); }
.gen-again:disabled { opacity:.45; cursor:default; }
.gen-job { padding:14px 18px; border-radius:16px; margin-bottom:10px; background:color-mix(in srgb, var(--panel) 92%, transparent); border:1px solid var(--border); backdrop-filter:blur(12px); box-shadow:0 18px 40px -20px rgba(0,0,0,.6); animation:gen-rise .3s ease both; }
.gen-job.running { border-color:color-mix(in srgb, var(--accent) 50%, var(--border)); }
.gen-job.done { border-color:color-mix(in srgb, #22c55e 50%, var(--border)); }
.gen-job.failed { border-color:color-mix(in srgb, #e5484d 60%, var(--border)); }
.gen-job-head { display:flex; gap:12px; align-items:center; }
.gen-job-title { flex:1; min-width:0; display:flex; flex-direction:column; gap:1px; }
.gen-job-title b { font-size:14px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.gen-job-title span { font-size:11.5px; color:var(--muted); font-variant-numeric:tabular-nums; }
.gen-pct { font-size:22px; font-variant-numeric:tabular-nums; color:var(--accent); }
.gen-badge { width:22px; height:22px; border-radius:50%; display:flex; align-items:center; justify-content:center; font-size:12px; font-weight:800; color:#fff; flex:none; }
.gen-badge.good { background:#22c55e; } .gen-badge.bad { background:#e5484d; }
.gen-job.cancelled .gen-badge.good { background:var(--muted); }
.gen-cancel, .gen-dismiss, .gen-play, .gen-logbtn { border:1px solid var(--border); background:var(--panel2); color:var(--text); border-radius:9px; padding:6px 12px; font-size:12px; cursor:pointer; }
.gen-cancel:hover { border-color:#e5484d; color:#e5484d; }
.gen-play { display:inline-flex; gap:6px; align-items:center; background:var(--accent); color:#fff; border-color:transparent; font-weight:700; }
.gen-play .ico { filter:invert(1); }
.gen-dismiss { padding:6px 9px; }
.gen-logbtn { margin-top:10px; padding:3px 9px; font-size:11px; color:var(--muted); }
.gen-bar { position:relative; height:10px; border-radius:999px; background:var(--panel3); overflow:hidden; margin:12px 0 2px; }
.gen-bar i { position:absolute; left:0; top:0; bottom:0; border-radius:999px; transition:width .45s ease;
  background:linear-gradient(90deg, var(--accent), color-mix(in srgb, var(--accent) 50%, #a855f7)); }
.gen-bar i::after { content:""; position:absolute; inset:0; border-radius:inherit; }
.gen-job.running .gen-bar i { box-shadow:0 0 14px color-mix(in srgb, var(--accent) 60%, transparent); }
.gen-job.running .gen-bar i::after { background-image:linear-gradient(45deg, rgba(255,255,255,.2) 25%, transparent 25%, transparent 50%, rgba(255,255,255,.2) 50%, rgba(255,255,255,.2) 75%, transparent 75%, transparent); background-size:20px 20px; animation:gen-stripes .6s linear infinite; }
.gen-job.done .gen-bar i { background:#22c55e; }
.gen-job.failed .gen-bar i { background:#e5484d; }
.gen-bar.indeterminate i { width:40% !important; transition:none; animation:gen-slide 1.3s ease-in-out infinite; }
.gen-chips { display:flex; flex-wrap:wrap; gap:6px; margin-top:12px; }
.gen-chip { position:relative; overflow:hidden; display:inline-flex; gap:6px; align-items:center; padding:3px 10px; border-radius:999px; font-size:11px; border:1px solid var(--border); background:var(--panel2); color:var(--muted); font-family:ui-monospace, Menlo, Consolas, monospace; }
.gen-chip em { font-style:normal; opacity:.75; }
.gen-chip.done { color:var(--text); border-color:color-mix(in srgb, #22c55e 45%, transparent); }
.gen-chip.done .mark { color:#22c55e; }
.gen-chip.cur { color:var(--text); border-color:var(--accent); }
.gen-chip.cur .mark { color:var(--accent); animation:gen-pulse 1s ease-in-out infinite; }
.gen-chip.cur i { position:absolute; left:0; bottom:0; height:2px; background:var(--accent); transition:width .4s; }
.gen-err, .gen-log { margin:10px 0 0; max-height:180px; overflow:auto; font-size:11px; background:var(--panel2); border:1px solid var(--border); border-radius:8px; padding:8px 10px; white-space:pre-wrap; }
.gen-err { color:#e5484d; }
@media (max-width: 700px) {
  .gen-wrap { padding:18px 14px; } .gen-dock { padding:10px 14px 12px; }
  .gen-hero h2 { font-size:24px; } .gen-stats { width:100%; } .gen-stat { flex:1; min-width:0; }
  .gen-action { flex-direction:column; align-items:stretch; } .gen-buttons { justify-content:stretch; } .gen-buttons > * { flex:1; } .gen-go { justify-content:center; }
}
/* ---- recorded voice picker (player bar) */
.vpick { position:relative; margin:2px 0; }
.vavatar { flex:none; border-radius:50%; display:inline-flex; align-items:center; justify-content:center; font-weight:800; color:#fff; background:linear-gradient(135deg, hsl(var(--h) 75% 58%), hsl(calc(var(--h) + 50) 70% 45%)); box-shadow:inset 0 -2px 5px rgba(0,0,0,.2); line-height:1; }
.vchip { display:flex; align-items:center; gap:8px; width:100%; padding:4px 10px 4px 4px; border-radius:999px; border:1px solid var(--border); background:var(--panel2); color:var(--text); cursor:pointer; text-align:left; transition:border-color .15s, box-shadow .2s; }
.vchip:hover { border-color:color-mix(in srgb, var(--accent) 60%, var(--border)); }
.vchip.on { border-color:color-mix(in srgb, var(--accent) 55%, var(--border)); box-shadow:0 0 0 3px color-mix(in srgb, var(--accent) 15%, transparent); }
.vchip:not(.on) .vavatar { filter:grayscale(.8); opacity:.7; }
.vchip .vlabel { flex:1; min-width:0; display:flex; flex-direction:column; line-height:1.15; }
.vchip .vlabel small { font-size:9.5px; text-transform:uppercase; letter-spacing:.5px; color:var(--muted); }
.vchip .vlabel b { font-size:12.5px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.vchip .caret { color:var(--muted); font-size:10px; }
.vchip .vwarn { flex:none; font-size:10.5px; font-weight:700; font-variant-numeric:tabular-nums; padding:1px 6px; border-radius:999px; color:#d97706; background:color-mix(in srgb, #f59e0b 14%, transparent); border:1px solid color-mix(in srgb, #f59e0b 45%, transparent); }
.vpop-shade { position:fixed; inset:0; z-index:59; }
.vpop { position:absolute; left:0; bottom:calc(100% + 8px); width:290px; z-index:60; padding:8px; border-radius:14px; background:color-mix(in srgb, var(--panel) 96%, transparent); border:1px solid var(--border); backdrop-filter:blur(14px); box-shadow:0 20px 44px -12px rgba(0,0,0,.55); animation:vpop-in .16s ease-out both; transform-origin:bottom left; }
@keyframes vpop-in { from { opacity:0; transform:translateY(6px) scale(.97); } to { opacity:1; transform:none; } }
.vpop-head { display:flex; justify-content:space-between; align-items:center; padding:4px 8px 8px; font-size:10.5px; font-weight:700; text-transform:uppercase; letter-spacing:.6px; color:var(--muted); }
.vpop-head em { font-style:normal; background:var(--panel3); color:var(--text); border-radius:999px; padding:0 7px; }
.vopt { display:flex; align-items:center; gap:10px; width:100%; padding:7px 8px; border:1px solid transparent; border-radius:10px; background:none; color:var(--text); cursor:pointer; text-align:left; transition:background .12s, border-color .12s; }
.vopt:hover { background:var(--panel2); }
.vopt.sel { background:color-mix(in srgb, var(--accent) 12%, transparent); border-color:color-mix(in srgb, var(--accent) 40%, transparent); }
.vopt .vtext { flex:1; min-width:0; display:flex; flex-direction:column; gap:2px; }
.vopt .vtext b { font-size:13px; display:flex; gap:5px; align-items:center; }
.vopt .vtext .star { color:#f59e0b; font-size:10px; }
.vopt .vtext > span:not(.vcover) { font-size:11px; color:var(--muted); }
.vopt .vcover { display:block; height:3px; border-radius:999px; background:var(--panel3); overflow:hidden; margin-top:2px; }
.vopt .vcover i { display:block; height:100%; background:#22c55e; border-radius:999px; }
.vopt.partial .vcover i { background:#f59e0b; }
.vopt .vtext > span.vmiss { color:#d97706; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.vopt .vcount { display:flex; flex-direction:column; align-items:flex-end; font-size:12px; font-weight:700; font-variant-numeric:tabular-nums; line-height:1.1; }
.vopt .vcount small { font-size:9.5px; font-weight:400; color:var(--muted); }
.vopt .vcheck { width:14px; color:var(--accent); font-weight:800; }
.vpop-foot { display:flex; gap:6px; margin-top:6px; padding-top:8px; border-top:1px solid var(--border); }
.vpop-foot button { flex:1; font-size:11.5px; padding:5px 8px; border-radius:8px; border:1px solid var(--border); background:var(--panel2); color:var(--text); cursor:pointer; }
.vpop-foot button.sel { border-color:var(--accent); color:var(--accent); }
.gen-others { display:flex; }
.gen-others .avatar { width:18px; height:18px; border-radius:50%; display:flex; align-items:center; justify-content:center; font-size:9px; font-weight:800; color:#fff; margin-left:-5px; box-shadow:0 0 0 2px var(--panel); background:linear-gradient(135deg, hsl(var(--h) 75% 58%), hsl(calc(var(--h) + 50) 70% 45%)); }
.gen-voice .vhas { margin-left:6px; color:#22c55e; font-weight:600; }
.gen-outdated { border-color:color-mix(in srgb, #f59e0b 50%, var(--border)); background:linear-gradient(135deg, color-mix(in srgb, #f59e0b 9%, var(--panel)), var(--panel) 60%); }
.gen-out-head { display:flex; gap:14px; align-items:flex-start; margin-bottom:12px; }
.gen-out-head h3 { margin:0; font-size:15px; }
.gen-out-head p { margin:2px 0 0; font-size:12px; color:var(--muted); }
.gen-out-icon { flex:none; width:28px; height:28px; border-radius:9px; display:flex; align-items:center; justify-content:center; font-weight:800; color:#fff; background:linear-gradient(135deg, #f59e0b, #ef4444); box-shadow:0 4px 12px -4px rgba(245,158,11,.7); }
.gen-out-list { display:flex; flex-direction:column; gap:6px; }
.gen-out-row { display:flex; gap:12px; align-items:center; padding:8px 10px; border-radius:10px; background:var(--panel2); border:1px solid var(--border); }
.gen-out-row .avatar { flex:none; width:28px; height:28px; border-radius:50%; display:flex; align-items:center; justify-content:center; font-weight:800; color:#fff; font-size:12px; background:linear-gradient(135deg, hsl(var(--h) 75% 58%), hsl(calc(var(--h) + 50) 70% 45%)); }
.gen-out-row .avatar.trash { background:var(--panel3); }
.gen-out-row .what { flex:1; min-width:0; display:flex; flex-direction:column; }
.gen-out-row .what b { font-size:13px; }
.gen-out-row .what span { font-size:11px; color:var(--muted); font-family:ui-monospace, Menlo, Consolas, monospace; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.gen-out-note { font-size:11px; color:var(--muted); }
.gen-out-foot { display:flex; gap:8px; justify-content:flex-end; margin-top:12px; flex-wrap:wrap; }
.gen-mini { border:1px solid var(--border); background:var(--panel); color:var(--text); border-radius:8px; padding:5px 11px; font-size:12px; cursor:pointer; white-space:nowrap; transition:border-color .15s, background .15s; }
.gen-mini:hover:not(:disabled) { border-color:var(--accent); }
.gen-mini:disabled { opacity:.5; cursor:default; }
.gen-mini.go { background:var(--accent); color:#fff; border-color:transparent; font-weight:700; }
.gen-mini.go.big { padding:7px 14px; }
.gen-mini.danger { background:#e5484d; color:#fff; border-color:transparent; font-weight:700; animation:gen-pulse 1.2s ease-in-out infinite; }
.gen-btn .gen-dot.warn { background:#f59e0b; }
"#;
