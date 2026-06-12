use bevy::prelude::{Resource, Vec3};
use spacegraph_core::{
    id_file, id_process, id_user, Delta, Edge, EdgeKind, FileKind, Msg, Node, NodeId,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::graph::explain::{self, PathStep};
use crate::graph::grid::Grid;
use crate::graph::interner::{NodeIndex, NodeInterner};
use crate::graph::model::GraphModel;
use crate::graph::namespace;
use crate::graph::synthetic;
use crate::graph::timeline::{BatchSpan, NodeLife, TimelineEvt, TimelineEvtKind};
use crate::graph::tree;
use crate::net::{Incoming, IncomingKind, ReaderHandle};
use crate::util::config::{
    AgentEndpoint, AgentMode, LodEdgesMode, ViewerConfig, ViewerViewMode, VisualTheme,
};
use crate::util::ids::{node_label_long, node_label_short};

#[derive(Default)]
pub struct SpatialState {
    /// `NodeId` ⇄ dense `NodeIndex` projection for the viewer hot paths.
    pub interner: NodeInterner,
    /// Per-index spatial state, all indexed by `NodeIndex` and sized to
    /// `interner.capacity()`. Freed slots are cleared (see [`Self::release`]).
    pub positions: Vec<Vec3>,
    pub velocities: Vec<Vec3>,
    pub placed: Vec<bool>,
    pub glow_until: Vec<Option<Instant>>,

    /// Layout spring list: model edges resolved to index pairs, rebuilt only on
    /// topology change (`springs_dirty`) — never per frame.
    pub spring_edges: Vec<(NodeIndex, NodeIndex)>,
    pub springs_dirty: bool,

    /// Reused per-frame scratch buffers (avoid per-frame allocation/clones).
    pub forces: Vec<Vec3>,
    pub active: Vec<NodeIndex>,
    pub visible_mask: Vec<bool>,

    /// Visible set computed once per frame by the layout system and reused by
    /// the renderers (entity sync, edge/tooltip drawing) — avoids recomputing
    /// the capped projection multiple times per frame.
    pub vis_cache: HashSet<NodeId>,

    /// Uniform grid for neighbour-only repulsion + its reused candidate buffer.
    pub grid: Grid,
    pub grid_scratch: Vec<NodeIndex>,
    /// Resume cursor for budget-split repulsion passes (0 = start of a pass).
    pub repulsion_cursor: usize,

    pub in_batch: bool,
    pub touched_nodes: HashSet<NodeId>,
    pub touched_edges: HashSet<Edge>,
    pub glow_edges: HashMap<Edge, Instant>,
    pub last_batch_id: Option<u64>,

    pub active_vis_cache: Vec<NodeId>,
    pub progressive_cursor: usize,
    pub dirty_layout: bool,
    pub lod_active: bool,
    pub tree_dir_children: HashSet<NodeId>,
}

impl SpatialState {
    /// Grow per-index `Vec`s to cover every interner slot.
    fn ensure_capacity(&mut self) {
        let cap = self.interner.capacity();
        if self.positions.len() < cap {
            self.positions.resize(cap, Vec3::ZERO);
            self.velocities.resize(cap, Vec3::ZERO);
            self.placed.resize(cap, false);
            self.glow_until.resize(cap, None);
        }
    }

    /// Intern `id` and make sure its per-index storage exists.
    pub fn intern(&mut self, id: &NodeId) -> NodeIndex {
        let idx = self.interner.intern(id);
        self.ensure_capacity();
        idx
    }

    pub fn index_of(&self, id: &NodeId) -> Option<NodeIndex> {
        self.interner.index_of(id)
    }

    pub fn is_placed(&self, id: &NodeId) -> bool {
        self.index_of(id)
            .map(|idx| self.placed[idx.slot()])
            .unwrap_or(false)
    }

    pub fn position_of(&self, id: &NodeId) -> Option<Vec3> {
        let idx = self.index_of(id)?;
        if self.placed[idx.slot()] {
            Some(self.positions[idx.slot()])
        } else {
            None
        }
    }

    pub fn set_position(&mut self, idx: NodeIndex, pos: Vec3) {
        self.positions[idx.slot()] = pos;
        self.placed[idx.slot()] = true;
    }

    /// Clear all per-index state for a slot (called on release so reuse is safe).
    fn clear_slot(&mut self, idx: NodeIndex) {
        let i = idx.slot();
        self.positions[i] = Vec3::ZERO;
        self.velocities[i] = Vec3::ZERO;
        self.placed[i] = false;
        self.glow_until[i] = None;
    }

    /// Release a node, freeing its slot for reuse and clearing its state.
    pub fn release(&mut self, id: &NodeId) {
        if let Some(idx) = self.interner.release(id) {
            self.clear_slot(idx);
        }
    }

    pub fn set_node_glow(&mut self, id: &NodeId, until: Instant) {
        let idx = self.intern(id);
        self.glow_until[idx.slot()] = Some(until);
    }

    pub fn node_glow(&self, id: &NodeId) -> Option<Instant> {
        let idx = self.index_of(id)?;
        self.glow_until[idx.slot()]
    }

    pub fn is_glowing(&self, id: &NodeId) -> bool {
        self.node_glow(id).is_some()
    }

    /// Whether an index is currently placed and its node is in `vis`.
    pub fn index_visible(&self, idx: NodeIndex, vis: &HashSet<NodeId>) -> bool {
        self.placed.get(idx.slot()).copied().unwrap_or(false)
            && self
                .interner
                .resolve(idx)
                .map(|id| vis.contains(id))
                .unwrap_or(false)
    }

    /// Iterate placed nodes as `(id, position)` for rendering / picking.
    pub fn placed_positions(&self) -> impl Iterator<Item = (&NodeId, Vec3)> + '_ {
        self.interner.iter().filter_map(move |(idx, id)| {
            if self.placed[idx.slot()] {
                Some((id, self.positions[idx.slot()]))
            } else {
                None
            }
        })
    }

    /// Expire node glow past its deadline; returns whether anything changed.
    pub fn expire_node_glow(&mut self, now: Instant) -> bool {
        let mut changed = false;
        for slot in self.glow_until.iter_mut() {
            if matches!(*slot, Some(until) if until <= now) {
                *slot = None;
                changed = true;
            }
        }
        changed
    }

    /// Reset all index-keyed spatial state (interner, positions, springs).
    pub fn reset(&mut self) {
        self.interner.clear();
        self.positions.clear();
        self.velocities.clear();
        self.placed.clear();
        self.glow_until.clear();
        self.spring_edges.clear();
        self.springs_dirty = true;
        self.forces.clear();
        self.active.clear();
        self.visible_mask.clear();
        self.vis_cache.clear();
        self.grid.clear();
        self.grid_scratch.clear();
        self.repulsion_cursor = 0;
    }
}

