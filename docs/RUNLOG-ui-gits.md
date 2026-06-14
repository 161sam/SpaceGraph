# RUNLOG — MP-UI-GitS-overhaul

**Branch:** `feat/ui-gits-overhaul` (from `main` == `feat/mcp-provider`, zero divergence)
**Track:** A (viewer-only). **Review-gated, NOT auto-merged** — Sam reviews the
before/after screenshot set before merge. Visual quality is not test-gated.
**Scope guard:** no `spacegraph-core`/`spacegraph-graph` change, no wire bump (stays
v4), no agent change, no offensive/mutating code. All work in `crates/spacegraph-viewer`.

**Baseline (`26b2516`):** `cargo fmt --check` ✓ · `clippy --workspace --all-targets
-D warnings` ✓ (no warnings) · `cargo test --workspace` ✓ **192 passed / 0 failed**.

## Phase status
- [x] **P0** — UI inventory + bug repro (`8ee4007`)
- [x] **P1** — Panel-layer system + bugfix (`12ab3e2`)
- [x] **P2** — Sidebar replacement (`848f2df`)
- [x] **P3** — Focused-node detail (`25cc208`)
- [x] **P4** — Consistency pass + close-out
- [ ] **Sam's screenshot review** → merge (review-gated; NOT auto-merged)

---

## P0 — UI inventory + bug repro

Inventory gathered by a parallel-reader sweep over all 19 `ui/` modules + the
render focus-path (`render/{spatial,node_glyph,node_icon,node_mesh,theme,camera,
interaction,mod}.rs`) + app wiring (`app/mod.rs`); every load-bearing claim below
was re-verified against the source.

### Module map (ui/)

| Module | Role | egui layer / anchoring | Notes |
|---|---|---|---|
| `panel.rs` | The deletable **left dev rail** | `SidePanel::left` | **Triple-responsibility** (see below). ~50 controls. |
| `layout.rs` | `UiLayout { panel_rect, content_rect }` Resource (Copy) | — | The anchoring contract; carries no math. Default `Rect::NOTHING`. |
| `mod.rs` | re-exports + `HUD_EDGE_PADDING=10`, `HUD_MIN_CONTENT_W=200`, `HUD_FALLBACK_Y_OFFSET=220`, `egui_color()` | — | **No overlay coordinator exists** — the structural gap P1 fills. |
| `tokens.rs` | Design tokens: `color`/`space`/`font` | — | **EXTEND target** for P2; no alpha/radius/stroke/elevation tokens yet. |
| `theme_egui.rs` | `apply_egui_theme` sets global Style/Visuals | — | Standard=`gits_style()`, Minimal=`Style::default()`. Sets no `Order`. |
| `context_menu.rs` | `context_menu_overlay` + `radial_hud`/`render_radial` + `RadialMenu` | both `Order::Foreground` | Centred on right-clicked / focused-node projection; **no mutual exclusion**. |
| `node_preview.rs` | `node_preview_overlay` (`◈ PREVIEW`) + decode pipeline | `Window`, **default `Order::Middle`** | `.anchor(CENTER_CENTER)` in focus, else `RIGHT_BOTTOM`. |
| `tooltips.rs` | `render_tooltip(ctx,id,pos,lines)` | bare `Area::fixed_pos`, **default `Order::Middle`** | **No `.order()`, no edge clamp, no node-bounds awareness.** |
| `reticle.rs` | `reticle_overlay` brackets + readout | `layer_painter`, `Order::Foreground` | Readout box at `node+(34,-24)`, 240px. Standard-only. |
| `focus.rs` | `enter/exit/double_click/focus_overlay` | `layer_painter`, `Order::Background` | Centerpiece ring r=148 on projected node. Minimal = dim only. |
| `inspector.rs` | `inspector_overlay` detail panel | `SidePanel::right` (reserves space, no z-fight) | **Only** home of connection-nav, Pin-compare, why-connected, Fly-to(F). P3 entity-card target. |
| `hud.rs` | `hud_frame_overlay` (brackets+status strip) + `hud_overlay` (debug stats) | `Background` / `Foreground` | Status strip AGENTS/ALERTS/MODE/FPS/TIER. |
| `legend.rs`/`help.rs`/`search.rs`/`command_palette.rs` | overlays | `Window`, default `Order::Middle` | palette anchors `CENTER_TOP,[0,60]` (ignores content_rect). |
| `settings_agents.rs`/`settings_paths.rs` | agent/path config windows | `Window`, `Order::Middle`, `.constrain_to(content_rect)` | Depend on `content_rect` being correct post-removal. |
| `shortcuts.rs` | `handle_shortcuts` | — | **Esc cascade is the ONLY mutual-exclusion mechanism.** F/Ctrl+P/I/L/O/T/Space/? keybinds. |
| `minimap.rs` | `minimap` | `Area`, `Order::Middle`, `RIGHT_TOP` | Spatial-only. |

