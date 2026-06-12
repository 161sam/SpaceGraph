//! Event sources — the agent's collector extension point.
//!
//! Each source runs independently and emits `Msg` (typically
//! `Msg::Event { delta }`) on its channel. New collectors (eBPF, auditd, Zeek,
//! Falco, …) implement [`EventSource`]; the agent wires them uniformly in
//! `main`. The existing filesystem / process collectors are exposed as
//! [`FsSource`] / [`ProcSource`] wrappers over `watch_fs` / `watch_proc`.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use spacegraph_core::Msg;
use tokio::sync::mpsc;

use crate::config::AgentMode;
use crate::path_policy::PathPolicy;

pub mod net;
pub mod suricata_eve;

/// A source of graph events. `start` spawns the source's own task(s); it sends
/// events on `tx` until the process exits.
pub trait EventSource: Send + 'static {
    fn name(&self) -> &'static str;
    fn start(self: Box<Self>, node_id: String, tx: mpsc::Sender<Msg>) -> Result<()>;
}

/// Filesystem collector (fsnotify under the path policy).
pub struct FsSource {
    pub mode: AgentMode,
    pub policy: Arc<PathPolicy>,
    pub roots: Vec<PathBuf>,
}

impl EventSource for FsSource {
    fn name(&self) -> &'static str {
        "fs"
    }
    fn start(self: Box<Self>, node_id: String, tx: mpsc::Sender<Msg>) -> Result<()> {
        crate::watch_fs::spawn(&node_id, self.mode, self.policy, self.roots, tx)
    }
}

/// Process collector (procfs polling).
pub struct ProcSource;

impl EventSource for ProcSource {
    fn name(&self) -> &'static str {
        "proc"
    }
    fn start(self: Box<Self>, node_id: String, tx: mpsc::Sender<Msg>) -> Result<()> {
        crate::watch_proc::spawn(&node_id, tx)
    }
}
