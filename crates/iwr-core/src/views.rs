//! Build a positioned, colour-coded graph for each visualization mode from a
//! [`Project`]. The UI only draws what comes out of here.

use crate::layout::{self, LayoutEdge, LayoutNode, LayoutOptions, TreeNode};
use crate::model::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Simplified "code as blocks" view (rendered directly by the UI, no graph).
    Code,
    CallTree,
    ControlFlow,
    Architecture,
    BranchTree,
    Structure,
    Types,
    ErrorFlow,
}

impl Mode {
    /// Graph modes (everything except `Code`).
    pub const GRAPHS: [Mode; 7] = [Mode::CallTree, Mode::ControlFlow, Mode::Architecture, Mode::BranchTree, Mode::Structure, Mode::Types, Mode::ErrorFlow];
    pub const ALL: [Mode; 8] = [Mode::Code, Mode::CallTree, Mode::ControlFlow, Mode::Architecture, Mode::BranchTree, Mode::Structure, Mode::Types, Mode::ErrorFlow];
    pub fn label(&self) -> &'static str {
        match self {
            Mode::Code => "Code",
            Mode::CallTree => "Call tree",
            Mode::ControlFlow => "Control flow",
            Mode::Architecture => "Architecture",
            Mode::BranchTree => "Branch tree",
            Mode::Structure => "Structure",
            Mode::Types => "Types & traits",
            Mode::ErrorFlow => "Error flow",
        }
    }
    pub fn description(&self) -> &'static str {
        match self {
            Mode::Code => "The code itself as simple nested blocks: functions, loops, branches, calls. Click to open.",
            Mode::CallTree => "Which function calls which, starting from the entry point. Expand nodes to go deeper.",
            Mode::ControlFlow => "Statement-level flow of one function: branches, loops, early returns, error exits.",
            Mode::Architecture => "Modules and their dependencies. Expand a module to see its items.",
            Mode::BranchTree => "Every possible path through a function as a tree. Expand calls to inline the callee.",
            Mode::Structure => "Files, modules, impl blocks and items as nested blocks.",
            Mode::Types => "Structs, enums, traits and how they relate (fields, impls, supertraits).",
            Mode::ErrorFlow => "Where errors are created, propagated with ?, handled, or turned into panics.",
        }
    }
    /// Does this mode need a selected function as its root?
    pub fn needs_function(&self) -> bool {
        matches!(self, Mode::ControlFlow | Mode::BranchTree)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Function,
    Method,
    Main,
    Struct,
    Enum,
    Trait,
    TypeAlias,
    Const,
    Module,
    Impl,
    External,
    Macro,
    Entry,
    Exit,
    Block,
    If,
    Match,
    Loop,
    Call,
    Return,
    Jump,
    Propagate,
    Panic,
    Await,
    File,
    ErrorType,
    Note,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Call,
    CallInLoop,
    CallInBranch,
    Recursion,
    Propagate,
    Unwrap,
    Await,
    Construct,
    Next,
    True,
    False,
    Arm,
    LoopBody,
    LoopBack,
    Break,
    Continue,
    Error,
    Return,
    Contains,
    Implements,
    Supertrait,
    Uses,
    Aliases,
    Dependency,
    Handles,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VNode {
    pub id: String,
    pub label: String,
    pub sublabel: String,
    pub kind: NodeKind,
    pub item: Option<ItemId>,
    pub cfg_node: Option<usize>,
    pub span: Option<Span>,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub expandable: bool,
    pub expanded: bool,
    pub badges: Vec<String>,
    /// Nesting depth (used for tint in nested views).
    pub depth: usize,
    /// Parent block id in the structure view.
    pub parent: Option<String>,
    /// True for container boxes (module/impl/file) drawn behind their children.
    pub container: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    pub label: Option<String>,
    pub points: Vec<(f64, f64)>,
    pub backward: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Graph {
    pub mode: Option<Mode>,
    pub nodes: Vec<VNode>,
    pub edges: Vec<VEdge>,
    pub width: f64,
    pub height: f64,
    /// Human-readable note shown when the graph is empty or degraded.
    pub note: Option<String>,
}

impl Graph {
    pub fn node(&self, id: &str) -> Option<&VNode> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewOptions {
    /// Root item (function for call/branch/cfg views; module or type for others).
    pub root: Option<ItemId>,
    /// Restrict Architecture / Structure / Types / ErrorFlow to this module subtree.
    pub module: Option<ModuleId>,
    /// Node ids that the user expanded (or collapsed when `collapsed` contains them).
    pub expanded: HashSet<String>,
    pub collapsed: HashSet<String>,
    /// Default expansion depth.
    pub depth: usize,
    pub show_external: bool,
    pub show_macros: bool,
    pub show_tests: bool,
    pub show_constructs: bool,
    pub max_nodes: usize,
}

impl Default for ViewOptions {
    fn default() -> Self {
        Self {
            root: None,
            module: None,
            expanded: HashSet::new(),
            collapsed: HashSet::new(),
            depth: 2,
            show_external: false,
            show_macros: false,
            show_tests: false,
            show_constructs: false,
            max_nodes: 400,
        }
    }
}

impl ViewOptions {
    /// Is `m` inside the scoped module subtree (or is there no scope)?
    pub fn in_scope(&self, p: &Project, m: ModuleId) -> bool {
        let Some(scope) = self.module else { return true };
        let mut cur = Some(m);
        while let Some(c) = cur {
            if c == scope {
                return true;
            }
            cur = p.module(c).parent;
        }
        false
    }
    fn is_expanded(&self, id: &str, depth: usize) -> bool {
        if self.collapsed.contains(id) {
            return false;
        }
        if self.expanded.contains(id) {
            return true;
        }
        depth < self.depth
    }
}

const FONT: f64 = 13.0;

fn node_size(label: &str, sublabel: &str) -> (f64, f64) {
    let w = layout::text_width(label, FONT).max(layout::text_width(sublabel, FONT * 0.85)) + 28.0;
    let lines = label.lines().count().max(1) + if sublabel.is_empty() { 0 } else { 1 };
    (w.clamp(70.0, 300.0), 20.0 + 16.0 * lines as f64)
}

fn kind_of_item(it: &Item) -> NodeKind {
    match it.kind {
        ItemKind::Function => {
            if it.fn_info().map(|f| f.is_main).unwrap_or(false) {
                NodeKind::Main
            } else {
                NodeKind::Function
            }
        }
        ItemKind::Method => NodeKind::Method,
        ItemKind::Struct => NodeKind::Struct,
        ItemKind::Enum => {
            if it.name.ends_with("Error") || it.name.ends_with("Err") {
                NodeKind::ErrorType
            } else {
                NodeKind::Enum
            }
        }
        ItemKind::Trait => NodeKind::Trait,
        ItemKind::TypeAlias => NodeKind::TypeAlias,
        ItemKind::Const | ItemKind::Static => NodeKind::Const,
        ItemKind::Module => NodeKind::Module,
        ItemKind::Impl => NodeKind::Impl,
    }
}

fn fn_badges(f: &FnInfo) -> Vec<String> {
    let mut b = Vec::new();
    if f.is_async {
        b.push("async".into());
    }
    if f.is_unsafe {
        b.push("unsafe".into());
    }
    if f.returns_result {
        b.push("Result".into());
    } else if f.returns_option {
        b.push("Option".into());
    }
    if f.is_recursive {
        b.push("↻ recursive".into());
    }
    if f.loops > 0 {
        b.push(format!("{}× loop", f.loops));
    }
    if f.branches > 0 {
        b.push(format!("{}× branch", f.branches));
    }
    if f.question_marks > 0 {
        b.push(format!("{}× ?", f.question_marks));
    }
    if f.unwraps + f.expects + f.panics > 0 {
        b.push(format!("{}× panic!", f.unwraps + f.expects + f.panics));
    }
    if f.is_test {
        b.push("test".into());
    }
    b
}

fn item_label(it: &Item) -> String {
    match it.kind {
        ItemKind::Method => match it.fn_info().and_then(|f| f.owner_type_name.clone().or(f.trait_name.clone())) {
            Some(o) => format!("{}::{}", o, it.name),
            None => it.name.clone(),
        },
        ItemKind::Impl => it.name.clone(),
        _ => it.name.clone(),
    }
}

fn mk_node(id: String, it: &Item, depth: usize) -> VNode {
    let label = item_label(it);
    let sublabel = if let Some(f) = it.fn_info() {
        let p: Vec<&str> = f.params.iter().map(|p| p.ty.as_str()).collect();
        let s = format!("({})", p.join(", "));
        match &f.ret {
            Some(r) => format!("{} → {}", s, r),
            None => s,
        }
    } else {
        it.kind.label().to_string()
    };
    let sublabel = crate::shorten(&sublabel, 44);
    let badges = it.fn_info().map(fn_badges).unwrap_or_default();
    let (mut w, h) = node_size(&label, &sublabel);
    if let Some(b) = badges.first() {
        // badge sits top-right next to the title
        w = w.max(layout::text_width(&label, FONT) + layout::text_width(b, 9.0) + 40.0);
    }
    VNode {
        id,
        label,
        sublabel,
        kind: kind_of_item(it),
        item: Some(it.id),
        cfg_node: None,
        span: Some(it.span),
        x: 0.0,
        y: 0.0,
        w,
        h,
        expandable: false,
        expanded: false,
        badges,
        depth,
        parent: None,
        container: false,
    }
}

fn apply_layout(g: &mut Graph, res: &layout::LayoutResult) {
    for n in &mut g.nodes {
        if let Some(r) = res.rects.get(&n.id) {
            n.x = r.x;
            n.y = r.y;
            n.w = r.w;
            n.h = r.h;
        }
    }
    let mut routes: HashMap<(String, String), Vec<&layout::EdgeRoute>> = HashMap::new();
    for r in &res.edges {
        routes.entry((r.from.clone(), r.to.clone())).or_default().push(r);
    }
    let mut used: HashMap<(String, String), usize> = HashMap::new();
    for e in &mut g.edges {
        let k = (e.from.clone(), e.to.clone());
        let i = used.entry(k.clone()).or_insert(0);
        if let Some(rs) = routes.get(&k) {
            let r = rs[(*i).min(rs.len() - 1)];
            e.points = r.points.clone();
            e.backward = r.backward;
            *i += 1;
        }
    }
    g.width = res.width;
    g.height = res.height;
}

fn layered_layout(g: &mut Graph, reverse: &dyn Fn(&VEdge) -> bool, opts: &LayoutOptions) {
    let nodes: Vec<LayoutNode> = g.nodes.iter().map(|n| LayoutNode { id: n.id.clone(), w: n.w, h: n.h }).collect();
    let edges: Vec<LayoutEdge> = g.edges.iter().map(|e| LayoutEdge { from: e.from.clone(), to: e.to.clone(), reverse_rank: reverse(e) }).collect();
    let res = layout::layered(&nodes, &edges, opts);
    apply_layout(g, &res);
}

/// Entry point: build the graph for a mode.
pub fn build(p: &Project, mode: Mode, opts: &ViewOptions) -> Graph {
    let mut g = match mode {
        Mode::Code => Graph::default(),
        Mode::CallTree => call_tree(p, opts),
        Mode::ControlFlow => control_flow(p, opts),
        Mode::Architecture => architecture(p, opts),
        Mode::BranchTree => branch_tree(p, opts),
        Mode::Structure => structure(p, opts),
        Mode::Types => types(p, opts),
        Mode::ErrorFlow => error_flow(p, opts),
    };
    g.mode = Some(mode);
    g
}

/// Default root function: main, else the first function.
pub fn default_root(p: &Project) -> Option<ItemId> {
    p.main.or_else(|| p.functions().find(|f| f.fn_info().map(|f| f.cfg.is_some()).unwrap_or(false)).map(|f| f.id))
}

// ---------------------------------------------------------------- call tree

fn call_tree(p: &Project, opts: &ViewOptions) -> Graph {
    let mut g = Graph::default();
    let Some(root) = opts.root.or_else(|| default_root(p)) else {
        g.note = Some("No functions found.".into());
        return g;
    };
    let mut count = 0usize;
    fn walk(p: &Project, opts: &ViewOptions, item: ItemId, nid: String, depth: usize, stack: &mut Vec<ItemId>, g: &mut Graph, count: &mut usize) -> TreeNode {
        let it = p.item(item);
        let mut node = mk_node(nid.clone(), it, depth);
        let callees: Vec<&CallEdge> = p
            .callees(item)
            .into_iter()
            .filter(|c| match c.kind {
                CallKind::Macro => opts.show_macros,
                CallKind::Construct => opts.show_constructs,
                _ => c.to.is_some() || opts.show_external,
            })
            .filter(|c| c.to.map(|t| opts.show_tests || !p.item(t).fn_info().map(|f| f.is_test).unwrap_or(false)).unwrap_or(true))
            .collect();
        let mut children = Vec::new();
        let is_rec = stack.contains(&item);
        node.expandable = !callees.is_empty() && !is_rec;
        node.expanded = node.expandable && opts.is_expanded(&nid, depth) && *count < opts.max_nodes;
        if is_rec {
            node.badges.insert(0, "↻ recursion".into());
        }
        if node.expanded {
            stack.push(item);
            let mut seen: HashSet<String> = HashSet::new();
            for c in callees {
                let key = match c.to {
                    Some(t) => format!("i{}", t),
                    None => format!("x{}", c.callee),
                };
                if !seen.insert(key.clone()) {
                    continue;
                }
                *count += 1;
                let cid = format!("{}/{}", nid, key);
                let edge_kind = if c.is_recursive || c.to.map(|t| stack.contains(&t)).unwrap_or(false) {
                    EdgeKind::Recursion
                } else if c.propagated {
                    EdgeKind::Propagate
                } else if c.unwrapped {
                    EdgeKind::Unwrap
                } else if c.kind == CallKind::Construct {
                    EdgeKind::Construct
                } else if c.in_loop {
                    EdgeKind::CallInLoop
                } else if c.in_branch {
                    EdgeKind::CallInBranch
                } else {
                    EdgeKind::Call
                };
                let mut lbl = Vec::new();
                if c.in_loop {
                    lbl.push("in loop");
                }
                if c.in_branch {
                    lbl.push("conditional");
                }
                if c.propagated {
                    lbl.push("?");
                }
                if c.unwrapped {
                    lbl.push("unwrap");
                }
                if c.is_await {
                    lbl.push("await");
                }
                let label = if lbl.is_empty() { None } else { Some(lbl.join(", ")) };
                let child = match c.to {
                    Some(t) => walk(p, opts, t, cid.clone(), depth + 1, stack, g, count),
                    None => {
                        let (w, h) = node_size(&c.callee, "");
                        g.nodes.push(VNode {
                            id: cid.clone(),
                            label: c.callee.clone(),
                            sublabel: if c.kind == CallKind::Macro { "macro".into() } else { "external".into() },
                            kind: if c.kind == CallKind::Macro { NodeKind::Macro } else { NodeKind::External },
                            item: None,
                            cfg_node: None,
                            span: Some(c.span),
                            x: 0.0,
                            y: 0.0,
                            w,
                            h,
                            expandable: false,
                            expanded: false,
                            badges: vec![],
                            depth: depth + 1,
                            parent: None,
                            container: false,
                        });
                        TreeNode { id: cid.clone(), w, h, children: vec![] }
                    }
                };
                g.edges.push(VEdge { id: format!("e{}", g.edges.len()), from: nid.clone(), to: cid, kind: edge_kind, label, points: vec![], backward: false });
                children.push(child);
            }
            stack.pop();
        }
        let (w, h) = (node.w, node.h);
        g.nodes.push(node);
        TreeNode { id: nid, w, h, children }
    }
    let mut stack = Vec::new();
    let tree = walk(p, opts, root, format!("i{}", root), 0, &mut stack, &mut g, &mut count);
    let res = layout::tree(&tree, &LayoutOptions { layer_gap: 56.0, node_gap: 22.0, ..Default::default() });
    apply_layout(&mut g, &res);
    if count >= opts.max_nodes {
        g.note = Some(format!("Tree truncated at {} nodes; collapse or change the root.", opts.max_nodes));
    }
    g
}

// ---------------------------------------------------------------- control flow

fn cfg_kind(k: CfgNodeKind) -> NodeKind {
    match k {
        CfgNodeKind::Entry => NodeKind::Entry,
        CfgNodeKind::Exit => NodeKind::Exit,
        CfgNodeKind::Block => NodeKind::Block,
        CfgNodeKind::If => NodeKind::If,
        CfgNodeKind::Match => NodeKind::Match,
        CfgNodeKind::Loop => NodeKind::Loop,
        CfgNodeKind::Call => NodeKind::Call,
        CfgNodeKind::Return => NodeKind::Return,
        CfgNodeKind::Jump => NodeKind::Jump,
        CfgNodeKind::Propagate => NodeKind::Propagate,
        CfgNodeKind::Panic => NodeKind::Panic,
        CfgNodeKind::Await => NodeKind::Await,
    }
}

fn cfg_edge_kind(k: CfgEdgeKind) -> EdgeKind {
    match k {
        CfgEdgeKind::Next => EdgeKind::Next,
        CfgEdgeKind::True => EdgeKind::True,
        CfgEdgeKind::False => EdgeKind::False,
        CfgEdgeKind::Arm => EdgeKind::Arm,
        CfgEdgeKind::LoopBody => EdgeKind::LoopBody,
        CfgEdgeKind::LoopBack => EdgeKind::LoopBack,
        CfgEdgeKind::Break => EdgeKind::Break,
        CfgEdgeKind::Continue => EdgeKind::Continue,
        CfgEdgeKind::Error => EdgeKind::Error,
        CfgEdgeKind::Return => EdgeKind::Return,
    }
}

fn cfg_vnode(p: &Project, fn_item: ItemId, n: &CfgNode, id: String, depth: usize) -> VNode {
    let calls: Vec<String> = n.calls.iter().map(|c| item_label(p.item(*c))).collect();
    let sublabel = if n.kind == CfgNodeKind::Entry {
        p.item(fn_item).signature.clone()
    } else if calls.is_empty() {
        String::new()
    } else {
        format!("→ {}", calls.join(", "))
    };
    let sublabel = crate::shorten(&sublabel, 44);
    let (w, h) = node_size(&n.label, &sublabel);
    VNode {
        id,
        label: n.label.clone(),
        sublabel,
        kind: cfg_kind(n.kind),
        item: n.calls.first().copied().or(Some(fn_item)),
        cfg_node: Some(n.id),
        span: Some(n.span),
        x: 0.0,
        y: 0.0,
        w,
        h,
        expandable: !n.calls.is_empty(),
        expanded: false,
        badges: vec![],
        depth,
        parent: None,
        container: false,
    }
}

fn control_flow(p: &Project, opts: &ViewOptions) -> Graph {
    let mut g = Graph::default();
    let Some(root) = opts.root.or_else(|| default_root(p)) else {
        g.note = Some("Select a function.".into());
        return g;
    };
    let it = p.item(root);
    let Some(cfg) = it.fn_info().and_then(|f| f.cfg.as_ref()) else {
        g.note = Some(format!("{} has no body to show.", it.name));
        return g;
    };
    for n in &cfg.nodes {
        g.nodes.push(cfg_vnode(p, root, n, format!("c{}", n.id), 0));
    }
    for (i, e) in cfg.edges.iter().enumerate() {
        g.edges.push(VEdge { id: format!("e{}", i), from: format!("c{}", e.from), to: format!("c{}", e.to), kind: cfg_edge_kind(e.kind), label: e.label.clone(), points: vec![], backward: false });
    }
    // exit node should be last: give every node an edge weight toward exit by ranking only real edges;
    // loop-back / continue edges are reversed for ranking so the layout flows downward.
    layered_layout(&mut g, &|e| matches!(e.kind, EdgeKind::LoopBack | EdgeKind::Continue), &LayoutOptions { layer_gap: 44.0, node_gap: 26.0, max_per_layer: 6, ..Default::default() });
    // push the exit node to the very bottom
    if let Some(max_y) = g.nodes.iter().filter(|n| n.kind != NodeKind::Exit).map(|n| n.y + n.h).fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v)))) {
        if let Some(exit) = g.nodes.iter_mut().find(|n| n.kind == NodeKind::Exit) {
            if exit.y < max_y + 30.0 {
                exit.y = max_y + 44.0;
                let (ex, ey, eid) = (exit.x + exit.w / 2.0, exit.y, exit.id.clone());
                g.height = g.height.max(exit.y + exit.h + 24.0);
                for e in &mut g.edges {
                    if e.to == eid && !e.backward {
                        if let Some(last) = e.points.last_mut() {
                            *last = (ex, ey);
                        }
                    }
                }
            }
        }
    }
    g
}