**`panel.rs::ui_panel` triple-responsibility (the crux of P2):**
1. The left dev rail UI + ~50 controls.
2. **Sole writer** of `UiLayout.panel_rect`/`content_rect` (lines 569–583).
3. Dispatcher of **5 overlay windows** (lines 585–589): `path_editor_window`,
   `agent_manager_window`, `agent_editor_window`, `agent_command_window`,
   `search_overlay`.

Naive removal orphans (2) + (3) + ~50 controls → this is the MP's first
Stop-and-Show trigger. P2 must re-home all three before deleting the rail.

### Render focus-path (render/)
- `spatial.rs` — `draw_spatial` is the actual focus renderer; Standard draws the
  reticle (via `ui::reticle`) + suppresses gizmo bubbles, Minimal draws
  `gizmos.sphere` bubbles. Tooltip call sites at the node (~`1047`) and edge
  (~`1036`) build `pos = pointer.hover_pos() + (14,14)`. `highlight_style(theme)`
  maps Standard→Reticle, Minimal→Bubbles. `NodeRings`/`sync_node_rings` = orbital rings.
- `node_glyph.rs` — billboarded concentric-ring "gate" glyph (Standard, !LOD). **The
  concentric-ring precedent for P3** (shared-LineList billboard pattern).
- `node_mesh.rs` — `node_core`/`node_shell` + wireframe builders (`octahedron_wire`,
  `spiked_star_wire`, `wire_from_edges`). **P3 wireframe toolbox.**
- `node_icon.rs` — atlas-quad icon + `node_envelope(kind)` half-extent table — the
  size the P3 rings/card must sit *outside* of.
- `theme.rs` — single source of truth for render colours (`RETICLE_HOVER/SELECT/
  FOCUS`, `CLEAR_*`, `edge_color`, `lerp`). **No dedicated focus-core colour yet.**
- `camera.rs` — `apply_jump_to` clamps focus `target_radius` to **6.0..18.0** (the
  close-dive that makes the node project large near centre → why overlays collide).

### App wiring (the z-order authority)
`app/mod.rs` `SpaceGraphViewerPlugin`: the **19-system UI overlay tuple (86–108) is
registered WITHOUT `.chain()`** (ambiguous frame-to-frame order). The render
pipeline tuple (145–161) **is** `.chain()`ed. This unchained UI tuple, plus ad-hoc
per-overlay `Order` assignment, is the structural root of the overlap bug.

### Bug root-cause (repro by code analysis)

**(a) Radial / preview / tooltip / reticle / centerpiece z-order jumble.**
In Focus Mode FIVE surfaces converge on the SAME projected-node point at FOUR
different `Order`s and stack concentrically instead of laying out side-by-side:

| Surface | File | `Order` | Position |
|---|---|---|---|
| focus dim + centerpiece (r=148) | `focus.rs::focus_overlay` | `Background` | projected node |
| preview window | `node_preview.rs` | `Middle` (default) | `CENTER_CENTER` (= where the dive parks the node) |
| reticle brackets + readout | `reticle.rs` | `Foreground` | node proj / `node+(34,-24)` |
| radial rings (r 58/104) | `context_menu.rs::render_radial` | `Foreground` | projected node |
| context menu | `context_menu.rs::context_menu_overlay` | `Foreground` | clicked node |

