use bevy::prelude::{Res, ResMut, Time, Vec3};
use spacegraph_core::{Edge, FileKind, Node, NodeId};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::Ordering;
use std::time::Instant;

use crate::graph::state::{GraphState, ViewMode};
use crate::graph::tree;

/// Stable kind tag for the query-DSL (`type`/`kind`), graph-side (no render dep).
fn node_kind_str(node: &Node) -> &'static str {
    match node {
        Node::Process { .. } => "process",
        Node::File { .. } => "file",
        Node::User { .. } => "user",
        Node::Socket { .. } => "socket",
        Node::RemoteHost { .. } => "host",
        Node::Alert { .. } => "alert",
    }
}

pub fn update_layout_or_timeline(time: Res<Time>, mut st: ResMut<GraphState>) {
    let vis: HashSet<_> = st.visible_set_capped();
    let (raw_count, agg_count) = st.visible_edge_counts(&vis);
    st.set_visible_counts(vis.len(), raw_count, agg_count);

    match st.ui.view_mode {
        ViewMode::Spatial => {
            st.progressive_prepare(&vis);
            let dt = time.delta_seconds().min(0.033);
            // FM-2: freeze the force layout while a node is focused (Focus Mode) —
            // calmer + cheaper, the node stays put. Reversible UI state, not graph
            // truth (determinism-exempt). `force_step` itself is byte-unchanged.
            if !st.layout_frozen() {
                st.force_step(&vis, dt);
            }
        }
        ViewMode::Tree => {
            st.apply_tree_layout(&vis);
        }
        ViewMode::Timeline => {}
    }

    // Publish the visible set for the renderers (entity sync + edge/tooltip
    // drawing) so they don't recompute the capped projection each.
    st.spatial.vis_cache = vis;
}

impl GraphState {
    /// Whether the force layout is frozen this frame — true while a node is focused
    /// (Focus Mode) and `focus.freeze_layout` is on. Reversible UI state; resumes
    /// the instant focus clears. `force_step` is bypassed by the caller, never
    /// modified (determinism guard unaffected).
    pub fn layout_frozen(&self) -> bool {
        self.ui.focus_mode.is_some() && self.cfg.focus.freeze_layout
    }

    pub(crate) fn mark_dirty_all(&mut self) {
        self.spatial.dirty_layout = true;
        self.spatial.springs_dirty = true;
        self.spatial.active_vis_cache.clear();
        self.spatial.progressive_cursor = 0;
        self.explain_cache = None;
        self.needs_redraw.store(true, Ordering::Relaxed);
    }

    pub fn passes_filter(&self, id: &NodeId, node: &Node) -> bool {
        if self.ui.filter.trim().is_empty() {
            return true;
        }
        let f = self.ui.filter.to_lowercase();
        let id_ok = id.0.to_lowercase().contains(&f);
        let node_ok = match node {
            Node::File { path, .. } => path.to_lowercase().contains(&f),
            Node::Process { cmdline, exe, .. } => {
                cmdline.to_lowercase().contains(&f) || exe.to_lowercase().contains(&f)
            }
            Node::User { name, .. } => name.to_lowercase().contains(&f),
            Node::Socket {
                proto, local_addr, ..
            } => proto.to_lowercase().contains(&f) || local_addr.to_lowercase().contains(&f),
            Node::RemoteHost { addr, rdns } => {
                addr.to_lowercase().contains(&f)
                    || rdns
                        .as_deref()
                        .is_some_and(|r| r.to_lowercase().contains(&f))
            }
            Node::Alert {
                signature,
                severity,
                ..
            } => signature.to_lowercase().contains(&f) || severity.to_lowercase().contains(&f),
        };
        id_ok || node_ok
    }

    /// Evaluate the query-DSL filter against a node (v0.5.0, spec §3.8). `None`
    /// query (blank or malformed) matches everything (the UI shows the error).
    pub fn query_passes(
        &self,
        query: Option<&crate::graph::query::Query>,
        id: &NodeId,
        node: &Node,
    ) -> bool {
        let Some(q) = query else {
            return true;
        };
        let label = crate::util::ids::node_label_short(node);
        let (name, path, host, severity): (&str, &str, &str, &str) = match node {
            Node::Process { exe, .. } => (exe, "", "", ""),
            Node::File { path, .. } => ("", path, "", ""),
            Node::User { name, .. } => (name, "", "", ""),
            Node::Socket { local_addr, .. } => ("", "", local_addr, ""),
            Node::RemoteHost { addr, .. } => ("", "", addr, ""),
            Node::Alert {
                signature,
                severity,
                ..
            } => (signature, "", "", severity),
        };
        let view = crate::graph::query::NodeView {
            kind: node_kind_str(node),
            label: &label,
            name,
            path,
            host,
            severity,
            degree: self.core.model.degree(id) as u32,
            recent: self.node_is_glowing(id),
        };
        q.matches(&view)
    }

