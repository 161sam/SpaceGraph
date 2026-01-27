use bevy::prelude::{Res, ResMut, Time, Vec3};
use spacegraph_core::{Edge, Node, NodeId};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::Ordering;

use crate::graph::state::{GraphState, ViewMode};

pub fn update_layout_or_timeline(time: Res<Time>, mut st: ResMut<GraphState>) {
    let vis: HashSet<_> = st.visible_set_capped();
    let (raw_count, agg_count) = st.visible_edge_counts(&vis);
    st.set_visible_counts(vis.len(), raw_count, agg_count);

    if st.ui.view_mode == ViewMode::Spatial {
        st.progressive_prepare(&vis);
        let dt = time.delta_seconds().min(0.033);
        st.force_step(&vis, dt);
    }
}

impl GraphState {
    pub(crate) fn mark_dirty_all(&mut self) {
        self.spatial.dirty_layout = true;
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

        if base.len() > self.cfg.max_visible_nodes {
            let mut v: Vec<NodeId> = base.into_iter().collect();
            v.sort_by(|a, b| a.0.cmp(&b.0));
            v.truncate(self.cfg.max_visible_nodes);
            v.into_iter().collect()
        } else {
            base
        }
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
        let y_spread = self.cfg.y_spread;

        let take = self.cfg.progressive_nodes_per_frame.max(1);
        let start = self.spatial.progressive_cursor;
        let end = (start + take).min(self.spatial.active_vis_cache.len());

        let mut proc_ids = Vec::new();
        let mut file_ids = Vec::new();
        let mut user_ids = Vec::new();

        for id in &self.spatial.active_vis_cache[start..end] {
            if self.spatial.positions.contains_key(id) {
                continue;
            }
            if let Some(n) = self.model.nodes.get(id) {
                match n {
                    Node::Process { .. } => proc_ids.push(id.clone()),
                    Node::File { .. } => file_ids.push(id.clone()),
                    Node::User { .. } => user_ids.push(id.clone()),
                }
            }
        }

        place_ring(
            &mut self.spatial.positions,
            &proc_ids,
            radius * 0.7,
            0.0,
            y_spread,
        );
        place_ring(
            &mut self.spatial.positions,
            &file_ids,
            radius * 1.2,
            0.0,
            y_spread,
        );
        place_ring(
            &mut self.spatial.positions,
            &user_ids,
            radius * 0.35,
            0.0,
            y_spread,
        );

        for id in &self.spatial.active_vis_cache[start..end] {
            self.spatial
                .velocities
                .entry(id.clone())
                .or_insert(Vec3::ZERO);
            if !self.ui.show_3d {
                if let Some(p) = self.spatial.positions.get_mut(id) {
                    p.y = 0.0;
                }
            }
        }

        self.spatial.progressive_cursor = end;
        if self.spatial.progressive_cursor >= self.spatial.active_vis_cache.len() {
            self.spatial.dirty_layout = false;
        }

        self.needs_redraw.store(true, Ordering::Relaxed);
    }

    pub fn force_step(&mut self, vis: &HashSet<NodeId>, dt: f32) {
        if !self.cfg.layout_force {
            return;
        }

        let ids: Vec<NodeId> = vis
            .iter()
            .filter(|id| self.spatial.positions.contains_key(*id))
            .cloned()
            .collect();
        if ids.len() <= 1 {
            return;
        }

        let link_dist = self.cfg.link_distance.max(0.1);
        let repulsion = self.cfg.repulsion.max(0.0);
        let damping = self.cfg.damping.clamp(0.0, 1.0);
        let max_step = self.cfg.max_step.max(0.001);

        let mut forces: HashMap<NodeId, Vec3> = HashMap::new();
        for id in ids.iter() {
            forces.insert(id.clone(), Vec3::ZERO);
        }

        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let a = &ids[i];
                let b = &ids[j];
                let pa = *self.spatial.positions.get(a).unwrap_or(&Vec3::ZERO);
                let pb = *self.spatial.positions.get(b).unwrap_or(&Vec3::ZERO);

                let mut dir = pa - pb;
                if !self.ui.show_3d {
                    dir.y = 0.0;
                }
                let dist2 = dir.length_squared().max(0.01);
                let f = (repulsion / dist2) * dir.normalize_or_zero();

                *forces.get_mut(a).unwrap() += f;
                *forces.get_mut(b).unwrap() -= f;
            }
        }

        for id in vis.iter() {
            for edge in self.model.edges_for_node(id) {
                if &edge.from != id {
                    continue;
                }
                if !self.edge_visible(edge, vis) {
                    continue;
                }
                if !(self.spatial.positions.contains_key(&edge.from)
                    && self.spatial.positions.contains_key(&edge.to))
                {
                    continue;
                }
                let pa = *self
                    .spatial
                    .positions
                    .get(&edge.from)
                    .unwrap_or(&Vec3::ZERO);
                let pb = *self.spatial.positions.get(&edge.to).unwrap_or(&Vec3::ZERO);

                let mut d = pb - pa;
                if !self.ui.show_3d {
                    d.y = 0.0;
                }
                let len = d.length().max(0.001);
                let dir = d / len;
                let k = 0.6;
                let stretch = len - link_dist;
                let f = k * stretch * dir;

                *forces.get_mut(&edge.from).unwrap() += f;
                *forces.get_mut(&edge.to).unwrap() -= f;
            }
        }

        for id in ids.iter() {
            let v = self
                .spatial
                .velocities
                .entry(id.clone())
                .or_insert(Vec3::ZERO);
            let f = *forces.get(id).unwrap_or(&Vec3::ZERO);

            *v = (*v + f * dt) * damping;

            let mut step = *v * dt;
            if step.length() > max_step {
                step = step.normalize_or_zero() * max_step;
            }

            let p = self
                .spatial
                .positions
                .entry(id.clone())
                .or_insert(Vec3::ZERO);
            *p += step;
            if !self.ui.show_3d {
                p.y = 0.0;
            }
        }

        self.needs_redraw.store(true, Ordering::Relaxed);
    }
}

fn place_ring(pos: &mut HashMap<NodeId, Vec3>, ids: &[NodeId], r: f32, y_base: f32, y_spread: f32) {
    let n = ids.len().max(1) as f32;
    for (i, id) in ids.iter().enumerate() {
        if pos.contains_key(id) {
            continue;
        }
        let t = (i as f32) / n * std::f32::consts::TAU;
        let x = r * t.cos();
        let z = r * t.sin();
        let y = y_base
            + if y_spread > 0.0 {
                ((i as f32) % 7.0) / 7.0 * y_spread
            } else {
                0.0
            };
        pos.insert(id.clone(), Vec3::new(x, y, z));
    }
}
