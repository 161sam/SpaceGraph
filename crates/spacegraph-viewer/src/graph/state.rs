use bevy::prelude::{Resource, Vec3};
use spacegraph_core::{Delta, Edge, Msg, Node, NodeId};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::graph::explain::{self, PathStep};
use crate::graph::model::GraphModel;
use crate::graph::timeline::{BatchSpan, NodeLife, TimelineEvt, TimelineEvtKind};
use crate::net::Incoming;
use crate::util::ids::{node_label_long, node_label_short};

#[derive(Default)]
pub struct SpatialState {
    pub positions: HashMap<NodeId, Vec3>,
    pub velocities: HashMap<NodeId, Vec3>,

    pub in_batch: bool,
    pub touched_nodes: HashSet<NodeId>,
    pub touched_edges: HashSet<Edge>,
    pub glow_nodes: HashMap<NodeId, Instant>,
    pub glow_edges: HashMap<Edge, Instant>,
    pub last_batch_id: Option<u64>,

    pub active_vis_cache: Vec<NodeId>,
    pub progressive_cursor: usize,
    pub dirty_layout: bool,
}

#[derive(Default)]
pub struct TimelineState {
    pub window: Duration,
    pub scale: f32,
    pub pause: bool,
    pub frozen_now: Option<Instant>,
    pub scrub_seconds: f32,
    pub events: VecDeque<TimelineEvt>,
    pub max_events: usize,
    pub node_life: HashMap<NodeId, NodeLife>,
    pub batch_spans: VecDeque<BatchSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Spatial,
    Timeline,
}

#[derive(Default)]
pub struct UiState {
    pub filter: String,
    pub show_3d: bool,
    pub show_edges: bool,

    pub focus: Option<NodeId>,
    pub focus_hops: usize,

    pub hovered: Option<NodeId>,
    pub selected: Option<NodeId>,
    pub selected_a: Option<NodeId>,
    pub selected_b: Option<NodeId>,

    pub search_open: bool,
    pub search_query: String,
    pub search_hits: Vec<NodeId>,
    pub jump_to: Option<NodeId>,

    pub view_mode: ViewMode,
}

#[derive(Clone)]
pub struct ExplainCache {
    pub a: NodeId,
    pub b: NodeId,
    pub focus: Option<NodeId>,
    pub ts: Instant,
    pub result: Option<Vec<PathStep>>,
}

pub struct PerfState {
    pub fps: f32,
    pub event_rate: f32,
    pub visible_nodes: usize,
    pub visible_edges: usize,
    pub visible_raw_edges: usize,
    pub visible_agg_edges: usize,
    pub event_total: u64,
    pub ev_window: VecDeque<Instant>,
    pub gc_last_run: Instant,
}

impl Default for PerfState {
    fn default() -> Self {
        Self {
            fps: 0.0,
            event_rate: 0.0,
            visible_nodes: 0,
            visible_edges: 0,
            visible_raw_edges: 0,
            visible_agg_edges: 0,
            event_total: 0,
            ev_window: VecDeque::new(),
            gc_last_run: Instant::now(),
        }
    }
}

#[derive(Default)]
pub struct CfgState {
    pub layout_force: bool,
    pub link_distance: f32,
    pub repulsion: f32,
    pub damping: f32,
    pub max_step: f32,

    pub radius: f32,
    pub y_spread: f32,

    pub glow_duration: Duration,

    pub max_visible_nodes: usize,
    pub progressive_nodes_per_frame: usize,

    pub gc_enabled: bool,
    pub gc_ttl: Duration,
    pub gc_interval: Duration,

    pub show_raw_edges: bool,
    pub show_agg_edges: bool,
    pub explain_max_depth: usize,
}

#[derive(Resource)]
pub struct GraphState {
    pub model: GraphModel,
    pub spatial: SpatialState,
    pub timeline: TimelineState,
    pub ui: UiState,
    pub perf: PerfState,
    pub cfg: CfgState,
    pub explain_cache: Option<ExplainCache>,

    pub needs_redraw: AtomicBool,
}

