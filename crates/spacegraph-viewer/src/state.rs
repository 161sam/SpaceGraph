use bevy::prelude::*;
use spacegraph_core::{Delta, Edge, EdgeKind, Msg, Node, NodeId};
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub enum Incoming {
    Identity(Msg),
    Snapshot(Msg),
    Event(Msg),
    Other(Msg),
    Error(String),
}

#[derive(Component)]
pub struct NodeMarker {
    pub id: String,
}

// ---------------- Timeline / Feynman ----------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Spatial,
    Timeline,
}

impl Default for ViewMode {
    fn default() -> Self {
        ViewMode::Spatial
    }
}

#[derive(Debug, Clone)]
pub enum TimelineEvtKind {
    NodeUpsert,
    NodeRemove,
    EdgeUpsert,
    EdgeRemove,
    BatchBegin(u64),
    BatchEnd(u64),
}

#[derive(Debug, Clone)]
pub struct TimelineEvt {
    pub ts: Instant,
    pub kind: TimelineEvtKind,
    pub a: Option<NodeId>,
    pub b: Option<NodeId>,
    pub edge_kind: Option<EdgeKind>,
}

fn stable_u32(s: &str) -> u32 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    (h.finish() & 0xFFFF_FFFF) as u32
}

fn node_kind_lane(n: &Node) -> f32 {
    // Y lanes (simple & readable)
    match n {
        Node::Process { .. } => 8.0,
        Node::User { .. } => 0.0,
        Node::File { .. } => -8.0,
    }
}

// viewer-side "pretty path" (display only)
fn normalize_display_path(p: &str) -> String {
    let mut s = p.replace("/./", "/");
    while s.contains("//") {
        s = s.replace("//", "/");
    }
    let mut parts = Vec::new();
    for part in s.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            x => parts.push(x),
        }
    }
    let out = format!("/{}", parts.join("/"));
    if out == "/" { "/".into() } else { out }
}

fn edge_kind_name(k: &EdgeKind) -> &'static str {
    match k {
        EdgeKind::Opens { .. } => "opens",
        EdgeKind::Execs => "execs",
        EdgeKind::RunsAs => "runs_as",
    }
}

fn edge_explain(k: &EdgeKind) -> String {
    match k {
        EdgeKind::Opens { fd, mode } => format!("process opened file (fd={fd}, mode={mode})"),
        EdgeKind::Execs => "process execs file (exe)".to_string(),
        EdgeKind::RunsAs => "process runs as user (uid)".to_string(),
    }
}

#[derive(Default, Resource)]
pub struct GraphState {
    // full graph
    pub nodes: HashMap<NodeId, Node>,
    pub edges: HashSet<Edge>,

    // positions + velocities (spatial)
    pub positions: HashMap<NodeId, Vec3>,
    pub velocities: HashMap<NodeId, Vec3>,

    // UI
    pub filter: String,
    pub show_3d: bool,
    pub show_edges: bool,

    // Focus mode (spatial)
    pub focus: Option<NodeId>,
    pub focus_hops: usize,

    // Hover / Selection
    pub hovered: Option<NodeId>,
    pub selected: Option<NodeId>,

    // Search/Jump
    pub search_open: bool,
    pub search_query: String,
    pub search_hits: Vec<NodeId>,
    pub jump_to: Option<NodeId>,

    // Layout (force)
    pub layout_force: bool,
    pub link_distance: f32,
    pub repulsion: f32,
    pub damping: f32,
    pub max_step: f32,

    // init rings
    pub radius: f32,
    pub y_spread: f32,

    // Glow
    in_batch: bool,
    touched_nodes: HashSet<NodeId>,
    touched_edges: HashSet<Edge>,
    pub glow_nodes: HashMap<NodeId, Instant>,
    pub glow_edges: HashMap<Edge, Instant>,
    pub glow_duration: Duration,
    pub last_batch_id: Option<u64>,

    // Performance caps
    pub max_visible_nodes: usize,
    pub progressive_nodes_per_frame: usize,
    active_vis_cache: Vec<NodeId>,
    progressive_cursor: usize,

    // GC / TTL
    pub gc_enabled: bool,
    pub gc_ttl: Duration,
    pub last_seen: HashMap<NodeId, Instant>,
    pub gc_last_run: Instant,
    pub gc_interval: Duration,