#[derive(Default)]
pub struct TimelineState {
    pub window: Duration,
    pub scale: f32,
    pub pause: bool,
    pub frozen_now: Option<Instant>,
    pub scrub_seconds: f32,
    pub show_connectors: bool,
    pub events: VecDeque<TimelineEvt>,
    pub max_events: usize,
    pub node_life: HashMap<NodeId, NodeLife>,
    pub batch_spans: VecDeque<BatchSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Spatial,
    Tree,
    Timeline,
}

#[derive(Default)]
pub struct UiState {
    pub filter: String,
    pub show_3d: bool,
    pub show_edges: bool,
    pub help_open: bool,
    pub show_path_editor: bool,
    pub path_editor: PathEditorDraft,
    pub show_agent_manager: bool,
    pub show_agent_editor: bool,
    pub agent_editor: AgentEditorDraft,
    pub agent_command: AgentCommandDraft,

    pub focus: Option<NodeId>,
    pub focus_hops: usize,

    pub hovered: Option<NodeId>,
    pub selected: Option<NodeId>,
    pub selected_a: Option<NodeId>,
    pub selected_b: Option<NodeId>,
    /// Multi-selection from box-select (drag rectangle).
    pub multi_selected: HashSet<NodeId>,

    pub search_open: bool,
    pub search_query: String,
    pub search_hits: Vec<NodeId>,
    pub jump_to: Option<NodeId>,
    pub fit_to_view: bool,

    pub view_mode: ViewMode,
    pub tree_collapsed: HashSet<NodeId>,
    pub tree_expanded: HashSet<NodeId>,
    pub tree_show_files: bool,
    pub tree_zoom: f32,
    pub tree_file_zoom_threshold: f32,
    pub tree_center: Vec3,
    pub tree_default_expand_depth: usize,
}

#[derive(Default, Clone)]
pub struct PathEditorDraft {
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub include_input: String,
    pub exclude_input: String,
    pub include_notice: Option<String>,
    pub exclude_notice: Option<String>,
}

#[derive(Default, Clone)]
pub struct AgentEditorDraft {
    pub name_input: String,
    pub uds_input: String,
    pub auto_connect: bool,
    pub mode_override: Option<AgentMode>,
    pub notice: Option<String>,
}

#[derive(Default, Clone)]
pub struct AgentCommandDraft {
    pub open: bool,
    pub target: Option<String>,
}

