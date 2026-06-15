## Module map

| Module | Role | Key entry (file:line) |
|---|---|---|
| ui/overlay.rs | z-order classes + sole node-anchor/clamp primitive | `layer` :16, `place_card` :51, `hover_readout_suppressed` :93 |
| ui/rail.rs | command rail + publishes `UiLayout` (content_rect bug source) | `update_ui_layout` :50, `RAIL_WIDTH/TOP_OFFSET` :19, rail_button :94 |
| ui/layout.rs | `UiLayout` resource (panel_rect + content_rect) | :4 |
| ui/hud.rs | telemetry readout + raw HUD frame/strip | `hud_overlay` :76, `draw_hud_frame` :27, top strip line :61 |
| ui/hud_panels.rs | left HUD panels + dispatch/settings windows + layout sliders | dispatch_windows :24, Layout(Spatial) sliders :444 |
| ui/inspector.rs | docked right SidePanel detail (suppressed in focus) | `inspector_overlay` :29/:107 |
| ui/entity_card.rs | focus-mode detail card (flat dump today) | `entity_card_overlay` :27, `silhouette/kind_name` :140 |
| ui/node_preview.rs | conditional-anchor preview window | anchor branch :481 |
| ui/minimap.rs | Spatial top-down X/Z radar | `minimap` :22, `to_screen` :58, consts :10 |
| ui/reticle.rs | corner brackets + sole place_card readout | `draw_brackets` :117, `draw_readout` :127 |
| ui/context_menu.rs | radial HUD (float-label rings) + context menu | `render_radial` :314, `radial_hud` :204, `ACTIONS/CtxAct` :27, ctx menu :442 |
| ui/focus.rs | dim + 2D centerpiece arc labels | `focus_overlay` :82, `draw_centerpiece` :134 |
| ui/legend.rs | legend window (reads render/theme palette) | `legend_overlay` :54 |
| ui/tooltips.rs | hover tooltip renderer (no width/truncate) | `render_tooltip` :3 |
| ui/tokens.rs | egui-chrome palette/tokens (single source) | `color` :9, `roles_are_distinct` :86 |
| ui/gits.rs | shared chrome kit (frame/header/brackets) | `panel_frame` :23, `section_header` :37, `draw_brackets` :53 |
| ui/theme_egui.rs | tokens→egui visuals + font registration | `install_fonts` :23, `gits_visuals` :56 |
| render/focus_core.rs | P5 3D focus rig (to archive) | `sync_focus_core` :89, `spawn_core` :130, `animate_focus_core` :187, `setup_focus_core_resources` :67 |
| render/spatial.rs | node entity sync + materials + overlays | `sync_node_entities` :508, `node_material` :453, `node_meshes` :417, `ApertureStyle` :867, draw_overlays(pulse) :1209, (hover edge) :1023, labels :1235 |
| render/node_mesh.rs | per-kind core + optional wire shell | `node_core` :17, `node_shell` :39 |
| render/node_glyph.rs | per-type gate ring glyph + silhouette gate | `ring_color` :44, `silhouette_active` :64 |
| render/node_icon.rs | billboard face icons + per-kind envelope | `icon_for` :139, `node_envelope` :155 |
| render/edges.rs | batched LineList agg-edge mesh | `update_edge_mesh` :152, `setup_edge_mesh` :119, `edge_lod` :49, `EdgeFingerprint` :83 |
| render/theme.rs | 3D-scene palette + NodeKind/edge colours | color consts :16, `NodeKind` :107, `base_color` :149, `edge_color` :167, FOCUS_CORE_* :52 |
| render/quality.rs | tier gates (silhouettes) | `QualityTier::gates` :128 |
| render/camera.rs | camera spawn + fly-to | `apply_jump_to` :73, `setup_scene` :28 |
| graph/layout.rs | force-directed layout | `force_step` :514, spring k :641, cutoff :566, settle :696, idle short-circuit :528, `scatter_position` :750 |
| graph/state.rs | runtime state + cfg + tooltip lines | `CfgState` :540, cfg defaults :768, `node_tooltip_lines` :1637, `placed_positions` :187, `request_jump` :1744 |
| graph/grid.rs | uniform spatial grid for repulsion | `neighbors_into` :120 |
| util/ids.rs | node label builders | `node_label_short` :27, `node_label_long` :52, `normalize_display_path` :4 |
| spacegraph-graph/model.rs | aggregation + adjacency + degree | `AggEdge` :58, `edges_for_node` :167, `degree` :179, `agg_edge` :194 |
| spacegraph-core/lib.rs | core Node/Edge enums | `Node` :36, `Edge` :85, `EdgeKind` :93 |

