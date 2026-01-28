mod config;
mod path_policy;
mod server;
mod snapshot;
mod watch_fs;
mod watch_proc;

use anyhow::Result;
use config::{default_excludes, default_includes, parse_args, should_warn_privileged_without_root};
use path_policy::PathPolicy;
use spacegraph_core::{Capabilities, Delta, Msg, NodeIdentity};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

fn init_tracing() {
    let _ = tracing_subscriber::fmt::try_init();
}

fn runtime_sock_path() -> String {
    // Wayland-friendly: prefer XDG_RUNTIME_DIR
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        format!("{dir}/spacegraph.sock")
    } else {
        "/tmp/spacegraph.sock".to_string()
    }
}

fn default_node_id() -> String {
    std::env::var("SPACEGRAPH_NODE_ID")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .unwrap_or_else(|| "node".to_string())
        })
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let node_id = default_node_id();
    let sock_path = runtime_sock_path();
    let config = parse_args()?;
    let default_roots = default_includes(config.mode);

    let includes = if config.includes.is_empty() {
        default_roots.clone()
    } else {
        config.includes
    };
    let excludes = if config.excludes.is_empty() {
        default_excludes(config.mode)
    } else {
        config.excludes
    };

    let mut policy = PathPolicy::new(includes, excludes);
    policy.normalize();
    let policy = Arc::new(policy);

    let watch_roots = policy.includes().to_vec();
    let effective_root_count = watch_roots
        .iter()
        .filter(|root| root.exists() && policy.should_watch(root))
        .count();

    tracing::info!(
        includes = ?policy.includes(),
        excludes = ?policy.excludes(),
        effective_root_count,
        "path policy configured"
    );

    if should_warn_privileged_without_root(config.mode, unsafe { libc::geteuid() }) {
        tracing::warn!(
            "Privileged mode requested but not running as root; some paths will be skipped."
        );
    }

    // Clean stale socket
    let _ = std::fs::remove_file(&sock_path);

    // Build initial snapshot
    let (snap_nodes, snap_edges) = snapshot::build_snapshot(&node_id, &policy, config.mode)?;
    let snapshot_node_events: Vec<Msg> = snap_nodes
        .iter()
        .cloned()
        .map(|(id, node)| Msg::Event {
            delta: Delta::UpsertNode { id, node },
        })
        .collect();
    let snapshot_msg = Msg::Snapshot {
        nodes: snap_nodes,
        edges: snap_edges,
    };

    // Identity + capabilities (MVP)
    let ident = NodeIdentity {
        node_id: node_id.clone(),
        hostname: hostname::get()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        platform: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    };
    let caps = Capabilities {
        procfs: true,
        fd_edges: true,
        fs_notify: true,
        proc_poll: true,
        ebpf: false,
        cloud: false,
        windows: false,
    };
    let identity_msg = Msg::Identity { ident, caps };

    // Event bus (broadcast so multiple viewers can subscribe)
    let (bus_tx, _bus_rx) = broadcast::channel::<Msg>(32_768);

    // Watchers publish to bus
    let (fs_tx, fs_rx) = mpsc::channel::<Msg>(8192);
    let (proc_tx, proc_rx) = mpsc::channel::<Msg>(8192);

    watch_fs::spawn(
        &node_id,
        config.mode,
        Arc::clone(&policy),
        watch_roots,
        fs_tx,
    )?;
    watch_proc::spawn(&node_id, proc_tx)?;

    // Forward watcher channels → broadcast bus
    {
        let bus_tx = bus_tx.clone();
        tokio::spawn(async move {
            forward_to_bus(fs_rx, bus_tx).await;
        });
    }
    {
        let bus_tx = bus_tx.clone();
        tokio::spawn(async move {
            forward_to_bus(proc_rx, bus_tx).await;
        });
    }

    // Serve UDS
    server::run(
        &sock_path,
        identity_msg,
        snapshot_msg,
        snapshot_node_events,
        bus_tx,
    )
    .await
}

async fn forward_to_bus(mut rx: mpsc::Receiver<Msg>, bus_tx: broadcast::Sender<Msg>) {
    while let Some(msg) = rx.recv().await {
        // ignore lagging viewers
        let _ = bus_tx.send(msg);
    }
}
