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

#[derive(Debug, Clone, Default)]
pub struct LayoutEdge {
    pub from: String,
    pub to: String,
    /// Reverse this edge when computing ranks (e.g. `implements` edges so traits sit on top).
    pub reverse_rank: bool,
    /// Counts for ranking only: no dummy nodes, no pull on the ordering, routed straight
    /// (edges that are hidden most of the time, like architecture hub arrows).
    pub rank_only: bool,
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
    /// after ordering, centre every layer on one axis: a compact tower instead of rows drifting
    /// sideways (good for small layers, like modules)
    pub center_layers: bool,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self { layer_gap: 70.0, node_gap: 28.0, margin: 24.0, horizontal: false, max_per_layer: 8, center_layers: false }
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
    // ranking edges (deduplicated, self-loops removed); `light` ones only rank
    let mut rank_edges: Vec<(usize, usize)> = Vec::new();
    let mut seen = HashSet::new();
    let mut light: HashSet<(usize, usize)> = HashSet::new();
    let mut heavy: HashSet<(usize, usize)> = HashSet::new();
    for e in edges {
        let (Some(&a), Some(&b)) = (index.get(e.from.as_str()), index.get(e.to.as_str())) else { continue };
        if a == b {
            continue;
        }
        let (a, b) = if e.reverse_rank { (b, a) } else { (a, b) };
        if e.rank_only {
            light.insert((a, b));
        } else {
            heavy.insert((a, b));
        }
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
    for (i, &(a, b)) in dag.iter().enumerate() {
        let orig = rank_edges[i];
        if light.contains(&orig) && !heavy.contains(&orig) {
            // rank-only: a straight line later, no say in the ordering
            chains.insert((a, b), vec![a, b]);
            continue;
        }
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
    // sweeps keep the best ordering seen (a sweep can make things worse), then adjacent swaps
    // (transpose) remove the crossings the barycenters leave
    let mut best = layers.clone();
    let mut best_x = crossings(&layers, &vsuccs, &pos);
    for sweep in 0..24 {
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
        let x = crossings(&layers, &vsuccs, &pos);
        if x < best_x {
            best_x = x;
            best = layers.clone();
        }
        if best_x == 0 {
            break;
        }
    }
    layers = best;
    for layer in &layers {
        for (i, &u) in layer.iter().enumerate() {
            pos[u] = i as f64;
        }
    }
    transpose(&mut layers, &vpreds, &vsuccs, &mut pos);
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
    if opts.center_layers {
        for layer in &layers {
            let (lo, hi) = layer.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &u| (lo.min(cross[u]), hi.max(cross[u] + size_cross(u))));
            if lo.is_finite() {
                let shift = -(lo + hi) / 2.0;
                for &u in layer {
                    cross[u] += shift;
                }
            }
        }
    }
    // straighten long edges: each dummy slides toward the line between its neighbours, inside
    // the room its layer leaves it (boxes stay put, so the centring above is kept)
    for _ in 0..16 {
        for layer in &layers {
            for (i, &u) in layer.iter().enumerate() {
                if u < n || vpreds[u].is_empty() || vsuccs[u].is_empty() {
                    continue;
                }
                let centre = |v: usize| cross[v] + size_cross(v) / 2.0;
                let want = (centre(vpreds[u][0]) + centre(vsuccs[u][0])) / 2.0 - size_cross(u) / 2.0;
                let lo = if i > 0 { let l = layer[i - 1]; cross[l] + size_cross(l) + gap_cross } else { f64::NEG_INFINITY };
                let hi = if i + 1 < layer.len() { cross[layer[i + 1]] - gap_cross - size_cross(u) } else { f64::INFINITY };
                if lo <= hi {
                    cross[u] = want.clamp(lo, hi);
                }
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
            if i == 0 {
                pts.push(if opts.horizontal { if down { (r.right(), r.cy()) } else { (r.x, r.cy()) } } else if down { (r.cx(), r.bottom()) } else { (r.cx(), r.y) });
            } else if i + 1 == ordered.len() {
                pts.push(if opts.horizontal { if down { (r.x, r.cy()) } else { (r.right(), r.cy()) } } else if down { (r.cx(), r.y) } else { (r.cx(), r.bottom()) });
            } else {
                // a dummy: cross its whole layer band straight, so the curve bends only in the gaps
                // between layers and never cuts a corner through a neighbouring box
                let li = vn[u].rank;
                let (lo, hi) = (layer_main[li], layer_main[li] + layer_size[li]);
                let (enter, leave) = if down { (lo, hi) } else { (hi, lo) };
                if opts.horizontal {
                    pts.extend([(enter, r.cy()), (leave, r.cy())]);
                } else {
                    pts.extend([(r.cx(), enter), (r.cx(), leave)]);
                }
            }
        }
        // a cycle-broken edge between adjacent layers still points "up": mark backward so the UI curves it
        let backward = reversed.contains(&(a, b)) && ordered.len() == 2;
        routes.push(EdgeRoute { from: e.from.clone(), to: e.to.clone(), points: pts, backward });
    }
    spread_ports(&mut routes, &vrects[..n], opts.horizontal);
    LayoutResult { rects, edges: routes, width: width + opts.margin, height: height + opts.margin }
}

/// Edge crossings between every pair of neighbouring layers (edges only join neighbours).
fn crossings(layers: &[Vec<usize>], succs: &[Vec<usize>], pos: &[f64]) -> usize {
    let mut total = 0;
    for layer in layers {
        let mut ends: Vec<(f64, f64)> = layer.iter().flat_map(|&u| succs[u].iter().map(move |&v| (pos[u], pos[v]))).collect();
        ends.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for i in 0..ends.len() {
            for j in i + 1..ends.len() {
                if ends[i].0 < ends[j].0 && ends[j].1 < ends[i].1 {
                    total += 1;
                }
            }
        }
    }
    total
}

/// Crossings between the edges of `u` and those of `v` when `u` sits left of `v`.
fn pair_crossings(u: usize, v: usize, preds: &[Vec<usize>], succs: &[Vec<usize>], pos: &[f64]) -> usize {
    let side = |nb: &[Vec<usize>]| nb[u].iter().map(|&a| nb[v].iter().filter(|&&b| pos[b] < pos[a]).count()).sum::<usize>();
    side(preds) + side(succs)
}

/// Swap neighbours in a layer while that removes crossings.
fn transpose(layers: &mut [Vec<usize>], preds: &[Vec<usize>], succs: &[Vec<usize>], pos: &mut [f64]) {
    for _ in 0..12 {
        let mut improved = false;
        for layer in layers.iter_mut() {
            for i in 0..layer.len().saturating_sub(1) {
                let (u, v) = (layer[i], layer[i + 1]);
                if pair_crossings(u, v, preds, succs, pos) > pair_crossings(v, u, preds, succs, pos) {
                    layer.swap(i, i + 1);
                    pos[u] = (i + 1) as f64;
                    pos[v] = i as f64;
                    improved = true;
                }
            }
        }
        if !improved {
            break;
        }
    }
}

/// Spread the ends of the edges that meet one side of a box along that side, ordered by where
/// each edge comes from, so arrows no longer pile into the box centre and cross right there.
fn spread_ports(routes: &mut [EdgeRoute], rects: &[Rect], horizontal: bool) {
    // (route index, first end?) grouped by (node, side)
    let mut sides: HashMap<(usize, bool), Vec<(usize, bool, f64)>> = HashMap::new();
    let find = |p: (f64, f64)| -> Option<(usize, bool)> {
        rects.iter().position(|r| {
            if horizontal {
                ((p.0 - r.x).abs() < 0.5 || (p.0 - r.right()).abs() < 0.5) && (p.1 - r.cy()).abs() < 0.5
            } else {
                ((p.1 - r.y).abs() < 0.5 || (p.1 - r.bottom()).abs() < 0.5) && (p.0 - r.cx()).abs() < 0.5
            }
        })
        .map(|i| {
            let r = rects[i];
            (i, if horizontal { (p.0 - r.right()).abs() < 0.5 } else { (p.1 - r.bottom()).abs() < 0.5 })
        })
    };
    for (ri, route) in routes.iter().enumerate() {
        if route.backward || route.points.len() < 2 {
            continue;
        }
        let n = route.points.len();
        for (first, p, q) in [(true, route.points[0], route.points[1]), (false, route.points[n - 1], route.points[n - 2])] {
            if let Some(key) = find(p) {
                // where the edge heads: its next point, or for a direct edge the far end
                let k = if horizontal { q.1 } else { q.0 };
                sides.entry(key).or_default().push((ri, first, k));
            }
        }
    }
    for ((ni, _), mut ends) in sides {
        if ends.len() < 2 {
            continue;
        }
        let r = rects[ni];
        ends.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap().then(a.0.cmp(&b.0)));
        let len = if horizontal { r.h } else { r.w };
        let step = (len * 0.8 / ends.len() as f64).min(18.0);
        let start = -(step * (ends.len() - 1) as f64) / 2.0;
        for (i, (ri, first, _)) in ends.into_iter().enumerate() {
            let off = start + step * i as f64;
            let pts = &mut routes[ri].points;
            let p = if first { &mut pts[0] } else { pts.last_mut().unwrap() };
            if horizontal {
                p.1 = r.cy() + off;
            } else {
                p.0 = r.cx() + off;
            }
        }
    }
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

/// One piece of a drawn edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    Line((f64, f64), (f64, f64)),
    Quad((f64, f64), (f64, f64), (f64, f64)),
    Cubic((f64, f64), (f64, f64), (f64, f64), (f64, f64)),
}

impl Seg {
    pub fn at(&self, t: f64) -> (f64, f64) {
        let u = 1.0 - t;
        match *self {
            Seg::Line(a, b) => (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t),
            Seg::Quad(a, c, b) => (u * u * a.0 + 2.0 * u * t * c.0 + t * t * b.0, u * u * a.1 + 2.0 * u * t * c.1 + t * t * b.1),
            Seg::Cubic(a, c1, c2, b) => (
                u * u * u * a.0 + 3.0 * u * u * t * c1.0 + 3.0 * u * t * t * c2.0 + t * t * t * b.0,
                u * u * u * a.1 + 3.0 * u * u * t * c1.1 + 3.0 * u * t * t * c2.1 + t * t * t * b.1,
            ),
        }
    }
    /// The point the curve comes from as it arrives at its end (for the arrow head).
    fn before_end(&self) -> (f64, f64) {
        match *self {
            Seg::Line(a, _) => a,
            Seg::Quad(_, c, _) => c,
            Seg::Cubic(a, _, c2, b) => if (c2.0 - b.0).abs() + (c2.1 - b.1).abs() < 1e-6 { a } else { c2 },
        }
    }
}

/// How an edge route is drawn. Layered routes bend only between layers: every step is a cubic
/// whose ends follow the layout axis (`sideways` = left to right), a straight line when aligned.
/// `backward` routes (around the side) keep their corners; three points make one soft curve.
pub fn edge_segments(points: &[(f64, f64)], backward: bool, sideways: bool) -> Vec<Seg> {
    let n = points.len();
    if n < 2 {
        return vec![];
    }
    if backward && n > 2 {
        // straight runs, each corner rounded (radius up to 8, at most half of either run)
        let toward = |p: (f64, f64), q: (f64, f64), d: f64| {
            let len = ((q.0 - p.0).powi(2) + (q.1 - p.1).powi(2)).sqrt().max(1e-6);
            (p.0 + (q.0 - p.0) * d / len, p.1 + (q.1 - p.1) * d / len)
        };
        let dist = |p: (f64, f64), q: (f64, f64)| ((q.0 - p.0).powi(2) + (q.1 - p.1).powi(2)).sqrt();
        let mut segs = Vec::new();
        let mut cur = points[0];
        for i in 1..n - 1 {
            let (c, next) = (points[i], points[i + 1]);
            let r = 8.0f64.min(dist(points[i - 1], c) / 2.0).min(dist(c, next) / 2.0);
            let (a, b) = (toward(c, cur, r), toward(c, next, r));
            segs.push(Seg::Line(cur, a));
            segs.push(Seg::Quad(a, c, b));
            cur = b;
        }
        segs.push(Seg::Line(cur, points[n - 1]));
        return segs;
    }
    if n == 3 {
        return vec![Seg::Quad(points[0], points[1], points[2])];
    }
    points
        .windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            if sideways {
                if (a.1 - b.1).abs() < 0.5 {
                    return Seg::Line(a, b);
                }
                let dir = if b.0 >= a.0 { 1.0 } else { -1.0 };
                let d = (b.0 - a.0).abs().max(20.0) * 0.5;
                Seg::Cubic(a, (a.0 + dir * d, a.1), (b.0 - dir * d, b.1), b)
            } else {
                if (a.0 - b.0).abs() < 0.5 {
                    return Seg::Line(a, b);
                }
                let dir = if b.1 >= a.1 { 1.0 } else { -1.0 };
                let d = (b.1 - a.1).abs().max(20.0) * 0.5;
                Seg::Cubic(a, (a.0, a.1 + dir * d), (b.0, b.1 - dir * d), b)
            }
        })
        .collect()
}

