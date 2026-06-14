//! Graph-native detection rule engine (ADR-0005) + MITRE ATT&CK tagging
//! (ADR-0006). Compiled Rust matchers — not a DSL — run over the canonical
//! [`GraphModel`] and produce [`Detection`]s, each carrying a mandatory ATT&CK
//! technique + tactic. The viewer turns detections into first-class `Node::Alert`
//! (`source = "spacegraph-rule"`), reusing the existing alert plumbing — **no
//! `spacegraph-core` change, no new node/edge kind, no wire bump** (O-8). Nothing
//! here is agent-side and nothing reaches the network (O-7).
//!
//! [`evaluate_rules`] is a pure function of `&GraphModel` so the rules are
//! unit-tested without Bevy/ECS, mirroring the `graph/explain.rs` test posture.

use spacegraph_core::{Node, NodeId};

use crate::model::{EdgeKindClass, GraphModel};

/// The 14 MITRE ATT&CK Enterprise tactics, in kill-chain order (ADR-0006). The
/// closed enum is what makes the coverage view (D5) and tactic-phased motion (D2)
/// well-defined; it is also the ordered axis a campaign (D3) advances along.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tactic {
    Reconnaissance,
    ResourceDevelopment,
    InitialAccess,
    Execution,
    Persistence,
    PrivilegeEscalation,
    DefenseEvasion,
    CredentialAccess,
    Discovery,
    LateralMovement,
    Collection,
    CommandAndControl,
    Exfiltration,
    Impact,
}

impl Tactic {
    /// All tactics in kill-chain order (the ordered axis for D2/D3/D5).
    pub const ALL: [Tactic; 14] = [
        Tactic::Reconnaissance,
        Tactic::ResourceDevelopment,
        Tactic::InitialAccess,
        Tactic::Execution,
        Tactic::Persistence,
        Tactic::PrivilegeEscalation,
        Tactic::DefenseEvasion,
        Tactic::CredentialAccess,
        Tactic::Discovery,
        Tactic::LateralMovement,
        Tactic::Collection,
        Tactic::CommandAndControl,
        Tactic::Exfiltration,
        Tactic::Impact,
    ];

    /// Stable kebab id (folded into the alert signature; stable for the registry).
    pub fn id(self) -> &'static str {
        match self {
            Tactic::Reconnaissance => "reconnaissance",
            Tactic::ResourceDevelopment => "resource-development",
            Tactic::InitialAccess => "initial-access",
            Tactic::Execution => "execution",
            Tactic::Persistence => "persistence",
            Tactic::PrivilegeEscalation => "privilege-escalation",
            Tactic::DefenseEvasion => "defense-evasion",
            Tactic::CredentialAccess => "credential-access",
            Tactic::Discovery => "discovery",
            Tactic::LateralMovement => "lateral-movement",
            Tactic::Collection => "collection",
            Tactic::CommandAndControl => "command-and-control",
            Tactic::Exfiltration => "exfiltration",
            Tactic::Impact => "impact",
        }
    }

    /// Human label for the inspector/coverage view.
    pub fn label(self) -> &'static str {
        match self {
            Tactic::Reconnaissance => "Reconnaissance",
            Tactic::ResourceDevelopment => "Resource Development",
            Tactic::InitialAccess => "Initial Access",
            Tactic::Execution => "Execution",
            Tactic::Persistence => "Persistence",
            Tactic::PrivilegeEscalation => "Privilege Escalation",
            Tactic::DefenseEvasion => "Defense Evasion",
            Tactic::CredentialAccess => "Credential Access",
            Tactic::Discovery => "Discovery",
            Tactic::LateralMovement => "Lateral Movement",
            Tactic::Collection => "Collection",
            Tactic::CommandAndControl => "Command and Control",
            Tactic::Exfiltration => "Exfiltration",
            Tactic::Impact => "Impact",
        }
    }
}

/// Detection severity — reuses the `low`/`medium`/`high` vocabulary the alert
/// triage/render/cap machinery already understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Low,
    Medium,
    High,
}

impl Severity {
    /// The `Node::Alert.severity` string (the existing alert vocabulary).
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
        }
    }
}

