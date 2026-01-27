use spacegraph_core::{Node, NodeId};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::time::Instant;

use crate::graph::state::GraphState;

impl GraphState {
    // ----- Glow maintenance -----
    pub fn tick_glow(&mut self) {
        let now = Instant::now();
        let before_n = self.spatial.glow_nodes.len();
        let before_e = self.spatial.glow_edges.len();
        self.spatial.glow_nodes.retain(|_, until| *until > now);
        self.spatial.glow_edges.retain(|_, until| *until > now);
        if self.spatial.glow_nodes.len() != before_n || self.spatial.glow_edges.len() != before_e {
            self.needs_redraw.store(true, Ordering::Relaxed);
        }
    }

    // ----- GC orphan files -----
    pub fn tick_gc(&mut self) {
        if !self.cfg.gc_enabled {
            return;
        }
        let now = Instant::now();
        if now.duration_since(self.perf.gc_last_run) < self.cfg.gc_interval {
            return;
        }
        self.perf.gc_last_run = now;

        let mut degree: HashMap<NodeId, u32> = HashMap::new();
        for e in self.model.edges.iter() {
            *degree.entry(e.from.clone()).or_insert(0) += 1;
            *degree.entry(e.to.clone()).or_insert(0) += 1;
        }

        let mut to_remove: Vec<NodeId> = Vec::new();
        for (id, node) in self.model.nodes.iter() {
            let is_orphan = degree.get(id).copied().unwrap_or(0) == 0;
            if !is_orphan {
                continue;
            }
            if !matches!(node, Node::File { .. }) {
                continue;
            }
            let last = self.model.last_seen.get(id).copied().unwrap_or(now);
            if now.duration_since(last) >= self.cfg.gc_ttl {
                to_remove.push(id.clone());
            }
        }

        if to_remove.is_empty() {
            return;
        }

        for id in to_remove {
            self.model.nodes.remove(&id);
            self.spatial.positions.remove(&id);
            self.spatial.velocities.remove(&id);
            self.spatial.glow_nodes.remove(&id);
            self.model.last_seen.remove(&id);

            if self.ui.focus.as_ref() == Some(&id) {
                self.ui.focus = None;
            }
            if self.ui.selected.as_ref() == Some(&id) {
                self.ui.selected = None;
            }
            if self.ui.hovered.as_ref() == Some(&id) {
                self.ui.hovered = None;
            }
        }

        self.needs_redraw.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spacegraph_core::{FileKind, Node};
    use std::time::Duration;

    #[test]
    fn gc_removes_orphan_file_after_ttl() {
        let mut st = GraphState::default();
        let file_id = NodeId("file-1".to_string());
        st.model.nodes.insert(
            file_id.clone(),
            Node::File {
                path: "/tmp/test".to_string(),
                inode: 1,
                kind: FileKind::Regular,
            },
        );
        let now = Instant::now();
        st.model
            .last_seen
            .insert(file_id.clone(), now - Duration::from_secs(10));
        st.cfg.gc_ttl = Duration::from_secs(5);
        st.perf.gc_last_run = now - st.cfg.gc_interval - Duration::from_millis(1);

        st.tick_gc();

        assert!(!st.model.nodes.contains_key(&file_id));
    }
}