    /// Effective node budget = the user's `max_visible_nodes` capped by the active
    /// quality tier (`tier_max_nodes`). Non-destructive: the persisted user value
    /// is preserved, so raising the tier restores it.
    pub fn effective_max_nodes(&self) -> usize {
        self.cfg
            .max_visible_nodes
            .min(self.cfg.tier_max_nodes)
            .max(1)
    }

    pub fn visible_set_capped(&mut self) -> HashSet<NodeId> {
        // Parse the query-DSL filter once (not per node). Malformed → None (the
        // filter chip shows the error; everything stays visible).
        let query = if self.ui.filter.trim().is_empty() {
            None
        } else {
            crate::graph::query::parse_query(&self.ui.filter).ok()
        };
        let mut base: HashSet<NodeId> = self
            .core
            .model
            .nodes
            .iter()
            .filter(|(id, n)| self.query_passes(query.as_ref(), id, n) && self.stream_enabled(id))
            .map(|(id, _)| id.clone())
            .collect();

        if let Some(focus) = &self.ui.focus {
            base.insert(focus.clone());
            let hops = self.ui.focus_hops.max(1);

            let mut vis: HashSet<NodeId> = HashSet::new();
            let mut q: VecDeque<(NodeId, usize)> = VecDeque::new();
            vis.insert(focus.clone());
            q.push_back((focus.clone(), 0));

            while let Some((cur, d)) = q.pop_front() {
                if d >= hops {
                    continue;
                }
                for nb in self.core.model.neighbors(&cur) {
                    if !vis.contains(&nb) {
                        vis.insert(nb.clone());
                        q.push_back((nb, d + 1));
                    }
                    if vis.len() >= self.effective_max_nodes() {
                        break;
                    }
                }
                if vis.len() >= self.effective_max_nodes() {
                    break;
                }
            }

            base = vis.into_iter().filter(|id| base.contains(id)).collect();
        }

        if self.ui.view_mode == ViewMode::Tree {
            base = self.tree_visible_set(&base);
        }

        let mut result = if base.len() > self.effective_max_nodes() {
            if self.ui.view_mode == ViewMode::Tree {
                // File paths sort hierarchically, so a lexicographic slice keeps
                // subtrees contiguous — the right cap for the tree view.
                let mut v: Vec<NodeId> = base.into_iter().collect();
                v.sort_by(|a, b| a.0.cmp(&b.0));
                v.truncate(self.effective_max_nodes());
                v.into_iter().collect()
            } else {
                self.cap_visible_set_connected(base)
            }
        } else {
            base
        };

        // Alerts always render regardless of the node cap or LOD: union them in
        // (already bounded by `max_visible_alerts`).
        for id in &self.core.alert_order {
            if self.stream_enabled(id) {
                result.insert(id.clone());
            }
        }

        result
    }

    /// Reduce `base` to at most `max_visible_nodes` while preserving graph
    /// connectivity, so visible edges keep both endpoints.
    ///
    /// Node IDs sort by type prefix (`…:file:…` < `…:process:…` < `…:user:…`),
    /// so a naive lexicographic truncation drops one whole type and leaves zero
    /// edges with both endpoints visible — the "agg N edges but 0/0 visible"
    /// bug. Instead we grow the set by deterministic BFS over the in-`base`
    /// adjacency, pulling connected neighbours in together. Determinism: seed
    /// order and per-node neighbour order are both sorted by ID.
    fn cap_visible_set_connected(&self, base: HashSet<NodeId>) -> HashSet<NodeId> {
        let cap = self.effective_max_nodes().max(1);
        if base.len() <= cap {
            return base;
        }

        let mut seeds: Vec<NodeId> = base.iter().cloned().collect();
        seeds.sort_by(|a, b| a.0.cmp(&b.0));

        let mut visible: HashSet<NodeId> = HashSet::with_capacity(cap);
        let mut queue: VecDeque<NodeId> = VecDeque::new();

        for seed in seeds {
            if visible.len() >= cap {
                break;
            }
            if !visible.insert(seed.clone()) {
                continue;
            }
            queue.push_back(seed);

            while let Some(cur) = queue.pop_front() {
                if visible.len() >= cap {
                    break;
                }
                let mut neighbors: Vec<NodeId> = self
                    .core
                    .model
                    .neighbors(&cur)
                    .filter(|nb| base.contains(nb) && !visible.contains(nb))
                    .collect();
                neighbors.sort_by(|a, b| a.0.cmp(&b.0));
                neighbors.dedup();
                for nb in neighbors {
                    if visible.len() >= cap {
                        break;
                    }
                    if visible.insert(nb.clone()) {
                        queue.push_back(nb);
                    }
                }
            }
        }

        visible
    }

