//! Multi-stage correlation / campaign aggregation (D3, ADR-0007).
//!
//! An attack is a *sequence* of detections, not isolated alerts. This module
//! links the viewer's own `spacegraph-rule` detections (D1) into **campaigns**:
//! a campaign is ≥2 detections on a **shared or graph-adjacent subject** that
//! span **≥2 distinct ATT&CK tactics** (a kill-chain progression). Aggregation is
//! a pure function of `&GraphModel` (unit-tested without ECS); the highlighted-path
//! render + timeline lane that consume it are the visual layer. **Viewer-internal:
//! no new wire type, no `Campaign` core kind** — that is deferred behind a wire
//! bump (O-8); D3 keeps campaigns derived.

use std::collections::BTreeSet;

use spacegraph_core::{Node, NodeId};

use crate::model::{EdgeKindClass, GraphModel};
use crate::rules::{technique, Tactic};

/// A correlated multi-stage campaign — the linked detections, their subjects, and
/// the distinct tactics they span (kill-chain order). `key` is stable across ticks
/// for de-dup (ADR-0007), mirroring the D1 detection de-dup discipline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Campaign {
    pub subjects: Vec<NodeId>,
    pub alerts: Vec<NodeId>,
    pub tactics: Vec<Tactic>,
    pub key: String,
}

/// Kill-chain index of a tactic (position in `Tactic::ALL`) — the ordered axis a
/// campaign advances along.
fn tactic_rank(t: Tactic) -> usize {
    Tactic::ALL
        .iter()
        .position(|x| *x == t)
        .unwrap_or(usize::MAX)
}

/// The subject a detection alert is raised on (the `alerts_on` target).
fn alert_subject(model: &GraphModel, alert: &NodeId) -> Option<NodeId> {
    model.edges_for_node(alert).find_map(|e| {
        if &e.from == alert && EdgeKindClass::from_kind(&e.kind) == EdgeKindClass::AlertsOn {
            Some(e.to.clone())
        } else {
            None
        }
    })
}

/// The ATT&CK tactic of a `spacegraph-rule` alert, parsed from its signature
/// (`spacegraph-rule:{rule}:{technique}`) via the vendored technique table.
fn alert_tactic(signature: &str) -> Option<Tactic> {
    let tid = signature.rsplit(':').next()?;
    technique(tid).map(|t| t.tactic)
}

/// Minimal union-find over a fixed index space (subjects), for linking
/// shared/adjacent subjects into one campaign component.
struct UnionFind {
    parent: Vec<usize>,
}
impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
        }
    }
    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent[ra] = rb;
        }
    }
}

