mod server;
mod snapshot;
mod watch_fs;
mod watch_proc;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Dev socket path
    let sock_path = "/tmp/spacegraph.sock";
    let _ = std::fs::remove_file(sock_path);

    // 1) Initial snapshot
    let snap = snapshot::build_snapshot()?;

    // 2) Start watchers producing deltas
    let (tx, rx) = tokio::sync::mpsc::channel(4096);
    watch_fs::spawn(tx.clone())?;
    watch_proc::spawn(tx.clone())?;

    // 3) Serve snapshot + stream
    server::run(sock_path, snap, rx).await
}