## Current-state bug/gap repro (by code analysis)

**Telemetry / preview overlap** — In focus mode `node_preview` anchors `LEFT_BOTTOM [12,-12]` (node_preview.rs:483) and the telemetry HUD anchors `LEFT_BOTTOM [content_rect.min.x+10, -10]` (hud.rs:86,89-94); both are bottom-left at PANEL/Middle order, so a focused file/pinned node with telemetry visible stacks both in one corner. Outside focus, preview anchors `RIGHT_BOTTOM [-12,-12]` (node_preview.rs:484) — colliding with the inspector and minimap on the right, since `content_rect` (rail.rs:57-58) subtracts only the left rail.

**Radial overlap / floating labels** — `render_radial` lays the 6 inner-ring command labels as bare monospace text at evenly-spaced angles (`ang=(i/cmds)*TAU-TAU/4`, `painter.text(...)`) with no segment geometry, dividers, or backing (context_menu.rs:385-398); the outer paths ring repeats the same pattern at r=104 (:400-416). Variable-length labels ("Isolate subgraph", "Trace connections") overlap each other and the outer ring at small sizes. Only the dim backing disc + two full circle strokes are drawn regardless of slots (:345-383).

**Path overflow** — No middle-ellipsis helper exists. Card body uses `TextWrapMode::Truncate` (right-side, glyph granularity) so a long path loses its load-bearing filename tail (entity_card.rs:76); inspector detail uses `Wrap` (inspector.rs:121); `render_tooltip` prints each line with a bare `ui.label`, no width cap and no truncation, so a long cmdline/path spans the viewport (tooltips.rs:14-18). Labels arrive full-width from `node_label_short/long` (ids.rs:27/52).

**Primitive / monochrome nodes** — Standard theme is NOT monochrome: every visible node already renders a per-type 3D core, per-kind face icon, and per-kind gate ring (spatial.rs:417/453, node_glyph.rs:44, node_icon.rs:139). But the palette is off-spec — Process srgb(0.20,0.85,0.95), File srgb(0.25,0.95,0.45), User srgb(0.98,0.75,0.25), Socket srgb(0.30,0.60,0.98), HOST srgb(0.70,0.55,0.99), Alert srgb(0.98,0.22,0.25) vs MP hexes (theme.rs:16-26). Silhouettes are tier-gated off at Potato/Low (quality.rs:148-155) where the core falls back to a flat sphere (spatial.rs:417). The Minimal theme is the only true monochrome path.

**Hairball layout** — `force_step` has NO centering/gravity and NO degree scaling; every node is identical. Repulsion is neighbour-only with a HARD cutoff at `repulsion_radius=8.0` (cell==cutoff, `if dist2>cutoff2 {continue}` layout.rs:566-571,613) while springs pull all edges to `link_distance=6.0` (k=0.6 hardcoded, :641). For N=1000 the scatter side ≈cbrt(N)*8≈80u but pairs >8u apart exert zero repulsion, so the spring net contracts to a clump the size of the cutoff. The synthetic fixture has hubs (users ~deg 4+) but nothing makes them bigger/central/separated.

