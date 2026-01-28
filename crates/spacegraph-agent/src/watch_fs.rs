use anyhow::Result;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use spacegraph_core::{id_file, Delta, FileKind, Msg, Node};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc;

fn inode_for_path(path: &str) -> u64 {
    std::fs::metadata(path)
        .map(|m| {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                m.ino()
            }
            #[cfg(not(unix))]
            {
                0
            }
        })
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Upsert,
    Remove,
}

fn classify(kind: &EventKind) -> Option<Action> {
    // MVP mapping: Create/Modify => Upsert, Remove/Rename => Remove/Upsert depending on direction
    // notify 6 often delivers "Modify(Name(...))" for renames; we conservatively:
    // - any Remove => Remove
    // - any Create/Modify => Upsert
    match kind {
        EventKind::Create(_) => Some(Action::Upsert),
        EventKind::Modify(_) => Some(Action::Upsert),
        EventKind::Remove(_) => Some(Action::Remove),
        EventKind::Any => Some(Action::Upsert),
        _ => None,
    }
}

fn is_permission_denied(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::PermissionDenied
}

fn is_notify_permission_denied(error: &notify::Error) -> bool {
    match &error.kind {
        notify::ErrorKind::Io(io_err) => is_permission_denied(io_err),
        _ => false,
    }
}

fn add_watch_recursive(
    watcher: &mut RecommendedWatcher,
    root: &Path,
    skipped_paths_total: &mut usize,
) -> Result<()> {
    let mut stack = vec![root.to_path_buf()];

    while let Some(path) = stack.pop() {
        match watcher.watch(&path, RecursiveMode::NonRecursive) {
            Ok(()) => {}
            Err(err) if is_notify_permission_denied(&err) => {
                *skipped_paths_total += 1;
                tracing::debug!(path = %path.display(), "skipping path (permission denied)");
                continue;
            }
            Err(err) => return Err(err.into()),
        }

        let entries = match std::fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(err) if is_permission_denied(&err) => {
                *skipped_paths_total += 1;
                tracing::debug!(path = %path.display(), "skipping path (permission denied)");
                continue;
            }
            Err(err) => return Err(err.into()),
        };

        for entry_result in entries {
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(err) if is_permission_denied(&err) => {
                    *skipped_paths_total += 1;
                    tracing::debug!(path = %path.display(), "skipping entry (permission denied)");
                    continue;
                }
                Err(err) => return Err(err.into()),
            };

            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(err) if is_permission_denied(&err) => {
                    *skipped_paths_total += 1;
                    tracing::debug!(path = %entry.path().display(), "skipping entry (permission denied)");
                    continue;
                }
                Err(err) => return Err(err.into()),
            };

            if file_type.is_dir() {
                stack.push(entry.path());
            }
        }
    }

    Ok(())
}

pub fn spawn(node_id: &str, tx: mpsc::Sender<Msg>) -> Result<()> {
    let node_id = node_id.to_string();

    // Watch set (v0.1.2: still /etc by default; you can config later)
    let watch_paths = vec!["/etc"];

    // notify callback thread -> tokio channel
    let (raw_tx, mut raw_rx) = tokio::sync::mpsc::channel::<(String, Action)>(8192);

    let mut watcher: RecommendedWatcher = Watcher::new(
        move |res: std::result::Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                let action = match classify(&event.kind) {
                    Some(a) => a,
                    None => return,
                };
                for p in event.paths {
                    if let Some(s) = p.to_str() {
                        // ignore noisy temp files if needed later
                        let _ = raw_tx.try_send((s.to_string(), action));
                    }
                }
            }
        },
        notify::Config::default(),
    )?;

    let mut skipped_paths_total = 0usize;
    for p in &watch_paths {
        let path = PathBuf::from(p);
        if path.exists() {
            add_watch_recursive(&mut watcher, &path, &mut skipped_paths_total)?;
        }
    }
    if skipped_paths_total > 0 {
        tracing::warn!(
            skipped_paths_total,
            "FS watcher: skipped paths due to permissions"
        );
    }

    // Coalescer: 250ms window
    tokio::spawn(async move {
        let mut pending: HashMap<String, Action> = HashMap::new();
        let mut tick = tokio::time::interval(Duration::from_millis(250));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        let mut batch_id: u64 = 50_000;

        loop {
            tokio::select! {
                Some((path, action)) = raw_rx.recv() => {
                    // Coalesce rule:
                    // - Remove dominates Upsert (if something is removed, keep Remove)
                    // - otherwise latest Upsert wins
                    pending.entry(path).and_modify(|a| {
                        if *a != Action::Remove && action == Action::Remove {
                            *a = Action::Remove;
                        } else if action == Action::Upsert {
                            *a = Action::Upsert;
                        }
                    }).or_insert(action);
                }
                _ = tick.tick() => {
                    if pending.is_empty() {
                        continue;
                    }

                    let _ = tx.send(Msg::Event{ delta: Delta::BatchBegin{ id: batch_id }}).await;

                    for (path, action) in pending.drain() {
                        let id = id_file(&node_id, &path);
                        match action {
                            Action::Upsert => {
                                let node = Node::File {
                                    path: path.clone(),
                                    inode: inode_for_path(&path),
                                    kind: FileKind::Unknown,
                                };
                                let _ = tx.send(Msg::Event{ delta: Delta::UpsertNode{ id, node }}).await;
                            }
                            Action::Remove => {
                                let _ = tx.send(Msg::Event{ delta: Delta::RemoveNode{ id }}).await;
                            }
                        }
                    }

                    let _ = tx.send(Msg::Event{ delta: Delta::BatchEnd{ id: batch_id }}).await;
                    batch_id = batch_id.wrapping_add(1);
                }
            }
        }
    });

    // keep watcher alive
    std::mem::forget(watcher);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_permission_denied;
    use std::io;

    #[test]
    fn permission_denied_is_detected() {
        let err = io::Error::from(io::ErrorKind::PermissionDenied);
        assert!(is_permission_denied(&err));
    }

    #[test]
    fn non_permission_error_is_not_detected() {
        let err = io::Error::from(io::ErrorKind::NotFound);
        assert!(!is_permission_denied(&err));
    }
}
