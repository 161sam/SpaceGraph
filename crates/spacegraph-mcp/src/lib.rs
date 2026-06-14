//! `spacegraph-mcp` — a standalone **read-only** MCP stdio server hosting the
//! headless [`spacegraph_graph::GraphCore`] (ADR-0001 / MP-v0.6.0 P5).
//!
//! The ESN hub spawns this binary by `command` and proxies its tools as
//! `mcp__spacegraph__*`. It speaks newline-delimited JSON-RPC 2.0 over stdio
//! (the MCP stdio transport): `initialize`, `tools/list`, `tools/call`, `ping`.
//!
//! **Read-only only (O-7'):** every tool is a query over the canonical state —
//! topology, node lookup, alert feed, explain-path, campaigns, coverage, posture.
//! There are **no action / mutating tools**, and no agent egress. The tool result
//! schema is this crate's own contract (JSON), distinct from the agent wire
//! (`PROTOCOL_VERSION` unchanged, O-8).
//!
//! This module is the transport-agnostic core (tool catalog + dispatch +
//! JSON-RPC handling) so it is unit-/contract-tested without spawning a process;
//! `main.rs` wires it to stdio + the live agent-UDS ingest.

use std::sync::Mutex;

use serde_json::{json, Value};

use spacegraph_core::NodeId;
use spacegraph_graph::correlation::Campaign;
use spacegraph_graph::coverage::TacticCoverage;
use spacegraph_graph::explain::PathStep;
use spacegraph_graph::graph_core::{AlertView, NodeDetail, TopologyStats};
use spacegraph_graph::model::edge_class_name;
use spacegraph_graph::posture::Posture;
use spacegraph_graph::rules::Tactic;
use spacegraph_graph::GraphCore;

/// MCP protocol revision this server implements.
pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";
/// The server id the hub registers (`mcp__spacegraph__*`).
pub const SERVER_NAME: &str = "spacegraph";
/// This crate's version, reported in `initialize`.
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default alert-feed page size when a caller omits `limit`.
const DEFAULT_ALERTS_LIMIT: usize = 50;
/// Default explain-path search bound when a caller omits `max_depth`.
const DEFAULT_EXPLAIN_DEPTH: usize = 8;

/// The names of every tool this server exposes — the read-only surface. Used by
/// `tools/list` and the read-only-only audit test.
pub const TOOL_NAMES: &[&str] = &[
    "topology_stats",
    "node",
    "alerts",
    "explain_path",
    "campaigns",
    "coverage",
    "posture",
];

/// The `tools/list` result: the read-only tool catalog with JSON input schemas.
/// **No action/mutating tools** (O-7').
pub fn tools_list() -> Value {
    json!({
        "tools": [
            {
                "name": "topology_stats",
                "description": "Summary counts of the canonical graph: nodes (by kind), edges, aggregated edges, and alerts. Read-only.",
                "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
            },
            {
                "name": "node",
                "description": "Look up a node by id with its incident degree and neighbor ids. Read-only.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "id": { "type": "string", "description": "The node id." } },
                    "required": ["id"],
                    "additionalProperties": false
                }
            },
            {
                "name": "alerts",
                "description": "The alert feed (rule detections + ingested alerts), newest first, with the subject each was raised on. Read-only.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "limit": { "type": "integer", "minimum": 1, "description": "Max alerts to return (default 50)." } },
                    "additionalProperties": false
                }
            },
            {
                "name": "explain_path",
                "description": "Shortest causal path between two nodes over the graph, as typed edge steps. Read-only.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "from": { "type": "string", "description": "Source node id." },
                        "to": { "type": "string", "description": "Target node id." },
                        "max_depth": { "type": "integer", "minimum": 1, "description": "Search depth bound (default 8)." }
                    },
                    "required": ["from", "to"],
                    "additionalProperties": false
                }
            },
            {
                "name": "campaigns",
                "description": "Multi-stage attack campaigns correlated from detections (D3): shared/adjacent subjects spanning >=2 ATT&CK tactics. Read-only.",
                "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
            },
            {
                "name": "coverage",
                "description": "ATT&CK detection coverage (detected vs undetected techniques, grouped by tactic) from the rule registry (D5). Read-only.",
                "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
            },
            {
                "name": "posture",
                "description": "Deterministic posture/exposure score (0..100) over the current graph: attack surface + alert density amplified by the detection-coverage gap (D5). Read-only.",
                "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
            }
        ]
    })
}