    // HUD metrics
    pub fps: f32,
    pub event_rate: f32,
    pub visible_nodes: usize,
    pub visible_edges: usize,
    pub event_total: u64,
    ev_window: VecDeque<Instant>,

    // -------- Timeline --------
    pub view_mode: ViewMode,
    pub timeline_window: Duration,     // e.g. 60s
    pub timeline_scale: f32,           // units per second on X
    pub timeline_pause: bool,
    pub timeline_frozen_now: Option<Instant>,
    pub timeline_events: VecDeque<TimelineEvt>, // ringbuffer
    pub timeline_max_events: usize,    // safety cap (e.g. 20k)

    dirty_layout: bool,
    pub needs_redraw: AtomicBool,
}

impl Default for GraphState {
    fn default() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashSet::new(),
            positions: HashMap::new(),
            velocities: HashMap::new(),

            filter: String::new(),
            show_3d: true,
            show_edges: true,

            focus: None,
            focus_hops: 2,

            hovered: None,
            selected: None,

            search_open: false,
            search_query: String::new(),
            search_hits: Vec::new(),
            jump_to: None,

            layout_force: true,
            link_distance: 6.0,
            repulsion: 22.0,
            damping: 0.92,
            max_step: 0.35,

            radius: 25.0,
            y_spread: 6.0,

            in_batch: false,
            touched_nodes: HashSet::new(),
            touched_edges: HashSet::new(),
            glow_nodes: HashMap::new(),
            glow_edges: HashMap::new(),
            glow_duration: Duration::from_millis(900),
            last_batch_id: None,

            max_visible_nodes: 1200,
            progressive_nodes_per_frame: 250,
            active_vis_cache: Vec::new(),
            progressive_cursor: 0,

            gc_enabled: true,
            gc_ttl: Duration::from_secs(30),
            last_seen: HashMap::new(),
            gc_last_run: Instant::now(),
            gc_interval: Duration::from_secs(1),

            fps: 0.0,
            event_rate: 0.0,
            visible_nodes: 0,
            visible_edges: 0,
            event_total: 0,
            ev_window: VecDeque::new(),

            view_mode: ViewMode::Spatial,
            timeline_window: Duration::from_secs(60),
            timeline_scale: 0.35, // x units per second
            timeline_pause: false,
            timeline_frozen_now: None,
            timeline_events: VecDeque::new(),
            timeline_max_events: 20_000,

            dirty_layout: true,
            needs_redraw: AtomicBool::new(true),
        }
    }
}

impl GraphState {
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.edges.clear();
        self.positions.clear();
        self.velocities.clear();
        self.focus = None;
        self.hovered = None;
        self.selected = None;

        self.search_open = false;
        self.search_query.clear();
        self.search_hits.clear();
        self.jump_to = None;

        self.glow_nodes.clear();
        self.glow_edges.clear();
        self.last_seen.clear();
        self.ev_window.clear();
        self.event_total = 0;

        self.timeline_events.clear();
        self.timeline_pause = false;
        self.timeline_frozen_now = None;

        self.active_vis_cache.clear();
        self.progressive_cursor = 0;

