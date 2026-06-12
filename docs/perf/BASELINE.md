# SpaceGraph — Performance Baseline (Phase 0)

Branch: `chore/v0.1.x-baseline`. This document records the status quo **before**
the Phase 1–3 performance work, the bench harness, and the diagnosis + fix of
the edge-visibility bug.

## Dev machine

| | |
|---|---|
| CPU | Intel(R) Core(TM) i5-6300U @ 2.40GHz (2c/4t) |
| RAM | 16 GB |
| OS | Linux 6.8 (lowlatency) |
| rustc | 1.95.0 |
| Profile | `bench` (opt-level 3, inherits release) |

All `force_step` / `visible_set_capped` numbers below are criterion medians on
this machine. Re-run with `cargo bench -p spacegraph-viewer`.

## Synthetic graph + bench harness

* `crates/spacegraph-viewer/src/graph/synthetic.rs` — deterministic generator
  (`synthetic_graph(n)`), seeded SplitMix64, no new deps. `n` nodes (~1% users,
  ~40% processes, rest files) and ~`2n` edges (`runs_as` + `execs` + `opens`).
* `GraphState::load_synthetic_graph(n)` loads it as a snapshot.
* `--demo-load <n>` viewer flag seeds it instead of connecting to an agent.
* `crates/spacegraph-viewer/benches/layout.rs` — criterion benches for
  `force_step` and `visible_set_capped` at 500 / 1000 / 2000 / 5000 nodes.

## Baseline numbers (force_step, O(N²) all-pairs repulsion)

`force_step` per step (criterion median; `[low mid high]` in the source run):

| nodes | time | criterion interval |
|------:|-----:|:-------------------|
| 500   | **33.7 ms**  | [31.9 ms · 33.7 ms · 35.3 ms] |
| 1000  | **146 ms**   | [129 ms · 146 ms · 168 ms] |
| 2000  | **720 ms**   | [699 ms · 720 ms · 740 ms] |
| 5000  | **4.29 s**   | [4.23 s · 4.29 s · 4.36 s] |

`visible_set_capped` per call (median, cap = 0.75·n):

| nodes | time | criterion interval |
|------:|-----:|:-------------------|
| 500   | **0.85 ms** | [759 µs · 850 µs · 945 µs] |
| 1000  | **2.02 ms** | [1.93 ms · 2.02 ms · 2.11 ms] |
| 2000  | **4.33 ms** | [4.17 ms · 4.33 ms · 4.61 ms] |
| 5000  | **14.3 ms** | [13.7 ms · 14.3 ms · 14.7 ms] |

The `force_step` curve is unmistakably O(N²): 2× the nodes ≈ 4–5× the time
(146 ms → 720 ms → 4.29 s for 1000 → 2000 → 5000). At 2000 nodes a single
layout step already costs **720 ms** — ~1.4 FPS from layout alone. The Phase 2
gate (`< 4 ms` at 2000, `< 12 ms` at 5000) requires a ~180× / ~360×
improvement, delivered by index interning (Phase 1) + uniform-grid repulsion
(Phase 2).

Observations:

* `force_step` is O(N²): an all-pairs repulsion loop over the visible set with
  `HashMap<NodeId, Vec3>` lookups (`NodeId` = `String`) and per-pair `NodeId`
  clones for the force map. This is the dominant frame cost and the reason FPS
  collapses at ~1200 visible nodes (AGENTS.md §1.4 violation). Phase 1
  (index interning) and Phase 2 (grid repulsion) target this.
* `render::spatial::draw_spatial` despawns and respawns **all** node entities
  whenever `needs_redraw` is set, and `force_step` sets `needs_redraw` every
  frame → full entity churn per frame. Phase 3 targets this.

## Edge-visibility bug — diagnosis

**Symptom:** HUD reports thousands of aggregated edges (e.g. `agg 2233`) while
the spatial view shows `0 / 0` visible edges on a fresh, default config.

**Root cause (confirmed): the visible-set cap destroys connectivity.**

In `graph/layout.rs::visible_set_capped`, when the graph has more nodes than
`max_visible_nodes` (default 1200), the previous code reduced the set by a
plain **lexicographic truncation**:

```rust
let mut v: Vec<NodeId> = base.into_iter().collect();
v.sort_by(|a, b| a.0.cmp(&b.0));
v.truncate(self.cfg.max_visible_nodes);
```

Node IDs are typed string keys built by `spacegraph-core`:

```
<scope>:file:<path>
<scope>:process:pid:<pid>
<scope>:user:<uid>
```

These sort by **type prefix**: `file` (`f`) < `process` (`p`) < `user` (`u`).
So for any graph with more than `max_visible_nodes` file nodes, the first 1200
sorted IDs are **all `file` nodes** — zero processes, zero users. Every edge in
the model is `process → file` (opens/execs) or `process → user` (runs_as), so
**no edge has both endpoints in the visible set** → `visible_edge_counts`
returns `(0, 0)` while `agg_edge_count` is unchanged. Hence `agg N` but `0/0`
visible.

This is *not* caused by the LOD default: with `max_visible_nodes = 1200 <
lod_threshold_nodes = 1500`, `lod_active` is always false on default config, so
edges already route through the `LodEdgesMode::All` branch. The cap, not the
LOD mode, is the culprit. The fix is local to viewer logic — **no protocol
change required** (no hard stop).

**Fix:** `visible_set_capped` now caps via a deterministic, connectivity-aware
BFS (`cap_visible_set_connected`) for the spatial view: seeds in sorted ID
order, grows the set by pulling in graph neighbours (sorted) until the cap is
reached, so connected nodes are co-selected and edges keep both endpoints. Tree
view keeps the lexicographic slice (file paths sort hierarchically, so subtrees
stay contiguous). Determinism is preserved (sorted seed + neighbour order).

**Regression tests** (`graph/layout.rs`):

* `capped_visible_set_preserves_edges` — 3000-node synthetic, cap 1200: asserts
  many visible aggregated edges (≥100). On the old lexicographic cap this is
  exactly 0 (all 1200 slots are file nodes).
* `capped_visible_set_is_deterministic` — two fresh states yield identical caps.
* `uncapped_visible_set_returns_all_nodes` — below-cap graphs are unaffected.

## Gate 0 status

- [x] Workspace builds; `cargo test --workspace` green.
- [x] Benches build and run (`cargo bench -p spacegraph-viewer`).
- [x] Root cause of edge bug documented and fixed (viewer-local).
- [x] Baseline numbers recorded (above).