/// A vendored ATT&CK technique entry: `(id, name, tactic)`. Curated subset, not
/// the full corpus, **never fetched** (O-7) — extended as rules are added.
pub struct Technique {
    pub id: &'static str,
    pub name: &'static str,
    pub tactic: Tactic,
}

/// The vendored technique table — the techniques SpaceGraph can reason about.
/// Single source of truth the coverage view (D5) reads.
pub const TECHNIQUES: &[Technique] = &[
    Technique {
        id: "T1021",
        name: "Remote Services",
        tactic: Tactic::LateralMovement,
    },
    Technique {
        id: "T1071",
        name: "Application Layer Protocol",
        tactic: Tactic::CommandAndControl,
    },
    Technique {
        id: "T1571",
        name: "Non-Standard Port",
        tactic: Tactic::CommandAndControl,
    },
    Technique {
        id: "T1041",
        name: "Exfiltration Over C2 Channel",
        tactic: Tactic::Exfiltration,
    },
];

/// Look up a technique by id (the rule registry's tag must resolve here).
pub fn technique(id: &str) -> Option<&'static Technique> {
    TECHNIQUES.iter().find(|t| t.id == id)
}

/// Format the ATT&CK tag of a detection alert signature
/// (`spacegraph-rule:{rule}:{technique}`) for the inspector — e.g.
/// `"ATT&CK T1021 Remote Services · Lateral Movement"`. Returns `None` for a
/// signature whose trailing segment is not a vendored technique (e.g. a Suricata
/// free-text signature).
pub fn attack_tag(signature: &str) -> Option<String> {
    let tid = signature.rsplit(':').next()?;
    let t = technique(tid)?;
    Some(format!("ATT&CK {} {} · {}", t.id, t.name, t.tactic.label()))
}

/// A detection produced by a rule: the matched subject, the supporting subgraph
/// (for rendering the "why"), and a stable de-dup key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    pub rule_id: &'static str,
    pub technique: &'static str,
    pub tactic: Tactic,
    pub severity: Severity,
    /// The entity the alert is raised on (the alert's `alerts_on` target).
    pub subject: NodeId,
    /// Supporting node ids (subject + corroborating nodes), for the "why" view.
    pub subgraph: Vec<NodeId>,
    /// Stable key making the de-dup id `{rule_id}|{subgraph_key}` (ADR-0005).
    pub subgraph_key: String,
}

impl Detection {
    /// The `Node::Alert.signature` for this detection
    /// (`spacegraph-rule:{rule_id}:{technique}`, ADR-0006).
    pub fn signature(&self) -> String {
        format!("spacegraph-rule:{}:{}", self.rule_id, self.technique)
    }

    /// The stable de-dup key fed to `id_alert` (`{rule_id}|{subgraph_key}`).
    pub fn dedup_key(&self) -> String {
        format!("{}|{}", self.rule_id, self.subgraph_key)
    }
}

/// A compiled detection matcher (ADR-0005). Every rule declares a mandatory
/// ATT&CK `technique` + `tactic`; a rule whose technique is not in [`TECHNIQUES`]
/// is rejected by the registry (`registry_techniques_are_vendored` test).
pub trait Rule: Send + Sync {
    fn id(&self) -> &'static str;
    fn technique(&self) -> &'static str;
    fn tactic(&self) -> Tactic;
    fn severity(&self) -> Severity;
    fn evaluate(&self, model: &GraphModel) -> Vec<Detection>;
}

// ---- helpers over the prebuilt GraphModel indices (no per-frame full scan) ----

/// Basename of a path-like string.
fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Whether a file path names a common interactive shell.
fn is_shell(path: &str) -> bool {
    const SHELLS: &[&str] = &[
        "sh",
        "bash",
        "zsh",
        "dash",
        "ksh",
        "fish",
        "csh",
        "tcsh",
        "ash",
        "busybox",
        "pwsh",
        "powershell",
        "cmd.exe",
    ];
    let base = basename(path);
    SHELLS.contains(&base)
}

/// Common server-listener ports a new listener is *not* suspicious on.
fn is_common_listen_port(port: u16) -> bool {
    const COMMON: &[u16] = &[
        22, 25, 53, 80, 110, 143, 443, 465, 587, 631, 993, 995, 3306, 5432, 6379, 8080, 8443,
    ];
    COMMON.contains(&port)
}