/// Aggregate the model's `spacegraph-rule` detections into multi-stage campaigns.
/// Pure; the single source of truth for the D3 render + timeline lane.
pub fn correlate(model: &GraphModel) -> Vec<Campaign> {
    // 1. Collect detections: (alert_id, subject, tactic).
    let mut dets: Vec<(NodeId, NodeId, Tactic)> = Vec::new();
    for (id, node) in model.nodes.iter() {
        let Node::Alert {
            source, signature, ..
        } = node
        else {
            continue;
        };
        if source != "spacegraph-rule" {
            continue;
        }
        let Some(tactic) = alert_tactic(signature) else {
            continue;
        };
        let Some(subject) = alert_subject(model, id) else {
            continue;
        };
        dets.push((id.clone(), subject, tactic));
    }
    if dets.len() < 2 {
        return Vec::new();
    }

    // 2. Index the distinct subjects; union equal/adjacent subjects.
    let mut subjects: Vec<NodeId> = dets.iter().map(|(_, s, _)| s.clone()).collect();
    subjects.sort_by(|a, b| a.0.cmp(&b.0));
    subjects.dedup();
    let subj_index = |s: &NodeId| subjects.iter().position(|x| x == s).unwrap();
    let mut uf = UnionFind::new(subjects.len());
    for (i, s) in subjects.iter().enumerate() {
        for nbr in model.neighbors(s) {
            if let Some(j) = subjects.iter().position(|x| *x == nbr) {
                uf.union(i, j);
            }
        }
    }

    // 3. Group detections by subject component.
    let mut groups: std::collections::HashMap<usize, Vec<&(NodeId, NodeId, Tactic)>> =
        std::collections::HashMap::new();
    for d in &dets {
        let root = uf.find(subj_index(&d.1));
        groups.entry(root).or_default().push(d);
    }

    // 4. A component spanning ≥2 distinct tactics is a campaign.
    let mut out = Vec::new();
    for (_, members) in groups {
        let tactics: BTreeSet<usize> = members.iter().map(|(_, _, t)| tactic_rank(*t)).collect();
        if members.len() < 2 || tactics.len() < 2 {
            continue;
        }
        let mut alerts: Vec<NodeId> = members.iter().map(|(a, _, _)| a.clone()).collect();
        alerts.sort_by(|a, b| a.0.cmp(&b.0));
        let mut subs: Vec<NodeId> = members.iter().map(|(_, s, _)| s.clone()).collect();
        subs.sort_by(|a, b| a.0.cmp(&b.0));
        subs.dedup();
        let mut tac: Vec<Tactic> = members.iter().map(|(_, _, t)| *t).collect();
        tac.sort_by_key(|t| tactic_rank(*t));
        tac.dedup();
        // Stable key: sorted subjects + the tactic progression.
        let key = format!(
            "{}|{}",
            subs.iter()
                .map(|s| s.0.as_str())
                .collect::<Vec<_>>()
                .join(","),
            tac.iter().map(|t| t.id()).collect::<Vec<_>>().join(">")
        );
        out.push(Campaign {
            subjects: subs,
            alerts,
            tactics: tac,
            key,
        });
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use spacegraph_core::{Edge, EdgeKind, FileKind};
    use std::time::Instant;

    fn rule_alert(id: &str, technique: &str) -> (NodeId, Node) {
        (
            NodeId(id.into()),
            Node::Alert {
                source: "spacegraph-rule".into(),
                signature: format!("spacegraph-rule:r:{technique}"),
                severity: "high".into(),
                ts: String::new(),
            },
        )
    }
    fn host(id: &str) -> (NodeId, Node) {
        (
            NodeId(id.into()),
            Node::RemoteHost {
                addr: id.into(),
                rdns: None,
            },
        )
    }
    fn file(id: &str) -> (NodeId, Node) {
        (
            NodeId(id.into()),
            Node::File {
                path: id.into(),
                inode: 1,
                kind: FileKind::Regular,
            },
        )
    }
    fn alerts_on(alert: &str, subject: &str) -> Edge {
        Edge {
            from: NodeId(alert.into()),
            to: NodeId(subject.into()),
            kind: EdgeKind::AlertsOn,
        }
    }
    fn link(a: &str, b: &str) -> Edge {
        Edge {
            from: NodeId(a.into()),
            to: NodeId(b.into()),
            kind: EdgeKind::ConnectsTo,
        }
    }
    fn model_of(nodes: Vec<(NodeId, Node)>, edges: Vec<Edge>) -> GraphModel {
        let mut m = GraphModel::default();
        m.load_snapshot(nodes, edges, Instant::now());
        m
    }

    #[test]
    fn multi_tactic_same_subject_is_one_campaign() {
        // Two detections of different tactics on the same subject host → 1 campaign.
        let m = model_of(
            vec![
                host("h"),
                rule_alert("a1", "T1021"), // LateralMovement
                rule_alert("a2", "T1071"), // CommandAndControl
            ],
            vec![alerts_on("a1", "h"), alerts_on("a2", "h")],
        );
        let c = correlate(&m);
        assert_eq!(
            c.len(),
            1,
            "linked multi-tactic detections form ONE campaign"
        );
        assert_eq!(c[0].tactics.len(), 2);
        assert_eq!(c[0].alerts.len(), 2);
    }

    #[test]
    fn progression_across_adjacent_subjects_links() {
        // Detections on two *adjacent* subjects (h1 connects_to h2) with distinct
        // tactics → one campaign spanning both subjects.
        let m = model_of(
            vec![
                host("h1"),
                host("h2"),
                rule_alert("a1", "T1021"),
                rule_alert("a2", "T1041"), // Exfiltration
            ],
            vec![
                link("h1", "h2"),
                alerts_on("a1", "h1"),
                alerts_on("a2", "h2"),
            ],
        );
        let c = correlate(&m);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].subjects.len(), 2);
        // Tactics are in kill-chain order (LateralMovement before Exfiltration).
        assert_eq!(
            c[0].tactics,
            vec![Tactic::LateralMovement, Tactic::Exfiltration]
        );
    }

    #[test]
    fn single_tactic_is_not_a_campaign() {
        // Two detections but the SAME tactic on the same subject → no progression.
        let m = model_of(
            vec![
                host("h"),
                rule_alert("a1", "T1071"),
                rule_alert("a2", "T1071"),
            ],
            vec![alerts_on("a1", "h"), alerts_on("a2", "h")],
        );
        assert!(
            correlate(&m).is_empty(),
            "no tactic progression → no campaign"
        );
    }

    #[test]
    fn unrelated_subjects_do_not_link() {
        // Distinct tactics but on UNCONNECTED subjects → no campaign.
        let m = model_of(
            vec![
                host("h1"),
                file("f2"),
                rule_alert("a1", "T1021"),
                rule_alert("a2", "T1071"),
            ],
            vec![alerts_on("a1", "h1"), alerts_on("a2", "f2")],
        );
        assert!(
            correlate(&m).is_empty(),
            "no shared/adjacent subject → no chain"
        );
    }

    #[test]
    fn campaign_key_is_stable_across_passes() {
        let m = model_of(
            vec![
                host("h"),
                rule_alert("a1", "T1021"),
                rule_alert("a2", "T1071"),
            ],
            vec![alerts_on("a1", "h"), alerts_on("a2", "h")],
        );
        let k1: Vec<String> = correlate(&m).into_iter().map(|c| c.key).collect();
        let k2: Vec<String> = correlate(&m).into_iter().map(|c| c.key).collect();
        assert_eq!(k1, k2, "campaign keys are stable across ticks (de-dup)");
    }

    #[test]
    fn non_rule_alerts_are_ignored() {
        // A Suricata alert (not spacegraph-rule) does not seed a campaign.
        let mut suricata = rule_alert("a2", "T1071");
        if let Node::Alert { source, .. } = &mut suricata.1 {
            *source = "suricata".into();
        }
        let m = model_of(
            vec![host("h"), rule_alert("a1", "T1021"), suricata],
            vec![alerts_on("a1", "h"), alerts_on("a2", "h")],
        );
        assert!(
            correlate(&m).is_empty(),
            "only spacegraph-rule detections correlate"
        );
    }
}
