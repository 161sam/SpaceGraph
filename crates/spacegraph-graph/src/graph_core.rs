//! `GraphCore` — the headless canonical-state object (ADR-0001 / MP-v0.6.0 P4).
//!
//! Owns the graph [`GraphModel`] + the alert ledger, ingests agent deltas, runs
//! the detection pipeline, and answers **read-only** queries. No Bevy / render /
//! GUI dependency. Two consumers wrap it:
//! - the viewer's `GraphState` embeds a `GraphCore` + render/ui fields and adds
//!   render bookkeeping (glow, timeline, spatial layout) around the same data;
//! - `spacegraph-mcp` (P5) hosts a `GraphCore` directly, ingesting from one agent
//!   over UDS and serving the read-only tool surface.
//!
//! The viewer's ingest is a render-ful, multi-stream superset (it namespaces ids
//! and drives layout/timeline); the headless [`GraphCore::apply_delta`] /
//! [`GraphCore::apply_snapshot`] here are the minimal single-stream canonical
//! path. Both build on the shared `GraphModel` mutators — no duplicated graph
//! logic.

use std::collections::{HashSet, VecDeque};
use std::time::Instant;

use spacegraph_core::{id_alert, Delta, Edge, EdgeKind, Node, NodeId};

use crate::correlation::{correlate, Campaign};
use crate::coverage::{coverage, TacticCoverage};
use crate::explain::{shortest_path, PathStep};
use crate::model::GraphModel;
use crate::posture::{posture, Posture};
use crate::rules::{evaluate_rules, Detection};

/// The default retained-alert cap when a consumer does not specify one. The
/// viewer overrides this with its configured `max_visible_alerts`.
pub const DEFAULT_ALERT_CAP: usize = 512;

/// Headless canonical-state core: the graph model + the alert ledger. The viewer
/// wraps this in its `GraphState` `Resource`; `spacegraph-mcp` hosts it directly.
#[derive(Default)]
pub struct GraphCore {
    pub model: GraphModel,
    /// Insertion order of retained alert nodes (oldest first) for cap eviction.
    pub alert_order: VecDeque<NodeId>,
}

/// Stable, render-free name for a node's kind (topology breakdown / MCP results).
pub fn node_kind_name(node: &Node) -> &'static str {
    match node {
        Node::Process { .. } => "process",
        Node::File { .. } => "file",
        Node::User { .. } => "user",
        Node::Socket { .. } => "socket",
        Node::RemoteHost { .. } => "remote_host",
        Node::Alert { .. } => "alert",
    }
}

/// Topology summary (MCP `topology_stats`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopologyStats {
    pub nodes: usize,
    pub edges: usize,
    pub agg_edges: usize,
    pub alerts: usize,
    pub processes: usize,
    pub files: usize,
    pub users: usize,
    pub sockets: usize,
    pub remote_hosts: usize,
}

/// A node lookup result (MCP `node` query): the node plus its incident degree and
/// neighbor ids from the prebuilt adjacency.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeDetail {
    pub id: NodeId,
    pub node: Node,
    pub degree: usize,
    pub neighbors: Vec<NodeId>,
}

/// An alert summary (MCP `alerts` feed): the alert node fields plus the subject
/// it was raised on (the `alerts_on` edge target), if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertView {
    pub id: NodeId,
    pub source: String,
    pub signature: String,
    pub severity: String,
    pub subject: Option<NodeId>,
}

impl GraphCore {
    // ---- Ingest (headless, single-stream) ------------------------------------

    /// Replace the whole graph from a snapshot (the agent's `Snapshot`), then
    /// rebuild the alert ledger from the snapshot's alert nodes. Authoritative —
    /// no cap eviction (a snapshot is the source of truth).
    pub fn apply_snapshot(&mut self, nodes: Vec<(NodeId, Node)>, edges: Vec<Edge>) {
        let now = Instant::now();
        self.model.load_snapshot(nodes, edges, now);
        self.alert_order = self
            .model
            .nodes
            .iter()
            .filter(|(_, n)| matches!(n, Node::Alert { .. }))
            .map(|(id, _)| id.clone())
            .collect();
    }

    /// Apply one agent `Delta` to the canonical state (the render-free subset of
    /// the viewer's ingest). `BatchBegin`/`BatchEnd` carry no graph change (they
    /// drive viewer glow batching) and are ignored here. Returns alert ids evicted
    /// by the cap, so a render consumer can release their layout slots.
    pub fn apply_delta(&mut self, d: Delta, alert_cap: usize) -> Vec<NodeId> {
        let now = Instant::now();
        match d {
            Delta::BatchBegin { .. } | Delta::BatchEnd { .. } => Vec::new(),
            Delta::UpsertNode { id, node } => {
                let is_alert = matches!(node, Node::Alert { .. });
                self.model.upsert_node(id.clone(), node, now);
                if is_alert {
                    self.note_alert(id, alert_cap)
                } else {
                    Vec::new()
                }
            }
            Delta::RemoveNode { id } => {
                self.model.remove_node(&id);
                self.alert_order.retain(|a| a != &id);
                Vec::new()
            }
            Delta::UpsertEdge { edge } => {
                self.model.upsert_edge(edge, now);
                Vec::new()
            }
            Delta::RemoveEdge { edge } => {
                self.model.remove_edge(&edge);
                Vec::new()
            }
        }
    }

