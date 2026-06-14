//! Posture / exposure score (D5, ADR-0006 §3): detection coverage combined with
//! observed attack-surface signals over the in-memory graph. **Deterministic** (a
//! fixture graph yields the same score), read-only, no egress, no wire change.

use spacegraph_core::Node;

use crate::graph::coverage::coverage_ratio;
use crate::graph::model::GraphModel;
use crate::render::spatial::{exposure_bucket, Exposure};

/// A posture snapshot: the components + a 0..=100 risk score (higher = more
/// exposed). The components are surfaced so the view can explain the score.
#[derive(Debug, Clone, PartialEq)]
pub struct Posture {
    /// Detection coverage ratio (0..=1) from the rule registry.
    pub coverage: f32,
    /// Public-facing listening sockets (the outward attack surface).
    pub exposed_listeners: usize,
    /// Active alerts (detection + ingested).
    pub alert_count: usize,
    /// Total nodes (for density normalization).
    pub node_count: usize,
    /// Deterministic risk score, 0 (low) .. 100 (high exposure).
    pub score: f32,
}

/// Compute the posture over `model`. Pure + deterministic: surface (public
/// listeners) + alert density, amplified by the detection-coverage gap.
pub fn posture(model: &GraphModel) -> Posture {
    let mut exposed_listeners = 0usize;
    let mut alert_count = 0usize;
    for node in model.nodes.values() {
        match node {
            Node::Socket {
                state, local_addr, ..
            } if state == "LISTEN" && exposure_bucket(local_addr) == Exposure::Public => {
                exposed_listeners += 1;
            }
            Node::Alert { .. } => alert_count += 1,
            _ => {}
        }
    }
    let node_count = model.nodes.len();
    let coverage = coverage_ratio();

    // Surface: each public listener adds risk, capped. Density: alerts per node.
    let surface = (exposed_listeners as f32 * 8.0).min(60.0);
    let density = if node_count > 0 {
        ((alert_count as f32 / node_count as f32) * 100.0).min(40.0)
    } else {
        0.0
    };
    // Undetected fraction amplifies the observed risk (gaps make exposure worse).
    let gap = 1.0 - coverage;
    let score = ((surface + density) * (0.5 + 0.5 * gap)).clamp(0.0, 100.0);

    Posture {
        coverage,
        exposed_listeners,
        alert_count,
        node_count,
        score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spacegraph_core::{Node, NodeId};
    use std::time::Instant;

    fn socket(id: &str, state: &str, addr: &str) -> (NodeId, Node) {
        (
            NodeId(id.into()),
            Node::Socket {
                proto: "tcp".into(),
                local_addr: addr.into(),
                local_port: 4444,
                state: state.into(),
            },
        )
    }
    fn alert(id: &str) -> (NodeId, Node) {
        (
            NodeId(id.into()),
            Node::Alert {
                source: "spacegraph-rule".into(),
                signature: "x".into(),
                severity: "high".into(),
                ts: String::new(),
            },
        )
    }
    fn model_of(nodes: Vec<(NodeId, Node)>) -> GraphModel {
        let mut m = GraphModel::default();
        m.load_snapshot(nodes, Vec::new(), Instant::now());
        m
    }

    #[test]
    fn counts_public_listeners_and_alerts() {
        let m = model_of(vec![
            socket("s1", "LISTEN", "0.0.0.0"),      // public listener
            socket("s2", "LISTEN", "127.0.0.1"),    // loopback — not exposed
            socket("s3", "ESTABLISHED", "0.0.0.0"), // not listening
            alert("a1"),
        ]);
        let p = posture(&m);
        assert_eq!(
            p.exposed_listeners, 1,
            "only the public LISTEN socket counts"
        );
        assert_eq!(p.alert_count, 1);
    }

    #[test]
    fn score_is_deterministic_and_bounded() {
        let build = || {
            model_of(vec![
                socket("s1", "LISTEN", "0.0.0.0"),
                socket("s2", "LISTEN", "203.0.113.5"),
                alert("a1"),
                alert("a2"),
            ])
        };
        let p1 = posture(&build());
        let p2 = posture(&build());
        assert_eq!(p1.score, p2.score, "same graph -> same score");
        assert!((0.0..=100.0).contains(&p1.score));
        assert!(p1.score > 0.0, "exposed listeners + alerts raise the score");
    }

    #[test]
    fn empty_graph_is_zero_risk() {
        let p = posture(&model_of(Vec::new()));
        assert_eq!(p.exposed_listeners, 0);
        assert_eq!(p.alert_count, 0);
        assert_eq!(p.score, 0.0);
    }
}
