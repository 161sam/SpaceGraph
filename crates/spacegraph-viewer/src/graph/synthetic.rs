//! Deterministic synthetic graph generation.
//!
//! Produces a reproducible graph of `n` nodes and roughly `2 * n` edges for
//! benchmarking ([`benches/layout.rs`]) and for the viewer's `--demo-load <n>`
//! smoke-test flag. Determinism is mandatory: the same `n` always yields the
//! exact same nodes and edges (seeded PRNG, no wall-clock input), so layout
//! trajectories and perf numbers are comparable across runs.

use spacegraph_core::{id_file, id_process, id_user, Edge, EdgeKind, FileKind, Node, NodeId};

/// Scope used for all synthetic node IDs.
pub const SYNTHETIC_SCOPE: &str = "synthetic";

/// SplitMix64 — a tiny, fast, fully deterministic PRNG (no external deps).
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform value in `0..n` (returns 0 when `n == 0`).
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
}

/// Build a deterministic graph of `n` nodes and ~`2 * n` edges.
///
/// Composition: ~1% users, ~40% processes, remainder files. Each process
/// `runs_as` a user, `execs` one file, and `opens` a few files — yielding a
/// connected, edge-dense graph that mirrors real agent output shape.
pub fn synthetic_graph(n: usize) -> (Vec<(NodeId, Node)>, Vec<Edge>) {
    if n == 0 {
        return (Vec::new(), Vec::new());
    }

    // ~10% users keeps `runs_as` hubs bounded (≈4 processes per user). A
    // bounded node degree is what makes the uniform-grid repulsion genuinely
    // O(N): with very few users (huge hubs) a hub's neighbourhood holds O(degree)
    // nodes within the cutoff, which no uniform grid can keep linear.
    let num_users = (n / 10).max(1);
    // Cap processes/users so files never go negative for tiny n.
    let num_procs = ((n * 2 / 5).max(1)).min(n.saturating_sub(num_users));
    let num_files = n - num_users - num_procs;

    let mut nodes: Vec<(NodeId, Node)> = Vec::with_capacity(n);

    // Users: uid 1000..
    let mut user_ids = Vec::with_capacity(num_users);
    for u in 0..num_users {
        let uid = 1000 + u as u32;
        let id = id_user(SYNTHETIC_SCOPE, uid);
        nodes.push((
            id.clone(),
            Node::User {
                uid,
                name: format!("user{u}"),
            },
        ));
        user_ids.push(id);
    }

    // Files: deterministic directory-bucketed paths.
    let mut file_ids = Vec::with_capacity(num_files);
    for f in 0..num_files {
        let path = format!("/synthetic/dir{:03}/file{:06}.dat", f / 64, f);
        let id = id_file(SYNTHETIC_SCOPE, &path);
        nodes.push((
            id.clone(),
            Node::File {
                path,
                inode: 1_000_000 + f as u64,
                kind: FileKind::Regular,
            },
        ));
        file_ids.push(id);
    }

    // Processes: pid 1000.. with deterministic parentage.
    let mut proc_ids = Vec::with_capacity(num_procs);
    for p in 0..num_procs {
        let pid = 1000 + p as i32;
        let ppid = if p == 0 { 1 } else { 1000 + (p as i32 - 1) / 2 };
        let id = id_process(SYNTHETIC_SCOPE, pid);
        let uid = 1000 + (p % num_users) as u32;
        nodes.push((
            id.clone(),
            Node::Process {
                pid,
                ppid,
                exe: format!("/synthetic/bin/proc{p}"),
                cmdline: format!("/synthetic/bin/proc{p} --id {p}"),
                uid,
            },
        ));
        proc_ids.push(id);
    }

    // Target ~2N edges. Each process emits: runs_as (1) + execs (1) + opens (k).
    let target_edges = 2 * n;
    let opens_per_proc = target_edges
        .saturating_sub(2 * num_procs)
        .checked_div(num_procs)
        .map_or(0, |k| k.max(1));

    let mut edges: Vec<Edge> = Vec::with_capacity(target_edges);
    for (p, proc_id) in proc_ids.iter().enumerate() {
        // Seed per-process so file selection is deterministic and stable.
        let mut rng = SplitMix64::new(0xC0FF_EE00_1234_5678 ^ (p as u64).wrapping_mul(0x100_0001));

        // runs_as → user (round-robin, already deterministic)
        if !user_ids.is_empty() {
            edges.push(Edge {
                from: proc_id.clone(),
                to: user_ids[p % user_ids.len()].clone(),
                kind: EdgeKind::RunsAs,
            });
        }

        if !file_ids.is_empty() {
            // execs → one file
            let exe_idx = rng.below(file_ids.len());
            edges.push(Edge {
                from: proc_id.clone(),
                to: file_ids[exe_idx].clone(),
                kind: EdgeKind::Execs,
            });

            // opens → k files
            for o in 0..opens_per_proc {
                let fidx = rng.below(file_ids.len());
                let mode = match o % 3 {
                    0 => "r",
                    1 => "w",
                    _ => "rw",
                };
                edges.push(Edge {
                    from: proc_id.clone(),
                    to: file_ids[fidx].clone(),
                    kind: EdgeKind::Opens {
                        fd: 3 + o as i32,
                        mode: mode.to_string(),
                    },
                });
            }
        }
    }

    (nodes, edges)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn produces_requested_node_count() {
        let (nodes, _) = synthetic_graph(1000);
        assert_eq!(nodes.len(), 1000);
    }

    #[test]
    fn edges_are_roughly_double_nodes() {
        let n = 2000;
        let (_, edges) = synthetic_graph(n);
        let unique: HashSet<&Edge> = edges.iter().collect();
        // ~2N before dedup; allow generous slack for collisions.
        assert!(
            unique.len() >= n,
            "expected >= {n} unique edges, got {}",
            unique.len()
        );
        assert!(edges.len() <= 3 * n);
    }

    #[test]
    fn generation_is_deterministic() {
        let (n1, e1) = synthetic_graph(500);
        let (n2, e2) = synthetic_graph(500);
        let ids1: Vec<&NodeId> = n1.iter().map(|(id, _)| id).collect();
        let ids2: Vec<&NodeId> = n2.iter().map(|(id, _)| id).collect();
        assert_eq!(ids1, ids2);
        assert_eq!(e1, e2);
    }

    #[test]
    fn every_edge_endpoint_is_a_node() {
        let (nodes, edges) = synthetic_graph(800);
        let ids: HashSet<&NodeId> = nodes.iter().map(|(id, _)| id).collect();
        for e in &edges {
            assert!(ids.contains(&e.from), "missing from endpoint");
            assert!(ids.contains(&e.to), "missing to endpoint");
        }
    }

    #[test]
    fn tiny_n_does_not_panic() {
        for n in 0..5 {
            let (nodes, _) = synthetic_graph(n);
            assert_eq!(nodes.len(), n);
        }
    }
}
