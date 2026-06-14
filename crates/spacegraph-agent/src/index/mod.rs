//! Filesystem search index (spec §2/§4/§5). **The index is not the graph:** it
//! is the searchable universe of paths; only a *picked* result materialises into
//! a node. The index is the builtin walker: it walks the scoped roots into a
//! cached path list, kept fresh by incremental inotify updates. The agent never
//! shells out to a system `locate` binary — its read-only / no-exec guarantee
//! (O-7') is preserved.
//!
//! Scope / privilege / path-policy are applied **at build time** (the walker only
//! records allowed entries) and re-checked as a **post-filter** in
//! [`FsIndex::search`]. Either way, in `User` mode an excluded or unreadable path
//! is never returned (§5) — [`path_allowed`] is the single chokepoint, asserted
//! by the security test.

mod rank;
mod walker;

pub use rank::Candidate;
pub use walker::Walker;

use std::path::Path;
use std::sync::{Arc, RwLock};

use spacegraph_core::{
    id_file, Delta, FileKind, MaterialiseRequest, Node, SearchRequest, SearchResponse,
};

use crate::config::AgentMode;
use crate::path_policy::PathPolicy;

/// Hard ceiling on results regardless of the request limit — bounds the work and
/// the wire payload.
const MAX_RESULTS: usize = 1000;

/// The agent-side filesystem index, backed by the builtin walker (scope and
/// privilege applied at build time; re-checked on search).
pub struct FsIndex {
    walker: Arc<RwLock<Walker>>,
    policy: PathPolicy,
    mode: AgentMode,
    node_id: String,
}

impl FsIndex {
    pub fn new(
        walker: Arc<RwLock<Walker>>,
        policy: PathPolicy,
        mode: AgentMode,
        node_id: String,
    ) -> Self {
        Self {
            walker,
            policy,
            mode,
            node_id,
        }
    }

    /// Query the index for `req`, returning ranked, policy-filtered hits.
    pub fn search(&self, req: &SearchRequest) -> SearchResponse {
        let limit = (req.limit as usize).clamp(1, MAX_RESULTS);
        let raw = self.raw_candidates(&req.query);
        let candidates: Vec<Candidate> = raw
            .into_iter()
            .filter(|path| {
                path_allowed(
                    Path::new(path),
                    &self.policy,
                    self.mode,
                    req.full_system,
                    &is_readable,
                )
            })
            .map(|path| candidate_for(&path))
            .collect();
        let (results, truncated) = rank::rank_hits(&req.query, candidates, limit);
        SearchResponse { results, truncated }
    }

    /// Materialise a *picked* path into node delta(s). Returns empty when the
    /// path is not permitted (excluded / unreadable in `User` mode) — only
    /// picked, permitted results materialise, and the result is bounded (a
    /// single `File` node). Scope (the root-set) is not re-enforced here: the
    /// user picked this path from results the agent already produced, but the
    /// security invariants (excludes + readability) still apply.
    pub fn materialise(&self, req: &MaterialiseRequest) -> Vec<Delta> {
        let path = Path::new(&req.path);
        if !path_allowed(path, &self.policy, self.mode, true, &is_readable) {
            return Vec::new();
        }
        let candidate = candidate_for(&req.path);
        vec![Delta::UpsertNode {
            id: id_file(&self.node_id, &req.path),
            node: Node::File {
                path: req.path.clone(),
                inode: inode_of(path),
                kind: candidate.kind,
            },
        }]
    }

    fn raw_candidates(&self, query: &str) -> Vec<String> {
        self.walker
            .read()
            .map(|walker| walker.query(query))
            .unwrap_or_default()
    }
}

/// Whether a path may be returned to the viewer. The single security chokepoint
/// (spec §5):
/// - an **excluded** path is never returned (even privileged);
/// - **scope**: root-set by default; `full_system` widens it;
/// - **privilege**: `User` returns only readable paths; `Privileged` may surface
///   unreadable ones (the audited full-system surface).
///
/// `readable` is injected so the policy is testable independent of the test
/// runner's euid.
pub fn path_allowed(
    path: &Path,
    policy: &PathPolicy,
    mode: AgentMode,
    full_system: bool,
    readable: &dyn Fn(&Path) -> bool,
) -> bool {
    if policy.is_excluded(path) {
        return false;
    }
    let scope_ok = full_system || policy.includes().is_empty() || policy.is_included(path);
    if !scope_ok {
        return false;
    }
    match mode {
        AgentMode::Privileged => true,
        AgentMode::User => readable(path),
    }
}