/// Directed neighbours of `id` over an edge `class`, in the requested direction.
/// `outgoing` = edges with `from == id`; otherwise edges with `to == id`.
fn directed_neighbors<'a>(
    model: &'a GraphModel,
    id: &'a NodeId,
    class: EdgeKindClass,
    outgoing: bool,
) -> impl Iterator<Item = NodeId> + 'a {
    model.edges_for_node(id).filter_map(move |edge| {
        if EdgeKindClass::from_kind(&edge.kind) != class {
            return None;
        }
        if outgoing && &edge.from == id {
            Some(edge.to.clone())
        } else if !outgoing && &edge.to == id {
            Some(edge.from.clone())
        } else {
            None
        }
    })
}

// ---- Rule 1: lateral-movement candidate (T1021) ----

/// A `Process` execs a shell **and** owns a socket connecting to a `RemoteHost`
/// **and** carries a correlated alert — a lateral-movement candidate (ADR-0005).
pub struct LateralMovementRule;

impl Rule for LateralMovementRule {
    fn id(&self) -> &'static str {
        "lateral-movement"
    }
    fn technique(&self) -> &'static str {
        "T1021"
    }
    fn tactic(&self) -> Tactic {
        Tactic::LateralMovement
    }
    fn severity(&self) -> Severity {
        Severity::High
    }

    fn evaluate(&self, model: &GraphModel) -> Vec<Detection> {
        let mut out = Vec::new();
        for (pid, node) in model.nodes.iter() {
            if !matches!(node, Node::Process { .. }) {
                continue;
            }
            // execs a shell binary (process --execs--> shell File)
            let shell = directed_neighbors(model, pid, EdgeKindClass::Execs, true).find(
                |f| matches!(model.nodes.get(f), Some(Node::File { path, .. }) if is_shell(path)),
            );
            let Some(shell) = shell else { continue };
            // owns a socket that connects to a remote host
            let mut remote_via_socket = None;
            for sock in directed_neighbors(model, pid, EdgeKindClass::OwnsSocket, true) {
                let remote = directed_neighbors(model, &sock, EdgeKindClass::ConnectsTo, true)
                    .find(|r| matches!(model.nodes.get(r), Some(Node::RemoteHost { .. })));
                if let Some(remote) = remote {
                    remote_via_socket = Some((sock, remote));
                    break;
                }
            }
            let Some((sock, remote)) = remote_via_socket else {
                continue;
            };
            // a correlated alert raised on the process (alert --alerts_on--> P)
            let has_alert = directed_neighbors(model, pid, EdgeKindClass::AlertsOn, false)
                .any(|a| matches!(model.nodes.get(&a), Some(Node::Alert { .. })));
            if !has_alert {
                continue;
            }
            out.push(Detection {
                rule_id: self.id(),
                technique: self.technique(),
                tactic: self.tactic(),
                severity: self.severity(),
                subject: pid.clone(),
                subgraph: vec![pid.clone(), shell, sock, remote.clone()],
                subgraph_key: remote.0,
            });
        }
        out
    }
}

// ---- Rule 2: suspicious new listener (T1571) ----

/// A `Socket` in `LISTEN` state on an unusual port owned by a process — a
/// non-standard-port listener (ADR-0005, T1571).
pub struct SuspiciousListenerRule;

impl Rule for SuspiciousListenerRule {
    fn id(&self) -> &'static str {
        "suspicious-listener"
    }
    fn technique(&self) -> &'static str {
        "T1571"
    }
    fn tactic(&self) -> Tactic {
        Tactic::CommandAndControl
    }
    fn severity(&self) -> Severity {
        Severity::Medium
    }

    fn evaluate(&self, model: &GraphModel) -> Vec<Detection> {
        let mut out = Vec::new();
        for (sid, node) in model.nodes.iter() {
            let Node::Socket {
                state, local_port, ..
            } = node
            else {
                continue;
            };
            if state != "LISTEN" || is_common_listen_port(*local_port) {
                continue;
            }
            // owned by a process (process --listens_on--> socket)
            let owner = directed_neighbors(model, sid, EdgeKindClass::ListensOn, false)
                .find(|p| matches!(model.nodes.get(p), Some(Node::Process { .. })));
            let Some(owner) = owner else { continue };
            out.push(Detection {
                rule_id: self.id(),
                technique: self.technique(),
                tactic: self.tactic(),
                severity: self.severity(),
                subject: sid.clone(),
                subgraph: vec![sid.clone(), owner],
                subgraph_key: local_port.to_string(),
            });
        }
        out
    }
}