impl Default for GraphState {
    fn default() -> Self {
        Self {
            model: GraphModel::default(),
            spatial: SpatialState {
                positions: HashMap::new(),
                velocities: HashMap::new(),
                in_batch: false,
                touched_nodes: HashSet::new(),
                touched_edges: HashSet::new(),
                glow_nodes: HashMap::new(),
                glow_edges: HashMap::new(),
                last_batch_id: None,
                active_vis_cache: Vec::new(),
                progressive_cursor: 0,
                dirty_layout: true,
            },
            timeline: TimelineState {
                window: Duration::from_secs(60),
                scale: 0.35,
                pause: false,
                frozen_now: None,
                scrub_seconds: 0.0,
                events: VecDeque::new(),
                max_events: 20_000,
                node_life: HashMap::new(),
                batch_spans: VecDeque::new(),
            },
            ui: UiState {
                filter: String::new(),
                show_3d: true,
                show_edges: true,
                focus: None,
                focus_hops: 2,
                hovered: None,
                selected: None,
                selected_a: None,
                selected_b: None,
                search_open: false,
                search_query: String::new(),
                search_hits: Vec::new(),
                jump_to: None,
                view_mode: ViewMode::Spatial,
            },
            perf: PerfState {
                fps: 0.0,
                event_rate: 0.0,
                visible_nodes: 0,
                visible_edges: 0,
                visible_raw_edges: 0,
                visible_agg_edges: 0,
                event_total: 0,
                ev_window: VecDeque::new(),
                gc_last_run: Instant::now(),
            },
            cfg: CfgState {
                layout_force: true,
                link_distance: 6.0,
                repulsion: 22.0,
                damping: 0.92,
                max_step: 0.35,
                radius: 25.0,
                y_spread: 6.0,
                glow_duration: Duration::from_millis(900),
                max_visible_nodes: 1200,
                progressive_nodes_per_frame: 250,
                gc_enabled: true,
                gc_ttl: Duration::from_secs(30),
                gc_interval: Duration::from_secs(1),
                show_raw_edges: false,
                show_agg_edges: true,
                explain_max_depth: 4,
            },
            needs_redraw: AtomicBool::new(true),
            explain_cache: None,
        }
    }
}

impl GraphState {
    pub fn clear(&mut self) {
        self.model.clear();
        self.spatial.positions.clear();
        self.spatial.velocities.clear();
        self.ui.focus = None;
        self.ui.hovered = None;
        self.ui.selected = None;
        self.ui.selected_a = None;
        self.ui.selected_b = None;

        self.ui.search_open = false;
        self.ui.search_query.clear();
        self.ui.search_hits.clear();
        self.ui.jump_to = None;

        self.spatial.glow_nodes.clear();
        self.spatial.glow_edges.clear();
        self.perf.ev_window.clear();
        self.perf.event_total = 0;

        self.timeline.events.clear();
        self.timeline.pause = false;
        self.timeline.frozen_now = None;
        self.timeline.scrub_seconds = 0.0;
        self.timeline.node_life.clear();
        self.timeline.batch_spans.clear();

        self.spatial.active_vis_cache.clear();
        self.spatial.progressive_cursor = 0;
        self.spatial.dirty_layout = true;
        self.explain_cache = None;

        self.needs_redraw.store(true, Ordering::Relaxed);
    }

    // ----- Apply incoming graph data -----
    pub fn apply(&mut self, inc: Incoming) {
        self.on_message();
        match inc {
            Incoming::Snapshot(Msg::Snapshot { nodes, edges }) => {
                let now = Instant::now();
                self.model.load_snapshot(nodes, edges, now);
                for id in self.model.nodes.keys() {
                    self.timeline.record_node_upsert(id, now);
                }
                self.mark_dirty_all();
            }
            Incoming::Event(Msg::Event { delta }) => self.apply_delta(delta),
            _ => {}
        }
    }