/// Whether the agent user can read `path` (real uid/gid via `access`).
pub(crate) fn is_readable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        match CString::new(path.as_os_str().as_bytes()) {
            Ok(c) => unsafe { libc::access(c.as_ptr(), libc::R_OK) == 0 },
            Err(_) => false,
        }
    }
    #[cfg(not(unix))]
    {
        path.exists()
    }
}

fn candidate_for(path: &str) -> Candidate {
    let p = Path::new(path);
    let (kind, size, mtime) = match std::fs::symlink_metadata(p) {
        Ok(meta) => (file_kind(&meta, path), Some(meta.len()), mtime_secs(&meta)),
        Err(_) => (file_kind_from_path(path), None, None),
    };
    Candidate {
        path: path.to_string(),
        kind,
        size,
        mtime,
        readable: is_readable(p),
    }
}

fn file_kind(meta: &std::fs::Metadata, path: &str) -> FileKind {
    let ft = meta.file_type();
    if ft.is_dir() {
        FileKind::Dir
    } else if ft.is_file() {
        FileKind::Regular
    } else {
        // Symlink / fifo / socket / device — fall back to the path heuristic.
        file_kind_from_path(path)
    }
}

fn file_kind_from_path(path: &str) -> FileKind {
    if path.starts_with("/dev/") {
        FileKind::Device
    } else {
        FileKind::Unknown
    }
}

fn mtime_secs(meta: &std::fs::Metadata) -> Option<i64> {
    let modified = meta.modified().ok()?;
    let dur = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(dur.as_secs() as i64)
}

