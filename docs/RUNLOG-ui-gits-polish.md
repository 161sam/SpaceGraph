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
- [x] **P2** — Complete panel-layer + middle-ellipsis
- [x] **P3** — Minimap: make it real
- [x] **P4** — Edges (curved / per-class / falloff / flow)
- [x] **P5** — Nodes + colour semantics + label anti-overlap
- [x] **P6** — Layout spread (de-hairball)
- [x] **P7** — Entity-card detail + chrome consistency + close-out
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

---

## P2 — Complete panel-layer + middle-ellipsis

### What changed
- **`content_rect` is now the full layout authority** (`ui/rail.rs::update_ui_layout`):
  screen minus the rail (left), the top status strip, **and the docked inspector
  column** (right, when it will render — new pure `inspector_reserves`, tested). Every
  floating panel constrains to it instead of a bare screen corner.
- **Telemetry/preview collision fixed** (`ui/node_preview.rs`): the preview is
  **suppressed in Focus Mode** (the entity card is the sole focus-detail surface, per
  the mockup) — so telemetry owns bottom-left alone. Outside focus it docks
  `RIGHT_BOTTOM` **constrained to `content_rect`** (clears the inspector). The old
  focus-mode `LEFT_BOTTOM` special-case is gone.
- **Right-column panels are content_rect-aware**: the **minimap** (`ui/minimap.rs`) now
  positions via the tested `overlay::corner_anchor(RIGHT_TOP)` + `order(layer::PANEL)`;
  the **entity card** (`ui/entity_card.rs`) gained `.constrain_to(content_rect)`; the
  **context menu** (`ui/context_menu.rs`) is clamped to `content_rect` so an edge click
  doesn't push it off-screen.
- **Middle-ellipsis truncation everywhere** (new pure `overlay::middle_truncate`, char-
  boundary safe, tail-favoured so the basename survives — unit-tested): applied to the
  **entity-card fields**, the **hover tooltip** (`ui/tooltips.rs`, now width-capped +
  full-on-hover), and the **inspector** title + **connection rows** (was right-truncate,
  dropping the basename). Full value always on hover.
- New pure `overlay::corner_anchor` (content_rect-aware corner placement) underpins the
  zone model; the placement assertions (`corner_panels_do_not_overlap`,
  `corner_anchor_clears_a_reserved_inspector_column`) prove minimap/card/telemetry zones
  stay disjoint and the right column shifts left to clear the inspector.

