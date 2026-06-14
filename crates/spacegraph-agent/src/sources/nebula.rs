//! Nebula engagement-log [`EventSource`] (D2-core, ADR-0009): tail Nebula's
//! engagement log and surface the observed red-team activity as **existing** graph
//! kinds (`RemoteHost` targets + `ConnectsTo` lateral hops). SpaceGraph only
//! **observes** Nebula — it never launches an engagement (O-9). Read-only file
//! tail; **no exec, no egress** (mirrors `suricata_eve`).
//!
//! **Assumed schema (A.5 — verify on the operator's host).** Nebula's real log
//! schema is not verified from here; this parser targets a documented JSONL
//! assumption: one event per line,
//! `{"ts": "...", "event": "connect"|"scan"|"lateral"|"finding",
//!   "src": "<ip>"?, "target": "<ip>", "port": <u16>?, "proto": "<str>"?}`.
//! If the real schema differs, [`parse_nebula_event`] is the single place to
//! adjust (the fixture documents the contract). Deploy the source as its own
//! agent **stream** named `nebula-*` so the viewer styles its entities red-team
//! (ADR-0009, purple-team origin).

use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use spacegraph_core::{id_remote_host, Delta, Edge, EdgeKind, Msg, Node, NodeId};
use tokio::sync::mpsc;

use super::EventSource;

/// A parsed Nebula engagement event (only the fields we use).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NebulaEvent {
    pub ts: String,
    pub event: String,
    /// Optional source host (present for `lateral` hops).
    pub src: Option<String>,
    pub target: String,
}

/// Parse one Nebula JSONL line; returns `Some` only when a `target` is present.
pub fn parse_nebula_event(line: &str) -> Option<NebulaEvent> {
    let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let target = v.get("target")?.as_str()?.to_string();
    if target.is_empty() {
        return None;
    }
    let event = v
        .get("event")
        .and_then(|s| s.as_str())
        .unwrap_or("connect")
        .to_string();
    let src = v
        .get("src")
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    Some(NebulaEvent {
        ts: v
            .get("ts")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string(),
        event,
        src,
        target,
    })
}

/// Build the graph for a Nebula event using **existing kinds only** (no wire
/// change, O-8): the target as a `RemoteHost`; a `lateral` hop adds the source
/// `RemoteHost` + a `ConnectsTo` edge so the engagement path is visible.
pub fn build_nebula_graph(node_id: &str, ev: &NebulaEvent) -> (Vec<(NodeId, Node)>, Vec<Edge>) {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    let target_id = id_remote_host(node_id, &ev.target);
    nodes.push((
        target_id.clone(),
        Node::RemoteHost {
            addr: ev.target.clone(),
            rdns: None,
        },
    ));

    if let Some(src) = &ev.src {
        let src_id = id_remote_host(node_id, src);
        nodes.push((
            src_id.clone(),
            Node::RemoteHost {
                addr: src.clone(),
                rdns: None,
            },
        ));
        edges.push(Edge {
            from: src_id,
            to: target_id,
            kind: EdgeKind::ConnectsTo,
        });
    }

    (nodes, edges)
}

/// Deltas to emit for a freshly parsed Nebula event (upserts only; the viewer
/// caps and the diff is naturally idempotent on a stable engagement).
pub fn nebula_deltas(node_id: &str, ev: &NebulaEvent) -> Vec<Delta> {
    let (nodes, edges) = build_nebula_graph(node_id, ev);
    let mut out = Vec::new();
    for (id, node) in nodes {
        out.push(Delta::UpsertNode { id, node });
    }
    for edge in edges {
        out.push(Delta::UpsertEdge { edge });
    }
    out
}

/// The Nebula engagement-log event source.
pub struct NebulaSource {
    pub nebula_log: PathBuf,
    pub poll_interval: Duration,
}

impl EventSource for NebulaSource {
    fn name(&self) -> &'static str {
        "nebula"
    }
    fn start(self: Box<Self>, node_id: String, tx: mpsc::Sender<Msg>) -> Result<()> {
        tokio::spawn(run(self.nebula_log, self.poll_interval, node_id, tx));
        Ok(())
    }
}