// ---- Rule 3: beaconing candidate (T1071) ----

/// Minimum repeated `connects_to` events to the same remote to flag beaconing.
const BEACON_MIN_COUNT: u64 = 5;

/// Repeated `connects_to` the same `RemoteHost` (aggregated event count over the
/// edge's lifetime) — a beaconing candidate (ADR-0005, T1071). Cadence
/// *regularity* (jitter) would need per-interval samples the model does not
/// retain; the aggregated count is the available proxy.
pub struct BeaconingRule;

impl Rule for BeaconingRule {
    fn id(&self) -> &'static str {
        "beaconing"
    }
    fn technique(&self) -> &'static str {
        "T1071"
    }
    fn tactic(&self) -> Tactic {
        Tactic::CommandAndControl
    }
    fn severity(&self) -> Severity {
        Severity::Medium
    }

    fn evaluate(&self, model: &GraphModel) -> Vec<Detection> {
        let mut out = Vec::new();
        for agg in model.agg_edges() {
            if agg.key.class != EdgeKindClass::ConnectsTo || agg.stats.count < BEACON_MIN_COUNT {
                continue;
            }
            if !matches!(model.nodes.get(&agg.key.to), Some(Node::RemoteHost { .. })) {
                continue;
            }
            out.push(Detection {
                rule_id: self.id(),
                technique: self.technique(),
                tactic: self.tactic(),
                severity: self.severity(),
                subject: agg.key.to.clone(),
                subgraph: vec![agg.key.from.clone(), agg.key.to.clone()],
                subgraph_key: format!("{}->{}", agg.key.from.0, agg.key.to.0),
            });
        }
        out
    }
}

/// The active rule set (ADR-0005). The `technique ↔ rule` mapping here is the
/// single source of truth the ATT&CK coverage view (D5) reads.
pub struct RuleRegistry {
    rules: Vec<Box<dyn Rule>>,
}

impl Default for RuleRegistry {
    fn default() -> Self {
        Self {
            rules: vec![
                Box::new(LateralMovementRule),
                Box::new(SuspiciousListenerRule),
                Box::new(BeaconingRule),
            ],
        }
    }
}

impl RuleRegistry {
    pub fn rules(&self) -> &[Box<dyn Rule>] {
        &self.rules
    }

    /// Run every rule over `model`, returning all detections (de-dup by stable id
    /// happens at emission time, ADR-0005).
    pub fn evaluate(&self, model: &GraphModel) -> Vec<Detection> {
        self.rules.iter().flat_map(|r| r.evaluate(model)).collect()
    }
}

