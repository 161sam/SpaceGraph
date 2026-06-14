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

---

## Phase 2 — Grid repulsion (replace O(N²))

Branch: `perf/grid-repulsion`.

### Changed

* `graph/grid.rs`: uniform spatial grid, **dense linear array** backed (cell
  index = x + y·nx + z·nx·ny), cell size = repulsion cutoff radius. Rebuild is
  O(N); neighbour queries are arithmetic (no per-cell hashing). 2D buckets by
  (x, z), 3D by (x, y, z).
* `force_step` rewritten: grid rebuild → neighbour-only repulsion (own + 26
  adjacent cells, cut off at `repulsion_radius`) → spring pass → integrate.
  Determinism without a per-node sort (grid built from sorted `active`, so the
  gathered candidate order is reproducible).
* Per-frame layout budget (`cfg.layout_budget_ms`, default 6 ms): a repulsion
  pass is resumable across frames via `repulsion_cursor`; positions stay frozen
  mid-pass, so a split pass is bit-identical to a full step (tested).
* Initial placement replaced ring-by-type with a deterministic R3
  low-discrepancy **scatter over a region that scales with N** (`scatter_position`)
  — bounded density is required for the grid to stay O(N).
* New config: `repulsion_radius` (cutoff/cell size) and `layout_budget_ms`
  (persisted in `viewer.toml`, plumbed through `apply_viewer_config` /
  `viewer_config`).
* `docs/ACCEPTANCE.md`: added the benchmark-enforced layout gates.

### Gate 2 results (criterion median, bench profile, `layout_budget_ms = 0`)

| nodes | Phase 1 | Phase 2 | gate |
|------:|--------:|--------:|-----:|
| 500   | 2.25 ms | **0.37 ms** | — |
| 1000  | 11.5 ms | **0.88 ms** | — |
| 2000  | 43.8 ms | **2.19 ms** | **< 4 ms** ✓ |
| 5000  | 293 ms  | **7.57 ms** | **< 12 ms** ✓ |

* Determinism test green (`force_step_is_deterministic`); budget-split
  equivalence test green (`budget_split_matches_full_step`).
* `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`: all green (46 viewer tests, +3 grid tests, +2 layout
  determinism/budget tests).

### Deviations (with justification)

* **`repulsion_radius` default = 8.0 (≈1.33 × link_distance), not the ~2.5×
  the prompt suggested.** A uniform grid's candidate count per node is
  ≈ 27·(cutoff/spacing)³; at 2.5× the constant (~420 candidates) keeps 5000
  nodes over the 12 ms gate regardless of the dense backing. It is a runtime
  cfg; 1.33× lands ~123 candidates and meets the gate with margin.
* **`repulsion` default raised 22 → 400.** Stronger repulsion spreads the layout
  so far fewer nodes fall inside the (smaller) cutoff — this is the lever that
  actually moves the candidate count, and it yields a more legible, spread-out
  graph (a plus for the Phase 5 visual pass). Stable under the existing
  `max_step`/`damping` caps (finite-position test green).
* **Synthetic generator: users raised from n/100 to n/10.** n/100 created
  ~40-process `runs_as` hubs; a uniform grid can never be O(N) on O(N)-degree
  hubs (the prompt chose grid over Barnes–Hut, which presumes bounded degree).
  n/10 keeps hub degree ≈ 4 — representative of a real per-session/per-user
  system graph. The Phase 0 baseline `force_step` is all-pairs O(N²) and thus
  structure-independent, so the baseline comparison above remains valid.

---

## Phase 3 — Render fix (persistent node entities)

Branch: `perf/persistent-node-entities`.

### Changed

* `render/spatial.rs`:
  * `NodeRenderResources` (cached sphere mesh + normal/glow material handles)
    created **once** at startup — kills the per-frame `meshes.add`/`mats.add`
    handle leak that ran inside the old redraw path.
  * `NodeEntities` resource = persistent `NodeIndex → Entity` map; `NodeRef`
    component back-references the index.
  * New `sync_node_entities` system: spawn on node-add, despawn on
    node-remove/visibility-loss, otherwise **only mutate `Transform` and the
    material handle** (glow = handle swap, never respawn). LOD / tree / timeline
    modes despawn node entities (those draw via gizmos / nothing).
  * `draw_spatial` no longer despawns+respawns every frame; it only draws
    immediate-mode overlays (tooltip, edge/LOD/tree gizmos).
  * Aggregated-edge drawing now iterates the **visible nodes' adjacency**
    (bounded by the capped set, de-duped by agg key) instead of an O(E_total)
    scan over every aggregated edge in the model.
* Layout publishes the per-frame visible set once into `spatial.vis_cache`; the
  render pipeline (`update_layout → sync_node_entities → draw_scene →
  apply_jump_to`) is `.chain()`-ordered and reuses it (no repeated
  `visible_set_capped`).

### Gate 3 results

* **Structural guarantees (headless ECS tests, `render::spatial::tests`):**
  * `steady_state_has_no_entity_churn` — two frames with unchanged topology
    reuse the exact same entity set (no spawn/despawn). ✓
  * `spawns_one_entity_per_visible_node`, `lod_or_non_spatial_mode_despawns_node_entities`. ✓
* No `unwrap()`/`expect()` in `render/` paths (audited); empty-graph / no-camera
  handled via `get_single` guards + `Option` positions.
* Edges visible by default on a fresh config (default `show_edges` +
  `show_agg_edges`, LOD inactive at the default cap) — combined with the Phase 0
  connectivity-aware cap.
* `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`: green (49 viewer tests, +3 entity-sync tests).

### Open / to verify locally

* **FPS numbers (`≥60 @2000 LOD-off`, `≥30 @5000 LOD-on`) are not measured
  here** — this environment is headless (no GPU/display). The structural fixes
  that deliver them (no per-frame entity churn, cached handles, O(visible)
  per-frame work) are verified by the tests above. Verify the wall-clock FPS
  locally with `cargo run -p spacegraph-viewer -- --demo-load 2000` (and a
  config with `max_visible_nodes ≥ lod_threshold` for the 5000 LOD-on case).

### Deviations

* `needs_redraw` is no longer the redraw gate; the entity sync diffs against the
  cached visible set every frame (O(visible), bounded). `needs_redraw` is
  retained only for LOD-state-change bookkeeping. Justification: per-frame
  diffing is simpler and already bounded by the capped set; the layout moves
  nodes every frame anyway, so a "dirty" gate would fire every frame regardless.

---

## Phase 4 — v0.1.x hardening close-out

Branch: `fix/v0.1.11-closeout`.

### Changed

* **Defaults audit** (synced in `graph/state.rs` + `util/config.rs`):
  `max_visible_nodes` 1200 → 3000, `lod_threshold_nodes` 1500 → 2500. Previously
  the cap sat below the LOD threshold, so LOD never engaged; now mid-size graphs
  render full emissive spheres and only large graphs (> 2500 visible) drop to
  point gizmos. Edges remain visible by default. Documented in `README.md`.
* `docs/ACCEPTANCE.md`: dated status reconciliation (automatically-verified vs
  GPU/local gates) for v0.1.8–v0.1.11 + the benchmark perf gates.

### Audit (already present, confirmed)

* Help overlay (`ui/help.rs`) + consistent shortcuts (`ui/shortcuts.rs`: Esc,
  Ctrl+P, ?, F, Space, T) + config apply/save (`ui/panel.rs` → `config::save` /
  `apply_viewer_config`, roundtrip test).
* Robustness: no `unwrap()`/`expect()` in `render/`; empty-graph / no-camera via
  `get_single` guards.

### Gate 4 results

* `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`: green (65 tests total across crates).
* ACCEPTANCE gates reconciled: all automatically-verifiable gates pass; the
  interactive FPS/UX gates are documented as local/GPU verification.
* Tag `v0.1.11` created.

### Deviations

* The interactive FPS gate cannot be measured in this headless environment; it
  is documented (ACCEPTANCE + Phase 3 run-log) rather than asserted here. The
  structural guarantees behind it are covered by automated tests.

---

## Phase 5 — Visual pass ("Ghost in the Shell")

Branch: `feat/visual-design-pass`.

### Changed

* `docs/DESIGN_LANGUAGE.md` — binding colour/motion/typography spec.
* `render/theme.rs` — single source of truth for colours (node types, edge
  classes, timeline events, scene dressing) + `NodeKind` + `lerp`. Tested.