### Decisions recorded
- **Telemetry vs preview** (the MP's flagged Stop-and-Show): resolved per the mockup —
  **no preview panel in Focus Mode** (the entity card carries the detail); telemetry
  owns bottom-left; the hover-peek preview stays bottom-right outside focus.

### Gate
- `fmt --check` ✓ · `clippy --workspace --all-targets -D warnings` ✓ ·
  `test --workspace` ✓ **203 viewer** (+4: `middle_truncate_keeps_head_tail_and_respects_max`,
  `corner_panels_do_not_overlap`, `corner_anchor_clears_a_reserved_inspector_column`,
  `inspector_reserves_only_when_visible`). `build` ✓.
- Scope: viewer-only (`ui/{overlay,rail,minimap,node_preview,entity_card,tooltips,
  inspector,context_menu}`). No core/graph/agent/wire change · Minimal parity preserved.

### Screenshots
`afterp2-polish-focus.png` — telemetry alone bottom-left (preview gone), entity-card
path middle-ellipsis'd `/synthetic/dir005/fi…/file000322.dat`, no panel overlaps — vs
`afterp1-polish-focus.png` (preview still colliding bottom-left). Default/minimal parity.

---

## P3 — Minimap: make it real

### What changed (`ui/minimap.rs`, rewritten)
- **Accurate, stable positions**: dots are the **real projected** node XZ (was already
  type-coloured via `NodeKind::base_color`), now over **padded, squared, full-set
  bounds** (pure `minimap_bounds`) so the radar no longer rubber-bands as nodes move or
  fog toggles. Projection is the pure, unit-tested `minimap_project` / `minimap_unproject`
  (roundtrip-tested).
- **Viewport frustum**: the camera's 4 viewport corners are ray-cast onto the ground
  plane `Y=0` (`ground_hit`, reusing the picking ray pattern) and drawn as a cyan quad —
  it tracks the camera live. A near-horizontal view (a corner ray that misses the plane)
  falls back to a camera→pivot heading line.
- **Focus marker**: the focused / selected node is marked with a distinct cyan ring +
  crosshair at its projected position; a white ring marks the camera.
- **Click-to-fly**: the painter now `Sense::click()`s; a click maps back through
  `minimap_unproject` to a world XZ and drives the camera via a new **position-keyed
  jump** — `GraphState::request_jump_pos` → `ui.jump_to_pos` → `apply_jump_to`
  (`render/camera.rs`) eases the orbit pivot there (no selection change, Spatial only).
  Distinct from the NodeId `request_jump`.
- **Chrome**: a `● LIVE` pill, corner brackets (Standard), a world-span scale hint
  (`⟷ N`), `order(layer::PANEL)`, and an opaque radar background.

### Known interaction (accepted, not chased)
In the **default** view the bright neon scene edges' **bloom glow composites over the
egui minimap** (the postfx/bloom pass runs over the egui layer) — it bleeds even at full
opacity, so it is not fixable by an overlay tweak (it would need a render-graph change —
out of scope per the MP Stop-and-Show). The radar stays readable; in Focus Mode (edges
culled) it is clean — see `afterp3-polish-focus.png`.

### Gate
- `fmt --check` ✓ · `clippy --workspace --all-targets -D warnings` ✓ ·
  `test --workspace` ✓ **205 viewer** (+2: `bounds_are_square_padded_and_stable`,
  `project_unproject_roundtrip`). `build` ✓.
- Scope: viewer-only (`ui/minimap`, `graph/state` viewer UI field `jump_to_pos`,
  `render/camera`). No `spacegraph-graph`/core/agent/wire change · Minimal parity (LIVE
  pill/brackets are Standard-gated; dots/frustum/markers/click work in both).

### Screenshots
`afterp3-polish-default.png` (cropped: type-coloured dots at real positions + cyan
viewport frustum + camera marker + LIVE + scale) and `afterp3-polish-focus.png` (focus
marker crosshair on the radar) vs the crude scatter in `before-polish-default.png`.

---

## P4 — Edges (curved / per-class / falloff / weight / threat)

### What changed (`render/edges.rs::update_edge_mesh`)
- **Curved** — each aggregated edge is now an 8-segment quadratic **bézier** (was a
  straight 2-vertex segment) bowed perpendicular to the chord by a stable per-edge hash
  (`edge_bow`), so **parallel edges fan** apart and long edges arc — the straight-line
  hairball reads far better (see the before/after crop).
- **Continuous opacity falloff** — `edge_falloff` replaces the binary `Dim`/`Full` with
  a smoothstep from full at `near` down to `far_dim` across `near..far`, then cull —
  distant/peripheral edges fade out smoothly instead of popping.
- **Directional gradient** — brighter at `from`, fading toward `to`, so edge direction
  reads.
- **Weight → brightness** — `weight_brightness(agg.stats.count)` (log-normalised) makes
  heavier aggregated edges read brighter — the LineList "thickness" proxy.
- **Threat edges** — an edge of class `AlertsOn` or touching an `Alert` node renders the
  red `theme::ALERT`, boosted, with a static dash pattern (the alerted-node set is
  precomputed once per build). Per-class colours otherwise unchanged (`theme::edge_color`).
- Pure + unit-tested: `edge_falloff`, `edge_bow`, `weight_brightness`.

### Decisions (Stop-and-Show scope)
- **Thickness** is encoded as **HDR brightness**, not geometric width — a 1px `LineList`
  can't vary width; true thickness needs camera-facing **quad strips** (a topology /
  render-architecture change), deferred per the MP guard.
