//! `spacegraph-mcp` binary — the read-only MCP stdio server the ESN hub spawns.
//!
//! It hosts a headless [`GraphCore`], ingests from one `spacegraph-agent` over UDS
//! independently of the viewer, and serves the read-only tools over newline-
//! delimited JSON-RPC 2.0 on stdin/stdout. All diagnostics go to **stderr**
//! (stdout is the protocol channel). Read-only — no agent egress, no mutation.
//!
//! Agent socket: positional arg 1, else `$SPACEGRAPH_AGENT_UDS`. With neither, the
//! server still starts and serves an empty graph (so `tools/list` works without an
//! agent) — handshake/ingest begin once a socket is provided.

use std::collections::HashSet;
use std::io::{self, BufRead, Write};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use spacegraph_core::Msg;
use spacegraph_graph::graph_core::DEFAULT_ALERT_CAP;
use spacegraph_graph::net::{spawn_reader, Incoming, IncomingKind};
use spacegraph_graph::GraphCore;

fn main() {
    let core = Arc::new(Mutex::new(GraphCore::default()));

    if let Some(sock_path) = agent_socket_path() {
        spawn_ingest(Arc::clone(&core), sock_path);
    } else {
        eprintln!(
            "spacegraph-mcp: no agent socket (arg 1 or $SPACEGRAPH_AGENT_UDS); \
             serving an empty graph until one is provided"
        );
    }

    serve_stdio(&core);
}

fn agent_socket_path() -> Option<String> {
    std::env::args()
        .nth(1)
        .filter(|a| !a.starts_with('-'))
        .or_else(|| std::env::var("SPACEGRAPH_AGENT_UDS").ok())
        .filter(|p| !p.is_empty())
}

/// Background ingest: connect to the agent over UDS, apply decoded snapshots/deltas
/// to the shared core, and run the detection pipeline so the read-only queries
/// reflect live state. Read-only — the outbound channel is held open but never
/// used (the MCP issues no agent requests).
fn spawn_ingest(core: Arc<Mutex<GraphCore>>, sock_path: String) {
    let (tx, rx) = crossbeam_channel::unbounded::<Incoming>();
    // Hold the outbound sender for the reader's lifetime: dropping it would make
    // the reader's `outbound_rx.recv()` return `None` and tear the stream down.
    let (outbound_tx, outbound_rx) = tokio::sync::mpsc::channel::<Msg>(1);
    let _reader = spawn_reader("agent".to_string(), sock_path, tx, outbound_rx);

    std::thread::spawn(move || {
        // Keep the outbound sender alive alongside the reader handle.
        let _outbound_tx = outbound_tx;
        let _reader = _reader;
        let mut active: HashSet<spacegraph_core::NodeId> = HashSet::new();
        for inc in rx.iter() {
            let mut graph = core.lock().expect("graph core mutex poisoned");
            let changed = apply_incoming(&mut graph, inc);
            if changed {
                let (next, _evicted) = graph.run_detection(&active, DEFAULT_ALERT_CAP);
                active = next;
            }
        }
        eprintln!("spacegraph-mcp: agent ingest channel closed");
    });
}

/// Apply one decoded agent message to the canonical state. Returns whether the
/// graph changed (so the caller re-runs detection). Non-graph events
/// (connect/identity/search/errors) are ignored by this read-only consumer.
fn apply_incoming(graph: &mut GraphCore, inc: Incoming) -> bool {
    match inc.kind {
        IncomingKind::Snapshot(Msg::Snapshot { nodes, edges }) => {
            graph.apply_snapshot(nodes, edges);
            true
        }
        IncomingKind::Event(Msg::Event { delta }) => {
            graph.apply_delta(delta, DEFAULT_ALERT_CAP);
            true
        }
        _ => false,
    }
}

/// The JSON-RPC 2.0 stdio loop: one message per line in, one response per line
/// out, flushed. Notifications produce no reply.
fn serve_stdio(core: &Mutex<GraphCore>) {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("spacegraph-mcp: stdin read error: {e}");
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => spacegraph_mcp::handle_message(core, &msg),
            Err(e) => Some(parse_error(format!("parse error: {e}"))),
        };
        if let Some(resp) = response {
            if writeln!(stdout, "{resp}")
                .and_then(|_| stdout.flush())
                .is_err()
            {
                break; // stdout closed — the hub went away.
            }
        }
    }
}

fn parse_error(message: String) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": Value::Null,
        "error": { "code": -32700, "message": message }
    })
}
