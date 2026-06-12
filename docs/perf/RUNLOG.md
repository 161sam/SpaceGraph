# SpaceGraph — Master-Prompt Run Log

One section per phase: what changed, gate results with numbers, and any
deviations from the master prompt (each with a one-line justification).

---

## Phase 0 — Baseline, diagnostics, bench harness

Branch: `chore/v0.1.x-baseline`.

### Changed

* Added a library target (`src/lib.rs`) exposing the viewer modules so
  benches/tests can construct `GraphState` without a running Bevy app; `main.rs`
  is now a thin boot wrapper that also parses `--demo-load <n>`.
* `graph/synthetic.rs` — deterministic synthetic-graph generator (seeded
  SplitMix64, zero new runtime deps). `n` nodes / ~`2n` edges.
* `GraphState::load_synthetic_graph(n)` + `--demo-load <n>` flag (seeds the
  synthetic graph instead of auto-connecting agents).
* `benches/layout.rs` — criterion benches for `force_step` and
  `visible_set_capped` at 500/1000/2000/5000.
* **Edge-visibility bug fixed** in `graph/layout.rs`: connectivity-aware
  deterministic BFS cap (`cap_visible_set_connected`) replaces the lexicographic
  truncation that dropped every process node. See `BASELINE.md` for the full
  root-cause analysis.

### Gate 0 results

* `cargo test --workspace`: green (38 viewer tests incl. 3 new edge-bug
  regression tests + synthetic generator tests; agent/core suites green).
* `cargo fmt --check`: clean.
* `cargo clippy --workspace -- -D warnings`: clean.
* `cargo bench -p spacegraph-viewer`: runs; baseline numbers in `BASELINE.md`.
* Edge-bug root cause documented and fixed (viewer-local, no protocol change).

### Deviations

* Added `criterion` as a **dev-dependency** (not a runtime dep). Justification:
  Phase 0 explicitly requires "criterion benches"; it is test-only and does not
  affect the shipped binary. Used with `default-features = false` (+
  `cargo_bench_support`) to avoid pulling plotters/rayon.
* Introduced `src/lib.rs`. Justification: the viewer was a binary-only crate;
  criterion benches and unit tests need to import `GraphState`. This is a
  structural (move-only) change required by the Phase 0 bench mandate; runtime
  behaviour is unchanged.

---

## Phase 1 — Index-IDs (intern NodeId → dense u32)

Branch: `perf/node-index-interning`.

### Changed

* `graph/interner.rs`: `NodeIndex(u32)` + `NodeInterner` (bidirectional
  `NodeId` ⇄ index map, free-list slot reuse). `GraphModel` keeps `NodeId` as
  the truth identity; the interner is a viewer-internal projection.
* `SpatialState` hot storage converted from `HashMap<NodeId, _>` to flat `Vec`s
  indexed by `NodeIndex`: `positions`, `velocities`, `placed`, `glow_until`,
  plus reused scratch buffers (`forces`, `active`, `visible_mask`). Accessors
  (`position_of`, `placed_positions`, `set_node_glow`, `release`, …) keep the
  call sites in `render`/`camera`/`gc` clean.
* Layout: `spring_edges: Vec<(NodeIndex, NodeIndex)>` rebuilt only on topology
  change (`springs_dirty`), never per frame. `force_step` rewritten to operate
  on `Vec`-indexed positions + the prebuilt spring list — **same O(N²)
  repulsion algorithm** (grid comes in Phase 2), but array indexing instead of
  string-keyed `HashMap` lookups, and zero per-frame `NodeId` clones in the
  force/integrate/spring loops.
* Slot reuse is safe: `release` clears all per-index state for the freed slot
  (tested). GC / `RemoveNode` go through `release`; edge/remove deltas mark
  `springs_dirty`.

### Gate 1 results

* `cargo test --workspace`: green (41 viewer tests; +interner roundtrip / reuse,
  slot-reuse-clears-state, edge-resolution-after-removal, force-step-finite).
* `cargo clippy --workspace --all-targets -- -D warnings`: clean.
* `cargo fmt --check`: clean.
* `force_step` improvement (criterion median, bench profile) — **purely from the
  data-layout change, algorithm unchanged**:

  | nodes | baseline | Phase 1 | speedup |
  |------:|---------:|--------:|--------:|
  | 500   | 33.7 ms  | 2.25 ms | ~15× |
  | 1000  | 146 ms   | 11.5 ms | ~13× |
  | 2000  | 720 ms   | 43.8 ms | ~16× |
  | 5000  | 4.29 s   | 293 ms  | ~15× |

  Measurable improvement at 1000+ nodes (Gate 1). Still O(N²) — 5000 nodes is
  293 ms; Phase 2 (uniform grid) targets the < 4 ms / < 12 ms gates.
* Zero per-frame `NodeId` clones in the `force_step` layout hot path (repulsion,
  spring, integrate loops use `NodeIndex` array access). Note:
  `visible_set_capped` still clones `NodeId`s to build the projection set — that
  is the model→viewer projection step (unchanged), not the layout algorithm;
  full `Gid` interning of the projection lands in Phase 6.

### Deviations

* None.
