//! Pure-Rust graph layout: a layered (Sugiyama-style) layout for general
//! directed graphs and a tidy tree layout for hierarchies. Produces
//! positions + edge polylines; rendering is up to the UI.

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn cx(&self) -> f64 {
        self.x + self.w / 2.0
    }
    pub fn cy(&self) -> f64 {
        self.y + self.h / 2.0
    }
    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }
    pub fn right(&self) -> f64 {
        self.x + self.w
    }
}

#[derive(Debug, Clone)]
pub struct LayoutNode {
    pub id: String,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone)]
pub struct LayoutEdge {
    pub from: String,
    pub to: String,
    /// Reverse this edge when computing ranks (e.g. `implements` edges so traits sit on top).
    pub reverse_rank: bool,
}

#[derive(Debug, Clone)]
pub struct LayoutOptions {
    pub layer_gap: f64,
    pub node_gap: f64,
    pub margin: f64,
    /// left-to-right instead of top-to-bottom
    pub horizontal: bool,
    /// wider layers are split into several stacked ranks
    pub max_per_layer: usize,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self { layer_gap: 70.0, node_gap: 28.0, margin: 24.0, horizontal: false, max_per_layer: 8 }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LayoutResult {
    pub rects: HashMap<String, Rect>,
    /// Polyline for every edge, keyed by (from, to, index).
    pub edges: Vec<EdgeRoute>,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone)]
pub struct EdgeRoute {
    pub from: String,
    pub to: String,
    pub points: Vec<(f64, f64)>,
    /// The edge goes against the layering (a loop back or an upward edge).
    pub backward: bool,
}

/// Layered layout for a directed graph. Disconnected components are laid out
/// separately and packed into rows so wide graphs stay roughly square.
pub fn layered(nodes: &[LayoutNode], edges: &[LayoutEdge], opts: &LayoutOptions) -> LayoutResult {
    let n = nodes.len();
    if n == 0 {
        return LayoutResult::default();
    }
    // ---- connected components (undirected)
    let index: HashMap<&str, usize> = nodes.iter().enumerate().map(|(i, nd)| (nd.id.as_str(), i)).collect();
    let mut comp = vec![usize::MAX; n];
    let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
    for e in edges {
        if let (Some(&a), Some(&b)) = (index.get(e.from.as_str()), index.get(e.to.as_str())) {
            adj[a].push(b);
            adj[b].push(a);
        }
    }
    let mut ncomp = 0;
    for start in 0..n {
        if comp[start] != usize::MAX {
            continue;
        }
        let mut stack = vec![start];
        comp[start] = ncomp;
        while let Some(u) = stack.pop() {
            for &v in &adj[u] {
                if comp[v] == usize::MAX {
                    comp[v] = ncomp;
                    stack.push(v);
                }
            }
        }
        ncomp += 1;
    }
    if ncomp > 1 {
        let mut results: Vec<LayoutResult> = Vec::new();
        for c in 0..ncomp {
            let sub_nodes: Vec<LayoutNode> = nodes.iter().enumerate().filter(|(i, _)| comp[*i] == c).map(|(_, nd)| nd.clone()).collect();
            let ids: HashSet<&str> = sub_nodes.iter().map(|nd| nd.id.as_str()).collect();
            let sub_edges: Vec<LayoutEdge> = edges.iter().filter(|e| ids.contains(e.from.as_str()) && ids.contains(e.to.as_str())).cloned().collect();
            results.push(layered_component(&sub_nodes, &sub_edges, opts));
        }
        // pack: biggest first, rows wrapped at a target width
        let mut order: Vec<usize> = (0..ncomp).collect();
        order.sort_by(|a, b| (results[*b].height * results[*b].width).partial_cmp(&(results[*a].height * results[*a].width)).unwrap());
        let total_area: f64 = results.iter().map(|r| r.width * r.height).sum();
        let target_w = (total_area.sqrt() * 1.4).max(1200.0).max(results.iter().map(|r| r.width).fold(0.0, f64::max));
        let mut out = LayoutResult::default();
        let (mut x, mut y, mut row_h) = (0.0, 0.0, 0.0f64);
        for c in order {
            let r = &results[c];
            if x > 0.0 && x + r.width > target_w {
                x = 0.0;
                y += row_h + opts.layer_gap;
                row_h = 0.0;
            }
            for (id, rect) in &r.rects {
                out.rects.insert(id.clone(), Rect { x: rect.x + x, y: rect.y + y, w: rect.w, h: rect.h });
            }
            for e in &r.edges {
                out.edges.push(EdgeRoute { from: e.from.clone(), to: e.to.clone(), points: e.points.iter().map(|p| (p.0 + x, p.1 + y)).collect(), backward: e.backward });
            }
            out.width = out.width.max(x + r.width);
            out.height = out.height.max(y + r.height);
            row_h = row_h.max(r.height);
            x += r.width + opts.node_gap;
        }
        return out;
    }
    layered_component(nodes, edges, opts)
}