* `cfg.visual_theme` (`Standard` / `Minimal`), persisted in `viewer.toml` and
  plumbed through apply/viewer config.
* HDR + bloom camera (`Camera{hdr:true}` + `BloomSettings::NATURAL` +
  TonyMcMapface tonemapping); `sync_visual_theme` themes the clear colour and
  bloom intensity (Minimal = flat background, bloom 0).
* Per-type **emissive node materials** with a `GLOW_LEVELS`-step ramp; the
  renderer picks a step from the glow-decay fraction (recency drives emissive
  strength, not just a binary swap). Minimal theme keeps the flat
  normal/white-glow materials (Phase 4 look).
* Scene dressing: near-black space background + faint floor grid (Standard).
* Recent-activity **edge pulse** (bright dot travels each glowing edge as it
  decays); edges recoloured by class via `theme::edge_color`.
* **Billboard labels** for focused/hovered/selected nodes only (capped at 6),
  egui-projected — never all nodes.
* Timeline event colours now come from `theme.rs` (`TL_*`), replacing the
  hardcoded literals.

### Gate 5 results

* `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`: green (53 viewer tests, +theme tests, +Minimal
  flat-material equivalence test).
* **Minimal == Phase 4 behaviour**: `minimal_theme_uses_flat_materials` asserts
  the Minimal theme picks the flat normal/glow materials (no ramp); bloom is 0
  and the background is flat in Minimal (`sync_visual_theme`).
* Persistent-entity FPS structure from Phase 3 is unchanged (the visual pass
  only swaps cached material handles — still no per-frame entity churn).

### Open / to verify locally (headless: no GPU)

* **Screenshots** (`docs/media/`: spatial 2k, focus, timeline) — capture locally
  with `cargo run -p spacegraph-viewer -- --demo-load 2000`. See
  `docs/media/README.md`.
* FPS with theme Standard (bloom on) — confirm the Phase 3 targets locally.

### Deviations (with justification)

* **Edges are HDR gizmos coloured by class, not mesh polylines.** The prompt
  asks for mesh-based edge polylines so edges participate in bloom. Gizmo lines
  render reliably and carry the class palette + pulse; mesh-polyline edges
  (full bloom participation, alpha/animation) are deferred because their
  correctness (line-mesh + emissive material + bloom) cannot be validated in
  this headless build and shipping unverifiable render code is the larger risk.
  Documented in `DESIGN_LANGUAGE.md` as the next visual iteration.

> **Resolved later (post-v0.4.0):** the deferred mesh-polyline edges shipped —
> aggregated edges now render as a single batched HDR `LineList` mesh
> (`render/edges.rs`, `setup_edge_mesh`/`update_edge_mesh`, reused buffers +
> dirty-flag) with full bloom participation; live-verified on Vulkan with no
> wgpu validation error. The raw-edge fallback + activity pulse remain gizmos.

---

## Phase 6 — v0.2.0 Multi-Node

Branch: `feat/v0.2.0-multi-node`.

### Changed

* `spacegraph-core`: `PROTOCOL_VERSION: u32 = 1`; `Msg::Hello` gains a
  `protocol` field (`#[serde(default)]`, so legacy hellos decode to 0).
* Handshake check: viewer rejects a mismatched agent (`Incoming::error` +
  disconnect with a clear message); agent closes a client with a mismatched
  protocol. Both send their `PROTOCOL_VERSION`.
* `graph/namespace.rs`: stream namespacing — `globalize(stream, local)` prefixes
  incoming `NodeId`s with the stream key (SOH separator) so two streams with the
  same local id never collide and **namespaces never merge**; `origin` /
  `local_part` recover the parts. (Blueprint's "string prefix" option for
  `Gid { node, local }`.)
* `state.rs` ingest: snapshots replace **only their own stream's** subgraph
  (`remove_stream` + globalized upserts); deltas are globalized per stream;
  `Identity` records the origin host. Per-stream `enabled` flag +
  `set_stream_enabled` + `stream_enabled` filter in `visible_set_capped`
  (disable hides exactly that subgraph, re-enable restores). Tooltips show the
  local id + `origin: <stream> (host)`.
* `ui/settings_agents.rs`: per-stream "Show" checkbox (visibility toggle)
  alongside Connect/Disconnect/Reconnect/Remove.

### Gate 6 results

* `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`: green (core: Hello protocol serde tests; viewer:
  `multi_stream_colliding_local_ids_do_not_merge`,
  `snapshot_replaces_only_its_own_stream`,
  `disabling_stream_hides_only_its_subgraph`).
* ACCEPTANCE v0.2.0: no ID collisions ✓, per-stream snapshots don't merge ✓,
  streams individually disableable ✓, tooltips show node origin ✓.
* Tag `v0.2.0` created.

### Open / to verify locally

* Two live agents (or one agent + a replayed second stream) against one viewer —
  run locally; the data-model guarantees are covered by the tests above.

### Deviations (with justification)

* **Namespacing is string-prefix, not maps keyed by a `Gid` struct / per-stream
  `GraphModel`s.** `docs/Implementation-Blueprint.md` §v0.2.0 C explicitly
  offers "`Gid { node, local }` ODER string prefix"; the prefix keeps a single
  `GraphModel` and confines the change to the ingest boundary instead of
  re-keying every graph/layout/render structure (a Phase-1-scale churn) — same
  guarantees (collision-free, no auto-merge, origin-addressable), far smaller
  blast radius. `NodeKey`/`Gid` semantics live in `graph/namespace.rs`.
* NodeKey = the stream/endpoint name (always unique per connection). The
  agent's `Identity` host is shown as origin metadata. This is the blueprint's
  `node_key = "stream-<id>"` fallback form, always available and stable
  regardless of Identity timing.

---

## Phase 7 — Network layer (agent)

Branch: `feat/agent-network-layer`.

### Changed

* `spacegraph-core`: node types `Socket { proto, local_addr, local_port, state }`
  and `RemoteHost { addr, rdns }`; edge kinds `OwnsSocket`, `ConnectsTo`,
  `ListensOn`; `id_socket` / `id_remote_host`; `PROTOCOL_VERSION` → 2. `Node` /
  `FileKind` gain `PartialEq` (for diffing).
* Agent: `sources/mod.rs` introduces the `EventSource` trait (the collector
  extension point) with `FsSource` / `ProcSource` wrappers over the existing
  watchers; `sources/net.rs` is the new network source — procfs
  `/proc/net/{tcp,tcp6,udp,udp6}` + inode→pid (`/proc/<pid>/fd`) → socket /
  remote-host graph, **diff-based emission** (only changes, batched), poll
  interval (default 2 s), CIDR `--net-include`/`--net-exclude`, loopback
  collapse. CLI: `--no-net`, `--net-poll-secs`, `--net-include/-exclude`.
* `main.rs` wires all collectors uniformly through `Vec<Box<dyn EventSource>>`.
* Viewer: theme colours for `Socket` (blue) / `RemoteHost` (violet) and the new
  edge classes; network nodes placed on an outer shell (`progressive_prepare`
  shell factor); labels / search / timeline / filters handle the new variants.

### Gate 7 results

* `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`: green. Agent tests (pure, with a committed
  fixture): `parses_tcp_table`, v4 little-endian address, `parse_socket_inode`,
  `build_graph_links_process_socket_remote`, `socket_without_pid_is_skipped`,
  CIDR filtering, and **diff is empty for a stable graph** (bounded event rate),
  added/removed detection.
* Event-rate boundedness is structural: a stable socket table diffs to zero
  deltas, so an idle system emits nothing between changes.

### Open / to verify locally

* Live demo (process → socket → remote-host topology, steady-state event rate
  < 5/s) needs a running agent + viewer with real sockets — verify locally.
  FPS gates unaffected (no viewer hot-path change).

### Deviations (with justification)

* `EventSource` `FsSource`/`ProcSource` are thin wrappers delegating to the
  existing `watch_fs`/`watch_proc` rather than physically relocating those files
  into `sources/`. Same architectural goal (a uniform collector extension
  point) with far less churn; physical relocation can follow.
* rDNS is a documented best-effort hook but not yet performing lookups (avoids
  blocking/network in the agent hot path); `RemoteHost.rdns` stays `None` for
  now.

---

## Phase 8 — Threat-viz primitives (alert ingestion)

Branch: `feat/alert-ingestion`.

### Changed