    // ---- Alert ledger + detection pipeline -----------------------------------

    /// Record an alert id in the ledger and evict the oldest beyond `cap`,
    /// removing the evicted nodes from the model. Returns the evicted ids (for a
    /// render consumer to release layout slots). Idempotent on a known id.
    pub fn note_alert(&mut self, id: NodeId, cap: usize) -> Vec<NodeId> {
        if self.alert_order.contains(&id) {
            return Vec::new();
        }
        self.alert_order.push_back(id);
        let cap = cap.max(1);
        let mut evicted = Vec::new();
        while self.alert_order.len() > cap {
            if let Some(old) = self.alert_order.pop_front() {
                self.model.remove_node(&old);
                evicted.push(old);
            }
        }
        evicted
    }

    /// Emit a rule detection (ADR-0005) as a first-class `Node::Alert`
    /// (`source = "spacegraph-rule"`) + an `alerts_on` edge to its subject.
    /// Idempotent on the stable id. Returns the alert id + any cap-evicted ids.
    pub fn emit_detection(&mut self, det: &Detection, cap: usize) -> (NodeId, Vec<NodeId>) {
        let now = Instant::now();
        let alert_id = id_alert(&det.subject.0, &det.dedup_key());
        self.model.upsert_node(
            alert_id.clone(),
            Node::Alert {
                source: "spacegraph-rule".to_string(),
                signature: det.signature(),
                severity: det.severity.as_str().to_string(),
                ts: String::new(),
            },
            now,
        );
        self.model.upsert_edge(
            Edge {
                from: alert_id.clone(),
                to: det.subject.clone(),
                kind: EdgeKind::AlertsOn,
            },
            now,
        );
        let evicted = self.note_alert(alert_id.clone(), cap);
        (alert_id, evicted)
    }

    /// Clear a detection alert (re-arm): remove the node + its ledger entry.
    pub fn clear_detection(&mut self, alert_id: &NodeId) {
        self.model.remove_node(alert_id);
        self.alert_order.retain(|id| id != alert_id);
    }

    /// Reconcile the active detection set to `detections`: emit new alerts, clear
    /// the ones that cleared (re-arm). Returns `(new_active, evicted)` — the new
    /// active id set and any cap-evicted alert ids. Deterministic (no `Time`).
    pub fn apply_detections(
        &mut self,
        detections: &[Detection],
        active: &HashSet<NodeId>,
        cap: usize,
    ) -> (HashSet<NodeId>, Vec<NodeId>) {
        let mut current = HashSet::new();
        let mut evicted = Vec::new();
        for det in detections {
            let alert_id = id_alert(&det.subject.0, &det.dedup_key());
            if current.insert(alert_id) {
                let (_, mut ev) = self.emit_detection(det, cap);
                evicted.append(&mut ev);
            }
        }
        for id in active.difference(&current) {
            self.clear_detection(id);
        }
        (current, evicted)
    }

    /// Run the detection rule registry over the current graph and reconcile the
    /// active set (headless pipeline tick — the MCP's detection step). Returns the
    /// new active set + any cap-evicted alert ids.
    pub fn run_detection(
        &mut self,
        active: &HashSet<NodeId>,
        cap: usize,
    ) -> (HashSet<NodeId>, Vec<NodeId>) {
        let detections = evaluate_rules(&self.model);
        self.apply_detections(&detections, active, cap)
    }

    // ---- Read-only queries (the MCP tool surface) ----------------------------

    /// Topology summary: counts of nodes (by kind), edges, aggregated edges, and
    /// alerts.
    pub fn topology_stats(&self) -> TopologyStats {
        let mut s = TopologyStats {
            nodes: self.model.nodes.len(),
            edges: self.model.edges.len(),
            agg_edges: self.model.agg_edge_count(),
            alerts: 0,
            processes: 0,
            files: 0,
            users: 0,
            sockets: 0,
            remote_hosts: 0,
        };
        for node in self.model.nodes.values() {
            match node {
                Node::Process { .. } => s.processes += 1,
                Node::File { .. } => s.files += 1,
                Node::User { .. } => s.users += 1,
                Node::Socket { .. } => s.sockets += 1,
                Node::RemoteHost { .. } => s.remote_hosts += 1,
                Node::Alert { .. } => s.alerts += 1,
            }
        }
        s
    }