fn layered_component(nodes: &[LayoutNode], edges: &[LayoutEdge], opts: &LayoutOptions) -> LayoutResult {
    let n = nodes.len();
    let index: HashMap<&str, usize> = nodes.iter().enumerate().map(|(i, nd)| (nd.id.as_str(), i)).collect();
    // ranking edges (deduplicated, self-loops removed)
    let mut rank_edges: Vec<(usize, usize)> = Vec::new();
    let mut seen = HashSet::new();
    for e in edges {
        let (Some(&a), Some(&b)) = (index.get(e.from.as_str()), index.get(e.to.as_str())) else { continue };
        if a == b {
            continue;
        }
        let (a, b) = if e.reverse_rank { (b, a) } else { (a, b) };
        if seen.insert((a, b)) {
            rank_edges.push((a, b));
        }
    }
    // cycle removal: DFS, reverse back edges
    let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
    for &(a, b) in &rank_edges {
        adj[a].push(b);
    }
    let mut state = vec![0u8; n]; // 0 unvisited 1 in-stack 2 done
    let mut reversed: HashSet<(usize, usize)> = HashSet::new();
    fn dfs(u: usize, adj: &Vec<Vec<usize>>, state: &mut Vec<u8>, reversed: &mut HashSet<(usize, usize)>) {
        state[u] = 1;
        for &v in &adj[u] {
            if state[v] == 1 {
                reversed.insert((u, v));
            } else if state[v] == 0 {
                dfs(v, adj, state, reversed);
            }
        }
        state[u] = 2;
    }
    // start from sources first (nodes without incoming edges) so the natural root ends up on top
    let mut indeg = vec![0usize; n];
    for &(_, b) in &rank_edges {
        indeg[b] += 1;
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| (indeg[i], i));
    for u in order {
        if state[u] == 0 {
            dfs(u, &adj, &mut state, &mut reversed);
        }
    }
    let dag: Vec<(usize, usize)> = rank_edges.iter().map(|&(a, b)| if reversed.contains(&(a, b)) { (b, a) } else { (a, b) }).collect();
    // longest path layering
    let mut preds: Vec<Vec<usize>> = vec![vec![]; n];
    for &(a, b) in &dag {
        preds[b].push(a);
    }
    let mut rank = vec![usize::MAX; n];
    fn rank_of(u: usize, preds: &Vec<Vec<usize>>, rank: &mut Vec<usize>) -> usize {
        if rank[u] != usize::MAX {
            return rank[u];
        }
        rank[u] = 0; // guard
        let r = preds[u].iter().map(|&p| rank_of(p, preds, rank) + 1).max().unwrap_or(0);
        rank[u] = r;
        r
    }
    for u in 0..n {
        rank_of(u, &preds, &mut rank);
    }
    // ---- wrap very wide layers: split them into several stacked ranks so the graph grows down, not sideways
    let max_per_layer = opts.max_per_layer.max(1);
    {
        let max_rank = *rank.iter().max().unwrap_or(&0);
        let mut by_rank: Vec<Vec<usize>> = vec![vec![]; max_rank + 1];
        for u in 0..n {
            by_rank[rank[u]].push(u);
        }
        let mut new_rank = vec![0usize; n];
        let mut next = 0usize;
        for layer in &by_rank {
            if layer.is_empty() {
                continue;
            }
            let rows = layer.len().div_ceil(max_per_layer);
            // keep nodes with more connections in the first row so edges stay short
            let mut ordered = layer.clone();
            ordered.sort_by_key(|&u| std::cmp::Reverse(preds[u].len() + dag.iter().filter(|(a, _)| *a == u).count()));
            for (i, &u) in ordered.iter().enumerate() {
                new_rank[u] = next + i / max_per_layer;
            }
            next += rows;
        }
        rank = new_rank;
    }
    // ---- virtual nodes: split edges spanning more than one layer so ordering/routing can avoid nodes
    // all nodes (real + dummy) live in `vn`; index < n are real.
    struct VNode {
        w: f64,
        h: f64,
        rank: usize,
    }
    let mut vn: Vec<VNode> = (0..n).map(|u| VNode { w: nodes[u].w, h: nodes[u].h, rank: rank[u] }).collect();
    let dummy_w = 12.0;
    // per original dag edge: chain of virtual node ids (a, d1, d2, ..., b)
    let mut chains: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    let mut vpreds: Vec<Vec<usize>> = vec![vec![]; n];
    let mut vsuccs: Vec<Vec<usize>> = vec![vec![]; n];
    for &(a, b) in &dag {
        let mut chain = vec![a];
        let (ra, rb) = (rank[a], rank[b]);
        if rb > ra + 1 {
            for r in ra + 1..rb {
                let id = vn.len();
                vn.push(VNode { w: dummy_w, h: 0.0, rank: r });
                vpreds.push(vec![]);
                vsuccs.push(vec![]);
                chain.push(id);
            }
        }
        chain.push(b);
        for w in chain.windows(2) {
            vsuccs[w[0]].push(w[1]);
            vpreds[w[1]].push(w[0]);
        }
        chains.insert((a, b), chain);
    }
    let vn_len = vn.len();
    let max_rank = vn.iter().map(|v| v.rank).max().unwrap_or(0);
    let mut layers: Vec<Vec<usize>> = vec![vec![]; max_rank + 1];
    for u in 0..vn_len {
        layers[vn[u].rank].push(u);
    }
    // ordering: barycenter sweeps
    let mut pos: Vec<f64> = vec![0.0; vn_len];
    for layer in &layers {
        for (i, &u) in layer.iter().enumerate() {
            pos[u] = i as f64;
        }
    }
    for sweep in 0..8 {
        let down = sweep % 2 == 0;
        let idxs: Vec<usize> = if down { (1..layers.len()).collect() } else { (0..layers.len().saturating_sub(1)).rev().collect() };
        for li in idxs {
            let mut keyed: Vec<(f64, usize)> = layers[li]
                .iter()
                .map(|&u| {
                    let nb: &Vec<usize> = if down { &vpreds[u] } else { &vsuccs[u] };
                    let bc = if nb.is_empty() { pos[u] } else { nb.iter().map(|&v| pos[v]).sum::<f64>() / nb.len() as f64 };
                    (bc, u)
                })
                .collect();
            keyed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.cmp(&b.1)));
            layers[li] = keyed.iter().map(|k| k.1).collect();
            for (i, &u) in layers[li].iter().enumerate() {
                pos[u] = i as f64;
            }
        }
    }
    // coordinates
    let (gap_main, gap_cross) = (opts.layer_gap, opts.node_gap);
    let size_cross = |u: usize| if opts.horizontal { vn[u].h.max(if u >= n { dummy_w } else { 0.0 }) } else { vn[u].w };
    let size_main = |u: usize| if opts.horizontal { vn[u].w } else { vn[u].h };
    let mut cross: Vec<f64> = vec![0.0; vn_len];
    let mut layer_widths: Vec<f64> = Vec::new();
    for layer in &layers {
        let total: f64 = layer.iter().map(|&u| size_cross(u)).sum::<f64>() + gap_cross * (layer.len().saturating_sub(1)) as f64;
        layer_widths.push(total);
    }
    let max_width = layer_widths.iter().cloned().fold(0.0, f64::max);
    for (li, layer) in layers.iter().enumerate() {
        let mut x = (max_width - layer_widths[li]) / 2.0;
        for &u in layer {
            cross[u] = x;
            x += size_cross(u) + gap_cross;
        }
    }
    // relaxation: move nodes toward the mean of their neighbours while keeping order and gaps
    for _ in 0..12 {
        for layer in &layers {
            let mut desired: Vec<(usize, f64)> = layer
                .iter()
                .map(|&u| {
                    let mut nb: Vec<usize> = vpreds[u].clone();
                    nb.extend(vsuccs[u].iter().cloned());
                    let d = if nb.is_empty() {
                        cross[u]
                    } else {
                        nb.iter().map(|&v| cross[v] + size_cross(v) / 2.0).sum::<f64>() / nb.len() as f64 - size_cross(u) / 2.0
                    };
                    (u, d)
                })
                .collect();
            let mut min_x = f64::NEG_INFINITY;
            for (u, d) in desired.iter_mut() {
                *d = d.max(min_x);
                min_x = *d + size_cross(*u) + gap_cross;
            }
            let mut max_x = f64::INFINITY;
            for (u, d) in desired.iter_mut().rev() {
                *d = d.min(max_x - size_cross(*u));
                max_x = *d - gap_cross;
            }
            for (u, d) in desired {
                cross[u] = (cross[u] + d) / 2.0;
            }
        }
    }
    let min_cross = cross.iter().cloned().fold(f64::INFINITY, f64::min);
    for c in cross.iter_mut() {
        *c -= min_cross;
        *c += opts.margin;
    }
    // main axis
    let mut layer_main: Vec<f64> = Vec::new();
    let mut layer_size: Vec<f64> = Vec::new();
    let mut y = opts.margin;
    for layer in &layers {
        layer_main.push(y);
        let h = layer.iter().map(|&u| size_main(u)).fold(0.0, f64::max);
        layer_size.push(h);
        y += h + gap_main;
    }
    // rect for every virtual node (dummies get a zero-height rect centred in the layer)
    let mut vrects: Vec<Rect> = Vec::with_capacity(vn_len);
    for u in 0..vn_len {
        let li = vn[u].rank;
        let h = layer_size[li];
        let r = if opts.horizontal {
            Rect { x: layer_main[li] + (h - vn[u].w) / 2.0, y: cross[u], w: vn[u].w, h: size_cross(u) }
        } else {
            Rect { x: cross[u], y: layer_main[li] + (h - vn[u].h) / 2.0, w: vn[u].w, h: vn[u].h }
        };
        vrects.push(r);
    }
    let mut rects = HashMap::new();
    let mut width: f64 = 0.0;
    let mut height: f64 = 0.0;
    for u in 0..n {
        let r = vrects[u];
        width = width.max(r.right());
        height = height.max(r.bottom());
        rects.insert(nodes[u].id.clone(), r);
    }
    // edge routes
    let mut routes = Vec::new();
    let mut back_count = 0usize;
    for e in edges {
        let (Some(&a), Some(&b)) = (index.get(e.from.as_str()), index.get(e.to.as_str())) else { continue };
        let (ra, rb) = (vrects[a], vrects[b]);
        if a == b {
            let (x, y) = (ra.right(), ra.cy());
            routes.push(EdgeRoute { from: e.from.clone(), to: e.to.clone(), points: vec![(x, y - 8.0), (x + 22.0, y - 14.0), (x + 22.0, y + 14.0), (x, y + 8.0)], backward: true });
            continue;
        }
        // which dag edge does this correspond to?
        let (ka, kb) = if e.reverse_rank { (b, a) } else { (a, b) };
        let (ka, kb) = if reversed.contains(&(ka, kb)) { (kb, ka) } else { (ka, kb) };
        let chain = chains.get(&(ka, kb)).cloned().unwrap_or_else(|| vec![ka, kb]);
        let goes_down = chain.first() == Some(&a); // the drawn edge follows the dag direction
        let mut pts: Vec<(f64, f64)> = Vec::new();
        let ordered: Vec<usize> = if goes_down { chain.clone() } else { chain.iter().rev().cloned().collect() };
        let is_back = ordered.first() == Some(&a) && (if opts.horizontal { rb.x < ra.x } else { rb.y < ra.y }) && !e.reverse_rank && !reversed.contains(&(a, b)) && ordered.len() == 2;
        if is_back {
            // genuine back edge that was reversed to break a cycle: route around the side
            back_count += 1;
            let off = 18.0 + (back_count % 5) as f64 * 8.0;
            let pts = if opts.horizontal {
                let ymax = ra.bottom().max(rb.bottom()) + off;
                vec![(ra.cx(), ra.bottom()), (ra.cx(), ymax), (rb.cx(), ymax), (rb.cx(), rb.bottom())]
            } else {
                let xmax = ra.right().max(rb.right()) + off;
                vec![(ra.right(), ra.cy()), (xmax, ra.cy()), (xmax, rb.cy()), (rb.right(), rb.cy())]
            };
            width = width.max(pts.iter().map(|p| p.0).fold(0.0, f64::max));
            height = height.max(pts.iter().map(|p| p.1).fold(0.0, f64::max));
            routes.push(EdgeRoute { from: e.from.clone(), to: e.to.clone(), points: pts, backward: true });
            continue;
        }
        let down = if opts.horizontal { rb.x >= ra.x } else { rb.y >= ra.y };
        for (i, &u) in ordered.iter().enumerate() {
            let r = vrects[u];
            let p = if i == 0 {
                if opts.horizontal { if down { (r.right(), r.cy()) } else { (r.x, r.cy()) } } else if down { (r.cx(), r.bottom()) } else { (r.cx(), r.y) }
            } else if i + 1 == ordered.len() {
                if opts.horizontal { if down { (r.x, r.cy()) } else { (r.right(), r.cy()) } } else if down { (r.cx(), r.y) } else { (r.cx(), r.bottom()) }
            } else {
                (r.cx(), r.cy())
            };
            pts.push(p);
        }
        // a cycle-broken edge between adjacent layers still points "up": mark backward so the UI curves it
        let backward = reversed.contains(&(a, b)) && ordered.len() == 2;
        routes.push(EdgeRoute { from: e.from.clone(), to: e.to.clone(), points: pts, backward });
    }
    LayoutResult { rects, edges: routes, width: width + opts.margin, height: height + opts.margin }
}

