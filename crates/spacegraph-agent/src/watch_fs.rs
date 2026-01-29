use anyhow::Result;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use spacegraph_core::{id_file, Delta, FileKind, Msg, Node};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::config::AgentMode;
use crate::path_policy::PathPolicy;
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

fn is_watch_limit_error(error: &io::Error) -> bool {
    #[cfg(unix)]
    {
        error.raw_os_error() == Some(libc::ENOSPC)
    }
    #[cfg(not(unix))]
    {
        let _ = error;
        false
    }
}

fn is_notify_permission_denied(error: &notify::Error) -> bool {
    match &error.kind {
        notify::ErrorKind::Io(io_err) => is_permission_denied(io_err),
        _ => false,
    }
}

fn is_notify_watch_limit(error: &notify::Error) -> bool {
    match &error.kind {
        notify::ErrorKind::Io(io_err) => is_watch_limit_error(io_err),
        _ => error.to_string().to_lowercase().contains("watch limit"),
    }
}

fn log_permission_denied(mode: AgentMode, path: &Path, context: &str) {
    match mode {
        AgentMode::User => {
            tracing::debug!(path = %path.display(), "{context} (permission denied)");
        }
        AgentMode::Privileged => {
            tracing::warn!(
                path = %path.display(),
                "{context} (permission denied; run with sudo or adjust permissions)"
            );
        }
    }
}

fn log_watch_limit(path: &Path, context: &str) {
    tracing::warn!(
        path = %path.display(),
        "{context} (watch limit reached; increase fs.inotify.max_user_watches)"
    );
}

#[derive(Default)]
struct WatchStats {
    watched: usize,
    skipped_permission: usize,
    skipped_watch_limit: usize,
}

fn add_watch_recursive(
    watcher: &mut RecommendedWatcher,
    root: &Path,
    policy: &PathPolicy,
    mode: AgentMode,
    stats: &mut WatchStats,
) -> Result<()> {
    let mut stack = vec![root.to_path_buf()];

    while let Some(path) = stack.pop() {
        if !policy.should_watch(&path) {
            continue;
        }

        match watcher.watch(&path, RecursiveMode::NonRecursive) {
            Ok(()) => {
                stats.watched += 1;
            }
            Err(err) if is_notify_permission_denied(&err) => {
                stats.skipped_permission += 1;
                log_permission_denied(mode, &path, "skipping path");
                continue;
            }
            Err(err) if is_notify_watch_limit(&err) => {
                stats.skipped_watch_limit += 1;
                log_watch_limit(&path, "skipping path");
                continue;
            }
            Err(err) => return Err(err.into()),
        }

        let entries = match std::fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(err) if is_permission_denied(&err) => {
                stats.skipped_permission += 1;
                log_permission_denied(mode, &path, "skipping path");
                continue;
            }
            Err(err) => return Err(err.into()),
        };

        for entry_result in entries {
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(err) if is_permission_denied(&err) => {
                    stats.skipped_permission += 1;
                    log_permission_denied(mode, &path, "skipping entry");
                    continue;
                }
                Err(err) => return Err(err.into()),
            };

            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(err) if is_permission_denied(&err) => {
                    stats.skipped_permission += 1;
                    log_permission_denied(mode, &entry.path(), "skipping entry");
                    continue;
                }
                Err(err) => return Err(err.into()),
            };

            if file_type.is_dir() {
                let entry_path = entry.path();
                if policy.should_watch(&entry_path) {
                    stack.push(entry_path);
                }
            }
        }
    }

    Ok(())
}

pub fn spawn(
    node_id: &str,
    mode: AgentMode,
    policy: Arc<PathPolicy>,
    roots: Vec<PathBuf>,
    tx: mpsc::Sender<Msg>,
) -> Result<()> {
    let node_id = node_id.to_string();

    // notify callback thread -> tokio channel
    let (raw_tx, mut raw_rx) = tokio::sync::mpsc::channel::<(String, Action)>(8192);

    let policy_for_events = Arc::clone(&policy);
    let mut watcher: RecommendedWatcher = Watcher::new(
        move |res: std::result::Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                let action = match classify(&event.kind) {
                    Some(a) => a,
                    None => return,
                };
                for p in event.paths {
                    if !policy_for_events.should_watch(&p) {
                        continue;
                    }
                    if let Some(s) = p.to_str() {
                        // ignore noisy temp files if needed later
                        let _ = raw_tx.try_send((s.to_string(), action));
                    }
                }
            }
        },
        notify::Config::default(),
    )?;

    let mut stats = WatchStats::default();
    for path in roots {
        if path.exists() {
            add_watch_recursive(&mut watcher, &path, &policy, mode, &mut stats)?;
        }
    }
    tracing::info!(
        watched_count = stats.watched,
        skipped_permission = stats.skipped_permission,
        skipped_watch_limit = stats.skipped_watch_limit,
        "FS watcher: initial watch summary"
    );

    // Coalescer: 250ms window
    tokio::spawn(async move {
        let mut pending: HashMap<String, Action> = HashMap::new();
        let mut tick = tokio::time::interval(Duration::from_millis(250));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        let mut batch_id: u64 = 50_000;
        let mut last_log = Instant::now() - Duration::from_secs(1);

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

                    let total = pending.len();
                    let mut upserts = 0usize;
                    let mut removes = 0usize;
                    for action in pending.values() {
                        match action {
                            Action::Upsert => upserts += 1,
                            Action::Remove => removes += 1,
                        }
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
                    if last_log.elapsed() >= Duration::from_secs(1) {
                        tracing::debug!(
                            event_type = "fs",
                            batch_id,
                            total,
                            upserts,
                            removes,
                            "broadcast batch"
                        );
                        last_log = Instant::now();
                    }
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
    use super::{is_permission_denied, is_watch_limit_error};
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

    #[test]
    #[cfg(unix)]
    fn watch_limit_error_is_detected() {
        let err = io::Error::from_raw_os_error(libc::ENOSPC);
        assert!(is_watch_limit_error(&err));
    }

    #[test]
    fn non_watch_limit_error_is_not_detected() {
        let err = io::Error::from(io::ErrorKind::NotFound);
        assert!(!is_watch_limit_error(&err));
    }
}
