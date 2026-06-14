# MP-UI-GitS-overhaul — Sidebar replacement + focus-node detail + GUI bugfix

**Mode:** Autonomous execution on a branch — **review-gated, NOT auto-merged.**
Visual quality cannot be gated by tests; **Sam reviews screenshots before merge.**
Track A (viewer-only), no wire change, no offensive/mutating anything.
**Repo root:** `/home/dev/SpaceGraph`
**Branch:** `feat/ui-gits-overhaul`
**Specs:** ROADMAP Track A + DESIGN_LANGUAGE.md (the 3D scene aesthetic this extends
to the 2D chrome). v0.5.0 delivered the operator-shell *functionality* (dockable
shell, command palette, query-DSL, alert inbox); this MP delivers the **GitS visual
pass on the UI chrome** + focus detail + bugfix.
**Estimated size:** L.

## Critical: CC cannot see the reference images
The operator (Sam) provided "Ghost in the Shell" reference frames that **you cannot
see**. This MP translates them into concrete spec below. Build to the spec; the
operator judges the visual result. When in doubt about look, match
`DESIGN_LANGUAGE.md`'s existing 3D palette/glow and the descriptions here — do not
invent a different aesthetic.

## The visual target (translated from the refs)
- **HUD panels = holographic technical readouts**, not flat widget lists: thin
  glowing borders, **corner brackets** (┌ ┐ └ ┘ accents), monospace technical
  labels, the existing green/cyan-on-dark palette, subtle scanline/circuit-trace
  texture, faint concentric-ring framing on key panels. Think a device schematic
  card, not a settings dialog.
- **Focused node = a layered core**: concentric rings + a wireframe schematic
  overlay + small technical labels around it (the "KOSHIKI core" look), richer than
  today's reticle+blob.
- **Restraint:** glow and detail serve legibility, never clutter. Degrades to the
  Minimal theme cleanly (flat, no glow/brackets).

## Mission
1. Remove the permanent dev-debug left sidebar; replace it with a slim command rail
   + corner-anchored collapsible GitS HUD panels (graph gets full width).
2. Give focused nodes the layered-core detail treatment + a framed entity card that
   never overlaps the node.
3. Fix the radial-menu / preview / tooltip z-order overlap bugs.

## Pre-approved decisions
1. **Viewer-only.** All work in `crates/spacegraph-viewer` (egui chrome + the 3D
   focus render). **No `spacegraph-core`/`spacegraph-graph` change, no wire bump, no
   agent change** — the data model is fixed; this is presentation.
2. **Reuse / extend the existing egui style tokens** (v0.5.0 design tokens) — extend
   them into a GitS panel style; don't fork a parallel theme. Both **Standard**
   (full GitS) and **Minimal** (flat) themes must work.
3. **A panel-layer system** is the bugfix foundation: a single owner of on-screen
   panel z-order + anchoring + mutual exclusion (radial menu vs hover tooltip vs
   entity card vs preview), screen-edge-aware so nothing clips or stacks.
4. **Behavior-preserving for functionality:** every control that exists today
   (view mode, 3D/edges toggles, filter/query-DSL, focus hops, theme/quality,
   alerts, agents, search, settings, incident-hunt, actions) remains reachable —
   relocated/restyled, not removed. No feature regression.
5. **Don't regress FPS:** keep the existing `needs_redraw` repaint discipline; egui
   panels repaint only on change. (The separate FPS-4 perf issue is out of scope.)

## Out of scope
The FPS/perf problem (separate perf pass). Any 3D scene-geometry change beyond the
focused-node detail overlay. New data/fields. Any wire/core/agent change. Offensive/
mutating anything.

## Phases & gates (each: implement → `fmt`/`clippy`/`test --workspace` green → screenshot to RUNLOG → commit)

**P0 — UI inventory + bug repro.** Map the current `ui/` modules (sidebar, inspector,
hud, radial menu, preview, tooltip — confirm exact filenames) and the focus-render
path in `render/`. Reproduce the overlap bug (radial + preview + tooltip) and the
tooltip-on-node overlap; capture before-screenshots to RUNLOG.
*Gate:* module map + repro recorded.