/// Pure evaluation entrypoint — the unit-testable core. Runs the default registry.
pub fn evaluate_rules(model: &GraphModel) -> Vec<Detection> {
    RuleRegistry::default().evaluate(model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use spacegraph_core::{Edge, EdgeKind, FileKind};
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

    #[test]
    fn registry_techniques_are_vendored() {
        // ADR-0006 discipline: every rule's technique resolves in TECHNIQUES.
        for rule in RuleRegistry::default().rules() {
            assert!(
                technique(rule.technique()).is_some(),
                "rule {} has an unvendored technique {}",
                rule.id(),
                rule.technique()
            );
        }
    }

    #[test]
    fn techniques_table_is_well_formed() {
        for t in TECHNIQUES {
            assert!(!t.id.is_empty() && !t.name.is_empty());
            assert!(Tactic::ALL.contains(&t.tactic));
        }
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

    #[test]
    fn lateral_movement_fires_on_full_pattern() {
        let m = lateral_movement_graph();
        let d = LateralMovementRule.evaluate(&m);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].technique, "T1021");
        assert_eq!(d[0].tactic, Tactic::LateralMovement);
        assert_eq!(d[0].subject, NodeId("p".into()));
    }

    #[test]
    fn lateral_movement_silent_without_alert() {
        // Same graph minus the correlated alert → no detection.
        let m = model_of(
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
        assert!(LateralMovementRule.evaluate(&m).is_empty());
    }

    #[test]
    fn suspicious_listener_fires_on_unusual_port() {
        let m = model_of(
            vec![proc("p"), socket("s", "LISTEN", 4444)],
            vec![edge("p", "s", EdgeKind::ListensOn)],
        );
        let d = SuspiciousListenerRule.evaluate(&m);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].technique, "T1571");
        assert_eq!(d[0].subject, NodeId("s".into()));
    }

    #[test]
    fn suspicious_listener_silent_on_common_port() {
        let m = model_of(
            vec![proc("p"), socket("s", "LISTEN", 443)],
            vec![edge("p", "s", EdgeKind::ListensOn)],
        );
        assert!(SuspiciousListenerRule.evaluate(&m).is_empty());
    }

    #[test]
    fn beaconing_fires_above_threshold() {
        let mut m = model_of(vec![socket("s", "ESTABLISHED", 50000), remote("r")], vec![]);
        let now = Instant::now();
        // Repeated connects_to the same remote bump the aggregated count.
        for i in 0..(BEACON_MIN_COUNT + 1) {
            m.upsert_edge(
                edge("s", "r", EdgeKind::ConnectsTo),
                now + std::time::Duration::from_secs(i),
            );
        }
        let d = BeaconingRule.evaluate(&m);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].technique, "T1071");
        assert_eq!(d[0].subject, NodeId("r".into()));
    }

    #[test]
    fn beaconing_silent_below_threshold() {
        let mut m = model_of(vec![socket("s", "ESTABLISHED", 50000), remote("r")], vec![]);
        let now = Instant::now();
        m.upsert_edge(edge("s", "r", EdgeKind::ConnectsTo), now);
        assert!(BeaconingRule.evaluate(&m).is_empty());
    }

    #[test]
    fn all_three_fire_distinctly_and_dedup_is_stable() {
        // Compose a graph that trips all three rules at once.
        let mut m = lateral_movement_graph();
        // add a suspicious listener
        m.upsert_node(
            NodeId("ls".into()),
            socket("ls", "LISTEN", 4444).1,
            Instant::now(),
        );
        m.upsert_edge(edge("p", "ls", EdgeKind::ListensOn), Instant::now());
        // beaconing: bump the existing s->r connects_to over threshold
        let now = Instant::now();
        for i in 0..BEACON_MIN_COUNT {
            m.upsert_edge(
                edge("s", "r", EdgeKind::ConnectsTo),
                now + std::time::Duration::from_secs(i),
            );
        }

        let first = evaluate_rules(&m);
        let techniques: std::collections::HashSet<&str> =
            first.iter().map(|d| d.technique).collect();
        assert!(techniques.contains("T1021"));
        assert!(techniques.contains("T1571"));
        assert!(techniques.contains("T1071"));

        // Stable de-dup keys across a second evaluation (same subgraph → same id).
        let second = evaluate_rules(&m);
        let keys1: std::collections::HashSet<String> =
            first.iter().map(|d| d.dedup_key()).collect();
        let keys2: std::collections::HashSet<String> =
            second.iter().map(|d| d.dedup_key()).collect();
        assert_eq!(keys1, keys2, "de-dup keys must be stable across ticks");
    }

    #[test]
    fn signature_format_is_attack_tagged() {
        let m = lateral_movement_graph();
        let d = &LateralMovementRule.evaluate(&m)[0];
        assert_eq!(d.signature(), "spacegraph-rule:lateral-movement:T1021");
    }

    #[test]
    fn attack_tag_formats_known_technique_only() {
        assert_eq!(
            attack_tag("spacegraph-rule:lateral-movement:T1021").as_deref(),
            Some("ATT&CK T1021 Remote Services · Lateral Movement")
        );
        // A Suricata free-text signature has no vendored technique → no tag.
        assert_eq!(attack_tag("ET MALWARE Suspicious Beacon"), None);
    }
}