/// Read bytes appended since `offset`; resets on truncation/rotation. Read-only.
fn read_new(path: &PathBuf, offset: &mut u64) -> Vec<String> {
    let Ok(mut f) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    if len < *offset {
        *offset = 0;
    }
    if f.seek(SeekFrom::Start(*offset)).is_err() {
        return Vec::new();
    }
    let mut buf = String::new();
    if f.read_to_string(&mut buf).is_err() {
        return Vec::new();
    }
    if let Some(last_nl) = buf.rfind('\n') {
        *offset += (last_nl + 1) as u64;
        buf[..=last_nl].lines().map(|l| l.to_string()).collect()
    } else {
        Vec::new()
    }
}

async fn run(nebula_log: PathBuf, poll: Duration, node_id: String, tx: mpsc::Sender<Msg>) {
    let mut offset: u64 = 0;
    let mut batch_id: u64 = 0;
    loop {
        let path = nebula_log.clone();
        let mut off = offset;
        let lines = tokio::task::spawn_blocking(move || {
            let l = read_new(&path, &mut off);
            (l, off)
        })
        .await;
        let lines = match lines {
            Ok((l, new_off)) => {
                offset = new_off;
                l
            }
            Err(_) => {
                tokio::time::sleep(poll).await;
                continue;
            }
        };

        let mut deltas = Vec::new();
        for line in &lines {
            if let Some(ev) = parse_nebula_event(line) {
                deltas.extend(nebula_deltas(&node_id, &ev));
            }
        }
        if !deltas.is_empty() {
            batch_id = batch_id.wrapping_add(1);
            if tx
                .send(Msg::Event {
                    delta: Delta::BatchBegin { id: batch_id },
                })
                .await
                .is_err()
            {
                return;
            }
            for delta in deltas {
                if tx.send(Msg::Event { delta }).await.is_err() {
                    return;
                }
            }
            let _ = tx
                .send(Msg::Event {
                    delta: Delta::BatchEnd { id: batch_id },
                })
                .await;
        }
        tokio::time::sleep(poll).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONNECT_LINE: &str = r#"{"ts":"2024-01-01T00:00:00Z","event":"connect","target":"198.51.100.7","port":443,"proto":"tcp"}"#;
    const LATERAL_LINE: &str = r#"{"ts":"2024-01-01T00:00:05Z","event":"lateral","src":"198.51.100.7","target":"198.51.100.9"}"#;

    #[test]
    fn parses_connect_event() {
        let ev = parse_nebula_event(CONNECT_LINE).expect("parsed");
        assert_eq!(ev.event, "connect");
        assert_eq!(ev.target, "198.51.100.7");
        assert_eq!(ev.src, None);
        assert!(parse_nebula_event("not json").is_none());
        // A line without a target is ignored.
        assert!(parse_nebula_event(r#"{"event":"finding"}"#).is_none());
    }

    #[test]
    fn connect_emits_remote_host_only() {
        let ev = parse_nebula_event(CONNECT_LINE).unwrap();
        let (nodes, edges) = build_nebula_graph("nebula-lab", &ev);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].0, id_remote_host("nebula-lab", "198.51.100.7"));
        assert!(matches!(nodes[0].1, Node::RemoteHost { .. }));
        assert!(edges.is_empty());
    }

    #[test]
    fn lateral_emits_connects_to_between_targets() {
        let ev = parse_nebula_event(LATERAL_LINE).unwrap();
        let (nodes, edges) = build_nebula_graph("nebula-lab", &ev);
        // src + target remote hosts, one ConnectsTo edge (existing kind, no wire).
        assert_eq!(nodes.len(), 2);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].kind, EdgeKind::ConnectsTo);
        assert_eq!(edges[0].from, id_remote_host("nebula-lab", "198.51.100.7"));
        assert_eq!(edges[0].to, id_remote_host("nebula-lab", "198.51.100.9"));
    }

    #[test]
    fn fixture_yields_expected_events() {
        // Committed fixture documents the assumed schema (A.5 — verify on host).
        let fixture = include_str!("fixtures/nebula.jsonl");
        let events: Vec<NebulaEvent> = fixture.lines().filter_map(parse_nebula_event).collect();
        assert_eq!(
            events.len(),
            3,
            "3 targeted events; the finding line without a target is skipped"
        );
        assert!(events
            .iter()
            .any(|e| e.event == "lateral" && e.src.is_some()));
    }
}