    fn apply_delta(&mut self, d: Delta) {
        let ts = Instant::now();
        match d {
            Delta::BatchBegin { id } => {
                self.spatial.in_batch = true;
                self.spatial.last_batch_id = Some(id);
                self.spatial.touched_nodes.clear();
                self.spatial.touched_edges.clear();
                self.push_timeline_at(ts, TimelineEvtKind::BatchBegin(id), None, None, None);
            }
            Delta::BatchEnd { id } => {
                self.spatial.in_batch = false;
                let until = ts + self.cfg.glow_duration;

                for idn in self.spatial.touched_nodes.drain() {
                    self.spatial.glow_nodes.insert(idn, until);
                }
                for e in self.spatial.touched_edges.drain() {
                    self.spatial.glow_edges.insert(e, until);
                }
                self.push_timeline_at(ts, TimelineEvtKind::BatchEnd(id), None, None, None);
                self.needs_redraw.store(true, Ordering::Relaxed);
            }
            Delta::UpsertNode { id, node } => {
                self.model.upsert_node(id.clone(), node, ts);
                self.spatial.dirty_layout = true;

                self.push_timeline_at(
                    ts,
                    TimelineEvtKind::NodeUpsert,
                    Some(id.clone()),
                    None,
                    None,
                );

                if self.spatial.in_batch {
                    self.spatial.touched_nodes.insert(id);
                } else {
                    self.spatial
                        .glow_nodes
                        .insert(id, ts + self.cfg.glow_duration);
                }
                self.needs_redraw.store(true, Ordering::Relaxed);
            }
            Delta::RemoveNode { id } => {
                let removed_edges = self.model.remove_node(&id);
                self.spatial.positions.remove(&id);
                self.spatial.velocities.remove(&id);
                self.spatial.glow_nodes.remove(&id);
                for edge in removed_edges {
                    self.spatial.glow_edges.remove(&edge);
                    self.spatial.touched_edges.remove(&edge);
                }

                if self.ui.focus.as_ref() == Some(&id) {
                    self.ui.focus = None;
                }
                if self.ui.selected.as_ref() == Some(&id) {
                    self.ui.selected = None;
                }
                if self.ui.selected_a.as_ref() == Some(&id) {
                    self.ui.selected_a = None;
                }
                if self.ui.selected_b.as_ref() == Some(&id) {
                    self.ui.selected_b = None;
                }
                if self.ui.hovered.as_ref() == Some(&id) {
                    self.ui.hovered = None;
                }

                self.push_timeline_at(
                    ts,
                    TimelineEvtKind::NodeRemove,
                    Some(id.clone()),
                    None,
                    None,
                );

                self.spatial.dirty_layout = true;
                if self.spatial.in_batch {
                    self.spatial.touched_nodes.insert(id);
                }
                self.needs_redraw.store(true, Ordering::Relaxed);
            }
            Delta::UpsertEdge { edge } => {
                self.model.upsert_edge(edge.clone(), ts);
                self.touch_node_at(&edge.from, ts);
                self.touch_node_at(&edge.to, ts);
                self.spatial.dirty_layout = true;

                self.push_timeline_at(
                    ts,
                    TimelineEvtKind::EdgeUpsert,
                    Some(edge.from.clone()),
                    Some(edge.to.clone()),
                    Some(edge.kind.clone()),
                );

                if self.spatial.in_batch {
                    self.spatial.touched_edges.insert(edge.clone());
                    self.spatial.touched_nodes.insert(edge.from.clone());
                    self.spatial.touched_nodes.insert(edge.to.clone());
                } else {
                    self.spatial
                        .glow_edges
                        .insert(edge.clone(), ts + self.cfg.glow_duration);
                }
                self.needs_redraw.store(true, Ordering::Relaxed);
            }
            Delta::RemoveEdge { edge } => {
                self.model.remove_edge(&edge);
                self.spatial.glow_edges.remove(&edge);

                self.push_timeline_at(
                    ts,
                    TimelineEvtKind::EdgeRemove,
                    Some(edge.from.clone()),
                    Some(edge.to.clone()),
                    Some(edge.kind.clone()),
                );

                self.needs_redraw.store(true, Ordering::Relaxed);
            }
        }
    }

    fn touch_node_at(&mut self, id: &NodeId, ts: Instant) {
        self.model.last_seen.insert(id.clone(), ts);
    }