/// A tree node for the tidy tree layout.
pub struct TreeNode {
    pub id: String,
    pub w: f64,
    pub h: f64,
    pub children: Vec<TreeNode>,
}

/// Tidy tree layout (top-down). Children are centered under their parent.
pub fn tree(root: &TreeNode, opts: &LayoutOptions) -> LayoutResult {
    fn subtree_width(t: &TreeNode, gap: f64, memo: &mut HashMap<String, f64>) -> f64 {
        if t.children.is_empty() {
            memo.insert(t.id.clone(), t.w);
            return t.w;
        }
        let sum: f64 = t.children.iter().map(|c| subtree_width(c, gap, memo)).sum::<f64>() + gap * (t.children.len() - 1) as f64;
        let w = sum.max(t.w);
        memo.insert(t.id.clone(), w);
        w
    }
    fn place(t: &TreeNode, x: f64, y: f64, gap: f64, layer_gap: f64, memo: &HashMap<String, f64>, out: &mut LayoutResult) {
        let sw = memo[&t.id];
        let nx = x + (sw - t.w) / 2.0;
        out.rects.insert(t.id.clone(), Rect { x: nx, y, w: t.w, h: t.h });
        out.width = out.width.max(nx + t.w);
        out.height = out.height.max(y + t.h);
        let mut cx = x;
        let child_y = y + t.h + layer_gap;
        for c in &t.children {
            let cw = memo[&c.id];
            place(c, cx, child_y, gap, layer_gap, memo, out);
            let cr = out.rects[&c.id];
            out.edges.push(EdgeRoute { from: t.id.clone(), to: c.id.clone(), points: vec![(nx + t.w / 2.0, y + t.h), (cr.cx(), cr.y)], backward: false });
            cx += cw + gap;
        }
    }
    let mut memo = HashMap::new();
    subtree_width(root, opts.node_gap, &mut memo);
    let mut out = LayoutResult::default();
    place(root, opts.margin, opts.margin, opts.node_gap, opts.layer_gap, &memo, &mut out);
    out.width += opts.margin;
    out.height += opts.margin;
    out
}

/// Width for a text label in a node box (approximate monospace-ish metrics).
pub fn text_width(s: &str, font_px: f64) -> f64 {
    s.lines().map(|l| l.chars().count()).max().unwrap_or(0) as f64 * font_px * 0.6
}
