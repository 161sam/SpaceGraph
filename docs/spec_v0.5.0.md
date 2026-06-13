# SpaceGraph v0.5.0 — Technical Specification
## GitS UX-Shell, Radial Command HUD & Quality-Tier System

**Status:** v0.1 (for review), 2026-06-14 · **Owner:** 161sam (Sam)
**Feeds:** the v0.5.0 implementation master-prompt (which is pure orchestration
over this spec). **Scope:** Track-A, viewer-local — **no ESN contract
dependency**, so this spec can proceed now; v0.6.0+ specs must wait for the
contracts the recon flagged as missing locally.

This spec resolves the recon's v0.5.0 SPEC-REQUIRED blockers (O-1 token parity;
docking under bevy_egui 0.28; query-DSL grammar), folds in finding F1
(visual_theme has no in-app selector), integrates the operator-approved radial
command HUD design, and adds the **Pi → desktop quality-tier system** as the new
spine (operator constraint: must run on diverse hardware incl. Raspberry Pi).

Read alongside the recon artifacts: `docs/recon/RECON_REPORT.md` (Part B has the
per-spec checklist this fulfils), `docs/ARCHITECTURE.md`, `docs/GRAPH_SCHEMA.md`,
`AGENTS.md`, `docs/DESIGN_LANGUAGE.md`.

---

## 1. Resolved decisions

- **D-1 (O-1 — token parity, not component reuse).** SpaceGraph is Bevy/egui
  native and cannot consume Smolitux-UI (React). v0.5.0 ships a SpaceGraph
  *design-token module* that mirrors the house token **semantics** (palette
  roles, typography Inter / Space Grotesk / JetBrains Mono, spacing) into egui.
  Parity, not import. (§3.1)
- **D-2 (docking under bevy_egui 0.28).** Use **native egui panels**
  (`SidePanel::left/right`, `TopBottomPanel::bottom`, `CentralPanel`) for the
  IDE-style shell with collapsible, width-persisted panels. **No `egui_dock`
  dependency** — avoids a new top-level dep and version churn under egui 0.28.
  (§3.2)
- **D-3 (quality tiers — the spine).** A new axis `QualityTier {Potato, Low,
  Medium, High}`, **orthogonal** to the existing aesthetic axis `VisualTheme
  {Standard, Minimal}`. Auto-detected at startup, runtime-adaptive, manually
  overridable. (§2)
- **D-4 (GitS-at-low-cost principle).** The GitS *identity* is tier-independent;
  only *expensive GPU effects* are tier-gated. A Raspberry Pi at the Potato tier
  still reads unmistakably as Ghost-in-the-Shell. (§2.3)
- **D-5 (radial command HUD).** Operator-approved layout: inner ring = fixed
  commands, outer ring = paths/neighbours, keyboard-driven (1–9 / ←→ / Tab /
  paging), rendered screen-space in egui, built by **evolving the existing
  `ui/context_menu.rs`** (existing-code-first). (§3.4)
- **D-6 (query-DSL).** Grammar and predicate semantics defined; replaces the
  substring filter. (§3.8)
- **D-7 (F1 — theme/tier selectors).** In-app selectors for both axes in the
  settings panel, persisted. (§3.9)

---

## 2. Quality-tier system (spine)

### 2.1 Axes
Two orthogonal axes, both persisted:
- `VisualTheme {Standard, Minimal}` — **aesthetic** (existing; Minimal stays the
  pre-visual-pass flat look and the behavioural-equivalence baseline).
- `QualityTier {Potato, Low, Medium, High}` — **cost**. New.

These compose: a Pi user can run `Standard` aesthetic at `Potato` cost (GitS look,
no bloom/post-FX). `Minimal` forces the cheapest path regardless of tier.

### 2.2 What each tier gates