    /// Look up a node by id with its incident degree + neighbor ids.
    pub fn node_detail(&self, id: &NodeId) -> Option<NodeDetail> {
        let node = self.model.nodes.get(id)?.clone();
        Some(NodeDetail {
            id: id.clone(),
            node,
            degree: self.model.degree(id),
            neighbors: self.model.neighbors(id).collect(),
        })
    }

    /// Alert feed, newest first, capped at `limit`. Each carries the subject the
    /// alert was raised on (the `alerts_on` edge target), if present.
    pub fn alerts(&self, limit: usize) -> Vec<AlertView> {
        self.alert_order
            .iter()
            .rev()
            .take(limit)
            .filter_map(|id| match self.model.nodes.get(id) {
                Some(Node::Alert {
                    source,
                    signature,
                    severity,
                    ..
                }) => Some(AlertView {
                    id: id.clone(),
                    source: source.clone(),
                    signature: signature.clone(),
                    severity: severity.clone(),
                    subject: self.alert_subject(id),
                }),
                _ => None,
            })
            .collect()
    }

    /// The subject an alert was raised on (its `alerts_on` edge target).
    fn alert_subject(&self, alert_id: &NodeId) -> Option<NodeId> {
        self.model.edges_for_node(alert_id).find_map(|e| {
            if e.from == *alert_id && matches!(e.kind, EdgeKind::AlertsOn) {
                Some(e.to.clone())
            } else {
                None
            }
        })
    }

    /// Current alerts by severity `(low, medium, high)`.
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

    /// Alert node ids, newest first.
    pub fn alerts_newest_first(&self) -> impl Iterator<Item = &NodeId> {
        self.alert_order.iter().rev()
    }

    /// Multi-stage campaigns correlated from the current detections (D3, ADR-0007).
    pub fn campaigns(&self) -> Vec<Campaign> {
        correlate(&self.model)
    }

    /// ATT&CK coverage (detected/undetected, tactic-grouped) from the rule
    /// registry (D5, ADR-0006 §3).
    pub fn coverage(&self) -> Vec<TacticCoverage> {
        coverage()
    }

    /// Posture / exposure score over the current graph (D5). Deterministic.
    pub fn posture(&self) -> Posture {
        posture(&self.model)
    }