/// SVG path data of [`edge_segments`].
pub fn edge_path(segs: &[Seg]) -> String {
    let mut d = String::new();
    for (i, s) in segs.iter().enumerate() {
        let start = match *s {
            Seg::Line(a, _) | Seg::Quad(a, _, _) | Seg::Cubic(a, _, _, _) => a,
        };
        if i == 0 {
            d.push_str(&format!("M{:.1},{:.1}", start.0, start.1));
        }
        match *s {
            Seg::Line(_, b) => d.push_str(&format!(" L{:.1},{:.1}", b.0, b.1)),
            Seg::Quad(_, c, b) => d.push_str(&format!(" Q{:.1},{:.1} {:.1},{:.1}", c.0, c.1, b.0, b.1)),
            Seg::Cubic(_, c1, c2, b) => d.push_str(&format!(" C{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}", c1.0, c1.1, c2.0, c2.1, b.0, b.1)),
        }
    }
    d
}

/// End point of the edge and the direction it arrives from (unit vector), for an arrow head.
pub fn edge_end(segs: &[Seg]) -> Option<((f64, f64), (f64, f64))> {
    let s = segs.last()?;
    let end = s.at(1.0);
    let from = s.before_end();
    let (dx, dy) = (end.0 - from.0, end.1 - from.1);
    let len = (dx * dx + dy * dy).sqrt();
    (len > 1e-6).then(|| (end, (dx / len, dy / len)))
}

/// Points along the drawn edge, about every `step` pixels.
pub fn edge_samples(segs: &[Seg], step: f64) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for s in segs {
        let (a, b) = (s.at(0.0), s.at(1.0));
        let k = ((((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt() / step).ceil() as usize).clamp(1, 200);
        let skip = usize::from(!out.is_empty());
        out.extend((skip..=k).map(|i| s.at(i as f64 / k as f64)));
    }
    out
}
