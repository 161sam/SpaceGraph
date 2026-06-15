# RUNLOG — MP-UI-GitS-polish

**Branch:** `feat/ui-gits-polish` (worktree, from `main` == `8c62f03`, the merged
MP-UI-GitS overhaul). **Mode:** autonomous execution on a branch, **review-gated —
NOT auto-merged.** Visual quality is judged by Sam on screenshots (every phase ships a
before/after set here). Phases are bugs/reverts-first then polish, **each independently
mergeable**; Sam may merge incrementally.

**Scope guard (audited every commit):** Track A (viewer-only) — no `spacegraph-core`/
`spacegraph-graph`/agent change, no wire bump (stays v4), nothing offensive/mutating,
behaviour-preserving (every control still reachable), Minimal theme parity preserved,
**FPS not regressed** (the separate FPS issue is the perf MP's job; animations stay
focus-only / visible-only). No `unwrap`/`expect` in render/ui paths. No AI-authorship
markers. Archive-not-delete (the reverted `focus_core` → `legacy/`).

**Baseline (`8c62f03`):** `cargo fmt --check` ✓ · `clippy --workspace --all-targets
-D warnings` ✓ (0 warnings) · `cargo test --workspace` ✓ **301 passed / 0 failed**
(196 viewer).

## Phase status
- [x] **P0** — Inventory + before-shots
- [x] **P1** — Revert 3D core + clean focus treatment + segmented action ring
- [ ] **P2** — Complete panel-layer + middle-ellipsis
- [ ] **P3** — Minimap: make it real
- [ ] **P4** — Edges (curved / per-class / falloff / flow)
- [ ] **P5** — Nodes + colour semantics + label anti-overlap
- [ ] **P6** — Layout spread (de-hairball)
- [ ] **P7** — Entity-card detail + chrome consistency + close-out
- [ ] **Sam's screenshot review** → merge (review-gated)

---

## P0 — Inventory + before-shots

### Inventory
Full code-level module map + per-phase change targets (file:line) + risk map produced
by an 8-reader parallel sweep over the current (post-overhaul) viewer, synthesised to
**`docs/recon/P0_POLISH_INVENTORY.md`** — the actionable build plan for P1–P7. Every
load-bearing claim cites `crates/spacegraph-viewer/src/...:line`.

### Bug/gap repro (confirmed by code + the before-shots)
Captured offline from `--demo-load 2000` (Standard, `XDG_CONFIG_HOME`-isolated theme;
focus via the gated `SPACEGRAPH_DEMO_FOCUS` hub-autofocus). Capture env renders
software-GL → `TIER POTATO` / low FPS; that is a capture artifact, not the target GPU.

- **`before-polish-focus.png`** reproduces, in one frame: the **radial floating-label
  jumble** (inner command labels `1 Fly-to`/`2 Isolate subgraph`/… overlapping the
  outer neighbour-path labels `synthetic/bin/pro` concentrically over the node —
  context_menu.rs:385-416), the **telemetry/preview bottom-left collision**
  (`◢ TELEMETRY` and `◈ PREVIEW` stacked — hud.rs:86 vs node_preview.rs:483), **path
  overflow** (`…/file000322.dat (synthe` — no middle-ellipsis), the **3D focus core**
  rings on the node (focus_core.rs), and the **flat entity card** that *duplicates* the
  radial's Isolate/Trace/Mark actions (entity_card.rs:93-115).
- **`before-polish-default.png`** reproduces: **flat uniform edges** (straight, ~1px,
  single-colour, edge-spaghetti — edges.rs:265-275), near-**monochrome** node field
  (cyan/green dominate; per-type palette off-spec — theme.rs:16-26), the **crude
  minimap** (rubber-banding green scatter — minimap.rs:44-65), and the **verbose
  telemetry dump** (hud.rs:99-164).
- `before-polish-minimal.png` / `before-polish-minimal-focus.png` = the Minimal-theme
  baseline (parity must survive every phase).

### Decisions recorded (MP Stop-and-Show items resolvable from the mockup + MP text)
- **Telemetry vs preview (both want bottom-left).** Per the mockup: telemetry owns
  **bottom-left alone**; in Focus Mode there is **no separate preview panel** — the
  **entity card** (right) is the sole focus-detail surface and absorbs the preview
  content; outside focus the hover-peek preview docks in the **right column under the
  minimap**, never bottom-left. (P2 removes the focus-mode `LEFT_BOTTOM` preview
  special-case — node_preview.rs:481-485.)
- **Card actions vs the ring.** The card keeps a **primary** action row (Fly-to + a
  pin/compare); Inspect/Trace/Isolate/Mark live on the **segmented ring** — no
  duplication (resolves the entity_card.rs:93-115 vs ring overlap).
