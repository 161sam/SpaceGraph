//! Builtin filesystem walker (spec §2, D-1 fallback). When no system locate is
//! present, the agent walks the scoped roots into a cached path list, kept fresh
//! by incremental inotify updates (`on_upsert`/`on_remove`) plus a periodic
//! rebuild. Scope + privilege are applied **at build time**: the walker only
//! descends allowed roots and only records readable entries unless privileged
//! (spec §5).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::config::AgentMode;
use crate::index::is_readable;
use crate::path_policy::PathPolicy;

/// A cached, queryable path list built from the scoped roots.
#[derive(Debug, Default)]
pub struct Walker {
    paths: BTreeSet<String>,
}

impl Walker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether `path` may be recorded under `policy`/`mode` (the build-time
    /// scope+privilege filter). In `User` mode an unreadable entry is dropped.
    fn admits(path: &Path, policy: &PathPolicy, mode: AgentMode) -> bool {
        if !policy.should_watch(path) {
            return false;
        }
        match mode {
            AgentMode::Privileged => true,
            AgentMode::User => is_readable(path),
        }
    }

    /// Build by walking `roots` under `policy`/`mode`. Records every admitted
    /// entry (files and directories) and descends admitted directories only.
    pub fn build(roots: &[PathBuf], policy: &PathPolicy, mode: AgentMode) -> Self {
        let mut walker = Self::new();
        let mut stack: Vec<PathBuf> = roots.to_vec();
        while let Some(path) = stack.pop() {
            if !Self::admits(&path, policy, mode) {
                continue;
            }
            if let Some(s) = path.to_str() {
                walker.paths.insert(s.to_string());
            }
            let meta = match std::fs::symlink_metadata(&path) {
                Ok(meta) => meta,
                Err(_) => continue,
            };
            if meta.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&path) {
                    for entry in entries.flatten() {
                        stack.push(entry.path());
                    }
                }
            }
        }
        walker
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /// Case-insensitive substring query. Returns matching paths (unranked — the
    /// caller ranks via `rank`).
    pub fn query(&self, pattern: &str) -> Vec<String> {
        let needle = pattern.to_lowercase();
        self.paths
            .iter()
            .filter(|path| path.to_lowercase().contains(&needle))
            .cloned()
            .collect()
    }

    /// Incremental add/update from an inotify event. Recorded only if it still
    /// passes the build-time filter (so a now-excluded/unreadable path is not
    /// silently indexed).
    pub fn on_upsert(&mut self, path: &str, policy: &PathPolicy, mode: AgentMode) {
        if Self::admits(Path::new(path), policy, mode) {
            self.paths.insert(path.to_string());
        }
    }

    /// Incremental removal from an inotify event.
    pub fn on_remove(&mut self, path: &str) {
        self.paths.remove(path);
    }

    /// Test-only constructor: a walker holding exactly `paths`, bypassing the
    /// build-time scope/privilege filter so the search-time post-filter can be
    /// exercised over a synthetic path universe.
    #[cfg(test)]
    pub(crate) fn from_paths(paths: impl IntoIterator<Item = String>) -> Self {
        Self {
            paths: paths.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_root(name: &str) -> PathBuf {
        let mut root = std::env::temp_dir();
        root.push(format!("spacegraph-walker-{name}-{}", std::process::id()));
        root
    }

    #[test]
    fn build_then_query_hits_and_misses() {
        let root = temp_root("build");
        let _ = fs::remove_dir_all(&root);
        let sub = root.join("docs");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("report.txt"), b"x").unwrap();
        fs::write(root.join("notes.md"), b"y").unwrap();

        let mut policy = PathPolicy::new(vec![root.clone()], Vec::new());
        policy.normalize();
        let walker = Walker::build(std::slice::from_ref(&root), &policy, AgentMode::User);

        assert!(
            !walker.query("report").is_empty(),
            "an indexed file is found"
        );
        assert!(walker.query("does-not-exist").is_empty(), "a miss is empty");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn excluded_subdir_is_not_recorded_at_build() {
        let root = temp_root("excluded");
        let _ = fs::remove_dir_all(&root);
        let blocked = root.join("node_modules");
        fs::create_dir_all(&blocked).unwrap();
        fs::write(blocked.join("report.js"), b"x").unwrap();

        let mut policy = PathPolicy::new(vec![root.clone()], vec![PathBuf::from("node_modules")]);
        policy.normalize();
        let walker = Walker::build(std::slice::from_ref(&root), &policy, AgentMode::User);

        assert!(
            walker.query("report").is_empty(),
            "an excluded subdir must not be indexed"
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn incremental_upsert_and_remove() {
        let root = temp_root("incremental");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let file = root.join("fresh.log");
        fs::write(&file, b"z").unwrap();

        let mut policy = PathPolicy::new(vec![root.clone()], Vec::new());
        policy.normalize();
        let mut walker = Walker::build(std::slice::from_ref(&root), &policy, AgentMode::User);

        // A new file appears (inotify Create) → query finds it.
        let added = root.join("appeared.log");
        fs::write(&added, b"z").unwrap();
        let added_str = added.to_str().unwrap();
        assert!(walker.query("appeared").is_empty(), "not indexed yet");
        walker.on_upsert(added_str, &policy, AgentMode::User);
        assert!(
            !walker.query("appeared").is_empty(),
            "inotify upsert indexes the new path"
        );

        // The original file is removed (inotify Remove) → query misses it.
        walker.on_remove(file.to_str().unwrap());
        assert!(
            walker.query("fresh").is_empty(),
            "inotify remove drops the path"
        );

        let _ = fs::remove_dir_all(&root);
    }
}