/// Dispatch a read-only tool call against `core`, returning the structured JSON
/// result. `Err` carries a human-readable message for an unknown tool or bad
/// arguments (surfaced as an MCP `isError` result, not a transport error).
pub fn call_tool(core: &GraphCore, name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "topology_stats" => Ok(topology_json(&core.topology_stats())),
        "node" => {
            let id = require_str(args, "id")?;
            match core.node_detail(&NodeId(id.clone())) {
                Some(detail) => Ok(node_detail_json(&detail)),
                None => Ok(json!({ "found": false, "id": id })),
            }
        }
        "alerts" => {
            let limit = opt_usize(args, "limit").unwrap_or(DEFAULT_ALERTS_LIMIT);
            let alerts: Vec<Value> = core.alerts(limit).iter().map(alert_json).collect();
            Ok(json!({ "alerts": alerts }))
        }
        "explain_path" => {
            let from = NodeId(require_str(args, "from")?);
            let to = NodeId(require_str(args, "to")?);
            let depth = opt_usize(args, "max_depth").unwrap_or(DEFAULT_EXPLAIN_DEPTH);
            match core.explain_path(&from, &to, depth) {
                Some(steps) => Ok(json!({
                    "found": true,
                    "path": steps.iter().map(path_step_json).collect::<Vec<_>>(),
                })),
                None => Ok(json!({ "found": false, "path": [] })),
            }
        }
        "campaigns" => Ok(json!({
            "campaigns": core.campaigns().iter().map(campaign_json).collect::<Vec<_>>(),
        })),
        "coverage" => Ok(json!({
            "coverage": core.coverage().iter().map(tactic_coverage_json).collect::<Vec<_>>(),
        })),
        "posture" => Ok(posture_json(&core.posture())),
        other => Err(format!("unknown tool: {other}")),
    }
}

/// Handle one JSON-RPC 2.0 message. Returns the response value to write back, or
/// `None` for a notification (no `id` → no reply). Pure given the locked `core`.
pub fn handle_message(core: &Mutex<GraphCore>, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    match method {
        "initialize" => Some(ok(id, initialize_result())),
        "tools/list" => Some(ok(id, tools_list())),
        "ping" => Some(ok(id, json!({}))),
        "tools/call" => {
            let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let guard = core.lock().expect("graph core mutex poisoned");
            let result = match call_tool(&guard, name, &args) {
                Ok(value) => tool_result_content(value),
                Err(message) => tool_error_content(message),
            };
            Some(ok(id, result))
        }
        // Any notification (no id) — e.g. `notifications/initialized` — is silent.
        _ if id.is_none() => None,
        _ => Some(err(id, -32601, format!("method not found: {method}"))),
    }
}

/// The `initialize` result: protocol revision, advertised capabilities, server id.
pub fn initialize_result() -> Value {
    json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION }
    })
}

// ---- JSON-RPC envelope helpers ----------------------------------------------

fn ok(id: Option<Value>, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "result": result })
}

fn err(id: Option<Value>, code: i64, message: String) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "error": { "code": code, "message": message } })
}

/// Wrap a structured tool result in the MCP `tools/call` shape: a text block (the
/// JSON, for any client) plus `structuredContent` (for structured clients).
fn tool_result_content(value: Value) -> Value {
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    json!({
        "content": [ { "type": "text", "text": text } ],
        "structuredContent": value,
        "isError": false
    })
}

fn tool_error_content(message: String) -> Value {
    json!({
        "content": [ { "type": "text", "text": message } ],
        "isError": true
    })
}

// ---- Argument helpers --------------------------------------------------------

fn require_str(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("missing required string argument: {key}"))
}

fn opt_usize(args: &Value, key: &str) -> Option<usize> {
    args.get(key).and_then(Value::as_u64).map(|n| n as usize)
}

// ---- Core type -> JSON (this crate's contract; ADR-0001 §3) ------------------

fn topology_json(s: &TopologyStats) -> Value {
    json!({
        "nodes": s.nodes,
        "edges": s.edges,
        "agg_edges": s.agg_edges,
        "alerts": s.alerts,
        "by_kind": {
            "process": s.processes,
            "file": s.files,
            "user": s.users,
            "socket": s.sockets,
            "remote_host": s.remote_hosts,
            "alert": s.alerts
        }
    })
}

fn node_detail_json(d: &NodeDetail) -> Value {
    json!({
        "found": true,
        "id": d.id.0,
        "node": serde_json::to_value(&d.node).unwrap_or(Value::Null),
        "degree": d.degree,
        "neighbors": d.neighbors.iter().map(|n| n.0.clone()).collect::<Vec<_>>()
    })
}

