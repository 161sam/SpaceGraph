use anyhow::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher, EventKind};
use spacegraph_core::{id_file, Delta, FileKind, Msg, Node};
use std::path::Path;
use tokio::sync::mpsc;

fn inode_for_path(path: &str) -> u64 {
    std::fs::metadata(path).map(|m| {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            m.ino()
        }
        #[cfg(not(unix))]
        {
            0
        }
    }).unwrap_or(0)
}

pub fn spawn(node_id: &str, tx: mpsc::Sender<Msg>) -> Result<()> {
    let node_id = node_id.to_string();

    // What we watch in v0.1: /etc (you can add config later)
    let paths = vec!["/etc"];

    // notify callback runs in its own thread; forward into tokio
    let (evt_tx, mut evt_rx) = tokio::sync::mpsc::channel::<String>(4096);

    let mut watcher: RecommendedWatcher = Watcher::new(
        move |res| {
            if let Ok(event) = res {
                // only meaningful file path events
                if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) | EventKind::Any) {
                    for p in event.paths {
                        if let Some(s) = p.to_str() {
                            let _ = evt_tx.try_send(s.to_string());
                        }
                    }
                }
            }
        },
        notify::Config::default(),
    )?;

    for p in &paths {
        if Path::new(p).exists() {
            watcher.watch(Path::new(p), RecursiveMode::Recursive)?;
        }
    }

    tokio::spawn(async move {
        let mut batch_id: u64 = 10_000;
        while let Some(path) = evt_rx.recv().await {
            // Upsert file node on any change (MVP)
            let id = id_file(&node_id, &path);
            let node = Node::File {
                path: path.clone(),
                inode: inode_for_path(&path),
                kind: FileKind::Unknown,
            };

            // Batch helps UI coalesce
            let _ = tx.send(Msg::Event { delta: Delta::BatchBegin { id: batch_id } }).await;
            let _ = tx.send(Msg::Event { delta: Delta::UpsertNode { id, node } }).await;
            let _ = tx.send(Msg::Event { delta: Delta::BatchEnd { id: batch_id } }).await;
            batch_id = batch_id.wrapping_add(1);
        }
    });

    // keep watcher alive
    std::mem::forget(watcher);
    Ok(())
}