    fn tree_visible_set(&mut self, base: &HashSet<NodeId>) -> HashSet<NodeId> {
        let mut path_by_id: HashMap<NodeId, String> = HashMap::new();
        let mut kind_by_id: HashMap<NodeId, FileKind> = HashMap::new();
        let mut non_file_ids: Vec<NodeId> = Vec::new();
        for id in base.iter() {
            match self.core.model.nodes.get(id) {
                Some(Node::File { path, kind, .. }) => {
                    path_by_id.insert(id.clone(), path.clone());
                    kind_by_id.insert(id.clone(), kind.clone());
                }
                Some(_) => non_file_ids.push(id.clone()),
                None => {}
            }
        }

        let mut path_to_id: HashMap<String, NodeId> = HashMap::new();
        for (id, path) in &path_by_id {
            path_to_id.insert(path.clone(), id.clone());
        }

        let mut children: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        let mut roots: Vec<NodeId> = Vec::new();
        for (id, path) in &path_by_id {
            let parent =
                tree::parent_path(path).and_then(|parent| path_to_id.get(&parent).cloned());
            if let Some(parent_id) = parent {
                children.entry(parent_id).or_default().push(id.clone());
            } else {
                roots.push(id.clone());
            }
        }

        self.spatial.tree_dir_children = children
            .iter()
            .filter_map(|(id, kids)| {
                if kids.is_empty() {
                    return None;
                }
                match kind_by_id.get(id) {
                    Some(FileKind::Dir) => Some(id.clone()),
                    _ => None,
                }
            })
            .collect();

        let show_files =
            self.ui.tree_show_files || self.ui.tree_zoom >= self.ui.tree_file_zoom_threshold;

        let mut visible: HashSet<NodeId> = HashSet::new();
        visible.extend(non_file_ids);

        let mut stack: Vec<NodeId> = Vec::new();
        for root in roots {
            let Some(kind) = kind_by_id.get(&root) else {
                continue;
            };
            match kind {
                FileKind::Dir => {
                    visible.insert(root.clone());
                    let depth = path_by_id
                        .get(&root)
                        .map(|p| tree::path_depth(p))
                        .unwrap_or(0);
                    if self.tree_dir_is_expanded_depth(&root, depth) {
                        stack.push(root);
                    }
                }
                _ => {
                    if show_files {
                        visible.insert(root);
                    }
                }
            }
        }

        while let Some(dir) = stack.pop() {
            let Some(kids) = children.get(&dir) else {
                continue;
            };
            for kid in kids {
                let Some(kind) = kind_by_id.get(kid) else {
                    continue;
                };
                match kind {
                    FileKind::Dir => {
                        visible.insert(kid.clone());
                        let depth = path_by_id
                            .get(kid)
                            .map(|p| tree::path_depth(p))
                            .unwrap_or(0);
                        if self.tree_dir_is_expanded_depth(kid, depth) {
                            stack.push(kid.clone());
                        }
                    }
                    _ => {
                        if show_files {
                            visible.insert(kid.clone());
                        }
                    }
                }
            }
        }

        visible
    }

    pub fn edge_visible(&self, e: &Edge, vis: &HashSet<NodeId>) -> bool {
        vis.contains(&e.from) && vis.contains(&e.to)
    }

    pub fn set_visible_counts(&mut self, vis_nodes: usize, raw_edges: usize, agg_edges: usize) {
        self.perf.visible_nodes = vis_nodes;
        self.perf.visible_raw_edges = raw_edges;
        self.perf.visible_agg_edges = agg_edges;
        self.perf.visible_edges = raw_edges + agg_edges;
    }

    pub fn visible_edge_counts(&self, vis: &HashSet<NodeId>) -> (usize, usize) {
        let mut raw_count = 0usize;
        for id in vis.iter() {
            for edge in self.core.model.edges_for_node(id) {
                if &edge.from != id {
                    continue;
                }
                if self.edge_visible(edge, vis) {
                    raw_count += 1;
                }
            }
        }

        let agg_count = self
            .core
            .model
            .agg_edges()
            .filter(|edge| vis.contains(&edge.key.from) && vis.contains(&edge.key.to))
            .count();
        (raw_count, agg_count)
    }