// ---------------------------------------------------------------- branch tree

fn branch_tree(p: &Project, opts: &ViewOptions) -> Graph {
    let mut g = Graph::default();
    let Some(root) = opts.root.or_else(|| default_root(p)) else {
        g.note = Some("Select a function.".into());
        return g;
    };
    let mut count = 0usize;
    #[allow(clippy::too_many_arguments)]
    fn walk_cfg(p: &Project, opts: &ViewOptions, fn_item: ItemId, cfg: &Cfg, node: usize, nid: String, depth: usize, path: &mut Vec<usize>, fn_stack: &mut Vec<ItemId>, g: &mut Graph, count: &mut usize) -> TreeNode {
        let n = &cfg.nodes[node];
        let mut v = cfg_vnode(p, fn_item, n, nid.clone(), depth);
        let mut children = Vec::new();
        *count += 1;
        // inline callee
        if !n.calls.is_empty() {
            let callee = n.calls[0];
            let inline_id = format!("{}/f{}", nid, callee);
            let can = p.item(callee).fn_info().map(|f| f.cfg.is_some()).unwrap_or(false) && !fn_stack.contains(&callee);
            v.expandable = can;
            v.expanded = can && opts.expanded.contains(&nid) && *count < opts.max_nodes;
            if v.expanded {
                let ccfg = p.item(callee).fn_info().unwrap().cfg.as_ref().unwrap();
                fn_stack.push(callee);
                let mut cpath = Vec::new();
                let child = walk_cfg(p, opts, callee, ccfg, ccfg.entry(), inline_id.clone(), depth + 1, &mut cpath, fn_stack, g, count);
                fn_stack.pop();
                g.edges.push(VEdge { id: format!("e{}", g.edges.len()), from: nid.clone(), to: inline_id, kind: EdgeKind::Call, label: Some(format!("enter {}", p.item(callee).name)), points: vec![], backward: false });
                children.push(child);
            }
        }
        if *count < opts.max_nodes && !path.contains(&node) {
            path.push(node);
            let succ: Vec<&CfgEdge> = cfg.succ(node).collect();
            for (i, e) in succ.iter().enumerate() {
                let cid = format!("{}/{}", nid, i);
                let kind = cfg_edge_kind(e.kind);
                let child = if matches!(e.kind, CfgEdgeKind::LoopBack | CfgEdgeKind::Continue) || path.contains(&e.to) {
                    // loop closes: draw a leaf pointing back
                    let label = if e.kind == CfgEdgeKind::Continue { "↻ continue".to_string() } else { format!("↻ back to: {}", crate::shorten(&cfg.nodes[e.to].label, 24)) };
                    let (w, h) = node_size(&label, "");
                    g.nodes.push(VNode { id: cid.clone(), label, sublabel: String::new(), kind: NodeKind::Loop, item: Some(fn_item), cfg_node: Some(e.to), span: Some(cfg.nodes[e.to].span), x: 0.0, y: 0.0, w, h, expandable: false, expanded: false, badges: vec![], depth, parent: None, container: false });
                    *count += 1;
                    TreeNode { id: cid.clone(), w, h, children: vec![] }
                } else {
                    walk_cfg(p, opts, fn_item, cfg, e.to, cid.clone(), depth, path, fn_stack, g, count)
                };
                g.edges.push(VEdge { id: format!("e{}", g.edges.len()), from: nid.clone(), to: cid, kind, label: e.label.clone(), points: vec![], backward: false });
                children.push(child);
            }
            path.pop();
        }
        let (w, h) = (v.w, v.h);
        g.nodes.push(v);
        TreeNode { id: nid, w, h, children }
    }
    let it = p.item(root);
    let Some(cfg) = it.fn_info().and_then(|f| f.cfg.as_ref()) else {
        g.note = Some(format!("{} has no body to show.", it.name));
        return g;
    };
    let mut path = Vec::new();
    let mut fn_stack = vec![root];
    let tree = walk_cfg(p, opts, root, cfg, cfg.entry(), "b".into(), 0, &mut path, &mut fn_stack, &mut g, &mut count);
    let res = layout::tree(&tree, &LayoutOptions { layer_gap: 48.0, node_gap: 18.0, ..Default::default() });
    apply_layout(&mut g, &res);
    if count >= opts.max_nodes {
        g.note = Some(format!("Tree truncated at {} nodes.", opts.max_nodes));
    }
    g
}