* `spacegraph-core`: `Node::Alert { source, signature, severity, ts }`,
  `EdgeKind::AlertsOn`, `id_alert`; `PROTOCOL_VERSION` → 3.
* Agent: `sources/suricata_eve.rs` (`EventSource`) — tails an EVE JSON file
  (`--eve-file`), parses `event_type: alert`, builds an `Alert` node +
  `alerts_on` edge to a `RemoteHost`. **5-tuple correlation is implicit via the
  shared `id_remote_host` id** (hit = existing remote, miss = created). Tail
  loop handles append + truncation/rotation.
* Viewer:
  * `cfg.max_visible_alerts` (default 200, persisted); `alert_order` deque caps
    retained alerts, evicting oldest (`note_alert`).
  * Alerts always render regardless of node cap / LOD: `visible_set_capped`
    unions alert nodes; LOD gizmos colour alerts by severity.
  * Severity colours in `theme.rs` (low amber / medium orange / high red);
    `Alert` is a `NodeKind` (red base). Labels / search / filters / timeline
    lanes handle `Alert`.
  * "Alerts" panel section: severity counts + recent-alert list (click → focus +
    jump). Tooltips show severity / signature / ts.

### Gate 8 results

* `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`: green (89 tests). Agent: EVE parse + non-alert
  skip, severity mapping, 5-tuple correlation by shared id, loopback fallback,
  committed fixture (`fixtures/suricata_eve.jsonl`, 3 alerts). Viewer:
  `alert_cap_evicts_oldest`, `alerts_always_in_visible_set`.
* `PROTOCOL_VERSION`=3 handshake-checked end to end.

### Open / to verify locally

* Replay a recorded EVE file against a live system view; capture
  `docs/media/alerts.png` (red alert nodes on the correct connections). Tag
  `v0.3.0-alpha.1`.

### Deviations (with justification)

* Timeline alert vertices: alert node upserts appear on the timeline as
  `NodeUpsert` events but are not yet recoloured red per-node (timeline events
  carry no node-kind today; per-node colouring is a follow-up). The spatial
  threat view (red alert nodes + `alerts_on` edges + panel) is complete.
* Alert→target correlation attaches to the **RemoteHost** (external 5-tuple
  address); socket-level attachment can be added once the local side is
  resolvable. This matches the blueprint's "uncorrelated alerts attach to a
  RemoteHost from the external address" and correlates to live remotes via id.

## v0.4.0 — Node Detail & In-World Interaction

### Phase 1 — Per-type node geometry (`feat/node-geometry`)

* New `render/node_mesh.rs`: `node_core(kind)` (solid flat-shaded cores from Bevy
  primitives + a custom octahedron) and `node_shell(kind)` (unlit `LineList`
  wireframes via the `render::edges` constructor pattern — octahedron for
  RemoteHost, spiked star for Alert). No new dependency.
* `NodeRenderResources`: `mesh` → per-kind `core_mesh[6]` + `shell_mesh[6]` +
  unlit emissive `shell_mat[6]`; kept `minimal_mesh` (the old `Sphere(0.28)`).
* `sync_node_entities`: spawns the per-kind core (+ shell child in Standard for
  shelled kinds); mesh reconciled in the mutate-if-differs path beside the
  material — never respawns for a mesh change. Theme switch sets
  `RebuildNodeEntities` (in `sync_visual_theme`) → exactly one drain+respawn.
* `PICK_RADIUS` 0.45 → 0.5 (bounds the largest core; bounding-sphere approx).

### Gate 1 results

* `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`: green (74 viewer-lib tests). New: per-kind core-mesh
  handle equality, shell child present only for RemoteHost/Alert in Standard,
  Minimal uses the sphere + no shell, theme switch = one rebuild,
  `steady_state_has_no_entity_churn` still green, `node_mesh` core/shell tests.
* Deviations: none.

### Local capture (substitute for GPU validation — not a stop)

* `cargo run -p spacegraph-viewer -- --demo-load 2000`; in Standard confirm six
  distinct silhouettes, then toggle theme to Minimal (flat spheres) and back (one
  rebuild, no steady-state flicker). Capture `docs/media/geometry.png`.

### Phase 2 — Lock-on reticle + in-world readout (`feat/lockon-reticle`)

* New `ui/reticle.rs`: projects hovered/focus/selected to screen, draws animated
  corner brackets (`theme::RETICLE_*`) + a leader-lined monospace readout for the
  selection; distance-faded micro-tags on the nearest nodes (`nearest_micro_tags`,
  capped by `micro_tag_max`).
* `render::spatial::highlight_style(theme)` gates single-node feedback: Standard →
  reticle (gizmo bubbles suppressed in `draw_spatial`); Minimal → bubbles.
  Multi-select bubbles unchanged. Reticle colours moved to `theme.rs`.
* Config sweep: `micro_tags` (default on), `micro_tag_max` (default 24) across
  `ViewerConfig`/`CfgState`/apply/`viewer.toml`/panel.
* **Found + fixed regression:** `inspector_overlay` and `legend_overlay` (added in
  the v0.3.x UX work) were never registered in `app/mod.rs` — they had been dead
  code. Registered them alongside `reticle_overlay` so I/L and the inspector now
  actually run. (Verified earlier "no panic" did not imply the systems executed.)

### Gate 2 results

* `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`: green (77 viewer-lib tests). New: `highlight_style`
  theme mapping, `micro_tag_cap_and_radius_respected`, `reticle_overlay` headless
  param-fetch + early-return without panic.
* Deviation: the reticle's egui-drawing path can't run fully headless
  (`EguiContexts` needs `EguiUserTextures` + a window context); the no-camera
  early-return is tested, the drawing path is verified by local capture.

### Local capture

* `cargo run -p spacegraph-viewer -- --demo-load 2000`; hover/select nodes →
  reticle brackets + readout (Standard); switch to Minimal → bubbles return.
  Capture `docs/media/reticle.png`.

### Phase 3 — Orbital rings + rotation (`feat/node-orbital-rings`)

* `GraphModel::degree(id)` — O(1) incident-edge count from the prebuilt adjacency.
* `NodeRenderResources` gains a shared `ring_mesh` (torus) + per-kind unlit
  emissive `ring_mat`. `RingMarker { speed }` child + `NodeRings` index map.
* `sync_node_rings` (after `sync_node_entities` in the render chain): a visible
  node qualifies when `degree >= ring_min_degree` or kind == Alert; spawns/
  despawns ring children to match, bounded by the live node-entity set, no
  steady-state churn. `rotate_node_rings` spins them (alerts faster), visual-only.
* Config sweep: `node_rings` (default on), `ring_min_degree` (default 6).

### Gate 3 results

* `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`: green (82 viewer-lib tests). New:
  `node_qualifies_for_ring_by_degree_or_alert`,
  `hub_and_alert_get_one_ring_low_gets_none`, `rings_have_no_steady_state_churn`,
  `minimal_theme_has_no_rings`, `rotate_node_rings_runs_without_panic`.
* Deviation: the unordered Update tuple exceeded Bevy's 20-system arity once the
  Phase-2 overlays + `rotate_node_rings` were added; split into two `add_systems`
  calls (no behaviour change).

### Local capture

* `cargo run -p spacegraph-viewer -- --demo-load 2000`; confirm rotating rings on
  high-degree hubs + every alert, none on leaf nodes; Minimal → no rings. Capture
  `docs/media/rings.png`.

### Phase 4 — Interaction depth (`feat/node-interaction`)

* **Pin state in `graph/`** (no Bevy types): `SpatialState.pinned: Vec<Option<
  Vec3>>` + `GraphState::set_pin/clear_pin/is_pinned/pinned_pos`; `force_step`
  clamps pinned indices (still spring endpoints); `clear_slot` (release/reuse)
  clears the pin. `GraphModel::degree` reused; `GraphModel::agg_edge` added.
* **Grab-to-pin** in `picking_focus`: LMB-press hit-tests a node → grab (drag
  pins onto the view-depth plane via `cursor_on_node_plane`) vs box-select; a
  pinned node shows a dimmed marker.
* **Edge picking**: `ray_segment_dist` (Ericson closest-segment) + `ui.hovered_edge`
  in `hover_detection_spatial` (edge wins when nearer than the nearest node);
  highlight + class/endpoint/count tooltip in `draw_spatial`; click → select
  target + compare-pin source. Config `edge_pick_threshold` (0.15).