**Crude minimap** — `minimap` recomputes a fresh X/Z AABB over only the visible set every frame (minimap.rs:44-65), so the whole map rubber-bands/rescales as nodes move or fog toggles. It queries only `&Transform With<Camera>` (no `&Camera`/`&GlobalTransform`), so it can only draw a white camera ring (:85-91) — no frustum rect, no focus/selection marker, no LIVE/scale hint. Painter uses `Sense::hover()` (:35) so zero click handling, and the Area sets no `.order(...)` (:31-32).

**Flat edges** — Edges are straight 2-vertex LineList segments (edges.rs:124,272-275), uniform ~1px width, constant colour along the edge (single `col` to both vertices :265-275), alpha always 1.0 with only a discrete 3-band brightness LOD (:264). No curvature, no thickness, no gradient/flow, no weight (`AggEdge.stats.count` exists but is never read), no threat styling. The only motion is a single decaying pulse sphere on `glow_edges` (spatial.rs:1209-1228). Default `lod_edges_mode=FocusOnly` (config.rs:421) means almost no agg edges even build until something is selected.

**Dominating 3D focus core** — Default Spatial+Standard focus layers FOUR systems on one node: the P5 3D mesh rig (3 spinning tori scale 1.35 + octahedron wire shell + pulsing pip, focus_core.rs:130-183), the 2D arc-label centerpiece at r=150 (focus.rs:134-178), the two-ring radial HUD (context_menu.rs), and the reticle corner brackets (reticle.rs:54-69). The 3D core animates every frame and forces a redraw while focus is active (focus_core.rs:187-204).

## Per-phase change targets