    /// Shortest explain-path between two nodes over the whole graph, bounded by
    /// `max_depth`. (The viewer's variant restricts the search to a focus subset;
    /// the headless query allows all nodes.)
    pub fn explain_path(&self, a: &NodeId, b: &NodeId, max_depth: usize) -> Option<Vec<PathStep>> {
        let allowed: HashSet<NodeId> = self.model.nodes.keys().cloned().collect();
        shortest_path(
            &self.model,
            a.clone(),
            b.clone(),
            max_depth.max(1),
            &allowed,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spacegraph_core::{EdgeKind, FileKind};

    fn proc(id: &str) -> (NodeId, Node) {
        (
            NodeId(id.into()),
            Node::Process {
                pid: 1,
                ppid: 0,
                exe: id.into(),
                cmdline: String::new(),
                uid: 0,
            },
        )
    }
    fn file(id: &str, path: &str) -> (NodeId, Node) {
        (
            NodeId(id.into()),
            Node::File {
                path: path.into(),
                inode: 1,
                kind: FileKind::Regular,
            },
        )
    }
    fn socket(id: &str, state: &str, port: u16) -> (NodeId, Node) {
        (
            NodeId(id.into()),
            Node::Socket {
                proto: "tcp".into(),
                local_addr: "0.0.0.0".into(),
                local_port: port,
                state: state.into(),
            },
        )
    }
    fn remote(id: &str) -> (NodeId, Node) {
        (
            NodeId(id.into()),
            Node::RemoteHost {
                addr: id.into(),
                rdns: None,
            },
        )
    }
    fn edge(from: &str, to: &str, kind: EdgeKind) -> Edge {
        Edge {
            from: NodeId(from.into()),
            to: NodeId(to.into()),
            kind,
        }
    }

    /// A small graph: a process that execs bash, owns a socket, and talks to a
    /// remote. No detection trips (no correlated alert; socket is ESTABLISHED on a
    /// high port; a single connect). Used for topology / node / explain tests.
    fn sample_core() -> GraphCore {
        let mut c = GraphCore::default();
        c.apply_snapshot(
            vec![
                proc("p"),
                file("sh", "/usr/bin/bash"),
                socket("s", "ESTABLISHED", 44321),
                remote("r"),
            ],
            vec![
                edge("p", "sh", EdgeKind::Execs),
                edge("p", "s", EdgeKind::OwnsSocket),
                edge("s", "r", EdgeKind::ConnectsTo),
            ],
        );
        c
    }

    /// A process listening on an unusual port — trips `SuspiciousListenerRule`
    /// (T1571) deterministically.
    fn suspicious_listener_core() -> GraphCore {
        let mut c = GraphCore::default();
        c.apply_snapshot(
            vec![proc("p"), socket("s", "LISTEN", 4444)],
            vec![edge("p", "s", EdgeKind::ListensOn)],
        );
        c
    }

    #[test]
    fn topology_stats_counts_by_kind() {
        let c = sample_core();
        let s = c.topology_stats();
        assert_eq!(s.nodes, 4);
        assert_eq!(s.processes, 1);
        assert_eq!(s.files, 1);
        assert_eq!(s.sockets, 1);
        assert_eq!(s.remote_hosts, 1);
        assert_eq!(s.alerts, 0);
        assert_eq!(s.edges, 3);
    }

    #[test]
    fn node_detail_reports_degree_and_neighbors() {
        let c = sample_core();
        let d = c.node_detail(&NodeId("p".into())).expect("p exists");
        assert!(matches!(d.node, Node::Process { .. }));
        assert_eq!(d.degree, 2, "p execs sh and owns s");
        let mut neigh: Vec<String> = d.neighbors.iter().map(|n| n.0.clone()).collect();
        neigh.sort();
        assert_eq!(neigh, vec!["s".to_string(), "sh".to_string()]);
        assert!(c.node_detail(&NodeId("missing".into())).is_none());
    }

    #[test]
    fn apply_delta_upserts_and_removes() {
        let mut c = GraphCore::default();
        let (id, node) = proc("p");
        assert!(c
            .apply_delta(
                Delta::UpsertNode {
                    id: id.clone(),
                    node
                },
                DEFAULT_ALERT_CAP
            )
            .is_empty());
        assert_eq!(c.topology_stats().processes, 1);
        c.apply_delta(Delta::RemoveNode { id }, DEFAULT_ALERT_CAP);
        assert_eq!(c.topology_stats().nodes, 0);
    }

    #[test]
    fn detection_emits_dedups_rearms_and_feeds_queries() {
        let mut c = suspicious_listener_core();
        let (current, _) = c.run_detection(&HashSet::new(), DEFAULT_ALERT_CAP);
        // The suspicious listener (LISTEN :4444) trips T1571 → one detection alert.
        assert_eq!(current.len(), 1, "suspicious-listener detection fired");
        let alerts = c.alerts(10);
        assert_eq!(alerts.len(), current.len());
        assert!(alerts.iter().all(|a| a.source == "spacegraph-rule"));
        assert!(
            alerts.iter().all(|a| a.subject.is_some()),
            "alerts_on subject set"
        );

        // Re-running with the same active set de-dups (no new alert nodes).
        let before = c.topology_stats().alerts;
        let (current2, _) = c.run_detection(&current, DEFAULT_ALERT_CAP);
        assert_eq!(current2, current);
        assert_eq!(
            c.topology_stats().alerts,
            before,
            "de-dup: no duplicate alerts"
        );

        // Severity counts are consistent with the alert feed.
        let (low, med, high) = c.alert_severity_counts();
        assert_eq!(low + med + high, current.len());
    }

    #[test]
    fn note_alert_evicts_oldest_beyond_cap() {
        let mut c = GraphCore::default();
        let mk = |i: usize| NodeId(format!("alert-{i}"));
        for i in 0..3 {
            c.model.upsert_node(
                mk(i),
                Node::Alert {
                    source: "x".into(),
                    signature: "s".into(),
                    severity: "high".into(),
                    ts: String::new(),
                },
                Instant::now(),
            );
        }
        assert!(c.note_alert(mk(0), 2).is_empty());
        assert!(c.note_alert(mk(1), 2).is_empty());
        let evicted = c.note_alert(mk(2), 2);
        assert_eq!(evicted, vec![mk(0)], "oldest evicted at cap 2");
        assert_eq!(c.alert_order.len(), 2);
        assert!(!c.model.nodes.contains_key(&mk(0)), "evicted node removed");
    }

    #[test]
    fn queries_are_well_typed_over_a_fixture() {
        let c = sample_core();
        // coverage is registry-derived (independent of the graph) and non-empty.
        assert!(!c.coverage().is_empty());
        // posture is deterministic + bounded.
        let p = c.posture();
        assert!((0.0..=100.0).contains(&p.score));
        // explain-path connects the process to the remote it talks to.
        let path = c.explain_path(&NodeId("p".into()), &NodeId("r".into()), 8);
        assert!(path.is_some(), "p -> s -> r is reachable");
        // campaigns is a typed (possibly empty) result.
        let _campaigns: Vec<Campaign> = c.campaigns();
    }
}