- **Ring order** follows the MP/mockup exactly: **fly-to · inspect · trace · isolate ·
  mark · pin** (reorders `ACTIONS`; the one order-dependent test is updated).
- **Edge thickness / default visibility (P4, decide at build time).** Default to
  viewer-side styling that needs **no topology change and no FPS regression**:
  weight-as-HDR-brightness (+ optional parallel strands) rather than camera-facing quad
  strips; keep within the `EdgeFingerprint`/cam-cell "settled→cheap" gate. True
  geometric thickness (quad strips) is a render-architecture change — deferred / surface
  before adopting. Edge-default-visibility flip (`lod_edges_mode` FocusOnly→All)
  gated on confirming the perf budget.

### P0 gate
Module map + bug repro recorded ✓ · before-shots captured ✓ · no code changed (the
worktree is clean `main`). Baseline fmt/clippy/test recorded at this commit.

---

## P1 — Revert 3D core + clean focus treatment + segmented action ring

### What changed
- **Reverted the P5 3D focus core.** `render/focus_core.rs` → `legacy/render/` (kept,
  not deleted, with an archive header pointing at the re-wire steps). Unwired the
  three call sites (`app/mod.rs` Startup + focus-Update + chained render tuple) and the
  `render/mod.rs` mod + re-export; removed the now-dead `FOCUS_CORE_*` colours
  (`render/theme.rs`). The focused node's **own per-type silhouette** (its 3D mesh) is
  now the centrepiece — no occluding glow rig.
- **Clean focus treatment** (`ui/focus.rs`, Standard): the reticle **corner brackets**
  (`ui/reticle.rs`, unchanged) frame the node; `focus_overlay` adds exactly **one** thin
  indicator ring hugging it + a `◀ FOCUS ▶` tag with a `kind · 0xHEXID` subtitle. The
  old centrepiece's links/identity arcs are **gone** (they live in the entity card).
  New `util::ids::short_hex_id` (stable FNV chip, unit-tested) feeds the subtitle.
- **Segmented radial action ring** (`ui/context_menu.rs::render_radial`): the floating
  command/neighbour text rings are replaced by **6 arc-segment wedges** evenly at 60°
  (gapped dividers), numbered, the keyboard-cursor **or pointer-hovered** wedge
  highlighted brighter — over a faint inner tick gauge. `ACTIONS` reordered to the
  MP/mockup order **fly-to · inspect · trace · isolate · mark · pin**. The neighbour
  (Paths) ring degrades to faint positional ticks — **names no longer float over the
  node** (they go to the entity card). Pure, unit-tested geometry:
  `segment_center_angle` (even 60° from the top) + `segment_at` (hover/hit-test, misses
  holes + divider gaps). Keyboard model (`command_at`/input handler/dive) untouched.
- **Focus de-clutter** (`render/spatial.rs::draw_node_labels`): the focus-mode subject's
  billboard path label is suppressed (the FOCUS subtitle + entity card name it) so the
  long path no longer floats across the node.
- **Minimal** unchanged: the radial only opens in Standard and the indicator
  ring/tag/centrepiece are Standard-gated — Minimal stays a plain dim + gizmo bubble +
  flat card (verified in `afterp1-polish-minimal-focus.png`).

### Decisions / deferrals
- Entity-card **action de-duplication** (the card still shows Isolate/Trace/Mark, now
  also on the ring) is deferred to **P7**'s full card rewrite — trimming three buttons
  only to rewrite the whole card in P7 is wasted churn; they still function meanwhile.
  Per the recorded split, P7's card keeps a **primary** row (Fly-to + Pin) and the rest
  stay on the ring.

### Gate
- `fmt --check` ✓ · `clippy --workspace --all-targets -D warnings` ✓ (0 warnings) ·
  `test --workspace` ✓ **304 passed / 0 failed** (199 viewer; +3: `short_hex_id`,
  `segment_centers_are_evenly_spaced_from_the_top`,
  `segment_at_hits_centres_and_misses_holes_and_gaps`; `radial_commands_map_to_actions`
  updated for the new order). `build -p spacegraph-viewer` ✓.
- Scope audit: viewer-only (`render/{focus_core→legacy, mod, theme, spatial}`,
  `ui/{context_menu, focus}`, `util/ids`, `app/mod`). No core/graph/agent change · no
  wire bump · nothing offensive · Minimal parity preserved · no `unwrap`/`expect` added.

### Screenshots (`docs/media/gits/`)
`afterp1-polish-focus.png` (segmented ring + clean node treatment + FOCUS tag) vs
`before-polish-focus.png` (the 3D-core + floating-label jumble);
`afterp1-polish-minimal-focus.png` (flat degrade); default/hover/minimal for parity.

