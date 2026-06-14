//! Viewer-side Bevy integration for the detection rule engine. The **pure** engine
//! (`RuleRegistry`, `Detection`, `Tactic`, ATT&CK tagging, `evaluate_rules`) lives
//! in [`spacegraph_graph::rules`] and is re-exported here; this module adds the
//! budgeted `Update` system that runs it over the live `GraphState` and emits/clears
//! `spacegraph-rule` alerts (ADR-0005/0006). Headless consumers (`spacegraph-mcp`)
//! call the pure engine directly — no Bevy needed.

pub use spacegraph_graph::rules::*;

use bevy::prelude::{Res, ResMut, Resource};
use bevy::time::Time;
use std::collections::HashSet;

use spacegraph_core::NodeId;

use crate::graph::GraphState;

/// Engine state: the rule registry, the currently-active detection alert ids
/// (for de-dup + re-arm), and the last-run timestamp (the budgeted cadence).
#[derive(Resource)]
pub struct DetectionState {
    registry: RuleRegistry,
    active: HashSet<NodeId>,
    last_run_secs: f32,
}

impl Default for DetectionState {
    fn default() -> Self {
        Self {
            registry: RuleRegistry::default(),
            active: HashSet::new(),
            last_run_secs: f32::NEG_INFINITY,
        }
    }
}

/// Budgeted `Update` system (scheduled after `update_layout_or_timeline`): runs
/// the rule registry over the canonical `GraphModel` on the configured interval
/// and emits/clears `spacegraph-rule` alerts. Honors `detection_enabled` and the
/// interval cadence — never a per-frame full rescan (ADR-0005). The evaluation is
/// O(nodes + agg-edges) over prebuilt indices, bounded by the interval.
pub fn run_detection_rules(
    mut st: ResMut<GraphState>,
    mut ds: ResMut<DetectionState>,
    time: Res<Time>,
) {
    if !st.cfg.detection_enabled {
        if !ds.active.is_empty() {
            let active = std::mem::take(&mut ds.active);
            for id in &active {
                st.clear_detection(id);
            }
        }
        return;
    }
    let now = time.elapsed_seconds();
    let interval = (st.cfg.detection_interval_ms as f32 / 1000.0).max(0.0);
    if now - ds.last_run_secs < interval {
        return;
    }
    ds.last_run_secs = now;

    let detections = ds.registry.evaluate(&st.model);
    let active = ds.active.clone();
    ds.active = st.apply_detections(&detections, &active);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::model::GraphModel;
    use spacegraph_core::{Edge, EdgeKind, FileKind, Node};
    use std::time::Instant;

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
    fn alert(id: &str) -> (NodeId, Node) {
        (
            NodeId(id.into()),
            Node::Alert {
                source: "suricata".into(),
                signature: "x".into(),
                severity: "high".into(),
                ts: "t".into(),
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
    fn model_of(nodes: Vec<(NodeId, Node)>, edges: Vec<Edge>) -> GraphModel {
        let mut m = GraphModel::default();
        m.load_snapshot(nodes, edges, Instant::now());
        m
    }

    fn lateral_movement_graph() -> GraphModel {
        // proc --execs--> bash; proc --owns_socket--> sock --connects_to--> remote;
        // alert --alerts_on--> proc.
        model_of(
            vec![
                proc("p"),
                file("sh", "/usr/bin/bash"),
                socket("s", "ESTABLISHED", 44321),
                remote("r"),
                alert("a"),
            ],
            vec![
                edge("p", "sh", EdgeKind::Execs),
                edge("p", "s", EdgeKind::OwnsSocket),
                edge("s", "r", EdgeKind::ConnectsTo),
                edge("a", "p", EdgeKind::AlertsOn),
            ],
        )
    }

    fn count_rule_alerts(st: &GraphState) -> usize {
        st.alert_order
            .iter()
            .filter(|id| {
                matches!(
                    st.model.nodes.get(id),
                    Some(Node::Alert { source, .. }) if source == "spacegraph-rule"
                )
            })
            .count()
    }

    #[test]
    fn emission_dedups_and_rearms() {
        let mut st = GraphState {
            model: lateral_movement_graph(),
            ..Default::default()
        };
        let dets = evaluate_rules(&st.model);
        assert_eq!(dets.len(), 1, "the fixture trips exactly lateral-movement");

        // First apply emits the rule alert.
        let active = st.apply_detections(&dets, &HashSet::new());
        assert_eq!(active.len(), 1);
        assert_eq!(count_rule_alerts(&st), 1);

        // Re-applying the same detections de-dups (no duplicate alert).
        let active2 = st.apply_detections(&dets, &active);
        assert_eq!(active2, active);
        assert_eq!(count_rule_alerts(&st), 1);

        // Re-arm: the match clears → the alert is removed.
        let cleared = st.apply_detections(&[], &active2);
        assert!(cleared.is_empty());
        assert_eq!(count_rule_alerts(&st), 0);
    }
}