// ---------------------------------------------------------------- architecture

fn architecture(p: &Project, opts: &ViewOptions) -> Graph {
    let mut g = Graph::default();
    let root_mod = opts.module.unwrap_or(0);
    // module nodes
    let mod_id = |m: ModuleId| format!("m{}", m);
    fn walk(p: &Project, opts: &ViewOptions, m: ModuleId, depth: usize, g: &mut Graph) -> TreeNode {
        let module = p.module(m);
        let nid = format!("m{}", m);
        let items: Vec<&Item> = module.items.iter().map(|i| p.item(*i)).filter(|i| i.kind != ItemKind::Method && (opts.show_tests || !i.fn_info().map(|f| f.is_test).unwrap_or(false))).collect();
        let n_fn = items.iter().filter(|i| i.is_callable() || i.kind == ItemKind::Impl).count();
        let n_ty = items.iter().filter(|i| i.is_type()).count();
        let sublabel = format!("{} fn · {} types · {} submodules", n_fn, n_ty, module.children.len());
        let is_ws_root = module.path == "crate" && module.file.is_none() && !module.children.is_empty();
        let label = if is_ws_root { format!("workspace {}", p.name) } else if module.path == "crate" { format!("crate {}", p.name) } else if module.parent == Some(0) && p.module(0).file.is_none() { format!("crate {}", module.name) } else { module.name.clone() };
        let (w, h) = node_size(&label, &sublabel);
        let expandable = !items.is_empty();
        let expanded = expandable && (opts.expanded.contains(&nid));
        let mut node = VNode {
            id: nid.clone(),
            label,
            sublabel,
            kind: NodeKind::Module,
            item: None,
            cfg_node: None,
            span: module.span.or_else(|| module.file.map(|f| Span { file: f, line_start: 1, col_start: 1, line_end: 1, col_end: 1 })),
            x: 0.0,
            y: 0.0,
            w,
            h,
            expandable,
            expanded,
            badges: module.file.map(|f| vec![p.file(f).path.clone()]).unwrap_or_default(),
            depth,
            parent: None,
            container: false,
        };
        let mut children = Vec::new();
        for &c in &module.children {
            let child = walk(p, opts, c, depth + 1, g);
            g.edges.push(VEdge { id: format!("e{}", g.edges.len()), from: nid.clone(), to: child.id.clone(), kind: EdgeKind::Contains, label: None, points: vec![], backward: false });
            children.push(child);
        }
        if expanded {
            for it in items {
                let cid = format!("{}/i{}", nid, it.id);
                let mut v = mk_node(cid.clone(), it, depth + 1);
                v.badges.clear();
                let (w, h) = (v.w, v.h);
                g.nodes.push(v);
                g.edges.push(VEdge { id: format!("e{}", g.edges.len()), from: nid.clone(), to: cid.clone(), kind: EdgeKind::Contains, label: None, points: vec![], backward: false });
                children.push(TreeNode { id: cid, w, h, children: vec![] });
            }
        }
        if children.is_empty() {
            node.badges.truncate(1);
        }
        let (w, h) = (node.w, node.h);
        g.nodes.push(node);
        TreeNode { id: nid, w, h, children }
    }
    let _tree = walk(p, opts, root_mod, 0, &mut g);
    layered_layout(&mut g, &|_| false, &LayoutOptions { layer_gap: 64.0, node_gap: 26.0, max_per_layer: 7, ..Default::default() });
    // dependency edges between modules (calls / type uses), weighted
    let mut deps: HashMap<(ModuleId, ModuleId), usize> = HashMap::new();
    for c in &p.calls {
        if let Some(t) = c.to {
            let (a, b) = (p.item(c.from).module, p.item(t).module);
            if a != b {
                *deps.entry((a, b)).or_default() += 1;
            }
        }
    }
    for r in &p.type_rels {
        let (a, b) = (p.item(r.from).module, p.item(r.to).module);
        if a != b {
            *deps.entry((a, b)).or_default() += 1;
        }
    }
    let rects: HashMap<String, (f64, f64, f64, f64)> = g.nodes.iter().map(|n| (n.id.clone(), (n.x, n.y, n.w, n.h))).collect();
    let mut deps: Vec<((ModuleId, ModuleId), usize)> = deps.into_iter().collect();
    deps.sort();
    for (i, ((a, b), n)) in deps.into_iter().enumerate() {
        let (Some(ra), Some(rb)) = (rects.get(&mod_id(a)), rects.get(&mod_id(b))) else { continue };
        let (ax, ay) = (ra.0 + ra.2 / 2.0, ra.1 + ra.3 / 2.0);
        let (bx, by) = (rb.0 + rb.2 / 2.0, rb.1 + rb.3 / 2.0);
        // curved via a control point offset perpendicular to the segment
        let (dx, dy) = (bx - ax, by - ay);
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        let off = 40.0 + len / 5.0 + (i % 3) as f64 * 14.0;
        let (mx, my) = ((ax + bx) / 2.0 - dy / len * off, (ay + by) / 2.0 + dx / len * off);
        // start/end on node borders (approx: shrink toward center by half size)
        let start = border_point(*ra, mx, my);
        let end = border_point(*rb, mx, my);
        g.edges.push(VEdge { id: format!("d{}", i), from: mod_id(a), to: mod_id(b), kind: EdgeKind::Dependency, label: Some(format!("{} use{}", n, if n == 1 { "" } else { "s" })), points: vec![start, (mx, my), end], backward: false });
    }
    if opts.show_external && !p.external_crates.is_empty() {
        let mut x = 24.0;
        let y = g.height + 20.0;
        for (i, c) in p.external_crates.iter().enumerate() {
            let (w, h) = node_size(c, "crate");
            g.nodes.push(VNode { id: format!("x{}", i), label: c.clone(), sublabel: "external crate".into(), kind: NodeKind::External, item: None, cfg_node: None, span: None, x, y, w, h, expandable: false, expanded: false, badges: vec![], depth: 0, parent: None, container: false });
            x += w + 16.0;
            g.width = g.width.max(x);
            g.height = g.height.max(y + h + 24.0);
        }
    }
    g
}