impl PathEditorDraft {
    pub fn from_cfg(cfg: &CfgState) -> Self {
        Self {
            includes: cfg.path_includes.clone(),
            excludes: cfg.path_excludes.clone(),
            include_input: String::new(),
            exclude_input: String::new(),
            include_notice: None,
            exclude_notice: None,
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetStreamStatus {
    Disconnected,
    Connecting,
    Connected,
}

pub struct NetStreamState {
    pub status: NetStreamStatus,
    /// Whether this stream's subgraph is shown. Disabling hides exactly this
    /// stream's nodes/edges (data retained); re-enabling restores them.
    pub enabled: bool,
    /// Origin identity reported by the agent's `Identity` message (host etc.),
    /// shown in tooltips. The namespace key is the stream name itself.
    pub origin_host: Option<String>,
    pub last_msg: Option<Instant>,
    pub last_seen: Option<Instant>,
    pub last_snapshot_at: Option<Instant>,
    pub last_event_at: Option<Instant>,
    pub msg_rate: f32,
    pub msg_window: VecDeque<Instant>,
    pub last_error: Option<String>,
}

pub struct NetState {
    pub endpoints: Vec<AgentEndpoint>,
    pub streams: HashMap<String, NetStreamState>,
    pub connections: HashMap<String, ReaderHandle>,
    pub msg_window: Duration,
    pub commands: Vec<NetCommand>,
}

pub enum NetCommand {
    Connect(String),
    Disconnect(String),
    Reconnect(String),
}

impl Default for NetStreamState {
    fn default() -> Self {
        Self::new()
    }
}

impl NetStreamState {
    pub fn new() -> Self {
        Self {
            status: NetStreamStatus::Disconnected,
            enabled: true,
            origin_host: None,
            last_msg: None,
            last_seen: None,
            last_snapshot_at: None,
            last_event_at: None,
            msg_rate: 0.0,
            msg_window: VecDeque::new(),
            last_error: None,
        }
    }
}

impl Default for NetState {
    fn default() -> Self {
        Self {
            endpoints: Vec::new(),
            streams: HashMap::new(),
            connections: HashMap::new(),
            msg_window: Duration::from_secs(2),
            commands: Vec::new(),
        }
    }
}

impl NetState {
    pub fn endpoint_names(&self) -> HashSet<String> {
        self.endpoints.iter().map(|e| e.name.clone()).collect()
    }

    pub fn is_configured(&self, name: &str) -> bool {
        self.endpoints.iter().any(|e| e.name == name)
    }

    pub fn ensure_stream(&mut self, name: &str) {
        self.streams.entry(name.to_string()).or_default();
    }

    pub fn active_connection_count(&self) -> usize {
        self.connections.len()
    }
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
    /// Repulsion cutoff radius / grid cell size. Drives the candidate count per
    /// node (≈ 27 · (radius / spacing)³); kept at ~1.5 × link_distance so the
    /// grid pass stays well inside the per-frame budget.
    pub repulsion_radius: f32,
    pub damping: f32,
    pub max_step: f32,
    /// Per-frame layout time budget in ms; ≤ 0 means unbounded (full step).
    pub layout_budget_ms: f32,

    pub radius: f32,
    pub y_spread: f32,

    pub glow_duration: Duration,

    pub max_visible_nodes: usize,
    pub progressive_nodes_per_frame: usize,
    /// Cap on retained alert nodes; oldest evicted past this (default 200).
    pub max_visible_alerts: usize,

    pub gc_enabled: bool,
    pub gc_ttl: Duration,
    pub gc_interval: Duration,

    pub show_raw_edges: bool,
    pub show_agg_edges: bool,
    pub explain_max_depth: usize,

    pub lod_enabled: bool,
    pub lod_threshold_nodes: usize,
    pub lod_edges_mode: LodEdgesMode,

    pub demo_mode: bool,
    pub path_includes: Vec<String>,
    pub path_excludes: Vec<String>,
    pub agent_default_mode: AgentMode,
    pub visual_theme: VisualTheme,
    /// Fog-of-war: when on, only revealed (explored) nodes render; placement and
    /// layout still run on the full projection so nodes can be revealed.
    pub fog_of_war: bool,
}

impl CfgState {
    pub fn lod_active(&self, visible_nodes: usize) -> bool {
        self.lod_enabled && visible_nodes >= self.lod_threshold_nodes
    }
}

#[derive(Resource)]
pub struct GraphState {
    pub model: GraphModel,
    pub spatial: SpatialState,
    pub timeline: TimelineState,
    pub ui: UiState,
    pub perf: PerfState,
    pub net: NetState,
    pub cfg: CfgState,
    pub explain_cache: Option<ExplainCache>,
    /// Insertion order of retained alert nodes (oldest first) for cap eviction.
    pub alert_order: VecDeque<NodeId>,
    /// Nodes revealed by exploration (camera proximity, scan, focus) — the
    /// fog-of-war render gate. Independent of placement/layout.
    pub revealed: HashSet<NodeId>,
    pub snapshot_loaded: bool,
    pub live_events_seen: bool,
    pub demo_loaded: bool,

    pub needs_redraw: AtomicBool,
}

impl From<ViewerViewMode> for ViewMode {
    fn from(mode: ViewerViewMode) -> Self {
        match mode {
            ViewerViewMode::Spatial => ViewMode::Spatial,
            ViewerViewMode::Tree => ViewMode::Tree,
            ViewerViewMode::Timeline => ViewMode::Timeline,
        }
    }
}

impl From<ViewMode> for ViewerViewMode {
    fn from(mode: ViewMode) -> Self {
        match mode {
            ViewMode::Spatial => ViewerViewMode::Spatial,
            ViewMode::Tree => ViewerViewMode::Tree,
            ViewMode::Timeline => ViewerViewMode::Timeline,
        }
    }
}

impl Default for GraphState {
    fn default() -> Self {
        Self {
            model: GraphModel::default(),
            spatial: SpatialState {
                dirty_layout: true,
                springs_dirty: true,
                ..Default::default()
            },
            timeline: TimelineState {
                window: Duration::from_secs(60),
                scale: 0.35,
                pause: false,
                frozen_now: None,
                scrub_seconds: 0.0,
                show_connectors: true,
                events: VecDeque::new(),
                max_events: 20_000,
                node_life: HashMap::new(),
                batch_spans: VecDeque::new(),
            },
            ui: UiState {
                filter: String::new(),
                show_3d: true,
                show_edges: true,
                help_open: false,
                show_path_editor: false,
                path_editor: PathEditorDraft::default(),
                show_agent_manager: false,
                show_agent_editor: false,
                agent_editor: AgentEditorDraft::default(),
                agent_command: AgentCommandDraft::default(),
                focus: None,
                focus_hops: 2,
                hovered: None,
                selected: None,
                selected_a: None,
                selected_b: None,
                multi_selected: HashSet::new(),
                search_open: false,
                search_query: String::new(),
                search_hits: Vec::new(),
                jump_to: None,
                fit_to_view: false,
                view_mode: ViewMode::Spatial,
                tree_collapsed: HashSet::new(),
                tree_expanded: HashSet::new(),
                tree_show_files: false,
                tree_zoom: 0.0,
                tree_file_zoom_threshold: 0.05,
                tree_center: Vec3::ZERO,
                tree_default_expand_depth: 2,
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
            net: NetState::default(),
            cfg: CfgState {
                layout_force: true,
                link_distance: 6.0,
                repulsion: 400.0,
                repulsion_radius: 8.0,
                damping: 0.92,
                max_step: 0.35,
                layout_budget_ms: 6.0,
                radius: 25.0,
                y_spread: 6.0,
                glow_duration: Duration::from_millis(900),
                max_visible_nodes: 3000,
                progressive_nodes_per_frame: 250,
                max_visible_alerts: 200,
                gc_enabled: true,
                gc_ttl: Duration::from_secs(30),
                gc_interval: Duration::from_secs(1),
                show_raw_edges: false,
                show_agg_edges: true,
                explain_max_depth: 4,
                lod_enabled: true,
                lod_threshold_nodes: 2500,
                lod_edges_mode: LodEdgesMode::FocusOnly,
                demo_mode: false,
                path_includes: vec!["/etc".to_string(), "/home".to_string(), "/var".to_string()],
                path_excludes: vec![
                    "/proc".to_string(),
                    "/sys".to_string(),
                    "/dev".to_string(),
                    "/run".to_string(),
                ],
                agent_default_mode: AgentMode::User,
                visual_theme: VisualTheme::Standard,
                fog_of_war: false,
            },
            needs_redraw: AtomicBool::new(true),
            explain_cache: None,
            alert_order: VecDeque::new(),
            revealed: HashSet::new(),
            snapshot_loaded: false,
            live_events_seen: false,
            demo_loaded: false,
        }
    }
}

impl GraphState {
    pub fn clear(&mut self) {
        self.model.clear();
        self.spatial.reset();
        self.ui.focus = None;
        self.ui.hovered = None;
        self.ui.selected = None;
        self.ui.selected_a = None;
        self.ui.selected_b = None;
        self.ui.multi_selected.clear();

        self.ui.search_open = false;
        self.ui.search_query.clear();
        self.ui.search_hits.clear();
        self.ui.jump_to = None;
        self.ui.fit_to_view = false;
        self.ui.help_open = false;

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
        self.alert_order.clear();
        self.revealed.clear();
        self.snapshot_loaded = false;
        self.live_events_seen = false;
        self.demo_loaded = false;

        self.needs_redraw.store(true, Ordering::Relaxed);
    }

    pub fn open_path_editor(&mut self) {
        let draft = PathEditorDraft::from_cfg(&self.cfg);
        self.ui.path_editor = draft;
        self.ui.show_path_editor = true;
    }

    pub fn set_demo_mode(&mut self, enabled: bool) {
        if enabled == self.cfg.demo_mode {
            return;
        }

        if enabled {
            if self.net.active_connection_count() > 0 {
                self.cfg.demo_mode = false;
                return;
            }
            if !self.model.nodes.is_empty() && !self.demo_loaded {
                self.cfg.demo_mode = false;
                return;
            }
            self.cfg.demo_mode = true;
            if !self.demo_loaded {
                self.load_demo_graph();
            }
        } else {
            self.cfg.demo_mode = false;
            if self.demo_loaded {
                self.clear();
            }
        }
    }

    pub fn ensure_demo_graph(&mut self) {
        if !self.cfg.demo_mode {
            return;
        }

        if self.net.active_connection_count() > 0 {
            self.set_demo_mode(false);
            return;
        }

        if !self.demo_loaded {
            if !self.model.nodes.is_empty() {
                self.cfg.demo_mode = false;
                return;
            }
            self.load_demo_graph();
        }
    }

    fn load_demo_graph(&mut self) {
        self.clear();
        let now = Instant::now();
        let node_id = "demo";

        let user = id_user(node_id, 1000);
        let proc_a = id_process(node_id, 4242);
        let proc_b = id_process(node_id, 4243);
        let file_a = id_file(node_id, "/home/demo/report.txt");
        let file_b = id_file(node_id, "/var/log/demo.log");
        let file_c = id_file(node_id, "/usr/bin/demo-app");

        let nodes = vec![
            (
                user.clone(),
                Node::User {
                    uid: 1000,
                    name: "demo".to_string(),
                },
            ),
            (
                proc_a.clone(),
                Node::Process {
                    pid: 4242,
                    ppid: 1,
                    exe: "/usr/bin/demo-app".to_string(),
                    cmdline: "/usr/bin/demo-app --demo".to_string(),
                    uid: 1000,
                },
            ),
            (
                proc_b.clone(),
                Node::Process {
                    pid: 4243,
                    ppid: 4242,
                    exe: "/usr/bin/demo-helper".to_string(),
                    cmdline: "/usr/bin/demo-helper --child".to_string(),
                    uid: 1000,
                },
            ),
            (
                file_a.clone(),
                Node::File {
                    path: "/home/demo/report.txt".to_string(),
                    inode: 1001,
                    kind: FileKind::Regular,
                },
            ),
            (
                file_b.clone(),
                Node::File {
                    path: "/var/log/demo.log".to_string(),
                    inode: 1002,
                    kind: FileKind::Regular,
                },
            ),
            (
                file_c.clone(),
                Node::File {
                    path: "/usr/bin/demo-app".to_string(),
                    inode: 1003,
                    kind: FileKind::Regular,
                },
            ),
        ];

        let edges = vec![
            Edge {
                from: proc_a.clone(),
                to: file_a.clone(),
                kind: EdgeKind::Opens {
                    fd: 3,
                    mode: "rw".to_string(),
                },
            },
            Edge {
                from: proc_a.clone(),
                to: file_b.clone(),
                kind: EdgeKind::Opens {
                    fd: 4,
                    mode: "w".to_string(),
                },
            },
            Edge {
                from: proc_b.clone(),
                to: file_b.clone(),
                kind: EdgeKind::Opens {
                    fd: 5,
                    mode: "r".to_string(),
                },
            },
            Edge {
                from: proc_a.clone(),
                to: user.clone(),
                kind: EdgeKind::RunsAs,
            },
            Edge {
                from: proc_b.clone(),
                to: user.clone(),
                kind: EdgeKind::RunsAs,
            },
        ];

        self.model.load_snapshot(nodes, edges, now);
        let node_ids: Vec<_> = self.model.nodes.keys().cloned().collect();
        for id in node_ids {
            self.push_timeline_at(now, TimelineEvtKind::NodeUpsert, Some(id), None, None);
        }
        let edges: Vec<_> = self.model.edges.iter().cloned().collect();
        for edge in edges {
            self.push_timeline_at(
                now,
                TimelineEvtKind::EdgeUpsert,
                Some(edge.from),
                Some(edge.to),
                Some(edge.kind),
            );
        }

        self.demo_loaded = true;
        self.spatial.dirty_layout = true;
        self.needs_redraw.store(true, Ordering::Relaxed);
    }

    /// Seed a deterministic synthetic graph of `n` nodes (~`2n` edges).
    ///
    /// Used by the `--demo-load <n>` CLI flag and by benchmarks. Clears any
    /// existing graph first. Not gated by `demo_mode`: this is an explicit
    /// developer/bench load, not the interactive demo toggle.
    pub fn load_synthetic_graph(&mut self, n: usize) {
        self.clear();
        let now = Instant::now();
        let (nodes, edges) = synthetic::synthetic_graph(n);
        self.model.load_snapshot(nodes, edges, now);
        for id in self.model.nodes.keys() {
            self.timeline.record_node_upsert(id, now);
        }
        self.snapshot_loaded = true;
        self.spatial.dirty_layout = true;
        self.needs_redraw.store(true, Ordering::Relaxed);
    }

    // ----- Apply incoming graph data -----
    pub fn apply(&mut self, inc: Incoming) {
        if !self.net.is_configured(&inc.stream) {
            match inc.kind {
                IncomingKind::Disconnected => {
                    self.net_on_disconnected(&inc.stream);
                }
                IncomingKind::Error(msg) => {
                    self.net_on_error(&inc.stream, msg);
                }
                _ => {}
            }
            return;
        }
        match inc.kind {
            IncomingKind::Connected => {
                self.net_on_connected(inc.stream);
            }
            IncomingKind::Disconnected => {
                self.net_on_disconnected(&inc.stream);
            }
            IncomingKind::Error(msg) => {
                self.net_on_error(&inc.stream, msg);
            }
            IncomingKind::Snapshot(Msg::Snapshot { nodes, edges }) => {
                self.on_message();
                self.net_on_message(&inc.stream);
                let now = Instant::now();
                self.net_on_snapshot(&inc.stream, now);
                // Replace only THIS stream's subgraph (namespaced) — never merge
                // across streams.
                self.remove_stream(&inc.stream);
                for (id, node) in nodes {
                    let gid = namespace::globalize(&inc.stream, &id);
                    self.model.upsert_node(gid.clone(), node, now);
                    self.timeline.record_node_upsert(&gid, now);
                }
                for edge in edges {
                    self.model
                        .upsert_edge(namespace::globalize_edge(&inc.stream, &edge), now);
                }
                self.snapshot_loaded = true;
                self.mark_dirty_all();
            }

            IncomingKind::Event(Msg::Event { delta }) => {
                self.on_message();
                self.net_on_message(&inc.stream);
                self.net_on_event(&inc.stream);
                let delta = Self::globalize_delta(&inc.stream, delta);
                self.apply_delta(delta);
            }
            IncomingKind::Identity(msg) => {
                self.on_message();
                self.net_on_message(&inc.stream);
                if let Msg::Identity { ident, .. } = &msg {
                    if let Some(s) = self.net.streams.get_mut(&inc.stream) {
                        s.origin_host = Some(ident.hostname.clone());
                    }
                }
            }
            IncomingKind::Other(_) => {
                self.on_message();
                self.net_on_message(&inc.stream);
            }

            _ => {}
        }
    }

    /// Namespace a delta's node/edge ids by the originating stream so two
    /// streams with colliding local ids never merge.
    fn globalize_delta(stream: &str, d: Delta) -> Delta {
        match d {
            Delta::UpsertNode { id, node } => Delta::UpsertNode {
                id: namespace::globalize(stream, &id),
                node,
            },
            Delta::RemoveNode { id } => Delta::RemoveNode {
                id: namespace::globalize(stream, &id),
            },
            Delta::UpsertEdge { edge } => Delta::UpsertEdge {
                edge: namespace::globalize_edge(stream, &edge),
            },
            Delta::RemoveEdge { edge } => Delta::RemoveEdge {
                edge: namespace::globalize_edge(stream, &edge),
            },
            other => other,
        }
    }

    /// Remove an entire stream's subgraph from the model and spatial state.
    fn remove_stream(&mut self, stream: &str) {
        let prefix = namespace::prefix(stream);
        let ids: Vec<NodeId> = self
            .model
            .nodes
            .keys()
            .filter(|id| id.0.starts_with(&prefix))
            .cloned()
            .collect();
        if ids.is_empty() {
            return;
        }
        for id in ids {
            self.model.remove_node(&id);
            self.spatial.release(&id);
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
        }
        self.spatial.springs_dirty = true;
        self.mark_dirty_all();
    }

    /// Whether the node's origin stream is enabled (non-namespaced nodes — demo
    /// / synthetic — are always shown).
    pub fn stream_enabled(&self, id: &NodeId) -> bool {
        match namespace::origin(id) {
            Some(stream) => self
                .net
                .streams
                .get(stream)
                .map(|s| s.enabled)
                .unwrap_or(true),
            None => true,
        }
    }

    /// Toggle a stream's visibility (hides/restores its subgraph without losing
    /// data).
    pub fn set_stream_enabled(&mut self, stream: &str, enabled: bool) {
        if let Some(s) = self.net.streams.get_mut(stream) {
            if s.enabled != enabled {
                s.enabled = enabled;
                self.mark_dirty_all();
            }
        }
    }

    /// Track a new alert node; evict the oldest past `max_visible_alerts`.
    fn note_alert(&mut self, id: NodeId) {
        if self.alert_order.contains(&id) {
            return;
        }
        self.alert_order.push_back(id);
        let cap = self.cfg.max_visible_alerts.max(1);
        while self.alert_order.len() > cap {
            if let Some(old) = self.alert_order.pop_front() {
                self.model.remove_node(&old);
                self.spatial.release(&old);
            }
        }
    }

    /// Count current alerts by severity (for the Alerts panel).
    pub fn alert_severity_counts(&self) -> (usize, usize, usize) {
        let (mut low, mut med, mut high) = (0, 0, 0);
        for id in &self.alert_order {
            if let Some(Node::Alert { severity, .. }) = self.model.nodes.get(id) {
                match severity.as_str() {
                    "low" => low += 1,
                    "medium" => med += 1,
                    _ => high += 1,
                }
            }
        }
        (low, med, high)
    }

    /// Alert node ids, newest first (for the Alerts panel list).
    pub fn alerts_newest_first(&self) -> impl Iterator<Item = &NodeId> {
        self.alert_order.iter().rev()
    }

    /// Fog-of-war render gate: fog off → always shown; fog on → only revealed
    /// nodes, plus alerts and the active focus/selection/hover.
    pub fn is_visible_rendered(&self, id: &NodeId) -> bool {
        if !self.cfg.fog_of_war
            || self.revealed.contains(id)
            || matches!(self.model.nodes.get(id), Some(Node::Alert { .. }))
        {
            return true;
        }
        self.ui.focus.as_ref() == Some(id)
            || self.ui.selected.as_ref() == Some(id)
            || self.ui.hovered.as_ref() == Some(id)
    }

    /// Mark a node as explored (fog-of-war).
    pub fn reveal(&mut self, id: &NodeId) {
        if self.revealed.insert(id.clone()) {
            self.needs_redraw.store(true, Ordering::Relaxed);
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

                let touched: Vec<NodeId> = self.spatial.touched_nodes.drain().collect();
                for idn in touched {
                    self.spatial.set_node_glow(&idn, until);
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

                let is_alert = matches!(self.model.nodes.get(&id), Some(Node::Alert { .. }));
                if matches!(self.model.nodes.get(&id), Some(Node::File { .. })) {
                    self.note_path_change(&id, ts);
                } else if self.spatial.in_batch {
                    self.spatial.touched_nodes.insert(id.clone());
                } else {
                    let until = ts + self.cfg.glow_duration;
                    self.spatial.set_node_glow(&id, until);
                }
                if is_alert {
                    self.note_alert(id);
                }
                self.needs_redraw.store(true, Ordering::Relaxed);
            }
            Delta::RemoveNode { id } => {
                let removed_edges = self.model.remove_node(&id);
                self.spatial.release(&id);
                self.spatial.springs_dirty = true;
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
                self.ui.tree_collapsed.remove(&id);
                self.ui.tree_expanded.remove(&id);

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
                self.spatial.springs_dirty = true;
                self.note_path_change(&edge.from, ts);
                self.note_path_change(&edge.to, ts);

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
                self.spatial.springs_dirty = true;

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

    fn note_path_change(&mut self, id: &NodeId, ts: Instant) {
        let Some(Node::File { .. }) = self.model.nodes.get(id) else {
            return;
        };
        let mut ids = vec![id.clone()];
        ids.extend(self.file_ancestor_ids(id));

        if self.spatial.in_batch {
            for nid in ids {
                self.spatial.touched_nodes.insert(nid);
            }
        } else {
            let until = ts + self.cfg.glow_duration;
            for nid in ids {
                self.spatial.set_node_glow(&nid, until);
            }
        }
    }

    fn file_ancestor_ids(&self, id: &NodeId) -> Vec<NodeId> {
        let Some(Node::File { path, .. }) = self.model.nodes.get(id) else {
            return Vec::new();
        };
        let Some(prefix) = id.0.split_once(":file:").map(|(prefix, _)| prefix) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for parent in tree::ancestor_paths(path) {
            let ancestor_id = NodeId(format!("{prefix}:file:{parent}"));
            if self.model.nodes.contains_key(&ancestor_id) {
                out.push(ancestor_id);
            }
        }
        out
    }

    fn net_on_connected(&mut self, stream: String) {
        self.set_demo_mode(false);
        let now = Instant::now();
        let entry = self.net.streams.entry(stream.clone()).or_default();
        entry.status = NetStreamStatus::Connected;
        entry.last_msg = None;
        entry.msg_window.clear();
        entry.msg_rate = 0.0;
        entry.last_seen = Some(now);
        entry.last_snapshot_at = None;
        entry.last_event_at = None;
        entry.last_error = None;
    }

    fn net_on_disconnected(&mut self, stream: &str) {
        if let Some(entry) = self.net.streams.get_mut(stream) {
            entry.status = NetStreamStatus::Disconnected;
        }
        self.net.connections.remove(stream);
    }

    fn net_on_error(&mut self, stream: &str, msg: String) {
        if let Some(entry) = self.net.streams.get_mut(stream) {
            entry.status = NetStreamStatus::Disconnected;
            entry.last_error = Some(msg);
        }
        self.net.connections.remove(stream);
    }

    fn net_on_message(&mut self, stream: &str) {
        self.set_demo_mode(false);
        let now = Instant::now();
        let window = self.net.msg_window;
        let entry = self.net.streams.entry(stream.to_string()).or_default();
        entry.status = NetStreamStatus::Connected;
        entry.last_msg = Some(now);
        entry.last_seen = Some(now);
        entry.last_error = None;
        entry.msg_window.push_back(now);
        Self::net_prune_stream(entry, now, window);
    }

    fn net_on_snapshot(&mut self, stream: &str, now: Instant) {
        let entry = self.net.streams.entry(stream.to_string()).or_default();
        entry.last_snapshot_at = Some(now);
    }

    fn net_on_event(&mut self, stream: &str) {
        let entry = self.net.streams.entry(stream.to_string()).or_default();
        entry.last_event_at = Some(Instant::now());
    }

    fn net_prune_stream(stream: &mut NetStreamState, now: Instant, window: Duration) {
        while let Some(front) = stream.msg_window.front() {
            if now.duration_since(*front) > window {
                stream.msg_window.pop_front();
            } else {
                break;
            }
        }
        stream.msg_rate = stream.msg_window.len() as f32 / window.as_secs_f32();
    }

    pub fn node_tooltip_lines(&self, id: &NodeId) -> Vec<String> {
        let Some(n) = self.model.nodes.get(id) else {
            return vec![id.0.clone()];
        };
        let mut out = Vec::new();
        out.push(format!(
            "{} ({})",
            node_label_short(n),
            namespace::local_part(id)
        ));
        out.extend(node_label_long(n));
        if let Some(stream) = namespace::origin(id) {
            match self
                .net
                .streams
                .get(stream)
                .and_then(|s| s.origin_host.clone())
            {
                Some(host) => out.push(format!("origin: {stream} ({host})")),
                None => out.push(format!("origin: {stream}")),
            }
        }
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
                    Node::Socket {
                        proto, local_addr, ..
                    } => {
                        proto.to_lowercase().contains(&q) || local_addr.to_lowercase().contains(&q)
                    }
                    Node::RemoteHost { addr, rdns } => {
                        addr.to_lowercase().contains(&q)
                            || rdns
                                .as_deref()
                                .is_some_and(|r| r.to_lowercase().contains(&q))
                    }
                    Node::Alert {
                        signature,
                        severity,
                        ..
                    } => {
                        signature.to_lowercase().contains(&q)
                            || severity.to_lowercase().contains(&q)
                    }
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
        self.spatial.is_glowing(id)
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

    pub fn toggle_tree_dir(&mut self, id: &NodeId) -> bool {
        let Some(Node::File { path, kind, .. }) = self.model.nodes.get(id) else {
            return false;
        };
        if !matches!(kind, FileKind::Dir) {
            return false;
        }
        let depth = tree::path_depth(path);
        let expanded = self.tree_dir_is_expanded_depth(id, depth);
        if expanded {
            self.ui.tree_collapsed.insert(id.clone());
            self.ui.tree_expanded.remove(id);
        } else {
            self.ui.tree_collapsed.remove(id);
            self.ui.tree_expanded.insert(id.clone());
        }
        self.spatial.dirty_layout = true;
        self.needs_redraw.store(true, Ordering::Relaxed);
        true
    }

    pub fn tree_dir_is_expanded(&self, id: &NodeId) -> bool {
        let Some(Node::File { path, kind, .. }) = self.model.nodes.get(id) else {
            return false;
        };
        if !matches!(kind, FileKind::Dir) {
            return false;
        }
        let depth = tree::path_depth(path);
        self.tree_dir_is_expanded_depth(id, depth)
    }

    pub(crate) fn tree_dir_is_expanded_depth(&self, id: &NodeId, depth: usize) -> bool {
        if self.ui.tree_collapsed.contains(id) {
            return false;
        }
        if self.ui.tree_expanded.contains(id) {
            return true;
        }
        depth <= self.ui.tree_default_expand_depth
    }

    fn sync_agent_endpoints(&mut self, endpoints: Vec<AgentEndpoint>) {
        let previous = self.net.endpoint_names();
        let next: HashSet<String> = endpoints.iter().map(|e| e.name.clone()).collect();

        for removed in previous.difference(&next) {
            if self.net.connections.contains_key(removed) {
                self.net
                    .commands
                    .push(NetCommand::Disconnect((*removed).to_string()));
            }
            self.net.connections.remove(removed);
            self.net.streams.remove(removed);
        }

        self.net.endpoints = endpoints;
        let names: Vec<String> = self
            .net
            .endpoints
            .iter()
            .map(|endpoint| endpoint.name.clone())
            .collect();
        for name in names {
            self.net.ensure_stream(&name);
        }
    }

    pub fn apply_viewer_config(&mut self, cfg: &ViewerConfig) {
        self.ui.view_mode = cfg.view_mode.into();
        self.ui.show_3d = cfg.show_3d;
        self.ui.show_edges = cfg.show_edges;
        self.ui.focus_hops = cfg.focus_hops.max(1);
        self.cfg.show_raw_edges = cfg.show_raw_edges;
        self.cfg.show_agg_edges = cfg.show_agg_edges;
        self.cfg.max_visible_nodes = cfg.max_visible_nodes.max(1);
        self.cfg.max_visible_alerts = cfg.max_visible_alerts.max(1);
        self.cfg.progressive_nodes_per_frame = cfg.progressive_nodes_per_frame.max(1);
        self.cfg.layout_force = cfg.layout_force;
        self.cfg.link_distance = cfg.link_distance;
        self.cfg.repulsion = cfg.repulsion;
        self.cfg.repulsion_radius = cfg.repulsion_radius;
        self.cfg.damping = cfg.damping;
        self.cfg.max_step = cfg.max_step;
        self.cfg.layout_budget_ms = cfg.layout_budget_ms;
        self.timeline.window = Duration::from_secs(cfg.timeline_window_secs.max(1));
        self.timeline.scale = cfg.timeline_scale.max(0.01);
        self.cfg.lod_enabled = cfg.lod_enabled;
        self.cfg.lod_threshold_nodes = cfg.lod_threshold_nodes.max(1);
        self.cfg.lod_edges_mode = cfg.lod_edges_mode;
        self.cfg.glow_duration = Duration::from_millis(cfg.glow_duration_ms.max(1));
        self.cfg.gc_enabled = cfg.gc_enabled;
        self.cfg.gc_ttl = Duration::from_secs(cfg.gc_ttl_secs.max(1));
        self.set_demo_mode(cfg.demo_mode);
        self.cfg.path_includes = cfg.path_includes.clone();
        self.cfg.path_excludes = cfg.path_excludes.clone();
        self.cfg.agent_default_mode = cfg.default_agent_mode;
        self.cfg.visual_theme = cfg.visual_theme;
        self.cfg.fog_of_war = cfg.fog_of_war;
        self.sync_agent_endpoints(cfg.agents.clone());

        self.needs_redraw.store(true, Ordering::Relaxed);
    }

    pub fn viewer_config(&self) -> ViewerConfig {
        ViewerConfig {
            view_mode: self.ui.view_mode.into(),
            show_3d: self.ui.show_3d,
            show_edges: self.ui.show_edges,
            show_raw_edges: self.cfg.show_raw_edges,
            show_agg_edges: self.cfg.show_agg_edges,
            demo_mode: self.cfg.demo_mode,
            path_includes: self.cfg.path_includes.clone(),
            path_excludes: self.cfg.path_excludes.clone(),
            focus_hops: self.ui.focus_hops,
            max_visible_nodes: self.cfg.max_visible_nodes,
            max_visible_alerts: self.cfg.max_visible_alerts,
            progressive_nodes_per_frame: self.cfg.progressive_nodes_per_frame,
            layout_force: self.cfg.layout_force,
            link_distance: self.cfg.link_distance,
            repulsion: self.cfg.repulsion,
            repulsion_radius: self.cfg.repulsion_radius,
            damping: self.cfg.damping,
            max_step: self.cfg.max_step,
            layout_budget_ms: self.cfg.layout_budget_ms,
            timeline_window_secs: self.timeline.window.as_secs(),
            timeline_scale: self.timeline.scale,
            lod_enabled: self.cfg.lod_enabled,
            lod_threshold_nodes: self.cfg.lod_threshold_nodes,
            lod_edges_mode: self.cfg.lod_edges_mode,
            glow_duration_ms: self.cfg.glow_duration.as_millis() as u64,
            gc_enabled: self.cfg.gc_enabled,
            gc_ttl_secs: self.cfg.gc_ttl.as_secs(),
            default_agent_mode: self.cfg.agent_default_mode,
            visual_theme: self.cfg.visual_theme,
            fog_of_war: self.cfg.fog_of_war,
            agents: self.net.endpoints.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spacegraph_core::id_process;

    fn endpoint(name: &str) -> AgentEndpoint {
        AgentEndpoint {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// A snapshot Incoming with `n` distinct process nodes (local ids reused
    /// across streams to exercise namespacing).
    fn snapshot_with(stream: &str, n: i32) -> Incoming {
        let nodes: Vec<(NodeId, Node)> = (0..n)
            .map(|i| {
                (
                    id_process("host", 1000 + i),
                    Node::Process {
                        pid: 1000 + i,
                        ppid: 1,
                        exe: "x".to_string(),
                        cmdline: "x".to_string(),
                        uid: 0,
                    },
                )
            })
            .collect();
        Incoming::snapshot(
            stream.to_string(),
            Msg::Snapshot {
                nodes,
                edges: Vec::new(),
            },
        )
    }

    #[test]
    fn multi_stream_colliding_local_ids_do_not_merge() {
        let mut st = GraphState::default();
        st.net.endpoints = vec![endpoint("a"), endpoint("b")];
        st.net.ensure_stream("a");
        st.net.ensure_stream("b");

        // Same local pid emitted by two streams must not collide.
        st.apply(snapshot_with("a", 1));
        st.apply(snapshot_with("b", 1));

        assert_eq!(
            st.model.nodes.len(),
            2,
            "colliding local ids across streams must not merge"
        );
        let origins: HashSet<Option<&str>> = st
            .model
            .nodes
            .keys()
            .map(|id| namespace::origin(id))
            .collect();
        assert!(origins.contains(&Some("a")) && origins.contains(&Some("b")));
    }

    #[test]
    fn snapshot_replaces_only_its_own_stream() {
        let mut st = GraphState::default();
        st.net.endpoints = vec![endpoint("a"), endpoint("b")];
        st.net.ensure_stream("a");
        st.net.ensure_stream("b");

        st.apply(snapshot_with("a", 2));
        st.apply(snapshot_with("b", 3));
        assert_eq!(st.model.nodes.len(), 5);

        // A fresh snapshot from "a" replaces only a's subgraph; b is untouched.
        st.apply(snapshot_with("a", 1));
        assert_eq!(st.model.nodes.len(), 4);
        let b_count = st
            .model
            .nodes
            .keys()
            .filter(|id| namespace::origin(id) == Some("b"))
            .count();
        assert_eq!(
            b_count, 3,
            "other streams must survive a per-stream snapshot"
        );
    }

    #[test]
    fn disabling_stream_hides_only_its_subgraph() {
        let mut st = GraphState::default();
        st.net.endpoints = vec![endpoint("a"), endpoint("b")];
        st.net.ensure_stream("a");
        st.net.ensure_stream("b");
        st.apply(snapshot_with("a", 2));
        st.apply(snapshot_with("b", 3));

        assert_eq!(st.visible_set_capped().len(), 5);

        st.set_stream_enabled("a", false);
        let vis = st.visible_set_capped();
        assert_eq!(vis.len(), 3);
        assert!(vis.iter().all(|id| namespace::origin(id) == Some("b")));

        // Re-enable restores exactly the same set.
        st.set_stream_enabled("a", true);
        assert_eq!(st.visible_set_capped().len(), 5);
    }

    #[test]
    fn fog_gates_rendering_but_not_placement() {
        let mut st = GraphState::default();
        let n = NodeId("host:file:/x".to_string());
        st.model.nodes.insert(
            n.clone(),
            Node::File {
                path: "/x".to_string(),
                inode: 1,
                kind: FileKind::Regular,
            },
        );
        // Fog off → everything renders.
        assert!(st.is_visible_rendered(&n));
        // Fog on → hidden until revealed.
        st.cfg.fog_of_war = true;
        assert!(!st.is_visible_rendered(&n));
        st.reveal(&n);
        assert!(st.is_visible_rendered(&n));
        // Alerts always render, even unrevealed.
        let a = NodeId("host:alert:1".to_string());
        st.model.nodes.insert(
            a.clone(),
            Node::Alert {
                source: "s".to_string(),
                signature: "x".to_string(),
                severity: "high".to_string(),
                ts: "t".to_string(),
            },
        );
        assert!(st.is_visible_rendered(&a));
    }

    #[test]
    fn alert_cap_evicts_oldest() {
        let mut st = GraphState::default();
        st.cfg.max_visible_alerts = 3;
        for i in 0..5 {
            let id = NodeId(format!("host:alert:{i}"));
            st.apply_delta(Delta::UpsertNode {
                id,
                node: Node::Alert {
                    source: "suricata".to_string(),
                    signature: format!("sig{i}"),
                    severity: "high".to_string(),
                    ts: format!("{i}"),
                },
            });
        }
        assert_eq!(
            st.alert_order.len(),
            3,
            "alerts capped at max_visible_alerts"
        );
        assert!(!st
            .model
            .nodes
            .contains_key(&NodeId("host:alert:0".to_string())));
        assert!(!st
            .model
            .nodes
            .contains_key(&NodeId("host:alert:1".to_string())));
        assert!(st
            .model
            .nodes
            .contains_key(&NodeId("host:alert:4".to_string())));
        assert_eq!(st.alert_severity_counts(), (0, 0, 3));
    }

    #[test]
    fn alerts_always_in_visible_set() {
        let mut st = GraphState::default();
        st.cfg.max_visible_nodes = 1; // tiny node cap
                                      // One alert + many plain nodes; the alert must survive the cap.
        st.apply_delta(Delta::UpsertNode {
            id: NodeId("host:alert:x".to_string()),
            node: Node::Alert {
                source: "suricata".to_string(),
                signature: "sig".to_string(),
                severity: "high".to_string(),
                ts: "t".to_string(),
            },
        });
        for i in 0..20 {
            st.model.nodes.insert(
                NodeId(format!("host:file:/f{i}")),
                Node::File {
                    path: format!("/f{i}"),
                    inode: i,
                    kind: FileKind::Regular,
                },
            );
        }
        let vis = st.visible_set_capped();
        assert!(
            vis.contains(&NodeId("host:alert:x".to_string())),
            "alerts bypass the node cap"
        );
    }

    #[test]
    fn spatial_slot_reuse_clears_state() {
        let mut sp = SpatialState::default();
        let a = NodeId("a".to_string());
        let ia = sp.intern(&a);
        sp.set_position(ia, Vec3::new(1.0, 2.0, 3.0));
        sp.velocities[ia.slot()] = Vec3::new(9.0, 9.0, 9.0);
        sp.set_node_glow(&a, Instant::now() + Duration::from_secs(1));
        assert!(sp.is_placed(&a));
        assert!(sp.is_glowing(&a));

        sp.release(&a);
        assert!(!sp.is_placed(&a));
        assert_eq!(sp.position_of(&a), None);
        assert!(sp.index_of(&a).is_none());

        // A new node must reuse the freed slot and start with cleared state.
        let b = NodeId("b".to_string());
        let ib = sp.intern(&b);
        assert_eq!(ib, ia, "freed slot should be reused");
        assert!(!sp.is_placed(&b));
        assert_eq!(sp.positions[ib.slot()], Vec3::ZERO);
        assert_eq!(sp.velocities[ib.slot()], Vec3::ZERO);
        assert_eq!(sp.glow_until[ib.slot()], None);
        assert!(!sp.is_glowing(&b));
    }
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

    #[test]
    fn lod_active_when_threshold_reached() {
        let cfg = CfgState {
            lod_enabled: true,
            lod_threshold_nodes: 10,
            ..Default::default()
        };

        assert!(cfg.lod_active(10));
        assert!(cfg.lod_active(11));
    }

    #[test]
    fn lod_inactive_when_disabled() {
        let cfg = CfgState {
            lod_enabled: false,
            lod_threshold_nodes: 1,
            ..Default::default()
        };

        assert!(!cfg.lod_active(100));
    }

    #[test]
    fn net_state_tracks_message_rate() {
        let mut st = GraphState::default();
        let stream = "local.sock".to_string();
        let start = Instant::now();
        let now = start + Duration::from_millis(500);

        st.net.streams.insert(
            stream.clone(),
            NetStreamState {
                status: NetStreamStatus::Connected,
                enabled: true,
                origin_host: None,
                last_msg: Some(now),
                last_seen: Some(now),
                last_snapshot_at: None,
                last_event_at: None,
                msg_rate: 0.0,
                msg_window: VecDeque::new(),
                last_error: None,
            },
        );

        if let Some(state) = st.net.streams.get_mut(&stream) {
            state.msg_window.push_back(start);
            state.msg_window.push_back(now);
            GraphState::net_prune_stream(state, now, st.net.msg_window);
        }

        let rate = st.net.streams.get(&stream).unwrap().msg_rate;
        assert!((rate - 1.0).abs() < 0.1);
    }

    #[test]
    fn net_state_prunes_old_messages() {
        let mut st = GraphState::default();
        let stream = "local.sock".to_string();
        let start = Instant::now();
        let now = start + Duration::from_secs(5);

        st.net.streams.insert(
            stream.clone(),
            NetStreamState {
                status: NetStreamStatus::Connected,
                enabled: true,
                origin_host: None,
                last_msg: Some(start),
                last_seen: Some(start),
                last_snapshot_at: None,
                last_event_at: None,
                msg_rate: 0.0,
                msg_window: VecDeque::new(),
                last_error: None,
            },
        );

        if let Some(state) = st.net.streams.get_mut(&stream) {
            state.msg_window.push_back(start);
            GraphState::net_prune_stream(state, now, st.net.msg_window);
        }

        let rate = st.net.streams.get(&stream).unwrap().msg_rate;
        assert_eq!(rate, 0.0);
    }
}