| Tier | HDR + Bloom | Post-FX | Node LOD | Orbital rings | Max nodes (default) | MSAA | Target FPS |
|---|---|---|---|---|---|---|---|
| **Potato** (Pi / GLES / llvmpipe) | off | off | gate-glyph only | off | 400 | off | 30 |
| **Low** (weak iGPU) | bloom, low | scanline-only @ ½-res | glyph far / silhouette near | hubs only | 800 | off | 30 |
| **Medium** (Intel HD 520-class) | bloom, natural | full @ ½-res | full LOD ladder | on | 1200 | 2× | 45 |
| **High** (discrete GPU) | bloom, natural | full @ full-res | full LOD ladder | on | 2500 | 4× | 60 |

Each tier is a named preset of render flags consumed where v0.4.0 already
branches on `VisualTheme` (camera HDR + `BloomSettings` presence, `postfx.rs`
enable/scale, `node_mesh`/glyph LOD distances, ring spawn predicate, the
`max_visible_nodes` default, MSAA sample count).

### 2.3 GitS-at-low-cost (D-4) — binding split

| Tier-independent (always on, cheap) | Tier-gated (expensive) |
|---|---|
| neon-on-black palette, per-type colours | HDR bloom |
| **gate-glyph nodes** (billboarded concentric ring + centre dot) | chromatic-aberration + grain post-FX |
| reticle + in-world readout (egui) | 3D per-type silhouettes + orbital-ring meshes |
| radial command HUD (egui) | high node budgets |
| HUD rand-frame, dive ripples (2D) | MSAA |
| query-DSL, command palette | |

Consequence: the cheapest path still renders gate-glyphs, neon colours, the
reticle, the command ring and the dive ripple — the GitS read survives on a Pi.

### 2.4 Auto-detection
At startup read Bevy's `RenderAdapterInfo` (`device_type`, `name`, `backend`)
and pick a default tier:

- `DiscreteGpu` → **High**
- `IntegratedGpu` → **Medium**, but if `backend` ∈ {Gl} or name matches a
  weak-iGPU list → **Low**
- `Cpu`/`Other`, or name contains `V3D` / `VideoCore` / `llvmpipe` / `swiftshader`,
  or backend is GLES → **Potato**

Heuristic lives in a pure fn `detect_tier(info) -> QualityTier` (unit-tested with
fixture adapter infos incl. a Pi V3D string). Always overridable by config /
settings; auto-detect only sets the *default*.

### 2.5 Runtime adaptive
A rolling FPS monitor (e.g. 1 s window). If mean FPS < tier target for 3 s →
step the **effective** tier down one notch (apply the lower tier's gates as a
runtime override, never below Potato). If mean FPS > target + margin for 10 s →
step back up, capped at the base tier. Toggle `quality.adaptive` (default on).
Hysteresis (the 3 s/10 s asymmetry + margin) prevents oscillation. The
adaptive state machine is pure and unit-tested with a synthetic FPS trace.

### 2.6 Runtime reconfiguration
Tier changes (manual or adaptive) reconfigure the camera once (HDR flag +
`BloomSettings` add/remove), toggle the post-FX node, swap node-LOD thresholds,
and add/remove orbital-ring children — exactly the **one-time rebuild on a rare
event** pattern v0.4.0 established for theme switches. **No per-frame churn.** A
structural test asserts a tier switch triggers exactly one reconfiguration and
steady state is churn-free.

### 2.7 Config
`[quality]` block in `viewer.toml`: `tier` (`auto`|`potato`|`low`|`medium`|`high`),
`adaptive` (bool), `target_fps_override` (optional int). Plumbed through all four
plumbing points (config.rs ↔ viewer.toml writer ↔ settings panel ↔ apply path),
mirroring how `visual_theme` is wired.

---

## 3. Visual / UX subsystems

### 3.1 Design tokens + egui GitS reskin
- New `ui/tokens.rs`: the SpaceGraph token set — colour roles (bg/surface/line/
  accent-cyan/accent-green/severity), spacing, and font roles. Maps house
  semantics (D-1) into egui.
- New `ui/theme_egui.rs`: builds `egui::Visuals` + `egui::Style` (dark, flat,
  segmented, corner-bracket framing, cyan accent) and custom fonts via
  `FontDefinitions` — embed `JetBrains Mono` (UI mono), `Space Grotesk`
  (headers), `Inter` (body) as assets (OFL, embeddable like the existing WAVs).
  Applied globally at startup and on theme change.