fn border_point(r: (f64, f64, f64, f64), tx: f64, ty: f64) -> (f64, f64) {
    let (cx, cy) = (r.0 + r.2 / 2.0, r.1 + r.3 / 2.0);
    let (dx, dy) = (tx - cx, ty - cy);
    if dx.abs() < 1e-6 && dy.abs() < 1e-6 {
        return (cx, cy);
    }
    let sx = if dx.abs() > 1e-6 { (r.2 / 2.0) / dx.abs() } else { f64::INFINITY };
    let sy = if dy.abs() > 1e-6 { (r.3 / 2.0) / dy.abs() } else { f64::INFINITY };
    let s = sx.min(sy);
    (cx + dx * s, cy + dy * s)
}

// ---------------------------------------------------------------- structure (blocks)

fn structure(p: &Project, opts: &ViewOptions) -> Graph {
    let mut g = Graph::default();
    const COL_W: f64 = 300.0;
    const PAD: f64 = 10.0;
    const ROW: f64 = 40.0;
    const GAP: f64 = 24.0;
    // one column per module (in tree order), items stacked; impl blocks nest their methods
    let mut order: Vec<ModuleId> = Vec::new();
    fn visit(p: &Project, m: ModuleId, out: &mut Vec<ModuleId>) {
        out.push(m);
        for c in &p.module(m).children {
            visit(p, *c, out);
        }
    }
    visit(p, opts.module.unwrap_or(0), &mut order);
    let n_mods = order.iter().filter(|m| !p.module(**m).items.is_empty()).count().max(1);
    let cols = ((n_mods as f64 * 0.9).sqrt().ceil() as usize).clamp(2, 8);
    let mut col_heights = vec![PAD; cols];
    let mut col_x: Vec<f64> = (0..cols).map(|i| PAD + i as f64 * (COL_W + GAP)).collect();
    for m in order {
        let module = p.module(m);
        // pick the shortest column
        let ci = (0..cols).min_by(|a, b| col_heights[*a].partial_cmp(&col_heights[*b]).unwrap()).unwrap();
        let x = col_x[ci];
        let y0 = col_heights[ci];
        let mid = format!("m{}", m);
        let mut y = y0 + 30.0 + PAD;
        let items: Vec<&Item> = module.items.iter().map(|i| p.item(*i)).filter(|i| i.kind != ItemKind::Method && (opts.show_tests || !i.fn_info().map(|f| f.is_test).unwrap_or(false))).collect();
        let collapsed = opts.collapsed.contains(&mid);
        if !collapsed {
            for it in &items {
                let iid = format!("{}/i{}", mid, it.id);
                let mut v = mk_node(iid.clone(), it, 1);
                v.parent = Some(mid.clone());
                v.label = crate::shorten(&v.label, 34);
                v.sublabel = crate::shorten(&v.sublabel, 40);
                v.x = x + PAD;
                v.y = y;
                v.w = COL_W - 2.0 * PAD;
                v.h = if v.sublabel.is_empty() { 26.0 } else { ROW };
                v.badges.clear();
                let methods: Vec<ItemId> = match &it.extra {
                    ItemExtra::Impl(i) => i.methods.clone(),
                    ItemExtra::Trait(t) => t.methods.clone(),
                    _ => vec![],
                };
                let expandable = !methods.is_empty();
                let expanded = expandable && opts.is_expanded(&iid, 1);
                v.expandable = expandable;
                v.expanded = expanded;
                v.container = expanded;
                if expanded {
                    let mut my = y + ROW + 4.0;
                    for mth in &methods {
                        let mi = p.item(*mth);
                        let mid2 = format!("{}/i{}", iid, mi.id);
                        let mut mv = mk_node(mid2, mi, 2);
                        mv.parent = Some(iid.clone());
                        mv.label = mi.name.clone();
                        mv.sublabel = crate::shorten(&mv.sublabel, 36);
                        mv.badges.clear();
                        mv.x = x + 2.0 * PAD + 8.0;
                        mv.y = my;
                        mv.w = COL_W - 4.0 * PAD - 8.0;
                        mv.h = ROW - 4.0;
                        g.nodes.push(mv);
                        my += ROW;
                    }
                    v.h = my - y + 4.0;
                }
                y += v.h + 6.0;
                g.nodes.push(v);
            }
        }
        let h = (y - y0).max(30.0 + PAD) + PAD / 2.0;
        if module.path == "crate" && module.file.is_none() && items.is_empty() {
            continue; // virtual workspace root: nothing to show as a block
        }
        let label = if module.path == "crate" { format!("crate {}", p.name) } else { module.path.clone() };
        g.nodes.push(VNode {
            id: mid.clone(),
            label,
            sublabel: module.file.map(|f| p.file(f).path.clone()).unwrap_or_default(),
            kind: NodeKind::Module,
            item: None,
            cfg_node: None,
            span: module.span.or_else(|| module.file.map(|f| Span { file: f, line_start: 1, col_start: 1, line_end: 1, col_end: 1 })),
            x,
            y: y0,
            w: COL_W,
            h,
            expandable: !items.is_empty(),
            expanded: !collapsed,
            badges: vec![],
            depth: 0,
            parent: None,
            container: true,
        });
        col_heights[ci] = y0 + h + GAP;
        let _ = &mut col_x;
    }
    // containers first so they are drawn behind
    g.nodes.sort_by_key(|n| n.depth);
    g.width = PAD + cols as f64 * (COL_W + GAP);
    g.height = col_heights.iter().cloned().fold(0.0, f64::max) + PAD;
    g
}