**P1 — Panel-layer system + bugfix.** Introduce a single panel-layer/anchoring
authority: clear z-order (modal radial menu on top; entity card + preview corner-
anchored; hover tooltip lowest and **suppressed while the radial menu or entity card
is open**); screen-edge-aware placement so nothing clips. Anchor the hover/focus
readout **beside/offset** the node, never on it.
*Gate:* the radial-menu/preview/tooltip jumble is gone (after-screenshot); tooltip no
longer overlaps the node; panel placement is edge-aware (a node near the screen edge
still shows a fully-visible card). Unit-test the anchoring/placement math
(pure-fn: given node screen-pos + panel size + viewport → non-overlapping,
on-screen rect).

**P2 — Sidebar replacement.** Remove the permanent left column. Replace with: a slim
**command rail** (icon-grouped: View/Display · Filter · Alerts · Agents · Settings)
that expands sections into corner-anchored collapsible HUD panels in the GitS
readout style; the **top status strip** (AGENTS/ALERTS/MODE/FPS/TIER) kept and
restyled as the persistent header; filter/query-DSL + search via the rail/overlay
(the Ctrl+P palette already exists). Graph renders full-width when panels are
collapsed.
*Gate:* the old sidebar is gone; **every prior control is reachable** (enumerate them
in RUNLOG, tick each); GitS panel styling applied (borders/brackets/monospace);
Minimal theme = flat equivalent; after-screenshot. No feature regression.

**P3 — Focused-node detail.** (a) 3D: the focused node gains concentric rings + a
wireframe schematic overlay + small technical labels (the layered-core look), keyed
off `theme.rs` constants, Minimal-degrading. (b) The detail panel becomes a framed
GitS **entity card** (type silhouette + the fields: path/inode/kind/origin/
connections + the Fly-to/Pin-compare actions) corner-anchored — **never overlapping
the node** (uses the P1 layer system).
*Gate:* focused node renders the layered detail (after-screenshot); the entity card
is framed, anchored, non-overlapping; Minimal = the plain readout; existing
inspector tests green.

**P4 — Consistency pass + close-out.** Apply the GitS readout treatment consistently
across the remaining chrome (the debug HUD, legend, search overlay, settings panels);
typography/spacing pass. Update `DESIGN_LANGUAGE.md` (the 2D-chrome visual language +
the panel-layer rules), ACCEPTANCE (the UI criteria), CODE_INVENTORY, RUNLOG (with
before/after screenshots).
*Gate:* consistent styling across chrome; `fmt`/`clippy`/`test --workspace` green;
a full before/after screenshot set in RUNLOG for Sam's review.

## Quality gates (every commit)
`fmt --check` · `clippy --workspace --all-targets -D warnings` · `test --workspace`;
no `unwrap`/`expect` in render/ui paths; **audited: no `spacegraph-core`/
`spacegraph-graph` change, no wire bump (stays v4), no agent change, no offensive/
mutating code**; conventional commits, English; **no AI-authorship markers**; naming
hygiene; existing-code-first (extend the egui style, don't fork); archive-not-delete.

## Stop-and-Show
- If removing the sidebar would drop a control with no obvious new home → surface
  (don't silently remove a feature).
- If the GitS panel styling fights egui's capabilities (e.g. a desired effect needs
  a custom painter that risks perf) → surface the trade-off with a recommendation.
- If the focused-node 3D detail needs a scene/geometry change beyond an overlay →
  stop (out of scope; that's a render-architecture decision).
- **Before merge: always** — this is review-gated; present the before/after
  screenshot set and wait for Sam.

## BLOCKED discipline
If genuinely blocked, write `BLOCKED.md`: phase, blocker, the constraint in tension,
1–2 options + recommendation. Never regress a feature, the Minimal theme, or FPS to
get unblocked.

## Done
- Dev-debug left sidebar removed; slim command rail + GitS HUD panels; graph
  full-width; every prior control reachable; Minimal theme intact.
- Focused node renders the layered-core detail; framed entity card, corner-anchored,
  non-overlapping.
- Radial/preview/tooltip overlap fixed via the panel-layer system; tooltip off the
  node; edge-aware placement (anchoring math unit-tested).
- DESIGN_LANGUAGE/ACCEPTANCE/CODE_INVENTORY/RUNLOG updated with before/after
  screenshots.
- No core/graph/agent/wire change; no offensive/mutating code.
- Branch ready for **Sam's screenshot review** — **not auto-merged.**