fn alert_json(a: &AlertView) -> Value {
    json!({
        "id": a.id.0,
        "source": a.source,
        "signature": a.signature,
        "severity": a.severity,
        "subject": a.subject.as_ref().map(|s| s.0.clone())
    })
}

fn path_step_json(step: &PathStep) -> Value {
    json!({
        "from": step.from.0,
        "to": step.to.0,
        "class": edge_class_name(step.class)
    })
}

fn campaign_json(c: &Campaign) -> Value {
    json!({
        "key": c.key,
        "subjects": c.subjects.iter().map(|n| n.0.clone()).collect::<Vec<_>>(),
        "alerts": c.alerts.iter().map(|n| n.0.clone()).collect::<Vec<_>>(),
        "tactics": c.tactics.iter().map(tactic_name).collect::<Vec<_>>()
    })
}

fn tactic_coverage_json(t: &TacticCoverage) -> Value {
    json!({
        "tactic": tactic_name(&t.tactic),
        "detected": t.detected,
        "total": t.total,
        "techniques": t.techniques.iter().map(|tech| json!({
            "id": tech.id,
            "name": tech.name,
            "detected": tech.detected
        })).collect::<Vec<_>>()
    })
}

fn posture_json(p: &Posture) -> Value {
    json!({
        "coverage": p.coverage,
        "exposed_listeners": p.exposed_listeners,
        "alert_count": p.alert_count,
        "node_count": p.node_count,
        "score": p.score
    })
}

