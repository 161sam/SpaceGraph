//! Suricata EVE [`EventSource`]: tail an EVE JSON file, turn `alert` events into
//! `Alert` nodes correlated (by 5-tuple shared id) to remote hosts.
//!
//! Correlation is implicit: an alert attaches to `id_remote_host(node_id,
//! external_addr)` — the same id the network source emits for an active
//! connection — so a correlated alert lands on the existing remote-host node,
//! and an uncorrelated one creates it. Parsing and graph-building are pure
//! functions (fixture-tested); only the tail loop touches the filesystem.

use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use spacegraph_core::{id_alert, id_remote_host, Delta, Edge, EdgeKind, Msg, Node, NodeId};
use tokio::sync::mpsc;

use super::EventSource;

/// A parsed EVE `alert` event (only the fields we use).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EveAlert {
    pub timestamp: String,
    pub signature: String,
    pub severity: String,
    pub src_ip: String,
    pub src_port: u16,
    pub dest_ip: String,
    pub dest_port: u16,
    pub proto: String,
}

/// Map Suricata's numeric severity (1 = highest) to a name.
pub fn severity_name(sev: i64) -> &'static str {
    match sev {
        1 => "high",
        2 => "medium",
        _ => "low",
    }
}

/// Parse one EVE JSON line; returns `Some` only for `event_type == "alert"`.
pub fn parse_eve_alert(line: &str) -> Option<EveAlert> {
    let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    if v.get("event_type")?.as_str()? != "alert" {
        return None;
    }
    let alert = v.get("alert")?;
    let signature = alert
        .get("signature")
        .and_then(|s| s.as_str())
        .unwrap_or("unknown")
        .to_string();
    let severity =
        severity_name(alert.get("severity").and_then(|s| s.as_i64()).unwrap_or(3)).to_string();
    let str_field = |k: &str| v.get(k).and_then(|s| s.as_str()).unwrap_or("").to_string();
    let port_field = |k: &str| v.get(k).and_then(|s| s.as_u64()).unwrap_or(0) as u16;

    Some(EveAlert {
        timestamp: str_field("timestamp"),
        signature,
        severity,
        src_ip: str_field("src_ip"),
        src_port: port_field("src_port"),
        dest_ip: str_field("dest_ip"),
        dest_port: port_field("dest_port"),
        proto: v
            .get("proto")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

/// The external (non-local-host) address an alert is about. MVP heuristic:
/// prefer the destination unless it is loopback, then fall back to source.
fn external_addr(a: &EveAlert) -> &str {
    let dest_loopback = a.dest_ip.starts_with("127.") || a.dest_ip == "::1";
    if dest_loopback && !a.src_ip.is_empty() {
        &a.src_ip
    } else {
        &a.dest_ip
    }
}

/// Build the (alert node, remote-host node, alerts_on edge) for an alert.
/// The remote-host id matches the network source's, so a live connection is
/// correlated; otherwise the remote host is created (uncorrelated but shown).
pub fn build_alert_graph(node_id: &str, a: &EveAlert) -> (Vec<(NodeId, Node)>, Vec<Edge>) {
    let addr = external_addr(a);
    let remote_id = id_remote_host(node_id, addr);
    let key = format!(
        "{}|{}|{}:{}->{}:{}",
        a.timestamp, a.signature, a.src_ip, a.src_port, a.dest_ip, a.dest_port
    );
    let alert_id = id_alert(node_id, &key);

    let nodes = vec![
        (
            remote_id.clone(),
            Node::RemoteHost {
                addr: addr.to_string(),
                rdns: None,
            },
        ),
        (
            alert_id.clone(),
            Node::Alert {
                source: "suricata".to_string(),
                signature: a.signature.clone(),
                severity: a.severity.clone(),
                ts: a.timestamp.clone(),
            },
        ),
    ];
    let edges = vec![Edge {
        from: alert_id,
        to: remote_id,
        kind: EdgeKind::AlertsOn,
    }];
    (nodes, edges)
}

/// Deltas to emit for a freshly parsed alert (upserts only; the viewer caps).
pub fn alert_deltas(node_id: &str, a: &EveAlert) -> Vec<Delta> {
    let (nodes, edges) = build_alert_graph(node_id, a);
    let mut out = Vec::new();
    for (id, node) in nodes {
        out.push(Delta::UpsertNode { id, node });
    }
    for edge in edges {
        out.push(Delta::UpsertEdge { edge });
    }
    out
}

/// The EVE alert event source.
pub struct SuricataEveSource {
    pub eve_file: PathBuf,
    pub poll_interval: Duration,
}

impl EventSource for SuricataEveSource {
    fn name(&self) -> &'static str {
        "suricata_eve"
    }
    fn start(self: Box<Self>, node_id: String, tx: mpsc::Sender<Msg>) -> Result<()> {
        tokio::spawn(run(self.eve_file, self.poll_interval, node_id, tx));
        Ok(())
    }
}

/// Read any bytes appended to the file since `offset`; handles truncation /
/// rotation by resetting the offset when the file shrinks.
fn read_new(path: &PathBuf, offset: &mut u64) -> Vec<String> {
    let Ok(mut f) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    if len < *offset {
        *offset = 0; // file truncated/rotated
    }
    if f.seek(SeekFrom::Start(*offset)).is_err() {
        return Vec::new();
    }
    let mut buf = String::new();
    if f.read_to_string(&mut buf).is_err() {
        return Vec::new();
    }
    // Only consume up to the last newline so partial lines aren't parsed.
    if let Some(last_nl) = buf.rfind('\n') {
        *offset += (last_nl + 1) as u64;
        buf[..=last_nl].lines().map(|l| l.to_string()).collect()
    } else {
        Vec::new()
    }
}

async fn run(eve_file: PathBuf, poll: Duration, node_id: String, tx: mpsc::Sender<Msg>) {
    let mut offset: u64 = 0;
    let mut batch_id: u64 = 0;
    loop {
        let path = eve_file.clone();
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
            if let Some(alert) = parse_eve_alert(line) {
                deltas.extend(alert_deltas(&node_id, &alert));
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

    const ALERT_LINE: &str = r#"{"timestamp":"2024-01-01T12:00:00.000000+0000","event_type":"alert","src_ip":"10.0.0.5","src_port":54321,"dest_ip":"93.184.216.34","dest_port":443,"proto":"TCP","alert":{"signature":"ET MALWARE Test","severity":1}}"#;
    const FLOW_LINE: &str = r#"{"timestamp":"2024-01-01T12:00:01.000000+0000","event_type":"flow","src_ip":"10.0.0.5"}"#;

    #[test]
    fn parses_alert_and_ignores_non_alert() {
        let a = parse_eve_alert(ALERT_LINE).expect("alert parsed");
        assert_eq!(a.signature, "ET MALWARE Test");
        assert_eq!(a.severity, "high");
        assert_eq!(a.dest_ip, "93.184.216.34");
        assert_eq!(a.dest_port, 443);
        assert!(parse_eve_alert(FLOW_LINE).is_none());
        assert!(parse_eve_alert("not json").is_none());
    }

    #[test]
    fn correlates_to_remote_host_by_shared_id() {
        let a = parse_eve_alert(ALERT_LINE).unwrap();
        let (nodes, edges) = build_alert_graph("host", &a);
        // The remote host id matches what the network source would emit (hit).
        let expected_remote = id_remote_host("host", "93.184.216.34");
        assert!(nodes.iter().any(|(id, _)| *id == expected_remote));
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].to, expected_remote);
        assert_eq!(edges[0].kind, EdgeKind::AlertsOn);
        // The alert node carries severity/signature.
        assert!(nodes
            .iter()
            .any(|(_, n)| matches!(n, Node::Alert { severity, .. } if severity == "high")));
    }

    #[test]
    fn loopback_dest_falls_back_to_source() {
        let line = r#"{"timestamp":"t","event_type":"alert","src_ip":"203.0.113.9","src_port":1,"dest_ip":"127.0.0.1","dest_port":80,"proto":"TCP","alert":{"signature":"x","severity":2}}"#;
        let a = parse_eve_alert(line).unwrap();
        assert_eq!(external_addr(&a), "203.0.113.9");
    }

    #[test]
    fn severity_mapping() {
        assert_eq!(severity_name(1), "high");
        assert_eq!(severity_name(2), "medium");
        assert_eq!(severity_name(3), "low");
        assert_eq!(severity_name(9), "low");
    }

    #[test]
    fn fixture_file_yields_three_alerts() {
        // Committed EVE fixture (mixed event types); only alerts are parsed.
        let fixture = include_str!("fixtures/suricata_eve.jsonl");
        let alerts: Vec<EveAlert> = fixture.lines().filter_map(parse_eve_alert).collect();
        assert_eq!(alerts.len(), 3);
        let sevs: Vec<&str> = alerts.iter().map(|a| a.severity.as_str()).collect();
        assert_eq!(sevs, vec!["high", "medium", "low"]);
    }
}