// ---------------------------------------------------------------- types

fn types(p: &Project, opts: &ViewOptions) -> Graph {
    let mut g = Graph::default();
    let type_items: Vec<&Item> = p.items.iter().filter(|i| i.is_type() && opts.in_scope(p, i.module)).collect();
    if type_items.is_empty() {
        g.note = Some("No structs, enums or traits found.".into());
        return g;
    }
    let mut present: HashSet<ItemId> = HashSet::new();
    for it in &type_items {
        let nid = format!("i{}", it.id);
        let mut v = mk_node(nid.clone(), it, 0);
        v.sublabel = match &it.extra {
            ItemExtra::Struct(s) => {
                let f: Vec<String> = s.fields.iter().take(4).map(|f| format!("{}: {}", f.name, crate::shorten(&f.ty, 14))).collect();
                let mut s2 = f.join("\n");
                if s.fields.len() > 4 {
                    s2.push_str(&format!("\n… {} more", s.fields.len() - 4));
                }
                s2
            }
            ItemExtra::Enum(e) => {
                let v: Vec<String> = e.variants.iter().take(5).map(|v| v.name.clone()).collect();
                let mut s2 = v.join(" | ");
                if e.variants.len() > 5 {
                    s2.push_str(" | …");
                }
                crate::shorten(&s2, 40)
            }
            ItemExtra::Trait(t) => {
                let m: Vec<String> = t.methods.iter().map(|m| format!("{}()", p.item(*m).name)).collect();
                crate::shorten(&m.join(" "), 40)
            }
            ItemExtra::TypeAlias { target } => format!("= {}", target),
            _ => String::new(),
        };
        let lines = v.sublabel.lines().count();
        let (w, _) = node_size(&v.label, &v.sublabel);
        v.w = w;
        v.h = 36.0 + 15.0 * lines as f64;
        v.expandable = true; // expand: show functions using this type
        v.expanded = opts.expanded.contains(&nid);
        present.insert(it.id);
        g.nodes.push(v);
    }
    for (i, r) in p.type_rels.iter().enumerate() {
        let kind = match r.kind {
            TypeRelKind::Contains => EdgeKind::Contains,
            TypeRelKind::Implements => EdgeKind::Implements,
            TypeRelKind::Supertrait => EdgeKind::Supertrait,
            TypeRelKind::Aliases => EdgeKind::Aliases,
            TypeRelKind::UsesInSignature => continue,
        };
        if present.contains(&r.from) && present.contains(&r.to) {
            g.edges.push(VEdge { id: format!("e{}", i), from: format!("i{}", r.from), to: format!("i{}", r.to), kind, label: r.label.clone(), points: vec![], backward: false });
        }
    }
    // expanded types: attach the functions that use them
    let mut extra_nodes = Vec::new();
    for r in &p.type_rels {
        if r.kind != TypeRelKind::UsesInSignature {
            continue;
        }
        let tid = format!("i{}", r.to);
        if !opts.expanded.contains(&tid) {
            continue;
        }
        let f = p.item(r.from);
        if !opts.show_tests && f.fn_info().map(|f| f.is_test).unwrap_or(false) {
            continue;
        }
        if !present.contains(&r.to) {
            continue;
        }
        let fid = format!("{}/f{}", tid, r.from);
        if g.nodes.iter().any(|n| n.id == fid) || extra_nodes.iter().any(|n: &VNode| n.id == fid) {
            continue;
        }
        let v = mk_node(fid.clone(), f, 1);
        extra_nodes.push(v);
        g.edges.push(VEdge { id: format!("u{}", g.edges.len()), from: fid, to: tid, kind: EdgeKind::Uses, label: None, points: vec![], backward: false });
    }
    g.nodes.extend(extra_nodes);
    // traits on top: reverse implements/supertrait for ranking; uses edges also point up to the type
    layered_layout(&mut g, &|e| matches!(e.kind, EdgeKind::Implements | EdgeKind::Supertrait | EdgeKind::Uses), &LayoutOptions { layer_gap: 70.0, node_gap: 30.0, max_per_layer: 7, ..Default::default() });
    g
}