- The reskin restyles **all** existing panels; it changes appearance only, not
  behaviour (anti-regression: every existing control remains functional).

### 3.2 Dockable IDE-shell (D-2)
Native egui panel layout replacing the single floating sidebar:
- `SidePanel::left` — operator rail: Status, Agents, Alerts (by severity),
  Filter (query-DSL). The **tuning controls** (Layout/LOD/Glow/Performance
  sliders) move into a **collapsible "Technician" section, collapsed by
  default** (demote-not-delete; Sam tunes layout live, so they stay reachable).
- `SidePanel::right` — Inspector (the existing `ui/inspector.rs`, reskinned).
- `TopBottomPanel::bottom` — Timeline lane (existing timeline view, docked).
- `CentralPanel` — the 3D viewport.
- Open/collapsed state + widths persisted in `viewer.toml` (`[shell]`). Panels
  individually toggleable (keybinds + command palette).

### 3.3 Node representation & LOD (gate-glyph layer)
- New `render/node_glyph.rs`: a **billboarded gate-glyph** — concentric ring(s)
  + centre dot, type-coloured, camera-facing, instanced. Cheap (a flat quad with
  a procedural/SDF ring, or a tiny ring mesh). This is the GitS "gate" icon from
  the reference frames.
- **LOD ladder** (distances per tier, §2.2):
  - far → gate-glyph only
  - mid → v0.4.0 3D silhouette + (tier-permitting) orbital ring
  - focused/selected → full radial command HUD + readout + dive ripple
- At **Potato/Low** the gate-glyph is the *primary* node representation (3D
  silhouettes/ring meshes suppressed). At **Medium/High** it's the far-LOD tier.
- Reconciles Sam's "nodes like the screenshots" with the existing v0.4.0
  geometry investment — nothing is thrown away; the glyph is an added far/cheap
  tier. Glyph spawn/despawn follows the persistent-entity / no-churn rule.

### 3.4 Radial command HUD (D-5) — the core interaction
Evolve `ui/context_menu.rs` (the v0.4.0 right-click radial menu) into the
keyboard-driven ring HUD; reuse `ui/reticle.rs` readout as the inner core.

**State** (`ui.radial: Option<RadialState>`): `focused: NodeId`,
`active_ring: Ring{Commands, Paths}`, `cursor: usize` (highlighted slot),
`path_page: usize`.

**Inner ring — commands** (fixed verbs, ≤8, reuse the inspector/context_menu
`Act` enum): Focus/Fly-to, Pin/Unpin, Trace (compare-pin → why-connected),
Isolate (focus subgraph), Inspect, Mark. (Forward-compatible: v0.7.0 appends
AdminBot actions for actionable node types, gated through the approval layer —
out of scope here.)

**Outer ring — paths**: the focused node's neighbours (`edges_for_node`),
paged by 9. Selecting a path re-centres focus on that neighbour and plays a dive
ripple — keyboard graph traversal.

**Input model:**
- open: `F` / click on a node; close: `Esc`
- `1`–`9`: select slot in the active ring; `Enter` (or number, for safe verbs):
  execute
- `Tab` / `↑↓`: switch active ring (Commands ⇄ Paths)
- `←→`: rotate the cursor around the active ring
- `[` `]` / PageUp-Down: page the outer ring when neighbours > 9
- tap/click a segment: select (mouse parity)

**Rendering:** egui painter — two concentric 2-D rings anchored at the focused
node's projected screen position; numbered slot markers; active-ring + selected-
slot highlight (amber); an arc-label with node identity; leader-line to the
readout box. Screen-space (not 3-D world rings) for legibility + picking.
Actions deferred via the `Act` enum to avoid borrow conflicts (existing pattern).
Tier-independent (egui, cheap). Determinism-exempt.

### 3.5 Dive ripples
New cheap effect: on focus / on alert, emit expanding concentric ripple rings
from the node, decaying ~0.6 s (2-D screen-space egui, or a billboard at the
lowest tiers). The node-focus analogue of the existing edge pulse. Tier-
independent, determinism-exempt.