    pub fn node_tooltip_lines(&self, id: &NodeId) -> Vec<String> {
        let Some(n) = self.model.nodes.get(id) else {
            return vec![id.0.clone()];
        };
        let mut out = Vec::new();
        out.push(format!("{} ({})", node_label_short(n), id.0));
        out.extend(node_label_long(n));
        out
    }

    // ---- Search helpers ----
    pub fn recompute_search_hits(&mut self, limit: usize) {
        self.ui.search_hits.clear();
        let q = self.ui.search_query.trim().to_lowercase();
        if q.is_empty() {
            return;
        }

        let mut hits: Vec<NodeId> = self
            .model
            .nodes
            .iter()
            .filter(|(id, n)| {
                let id_ok = id.0.to_lowercase().contains(&q);
                let node_ok = match n {
                    Node::File { path, .. } => path.to_lowercase().contains(&q),
                    Node::Process { cmdline, exe, .. } => {
                        cmdline.to_lowercase().contains(&q) || exe.to_lowercase().contains(&q)
                    }
                    Node::User { name, .. } => name.to_lowercase().contains(&q),
                };
                id_ok || node_ok
            })
            .map(|(id, _)| id.clone())
            .collect();

        hits.sort_by(|a, b| a.0.cmp(&b.0));
        hits.truncate(limit.max(1));
        self.ui.search_hits = hits;
    }

    pub fn request_jump(&mut self, id: NodeId) {
        self.ui.jump_to = Some(id);
    }

    // ---- Glow checks ----
    pub fn node_is_glowing(&self, id: &NodeId) -> bool {
        self.spatial.glow_nodes.contains_key(id)
    }
    pub fn edge_is_glowing(&self, e: &Edge) -> bool {
        self.spatial.glow_edges.contains_key(e)
    }

    pub fn explain_path_cached(
        &mut self,
        a: &NodeId,
        b: &NodeId,
        allowed: &HashSet<NodeId>,
    ) -> Option<Vec<PathStep>> {
        let now = Instant::now();
        let focus = self.ui.focus.clone();
        let ttl = Duration::from_millis(200);
        if let Some(cache) = &self.explain_cache {
            if cache.a == *a
                && cache.b == *b
                && cache.focus == focus
                && now.duration_since(cache.ts) <= ttl
            {
                return cache.result.clone();
            }
        }

        let result = explain::shortest_path(
            &self.model,
            a.clone(),
            b.clone(),
            self.cfg.explain_max_depth.max(1),
            allowed,
        );
        self.explain_cache = Some(ExplainCache {
            a: a.clone(),
            b: b.clone(),
            focus,
            ts: now,
            result: result.clone(),
        });
        result
    }

    pub fn node_label_with_id(&self, id: &NodeId) -> String {
        self.model
            .nodes
            .get(id)
            .map(|n| format!("{} ({})", node_label_short(n), id.0))
            .unwrap_or_else(|| id.0.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spacegraph_core::{FileKind, Node};

    #[test]
    fn search_returns_stable_sorted_hits_and_limit() {
        let mut st = GraphState::default();
        let a = NodeId("a-node".to_string());
        let b = NodeId("b-node".to_string());
        let c = NodeId("c-node".to_string());

        st.model.nodes.insert(
            b.clone(),
            Node::File {
                path: "/var/log/b.log".to_string(),
                inode: 2,
                kind: FileKind::Regular,
            },
        );
        st.model.nodes.insert(
            a.clone(),
            Node::File {
                path: "/var/log/a.log".to_string(),
                inode: 1,
                kind: FileKind::Regular,
            },
        );
        st.model.nodes.insert(
            c.clone(),
            Node::File {
                path: "/var/log/c.log".to_string(),
                inode: 3,
                kind: FileKind::Regular,
            },
        );

        st.ui.search_query = "log".to_string();
        st.recompute_search_hits(2);

        assert_eq!(st.ui.search_hits.len(), 2);
        assert_eq!(st.ui.search_hits[0].0, "a-node");
        assert_eq!(st.ui.search_hits[1].0, "b-node");
    }
}