### P1 — Revert 3D focus core + segmented radial ring + clean focus treatment
- **render/focus_core.rs**: archive entire file to `legacy/` (do not delete — repo discipline). Source: spawn_core :130-183.
- **render/mod.rs**: remove `pub mod focus_core;` (:6) and the re-export `pub use focus_core::{animate_focus_core, setup_focus_core_resources, sync_focus_core};` (:34).
- **app/mod.rs**: delete 3 call sites — `setup_focus_core_resources` (:91 Startup), `animate_focus_core` (:163 focus Update), `sync_focus_core` (:179 chained render Update). Delete whole line incl. trailing comma; remaining `.chain()` stays valid.
- **render/theme.rs**: remove `FOCUS_CORE_RING/SHELL/INNER` (:52-54), or repurpose cyan (0.30,0.95,1.0 ≈ #34d6c8) as the single focus indicator-ring colour. Keep `RETICLE_FOCUS` (:49).
- **ui/context_menu.rs `render_radial` :314-429**: replace the floating-text command ring with 6 segmented 60° wedges (filled sector/arc-path per slot, band ~40–72px), number 1-6, centre the verb in its wedge, highlight active wedge by fill not text-recolour (currently the `hot`/amber branch :390-396). Keep angle base `-TAU/4`, the accent/amber/dim Color32 set (:327-339), the backing disc (:345-350), outer paths ring at larger radius. Do NOT touch `command_at`/`command_count`/input handler (:238-289) or ACTIONS (:27-34) — unit tests depend on them.
- **ui/focus.rs**: add exactly ONE indicator ring in `focus_overlay` (:82) gated on `focus_mode.is_some()`, concentric with reticle focus bracket (radius aligned to base 30.0+4.0*pulse, reticle.rs:56); tighten `draw_centerpiece` r=150 (:142) inward; keep dim rect (:114-118) and arc labels.
- **ui/reticle.rs**: keep focus brackets as the surviving frame (no removal); align new ring to bracket size. Note: gate the new ring on focus_mode, NOT on `st.ui.focus` (set by selection too) to avoid double-ringing every selected node.
- **ui/legend.rs**: (chrome) point swatches at shared tokens / assert tokens == render/theme base (legend.rs:18-25,61-63).

### P2 — Overlay anchoring authority + path truncation + tooltip width
- **ui/rail.rs `update_ui_layout` :50-59**: make this the full anchoring authority — shrink `content_rect.max.x` by inspector width when `st.cfg.shell.right_open` (read `right_width` directly; inspector renders after this in-frame), shrink `content_rect.min.y` by top status strip and `.max.y` by telemetry band. Add a helper handing each right/bottom surface a non-overlapping sub-rect. React to `right_open` at runtime (defaults true, config.rs:161, user-togglable).
- **ui/minimap.rs**: re-anchor to inspector-adjusted `content_rect` top; add `.order(layer::PANEL)` (:31-32, currently none).
- **ui/entity_card.rs / ui/node_preview.rs**: route right-column surfaces through one stacked layout owner (minimap top → entity_card OR preview below) inside adjusted content_rect; never two surfaces on same RIGHT_BOTTOM offset (entity_card.rs:48, node_preview.rs:484).
- **ui/node_preview.rs :481-485**: remove the focus-mode `LEFT_BOTTOM` special-case so telemetry owns bottom-left alone (fold focus-mode preview into the entity card per :478 comment).
- **ui/overlay.rs**: make PANEL/READOUT distinct egui Orders or collapse the unused classes (:23-27); promote `place_card` (:51-78) to a content_rect-aware shared corner/edge helper. Wire reticle readout to `layer::READOUT`.
- **ui/help.rs :11-14 / ui/legend.rs :54-59**: add explicit anchor + `.constrain_to(content_rect)` (currently no anchor → egui auto-cascade onto rail/inspector). Set `.order(layer::PANEL)` on minimap, command_palette, help, legend, node_preview.
- **ui/context_menu.rs :442-444**: clamp ctx-menu `fixed_pos` to content_rect (reuse place_card-style clamp; currently unclamped click pos).
- **util/ids.rs (or new util)**: add `middle_truncate(s, max_chars)` — head+"…"+tail, path-aware (keep basename), char-boundary safe (`char_indices`, never byte slice — paths have multibyte/⚠ prefixes).
- **ui/entity_card.rs :76, ui/inspector.rs :116/:121/:144-148**: apply middle_truncate at render (display-time variant; do NOT mutate `node_tooltip_lines` — shared with search). Keep `on_hover_text(full)`.
- **ui/tooltips.rs `render_tooltip` :14-18**: add `ui.set_max_width`, render each line as middle-ellipsis Label + Truncate, full line via `on_hover_text`. Call sites spatial.rs:1051/1110, timeline.rs:473.

### P3 — Minimap radar
- **ui/minimap.rs**: extract pure `minimap_project(world_xz, bounds, rect)->Pos2` + inverse `minimap_unproject` (unit-test roundtrip). Stabilise bounds over the FULL placed set (not visible-only), pad + fixed aspect via `span.max_element()` so no rubber-banding. Compute bounds once, reuse for frustum + dots (currently 2 passes :44-53,:67-82).
- **ui/minimap.rs**: query `&Camera + &GlobalTransform` (+ PanOrbitCamera `target_focus`) instead of just `&Transform`; project 4 frustum corners onto Y=0 via `viewport_to_world`, draw stroked quad; fallback heading-triangle toward target_focus when a corner ray misses the plane (near-horizontal blow-up risk). Use a Camera marker/`get_single` carefully — tests spawn bare cameras.
- **ui/minimap.rs**: add focus/selection marker via `st.spatial.position_of(id)` (state.rs:119) → project → distinct ring (cyan #34d6c8 / crosshair).
- **ui/minimap.rs**: change painter to `Sense::click()`; on `clicked()` run `minimap_unproject` → world X/Z; drive camera via a new position-keyed request (extend GraphState — `request_jump` :1744 is NodeId-only; `apply_jump_to` camera.rs:73 also sets selection, so prefer a position field consumed by apply_jump_to over driving target_focus directly). Optionally snap to nearest dot.
- **ui/minimap.rs**: add LIVE pill + scale hint (world-span from AABB); add `.order(layer::PANEL)`.

### P4 — Edge styling
- **render/edges.rs `update_edge_mesh` :152**: replace 2-vertex LineList with tessellated bezier polyline — sample 8-16 pts along a quadratic/cubic curve, control point offset perpendicular to a→b chord (magnitude scaled by chord length and/or stable per-edge hash so parallel edges fan). Emit consecutive sample pairs as LineList segments (reuse positions/colors buffers + bloom material). Keep fingerprint/rebuild gate.
- **render/edges.rs :265-275**: keep `theme::edge_color` per-class; add directional gradient (brighter at `from`, fading to `to`) and/or a moving bright band phased by `f(time, per-edge hash)` for steady-state flow (reuse glow_duration cadence). Keep the decaying glow pulse (spatial.rs:1209) as the "just happened" accent.
- **render/edges.rs**: read `AggEdge` via `st.core.model.agg_edge(&key)` (model.rs:194); map `stats.count`/`live_count` to brightness (and/or curve thickness) — log/normalise vs running max. Pure viewer-side, no core change.
- **render/edges.rs**: flag threat edges (class AlertsOn, or endpoint Node::Alert, or endpoint has incident AlertsOn — precompute alerted-node `HashSet` once per build, O(E·degree) otherwise); render Alert red (#ff5d5d) boosted so threats pop by default.
- **render/edges.rs `edge_lod` :49 / :264**: replace binary Dim/Full with continuous smoothstep falloff over `mid_dist` between near_dist/far_dist; keep hard Cull past far_dist; quantize falloff input to `cam_cell` so rebuild gate stays cheap.
- **render/edges.rs :119 setup_edge_mesh**: if real thickness required → camera-facing quad strips (topology change here); else encode weight as HDR brightness (no topology change). Pick per MP.
- **render/edges.rs `EdgeFingerprint` :83**: add any new style input (flow phase, weight-max) or rebuilds go stale.
- **config.rs:421 / state.rs:794**: default `lod_edges_mode=FocusOnly` means styled edges are invisible by default — flip to `All` (or All-with-distance-LOD when nothing selected) so the styled web shows; bound via far_dist Cull + fingerprint; confirm vs perf budget first.
- **spatial.rs:1023 hover-edge highlight**: make the highlight follow the new bezier path once edges curve.

### P5 — Node palette + silhouette + state semantics
- **render/theme.rs :16-26**: retune six base colours to MP hexes — PROCESS=#2bb3a8 (reserve #34d6c8 for focus), FILE=#6fe06f, USER=#f5b942, SOCKET=#5fa8ff, HOST=#b09bfb, ALERT=#ff5d5d. Propagates via `base_color` (:149) to cores/shells/rings/gate-glyphs/icon tints (node_glyph.rs:149, node_icon.rs:203, spatial.rs:235/249/261). Edge-class colours (`EDGE_*` :63-68) hardcode their own triples and do NOT auto-follow — retune separately. Verify alert ramp `ALERT_LOW/MEDIUM/HIGH` (:28-30).
- **render/spatial.rs / node_glyph.rs / quality.rs**: decide whether per-type silhouette is always-on by default — the cheap core swap is one mesh, no extra draw cost vs sphere; today tier gate (quality.rs:148-155) suppresses at Potato/Low. Note `sync_node_entities` passes dist=0.0 (spatial.rs:580) so FAR_DIST never suppresses the CORE (only the glyph layer uses real distance) — any "cores fade with distance" assumption is wrong.
- **render/spatial.rs :485-496 `node_material`**: optionally wire `exposure_tint` (:942, currently unused in draw path) into socket/remote-host material if exposure should read by default; else scope P5 to colour+silhouette+severity and leave exposure to radial depth. Aperture Shuttered/Closing reachable but most test data only yields Open/Active/Closing.
- **render/spatial.rs `draw_node_labels` :1235**: "de-collide labels" has no current target — labels are capped at 6, only hovered/selected/focus/compare. If the goal is more always-on labels, this needs a budgeted always-on billboard set with screen-space collision avoidance. Confirm whether the target is this path or gate-glyph/icon overlap.
- Note: Alert core mesh == RemoteHost sphere (node_mesh.rs:31,33); distinguished only by wire shell + colour — if shells dropped, the two silhouettes collapse. `node_kind` defaults unknown id → File (spatial.rs:441,476), rendering as green silhouette after palette change.

### P6 — Force layout de-clump
- **graph/layout.rs `force_step` :514**: decouple repulsion cutoff from spread — raise `repulsion_radius` (grid cell/cutoff :566-571) to ~2.5–3.5× link_distance (e.g. radius≈18–22, link_distance≈8–10) so clustered nodes still repel across the clump, OR add weak sub-linear global centering/anti-gravity. Keep cell≈spacing so grid candidate count stays bounded (watch layout_budget_ms=6ms).
- **graph/layout.rs :602-628 (repulsion), :642-661 (spring), :666-689 (integration)**: make force degree-aware via `GraphCore::model.degree(id)` (model.rs:179, pure fn — no RNG/clock/HashSet-order) — scale per-node repulsion by degree and/or give hubs higher mass (step / (1+log(degree))) / lower spring pull so hubs centre and leaves fan. Must preserve determinism tests (force_step_is_deterministic :920).
- **ui/hud_panels.rs :452-454**: fix repulsion slider range (currently 0.0..=120.0, can't reach 400 default — silently clamps user edits to 120). Set 0.0..=800/1000; add `repulsion_radius`/spread slider (~4..=40) and optionally `spring k` slider. Keep the `changed → layout_settled=false; settle_streak=0; needs_redraw` wake block (:461-464).
- **graph/layout.rs :434-443 `scatter_position`**: decouple initial scatter spacing from `max(repulsion_radius, link_distance)` (use a dedicated `initial_spread` or link_distance alone) so widening the cutoff doesn't inflate the start cloud; keep deterministic.
- **graph/layout.rs :696-707 SETTLE_EPS/SETTLE_FRAMES**: re-tune (0.05/8) against new force magnitudes so the spread layout still converges and the idle short-circuit (:528) still fires; validate with `force_layout_settles_freezes_and_wakes` (:852), confirm `needs_redraw` stays false at rest (no FPS regression).
- **graph/state.rs :556-557,:779-780**: `cfg.radius` (25.0) / `cfg.y_spread` (6.0) are DEAD (zero readers) — wire into new spread/centering (y_spread = obvious Y-flatten knob) or remove; don't leave inert.

### P7 — Entity card 3-block + chrome tokens + rail icons + telemetry
- **ui/tokens.rs `color` :9-32**: rewrite to MP target — BG=05090c, SURFACE=08171c, LINE=1d4a4c, TEXT=cfe9e5, TEXT_DIM=88b8b2, ACCENT=2bb3a8, add ACCENT_HI=34d6c8, ACCENT_GREEN→FILE=6fe06f, SEV_HIGH→ALERT=ff5d5d. ADD missing semantic tokens SOCKET=5fa8ff, USER=f5b942, REMOTEHOST=b09bfb (none exist today). Extend `roles_are_distinct` (:86) for new roles.
- **ui/entity_card.rs `entity_card_overlay` :27/:73-91**: rewrite to 3 visually separated blocks. IDENTITY: title (middle-truncated) + per-kind fields read directly off core `Node` enum match (spacegraph-core lib.rs:36, like node_label_long ids.rs:52) — do NOT parse the flat `node_tooltip_lines`. STATE: kind, origin (+red-team via motion.rs:93 `origin_of`), degree, socket exposure (exposure.rs:28), alert severity (theme::alert_severity_color) + ATT&CK + campaign, fog/revealed (`is_visible_rendered`). CONNECTIONS: port the inspector's de-duped per-class-coloured (`theme::edge_color`) clickable neighbour list (inspector.rs:49-72,132-156) + why-connected path (inspector.rs:77-98,172-181). All sources already exist.
- **ui/entity_card.rs :58-60,:140 `silhouette/kind_name`**: colour silhouette + header + block accents by semantic type (currently ACCENT/TEXT only). Account for icon_tint/gate-glyph transforms when verifying hexes.
- **ui/rail.rs :39-45 SECTIONS / :94-120 rail_button**: replace Unicode-text glyphs (◳/⌕/⚠/⬡/⚙) with painter-drawn vector icons (or icon font in setup_fonts); draw a separate badge pill (top-right) coloured by max severity instead of tinting the whole button red; render active state as left accent bar / fill using ACCENT_HI. Changes button min_size/`.selected(true)` layout — re-verify.
- **ui/hud.rs `hud_overlay` :99-164**: collapse the 8–10 line telemetry dump to a tidy 2-line GitS readout (line1: FPS + visible n/e + tier; line2: event-rate + data-flow + last-activity), keep `◢ TELEMETRY` header; demote raw/agg split, total msgs, last batch, initial-snapshot behind the technician toggle (hud_panels.rs:408). Reuse top-strip one-liner pattern (:61-66).
- **ui/gits.rs :37 `section_header` + ui/theme_egui.rs :31-50**: add a header helper applying `.family(FontFamily::Name(font::HEADER.into()))` (Space Grotesk is registered but NEVER applied — all `◢` headers fall back to mono); route rail/hud_panels/entity_card/hud/legend headers through it. Note `◢/◈/⬡` may not exist in Space Grotesk → per-glyph fallback.
- **ui/gits.rs `panel_frame` :23**: add optional faint horizontal-scanline overlay inside the panel rect (Standard only; Minimal stays flat); expose scanline alpha as a tokens.rs alpha entry. Do NOT confuse with the 3D postfx scanline slider (hud_panels.rs:488).
- **ui/hud.rs :33-39**: route HUD-frame accent + arm/margin magic numbers (alpha 150, arm 26/14, margin 6) through tokens; reuse `gits::draw_brackets` (hud.rs duplicates it) so HUD frame + panel brackets share one impl/accent.
- **ui/legend.rs :18-25,61-63**: point swatches at shared semantic tokens (or assert tokens == render/theme base in a test) so legend/rail/card/3D stay in lock-step.

## Risks / Stop-and-Show triggers

**Telemetry/preview placement decision (Stop-and-Show)** — Folding the focus-mode preview into the right-side entity card vs offsetting telemetry upward is a layout-ownership call; pick before P2 build (node_preview.rs:481-485, hud.rs:84-94).

**content_rect ordering** — `update_ui_layout` is chained first (app/mod.rs:108) but `inspector_overlay` renders AFTER it in the same frame (:117), so reading the inspector's actual rendered width needs `right_width` from config directly or a one-frame lag. `content_rect` is shared by hud_overlay/dispatch/settings windows — shrinking it also re-shrinks those centred windows (probably desired; verify). `right_open` is runtime-togglable + persisted — react at runtime or leave a dead gap.

**egui anchor vs content_rect** — `.anchor` is screen-relative; respecting content_rect means switching to `.fixed_pos`/`.constrain_to`, changing drag/resize for resizable surfaces (right-column ones aren't resizable → low risk). Splitting PANEL/READOUT into distinct Orders may shift existing focus-mode stacking since the `.chain()` is no longer the tie-breaker — re-verify after.

**Palette change lands ONCE (Stop-and-Show)** — Two parallel palettes: `render/theme.rs` (3D scene; read by legend/minimap dots) and `ui/tokens.rs` (egui chrome). P5 retunes theme.rs, P7 rewrites tokens.rs — they MUST reconcile to the same MP hexes or legend/card swatches drift from chrome accent. Edge-class colours (`EDGE_*`) and MP bg/panel/border (#05090c/#08171c/#1d4a4c vs current tokens) don't auto-follow base_color — coordinate so the UI isn't half-recoloured. Changing tokens recolours ALL chrome (rail/HUD/inspector), not just the card.

**Per-type silhouette/state needing data not in graph** — Socket Shuttered/gated aperture is dormant until a D2 firewall source exists (spatial.rs:867-932); most test data only yields Open/Active/Closing. Exposure tint constants exist but aren't wired into the draw path — wiring is a viewer-side decision, no new data. Alert vs RemoteHost share the sphere core (distinguished only by shell+colour) — dropping shells collapses them. Everything else P5/P7 needs (identity/state/connections) is already on the core `Node`/`AggEdge` — no Stop-and-Show for data sources.

**Edge/layout changes needing render-architecture work beyond styling** —
- Real edge *thickness* is impossible on LineList (1px); requires camera-facing quad strips = topology change in `setup_edge_mesh` (edges.rs:119), more vertices/overdraw, interacts with bloom + perf budget. Brightness-as-weight avoids it. **Decide before P4.**
- Continuous edge *flow* via per-frame CPU rebuild defeats the "settled→cheap" `EdgeFingerprint`/cam_cell perf gate (edges.rs:181-184); needs a time uniform/shader or explicit bounding — check perf RUNLOG budget. Standard already ×2.5 HDR + bloom; stacking weight/threat brightness can blow out bloom — clamp/normalise.
- Default `lod_edges_mode=FocusOnly` makes all P4 styling invisible by default — flipping to All increases vertex count; confirm vs perf budget before flipping.

**Layout determinism is a hard contract** — `force_step_is_deterministic` (:920), `force_step_keeps_pinned_fixed_and_deterministic` (:947), `budget_split_matches_full_step` (:1002) fail on any RNG/wall-clock/HashSet-order/non-index-sorted term. Any degree-weighting must be a pure fn of model degree. FPS/idle regression: stronger/longer-range forces that never settle pin the frame loop — keep the idle short-circuit (:528) firing; re-validate `force_layout_settles_freezes_and_wakes` (:852). Widening `repulsion_radius` widens the grid cell → candidate count ~(radius/spacing)³ can blow `layout_budget_ms` — keep cell≈spacing.

**Radial keyboard model is load-bearing** — Change only the painting in `render_radial`; `command_at`/`command_count`/input handler (context_menu.rs:238-289) and ACTIONS (:27-34) drive `radial_commands_map_to_actions` (:586-591) — break them and tests fail.

**node_tooltip_lines is shared (do not mutate)** — Card + inspector + hover tooltip + search consume it; inject truncation/blocks via a display-time helper and read structured fields off `Node` instead, or you corrupt the tooltip/search paths. Middle-ellipsis must be char-boundary safe (multibyte paths, ⚠ prefix) — `char_indices`, never byte slice.

**Card action surface duplication (Stop-and-Show)** — Card today has inline button rows (entity_card.rs:93-115); the polish MP introduces the segmented radial action ring. Confirm whether the card keeps inline buttons or defers actions to the ring so the two don't duplicate.

**Minimap framing + camera query** — Stabilising AABB to the full placed set (not visible-only) changes current fog-of-war zoom framing — confirm full-world vs visible-only with MP. Multiple Camera entities exist at runtime (tests spawn bare cameras) — marker/`get_single` carefully or panic. Frustum corners on near-horizontal camera shoot to infinity — needs bounded heading-triangle fallback. Click-to-fly needs a position-keyed camera path (`request_jump` is NodeId-only); prefer extending GraphState over driving `target_focus` directly (bypasses fly-to easing).

**Minimal theme degrade must survive** — Every new chrome element (scanline, header font, badge pill, drawn icons, focus indicator ring) needs a Minimal fallback or the behavioural-equivalence baseline breaks (theme_egui.rs:126). The clean-focus indicator ring must respect the Standard-only gate so Minimal stays plain dim+centre.