- **Animated flow** would need a per-frame mesh rebuild, which **defeats the
  `EdgeFingerprint`/cam-cell "settled→cheap" perf gate** (an FPS regression — the perf
  MP's domain), or a time-uniform shader (render-architecture). So threat edges get a
  **static** dash (the "special" read) and animated flow is deferred. No new per-frame
  cost; the rebuild gate is intact.
- The demo graph has **0 alerts**, so the red/threat styling isn't visible in the shots
  (it's for real alert data); the curve/gradient/falloff/weight ARE visible by default
  (the demo's `lod_active` is false → `edges_mode = All`).

### Gate
- `fmt --check` ✓ · `clippy --workspace --all-targets -D warnings` ✓ ·
  `test --workspace` ✓ **208 viewer** (+3: `edge_falloff_smooth_then_cull`,
  `weight_brightness_is_monotone_and_clamped`, `edge_bow_is_deterministic…`). `build` ✓.
- Scope: viewer-only (`render/edges.rs`). No `spacegraph-core`/`spacegraph-graph` change
  (styling is viewer-side over existing edge data) · Minimal degrades (the `×2.5` HDR
  boost is Standard-only; Minimal keeps flat per-class lines).

### Screenshots
`afterp4-polish-default.png` (curved, fanned, gradient'd web) vs `before-polish-default.png`
(straight spaghetti) — cropped comparison confirms the curve.

---

## P5 — Nodes + colour semantics + label anti-overlap

### What changed
- **Palette retuned to the exact MP hexes** (`render/theme.rs`): Process `#2bb3a8`, File
  `#6fe06f`, User `#f5b942`, Socket `#5fa8ff`, RemoteHost `#b09bfb`, Alert `#ff5d5d`
  (focus reserves the brighter `#34d6c8`). Edge-class colours **aligned to the node
  palette** (`EDGE_OPENS = FILE`, `EDGE_EXECS = PROCESS`, …) so an edge reads as its
  endpoint kind; `connects_to`/`listens_on` keep distinct network hues.
- **Per-type silhouette is always-on in Standard** (`render/spatial.rs::node_meshes`):
  the per-kind **core** mesh (Process octahedron · File hex · User cone · Socket torus ·
  RemoteHost/Alert sphere) now renders on **every tier** — it's a single mesh, no
  costlier than the sphere it replaced — so **type reads from shape by default**, even
  at Potato (the capture tier). Only the wireframe **shell** (extra LineList geometry)
  stays tier-gated off at Potato/Low. On Medium+/HDR the shapes bloom and read hardest.
- **Label anti-overlap** (`ui/overlay.rs::decollide_labels`, pure + unit-tested): the
  capped (≤6) hovered/selected/focus billboard labels are de-collided — each keeps its
  anchor unless it would overlap an already-placed label, then it nudges straight down
  until clear. Wired into `render/spatial.rs::draw_node_labels`.

### State/severity/exposure (already encoded by default — no Stop-and-Show)
The existing vocabulary is already visible without detection and needs no new data:
alert nodes ramp by **severity** (`alert_severity_color`), sockets encode **exposure**
as radial shell depth (D0 `exposure_bucket`) + **port-state aperture**, and red-team
origin styles distinctly. P5 surfaces type via shape+colour; it adds no data source.

### Gate
- `fmt --check` ✓ · `clippy --workspace --all-targets -D warnings` ✓ ·
  `test --workspace` ✓ **209 viewer** (+1 `decollide_labels…`; `potato_tier_…` test
  retargeted to assert the always-on core + still-gated shell). `build` ✓.
- Scope: viewer-only (`render/theme`, `render/spatial`, `ui/overlay`). No
  core/graph/agent/wire change · Minimal keeps the flat sphere (parity verified by
  `minimal_theme_uses_sphere_mesh_and_no_shell`).

### Screenshots
`afterp5-polish-default.png` — type-distinct framed silhouettes + icons + the MP
palette + de-collided labels — vs the uniform disc field in `before-polish-default.png`.

---

## P6 — Layout spread (de-hairball)

### What changed (`graph/layout.rs`, `graph/state.rs`, `ui/hud_panels.rs`)
- **Wider repulsion reach** to de-clump: `repulsion_radius` 8 → **14** (link_distance
  6 → 7, so ~2× link). Because `scatter_position` scales its spacing with
  `repulsion_radius`, the initial cloud grows with the reach → density stays **~1 node
  per grid cell**, so the candidate count per node stays bounded (no `layout_budget_ms`
  blow-up) even as the graph spreads. The short cutoff was the contraction cause — a
  wider reach lets repulsion resist the spring net across a clump.
- **Degree-aware mass** (`node_mass`, pure + unit-tested): hubs (high degree) get a
  log-scaled heavier integration mass so they **accelerate less and anchor**, while
  leaves fan out around them — the **hubs-vs-leaves hierarchy**. The per-slot degree is
  built only while the layout is unsettled (it returns early at rest), so there's **no
  steady-state cost**.
- **Slider fixes** (`ui/hud_panels.rs`): the repulsion slider range was `0..=120` —
  it couldn't even reach the `400` default (silently clamping user edits); widened to
  `0..=1000`, and a new **`spread (radius)`** slider (`4..=40`) exposes the de-clump
  knob.

### Convergence / determinism (the hard contract — all green)
`force_layout_settles_freezes_and_wakes` ✓ (still converges + freezes + wakes with the
new reach + mass), `force_step_is_deterministic` ✓, `force_step_keeps_pinned_fixed…` ✓,
`budget_split_matches_full_step` ✓. The mass is a pure function of model degree (no
RNG/clock/HashSet-order), so determinism holds. `SETTLE_EPS`/`SETTLE_FRAMES` unchanged —
the more-spread equilibrium has smaller residual forces, so it settles within budget.

### Honest scope note
The `--demo-load 2000` graph (≤400 visible) was **not** a *tight* central hairball to
begin with, so the visible change is **moderate** — a more spread field with a clearer
central hub anchor. The mechanism (wider bounded-density reach + hub-anchoring) pays off
most on **denser / real** graphs. `cfg.radius` / `cfg.y_spread` remain reserved knobs
(not wired this pass — out of P6's converge/spread gate).

### Gate
- `fmt --check` ✓ · `clippy --workspace --all-targets -D warnings` ✓ ·
  `test --workspace` ✓ **210 viewer** (+1 `node_mass_anchors_hubs_sublinearly`). `build` ✓.
- Scope: viewer-only (`graph/layout`, `graph/state` cfg defaults, `ui/hud_panels`). No
  `spacegraph-graph`/core/agent/wire change · FPS: layout still settles + idles (the
  degree array is unsettled-only) — **no steady-state regression**.

### Screenshots
`afterp6-polish-default.png` (more spread + central hub anchor) vs
`afterp5-polish-default.png` / `before-polish-default.png`.

---

## P7 — Entity-card detail + rail icons + chrome consistency + close-out

### What changed
- **Tokens reconciled to the MP palette** (`ui/tokens.rs`): BG `#05090c`, SURFACE
  `#08171c`, LINE `#1d4a4c`, ACCENT `#2bb3a8` + ACCENT_HI `#34d6c8`, TEXT `#cfe9e5` /
  TEXT_DIM `#88b8b2`, and **per-type accents** FILE/PROCESS/SOCKET/USER/REMOTEHOST +
  the alert ramp — mirroring `render::theme` so chrome accents match the 3D node
  palette (one palette, two surfaces). `roles_are_distinct` extended.
- **Entity card → three blocks** (`ui/entity_card.rs`, rewritten): a header (type
  silhouette + `◢ ENTITY · kind` + hex-id chip + live dot), **◢ IDENTITY** (the
  per-kind fields, middle-ellipsis'd), **◢ STATE** (origin · degree + a **segmented
  meter** · alert severity chip), and **◢ CONNECTIONS · N** (the de-duped,
  per-class-coloured, **clickable** neighbour list — click re-centres focus — + "+N
  more"). The header/silhouette/accents are **type-coloured**. Actions trimmed to the
  **primary** row (Fly-to + Pin); Isolate/Trace/Mark now live **only on the ring** —
  this resolves the P1-deferred card/ring action **duplication**.
- **Rail real icons** (`ui/rail.rs`): the monospace glyph placeholders are replaced
  with **painter-drawn vector icons** (eye · funnel · warning-triangle · hexagon ·
  gear), an **active left-accent bar** + fill, and a **severity-coloured badge pill**
  (separate from the icon tint) for the alert/agent counts.
- **Tidy telemetry** (`ui/hud.rs`): the 8-line debug dump → a **2-line readout**
  (`FPS n · Nms · mode` / `Nn · Ne · flow`), keeping the `◢ TELEMETRY` header; the
  verbose per-stream breakdown is no longer dumped.
- **GitS "screen" chrome** (`ui/gits.rs`): `bracket_response` now also paints a faint
  **scanline** sheen (new `alpha::SCANLINE`) so the entity card / HUD panels / legend
  read as lit screens (Standard only; Minimal stays flat).
- Legend swatches read `render::theme` (retuned in P5) so legend/card/rail/3D stay in
  lock-step on the one palette.

### Gate
- `fmt --check` ✓ · `clippy --workspace --all-targets -D warnings` ✓ ·
  `test --workspace` ✓ **210 viewer**. `build` ✓.
- Scope: viewer-only (`ui/{tokens,entity_card,rail,hud,gits}`). No core/graph/agent/
  wire change · Minimal degrades (vector icons + card render flat; scanline/brackets/
  per-type accents are Standard-gated).

---

## Close-out — for Sam's screenshot review

**Branch `feat/ui-gits-polish` (worktree `/home/dev/sg-ui-polish`) is ready for review —
NOT auto-merged.** Commits: `7f56ca1` P0 · `274dc49` P1 · `e938dc1` P2 · `8ff0b1f` P3 ·
`409a7d9` P4 · `9a50148` P5 · `2ad23cf` P6 · P7 (this).

### Before / after screenshot set (`docs/media/gits/`, prefix `before-polish-*` → `afterp7-polish-*`)
| State | Before | After |
|---|---|---|
| Default chrome | `before-polish-default.png` | `afterp7-polish-default.png` |
| Focus mode | `before-polish-focus.png` (3D-core + floating-label jumble) | `afterp7-polish-focus.png` (segmented ring · clean node · 3-block card · tidy telemetry · real rail) |
| Hover | `before-polish-hover.png` | `afterp7-polish-hover.png` |
| Minimal | `before-polish-minimal.png` | `afterp7-polish-minimal.png` |
| Minimal focus | `before-polish-minimal-focus.png` | `afterp7-polish-minimal-focus.png` |
| Per-phase | — | `afterp{1..6}-polish-*.png` (incremental) + minimap/edge/node crops |

> Captured offline on `DISPLAY :0` from `--demo-load 2000` (Standard, isolated
> `XDG_CONFIG_HOME` theme; focus via the gated `SPACEGRAPH_DEMO_FOCUS` hub-autofocus).
> The capture env renders software-GL → `TIER POTATO`/low FPS — a capture artifact, not
> the target GPU (where HDR/bloom make the silhouettes, 3D depth and neon read harder).

### Reviewer notes / deferred (all noted, none blocking)
- **Edge thickness** = HDR brightness, not geometric width (LineList limit); true
  width = quad-strip topology, deferred. **Animated edge flow** deferred (a per-frame
  rebuild would break the settled→cheap gate / needs a time-uniform shader) — threat
  edges get a static dash. Demo has 0 alerts so threat styling isn't in the shots.
- **Minimap bloom bleed** in the default view = the postfx/bloom pass compositing over
  the egui layer (a render-graph interaction, not an overlay tweak) — accepted; the
  radar is clean in Focus Mode (edges culled).
- **Layout spread** is moderate on the demo (it wasn't a tight hairball); the mechanism
  (wider bounded-density reach + hub-anchoring) pays off on denser/real graphs.
- `cfg.radius`/`cfg.y_spread` left as reserved knobs (not wired).

### Post-build adversarial review (`24f2c02`)
A 5-dimension adversarial review of the branch diff (each finding independently
verified) surfaced **2 confirmed-real bugs** (both invisible to the static demo + the
test suite); both fixed pre-review:
1. **Stale edge mesh** — `EdgeFingerprint` didn't track the P4 threat/weight inputs, so
   an alert/weight change while the layout was settled could leave edge colours stale.
   Fixed by adding a `data_version` (`perf.event_total`) to the fingerprint — O(1), 0
   for the static demo, stable when idle (keeps the settled→cheap gate).
2. **Off-screen labels** — `decollide_labels` could nudge a label stack off the bottom
   of the viewport. Fixed by clamping each label on-screen (mirrors `place_card`) +
   threading the viewport through the call site + a new on-screen unit test.
Re-gate green: fmt/clippy/`test --workspace` (**316 tests**).

---

## Followup — render-vs-claim audit + the real gaps (MP-UI-GitS-polish-followup)

A second review reported the rendered result didn't match the mockup (old radial, big
focus sphere, flat card, panel overlap, central-cluster layout). **Root cause of the
"green tests ≠ right pixels" gap: tier detection, not missing code.** This box has a
real but weak GPU (**Intel HD 520, Vulkan**); `detect_tier` auto-classifies it as
**POTATO** and `adaptive_quality` steps it down further, so the prior `afterp*` captures
ran at TIER POTATO — which gates **off** HDR bloom / post-FX / orbital rings / shells.
Forcing `[quality] tier="high"` **+ `adaptive=false`** renders the real HIGH look.

**F0 audit** (`docs/recon/POLISH_RENDER_AUDIT.md`, commit `1a90916`): at forced-HIGH,
**8 of 9 reported gaps are already fixed at HEAD** — they are the exact state of `main`
(the overhaul still has `focus_core`, the float-label radial, the flat card, the overlap,
the pre-P6 layout). Sam confirmed the reviewed binary was **`main`**. The one genuine
residual was a "monochrome green" overview, plus the focused node's gate-glyph reading as
a busy "eye" at HIGH.

**Fixes** (commit `…`):
- **F1 — clean focus:** the gate-glyph (`render/node_glyph.rs`) and the orbital ring
  (`render/spatial.rs::node_qualifies_for_ring`) are **suppressed on the focus subject** —
  the focused node is now the clean reticle + single indicator ring + segmented action
  ring (the busy "eye" is gone). Respawns on focus exit.
- **F5a — Process cyan:** `theme::PROCESS` `#2bb3a8`→`#2bb0d0` (+ `tokens::PROCESS`) so
  Process reads clearly cyan and separates from File green at the overview zoom + bloom.
- **F5b — demo palette:** `graph/synthetic.rs` now seeds **~5% sockets / ~2.5% remote
  hosts / ~2% alerts** (deterministic; an all-kinds test) so the blue/violet/red palette
  manifests in the demo — not just file-green + process-cyan. Total node count unchanged.

**Verified at forced-HIGH** (`docs/media/gits/`): `afterfix-default.png` (full per-type
palette + spread layout + clean chrome), `afterfix-focus.png` (clean focus + segmented
ring + 3-block card), `afterfix-minimal-focus.png` (flat degrade). Comparison vs `main`:
`docs/media/gits/audit/main-HIGH-*` (the float-label radial + the big 3D focus_core sphere
+ flat card) vs the branch.

**Gate:** fmt/clippy/`test --workspace` green (**317 tests**, +1 all-kinds). Scope:
viewer-only; no core/graph/agent/wire change; Minimal parity preserved.

### Capture note (important for re-verification)
On this (weak) GPU the auto-tier is POTATO, which hides the HIGH-fidelity look. Re-verify
with the config forcing **both** `tier="high"` **and** `adaptive=false` (tier alone is
overridden back down by the adaptive stepper). On a GPU that sustains a higher tier this
is automatic.