// ---------------------------------------------------------------- error flow

fn error_flow(p: &Project, opts: &ViewOptions) -> Graph {
    let mut g = Graph::default();
    let interesting = |it: &Item| -> bool {
        opts.in_scope(p, it.module) && it.fn_info().map(|f| (f.returns_result || f.returns_option || f.unwraps + f.expects + f.panics > 0 || f.question_marks > 0) && (opts.show_tests || !f.is_test)).unwrap_or(false)
    };
    let mut ids: HashSet<ItemId> = p.items.iter().filter(|i| interesting(i)).map(|i| i.id).collect();
    // include callers of interesting fns (they handle or propagate)
    let callers: Vec<ItemId> = p.calls.iter().filter(|c| c.to.map(|t| ids.contains(&t)).unwrap_or(false)).map(|c| c.from).collect();
    for c in callers {
        if opts.in_scope(p, p.item(c).module) && (opts.show_tests || !p.item(c).fn_info().map(|f| f.is_test).unwrap_or(false)) {
            ids.insert(c);
        }
    }
    if ids.is_empty() {
        g.note = Some("No Result/Option-returning functions or panics found.".into());
        return g;
    }
    let mut sorted: Vec<ItemId> = ids.iter().copied().collect();
    sorted.sort();
    for id in &sorted {
        let it = p.item(*id);
        let mut v = mk_node(format!("i{}", *id), it, 0);
        let f = it.fn_info().unwrap();
        v.sublabel = if f.returns_result {
            format!("→ Result<{}, {}>", crate::shorten(f.result_ok.as_deref().unwrap_or("_"), 12), crate::shorten(f.result_err.as_deref().unwrap_or("_"), 16))
        } else if f.returns_option {
            "→ Option".into()
        } else if f.unwraps + f.expects + f.panics > 0 {
            "may panic".into()
        } else {
            "handles errors".into()
        };
        if f.unwraps + f.expects + f.panics > 0 && (f.returns_result || f.returns_option) {
            v.sublabel.push_str(" · may panic");
        }
        let (w, h) = node_size(&v.label, &v.sublabel);
        v.w = w;
        v.h = h;
        if f.unwraps + f.expects + f.panics > 0 && !f.returns_result && !f.returns_option {
            v.kind = NodeKind::Panic;
        }
        g.nodes.push(v);
    }
    // error types
    let mut err_types: HashSet<ItemId> = HashSet::new();
    for id in &sorted {
        let f = p.item(*id).fn_info().unwrap();
        if let Some(e) = &f.result_err {
            let last = e.rsplit("::").next().unwrap_or(e).split('<').next().unwrap_or(e).trim();
            if let Some(t) = p.items.iter().find(|i| i.is_type() && i.name == last) {
                err_types.insert(t.id);
                g.edges.push(VEdge { id: format!("t{}", g.edges.len()), from: format!("i{}", *id), to: format!("t{}", t.id), kind: EdgeKind::Error, label: Some("Err".into()), points: vec![], backward: false });
            }
        }
    }
    for t in &err_types {
        let mut v = mk_node(format!("t{}", *t), p.item(*t), 0);
        v.kind = NodeKind::ErrorType;
        if let ItemExtra::Enum(e) = &p.item(*t).extra {
            v.sublabel = crate::shorten(&e.variants.iter().map(|v| v.name.clone()).collect::<Vec<_>>().join(" | "), 40);
            let (w, h) = node_size(&v.label, &v.sublabel);
            v.w = w;
            v.h = h;
        }
        g.nodes.push(v);
    }
    // conversions between error types (From impls)
    for it in &p.items {
        if let ItemExtra::Impl(i) = &it.extra {
            if i.trait_name.as_deref().map(|t| t.starts_with("From<")).unwrap_or(false) {
                if let Some(target) = i.target_id.filter(|t| err_types.contains(t)) {
                    let inner = i.trait_name.as_ref().unwrap().trim_start_matches("From<").trim_end_matches('>');
                    let last = inner.rsplit("::").next().unwrap_or(inner);
                    if let Some(src) = err_types.iter().find(|e| p.item(**e).name == last) {
                        g.edges.push(VEdge { id: format!("f{}", g.edges.len()), from: format!("t{}", src), to: format!("t{}", target), kind: EdgeKind::Aliases, label: Some("From".into()), points: vec![], backward: false });
                    }
                }
            }
        }
    }
    // call edges
    let mut seen = HashSet::new();
    for c in &p.calls {
        let Some(t) = c.to else { continue };
        if !ids.contains(&c.from) || !ids.contains(&t) || c.kind == CallKind::Construct {
            continue;
        }
        if !seen.insert((c.from, t, c.propagated, c.unwrapped)) {
            continue;
        }
        let callee_fallible = p.item(t).fn_info().map(|f| f.returns_result || f.returns_option).unwrap_or(false);
        let (kind, label) = if c.propagated {
            (EdgeKind::Propagate, Some("? propagates".to_string()))
        } else if c.unwrapped {
            (EdgeKind::Unwrap, Some("unwrap → panic".to_string()))
        } else if callee_fallible {
            (EdgeKind::Handles, Some("handled".to_string()))
        } else {
            (EdgeKind::Call, None)
        };
        g.edges.push(VEdge { id: format!("e{}", g.edges.len()), from: format!("i{}", c.from), to: format!("i{}", t), kind, label, points: vec![], backward: false });
    }
    layered_layout(&mut g, &|_| false, &LayoutOptions { layer_gap: 64.0, node_gap: 30.0, max_per_layer: 7, ..Default::default() });
    g
}