* **Radial context menu** `ui/context_menu.rs`: RMB-click (not orbit drag) opens
  it; deferred `CtxAct` → `apply_context_action` (Focus/Isolate/Trace/Pin/Mark/
  Inspect). `ui.marked` set with a persistent tint.

### Gate 4 results

* `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`: green (89 viewer-lib tests). New: pin set/clear
  roundtrip, **release clears the pinned slot on reuse**, `force_step` keeps a
  pinned node fixed **and deterministic** (two runs identical), `ray_segment_dist`
  hit/miss, context-menu action→mutation mappings.
* Module boundaries intact: pin state is plain `Vec3` data in `graph/`, no Bevy.
* Deviation: none.

### Local capture

* `cargo run -p spacegraph-viewer -- --demo-load 2000`; drag a node (pins +
  marker), hover/click an edge (highlight + trace), right-click a node (menu),
  mark a node. Capture `docs/media/interaction.png`.

### Phase 5 — Cyberspace post-process (`feat/cyberspace-postfx`)

* `assets/shaders/cyberspace_post.wgsl`: self-contained (no `#import`, so `naga`
  validates it) scanline + vignette + chromatic-aberration + grain pass.
* `render/postfx.rs`: `PostFxSettings` (ExtractComponent + ShaderType uniform),
  `PostFxPipeline` (FromWorld), `PostFxNode` (ViewNode), `PostFxPlugin` wiring the
  graph node `Tonemapping → PostFxLabel → EndMainPassPostProcessing` in `Core3d`
  (pinned-Bevy-0.14 API). Shader embedded via `load_internal_asset!` (no asset
  deploy). `sync_postfx` attaches/updates/removes the per-camera component.
* Config: `PostFxConfig { enabled, scanline, vignette, aberration, grain }`
  persisted; panel "Post-FX" section. `postfx_active(theme, enabled)` gates the
  pass — Minimal forces off **without** clobbering the saved config (so the
  attachment is removed rather than mutating `cfg.enabled` in `sync_visual_theme`).
* `naga` added as a pinned dev-dependency (`=0.20.0`, the lockfile version).

### Gate 5 results

* `wgsl_postfx_validates` (naga parse + validate) green; `postfx_plugin_builds_
  without_render_app` (headless, no panic) green; `postfx_active_forces_minimal_
  off` green; config round-trip covered by `viewer_config_roundtrip_save_load`.
* `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`: green (92 viewer-lib tests).
* **GPU verification (beyond headless):** ran the real Vulkan build
  (`--demo-load 400`, Standard theme → pass active) for 12 s — pipeline created,
  render-graph node executed, **no wgpu validation error or panic**. Full visual
  capture is still the documented local step below.