    // ----- Progressive init / Force layout (spatial) -----
    pub fn progressive_prepare(&mut self, vis: &HashSet<NodeId>) {
        if self.spatial.active_vis_cache.is_empty() || self.spatial.dirty_layout {
            self.spatial.active_vis_cache = vis.iter().cloned().collect();
            self.spatial.active_vis_cache.sort_by(|a, b| a.0.cmp(&b.0));
            self.spatial.progressive_cursor = 0;
        }

        let count = self.spatial.active_vis_cache.len();
        // `.max(1)` only guards the side-scale maths below; slice bounds use the
        // real length so an empty visible set (no agent / no demo) can't panic.
        let total = count.max(1);
        let show_3d = self.ui.show_3d;
        // Initial spacing ≈ the repulsion cell size, so the grid starts at ~1
        // node per cell (bounded density). The region side scales with the node
        // count, keeping density bounded as N grows — without this the uniform
        // grid degenerates back to O(N²) because every node lands in a handful
        // of cells.
        let spacing = self
            .cfg
            .repulsion_radius
            .max(self.cfg.link_distance)
            .max(1.0);
        let side = if show_3d {
            (total as f32).cbrt() * spacing
        } else {
            (total as f32).sqrt() * spacing
        };

        let take = self.cfg.progressive_nodes_per_frame.max(1);
        let start = self.spatial.progressive_cursor.min(count);
        let end = (start + take).min(count);

        // Snapshot the slice so we can intern (mutate spatial) while iterating.
        let slice: Vec<NodeId> = self.spatial.active_vis_cache[start..end].to_vec();

        for (off, id) in slice.iter().enumerate() {
            // Network nodes sit on an outer shell (remote hosts furthest out),
            // so connections fan outward from the process core.
            // Network nodes sit on an outer shell; sockets refine their radial
            // depth by exposure (Public outermost, Loopback at the core) so the
            // host's attack surface reads as silhouette (D0/ADR-0012, no wire).
            let shell = match self.core.model.nodes.get(id) {
                Some(Node::RemoteHost { .. }) => 1.8_f32,
                Some(Node::Socket { local_addr, .. }) => {
                    if self.cfg.socket_display.exposure_depth {
                        crate::render::spatial::exposure_bucket(local_addr).shell_factor()
                    } else {
                        1.25
                    }
                }
                Some(_) => 1.0,
                None => continue,
            };
            let idx = self.spatial.intern(id);
            if self.spatial.placed[idx.slot()] {
                continue;
            }
            let pos = scatter_position(start + off, side, show_3d) * shell;
            self.spatial.set_position(idx, pos);
        }

        self.spatial.progressive_cursor = end;
        if self.spatial.progressive_cursor >= count {
            self.spatial.dirty_layout = false;
        }

        // Only request a redraw while there is placement work in flight; an
        // idle re-entry (everything already placed) must not keep the frame loop
        // awake, or reactive rendering can never go idle.
        if end > start {
            self.needs_redraw.store(true, Ordering::Relaxed);
        }
    }

    /// Rebuild the index-based spring list from current model edges. Called only
    /// when topology changes (`springs_dirty`), never per frame. Endpoints are
    /// interned in sorted edge order so index assignment (and thus the layout)
    /// is deterministic regardless of `HashSet` iteration order.
    fn rebuild_springs(&mut self) {
        let mut pairs: Vec<(NodeId, NodeId)> = self
            .core
            .model
            .edges
            .iter()
            .map(|e| (e.from.clone(), e.to.clone()))
            .collect();
        pairs.sort_by(|a, b| a.0 .0.cmp(&b.0 .0).then_with(|| a.1 .0.cmp(&b.1 .0)));
        self.spatial.spring_edges.clear();
        self.spatial.spring_edges.reserve(pairs.len());
        for (from, to) in pairs {
            let a = self.spatial.intern(&from);
            let b = self.spatial.intern(&to);
            self.spatial.spring_edges.push((a, b));
        }
        self.spatial.springs_dirty = false;
    }

