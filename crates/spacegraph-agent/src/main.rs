mod config;
mod index;
mod path_policy;
mod server;
mod snapshot;
mod sources;
mod watch_fs;
mod watch_proc;

use anyhow::Result;
use config::{default_excludes, default_includes, parse_args, should_warn_privileged_without_root};
use index::{FsIndex, Walker};
use path_policy::PathPolicy;
use sources::nebula::NebulaSource;
use sources::net::{NetConfig, NetSource};
use sources::suricata_eve::SuricataEveSource;
use sources::{EventSource, FsSource, ProcSource};
use spacegraph_core::{Capabilities, Delta, Msg, NodeIdentity};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

fn init_tracing() {
    let _ = tracing_subscriber::fmt::try_init();
}

fn default_uds_path() -> String {
    #[cfg(unix)]
    {
        let uid = unsafe { libc::geteuid() };
        let run_dir = format!("/run/user/{uid}");
        if std::path::Path::new(&run_dir).is_dir() {
            return format!("{run_dir}/spacegraph.sock");
        }
    }
    "/tmp/spacegraph.sock".to_string()
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
    let config = parse_args()?;
    let sock_path = config
        .uds_path
        .as_ref()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(default_uds_path);
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
    let snapshot_node_count = snap_nodes.len();
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
        // v4: this agent serves filesystem search/materialise requests.
        fs_search: true,
    };
    let identity_msg = Msg::Identity { ident, caps };

    // Filesystem search index (spec §2). The builtin walker walks the scoped
    // roots in the background (so startup is not blocked) and is kept fresh by
    // the FS watcher's inotify events; `fs_walker` is the handle we hand to
    // `FsSource`. The agent never shells out (O-7' no-exec).
    let (fs_index, fs_walker) =
        build_fs_index(&node_id, (*policy).clone(), config.mode, &watch_roots);
    let fs_index = Arc::new(fs_index);

    // Event bus (broadcast so multiple viewers can subscribe)
    let (bus_tx, _bus_rx) = broadcast::channel::<Msg>(32_768);

    // Serve UDS early so viewers can connect
    let server_handle = {
        let sock_path = sock_path.clone();
        let bus_tx = bus_tx.clone();
        let fs_index = Arc::clone(&fs_index);
        tokio::spawn(async move {
            server::run(
                &sock_path,
                identity_msg,
                snapshot_msg,
                snapshot_node_events,
                bus_tx,
                fs_index,
            )
            .await
        })
    };

    tracing::info!(
        uds_path = %sock_path,
        mode = ?config.mode,
        include_root_count = policy.includes().len(),
        exclude_root_count = policy.excludes().len(),
        snapshot_node_count,
        watch_root_count = effective_root_count,
        net_enabled = config.net_enabled,
        "startup summary"
    );

    // Event sources (EventSource trait) publish onto the broadcast bus.
    let mut sources: Vec<Box<dyn EventSource>> = vec![
        Box::new(FsSource {
            mode: config.mode,
            policy: Arc::clone(&policy),
            roots: watch_roots,
            // Feed inotify events into the walker so the index stays
            // incrementally fresh.
            index_walker: Some(fs_walker),
        }),
        Box::new(ProcSource),
    ];
    if config.net_enabled {
        let net_cfg = NetConfig::from_args(
            config.net_poll_secs,
            &config.net_include,
            &config.net_exclude,
        );
        sources.push(Box::new(NetSource { config: net_cfg }));
    }
    if let Some(eve_file) = config.eve_file.clone() {
        sources.push(Box::new(SuricataEveSource {
            eve_file,
            poll_interval: Duration::from_secs(1),
        }));
    }
    if let Some(nebula_log) = config.nebula_log.clone() {
        sources.push(Box::new(NebulaSource {
            nebula_log,
            poll_interval: Duration::from_secs(1),
        }));
    }

    for source in sources {
        let name = source.name();
        let (tx, rx) = mpsc::channel::<Msg>(8192);
        if let Err(err) = source.start(node_id.clone(), tx) {
            tracing::warn!(source = name, error = %err, "source failed to start");
            continue;
        }
        let bus_tx = bus_tx.clone();
        tokio::spawn(async move {
            forward_to_bus(rx, bus_tx).await;
        });
    }

    server_handle.await?
}

async fn forward_to_bus(mut rx: mpsc::Receiver<Msg>, bus_tx: broadcast::Sender<Msg>) {
    while let Some(msg) = rx.recv().await {
        // ignore lagging viewers
        let _ = bus_tx.send(msg);
    }
}

/// Build the FS search index (spec §2). The builtin walker walks the scoped
/// roots and is kept incrementally fresh by the FS watcher's inotify events. The
/// agent never shells out to a system `locate` binary (O-7' no-exec). Returns the
/// index plus the walker handle the FS watcher uses to apply incremental updates.
fn build_fs_index(
    node_id: &str,
    policy: PathPolicy,
    mode: config::AgentMode,
    roots: &[std::path::PathBuf],
) -> (FsIndex, Arc<RwLock<Walker>>) {
    let walker = Arc::new(RwLock::new(Walker::new()));
    // Build in the background so a large initial walk does not block startup.
    let build_walker = Arc::clone(&walker);
    let build_roots = roots.to_vec();
    let build_policy = policy.clone();
    tokio::task::spawn_blocking(move || {
        let built = Walker::build(&build_roots, &build_policy, mode);
        let indexed = built.len();
        if let Ok(mut guard) = build_walker.write() {
            *guard = built;
        }
        tracing::info!(indexed, "FS index: builtin walker build complete");
    });

    let index = FsIndex::new(Arc::clone(&walker), policy, mode, node_id.to_string());
    (index, walker)
}