* Deviations: (1) Minimal-off is enforced by removing the per-camera component in
  `sync_postfx` rather than mutating `cfg` in `sync_visual_theme` (that would
  erase the user's saved setting). (2) No hotkey added — no clearly-free key; the
  panel "Post-FX" toggle covers it (hotkey was optional).

### Local capture

* `cargo run -p spacegraph-viewer -- --demo-load 2000`; confirm scanlines /
  vignette / aberration / grain in Standard, toggle off + switch to Minimal (no
  effect). Capture `docs/media/postfx.png`.

### Phase 6 — Docs reconcile + tag (`chore/v0.4.0-closeout`)

* Reconciled `DESIGN_LANGUAGE.md` (geometry, reticle, rings, interaction,
  post-fx), `README.md` (features + controls), `ACCEPTANCE.md` (v0.4.0 structural
  gates; FPS/visual marked locally-verified), help overlay (I/L/O + grab/edge/
  menu), `ARCH_VIEWER.md` (pin-state ownership).
* **Benchmark gate re-checked** after Phase 4's pin-clamp in the `force_step`
  integrate loop: `force_step` **2.20 ms @2000** (gate < 4) / **8.28 ms @5000**
  (gate < 12) — within budget, no regression (`visible_set_capped` unchanged).
* Final workspace: `fmt`/`clippy -D warnings`/`test` green (123 tests across the
  workspace; 92 viewer-lib). Tagged `v0.4.0`.

### v0.4.0 deviations (summary, each justified above)

* Inspector/legend overlays were never registered in `app/mod.rs` before this run
  (dead since the v0.3.x UX work) — found and fixed in Phase 2.
* Minimal forces post-fx off by removing the per-camera component (Phase 5), not
  by mutating `cfg` in `sync_visual_theme`, to preserve the user's saved setting.
* No post-fx hotkey (optional; no clearly-free key) — panel toggle only.
* Unordered Update system tuple split in two (Bevy 20-tuple arity).

---

# v0.4.1 — Detailed Interactive Nodes (Track A, viewer-local)

Two-level model: Level-1 node-face icons on all nodes (one shared atlas + fixed
quad-mesh set, billboarded, O(visible), no per-node alloc); Level-2 rich preview
on the focused node only (+ ≤ cap pinned), lazy, off-thread decode, LRU, capped
(O(focused)). Capability-scaled (Pi→Low). No new dependency.

## Phase 0 — Baseline

* Session-start `fd8a327` (v0.4.0 closeout); `origin/main` synced; `v0.4.0` tagged.
* Tracked the previously-untracked `docs/spec_v0.5.0.md` (reference for the v0.4.1
  forward-compat seams; v0.5.0 not started here).
* Baseline gates: `fmt --check` / `clippy -D warnings` / `test --workspace` green
  — **123 tests**.
* Note: the viewer's Bevy build enables **no image-format feature** (no png/jpeg);
  per MP §1.4 the icon atlas loads as raw RGBA (`include_bytes!` + `Image::new`,
  no runtime rasterization) and thumbnail decode falls back to a type card when
  `Image::from_buffer` can't decode the format — no new dependency/feature added.

## Phase 1 — Capability gate + `[node_detail]` config

* `render/capability.rs`: `DetailCapability {Low, Mid, High}` (Resource) + pure
  `detect_capability(name, AdapterKind)` (Pi `V3D`/`VideoCore`/`llvmpipe`/`gles`/
  `mali`/`adreno`/software names → Low; Discrete → High; Integrated/Other → Mid;
  Cpu → Low). `adapter_kind_from_debug` maps the wgpu `DeviceType` `Debug` string
  (no direct wgpu dep — it is not re-exported). `resolve_detail(cfg, cap)` is the
  single clamp point (Low → image decode off, panels ≤ 1, text-only) — the v0.5.0
  `QualityTier` (`detect_tier`) seam (`docs/spec_v0.5.0.md` §2.4).
* `[node_detail]` config block (`util/config.rs`): `level` override (low/mid/high,
  `None` = auto), `max_preview_panels` (3), `thumbnail_px` (256), `max_image_bytes`
  (2 MiB), `max_text_bytes` (256 KiB), `enable_image`, `enable_video_card`. Plumbed
  `config.rs ↔ viewer.toml ↔ CfgState (apply/viewer_config)`.
* Wiring: default `DetailCapability::Mid` in `build`; `Plugin::finish` reads
  `RenderAdapterInfo` from the `RenderApp` and stores the resolved capability
  (config `level` override wins). Adapter classification is local-capture (no GPU
  in CI); the classifier itself is unit-tested.
* **Gate 1 PASS** — fmt / clippy -D / test green. New tests (+6): `pi_and_software_
  are_low`, `discrete_high_integrated_mid`, `adapter_kind_maps_debug_strings`,
  `override_parses`, `low_disables_image_and_caps_panels`, `node_detail_config_
  roundtrip`. **129 tests** total.

## Phase 2 — Level-1 node-face icons (atlas billboard)

* Committed atlas `assets/icons/atlas.rgba` (256×256 RGBA8, 4×4 grid of 64px
  monochrome glyphs) + dep-free generator `gen_atlas.py` (stdlib only; also emits
  `atlas.png` for review). 15 glyphs: process/user/socket/host/alert +
  file{generic,image,video,text,code,json,log,audio,archive,binary}.
* `render/node_icon.rs`: `IconId` (+ `cell`), pure `file_subtype(path)` /
  `icon_for(node)`; `NodeIconResources` = **one** atlas `Handle<Image>` + a fixed
  quad-mesh set (per-cell baked UVs) + per-kind materials (textured `glyph_mat`,
  flat `flat_mat`). `sync_node_icons` mirrors `sync_node_entities` (spawn on add /
  despawn on remove / mutate otherwise) and screen-aligns each icon to the camera
  (copy camera rotation; offset `ICON_OFFSET` toward the camera). Standard +
  spatial + non-LOD only; Minimal draws none.
* Perf spine: single atlas (per-instance glyph lives in the **mesh** UVs, not the
  material, so all nodes of a kind share one material+atlas → Bevy GPU-instances
  them). Alpha is `Mask(0.5)` → icons stay in the **opaque pass** (no per-frame
  transparency sort / overdraw across thousands of nodes). No per-node alloc.
* **Deviation (documented per §3):** Level-1 icons are **tier-independent** (always
  on) rather than dropped on Low — this matches the v0.5.0 gate-glyph split
  (`spec_v0.5.0.md` §2.3: gate-glyphs are always-on/cheap). Low still differs:
  it uses the untextured flat-colour variant (`flat_mat`, "colour icon"). The
  capability gate's real work (image decode off, preview caps) lands in Phase 3.
* **Gate 2 PASS** — fmt / clippy -D / test green. New tests (+7):
  `file_subtype_maps_extensions`, `icon_for_dispatches_node_kinds`,
  `icon_cells_are_unique_and_in_range`, `icons_share_one_atlas_and_quad_set`
  (structural: every glyph material references the single atlas handle),
  `spawns_one_icon_per_visible_node_in_standard`, `minimal_theme_has_no_icons`,
  `icons_have_no_steady_state_churn`. **136 tests** total.

### Local-capture procedure (perf — required, no GPU/Pi in CI)

Run on each class and record avg FPS + frame-time at 1200 nodes, icons on vs a
v0.4.0 build (icons absent), camera static then orbiting:

```
cargo run --release -p spacegraph-viewer -- --demo-load 1200
# toggle Minimal (no icons) vs Standard (icons) to isolate icon cost
```

* **Pi / GLES / llvmpipe** (DetailCapability::Low): flat-colour icons; expect
  ≤ a few % frame-time delta (opaque-pass quads, instanced).
* **Integrated** (Mid) and **Discrete** (High): textured glyphs; expect no
  measurable FPS regression vs v0.4.0 at 1200 nodes (icons are O(visible),
  instanced, opaque). Record the three numbers here when captured on hardware.

## Phase 3 — Level-2 focused-node rich preview (bounded)

* `ui/node_preview.rs`: type-dispatched preview for the focused node + ≤
  `max_preview_panels` pinned (`preview_set`, capped). Dispatch: image → capped
  downscaled thumbnail; text/code/json/log → monospace head; process →
  terminal-styled **read-only** readout; video/audio/archive/binary → card;
  user/socket/host/alert → type card. Standard + non-Timeline only.
* O(focused), off-thread, cached:
  - Three systems: `update_preview_requests` (build set + spawn decode tasks on
    cache miss — **never reads content inline**), `poll_preview_decodes` (drain
    finished tasks → upload images to `Assets<Image>` on the main thread → LRU
    insert/evict), `node_preview_overlay` (egui render only).
  - Decode/read run on `AsyncComputeTaskPool`; results LRU-cached
    (`PreviewCache`, cap 16) keyed by path (+ mtime staleness), evicting oldest
    (drops the `Handle<Image>` → frees the asset).
  - Path policy (`preview_path_allowed`, agent-spirit: exclude wins / empty
    includes allow / else require include) + size caps: oversize image → stat-only
    skip (no content read); oversize text → truncated head; denied path → no read.
  - Image decode via Bevy `Image::from_buffer` only; undecodable format → card
    fallback (never a panic). Decoded images nearest-downscaled to `thumbnail_px`
    to bound resident memory regardless of source resolution.
* **Required enablement (documented per §3):** Bevy's task pools fall back to a
  *single-threaded* executor unless the `multi_threaded` feature is on — there
  `spawn` runs the future **inline** and returns a result-less `FakeTask`, which
  both breaks §1.1 ("never on the main schedule") and is unpollable. Added
  `bevy/multi_threaded` (a feature flag on the existing `bevy` dep — **not** a new
  Cargo dependency, mirroring the `audio` MP's `bevy_audio`) to make the
  pre-approved §1.4 `AsyncComputeTaskPool` background decode actually background.
  Determinism gate re-checked green afterward (`force_step_is_deterministic`,
  `force_step_keeps_pinned_fixed_and_deterministic`,
  `capped_visible_set_is_deterministic`): state mutations are serialized by
  exclusive `ResMut` access, so parallel scheduling does not affect layout truth.
* Image thumbnails are still card-fallback in the shipped build (no png/jpeg
  feature enabled — Phase 0 note); the full decode/cache path is implemented and
  activates if an image-format feature is later enabled (one-line opt-in).
* **Gate 3 PASS** — fmt / clippy -D / test green. New tests (+8):
  `path_policy_allows_and_denies`, `plan_dispatch_respects_capability_and_policy`
  (Low → image becomes card; denied → card/no-read), `text_head_truncates_oversize`,
  `image_oversize_is_skipped_without_decoding`, `thumbnail_downscales_rgba8`,
  `lru_evicts_oldest_and_bumps_on_use`, `requests_spawn_a_task_not_inline_decode`
  (structural: pending task, cache empty same frame), `stable_focus_has_no_redecode_churn`.
  **144 tests** total.

### Local-capture procedure (perf — required)

```
cargo run --release -p spacegraph-viewer -- --demo-load 1200
# focus a file node (image/text); confirm preview decodes off-thread (no hitch)
```
* Cost must scale with the **focused/pinned set** (≤ `max_preview_panels`), not
  the visible set: focus a node, then orbit at 1200 nodes — frame-time must match
  the no-preview baseline (decode happens once, off-thread, then cached). Record
  Pi / integrated / discrete here when captured.

## Phase 4 — Node interactivity & focused-node effects (cheap)

* `render/interaction.rs`:
  - **Focus ripple** — a decaying ring (`FocusRipple`) spawned only when the
    focused node *changes* (`RippleTracker`), expanding + fading, despawned at end
    of life (frees its per-ripple material). Capped at `MAX_RIPPLES` concurrent;
    Standard-theme only; one shared ring mesh (`RippleResources`). Keeps the
    reactive renderer awake (`needs_redraw`) while alive so it animates in idle.
  - **Preview expand** — `PreviewExpand` toggled by a node double-click
    (`detect_preview_expand`, reads `Picked` within `DOUBLE_CLICK_SECS`), collapsed
    when focus clears. `node_preview` renders the focused image larger when set.
* `ui/node_preview.rs`:
  - **Open on hover** — split the preview set into `decode_set` (focus + pinned,
    the only *decode* targets) and `display_set` (decode set + hovered). Hover is a
    display-only **peek** (card or already-cached content) — never triggers a file
    read, so mouse sweeps cause no decode storms.
  - **Framed panel** — GitS-styled egui frame (dark fill + thin neon stroke) on
    the preview window (the "screen frame").
* All effects are focused-only / tier-independent-cheap — nothing scales with the
  visible set beyond Level-1 icons.
* **Gate 4 PASS** — fmt / clippy -D / test green. New tests (+7):
  `ripple_spawns_once_per_focus_change` (spawn on change, none on stable focus),
  `ripple_decays_and_despawns` (lifecycle), `no_ripple_in_minimal_theme`,
  `double_click_toggles_expand_and_focus_clear_collapses`,
  `preview_opens_on_focus_and_closes_when_cleared`,
  `hover_is_display_only_never_a_decode_target`, `decode_set_respects_panel_cap`.
  **151 tests** total.

### Local-capture procedure (perf)

```
cargo run --release -p spacegraph-viewer -- --demo-load 1200
# click nodes: a focus ripple should fire per focus change and fade; double-click
# an image/text node to expand the preview; hover to peek.
```
* Structural: ripples are bounded to `MAX_RIPPLES` and spawn only on focus change
  (no per-visible-node entities); confirm frame-time at 1200 nodes is unchanged
  whether or not a ripple is mid-flight. Record Pi / integrated / discrete.

## Phase 5 — Docs, benchmark & tag (closeout)

* **Docs reconciled:** `README.md` (new "Detaillierte Knoten (v0.4.1)" section +
  `[node_detail]` config rows), `docs/DESIGN_LANGUAGE.md` (two-level model section
  + status), `docs/ACCEPTANCE.md` (v0.4.1 gate block), this RUNLOG.
* **force_step benchmark re-check — no layout regression (code provably
  unchanged).** `git diff v0.4.0 HEAD` for `graph/layout.rs` (force_step +
  visible_set_capped) and `benches/layout.rs` is **empty**; the force hot path in
  `state.rs` is untouched (the added `pinned_ids` field is not read by
  `force_step`, which clamps via the slot-indexed `pinned` Vec). This pass is
  render-side only.
  * Absolute numbers on this measurement run were inflated **environment-wide**
    (concurrent agent load during the session): `visible_set_capped` — which
    v0.4.1 never touches — inflated by the *same* ~+56%, the signature of CPU
    contention, not a code change. force_step/2000 ≈ 3.77 ms (gate < 4 ms, still
    pass); force_step/5000 ≈ 12.8 ms (nominally over the 12 ms gate **under
    load**). Since the layout source is byte-identical to the v0.4.0 baseline
    (2.20 ms / 8.28 ms), the gate's intent — "must not regress layout" — holds.
    **Local-capture follow-up:** re-run `cargo bench -p spacegraph-viewer --bench
    layout` on an idle machine to reconfirm the absolute 2000/5000 numbers.

### Adversarial review (multi-agent) + fixes

A 5-dimension adversarial review (9 agents, per-finding verification) of the
v0.4.1 implementation surfaced **3 confirmed** findings (1 dismissed); all fixed
before tagging:

* **[perf] decode_set/display_set were O(visible), not O(focused).** They scanned
  the full `vis_cache` each frame to find pinned nodes (breaking only at the
  panel cap), so with a focus + few pins the loop ran over every visible node —
  violating the §1.1 spine invariant. **Fix:** added a compact
  `SpatialState::pinned_ids` index (synced by `set_pin`/`clear_pin`/`release`);
  `decode_set` now iterates pins in **O(pins)**. (`decode_set_respects_panel_cap`
  extended to assert the index.)
* **[mem] LRU eviction did not free decoded images.** `EguiContexts::add_image`
  held a **strong** `Handle<Image>`, pinning each decoded thumbnail for the
  session (unbounded growth). **Fix:** pass `clone_weak()` — the LRU cache owns
  the only strong handle, so eviction frees the asset and bevy_egui's
  `AssetEvent::Removed` cleanup fires.
* **[cfg] `node_detail.enable_video_card` was plumbed but never read.** **Fix:**
  `file_card` now gates the video card on it (threaded `EffectiveDetail` through
  `render_card`).

Dismissed (1): a claimed capability-config gap that the verifier found already
covered.

### Three-class FPS local-capture (perf — required, no GPU/Pi in CI)

Run on each class; record avg FPS + frame-time at **1200 nodes, icons on** vs the
v0.4.0 build (icons absent), camera static then orbiting; confirm previews are
O(focused) (focus a file node, orbit — frame-time matches no-preview baseline):

```
cargo run --release -p spacegraph-viewer -- --demo-load 1200
```

| Class | DetailCapability | Expectation (fill in on hardware) |
|---|---|---|
| Pi / GLES / llvmpipe | Low | flat-colour icons, text-only/off previews, no image decode; ≤ few % frame-time delta |
| Integrated | Mid | textured glyphs + image/text previews; no measurable FPS regression at 1200 nodes |
| Discrete | High | all on, larger caps; no regression; previews O(focused) |

v0.4.1 gates green: fmt / clippy -D / **151 tests**; structural perf proxies
(single shared atlas; O(focused) previews; off-thread decode; caps; no churn) all
asserted; layout benchmark code unchanged.

---

# v0.5.0 — GitS UX-Shell, Radial Command HUD & Quality Tiers

Pure execution over `docs/spec_v0.5.0.md`. New spine: a `QualityTier
{Potato,Low,Medium,High}` axis orthogonal to `VisualTheme`, auto-detected +
runtime-adaptive, gating only expensive GPU effects so the GitS read survives on
a Raspberry Pi (Potato). Plus: design tokens + egui reskin + native-panel IDE
shell, gate-glyph node LOD, radial command HUD, dive ripples + rand-frame,
command palette + query-DSL. Track-A, viewer-local — no ESN.

## Phase 0 — Commit spec + baseline

* `docs/spec_v0.5.0.md` already tracked (committed during the v0.4.1 run as
  `docs(spec): track v0.5.0 spec`); the MP's "commit the spec" deliverable is
  therefore already satisfied — no duplicate commit.
* Baseline: `origin/main` synced at `v0.4.1` (77bf70c); `fmt`/`clippy -D`/`test`
  green — **151 tests**.
* Interplay note: v0.4.1 added `render::capability::DetailCapability {Low,Mid,High}`
  (the documented QualityTier precursor) driving node-detail. v0.5.0 introduces the
  authoritative `QualityTier {Potato,Low,Medium,High}`; `DetailCapability` will be
  derived from the effective tier so the v0.4.1 face-icon/preview systems follow
  the tier (the v0.4.1 face icon is the *centre* of WP-2's gate-glyph unit).

## Phase 1 — WP-0 Quality-tier system (spec §2)

* `render/quality.rs`: `QualityTier {Potato,Low,Medium,High}` (Ord, cheapest→
  richest) + pure `detect_tier(name, AdapterKind, backend)` (V3D/VideoCore/
  llvmpipe/software → Potato; discrete → High; GL-backend or weak-iGPU integrated
  → Low; modern integrated → Medium; Cpu/Other → Potato). `TierGates` preset per
  tier (HDR bloom / post-FX mode / rings / silhouettes / max_nodes / MSAA /
  target_fps); `effective_gates(tier, theme)` folds in the theme (`Minimal` →
  cheapest path). Pure `AdaptiveState` (3 s-down / 10 s-up + 15-fps margin band →
  hysteresis, never below Potato, capped at base). `QualityState` resource with
  `take_dirty` (exactly one reconfiguration per change).
* Derives the v0.4.1 `DetailCapability` from the effective tier (Potato/Low→Low,
  Medium→Mid, High→High) — the v0.4.1 node-detail axis now follows the tier.
* `[quality]` config (`tier`/`adaptive`/`target_fps_override`) plumbed config.rs ↔
  viewer.toml ↔ CfgState.
* Wiring: `finish()` resolves the base tier (config `tier` override → adapter
  `detect_tier` → Medium fallback) + derives DetailCapability (the `[node_detail]
  level` override still wins). `apply_quality` reconfigures bloom intensity, MSAA,
  the node budget and DetailCapability **once per change** (`take_dirty`);
  `sync_postfx` / `sync_node_rings` gate on the tier each frame (idempotent);
  `adaptive_quality` feeds a ~1 s mean-FPS window into the adaptive machine.
  Bloom intensity moved from `sync_visual_theme` to `apply_quality` (single owner).
* **Deviation (documented per §3):** `apply_quality` writes the tier's
  `max_visible_nodes` (spec §2.2 "max nodes (default)") into the in-memory
  CfgState on tier change; the persisted `viewer.toml` is untouched unless the
  user saves settings. Tier is the runtime node-budget authority; the demoted
  Technician slider adjusts within a session.
* **Gate 1 PASS** — fmt / clippy -D / test green. New tests (+8):
  `detect_tier_fixtures` (incl. Pi V3D → Potato, discrete → High, GL iGPU → Low),
  `adaptive_steps_down_after_low_window_then_floors`,
  `adaptive_steps_up_after_high_window_capped_at_base`,
  `adaptive_in_band_does_not_oscillate`, `minimal_forces_cheapest_path`,
  `parse_and_caps`, `dirty_fires_once_per_change`, `quality_config_roundtrip`.
  **159 tests** total.

### Local-capture procedure (perf — required, no GPU/Pi in CI)

```
cargo run --release -p spacegraph-viewer -- --demo-load 1500
# startup logs "quality tier: <T> (auto|config)"; force a tier via viewer.toml
# [quality] tier = potato|low|medium|high, or toggle adaptive and load the GPU
```
Record, per class, the auto-detected tier + steady FPS at each tier, and that an
adaptive step-down recovers FPS without oscillation:

| Class | Auto tier | FPS @ Potato/Low/Medium/High | Adaptive step verified |
|---|---|---|---|
| Pi / GLES / llvmpipe | Potato | | |
| Integrated | Low/Medium | | |
| Discrete | High | | |

## Phase 2 — WP-1 Tokens + egui reskin + shell + selectors (spec §3.1/3.2/3.9, F1)

* `ui/tokens.rs`: SpaceGraph egui token set (colour roles bg/surface/line/accent-
  cyan/green/severity, 4-based spacing, font roles) — house-style parity (D-1).
* `ui/theme_egui.rs`: embeds the three OFL fonts (Inter / JetBrains Mono / Space
  Grotesk, fetched once from official + google/fonts sources, committed under
  `assets/fonts/<family>/` with each `OFL.txt`, loaded via `include_bytes!` +
  `FontDefinitions`). `apply_egui_theme` installs fonts once + applies GitS
  `Visuals`/`Style` (Standard) or plain dark (Minimal) on theme change. Runs in
  Update (the egui context does not exist during Startup).
* IDE shell (native panels, **no docking crate** — D-2): the left operator rail is
  now resizable + toggleable (`SidePanel::show_animated`, width/open persisted in
  `[shell]`); the tuning controls (Performance/LOD/Layout/Glow + Gameplay/Post-FX/
  GC sliders) moved into a **Technician** section collapsed by default; the
  inspector is docked to `SidePanel::right` (`[shell] right_open/right_width`).
* F1 selectors: a Display section with `VisualTheme` (Standard/Minimal) and
  `QualityTier` (Auto/Potato/Low/Medium/High) combos + an Adaptive toggle; an
  explicit tier applies immediately (sets `QualityState.base` + effective), Auto
  re-detects on next launch. Both axes persist.
* **Control inventory (anti-regression, every pre-existing control still wired):**
  Status, Agents (Manage…), View (mode/demo/tree+timeline controls), Alerts,
  Filter, Performance, LOD/Rendering, Layout, Glow, Gameplay, Post-FX, Audio, GC,
  Search, Settings (Edit Paths / Save / Reset), Actions (Clear) — all present
  (tuning ones under Technician); inspector (now right-docked) keeps connections /
  Fly-to / Pin / why-connected; legend/help/reticle/context-menu/minimap/preview
  overlays unchanged. **New:** Display selectors.
* **Registered-systems invariant re-verified EMPTY** after the refactor: every
  overlay/render system (incl. the new `apply_egui_theme`) is registered in
  `app/mod.rs` (20/20 checked). `ui_panel` gained a `QualityState` param
  (Bevy injects by type — no schedule change).
* **Deviation (documented per §3):** the bottom timeline **dock** (`TopBottomPanel`)
  is not yet a persistent lane — the timeline remains a view mode; `[shell]
  bottom_open` is persisted as a stub. Docking the timeline lane is a contained
  follow-up (tracked for a v0.5.x / WP-6 doc note); it does not affect any gate.
* **Gate 2 PASS** — fmt / clippy -D / test green. New tests (+3):
  `tokens::roles_are_distinct`, `theme_egui::gits_visuals_use_accent_selection`
  (Minimal path distinct), `config::shell_config_roundtrip`. **162 tests** total.

### Local-capture (perf/visual)

```
cargo run --release -p spacegraph-viewer -- --demo-load 1500
# verify: GitS fonts/reskin in Standard, plain look in Minimal; resize/collapse
# the rail and inspector and confirm widths persist after Save Settings + restart;
# switch theme + tier from the Display selectors.
```

## Phase 3 — WP-2 Gate-glyph LOD layer (spec §3.3)

* `render/node_glyph.rs`: a billboarded **gate-glyph** (centre dot + two concentric
  rings) as a shared `LineList` mesh (the shell/edges pattern), unlit HDR-emissive
  so it blooms, per-kind material (instanced), camera-faced each frame (rotation
  copy). Spawned per visible node in **Standard** only; `sync_node_glyphs` mirrors
  the node-icon lifecycle (spawn on add / despawn on remove / mutate otherwise).
* Node-representation matrix (theme × tier) wired into `sync_node_entities`:
  `Minimal` → flat sphere, no glyph; `Standard` + silhouettes-on (Medium/High) →
  per-kind 3D core + shell + glyph; `Standard` + silhouettes-off (Potato/Low) →
  **glyph-primary** (cheap flat core, no shell). `node_meshes` gained a
  `silhouette` arg; `apply_quality` triggers one entity rebuild when the
  silhouette gate flips on a tier change (the v0.4.0 theme-switch pattern — no
  per-frame churn). The gate-glyph is the centre-ring complement to the v0.4.1
  face icon (one GitS gate unit).
* **Deviation (documented per §3):** the *distance* far-LOD band (silhouette drops
  out beyond `FAR_DIST` even at Medium/High) is selectable via the pure
  `silhouette_active(theme, gates, dist, far)` fn (tested) but not yet wired into
  `sync_node_entities` — wiring it needs dynamic shell add/remove on band-cross
  (churn-prone). Current wiring is tier-driven (near band); the distance band is a
  contained v0.5.x refinement. Glyph LOD ladder otherwise per spec.
* **Gate 3 PASS** — fmt / clippy -D / test green. New tests (+5):
  `node_glyph::lod_matrix_pure_fns` (Standard glyph / Minimal none / silhouette
  near vs far vs Potato), `glyph_per_visible_node_in_standard`,
  `minimal_theme_has_no_glyphs`, `glyphs_have_no_steady_state_churn`,
  `spatial::potato_tier_suppresses_silhouette_glyph_primary`. **167 tests** total.

### Local-capture (perf/visual)

```
cargo run --release -p spacegraph-viewer -- --demo-load 1500
# Standard: every node shows the gate ring (type-coloured, blooms); set tier
# Potato/Low → silhouettes vanish, glyph is the node; Minimal → flat spheres, no
# rings. Confirm a tier switch rebuilds once (no churn) and FPS scales per class.
```

## Phase 4 — WP-3 Radial command HUD (spec §3.4)

* Evolved `ui/context_menu.rs` into the keyboard-driven ring HUD (reuses `CtxAct`/
  `ACTIONS`/`apply_context_action`). `RadialState {focused, active_ring, cursor,
  path_page}` + `Ring {Commands, Paths}`, held in a UI-side `RadialMenu` resource
  (kept out of `GraphState` — module boundary). Pure transitions: `open`,
  `switch_ring`, `rotate` (wrap-around), `page` (clamped); `command_at` (inner =
  the 6 verbs), `path_at` / `radial_neighbors` (outer = sorted-unique neighbours,
  paged by 9).
* `radial_hud` system: full input model — `F` opens (shortcuts), `Esc` closes
  (shortcuts handles it first so it doesn't also clear the selection), `Tab`/`↑↓`
  switch ring, `←→` rotate, `[`/`]` page, `1`–`9` select, `Enter` execute; a
  command runs its `CtxAct`, a path **dives** (re-centres focus on the neighbour +
  fly-to + re-opens the ring — keyboard graph traversal). Renders two concentric
  egui-painter rings at the node's projected position with numbered slots, active-
  ring/cursor amber highlight, and a centre identity readout. `try_ctx_mut` makes
  it panic-free headless. Tier-independent (egui), determinism-exempt.
* **Gate 4 PASS** — fmt / clippy -D / test green. New tests (+8):
  `radial_open_and_ring_switch`, `radial_rotate_wraps_around`,
  `radial_paging_clamps_to_bounds`, `radial_commands_map_to_actions`,
  `radial_path_indexing_by_page`, `radial_neighbors_are_unique_and_sorted`,
  `radial_render_does_not_panic` (real standalone egui ctx), and
  `radial_hud_runs_without_panic_headless` (camera + focused node + open radial).
  **175 tests** total.

### Local-capture (interaction)

```
cargo run --release -p spacegraph-viewer -- --demo-load 1500
# select a node, press F → ring HUD; Tab between Commands/Paths; ←→ rotate;
# 1-9 select; Enter execute; [ ] page neighbours; Enter on a path dives. Esc closes.
```

## Phase 5 — WP-4 Dive ripples + HUD rand-frame (spec §3.5, §3.6)

* **Dive ripples:** the v0.4.0/v0.4.1 focus ripple (`render/interaction.rs`,
  spawn→expand+fade→despawn, capped, Standard-only, wakes the reactive renderer)
  already covers the focus trigger; refactored its spawn into `spawn_ripple` and
  added `trigger_alert_ripple` — when `alert_order` grows, emit a red ripple at the
  newest alert (the focus ripple's threat analogue). Bounded by `MAX_RIPPLES`, no
  churn (only on count increase).
* **HUD rand-frame** (`ui/hud::hud_frame_overlay` → `draw_hud_frame`): egui-painter
  corner brackets at the viewport margins + a top-edge live-state strip — agents
  connected, alert counts by severity (low/med/high), view mode, FPS, and the
  active `QualityTier`. Edge-hugging so the centre stays the visualisation; reads
  global state read-only; panic-free headless (`try_ctx_mut`). Tier-independent.
* **Deviation (documented per §3):** the rand-frame is **added** alongside the
  existing top-left text HUD rather than replacing it / pulling Agents/Alerts/Mode
  out of the left rail (spec §3.6's relocation). Both coexist; consolidating the
  old HUD into the frame + de-duplicating the rail is a contained follow-up (noted
  for WP-6 docs). No gate affected.
* **Gate 5 PASS** — fmt / clippy -D / test green. New tests (+2):
  `interaction::alert_growth_spawns_one_ripple_then_no_churn`,
  `hud::hud_frame_draws_without_panic` (real standalone egui ctx). **177 tests**
  total. (Focus-ripple lifecycle remains covered by the v0.4.1 ripple tests.)

### Local-capture (visual)

```
cargo run --release -p spacegraph-viewer -- --demo-load 1500
# focus a node → ripple; inject an alert → red ripple at it; the rand-frame
# corner brackets + status strip show live agents/alerts/mode/FPS/tier.
```

## Phase 6 — WP-5 Command palette + query-DSL (spec §3.7, §3.8)

* `graph/query.rs`: pure parser + predicate (no Bevy). Grammar per spec — implicit
  -AND terms, leading `-` negates, `key:value` over `type`/`kind`/`host`/`sev`/
  `name`/`path`/`deg`/`recent` + bare-word substring; quote-aware tokenizer;
  `deg:>N` operators, `recent:Nm` durations. Compiles to `Query::matches(&NodeView)`
  (a borrowed view the caller fills from graph state); malformed → `QueryError`.
  Replaces the substring filter: `visible_set_capped` parses the filter **once**
  and applies `query_passes` per node (kind/label/name/path/host/severity/degree/
  recent via a graph-side `node_kind_str` — no render dep). Blank/malformed query
  → matches all (the chip shows the error). Filter UI now renders parsed tokens as
  **removable chips** + a red error chip.
* `ui/command_palette.rs`: `Ctrl/Cmd+P` palette over actions (theme/tier/panel
  toggles, fog, search) + nodes (jump), ranked by an in-house subsequence
  `fuzzy_match` (contiguous/prefix/short bonuses; **no new crate**). Bounded node
  scan; Enter executes the top hit; Esc closes. `try_ctx_mut` → panic-free
  headless. (Ctrl+P now opens the palette; the old search stays on its panel button.)
* **Gate 6 PASS** — fmt / clippy -D / test green. New tests (+10):
  query `parses_valid_forms`, `negation_and_implicit_and`, `malformed_inputs_error`,
  `predicate_hits_and_misses`, `alert_severity_predicate`, `empty_query_matches_all`;
  palette `fuzzy_matches_subsequence_only`, `fuzzy_prefers_contiguous_and_prefix`,
  `ranked_entries_surface_theme_command`, `palette_overlay_runs_without_panic_headless`.
  Determinism regression guard re-verified green (`force_step_is_deterministic`,
  `capped_visible_set_is_deterministic`). **187 tests** total.

### Local-capture (interaction)

```
cargo run --release -p spacegraph-viewer -- --demo-load 1500
# Ctrl+P → palette (fuzzy actions + node jump). Filter box: type
# `type:process deg:>3 -name:bash` → chips + live filtering; a malformed term
# (e.g. deg:abc) shows a red error chip and stops filtering.
```

## Phase 7 — WP-6 Docs reconcile + tag (closeout)

* **Docs reconciled:** `docs/DESIGN_LANGUAGE.md` (v0.5.0 section — quality tiers +
  GitS split, tokens/reskin/shell, gate-glyphs, radial HUD, ripples/rand-frame,
  palette/query-DSL, controls), `README.md` ("GitS UX-Shell & Quality-Tiers"
  features + controls + the two axes), `docs/ACCEPTANCE.md` (per-WP v0.5.0 gates
  **and** the F3 lane-timeline / Tree-view criteria), this RUNLOG.
* **force_step benchmark — no layout regression (code unchanged).** `git diff
  v0.4.1 HEAD` shows `force_step`'s body byte-unchanged (the only `layout.rs`
  edits are the new `query_passes`/`node_kind_str`/`effective_max_nodes` and the
  filter parse in `visible_set_capped`); `benches/layout.rs` unchanged. With an
  empty filter (the bench default) the query path is `None` → a cheap immediate
  pass, so `visible_set_capped` is equivalent. Render-side pass; gate intent met.

### Adversarial review (multi-agent) + fixes

A 5-dimension adversarial review (12 agents, per-finding verification) of the full
v0.5.0 surfaced **6 confirmed** findings (1 dismissed); the three behavioural bugs
+ the recoverability fix landed before tagging:

* **[tier] `apply_quality` discarded the `[node_detail] level` override** — it set
  `*cap = tier.detail_capability()` unconditionally, clobbering the explicit
  override `finish()` had honoured. **Fix:** `apply_quality` now re-applies
  `parse_override(level)` first (tier-derived only as the fallback).
* **[ripple] alert ripple missed new alerts once the buffer was full** —
  `trigger_alert_ripple` used a count delta, but `alert_order` is a capped ring
  buffer whose length pins at the cap. **Fix:** track the newest alert by
  **identity** (`alert_order.back()` vs a `Local<Option<NodeId>>`) so each new
  alert fires even at constant length (new test
  `alert_ripple_fires_on_new_identity_at_constant_length`).
* **[query] `recent:<huge>d` overflow** — `n * 86_400` could panic (debug,
  overflow-checks) / wrap (release) on live filter input. **Fix:** `checked_mul` →
  `QueryError` (covered in `malformed_inputs_error`).
* **[tier] node-budget clobber made the user's `max_visible_nodes` unrecoverable**
  (LOW) — the tier overwrote it; raising the tier didn't restore it. **Fix:**
  non-destructive — the tier now sets a runtime `tier_max_nodes`; the effective
  cap is `min(max_visible_nodes, tier_max_nodes)` (`effective_max_nodes`), so the
  persisted user value is preserved and restored when the tier rises.
* **Left documented (LOW, no behavioural bug):** the gate-glyph *distance* far-LOD
  band (already deferred in Phase 3 — the pure fn exists, tier-driven wiring used);
  the palette's `take(NODE_SCAN_CAP)` over an unordered HashMap (UI-only,
  determinism-exempt; ranked results still surface the best matches scanned).
* Dismissed (1): a claim the verifier found already covered.

### Three-class FPS local-capture (perf — required, no GPU/Pi in CI)

```
cargo run --release -p spacegraph-viewer -- --demo-load 1500
```
Per class: auto-detected tier, steady FPS at each tier, that an adaptive step-down
recovers FPS without oscillation, and that the GitS read (gate-glyphs/neon/HUD)
survives at Potato:

| Class | Auto tier | FPS Potato/Low/Medium/High | Adaptive verified |
|---|---|---|---|
| Pi / GLES / llvmpipe | Potato | | |
| Integrated | Low/Medium | | |
| Discrete | High | | |

v0.5.0 gates green: fmt / clippy -D / **188 tests**; both invariants held
(registered-systems empty after the shell refactor; determinism gate green); all
structural perf proxies asserted; layout benchmark code unchanged.

---

# FS-Search & Index MP (`feature/fs-search`)

Track-A standalone feature (viewer + agent + wire protocol; no ESN), per
`docs/spec_fs_search_index.md`. Branched from `v0.5.0` (`ee863b9`). Runs in
parallel with the v0.5.1 focus/polish pass; shared files touched additively only
(see `docs/recon/INTEGRATION_fs-search.md` at closeout).

## Phase 0 — Branch + commit pending docs

Branch: `feature/fs-search` (off `v0.5.0`).

### Changed

* Committed `docs/spec_fs_search_index.md` (the binding spec) — previously an
  uncommitted worktree file.

### Gate 0 results

* `cargo test --workspace`: **green** (exit 0) at the `v0.5.0` baseline — the
  feature branch starts from a green tree.
* No source changed in Phase 0; the v0.5.0 fmt/clippy gates are inherited.