    pub fn force_step(&mut self, vis: &HashSet<NodeId>, dt: f32) {
        if !self.cfg.layout_force {
            return;
        }

        // Any pending topology/placement work means the layout will move again.
        if self.spatial.dirty_layout || self.spatial.springs_dirty {
            self.spatial.layout_settled = false;
            self.spatial.settle_streak = 0;
        }

        // Once converged, freeze: skip integration entirely so positions don't
        // drift while the app renders reactively (and to save CPU). Any dirty
        // flag above clears `layout_settled`, so this only fires at true rest.
        if self.spatial.layout_settled && self.spatial.repulsion_cursor == 0 {
            return;
        }

        if self.spatial.springs_dirty {
            self.rebuild_springs();
        }

        let cap = self.spatial.interner.capacity();

        // Active set = visible AND placed; mask lets springs filter in O(1).
        // Sorted by index for deterministic force accumulation order.
        self.spatial.visible_mask.clear();
        self.spatial.visible_mask.resize(cap, false);
        self.spatial.active.clear();
        for id in vis.iter() {
            if let Some(idx) = self.spatial.interner.index_of(id) {
                let i = idx.slot();
                if self.spatial.placed[i] {
                    self.spatial.visible_mask[i] = true;
                    self.spatial.active.push(idx);
                }
            }
        }
        if self.spatial.active.len() <= 1 {
            self.spatial.repulsion_cursor = 0;
            self.spatial.layout_settled = true;
            return;
        }
        self.spatial.active.sort_unstable();

        let link_dist = self.cfg.link_distance.max(0.1);
        let repulsion = self.cfg.repulsion.max(0.0);
        let damping = self.cfg.damping.clamp(0.0, 1.0);
        let max_step = self.cfg.max_step.max(0.001);
        let show_3d = self.ui.show_3d;
        // Cell size == cutoff radius, so adjacent-cell search captures every
        // pair within the cutoff.
        let cell = if self.cfg.repulsion_radius > 0.0 {
            self.cfg.repulsion_radius
        } else {
            (2.5 * link_dist).max(0.1)
        };
        let cutoff2 = cell * cell;
        let budget_ms = self.cfg.layout_budget_ms;

        // A repulsion pass spans one or more frames. At the start of a pass
        // (cursor == 0) reset the force accumulator and rebuild the grid against
        // the (frozen) positions; positions only change once a pass completes,
        // so splitting a pass across frames is deterministic.
        if self.spatial.repulsion_cursor == 0 {
            self.spatial.forces.clear();
            self.spatial.forces.resize(cap, Vec3::ZERO);
            self.spatial
                .grid
                .rebuild(&self.spatial.positions, &self.spatial.active, cell, show_3d);
        } else if self.spatial.forces.len() < cap {
            self.spatial.forces.resize(cap, Vec3::ZERO);
        }

        // Neighbour-only repulsion, resumable under the per-frame time budget.
        let start = Instant::now();
        let n = self.spatial.active.len();
        let mut i = self.spatial.repulsion_cursor.min(n);
        let mut neigh = std::mem::take(&mut self.spatial.grid_scratch);
        while i < n {
            let a = self.spatial.active[i];
            let ia = a.slot();
            let pa = self.spatial.positions[ia];
            self.spatial.grid.neighbors_into(pa, &mut neigh);
            // No sort needed: the grid is built by pushing `active` (sorted by
            // index), so each bucket — and the gathered candidate list — is in a
            // deterministic order across runs, giving a deterministic force
            // accumulation order without an O(k log k) per-node sort.
            for &b in neigh.iter() {
                if b.0 <= a.0 {
                    continue; // each unordered pair acted on once (lower index)
                }
                let ib = b.slot();
                let pb = self.spatial.positions[ib];
                let mut dir = pa - pb;
                if !show_3d {
                    dir.y = 0.0;
                }
                let dist2 = dir.length_squared();
                if dist2 > cutoff2 {
                    continue;
                }
                let d2 = dist2.max(0.01);
                let f = (repulsion / d2) * dir.normalize_or_zero();
                self.spatial.forces[ia] += f;
                self.spatial.forces[ib] -= f;
            }
            i += 1;
            if budget_ms > 0.0
                && (i & 0xFF) == 0
                && start.elapsed().as_secs_f32() * 1000.0 > budget_ms
            {
                break;
            }
        }
        self.spatial.grid_scratch = neigh;

        let pass_complete = i >= n;
        self.spatial.repulsion_cursor = if pass_complete { 0 } else { i };
        if !pass_complete {
            // Positions stay frozen until the repulsion pass finishes.
            self.needs_redraw.store(true, Ordering::Relaxed);
            return;
        }

        // Springs: iterate the prebuilt index list, applying only to pairs that
        // are both visible (mask check) — no edge scans, no clones.
        let k = 0.6;
        for si in 0..self.spatial.spring_edges.len() {
            let (a, b) = self.spatial.spring_edges[si];
            let ia = a.slot();
            let ib = b.slot();
            if !self.spatial.visible_mask[ia] || !self.spatial.visible_mask[ib] {
                continue;
            }
            let pa = self.spatial.positions[ia];
            let pb = self.spatial.positions[ib];
            let mut d = pb - pa;
            if !show_3d {
                d.y = 0.0;
            }
            let len = d.length().max(0.001);
            let dir = d / len;
            let stretch = len - link_dist;
            let f = k * stretch * dir;
            self.spatial.forces[ia] += f;
            self.spatial.forces[ib] -= f;
        }

        // Integrate over the active set, tracking the largest displacement so we
        // can detect convergence and stop requesting redraws once at rest.
        let mut max_step2 = 0.0_f32;
        for k2 in 0..self.spatial.active.len() {
            let i = self.spatial.active[k2].slot();
            // Pinned nodes are clamped to their pin and skip integration, but
            // their (fixed) position still drove the spring forces above, so
            // neighbours settle around them. Deterministic.
            if let Some(pin) = self.spatial.pinned[i] {
                self.spatial.positions[i] = pin;
                self.spatial.velocities[i] = Vec3::ZERO;
                continue;
            }
            let f = self.spatial.forces[i];
            let v = &mut self.spatial.velocities[i];
            *v = (*v + f * dt) * damping;
            let mut step = *v * dt;
            if step.length() > max_step {
                step = step.normalize_or_zero() * max_step;
            }
            let p = &mut self.spatial.positions[i];
            *p += step;
            if !show_3d {
                p.y = 0.0;
            }
            max_step2 = max_step2.max(step.length_squared());
        }

        // The force layout doesn't damp to zero — a hard repulsion cutoff leaves
        // a small residual limit cycle (~0.01-0.03/frame). Treat motion below
        // SETTLE_EPS (well under the structural movement of an unsettled graph,
        // well above that residual) as "at rest", and require a short streak so
        // a single slow frame can't freeze a still-forming layout.
        const SETTLE_EPS: f32 = 0.05;
        const SETTLE_FRAMES: u32 = 8;
        if max_step2 <= SETTLE_EPS * SETTLE_EPS {
            self.spatial.settle_streak = self.spatial.settle_streak.saturating_add(1);
        } else {
            self.spatial.settle_streak = 0;
        }
        self.spatial.layout_settled = self.spatial.settle_streak >= SETTLE_FRAMES;
        if !self.spatial.layout_settled {
            self.needs_redraw.store(true, Ordering::Relaxed);
        }
    }