        self.dirty_layout = true;
        self.needs_redraw.store(true, Ordering::Relaxed);
    }

    // ----- HUD metrics -----
    pub fn tick_metrics(&mut self, now: Instant) {
        let window = Duration::from_secs(2);
        while let Some(front) = self.ev_window.front() {
            if now.duration_since(*front) > window {
                self.ev_window.pop_front();
            } else {
                break;
            }
        }
        self.event_rate = (self.ev_window.len() as f32) / window.as_secs_f32();
    }
    fn on_message(&mut self) {
        self.event_total += 1;
        self.ev_window.push_back(Instant::now());
    }

    // ----- Timeline ticks -----
    pub fn timeline_now(&self) -> Instant {
        if self.timeline_pause {
            self.timeline_frozen_now.unwrap_or_else(Instant::now)
        } else {
            Instant::now()
        }
    }

    pub fn tick_timeline(&mut self) {
        // cap + window trimming
        let now = self.timeline_now();
        while self.timeline_events.len() > self.timeline_max_events {
            self.timeline_events.pop_front();
        }
        while let Some(front) = self.timeline_events.front() {
            if now.duration_since(front.ts) > self.timeline_window {
                self.timeline_events.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn set_timeline_pause(&mut self, pause: bool) {
        if pause == self.timeline_pause {
            return;
        }
        self.timeline_pause = pause;
        if pause {
            self.timeline_frozen_now = Some(Instant::now());
        } else {
            self.timeline_frozen_now = None;
        }
        self.needs_redraw.store(true, Ordering::Relaxed);
    }

    fn push_timeline(&mut self, kind: TimelineEvtKind, a: Option<NodeId>, b: Option<NodeId>, ek: Option<EdgeKind>) {
        let evt = TimelineEvt {
            ts: Instant::now(),
            kind,
            a,
            b,
            edge_kind: ek,
        };
        self.timeline_events.push_back(evt);
    }

    // ----- Apply incoming graph data -----
    pub fn apply(&mut self, inc: Incoming) {
        self.on_message();
        match inc {
            Incoming::Snapshot(Msg::Snapshot { nodes, edges }) => {
                self.nodes = nodes.into_iter().collect();
                self.edges = edges.into_iter().collect();
                let now = Instant::now();
                for id in self.nodes.keys() {
                    self.last_seen.insert(id.clone(), now);
                }
                self.mark_dirty_all();
            }
            Incoming::Event(Msg::Event { delta }) => self.apply_delta(delta),
            _ => {}
        }
    }

    fn mark_dirty_all(&mut self) {
        self.dirty_layout = true;
        self.active_vis_cache.clear();
        self.progressive_cursor = 0;
        self.needs_redraw.store(true, Ordering::Relaxed);
    }

    fn touch_node(&mut self, id: &NodeId) {
        self.last_seen.insert(id.clone(), Instant::now());
    }

    fn apply_delta(&mut self, d: Delta) {
        match d {
            Delta::BatchBegin { id } => {
                self.in_batch = true;
                self.last_batch_id = Some(id);
                self.touched_nodes.clear();
                self.touched_edges.clear();
                self.push_timeline(TimelineEvtKind::BatchBegin(id), None, None, None);
            }
            Delta::BatchEnd { id } => {
                self.in_batch = false;
                let until = Instant::now() + self.glow_duration;

                for idn in self.touched_nodes.drain() {
                    self.glow_nodes.insert(idn, until);
                }
                for e in self.touched_edges.drain() {
                    self.glow_edges.insert(e, until);
                }
                self.push_timeline(TimelineEvtKind::BatchEnd(id), None, None, None);
                self.needs_redraw.store(true, Ordering::Relaxed);
            }
            Delta::UpsertNode { id, node } => {
                self.nodes.insert(id.clone(), node);
                self.touch_node(&id);
                self.dirty_layout = true;

                self.push_timeline(TimelineEvtKind::NodeUpsert, Some(id.clone()), None, None);

                if self.in_batch {
                    self.touched_nodes.insert(id);
                } else {
                    self.glow_nodes.insert(id, Instant::now() + self.glow_duration);
                }
                self.needs_redraw.store(true, Ordering::Relaxed);
            }
            Delta::RemoveNode { id } => {
                self.nodes.remove(&id);
                self.edges.retain(|e| e.from != id && e.to != id);
                self.positions.remove(&id);
                self.velocities.remove(&id);
                self.glow_nodes.remove(&id);
                self.last_seen.remove(&id);

                if self.focus.as_ref() == Some(&id) {
                    self.focus = None;
                }
                if self.selected.as_ref() == Some(&id) {
                    self.selected = None;
                }
                if self.hovered.as_ref() == Some(&id) {
                    self.hovered = None;
                }

                self.push_timeline(TimelineEvtKind::NodeRemove, Some(id.clone()), None, None);

                self.dirty_layout = true;
                if self.in_batch {
                    self.touched_nodes.insert(id);
                }
                self.needs_redraw.store(true, Ordering::Relaxed);
            }
            Delta::UpsertEdge { edge } => {
                self.edges.insert(edge.clone());
                self.touch_node(&edge.from);
                self.touch_node(&edge.to);
                self.dirty_layout = true;

                self.push_timeline(
                    TimelineEvtKind::EdgeUpsert,
                    Some(edge.from.clone()),
                    Some(edge.to.clone()),
                    Some(edge.kind.clone()),
                );

                if self.in_batch {
                    self.touched_edges.insert(edge.clone());
                    self.touched_nodes.insert(edge.from.clone());
                    self.touched_nodes.insert(edge.to.clone());
                } else {
                    self.glow_edges.insert(edge.clone(), Instant::now() + self.glow_duration);
                }
                self.needs_redraw.store(true, Ordering::Relaxed);
            }
            Delta::RemoveEdge { edge } => {
                self.edges.remove(&edge);
                self.glow_edges.remove(&edge);

                self.push_timeline(
                    TimelineEvtKind::EdgeRemove,
                    Some(edge.from.clone()),
                    Some(edge.to.clone()),
                    Some(edge.kind.clone()),
                );

                self.needs_redraw.store(true, Ordering::Relaxed);
            }
        }
    }

    // ----- Glow maintenance -----
    pub fn tick_glow(&mut self) {
        let now = Instant::now();
        let before_n = self.glow_nodes.len();
        let before_e = self.glow_edges.len();
        self.glow_nodes.retain(|_, until| *until > now);
        self.glow_edges.retain(|_, until| *until > now);
        if self.glow_nodes.len() != before_n || self.glow_edges.len() != before_e {
            self.needs_redraw.store(true, Ordering::Relaxed);
        }
    }

    // ----- GC orphan files -----
    pub fn tick_gc(&mut self) {
        if !self.gc_enabled {
            return;
        }
        let now = Instant::now();
        if now.duration_since(self.gc_last_run) < self.gc_interval {
            return;
        }
        self.gc_last_run = now;

        let mut degree: HashMap<NodeId, u32> = HashMap::new();
        for e in self.edges.iter() {
            *degree.entry(e.from.clone()).or_insert(0) += 1;
            *degree.entry(e.to.clone()).or_insert(0) += 1;
        }

        let mut to_remove: Vec<NodeId> = Vec::new();
        for (id, node) in self.nodes.iter() {
            let is_orphan = degree.get(id).copied().unwrap_or(0) == 0;
            if !is_orphan {
                continue;
            }
            if !matches!(node, Node::File { .. }) {
                continue;
            }
            let last = self.last_seen.get(id).copied().unwrap_or(now);
            if now.duration_since(last) >= self.gc_ttl {
                to_remove.push(id.clone());
            }
        }

        if to_remove.is_empty() {
            return;
        }

        for id in to_remove {
            self.nodes.remove(&id);
            self.positions.remove(&id);
            self.velocities.remove(&id);
            self.glow_nodes.remove(&id);
            self.last_seen.remove(&id);

            if self.focus.as_ref() == Some(&id) {
                self.focus = None;
            }
            if self.selected.as_ref() == Some(&id) {
                self.selected = None;
            }
            if self.hovered.as_ref() == Some(&id) {
                self.hovered = None;
            }
        }

        self.needs_redraw.store(true, Ordering::Relaxed);
    }

    // ----- Filter / Visibility / Capping -----
    pub fn passes_filter(&self, id: &NodeId, node: &Node) -> bool {
        if self.filter.trim().is_empty() {
            return true;
        }
        let f = self.filter.to_lowercase();
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
            .nodes
            .iter()
            .filter(|(id, n)| self.passes_filter(id, n))
            .map(|(id, _)| id.clone())
            .collect();

        if let Some(focus) = &self.focus {
            base.insert(focus.clone());
            let hops = self.focus_hops.max(1);

            let mut vis: HashSet<NodeId> = HashSet::new();
            let mut q: VecDeque<(NodeId, usize)> = VecDeque::new();
            vis.insert(focus.clone());
            q.push_back((focus.clone(), 0));

            while let Some((cur, d)) = q.pop_front() {
                if d >= hops {
                    continue;
                }
                for nb in self.neighbors(&cur) {
                    if !vis.contains(&nb) {
                        vis.insert(nb.clone());
                        q.push_back((nb, d + 1));
                    }
                    if vis.len() >= self.max_visible_nodes {
                        break;
                    }
                }
                if vis.len() >= self.max_visible_nodes {
                    break;
                }
            }

            base = vis.into_iter().filter(|id| base.contains(id)).collect();
        }

        if base.len() > self.max_visible_nodes {
            let mut v: Vec<NodeId> = base.into_iter().collect();
            v.sort_by(|a, b| a.0.cmp(&b.0));
            v.truncate(self.max_visible_nodes);
            v.into_iter().collect()
        } else {
            base
        }
    }

    fn neighbors(&self, id: &NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        for e in self.edges.iter() {
            if &e.from == id {
                out.push(e.to.clone());
            } else if &e.to == id {
                out.push(e.from.clone());
            }
        }
        out
    }

    pub fn edge_visible(&self, e: &Edge, vis: &HashSet<NodeId>) -> bool {
        vis.contains(&e.from) && vis.contains(&e.to)
    }

    pub fn set_visible_counts(&mut self, vis_nodes: usize, vis_edges: usize) {
        self.visible_nodes = vis_nodes;
        self.visible_edges = vis_edges;
    }

    // ----- Progressive init / Force layout (spatial) -----
    pub fn progressive_prepare(&mut self, vis: &HashSet<NodeId>) {
        if self.active_vis_cache.is_empty() || self.dirty_layout {
            self.active_vis_cache = vis.iter().cloned().collect();
            self.active_vis_cache.sort_by(|a, b| a.0.cmp(&b.0));
            self.progressive_cursor = 0;
        }

        let radius = if self.radius <= 0.0 { 25.0 } else { self.radius };
        let y_spread = self.y_spread;

        let take = self.progressive_nodes_per_frame.max(1);
        let start = self.progressive_cursor;
        let end = (start + take).min(self.active_vis_cache.len());

        let mut proc_ids = Vec::new();
        let mut file_ids = Vec::new();
        let mut user_ids = Vec::new();

        for id in &self.active_vis_cache[start..end] {
            if self.positions.contains_key(id) {
                continue;
            }
            if let Some(n) = self.nodes.get(id) {
                match n {
                    Node::Process { .. } => proc_ids.push(id.clone()),
                    Node::File { .. } => file_ids.push(id.clone()),
                    Node::User { .. } => user_ids.push(id.clone()),
                }
            }
        }

        place_ring(&mut self.positions, &proc_ids, radius * 0.7, 0.0, y_spread);
        place_ring(&mut self.positions, &file_ids, radius * 1.2, 0.0, y_spread);
        place_ring(&mut self.positions, &user_ids, radius * 0.35, 0.0, y_spread);

        for id in &self.active_vis_cache[start..end] {
            self.velocities.entry(id.clone()).or_insert(Vec3::ZERO);
            if !self.show_3d {
                if let Some(p) = self.positions.get_mut(id) {
                    p.y = 0.0;
                }
            }
        }

        self.progressive_cursor = end;
        if self.progressive_cursor >= self.active_vis_cache.len() {
            self.dirty_layout = false;
        }

        self.needs_redraw.store(true, Ordering::Relaxed);
    }

    pub fn force_step(&mut self, vis: &HashSet<NodeId>, dt: f32) {
        if !self.layout_force {
            return;
        }

        let ids: Vec<NodeId> = vis
            .iter()
            .filter(|id| self.positions.contains_key(*id))
            .cloned()
            .collect();
        if ids.len() <= 1 {
            return;
        }

        let link_dist = self.link_distance.max(0.1);
        let repulsion = self.repulsion.max(0.0);
        let damping = self.damping.clamp(0.0, 1.0);
        let max_step = self.max_step.max(0.001);

        let mut forces: HashMap<NodeId, Vec3> = HashMap::new();
        for id in ids.iter() {
            forces.insert(id.clone(), Vec3::ZERO);
        }

        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let a = &ids[i];
                let b = &ids[j];
                let pa = *self.positions.get(a).unwrap_or(&Vec3::ZERO);
                let pb = *self.positions.get(b).unwrap_or(&Vec3::ZERO);

                let mut dir = pa - pb;
                if !self.show_3d {
                    dir.y = 0.0;
                }
                let dist2 = dir.length_squared().max(0.01);
                let f = (repulsion / dist2) * dir.normalize_or_zero();

                *forces.get_mut(a).unwrap() += f;
                *forces.get_mut(b).unwrap() -= f;
            }
        }

        for e in self.edges.iter() {
            if !self.edge_visible(e, vis) {
                continue;
            }
            if !(self.positions.contains_key(&e.from) && self.positions.contains_key(&e.to)) {
                continue;
            }
            let pa = *self.positions.get(&e.from).unwrap();
            let pb = *self.positions.get(&e.to).unwrap();

            let mut d = pb - pa;
            if !self.show_3d {
                d.y = 0.0;
            }
            let len = d.length().max(0.001);
            let dir = d / len;
            let k = 0.6;
            let stretch = len - link_dist;
            let f = k * stretch * dir;

            *forces.get_mut(&e.from).unwrap() += f;
            *forces.get_mut(&e.to).unwrap() -= f;
        }

        for id in ids.iter() {
            let v = self.velocities.entry(id.clone()).or_insert(Vec3::ZERO);
            let f = *forces.get(id).unwrap_or(&Vec3::ZERO);

            *v = (*v + f * dt) * damping;

            let mut step = *v * dt;
            if step.length() > max_step {
                step = step.normalize_or_zero() * max_step;
            }

            let p = self.positions.entry(id.clone()).or_insert(Vec3::ZERO);
            *p += step;
            if !self.show_3d {
                p.y = 0.0;
            }
        }

        self.needs_redraw.store(true, Ordering::Relaxed);
    }

    // ----- Tooltip helpers -----
    pub fn node_tooltip_lines(&self, id: &NodeId) -> Vec<String> {
        let Some(n) = self.nodes.get(id) else { return vec![id.0.clone()]; };
        let mut out = Vec::new();
        out.push(format!("id: {}", id.0));
        match n {
            Node::File { path, inode, kind } => {
                out.push("kind: file".into());
                out.push(format!("path: {}", normalize_display_path(path)));
                out.push(format!("inode: {}", inode));
                out.push(format!("filekind: {:?}", kind));
            }
            Node::Process { pid, ppid, exe, cmdline, uid } => {
                out.push("kind: process".into());
                out.push(format!("pid: {pid} ppid: {ppid} uid: {uid}"));
                out.push(format!("exe: {}", normalize_display_path(exe)));
                out.push(format!("cmd: {}", cmdline));
            }
            Node::User { uid, name } => {
                out.push("kind: user".into());
                out.push(format!("uid: {uid} name: {name}"));
            }
        }
        out
    }

    pub fn edges_for_node(&self, id: &NodeId) -> Vec<Edge> {
        self.edges
            .iter()
            .filter(|e| &e.from == id || &e.to == id)
            .cloned()
            .collect()
    }

    pub fn explain_edge(&self, e: &Edge) -> String {
        format!(
            "{} -> {} : {} ({})",
            e.from.0,
            e.to.0,
            edge_kind_name(&e.kind),
            edge_explain(&e.kind)
        )
    }

    // ---- Search helpers ----
    pub fn recompute_search_hits(&mut self, limit: usize) {
        self.search_hits.clear();
        let q = self.search_query.trim().to_lowercase();
        if q.is_empty() {
            return;
        }

        let mut hits: Vec<NodeId> = self
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
        self.search_hits = hits;
    }

    pub fn request_jump(&mut self, id: NodeId) {
        self.jump_to = Some(id);
    }

    // ---- Glow checks ----
    pub fn node_is_glowing(&self, id: &NodeId) -> bool {
        self.glow_nodes.contains_key(id)
    }
    pub fn edge_is_glowing(&self, e: &Edge) -> bool {
        self.glow_edges.contains_key(e)
    }

    // ---- Timeline mapping helpers ----
    pub fn timeline_pos_for_node(&self, id: &NodeId) -> Vec3 {
        // Based on node kind -> Y lane; Z = stable hash; X set by event time elsewhere.
        let y = self.nodes.get(id).map(node_kind_lane).unwrap_or(0.0);
        let hz = stable_u32(&id.0) as f32 / 65535.0; // 0..1
        let z = (hz - 0.5) * 18.0;
        Vec3::new(0.0, y, z)
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
        let y = y_base + if y_spread > 0.0 { ((i as f32) % 7.0) / 7.0 * y_spread } else { 0.0 };
        pos.insert(id.clone(), Vec3::new(x, y, z));
    }
}
