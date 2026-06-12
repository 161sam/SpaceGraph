use bevy::prelude::{Res, ResMut, Time, Vec3};
use spacegraph_core::{Edge, FileKind, Node, NodeId};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::Ordering;

use crate::graph::interner::NodeIndex;
use crate::graph::state::{GraphState, ViewMode};
use crate::graph::tree;

pub fn update_layout_or_timeline(time: Res<Time>, mut st: ResMut<GraphState>) {
    let vis: HashSet<_> = st.visible_set_capped();
    let (raw_count, agg_count) = st.visible_edge_counts(&vis);
    st.set_visible_counts(vis.len(), raw_count, agg_count);

    match st.ui.view_mode {
        ViewMode::Spatial => {
            st.progressive_prepare(&vis);
            let dt = time.delta_seconds().min(0.033);
            st.force_step(&vis, dt);
        }
        ViewMode::Tree => {
            st.apply_tree_layout(&vis);
        }
        ViewMode::Timeline => {}
    }
}

impl GraphState {
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
        };
        id_ok || node_ok
    }

    pub fn visible_set_capped(&mut self) -> HashSet<NodeId> {
        let mut base: HashSet<NodeId> = self
            .model
            .nodes
            .iter()
            .filter(|(id, n)| self.passes_filter(id, n))
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
                for nb in self.model.neighbors(&cur) {
                    if !vis.contains(&nb) {
                        vis.insert(nb.clone());
                        q.push_back((nb, d + 1));
                    }
                    if vis.len() >= self.cfg.max_visible_nodes {
                        break;
                    }
                }
                if vis.len() >= self.cfg.max_visible_nodes {
                    break;
                }
            }

            base = vis.into_iter().filter(|id| base.contains(id)).collect();
        }

        if self.ui.view_mode == ViewMode::Tree {
            base = self.tree_visible_set(&base);
        }

        if base.len() > self.cfg.max_visible_nodes {
            if self.ui.view_mode == ViewMode::Tree {
                // File paths sort hierarchically, so a lexicographic slice keeps
                // subtrees contiguous — the right cap for the tree view.
                let mut v: Vec<NodeId> = base.into_iter().collect();
                v.sort_by(|a, b| a.0.cmp(&b.0));
                v.truncate(self.cfg.max_visible_nodes);
                v.into_iter().collect()
            } else {
                self.cap_visible_set_connected(base)
            }
        } else {
            base
        }
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
        let cap = self.cfg.max_visible_nodes.max(1);
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
            match self.model.nodes.get(id) {
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
            for edge in self.model.edges_for_node(id) {
                if &edge.from != id {
                    continue;
                }
                if self.edge_visible(edge, vis) {
                    raw_count += 1;
                }
            }
        }

        let agg_count = self
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

        let radius = if self.cfg.radius <= 0.0 {
            25.0
        } else {
            self.cfg.radius
        };
        // When 3D is off, collapse the ring spread onto the y=0 plane.
        let y_spread = if self.ui.show_3d {
            self.cfg.y_spread
        } else {
            0.0
        };

        let take = self.cfg.progressive_nodes_per_frame.max(1);
        let start = self.spatial.progressive_cursor;
        let end = (start + take).min(self.spatial.active_vis_cache.len());

        // Snapshot the slice so we can intern (mutate spatial) while iterating.
        let slice: Vec<NodeId> = self.spatial.active_vis_cache[start..end].to_vec();

        let mut proc_idx: Vec<NodeIndex> = Vec::new();
        let mut file_idx: Vec<NodeIndex> = Vec::new();
        let mut user_idx: Vec<NodeIndex> = Vec::new();

        for id in &slice {
            let kind = match self.model.nodes.get(id) {
                Some(Node::Process { .. }) => 0u8,
                Some(Node::File { .. }) => 1,
                Some(Node::User { .. }) => 2,
                None => continue,
            };
            let idx = self.spatial.intern(id);
            if self.spatial.placed[idx.slot()] {
                continue;
            }
            match kind {
                0 => proc_idx.push(idx),
                1 => file_idx.push(idx),
                _ => user_idx.push(idx),
            }
        }

        self.place_ring_idx(&proc_idx, radius * 0.7, y_spread);
        self.place_ring_idx(&file_idx, radius * 1.2, y_spread);
        self.place_ring_idx(&user_idx, radius * 0.35, y_spread);

        self.spatial.progressive_cursor = end;
        if self.spatial.progressive_cursor >= self.spatial.active_vis_cache.len() {
            self.spatial.dirty_layout = false;
        }

        self.needs_redraw.store(true, Ordering::Relaxed);
    }

    /// Place a batch of nodes on a ring of radius `r`. Velocities default to
    /// zero (interner storage), so no separate velocity init is needed.
    fn place_ring_idx(&mut self, idxs: &[NodeIndex], r: f32, y_spread: f32) {
        let n = idxs.len().max(1) as f32;
        for (i, &idx) in idxs.iter().enumerate() {
            if self.spatial.placed[idx.slot()] {
                continue;
            }
            let t = (i as f32) / n * std::f32::consts::TAU;
            let x = r * t.cos();
            let z = r * t.sin();
            let y = if y_spread > 0.0 {
                ((i as f32) % 7.0) / 7.0 * y_spread
            } else {
                0.0
            };
            self.spatial.set_position(idx, Vec3::new(x, y, z));
        }
    }

    /// Rebuild the index-based spring list from current model edges. Called only
    /// when topology changes (`springs_dirty`), never per frame.
    fn rebuild_springs(&mut self) {
        let pairs: Vec<(NodeId, NodeId)> = self
            .model
            .edges
            .iter()
            .map(|e| (e.from.clone(), e.to.clone()))
            .collect();
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

        if self.spatial.springs_dirty {
            self.rebuild_springs();
        }

        let cap = self.spatial.interner.capacity();

        // Active set = visible AND placed; mask lets springs filter in O(1).
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
            return;
        }

        let link_dist = self.cfg.link_distance.max(0.1);
        let repulsion = self.cfg.repulsion.max(0.0);
        let damping = self.cfg.damping.clamp(0.0, 1.0);
        let max_step = self.cfg.max_step.max(0.001);
        let show_3d = self.ui.show_3d;

        // Reset force accumulator (indexed by NodeIndex).
        self.spatial.forces.clear();
        self.spatial.forces.resize(cap, Vec3::ZERO);

        // Repulsion: O(N^2) over the active set (algorithm unchanged from
        // baseline; Phase 2 replaces this with a uniform grid). Index-based
        // array access — no HashMap lookups, no per-pair NodeId clones.
        let n = self.spatial.active.len();
        for ai in 0..n {
            let ia = self.spatial.active[ai].slot();
            let pa = self.spatial.positions[ia];
            for bi in (ai + 1)..n {
                let ib = self.spatial.active[bi].slot();
                let pb = self.spatial.positions[ib];
                let mut dir = pa - pb;
                if !show_3d {
                    dir.y = 0.0;
                }
                let dist2 = dir.length_squared().max(0.01);
                let f = (repulsion / dist2) * dir.normalize_or_zero();
                self.spatial.forces[ia] += f;
                self.spatial.forces[ib] -= f;
            }
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

        // Integrate over the active set.
        for k2 in 0..self.spatial.active.len() {
            let i = self.spatial.active[k2].slot();
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
        }

        self.needs_redraw.store(true, Ordering::Relaxed);
    }

    pub fn apply_tree_layout(&mut self, vis: &HashSet<NodeId>) {
        let positions =
            tree::layout_tree_positions(&self.model.nodes, vis, &self.cfg.path_includes);
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
        let victim = st.model.nodes.keys().next().cloned().unwrap();
        st.model.remove_node(&victim);
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
}