/// Stable tactic name for the JSON contract (the `Tactic` enum variant).
fn tactic_name(t: &Tactic) -> String {
    format!("{t:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use spacegraph_core::{Edge, EdgeKind, FileKind, Node};

    fn nid(s: &str) -> NodeId {
        NodeId(s.into())
    }

    /// A fixture core: a process listening on an unusual port (trips T1571) plus a
    /// pre-existing ingested alert — so every read-only tool returns content.
    fn fixture() -> GraphCore {
        let mut c = GraphCore::default();
        c.apply_snapshot(
            vec![
                (
                    nid("p"),
                    Node::Process {
                        pid: 1,
                        ppid: 0,
                        exe: "p".into(),
                        cmdline: String::new(),
                        uid: 0,
                    },
                ),
                (
                    nid("s"),
                    Node::Socket {
                        proto: "tcp".into(),
                        local_addr: "0.0.0.0".into(),
                        local_port: 4444,
                        state: "LISTEN".into(),
                    },
                ),
                (
                    nid("f"),
                    Node::File {
                        path: "/tmp/x".into(),
                        inode: 1,
                        kind: FileKind::Regular,
                    },
                ),
            ],
            vec![Edge {
                from: nid("p"),
                to: nid("s"),
                kind: EdgeKind::ListensOn,
            }],
        );
        // Run the detection pipeline so alerts/campaigns/coverage reflect state.
        let _ = c.run_detection(&std::collections::HashSet::new(), 512);
        c
    }

    #[test]
    fn tools_list_is_the_read_only_catalog() {
        let list = tools_list();
        let tools = list["tools"].as_array().expect("tools array");
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, TOOL_NAMES);
        // Every tool has a description + an object input schema.
        for t in tools {
            assert!(t["description"].as_str().unwrap().contains("Read-only"));
            assert_eq!(t["inputSchema"]["type"], "object");
        }
    }

    #[test]
    fn audit_no_mutating_tools() {
        // O-7': the surface is read-only. No tool name implies an action/mutation,
        // and the catalog matches the audited read-only set exactly.
        let banned = [
            "set", "update", "delete", "remove", "create", "write", "exec", "run", "kill", "start",
            "stop", "patch", "apply", "ingest", "send", "scan", "mutate", "emit", "clear",
        ];
        for name in TOOL_NAMES {
            for verb in banned {
                assert!(
                    !name.contains(verb),
                    "tool {name} looks mutating (contains {verb}) — O-7' read-only only"
                );
            }
        }
    }

    #[test]
    fn topology_stats_tool() {
        let c = fixture();
        let v = call_tool(&c, "topology_stats", &json!({})).unwrap();
        assert_eq!(v["by_kind"]["process"], 1);
        assert_eq!(v["by_kind"]["socket"], 1);
        assert_eq!(v["by_kind"]["file"], 1);
        assert!(
            v["alerts"].as_u64().unwrap() >= 1,
            "the listener detection alert"
        );
        // 3 seed nodes (process, socket, file) + the emitted detection alert.
        assert_eq!(v["nodes"], 4);
        // 1 seed edge (listens_on) + the detection's alerts_on edge.
        assert_eq!(v["edges"], 2);
    }

    #[test]
    fn node_tool_found_and_missing() {
        let c = fixture();
        let found = call_tool(&c, "node", &json!({ "id": "p" })).unwrap();
        assert_eq!(found["found"], true);
        assert_eq!(found["id"], "p");
        assert_eq!(found["degree"], 1);
        assert_eq!(found["neighbors"], json!(["s"]));

        let missing = call_tool(&c, "node", &json!({ "id": "nope" })).unwrap();
        assert_eq!(missing["found"], false);

        // Missing required arg -> Err (surfaced as isError, not a panic).
        assert!(call_tool(&c, "node", &json!({})).is_err());
    }

    #[test]
    fn alerts_tool_lists_detections_with_subject() {
        let c = fixture();
        let v = call_tool(&c, "alerts", &json!({ "limit": 10 })).unwrap();
        let alerts = v["alerts"].as_array().unwrap();
        assert!(!alerts.is_empty(), "detection produced an alert");
        let a = &alerts[0];
        assert_eq!(a["source"], "spacegraph-rule");
        assert_eq!(a["subject"], "s", "raised on the suspicious socket");
        assert!(a["signature"].as_str().unwrap().contains("T1571"));
    }

    #[test]
    fn explain_path_tool() {
        let c = fixture();
        let v = call_tool(&c, "explain_path", &json!({ "from": "p", "to": "s" })).unwrap();
        assert_eq!(v["found"], true);
        let path = v["path"].as_array().unwrap();
        assert_eq!(path.len(), 1);
        assert_eq!(path[0]["from"], "p");
        assert_eq!(path[0]["to"], "s");
        assert_eq!(path[0]["class"], "listens_on");

        let none = call_tool(&c, "explain_path", &json!({ "from": "p", "to": "f" })).unwrap();
        assert_eq!(none["found"], false);
    }

    #[test]
    fn coverage_posture_campaigns_tools() {
        let c = fixture();
        let cov = call_tool(&c, "coverage", &json!({})).unwrap();
        assert!(!cov["coverage"].as_array().unwrap().is_empty());

        let pos = call_tool(&c, "posture", &json!({})).unwrap();
        let score = pos["score"].as_f64().unwrap();
        assert!((0.0..=100.0).contains(&score));

        let camp = call_tool(&c, "campaigns", &json!({})).unwrap();
        assert!(camp["campaigns"].is_array());
    }

    #[test]
    fn unknown_tool_is_err() {
        let c = fixture();
        assert!(call_tool(&c, "delete_everything", &json!({})).is_err());
    }

    #[test]
    fn jsonrpc_initialize_list_and_call() {
        let core = Mutex::new(fixture());

        let init = handle_message(
            &core,
            &json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}
            }),
        )
        .unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], SERVER_NAME);
        assert_eq!(init["result"]["protocolVersion"], MCP_PROTOCOL_VERSION);
        assert_eq!(init["id"], 1);

        let list = handle_message(
            &core,
            &json!({
                "jsonrpc": "2.0", "id": 2, "method": "tools/list"
            }),
        )
        .unwrap();
        assert_eq!(
            list["result"]["tools"].as_array().unwrap().len(),
            TOOL_NAMES.len()
        );

        let call = handle_message(
            &core,
            &json!({
                "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": { "name": "posture", "arguments": {} }
            }),
        )
        .unwrap();
        assert_eq!(call["result"]["isError"], false);
        assert!(call["result"]["content"][0]["text"].is_string());
        assert!(call["result"]["structuredContent"]["score"].is_number());
    }

    #[test]
    fn jsonrpc_notification_has_no_reply_and_unknown_method_errs() {
        let core = Mutex::new(fixture());
        // Notification (no id) -> no response.
        assert!(handle_message(
            &core,
            &json!({
                "jsonrpc": "2.0", "method": "notifications/initialized"
            })
        )
        .is_none());
        // Unknown method (with id) -> JSON-RPC error.
        let e = handle_message(
            &core,
            &json!({
                "jsonrpc": "2.0", "id": 9, "method": "bogus/method"
            }),
        )
        .unwrap();
        assert_eq!(e["error"]["code"], -32601);
    }

    #[test]
    fn tools_call_unknown_tool_is_iserror_not_transport_error() {
        let core = Mutex::new(fixture());
        let resp = handle_message(
            &core,
            &json!({
                "jsonrpc": "2.0", "id": 4, "method": "tools/call",
                "params": { "name": "nope", "arguments": {} }
            }),
        )
        .unwrap();
        assert_eq!(resp["result"]["isError"], true);
    }
}