/// Human-readable relationship summary for a hovered item (used by the UI tooltip).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ItemInfo {
    pub title: String,
    pub kind: String,
    pub signature: String,
    pub summary: String,
    pub doc: Option<String>,
    pub location: String,
    pub params: Vec<Param>,
    pub ret: Option<String>,
    pub facts: Vec<String>,
    pub calls: Vec<(ItemId, String)>,
    pub called_by: Vec<(ItemId, String)>,
    pub related: Vec<(ItemId, String)>,
}

pub fn item_info(p: &Project, id: ItemId) -> ItemInfo {
    let it = p.item(id);
    let mut info = ItemInfo {
        title: item_label(it),
        kind: it.kind.label().to_string(),
        signature: it.signature.clone(),
        summary: it.summary(),
        doc: it.doc.clone(),
        location: format!("{}:{}", p.file(it.span.file).path, it.span.line_start),
        ..Default::default()
    };
    if let Some(f) = it.fn_info() {
        info.params = f.params.clone();
        info.ret = f.ret.clone();
        info.facts = fn_badges(f);
        if f.self_kind != SelfKind::None {
            info.facts.insert(0, match f.self_kind { SelfKind::Ref => "&self", SelfKind::RefMut => "&mut self", SelfKind::Owned => "self", SelfKind::None => "" }.into());
        }
        info.facts.push(format!("{} lines", f.body_lines));
        if let Some(t) = &f.trait_name {
            info.facts.push(format!("trait {}", t));
        }
        let mut seen = HashSet::new();
        for c in p.callees(id) {
            if let Some(t) = c.to {
                if seen.insert(t) {
                    info.calls.push((t, item_label(p.item(t))));
                }
            }
        }
        seen.clear();
        for c in p.callers(id) {
            if seen.insert(c.from) {
                info.called_by.push((c.from, item_label(p.item(c.from))));
            }
        }
        // trait method → implementations
        if f.impl_id.is_none() {
            if let Some(tid) = f.trait_id {
                for other in p.items.iter() {
                    if let Some(of) = other.fn_info() {
                        if of.trait_id == Some(tid) && of.impl_id.is_some() && other.name == it.name {
                            info.related.push((other.id, format!("impl: {}", item_label(other))));
                        }
                    }
                }
            }
        }
        if let Some(o) = f.owner_type {
            info.related.push((o, format!("on {}", p.item(o).name)));
        }
    }
    match &it.extra {
        ItemExtra::Struct(s) => {
            info.facts.push(format!("{} fields", s.fields.len()));
            if !s.derives.is_empty() {
                info.facts.push(format!("derive {}", s.derives.join(", ")));
            }
            info.params = s.fields.iter().map(|f| Param { name: f.name.clone(), ty: f.ty.clone() }).collect();
        }
        ItemExtra::Enum(e) => {
            info.facts.push(format!("{} variants", e.variants.len()));
            info.params = e.variants.iter().map(|v| Param { name: v.name.clone(), ty: v.fields.iter().map(|f| f.ty.clone()).collect::<Vec<_>>().join(", ") }).collect();
        }
        ItemExtra::Trait(t) => {
            info.params = t.methods.iter().map(|m| Param { name: p.item(*m).name.clone(), ty: p.item(*m).signature.clone() }).collect();
        }
        _ => {}
    }
    if it.is_type() {
        for r in &p.type_rels {
            if r.from == id {
                let l = match r.kind {
                    TypeRelKind::Contains => format!("contains {}", p.item(r.to).name),
                    TypeRelKind::Implements => format!("implements {}", p.item(r.to).name),
                    TypeRelKind::Supertrait => format!("requires {}", p.item(r.to).name),
                    TypeRelKind::Aliases => format!("alias of {}", p.item(r.to).name),
                    TypeRelKind::UsesInSignature => continue,
                };
                info.related.push((r.to, l));
            } else if r.to == id {
                let l = match r.kind {
                    TypeRelKind::Contains => format!("field of {}", p.item(r.from).name),
                    TypeRelKind::Implements => format!("implemented by {}", p.item(r.from).name),
                    TypeRelKind::Supertrait => format!("supertrait of {}", p.item(r.from).name),
                    TypeRelKind::Aliases => format!("aliased as {}", p.item(r.from).name),
                    TypeRelKind::UsesInSignature => format!("used by {}", item_label(p.item(r.from))),
                };
                info.related.push((r.from, l));
            }
        }
        for it2 in &p.items {
            if let ItemExtra::Impl(i) = &it2.extra {
                if i.target_id == Some(id) {
                    for m in &i.methods {
                        info.related.push((*m, format!("method {}", p.item(*m).name)));
                    }
                }
            }
        }
    }
    info
}