egui paints `Background < Middle < Foreground`, so the result is deterministic but
**wrong-by-design**: centerpiece under preview under radial+reticle, all concentric.
Within `Foreground` the radial vs reticle have **no defined relative z** (and the UI
tuple isn't chained). There is **no mutual exclusion** — F opens the radial without
closing the palette/search; multiple `Middle` windows coexist with the `Foreground`
radial at the same point.

**(b) Tooltip overlaps the node.**
`render/spatial.rs` builds `pos = pointer.hover_pos() + (14,14)` and hands it to
`tooltips.rs::render_tooltip` → bare `Area::fixed_pos`, **`Order::Middle`, no clamp**.
The only thing keeping it off the glyph is +14,+14, but the reticle brackets frame
the node to ~34px and the billboards grow large on a close dive → the box lands ON
the node. No viewport clamp → a node near the right/bottom edge pushes it off-screen.
It's also `Middle` (= preview) but below the `Foreground` reticle readout, so a
hovered+selected node shows two text boxes near the same spot at different layers.

**What the single panel-layer / anchoring authority (P1) must own:**
1. An explicit `Order` contract per overlay class (no same-tier ambiguity).
2. Content-rect/screen-rect authority **relocated out of `panel.rs`** so it survives
   the rail removal (writes `content_rect = full screen` when no rail exists).
3. A single **pure, unit-testable** `place_card(node_pt, node_half_px, card_size,
   viewport) -> Pos2` that offsets a card OFF the node bounds and clamps it inside
   the viewport — reused by tooltip, reticle readout, and the P3 entity card.
4. Mutual-exclusion bookkeeping (opening the radial closes the context menu, hover
   tooltip suppressed while radial/entity-card open) — preserving the Esc cascade.

### Control inventory — P2 "every prior control reachable" checklist

To be ticked during P2 as each control gets a confirmed new home (rail / HUD panel /
palette). **Nothing may be orphaned.**

**View/Display:** View mode Spatial/Tree/Timeline · Fit-to-view · Show files · Demo
Mode · 3D · Edges · Agg edges · Raw edges · Theme combo · Quality combo · Adaptive
quality · Legend (L) · Minimap.
**Timeline:** Pause (Space) · Window(s) · X-scale · Show connectors · Scrub · Reset
scrub · Jump to Spatial.
**Filter/Query:** Filter DSL text · removable filter chips · Focus hops · Clear focus.
**Alerts/Incident:** Incident Hunt (M) · recent-alert clickable rows · Scan pulse (G).
**Agents:** Manage Agents… · per-row Mode override · Connect/Disconnect/Reconnect ·
Show checkbox · Remove · Command… · status hover · Default mode combo · Add Agent…
(+form) · Command window Copy/Close.
**Settings:** Edit Paths… (+editor) · Save Settings · Reset Defaults · Technician
gate · Performance sliders · LOD (enable/threshold/edges) · Layout (force/link/
repulsion/damping/step) · Glow ms · Gameplay (fog O + reveal/scan/fly/look/micro-
tags/orbital-rings/edge-pick) · Post-FX (scanline/vignette/aberration/grain) ·
Audio (enable/volume) · GC (enable/orphan TTL).
**Focus/Navigation:** Left-click select · Right-drag orbit / Middle-drag pan / scroll
zoom · Right-click context menu · Double-click focus · F focus · Esc cascade · radial
keys (Esc/Tab/arrows/[]/Num/Enter) · V free-fly · drag box-select / grab-pin · click
edge trace · Inspector (I) + neighbour nav + Fly-to + Pin-compare + resize.
**Search:** Open Search (Ctrl+P) · search window · command palette (Ctrl+P) actions.
**Actions:** Clear graph · Pin/Unpin · Mark/Unmark.
**Misc/keybinds:** ? help · T cycle view · O fog · I inspector · L legend · Space
pause · Ctrl+P palette · `--demo-load <n>` CLI.

### Implementation approach (P1–P4)

- **P1 — panel-layer authority + bugfix.** Add `ui/overlay.rs`: (1) `update_ui_layout`
  system that writes `content_rect = full screen` / zero-width `panel_rect` (the
  value `panel.rs` already produces when collapsed), chained FIRST in a UI prelude;
  (2) an explicit overlay-`Order` contract enum; (3) the pure `place_card` fn
  (+ unit tests: off-node, viewport-clamp, side-flip); (4) give `tooltips.rs` an
  `Order` param; route the spatial tooltip + reticle readout through `place_card` at
  a `Tooltip`-class order; (5) `.chain()` / assert-order the UI tuple in
  `app/mod.rs`; add open-X-closes-Y mutual exclusion in `shortcuts.rs` (keep the Esc
  cascade).
- **P2 — sidebar → command rail + GitS HUD panels.** Re-home `panel.rs`'s layout
  authority (→ P1) + 5 overlay dispatches (→ a small host) + ~50 controls (→ new
  `ui/rail.rs` slim icon rail + `ui/hud_panels.rs` corner-anchored collapsible
  panels + palette), verifying against the checklist above. **EXTEND `tokens.rs`**
  (alpha/radius/stroke/elevation roles; converge hardcoded `Color32` literals onto
  tokens; extend `roles_are_distinct`). Standard = full GitS; Minimal degrades via
  `apply_egui_theme`.
- **P3 — layered-core 3D + entity card.** Add `render/focus_core.rs`
  (`sync_focus_core`, gated Standard + Spatial + focus + !LOD), reusing
  `node_glyph` billboards + `node_mesh` wireframe builders, sized via `node_envelope`
  + the 6..18 dive radius; new focus-core colours in `render/theme.rs` (distinct from
  `RETICLE_*`); wire into the chained render tuple. Replace the `CENTER_CENTER`
  preview anchoring with a side-anchored framed entity card via `place_card`; re-home
  inspector connection-nav/why-connected/pin-compare into it. Minimal = plain readout.
- **P4 — consistency pass.** Route legend/help/search/palette/settings through the P1
  `Order` contract + P2 tokens; reconcile the hud strip now that `content_rect ==
  screen`; repoint/remove the now-dead "Toggle left rail" palette action; final
  reachability sweep in Standard + Minimal.

### Risks / Stop-and-Show triggers
1. **Sidebar removal** orphans layout authority + 5 windows + ~50 controls → re-home
   all before deleting (handled in P2; surface if any control has no obvious home).
2. **P3 must stay an overlay** (billboard rings + wireframe-from-existing-builders +
   labels + egui card). New per-kind solid/extruded geometry or scene churn = a
   scene/geometry change beyond viewer-only scope → STOP.
3. egui cannot do DoF/depth-blur or shader FX for the focus core without a custom
   render-graph node (already deferred) → do not attempt.
4. Layout ordering: settings/palette `.constrain_to(content_rect)` — chain
   `update_ui_layout` first or they read stale `Rect::NOTHING` for a frame.
5. Double-click is consumed by both `focus_double_click` and `detect_preview_expand`
   off the same stream → verify P3 re-anchoring neutralises the centre pile-up.
6. Don't regress the Esc cascade when adding open-X-closes-Y.
7. `tokens.rs roles_are_distinct` must be extended when adding tokens.
8. Standard/Minimal parity: gate only 3D/centerpiece on Standard — never the
   controls themselves, or Minimal loses reachability.

### Before-screenshots
_Pending the capture decision (see Stop-and-Show)._ Capture recipe (local, GPU):
```bash
cargo run -p spacegraph-viewer -- --demo-load 2000
# then capture window states with: import -window "$(xdotool search --name SpaceGraph | head -1)" docs/media/<name>.png
```
Target before-shots: `before-default.png` (Standard chrome + left rail),
`before-focus-overlap.png` (focus mode showing the radial/preview/reticle pile-up),
`before-tooltip.png` (hover tooltip overlapping a node), `before-minimal.png`.

### P0 gate
Module map + bug repro (code-level) recorded ✓ · baseline green ✓ · no code changed.

---

## P1 — Panel-layer system + bugfix

### What changed
- **New `ui/overlay.rs`** — the panel-layer & anchoring authority:
  - `layer` — the canonical egui draw-order contract per overlay class
    (`BACKDROP`=Background, `PANEL`/`READOUT`=Middle, `MODAL`=Foreground), so
    z-order is owned in one place instead of ad-hoc `.order()` calls.
  - `place_card(node, node_half, size, vp, gap) -> Pos2` — the single **pure,
    unit-tested** anchoring rule: prefer the node's right, flip left near the
    right edge, vertically centre, clamp fully on-screen; clears the node
    footprint. Reused by the hover tooltip, the reticle readout, and (P3) the
    entity card. `node_half = 0` degrades to a clamped pointer anchor.
  - `estimate_text_size` (size a readout before egui lays it out) and
    `hover_readout_suppressed(focus_mode, context_menu_open)`.
- **Tooltip anchoring + suppression** (`render/spatial.rs`, `ui/tooltips.rs`):
  the node hover readout is now anchored **beside the hovered node's projection**
  via `place_card` (was `pointer + (14,14)`, which sat on the node), edge-aware,
  and **suppressed in Focus Mode / while the context menu is open**. The edge
  tooltip is clamped on-screen and suppressed the same way. `render_tooltip`
  gained an explicit `Order`. `draw_spatial` now receives the camera (threaded
  through `draw_scene`) to project the node.
- **Reticle readout** (`ui/reticle.rs`): the selection readout is placed via
  `place_card` (off-node, edge-aware, leader line to the nearest box edge) and
  **suppressed in Focus Mode and when the selection is also the hovered node**
  (the hover readout covers it) — removing the duplicate/concentric box.
- **Focus de-clutter**: the focus-mode preview no longer anchors `CENTER_CENTER`
  on the node (`ui/node_preview.rs`) — it docks to a screen corner; P3 reframes
  it as the entity card. Entering focus / diving now closes the context menu,
  palette and search (`ui/focus.rs`, `ui/context_menu.rs`) so the radial is the
  sole node-region overlay (mutual exclusion; the Esc cascade is preserved).
- **Deterministic paint order**: the egui overlay system tuple in `app/mod.rs`
  is now `.chain()`ed (was an ambiguous tuple — P0's flagged structural root).

### Gate
- `cargo fmt --check` ✓ · `clippy --workspace --all-targets -D warnings` ✓ ·
  `cargo test --workspace` ✓ **196 viewer tests** (+7 new `ui::overlay` tests:
  right-placement, left-flip near edge, vertical clamp, tiny-viewport no-panic,
  pointer anchor, size estimate, suppression).
- No `spacegraph-core`/`spacegraph-graph` change; no wire bump; no agent change.
- Before/after screenshots: `docs/media/gits/before-focus.png` (the concentric
  radial/preview/readout pile-up) vs `afterp1-focus.png` (preview corner-docked,
  readouts suppressed); `before-hover.png` vs `afterp1-hover.png` (tooltip moved
  off the node); Minimal parity in `*-minimal*.png`.

---

## P2 — Sidebar replacement

### What changed
- **Removed** the permanent left dev sidebar (`ui/panel.rs` deleted; its widget
  bodies relocated, history preserved in git). The 3D graph now renders
  **full-width**; the chrome floats over it (holographic HUD model).
- **New `ui/rail.rs`** — a slim always-on **command rail** (icon-grouped:
  VIEW · FILT · ALRT · AGNT · CFG) floating at the left edge; each button toggles
  one corner-anchored HUD panel (`RailState`). Carries alert/agent count badges.
  Also owns `update_ui_layout` (publishes `content_rect` = screen minus the rail,
  run first so panels read it fresh).
- **New `ui/hud_panels.rs`** — the controls, grouped by rail section, in a single
  GitS-framed panel anchored beside the rail; plus `dispatch_windows`, which hosts
  the four modal windows + node search the old sidebar used to dispatch.
- **New `ui/gits.rs`** — GitS chrome helpers (translucent panel frame, corner
  brackets, monospace section headers) built from the **extended** `tokens.rs`
  (`radius`, `stroke_w`, `alpha` added). Standard = GitS frame; Minimal = the
  plain egui popup frame (flat) — both verified.
- Moved the debug-telemetry HUD (`ui/hud.rs`) to the content area's bottom-left so
  it no longer collides with the top-left HUD panels.
- `app/mod.rs`: `ui_panel` → `(update_ui_layout, command_rail, hud_panels,
  dispatch_windows).chain()`; registered `RailState`.

### Reachability checklist (every prior control kept)
- [x] **View/Display** → VIEW panel: mode Spatial/Tree/Timeline, Fit-to-view,
  Show files, Demo Mode, 3D, Edges, Agg edges, Raw edges, Theme, Quality,
  Adaptive, **Show/Hide legend** (new button; was L-key only), Minimap (auto).
- [x] **Timeline** → VIEW panel (Timeline mode): Pause, Window, X-scale, Show
  connectors, Scrub, Reset scrub, Selection A/B, Jump to Spatial.
- [x] **Filter/Query** → FILT panel: filter DSL, removable chips, Focus hops,
  Clear focus.
- [x] **Alerts/Incident** → ALRT panel: Incident-Hunt score/status, recent-alert
  rows (click → jump), severity counts. (M / G keybinds unchanged.)
- [x] **Agents** → AGNT panel: count + Manage Agents… → the agent-manager window
  (all per-row controls + Add Agent form unchanged, via `dispatch_windows`).
- [x] **Settings** → CFG panel: Edit Paths…, Save Settings, Reset Defaults,
  Clear graph, Technician (full tuning block: Performance, LOD, Layout, Glow,
  Gameplay, Post-FX, Audio, GC), Open Search.
- [x] **Focus/Nav, Search palette, all keybinds** unchanged (Ctrl+P palette,
  F/I/L/O/T/?/M/G/V/Esc; right-click menu; radial).

### Gate
- `fmt --check` ✓ · `clippy --workspace --all-targets -D warnings` ✓ ·
  `test --workspace` ✓ (196 viewer tests).
- No core/graph/agent/wire change.
- Screenshots: `afterp2-default.png` (full-width graph + slim rail),
  `afterp2-view.png` / `afterp2-settings.png` (GitS HUD panels),
  `afterp2-minimal*.png` (flat Minimal parity). Compare vs `before-default.png`
  (old left sidebar).

---

## P3 — Focused-node detail

### (a) Layered core
`ui/focus.rs::draw_centerpiece` is now a **layered-core schematic** (Standard):
concentric core rings (outer accent + green mid + faint inner) with tick marks
(longer every 90°), a faint **wireframe octagon** with vertex nodes (the device-
schematic motif), a `◤ FOCUS ◥` tag, and the radial kind/links/identity labels —
keyed off `render::theme` + `tokens` colours. Static (no per-frame animation → no
FPS cost). Minimal degrades to the plain dim (unchanged gate).

**Scope decision (Stop-and-Show, recorded):** the layered core is a *screen-space
schematic over the projected node* — the same paradigm as the existing
centerpiece / reticle / node-glyph — **not** new 3D scene geometry. The MP's
out-of-scope guard says to stop if the focus detail needs a scene/geometry change
beyond an overlay; it does not, so no new entities / `RebuildNodeEntities` churn
were introduced. If Sam wants depth-true 3D core meshes that is a separate
render-architecture decision.

### (b) Entity card
New `ui/entity_card.rs` — a framed GitS **entity card** shown in Focus Mode,
corner-anchored **bottom-right** (clear of the centered node, the minimap top-
right and the focus preview bottom-left — uses the P1 layer model, never overlaps
the node). Shows a per-kind **type silhouette** glyph, the identity fields
(path/inode/pid/exe… via `node_tooltip_lines`), `origin` (namespace) and
`connections` (degree), plus **Fly-to (F)** and **Pin compare / Unpin** actions.
Standard = GitS frame + corner brackets; Minimal = the plain flat card.

### Gate
- `fmt --check` ✓ · `clippy --workspace --all-targets -D warnings` ✓ ·
  `test --workspace` ✓ (196 viewer tests; existing inspector/focus tests green).
- No core/graph/agent/wire change.
- Screenshots: `afterp3-focus.png` (layered core + entity card),
  `afterp3-minimal-focus.png` (flat degrade). Compare vs `before-focus.png`.

---

## P4 — Consistency pass + close-out

### What changed
- **Telemetry HUD** (`ui/hud.rs`): wrapped in the GitS `panel_frame` with a
  `◢ TELEMETRY` header (Standard) / flat (Minimal), consistent with the HUD panels.
- **Legend** (`ui/legend.rs`): GitS `panel_frame` + corner brackets (Standard).
- **Command palette** (`ui/command_palette.rs`): the now-dead "Toggle left rail"
  action is repointed to **"Toggle controls panel"** (toggles the View HUD panel
  via `RailState`); headless palette test updated.
- Search/help/settings dialogs inherit the global GitS `Visuals` (already
  consistent); the modal agent/path windows keep their dialog frame.
- Docs updated: `DESIGN_LANGUAGE.md` (the 2D-chrome visual language + panel-layer
  rules), `ACCEPTANCE.md` (the MP-UI-GitS acceptance criteria), `recon/
  CODE_INVENTORY.md` (the new `ui/` modules), this RUNLOG.

### Gate
- `fmt --check` ✓ · `clippy --workspace --all-targets -D warnings` ✓ ·
  `test --workspace` ✓ (196 viewer tests).
- No `spacegraph-core`/`spacegraph-graph` change · no wire bump (stays v4) · no
  agent change · no offensive/mutating code (audited across P1–P4).

---

## Close-out — for Sam's screenshot review

**Branch `feat/ui-gits-overhaul` is ready for review — NOT auto-merged.**

Commits: `8ee4007` P0 · `12ab3e2` P1 · `848f2df` P2 · `25cc208` P3 · P4 (this).

### Before / after screenshot set (`docs/media/gits/`)
| State | Before (baseline) | After |
|---|---|---|
| Default chrome | `before-default.png` (left dev sidebar) | `afterp2-default.png` (full-width + slim rail) |
| HUD panel | — | `afterp2-view.png`, `afterp2-settings.png` |
| Focus mode | `before-focus.png` (radial/preview/readout pile-up) | `afterp3-focus.png` (layered core + entity card) |
| Hover readout | `before-hover.png` (on the node) | `afterp1-hover.png` (beside the node) |
| Minimal theme | `before-minimal.png` | `afterp2-minimal.png`, `afterp3-minimal-focus.png` |

> Screenshots were captured autonomously on `DISPLAY :0` from the offline
> `--demo-load 2000` synthetic graph (no real agent data). Focus states use the
> gated `SPACEGRAPH_DEMO_FOCUS` hub-autofocus; theme via an isolated
> `XDG_CONFIG_HOME`. Re-capture: `/tmp/sg_capture_set.sh <out> <prefix>`.

### Reviewer notes / open decisions
- **Layered core** is a screen-space schematic over the focused node (the existing
  centerpiece/reticle paradigm), **not** new 3D scene geometry — per the MP's
  out-of-scope guard. If you want depth-true 3D core meshes, that's a separate
  render-architecture task.
- The gated `SPACEGRAPH_DEMO_FOCUS` hub-autofocus is screenshot-only tooling (no
  effect on a normal run); remove it if undesired.
- Rail glyphs/labels are functional placeholders (monospace); a dedicated icon
  font could replace them in a later polish pass.