### 3.6 HUD rand-frame
Peripheral arc/bracket segments at the viewport margins (egui painter) carrying
live global state: agents connected, alert counts by severity, mode, FPS/health,
active tier. Edge-hugging (corner arcs, not viewport-dominating circles) so the
centre stays the visualisation. Replaces the current top-left text HUD; pulls
Agents/Alerts/Mode out of the left rail into the frame.

### 3.7 Command palette
New `ui/command_palette.rs`: `Ctrl/Cmd+P` → fuzzy palette over **actions +
navigation + nodes** (focus search, toggle panels, switch theme/tier, run a
node command, jump to node). Extends the existing `ui/search.rs` (today
node-only). Fuzzy match unit-tested.

### 3.8 Query-DSL (D-6)
Replaces the substring filter. **Grammar:**
```
query      := term (WS term)*            ; implicit AND
term       := ['-'] (predicate | word)   ; leading '-' negates
predicate  := key ':' value
key        := type | kind | host | sev | name | path | deg | recent
value      := word | quoted | (op number) | duration
op         := '>' | '<' | '>=' | '<='     ; deg only
duration   := number ('s'|'m'|'h'|'d')    ; recent only
word       := /[^\s:]+/                    ; bare word → substring on label
```
**Semantics:** `type`/`kind` ∈ {process,file,user,socket,host,alert}; `sev` ∈
{low,med,high}; `deg:>N` degree filter; `recent:5m` = active/glowing within the
window; `name`/`path` = substring on that field; bare word = substring on label;
`-term` negates; terms AND together. (`OR` / `|` noted as a v0.5.x extension —
v1 is AND + negation.) Pure parser in `graph/query.rs` (no Bevy), compiling to a
`Fn(&NodeView) -> bool` predicate; unit-tested (valid parses, predicate hits/
misses, malformed input → graceful error shown as a chip). The filter UI renders
parsed terms as removable chips.

### 3.9 Theme / tier selectors (F1)
Settings panel: a `VisualTheme` selector (Standard/Minimal) **and** a
`QualityTier` selector (Auto/Potato/Low/Medium/High) + an `adaptive` toggle.
Both persisted. Resolves F1 (no in-app theme selector existed).

---

## 4. Work-package breakdown (→ MP phases)

Ordered by dependency. WP-0 is the prerequisite (it fixes the framerate the rest
is built on). Each WP merges on a green gate; one branch per WP.

| WP | Title | Key deliverables | Depends on |
|---|---|---|---|
| **WP-0** | Quality-tier system | `render/quality.rs` (tiers, detect, adaptive), camera/bloom/postfx/LOD/MSAA wiring, `[quality]` config | — |
| **WP-1** | Tokens + egui reskin + shell + selectors | `ui/tokens.rs`, `ui/theme_egui.rs`, embedded fonts, native-panel shell, `[shell]` config, F1 selectors | WP-0 |
| **WP-2** | Gate-glyph LOD layer | `render/node_glyph.rs`, LOD ladder, tier interplay | WP-0 |
| **WP-3** | Radial command HUD | evolve `ui/context_menu.rs`, `ui.radial` state, keyboard model, reuse reticle readout | WP-1, WP-2 |
| **WP-4** | Dive ripples + HUD rand-frame | ripple effect, peripheral global-state arcs | WP-1 |
| **WP-5** | Command palette + query-DSL | `ui/command_palette.rs`, `graph/query.rs`, chips | WP-1 |
| **WP-6** | Docs reconcile + tag | DESIGN_LANGUAGE / ACCEPTANCE (incl. F3) / README / RUNLOG; tag `v0.5.0` | all |

**Per-WP acceptance** (machine-checkable unless noted):
- WP-0: `detect_tier` fixtures incl. Pi V3D → Potato; adaptive state-machine on a
  synthetic FPS trace steps down/up with hysteresis; tier switch = exactly one
  reconfiguration, steady state churn-free; `[quality]` config round-trip;
  Minimal still forces cheapest path. *Local capture:* Pi + integrated + discrete
  FPS table in RUNLOG (the documented substitute for headless/CI perf).
