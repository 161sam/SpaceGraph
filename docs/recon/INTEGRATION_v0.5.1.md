# Integration Report — v0.5.1 "GitS Focus & Polish" (Track-A run)

For the operator to integrate the two parallel v0.5.1 runs **serially**. This is
the **Focus & Polish** run; the other is **FS-Search** (agent + protocol + search
UI). This run is **viewer-local** — no ESN, no protocol change.

## Branch

- **Feature branch:** `feature/v0.5.1-focus-polish`
- **Branched from:** `main` tip / `v0.5.0` (`ee863b9`)
- **HEAD at report time:** `5405e61` (Phase 5 merge; this report adds one commit on
  top, then its own `--no-ff` merge).
- **Phase merges** (`--no-ff`, one per phase):
  - Phase 1 — face-icon + ScrollArea bugfixes (`d4b19df`)
  - Phase 2 — gate-ring + radial-HUD polish (`44caeaf`)
  - Phase 3 — edge-perf pass (LOD + focus cull) (`39e034a`)
  - Phase 4 — Focus Mode, the headline (`b002973`)
  - Phase 5 — docs reconcile (`5405e61`)

## NOT done (by design — operator integrates)

- ❌ **Not merged to `main`.**
- ❌ **Not pushed.**
- ❌ **Not tagged** (`git tag --points-at HEAD` → empty).
- The pending uncommitted `docs/spec_fs_search_index.md` (FS-Search's spec) was
  **left untouched** — it does not exist in this worktree; not committed here.

## Shared files touched (serial-merge points)

The other run also edits these. All edits here are **additive** (appended keys /
appended doc sections; nothing reorganised) so serial-merge conflicts are trivial.

### `crates/spacegraph-viewer/src/util/config.rs`

Two new **nested config structs** + two new `ViewerConfig` fields (additive; the
`toml` serializer groups scalars before tables, so these serialise as their own
`[edge_lod]` / `[focus]` tables — no impact on existing keys):

- `EdgeLodConfig` → `ViewerConfig.edge_lod` (after `quality`):
  - new `viewer.toml` block **`[edge_lod]`**: `near_dist=70.0`, `far_dist=160.0`,
    `far_dim=0.35`, `focus_cull=true`
- `FocusConfig` → `ViewerConfig.focus` (after `edge_lod`):
  - new `viewer.toml` block **`[focus]`**: `dim=0.62`, `dof=false` (High-tier DoF
    deferred), `freeze_layout=true`
- Added the two structs' `Default` impls + the two fields in `ViewerConfig::default`.
- Added tests `edge_lod_config_roundtrip`, `focus_config_roundtrip`.

### `viewer.toml` (generated at runtime, not a committed file)

New persisted keys only — the two blocks above (`[edge_lod]`, `[focus]`). Existing
keys unchanged. A pre-v0.5.1 `viewer.toml` loads fine (serde defaults fill the new
blocks).

### Docs (all appended sections, no rewrites)

- `docs/DESIGN_LANGUAGE.md` — new `## v0.5.1 — Focus Mode, gate-ring polish,
  edge-LOD` section (Focus Mode, gate-ring polish, edge-LOD, face-icon fix, v0.5.1
  controls), appended after `### Controls (v0.5.0)`.
- `README.md` — appended v0.5.1 bullets (Focus Mode, edge-LOD, gate-ring polish)
  inside the existing GitS feature list, after the Query-DSL bullet.
- `docs/ACCEPTANCE.md` — new `### v0.5.1 — GitS Focus & Polish` section, appended
  after the F3 section, before `## Definition „Release-fähig“`.
- `docs/perf/RUNLOG.md` — new `## v0.5.1` section (Phase 0–5 entries + closeout +
  the 3-class FPS local-capture table), appended at end-of-file.

## Other internal files touched (not in the shared list, but flag for serial merge)

If FS-Search also edits `graph/state.rs` (e.g. search state on `UiState`), these
are the v0.5.1 additions to watch:

- `crates/spacegraph-viewer/src/graph/state.rs`
  - `UiState` gained `focus_mode: Option<NodeId>` (Focus Mode subject) — added to
    the struct **and** its exhaustive initialiser in `GraphState::default`.
  - `CfgState` gained `edge_lod: EdgeLodConfig` + `focus: FocusConfig` — struct,
    `Default`, and both mapping points (`apply_viewer_config`, `viewer_config`).
  - Config import line extended (`EdgeLodConfig`, `FocusConfig`).
- `crates/spacegraph-viewer/src/graph/layout.rs` — `update_layout_or_timeline`
  gained a freeze guard (`if !st.layout_frozen()`); new `GraphState::layout_frozen`.
  **`force_step` is byte-identical to v0.5.0** (verified: function extracted from
  both revisions → IDENTICAL, 194 lines).
- `crates/spacegraph-viewer/src/app/mod.rs` — registered 3 systems
  (`focus_overlay`, `focus_double_click`, `focus_mode_camera`) + inserted the
  `FocusCam` resource. (Focus systems in their own `add_systems` group to stay under
  the 20-tuple limit.)
- `crates/spacegraph-viewer/src/ui/mod.rs`, `render/mod.rs` — re-exports for the new
  focus systems / `FocusCam`.

## New files

- `crates/spacegraph-viewer/src/ui/focus.rs` — Focus Mode orchestration
  (`enter_focus`, `exit_focus`, `focus_double_click`, `focus_overlay` + the
  centerpiece arcs). Registered in `ui/mod.rs` + `app/mod.rs`.

## Other (non-shared) viewer files changed

`render/{node_icon,node_glyph,edges,camera,mod}.rs`,
`ui/{context_menu,node_preview,settings_paths,shortcuts}.rs` — see the per-phase
RUNLOG entries. Net: **19 files, +1244/−42**; **+1 new file**.

## Gate status

fmt clean · clippy `-D warnings` clean · `cargo test --workspace` **208 tests**
(baseline 188 + 20) · determinism gate green · `force_step` byte-unchanged ·
registered-systems intact (no orphaned system) · structural perf proxies asserted ·
layout benchmark code unchanged.

**Deferred (documented, not blockers):** High-tier DoF blur (dim-on-all-tiers ships;
§1.4 fallback); the 3-class FPS capture numbers (no GPU/Pi in this env — procedure +
expected direction in RUNLOG).