fn inode_of(path: &Path) -> u64 {
    std::fs::symlink_metadata(path)
        .map(|meta| {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                meta.ino()
            }
            #[cfg(not(unix))]
            {
                let _ = meta;
                0
            }
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;

    /// Build an `FsIndex` whose walker holds exactly `paths` (bypassing the
    /// build-time walk) so the search-time scope/exclude post-filter and ranking
    /// are exercised directly over a synthetic universe.
    fn index_over(paths: Vec<&str>, policy: PathPolicy, mode: AgentMode) -> FsIndex {
        let walker = Walker::from_paths(paths.into_iter().map(String::from));
        FsIndex::new(
            Arc::new(RwLock::new(walker)),
            policy,
            mode,
            "host".to_string(),
        )
    }

    fn policy_over(root: &Path) -> PathPolicy {
        let mut policy = PathPolicy::new(vec![root.to_path_buf()], vec![PathBuf::from("secret")]);
        policy.normalize();
        policy
    }

    // ---- Security: User mode never returns an excluded/unreadable path ----

    #[test]
    fn user_mode_drops_excluded_and_unreadable() {
        let root = PathBuf::from("/home/u/projects");
        let policy = {
            let mut p = PathPolicy::new(vec![root.clone()], vec![PathBuf::from("secret")]);
            p.normalize();
            p
        };

        let readable = root.join("readable.txt");
        let unreadable = root.join("unreadable.txt");
        let excluded = root.join("secret/keys.txt");
        let out_of_scope = PathBuf::from("/etc/shadow");

        // Inject readability: only `readable` is readable.
        let is_readable = |p: &Path| p == readable.as_path();

        // User mode: excluded dropped, unreadable dropped, out-of-scope dropped,
        // a readable in-scope path kept.
        assert!(path_allowed(
            &readable,
            &policy,
            AgentMode::User,
            false,
            &is_readable
        ));
        assert!(
            !path_allowed(&excluded, &policy, AgentMode::User, false, &is_readable),
            "excluded path is never returned"
        );
        assert!(
            !path_allowed(&unreadable, &policy, AgentMode::User, false, &is_readable),
            "unreadable path is never returned in User mode"
        );
        assert!(
            !path_allowed(&out_of_scope, &policy, AgentMode::User, false, &is_readable),
            "out-of-root-set path dropped without full_system"
        );
    }

    #[test]
    fn privileged_surfaces_unreadable_but_excludes_still_win() {
        let root = PathBuf::from("/home/u/projects");
        let policy = policy_over(&root);
        let unreadable = root.join("unreadable.txt");
        let excluded = root.join("secret/keys.txt");
        let none_readable = |_: &Path| false;

        // Privileged surfaces an unreadable in-scope path...
        assert!(path_allowed(
            &unreadable,
            &policy,
            AgentMode::Privileged,
            false,
            &none_readable
        ));
        // ...but an excluded path is dropped even when privileged.
        assert!(
            !path_allowed(
                &excluded,
                &policy,
                AgentMode::Privileged,
                false,
                &none_readable
            ),
            "excludes win over privilege"
        );
    }

    #[test]
    fn full_system_widens_scope_but_user_still_needs_readability() {
        let root = PathBuf::from("/home/u");
        let policy = policy_over(&root);
        let out = PathBuf::from("/opt/app/readme");
        let readable = |_: &Path| true;
        let unreadable = |_: &Path| false;

        // Without full_system: out-of-scope dropped.
        assert!(!path_allowed(
            &out,
            &policy,
            AgentMode::User,
            false,
            &readable
        ));
        // With full_system: in scope, kept iff readable (User).
        assert!(path_allowed(
            &out,
            &policy,
            AgentMode::User,
            true,
            &readable
        ));
        assert!(
            !path_allowed(&out, &policy, AgentMode::User, true, &unreadable),
            "full_system beyond the user's readable set requires Privileged"
        );
    }

    // ---- search() end-to-end over the walker index ----

    #[test]
    fn search_ranks_filters_and_caps() {
        // The walker holds the raw universe; search's post-filter drops the
        // excluded / out-of-scope paths.
        let mut policy = PathPolicy::new(
            vec![PathBuf::from("/home/u")],
            vec![PathBuf::from("secret")],
        );
        policy.normalize();
        let index = index_over(
            vec![
                "/home/u/report.txt",
                "/home/u/reporting/report2.txt",
                "/home/u/secret/report_leak.txt", // excluded
                "/etc/passwd",                    // out of scope
            ],
            policy,
            // Privileged so the (non-existent) synthetic paths aren't dropped for
            // unreadability — this test isolates scope/exclude + ranking.
            AgentMode::Privileged,
        );

        let resp = index.search(&SearchRequest {
            query: "report".to_string(),
            limit: 10,
            full_system: false,
        });
        let paths: HashSet<&str> = resp.results.iter().map(|h| h.path.as_str()).collect();
        assert!(paths.contains("/home/u/report.txt"));
        assert!(paths.contains("/home/u/reporting/report2.txt"));
        assert!(
            !paths.contains("/home/u/secret/report_leak.txt"),
            "excluded path filtered out"
        );
        assert!(
            !paths.contains("/etc/passwd"),
            "out-of-scope path filtered out"
        );
        // Exact filename "report.txt" — "report" is a prefix of report.txt and
        // report2.txt; the shallower/shorter name ranks first.
        assert_eq!(resp.results[0].path, "/home/u/report.txt");
    }

    #[test]
    fn search_sets_truncated_when_over_cap() {
        let mut policy = PathPolicy::new(vec![PathBuf::from("/home/u")], Vec::new());
        policy.normalize();
        let paths: Vec<String> = (0..5).map(|i| format!("/home/u/report{i}")).collect();
        let index = FsIndex::new(
            Arc::new(RwLock::new(Walker::from_paths(paths))),
            policy,
            AgentMode::Privileged,
            "host".into(),
        );
        let resp = index.search(&SearchRequest {
            query: "report".into(),
            limit: 2,
            full_system: false,
        });
        assert_eq!(resp.results.len(), 2);
        assert!(resp.truncated);
    }

    // ---- materialise() ----

    #[test]
    fn materialise_emits_file_node_for_real_path() {
        // A real, readable path inside scope materialises into one File node.
        let dir = std::env::temp_dir().join(format!("sg-mat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("picked.txt");
        std::fs::write(&file, b"hi").unwrap();

        let mut policy = PathPolicy::new(vec![dir.clone()], Vec::new());
        policy.normalize();
        let index = FsIndex::new(
            Arc::new(RwLock::new(Walker::new())),
            policy,
            AgentMode::User,
            "host".into(),
        );

        let deltas = index.materialise(&MaterialiseRequest {
            path: file.to_string_lossy().to_string(),
        });
        assert_eq!(deltas.len(), 1, "exactly one node materialises (bounded)");
        match &deltas[0] {
            Delta::UpsertNode {
                node: Node::File { path, kind, .. },
                ..
            } => {
                assert_eq!(path, &file.to_string_lossy());
                assert_eq!(*kind, FileKind::Regular);
            }
            other => panic!("expected a File UpsertNode, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn materialise_refuses_excluded_path() {
        let mut policy =
            PathPolicy::new(vec![PathBuf::from("/home/u")], vec![PathBuf::from("/proc")]);
        policy.normalize();
        let index = FsIndex::new(
            Arc::new(RwLock::new(Walker::new())),
            policy,
            AgentMode::Privileged,
            "host".into(),
        );
        let deltas = index.materialise(&MaterialiseRequest {
            path: "/proc/1/maps".to_string(),
        });
        assert!(deltas.is_empty(), "an excluded path never materialises");
    }
}