    pub fn apply_tree_layout(&mut self, vis: &HashSet<NodeId>) {
        let positions =
            tree::layout_tree_positions(&self.core.model.nodes, vis, &self.cfg.path_includes);
        let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        for id in vis {
            let Some(pos) = positions.get(id) else {
                continue;
            };
            min.x = min.x.min(pos.x);
            min.y = min.y.min(pos.y);
            min.z = min.z.min(pos.z);
            max.x = max.x.max(pos.x);
            max.y = max.y.max(pos.y);
            max.z = max.z.max(pos.z);
        }
        if min.x.is_finite() {
            self.ui.tree_center = (min + max) * 0.5;
        } else {
            self.ui.tree_center = Vec3::ZERO;
        }
        // Tree layout replaces all spatial positions: clear placements, zero
        // velocities, then set tree positions by index.
        self.spatial.placed.iter_mut().for_each(|p| *p = false);
        self.spatial
            .velocities
            .iter_mut()
            .for_each(|v| *v = Vec3::ZERO);
        for (id, pos) in &positions {
            let idx = self.spatial.intern(id);
            self.spatial.set_position(idx, *pos);
        }
        self.spatial.dirty_layout = false;
        self.needs_redraw.store(true, Ordering::Relaxed);
    }
}

/// Deterministic low-discrepancy scatter (R3 additive recurrence) over a cube
/// of the given `side`, centred at the origin. Spreads nodes at roughly uniform
/// density so the uniform-grid repulsion stays O(N) from the first frame. Pure
/// function of `global_index`, so placement is reproducible across runs.
fn scatter_position(global_index: usize, side: f32, show_3d: bool) -> Vec3 {
    // 1/φ, 1/φ², 1/φ³ for the cubic plastic constant φ ≈ 1.2207440846.
    const A1: f64 = 0.819_172_513_396_036_4;
    const A2: f64 = 0.671_043_606_703_789_5;
    const A3: f64 = 0.549_700_477_901_970_5;
    let g = global_index as f64;
    let frac = |x: f64| x - x.floor();
    let u = (frac(0.5 + A1 * g) - 0.5) as f32;
    let v = (frac(0.5 + A2 * g) - 0.5) as f32;
    let x = u * side;
    if show_3d {
        let w = (frac(0.5 + A3 * g) - 0.5) as f32;
        Vec3::new(x, w * side, v * side)
    } else {
        Vec3::new(x, 0.0, v * side)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with_synthetic(n: usize, cap: usize) -> GraphState {
        let mut st = GraphState::default();
        st.cfg.max_visible_nodes = cap;
        st.ui.view_mode = ViewMode::Spatial;
        st.load_synthetic_graph(n);
        st
    }

    #[test]
    fn capped_visible_set_preserves_edges() {
        // Regression for the "agg N edges but 0/0 visible" bug: lexicographic
        // truncation dropped every process node (IDs sort by type prefix),
        // leaving zero edges with both endpoints in the visible set.
        let mut st = state_with_synthetic(3000, 1200);
        let vis = st.visible_set_capped();
        assert_eq!(vis.len(), 1200, "cap should saturate for a 3000-node graph");

        let (raw, agg) = st.visible_edge_counts(&vis);
        assert!(
            agg > 0,
            "connectivity-aware cap must keep visible aggregated edges (got 0)"
        );
        // A connectivity-preserving cap keeps a dense subgraph, not a handful.
        assert!(
            agg >= 100,
            "expected many visible edges, got agg={agg} raw={raw}"
        );
    }

    #[test]
    fn capped_visible_set_is_deterministic() {
        let mut a = state_with_synthetic(3000, 1200);
        let mut b = state_with_synthetic(3000, 1200);
        assert_eq!(a.visible_set_capped(), b.visible_set_capped());
    }

    #[test]
    fn uncapped_visible_set_returns_all_nodes() {
        let mut st = state_with_synthetic(500, 10_000);
        let vis = st.visible_set_capped();
        assert_eq!(vis.len(), 500);
        let (_, agg) = st.visible_edge_counts(&vis);
        assert!(agg > 0);
    }

    #[test]
    fn spring_edges_resolve_after_node_removal() {
        let mut st = state_with_synthetic(80, 10_000);
        let vis = st.visible_set_capped();
        st.cfg.progressive_nodes_per_frame = 10_000;
        st.progressive_prepare(&vis);
        st.force_step(&vis, 0.016); // builds the spring list

        assert!(!st.spatial.spring_edges.is_empty());
        for &(a, b) in &st.spatial.spring_edges {
            assert!(st.spatial.interner.resolve(a).is_some());
            assert!(st.spatial.interner.resolve(b).is_some());
        }

        // Remove a node (GC-style) and recompute; spring endpoints must still
        // resolve and none may reference the removed node's slot.
        let victim = st.core.model.nodes.keys().next().cloned().unwrap();
        st.core.model.remove_node(&victim);
        st.spatial.release(&victim);
        st.spatial.springs_dirty = true;

        let vis2 = st.visible_set_capped();
        st.force_step(&vis2, 0.016);

        assert!(st.spatial.index_of(&victim).is_none());
        for &(a, b) in &st.spatial.spring_edges {
            let ra = st.spatial.interner.resolve(a);
            let rb = st.spatial.interner.resolve(b);
            assert!(ra.is_some() && rb.is_some());
            assert_ne!(ra, Some(&victim));
            assert_ne!(rb, Some(&victim));
        }
    }

    #[test]
    fn force_layout_settles_freezes_and_wakes() {
        let mut st = state_with_synthetic(80, 10_000);
        let vis = st.visible_set_capped();
        st.cfg.progressive_nodes_per_frame = 10_000;
        st.progressive_prepare(&vis);

        // Run until the force layout converges (bounded so a regression fails
        // rather than hangs).
        let mut settled = false;
        for _ in 0..4000 {
            st.force_step(&vis, 0.016);
            if st.spatial.layout_settled {
                settled = true;
                break;
            }
        }
        assert!(settled, "force layout should converge to a settled state");

        // While settled, a step is frozen: positions don't drift and no redraw
        // is requested (so the app can render reactively).
        st.needs_redraw.store(false, Ordering::Relaxed);
        let before = st.spatial.positions.clone();
        st.force_step(&vis, 0.016);
        assert_eq!(
            before, st.spatial.positions,
            "settled layout must not drift"
        );
        assert!(
            !st.needs_redraw.load(Ordering::Relaxed),
            "settled layout must not request redraws"
        );

        // Marking the layout dirty wakes it back up.
        st.spatial.dirty_layout = true;
        st.force_step(&vis, 0.016);
        assert!(
            !st.spatial.layout_settled,
            "dirty layout is no longer settled"
        );
    }

    #[test]
    fn force_step_is_finite_and_moves_nodes() {
        let mut st = state_with_synthetic(200, 10_000);
        let vis = st.visible_set_capped();
        st.cfg.progressive_nodes_per_frame = 10_000;
        st.progressive_prepare(&vis);
        for _ in 0..20 {
            st.force_step(&vis, 0.016);
        }
        // All placed positions stay finite (no NaN/inf blow-ups).
        for (_, pos) in st.spatial.placed_positions() {
            assert!(pos.is_finite(), "position went non-finite: {pos:?}");
        }
    }

    #[test]
    fn empty_graph_layout_does_not_panic() {
        // First-run state: no agent, no demo → empty visible set.
        let mut st = GraphState::default();
        let vis = st.visible_set_capped();
        assert!(vis.is_empty());
        st.progressive_prepare(&vis);
        st.force_step(&vis, 0.016);
        st.apply_tree_layout(&vis);
    }

    #[test]
    fn force_step_is_deterministic() {
        fn run() -> Vec<(String, Vec3)> {
            let mut st = state_with_synthetic(800, 10_000);
            st.cfg.layout_budget_ms = 0.0; // full step per call (no budget split)
            let vis = st.visible_set_capped();
            st.cfg.progressive_nodes_per_frame = 10_000;
            st.progressive_prepare(&vis);
            for _ in 0..40 {
                st.force_step(&vis, 0.016);
            }
            let mut ids: Vec<NodeId> = vis.iter().cloned().collect();
            ids.sort_by(|a, b| a.0.cmp(&b.0));
            ids.into_iter()
                .map(|id| {
                    let p = st.spatial.position_of(&id).unwrap_or(Vec3::ZERO);
                    (id.0, p)
                })
                .collect()
        }
        assert_eq!(
            run(),
            run(),
            "same seeded graph must produce identical positions after K steps"
        );
    }

    #[test]
    fn force_step_keeps_pinned_fixed_and_deterministic() {
        let pin_pos = Vec3::new(3.0, 0.0, -2.0);
        fn run(pin_pos: Vec3) -> (Vec3, Vec<(String, Vec3)>) {
            let mut st = state_with_synthetic(400, 10_000);
            st.cfg.layout_budget_ms = 0.0;
            let vis = st.visible_set_capped();
            st.cfg.progressive_nodes_per_frame = 10_000;
            st.progressive_prepare(&vis);
            let mut ids: Vec<NodeId> = vis.iter().cloned().collect();
            ids.sort_by(|a, b| a.0.cmp(&b.0));
            let pin_id = ids[0].clone();
            st.set_pin(&pin_id, pin_pos);
            for _ in 0..40 {
                st.force_step(&vis, 0.016);
            }
            let pinned_now = st.spatial.position_of(&pin_id).unwrap_or(Vec3::ZERO);
            let all = ids
                .into_iter()
                .map(|id| {
                    (
                        id.0.clone(),
                        st.spatial.position_of(&id).unwrap_or(Vec3::ZERO),
                    )
                })
                .collect();
            (pinned_now, all)
        }
        let (p1, a1) = run(pin_pos);
        let (p2, a2) = run(pin_pos);
        assert!(
            (p1 - pin_pos).length() < 1e-4,
            "pinned node stays clamped at its pin: {p1:?}"
        );
        assert_eq!(p1, p2);
        assert_eq!(a1, a2, "layout with a pinned node is deterministic");
    }

    #[test]
    fn layout_freezes_on_focus_and_resumes_on_exit() {
        let mut st = state_with_synthetic(200, 10_000);
        // Not focused → not frozen.
        assert!(!st.layout_frozen());
        // Enter Focus Mode → frozen (force_step is bypassed by the caller).
        st.ui.focus_mode = Some(NodeId("n".to_string()));
        assert!(st.layout_frozen(), "focus freezes the layout");
        // Disabling the config knob un-freezes even while focused.
        st.cfg.focus.freeze_layout = false;
        assert!(!st.layout_frozen());
        st.cfg.focus.freeze_layout = true;
        // Exit Focus Mode → resumes (reversible).
        st.ui.focus_mode = None;
        assert!(!st.layout_frozen(), "exiting focus resumes the layout");
    }

    #[test]
    fn budget_split_matches_full_step() {
        fn setup() -> (GraphState, HashSet<NodeId>) {
            let mut st = state_with_synthetic(600, 10_000);
            st.cfg.progressive_nodes_per_frame = 10_000;
            let vis = st.visible_set_capped();
            st.progressive_prepare(&vis);
            (st, vis)
        }
        fn positions(st: &GraphState, vis: &HashSet<NodeId>) -> Vec<(String, Vec3)> {
            let mut ids: Vec<NodeId> = vis.iter().cloned().collect();
            ids.sort_by(|a, b| a.0.cmp(&b.0));
            ids.into_iter()
                .map(|id| {
                    (
                        id.0.clone(),
                        st.spatial.position_of(&id).unwrap_or(Vec3::ZERO),
                    )
                })
                .collect()
        }

        // One full pass, unbudgeted.
        let (mut full, vis_full) = setup();
        full.cfg.layout_budget_ms = 0.0;
        full.force_step(&vis_full, 0.016);
        let p_full = positions(&full, &vis_full);

        // The same single pass, split across frames by a tiny time budget.
        let (mut split, vis_split) = setup();
        split.cfg.layout_budget_ms = 1e-6;
        let mut frames = 0;
        loop {
            split.force_step(&vis_split, 0.016);
            frames += 1;
            if split.spatial.repulsion_cursor == 0 {
                break; // pass completed (and integrated) this frame
            }
            assert!(frames < 100, "repulsion pass never completed");
        }
        assert!(
            frames > 1,
            "tiny budget should have split the pass across frames"
        );
        assert_eq!(
            p_full,
            positions(&split, &vis_split),
            "budget-split pass must match an unbudgeted full step"
        );
    }
}