- WP-1: shell layout + open/width persist round-trip; reskin leaves every
  existing control functional (anti-regression: registered-systems check from the
  recon stays green); selectors persist both axes.
- WP-2: glyph spawned per node, type-coloured; LOD selection pure-fn tested;
  Potato → glyph-only (no silhouette/ring meshes) asserted; no steady-state churn.
- WP-3: radial state transitions unit-tested (open/close, ring switch, rotate,
  paging, path-dive re-centres focus); each command maps to the correct `Act`;
  HUD renders without panic on a headless app with a focused node + camera.
- WP-4: ripple lifecycle (spawn on focus/alert, decays, despawns); rand-frame
  reads global state without panic.
- WP-5: query-DSL parser (valid/negation/malformed) + predicate hits/misses;
  palette fuzzy-match tested.
- WP-6: ACCEPTANCE gains the F3-missing criteria; docs consistent with code;
  `v0.5.0` tagged.

---

## 5. Test & acceptance strategy

Headless CI gates: `fmt`, `clippy -D warnings`, `test` (structural ECS + pure-fn
+ config round-trip). If the gate-glyph uses a WGSL shader, it is **naga-gated**
like the v0.4.0 post-FX. **Performance is not assertable in CI or on the Pi in
this env** → each perf-relevant WP writes a **local-capture procedure to
`docs/perf/RUNLOG.md`** covering at least three hardware classes (Pi, integrated,
discrete) — this is the deliverable, not a stop, exactly as v0.4.0 handled
headless GPU. The adaptive logic itself is fully unit-testable via synthetic FPS
traces, so the *behaviour* (not the wall-clock) is gated in CI.

---

## 6. Consolidated new config keys
```
[quality]   tier=auto  adaptive=true  target_fps_override=
[shell]     left_open=true left_width=… right_open=true right_width=…
            bottom_open=false  technician_open=false
[ui]        visual_theme=standard           ; now also surfaced in-app (F1)
```
All plumbed through config.rs ↔ viewer.toml writer ↔ settings panel ↔ apply path.

---

## 7. Open items / operator decisions

- **S-1 (sequencing choice, low stakes):** WP-0 (quality tiers) could ship as its
  own **v0.4.1** *before* the rest of v0.5.0, since it stands alone and fixes the
  framerate immediately. Default in this spec: WP-0 is the first phase *of*
  v0.5.0. Flag if you'd prefer the earlier perf release.
- **S-2 (later extension, already noted):** multi-category command rings
  (concentric command rings beyond inner/outer) — deferred per your "erstmal so
  ok, vlt später erweitern".
- **Recon findings handled here:** F1 → §3.9 (this spec). F3 (ACCEPTANCE missing
  lane-timeline/Tree criteria) → WP-6. **F2** (ROADMAP §1 stale post-v0.4.0) and
  **F4** (Blueprint v0.2.0 divergence) are **operator doc tasks**, out of this
  spec's scope — noted for a roadmap/vision refresh.
- No other open items: this spec is intended to leave the v0.5.0 implementation
  MP as pure orchestration.

---

## Appendix — file map

**New:** `render/quality.rs`, `render/node_glyph.rs`, `ui/tokens.rs`,
`ui/theme_egui.rs`, `ui/command_palette.rs`, `graph/query.rs`; embedded font
assets under `assets/fonts/`.
**Evolved:** `ui/context_menu.rs` (→ radial HUD), `ui/panel.rs`/`ui/hud.rs` (→
shell + rand-frame), `ui/inspector.rs` + `ui/legend.rs` + `ui/settings_*` (reskin
+ selectors), `render/postfx.rs` + camera setup + `render/node_mesh.rs` (tier
gating), `util/config.rs` + `viewer.toml` (config), `render/spatial.rs` (LOD,
ripple), `ui/reticle.rs` (readout reuse by the radial HUD).
**Docs:** `docs/DESIGN_LANGUAGE.md`, `docs/ACCEPTANCE.md`, `README.md`,
`docs/perf/RUNLOG.md`.