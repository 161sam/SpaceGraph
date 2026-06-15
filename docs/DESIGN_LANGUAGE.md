# SpaceGraph — Visual Design Language

Binding reference for the viewer's visual identity. The single source of truth
for colours is `crates/spacegraph-viewer/src/render/theme.rs`; this document
explains the intent. Aesthetic target: **"Ghost in the Shell" cyberspace** —
dark space, emissive neon elements, additive glow (HDR + bloom), fine grid,
data-dense but legible. Original design, no copied assets.

## Themes

Selectable via `cfg.visual_theme` in `viewer.toml` (no in-app theme selector yet
— see the recon report finding F1):

- **Standard** — the neon look: HDR camera + bloom, per-type emissive
  materials, dark-space background, floor grid, recent-activity pulses.
- **Minimal** — flat fallback for accessibility / low-end GPUs: no bloom, plain
  materials, flat dark background. Behaviourally equivalent to the
  pre-visual-pass viewer (verified: `minimal_theme_uses_flat_materials`).

## Colour semantics

Node types (emissive base colour; HDR emissive channels exceed 1.0 to bloom):

| Element | Colour | Const |
|---|---|---|
| Process | cyan | `theme::PROCESS` |
| File | green | `theme::FILE` |
| User | amber | `theme::USER` |
| Socket | blue | `theme::SOCKET` |
| RemoteHost | violet | `theme::HOST` |
| Alert / threat | red | `theme::ALERT` (severity ramp: `ALERT_LOW`/`MEDIUM`/`HIGH`) |

Selection/interaction feedback colours: `RETICLE_HOVER`/`RETICLE_SELECT`/
`RETICLE_FOCUS`, marked nodes `MARKED`, pinned `PINNED`, hovered edge `EDGE_HOVER`.

Edge classes:

| Edge | Colour | Const |
|---|---|---|
| `opens` | green | `theme::EDGE_OPENS` |
| `execs` | cyan | `theme::EDGE_EXECS` |
| `runs_as` | amber | `theme::EDGE_RUNS_AS` |
| `owns_socket` | blue | `theme::EDGE_OWNS_SOCKET` |
| `connects_to` | bright blue | `theme::EDGE_CONNECTS_TO` |
| `listens_on` | teal | `theme::EDGE_LISTENS_ON` |
| `alerts_on` | red | `theme::ALERT` |

Scene: near-black space (`CLEAR_STANDARD`), faint grid lines (`GRID_LINE`).
Timeline event markers reuse the palette (`TL_*`): node upsert green, node
remove red, edge upsert cyan, edge remove amber, batch neutral.

## Node geometry (v0.4.0)

In the **Standard** theme each node kind has a distinct silhouette — type is
readable from shape, not only colour. Each is a solid emissive **core** (lit,
recency glow stays on the core) plus, for some kinds, a holographic **wireframe
shell** (`LineList`, unlit HDR emissive so it blooms). Built in
`render::node_mesh` from Bevy primitives + `Mesh::new` wireframes (no new deps).
Cores stay within a ~0.30 envelope (Alert slightly larger) so the
bounding-sphere pick (`PICK_RADIUS = 0.5`) still covers them.

| Kind | Core | Shell | Reads as |
|---|---|---|---|
| Process | octahedron | — | active compute core |
| File | thin hexagonal prism | — | passive data plate |
| User | upward cone | — | identity / authority |
| Socket | small torus | — | port aperture |
| RemoteHost | small sphere | octahedron wire | distant station |
| Alert | sphere | spiked star wire | threat (blooms hardest) |

The **Minimal** theme keeps the flat sphere for every kind and draws no shell —
behaviourally identical to the pre-geometry viewer
(`minimal_theme_uses_sphere_mesh_and_no_shell`). A theme switch triggers exactly
one entity rebuild (`theme_switch_triggers_exactly_one_rebuild`); steady state
mutates `Transform`/material/mesh handles only — no entity churn.

### Orbital rings (v0.4.0)

Hubs and threats get a rotating emissive **orbital ring** (a torus child entity,
per-kind unlit material). A visible node qualifies when its degree is at least
`cfg.ring_min_degree` (default 6, from the prebuilt adjacency — no per-frame edge
scan) **or** it is an Alert (alerts always, and spin faster). `sync_node_rings`
spawns/despawns ring children to match qualification (bounded by the live node
set, no steady-state churn); `rotate_node_rings` animates them (visual-only,
determinism-exempt). Standard-only (`cfg.node_rings`, default on); Minimal draws
no rings.

### Interaction feedback (v0.4.0)

- **Grab-to-pin:** left-drag a node to reposition it; it pins (clamped by the
  layout) and shows a dimmed marker (`theme::PINNED`). Left-drag on empty space
  still box-selects.
- **Edge picking:** hovering near an aggregated edge highlights it
  (`theme::EDGE_HOVER`) with a class/endpoints/count tooltip; clicking it selects
  the target and anchors the "why connected" compare on the source.
- **Radial context menu:** right-click a node (a click, not an orbit drag) for
  Fly-to / Isolate / Trace / Pin / Mark / Inspect.
- **Marks:** marked nodes carry a persistent magenta tint (`theme::MARKED`).

## Cyberspace post-process (v0.4.0)

A fullscreen post pass (`render::postfx`, WGSL in
`assets/shaders/cyberspace_post.wgsl`) adds **scanlines + vignette + chromatic
aberration + film grain** after Tonemapping/Bloom — the final "screen" layer of
the Ghost-in-the-Shell look. Standard-theme only and toggleable
(`cfg.postfx.enabled` + per-effect intensities, persisted); `postfx_active`
forces it off under Minimal without clobbering the saved config. The pass only
runs when its per-camera `PostFxSettings` is attached (`sync_postfx`).

## Node detail — two-level model (v0.4.1)

Nodes carry type-specific detail without per-node cost, via a deliberate split.

**Level 1 — node-face icon (every visible node, cheap).** A monochrome glyph on
each node's camera-facing quad, sampled from a *single shared* atlas
(`assets/icons/atlas.rgba`, a 4×4 grid of 64px cells; reproducible via
`gen_atlas.py`). The per-instance glyph lives in per-cell quad-mesh UVs, so all
nodes of a kind share one atlas + one material and Bevy GPU-instances them; an
alpha **mask** keeps icons in the opaque pass (no per-frame transparency sort).
Type from the node kind, file subtype from the path extension
(image/video/text/code/json/log/audio/archive/binary). Standard theme only; Low
capability uses a flat colour variant; Minimal draws none. (This icon is the
*centre* of v0.5.0's gate-glyph unit.)

**Level 2 — focused-node preview (focused + ≤ `max_preview_panels` pinned,
bounded).** The focused node renders rich type-dispatched content in a framed
egui panel (dark fill + neon stroke — the "screen frame"): image → thumbnail;
text/code/json/log → monospace head; process → terminal-styled **read-only**
readout; video/audio/archive/binary → card; user/socket/host/alert → type card.
Content is read viewer-locally (path policy + size caps), decoded **off-thread**
and LRU-cached — cost is **O(focused)**, never O(visible). Hover peeks a card; a
focus change fires a decaying ripple; double-click expands the preview.

**Deliberate boundaries.** No live video decode (card + metadata; a decoder is a
later pass). No interactive terminal in a node — the read-only readout is the
*look*; the real terminal is the v0.7.0 AdminBot control plane behind the
approval layer. Detail scales to the GPU class (`render::capability`, the v0.5.0
`QualityTier` precursor): Low (Pi/GLES) → colour icons, text-only/off previews,
no image decode.

## Motion & recency

- **Recent activity glow:** on upsert/touch a node flashes toward white
  (`RECENT_GLOW`) and decays back to its type colour over `glow_duration`. In
  the Standard theme this is a per-type emissive ramp (`GLOW_LEVELS` steps)
  driven by the decay fraction — bright flash → steady neon — so the strength,
  not just a binary swap, encodes recency.
- **Edge pulse:** a bright dot travels along a glowing edge from source to
  target as the glow decays (shader-less; one gizmo dot per active edge).
- **Layout:** force-directed, deterministic; nodes ease into place (capped
  `max_step`/frame). No randomised motion.

## Typography & labels

- In-scene labels are **billboarded and capped**: only the focused / hovered /
  selected nodes are labelled (≤ 6), never all nodes. Projected to screen via
  egui in a light cyan-white (`rgb(200,230,255)`).
- HUD and tooltips: egui default proportional font; tooltips show name + ID and
  the "why connected?" path.

## Lock-on reticle & in-world readout (v0.4.0)

In the **Standard** theme, single-node selection/hover/focus is shown with an
in-world **reticle** (`ui::reticle`) instead of gizmo bubbles: animated corner
brackets framing the projected node (`theme::RETICLE_*` colours; the sweep
animates off the egui clock, visual-only), and for the selection a leader-lined
monospace **readout** box (`node_label_long` + recency). The **Minimal** theme
keeps the gizmo bubbles for parity — chosen by the pure function
`render::spatial::highlight_style(theme)`. Multi-select (box-select) bubbles show
in both themes.

**Micro-tags** (Standard, `cfg.micro_tags`, default on) label the `micro_tag_max`
(default 24) nearest nodes within a radius with a distance-faded compact id —
bounded by count, never all nodes (`nearest_micro_tags`).

## Rules (binding)

1. New visual elements add a constant to `theme.rs` — no ad-hoc `Color::srgb`
   literals in render code.
2. Every effect must degrade to the Minimal theme without changing graph state
   or behaviour (visuals never mask truth — AGENTS.md §1.2).
3. Bloom only on emissive elements; the background and grid stay dark so neon
   reads against space.
4. Labels and pulses are bounded (capped counts) — never O(N) text or O(E)
   per-frame allocation.

## Implementation status (v0.4.0)

Implemented: themes + `theme.rs` palette, HDR + bloom camera, per-type emissive
node ramps with decay, **per-type node geometry** (cores + wireframe shells),
**orbital rings** on hubs/alerts, dark-space background + floor grid,
recent-activity edge pulse, capped billboard labels, **lock-on reticle + readout
+ micro-tags**, **cyberspace post-FX** (scanlines/vignette/aberration/grain),
timeline palette.

**v0.4.1** adds the **two-level node detail** above: Level-1 shared-atlas face
icons on all nodes (`render::node_icon`) and Level-2 off-thread, LRU-cached
focused-node previews (`ui::node_preview`) + focus ripple / hover-peek /
double-click-expand (`render::interaction`), GPU-scaled via `render::capability`.

**Edges** now render as a single **batched HDR `LineList` mesh** (`render::edges`,
`setup_edge_mesh`/`update_edge_mesh`) with per-vertex HDR colours — full bloom
participation. The raw-edge fallback and the recent-activity pulse remain gizmos.
(The earlier "edges as gizmos, mesh deferred" deviation is resolved.)

Screenshots / GPU capture remain a local step (the build env is headless):
`cargo run -p spacegraph-viewer -- --demo-load 2000`.

## v0.5.0 — GitS UX-shell, quality tiers, radial HUD

### Quality tiers (the cost axis)
A `QualityTier {Potato, Low, Medium, High}` axis (`render::quality`) **orthogonal**
to `VisualTheme`. Auto-detected from the GPU adapter, runtime-adaptive (FPS
feedback with hysteresis), manually overridable. **GitS-at-low-cost split:**
tier-*independent* (neon palette, gate-glyphs, reticle, radial HUD, dive ripples,
rand-frame, palette, query-DSL) vs tier-*gated* (HDR bloom, post-FX, MSAA, 3D
silhouettes + orbital-ring meshes, node budgets). So a Raspberry Pi at `Potato`
still reads as Ghost-in-the-Shell. `Minimal` forces the cheapest path at any tier.

### egui design tokens + GitS reskin
`ui/tokens.rs` (neon-on-black colour roles, spacing, font roles) + `ui/theme_egui.rs`
(GitS `Visuals` in Standard, plain dark in Minimal; embedded OFL fonts — Inter
body, JetBrains Mono mono, Space Grotesk headers, committed under `assets/fonts/`).
IDE shell: resizable/toggleable left operator rail (tuning demoted into a collapsed
**Technician** section), right-docked inspector, `[shell]`-persisted.

### Gate-glyph node LOD (`render::node_glyph`)
A billboarded concentric-ring `LineList` glyph (unlit emissive, per-kind,
instanced) on every visible node in Standard — with the v0.4.1 face icon as its
centre, one **gate unit**. Primary at Potato/Low (3D silhouette suppressed),
far-LOD at Medium/High (per-kind silhouette near). Minimal draws no glyph.

### Radial command HUD (`ui/context_menu`)
`F` opens a keyboard-driven concentric ring HUD on the focused node: inner ring =
command verbs (`CtxAct`), outer ring = neighbour paths (paged by 9). Tab/↑↓ switch
ring, ←→ rotate, `[`/`]` page, 1–9 select, Enter execute, Esc close; a path dive
re-centres focus (keyboard graph traversal). egui painter, tier-independent.

### Dive ripples + HUD rand-frame
Decaying focus/alert ripples (`render::interaction`); a peripheral rand-frame
(`ui/hud::hud_frame_overlay`) — corner brackets + a live strip (agents, alert
severities, mode, FPS, active tier) hugging the viewport margins.

### Command palette + query-DSL
`Ctrl/Cmd+P` fuzzy command palette (actions + navigation + nodes; in-house
subsequence matcher). The filter is a **query-DSL** (`graph::query`):
`type:/kind:/host:/sev:/name:/path:/deg:>N/recent:Nm`, bare-word substring,
`-` negation, implicit AND — rendered as removable chips (red chip on malformed).

### Controls (v0.5.0)
`F` radial HUD · `Ctrl+P` command palette · query-DSL filter box · Display
selectors (theme + quality tier) in the left rail.

## v0.5.1 — Focus Mode, gate-ring polish, edge-LOD

### Focus Mode (the headline)
`F` / double-click enters **Focus Mode**: the camera eases the node to
screen-centre + close, the rest of the graph **dims on all tiers** (depth-of-field
blur is a High-tier enhancement, deferred), and the **force layout freezes** while
focused (reversible; `force_step` byte-unchanged). The node becomes the centerpiece
— prominent concentric **gate-rings + identity arcs** (kind / connections / id),
the v0.4.1 type-preview rendered **as the centre**, and the v0.5.0 radial command
ring symmetric around it. The keyboard radial model drives it; a **path dive**
re-centres focus on a neighbour (cinematic graph traversal). `Esc` exits (eased
camera return). **Minimal** degrades to a plain dim+centre (no rings/arcs/DoF).
Cost is O(1) — one focused node + one dim rect; no per-visible-node entity.

### Gate-ring polish
The shared gate-glyph `LineList` (one mesh per node, instanced) gains outer
**tick-marks** (cardinals longer) for the "gate" read, and alerts ring in the
**severity ramp** (low = amber, medium = orange, high = red) via shared materials
(`render::node_glyph::ring_color`). The radial HUD gets a dim backing disc so
labels read against a busy graph. Field glyphs stay static (billboarded) — animated
ring rotation is reserved for the O(1) focused centerpiece, to preserve the
reactive idle-pacing (a continuously-spinning field would never go idle).

### Edge-LOD (the FPS lever)
Edges are thinned render-side (`render::edges::edge_lod`): distant edges **dim**
then **cull** by camera distance (discrete bands, camera-cell-quantized so the mesh
rebuild stays bounded — "settled → cheap" preserved), and in Focus Mode only the
focused node's incident edges draw. Cuts overdraw/bloom on large graphs, where edge
fill-rate dominates. `force_step` (layout truth) is untouched. The `[edge_lod]` and
`[focus]` config blocks tune the distance bands, dim factor, and freeze/dim.

### Face-icon fix
The Level-1 face icons now cut cleanly — an explicit **nearest** sampler makes the
alpha mask yield a crisp glyph instead of a filled quad — and are **clamped to the
node's scale** (`icon_half_extent` ≤ the per-kind core envelope) so the glyph sits
*on* the node face rather than overhanging it as a block.

### Controls (v0.5.1)
`F` / double-click **Focus Mode** · `Esc` exit focus · in focus: `1`–`9` select,
`Tab`/`↑↓` switch ring, `←→` rotate, `[` `]` page, `Enter` execute, path dive
re-centres.

## v0.5.2 — Filesystem search (`IN GRAPH` vs `ON DISK`)

The Ctrl+P search surface merges two result classes, **visually distinguished**
so the user always knows what they are picking:

- **`IN GRAPH`** — a node already loaded in the graph (instant, in-memory).
  Tagged in a mint/green accent. Picking jumps the camera to the node.
- **`ON DISK`** — a filesystem index hit from the agent (async, debounced).
  Tagged in a cyan/blue accent. Picking **materialises** the path into a node
  (a single bounded `File` node) and flies to it once the agent streams it in.

Binding rule: `index ≠ graph`. An `ON DISK` row is a *pointer*, never a node,
until picked — so the search box can surface the whole filesystem while the graph
stays bounded. When no connected agent advertises `fs_search`, the surface shows
`IN GRAPH only` and the `ON DISK` section is absent (graceful v3 fallback). A
capped result set shows a "results capped — refine the query" hint.

## D0 — Perimeter & exposure (ADR-0012)

Four viewer-side derivations make the network surface legible with **no wire
change and no new data** — all derived from fields already on `Node::Socket`
(`state`, `local_addr`) and reusing `Node::RemoteHost`. All key off `theme.rs`
constants; all degrade to Minimal without changing graph truth.

- **Port-state-as-aperture** (`render::spatial::aperture_style`, Standard only).
  A socket renders by state: **LISTEN** → an open, bright outward aperture;
  **ESTABLISHED** → the active socket blue; **gated/filtered** → a dimmed,
  shuttered form behind a barrier ring (dormant until the D2 firewall source emits
  the state); **closing** (TIME_WAIT/CLOSE_WAIT/…) → dimmed. Idle sockets show the
  aperture tint; recent activity still flashes white via the glow ramp. Minimal
  keeps the flat torus.
- **Exposure-as-depth** (`render::spatial::exposure_bucket`, both themes). A
  socket's `local_addr` buckets to `Loopback` / `Lan` / `Public`, driving radial
  shell depth — **Public** on the outer shell facing the perimeter, **Lan** mid,
  **Loopback** at the core. Attack surface reads as *silhouette*. This positions
  truth (not decoration), so it applies in both themes.
- **Anomaly-as-scene-distortion** (`render::postfx`, Standard only). The most
  severe/recent alerts (bounded top-N, screen-projected) localize the post-fx — a
  proximity ramp that desaturates toward a danger wash and pulses — so the eye is
  drawn to *where* it is wrong. Off under Minimal (`postfx_active`); never clobbers
  saved config.
- **Gateway as a derived node.** The default-route gateway (read from
  `/proc/net/route` by the `net` source) appears as a `RemoteHost` on the outer
  shell — the egress hub outbound traffic sits near. Reuses the existing kind; no
  new type, no wire bump. (At D0 it is positional; the internet-membrane *region*
  it anchors is D4/ADR-0008.)

New `theme.rs` constants: `APERTURE_OPEN/ACTIVE/SHUTTERED/CLOSING`, `BARRIER_RING`,
`GATEWAY_ACCENT`, `EXPOSURE_LOOPBACK/LAN/PUBLIC`. Toggles: `[socket_display]`
`aperture_by_state` / `exposure_depth` / `anomaly_focus` (+ `anomaly_intensity`),
all default on.

## D2-core — Threat-motion + purple-team origin (ADR-0009)

- **Threat-motion** (`render::motion::motion_style`, Standard only). Each attack
  class moves by its ATT&CK **tactic**: C2 → beacon pulse, lateral movement →
  traversal sweep, exfiltration → outbound flow, credential access → rapid flash,
  execution/impact → worm-spread (others → calm pulse). Each `MotionStyle` carries
  `(speed, amplitude)` constants — no magic numbers in render. Minimal forces
  `Static` (`motion_style_themed`); motion never changes graph truth.
- **Purple-team origin** (`render::motion::origin_of`). Entities from a red-team
  feed (a stream named `nebula-*` / `red-team-*`) are tagged `[red-team]` in the
  inspector and styled distinctly in Standard, vs. `observed` for live telemetry —
  so authorized engagements don't read as real threats. Derived viewer-side from
  the emitting stream; **no wire field** (O-8). Minimal degrades to neutral.
- **Nebula source** is observe-only (existing kinds, read-only log tail); its log
  schema is an assumption to verify on the operator's host (A.5).

## D5 — ATT&CK coverage + posture (ADR-0006 §3)

- **Coverage heatmap** (`graph::coverage`): a Navigator-style grid, tactic-grouped,
  marking each vendored technique **detected** (a rule maps to it) or a **gap**.
  Honest by construction — an unmapped technique (e.g. T1041) shows red. Computed
  read-only from the rule registry; no live ATT&CK fetch (O-7').
- **Posture / exposure score** (`graph::posture`): a deterministic 0..100 risk
  read-out from public-facing listeners + alert density, amplified by the coverage
  gap. Surfaced in the HUD/posture view; the components explain the number.

## MP-UI-GitS — 2D chrome visual language (viewer-only)

Extends the 3D scene aesthetic above to the **egui chrome**. Holographic
technical readouts over a full-width scene, not flat widget lists. Standard =
full GitS; **Minimal = flat** (plain egui, no glow/brackets) — every rule below
degrades to the Minimal equivalent.

- **Tokens** (`ui/tokens.rs`, extended): the egui-side palette
  (`color::{BG,SURFACE,SURFACE_HI,LINE,ACCENT,ACCENT_GREEN,TEXT,TEXT_DIM,SEV_*}`),
  spacing (`space::XS..XL`), font roles (`font::{BODY,MONO,HEADER}`), plus
  `radius`, `stroke_w`, and `alpha` (translucent-surface opacities). Single source
  of chrome values — converge literals onto them, don't fork.
- **GitS chrome** (`ui/gits.rs`): `panel_frame(standard)` = translucent dark fill
  (`alpha::PANEL_FILL`, scene reads faintly through) + hairline neon stroke +
  flat rounding; `draw_brackets` / `bracket_response` = the ┌ ┐ └ ┘ corner
  accents; `section_header` = monospace accent labels (`◢ TITLE`). Minimal →
  plain egui popup frame, no brackets.
- **Layout model**: the 3D graph renders **full-window**; chrome **floats over
  it** (holographic HUD), it does not reserve a column. A slim always-on
  **command rail** (`ui/rail.rs`, left edge: VIEW·FILT·ALRT·AGNT·CFG) toggles one
  **corner-anchored HUD panel** (`ui/hud_panels.rs`) at a time. The top **status
  strip** (`hud_frame_overlay`: AGENTS/ALERTS/MODE/FPS/TIER) + edge corner
  brackets are the persistent frame; the telemetry readout docks bottom-left.
  `update_ui_layout` publishes `content_rect` = screen minus the rail.
- **Panel-layer / z-order authority** (`ui/overlay.rs`): a single owner of overlay
  draw-order and node-anchored placement, so surfaces never stack on the node.
  - `layer::{BACKDROP=Background, PANEL/READOUT=Middle, MODAL=Foreground}` — the
    canonical egui `Order` per overlay class (no ad-hoc `.order()`).
  - `place_card(node, node_half, size, viewport, gap)` — the one pure, unit-tested
    anchoring rule: place a card/readout **beside** a node (prefer right, flip
    left near the edge, vertically centre, clamp fully on-screen); clears the node
    footprint. Reused by the hover tooltip, reticle readout, and entity card.
    `node_half = 0` = a clamped pointer anchor.
  - **Mutual exclusion**: the hover tooltip is suppressed while a modal owns the
    node region (Focus Mode / context menu); entering Focus closes the context
    menu / palette / search. The Esc cascade is preserved.
- **Focused node — layered core** (`ui/focus.rs`, Standard): concentric core rings
  + tick marks + a wireframe-octagon schematic + a `◤ FOCUS ◥` tag + radial
  kind/links/identity labels, screen-space over the projected node (the
  centerpiece/reticle/glyph paradigm — **not** new scene geometry). Static (no
  per-frame animation). Minimal = plain dim.
- **Entity card** (`ui/entity_card.rs`): in Focus Mode, a framed GitS card (type
  silhouette + identity fields + origin + connections + Fly-to/Pin-compare),
  corner-anchored bottom-right via the `place_card` model — never overlaps the
  node; the docked inspector is suppressed in focus so they don't duplicate.

### Rules (binding, 2D chrome)
- Glow/detail serve legibility, never clutter; gate GitS decoration on the
  Standard theme — **never** gate a control itself (Minimal keeps full reachability).
- One owner of z-order + anchoring (`ui/overlay.rs`); new overlays pick a
  `layer::*` class and (if node-anchored) use `place_card` — no ad-hoc `.order()`
  or hand-rolled offsets.
- Chrome floats; the scene stays full-width. No permanent space-reserving dev
  sidebar.

## MP-UI-GitS-polish — focus ring, minimap, edges, nodes, layout, chrome

The polish pass refines the GitS chrome to the approved mockup. The single colour
source stays `render::theme` (3D scene) mirrored by `ui::tokens` (egui chrome) — both
on the **MP palette**: Process `#2bb3a8` (focus reserves `#34d6c8`), File `#6fe06f`,
User `#f5b942`, Socket `#5fa8ff`, RemoteHost `#b09bfb`, Alert `#ff5d5d`; bg `#05090c`,
panels `#08171c`, border `#1d4a4c`, text `#cfe9e5`/`#88b8b2`. Per-type colours are
**semantic and show in the default view**, not only on hover.

- **Focus treatment** (`ui/focus.rs`, `ui/reticle.rs`): the 3D focus-core rig is
  retired (archived to `legacy/render/focus_core.rs`). The focused node shows its own
  per-type silhouette framed by the reticle corner brackets + **one** thin indicator
  ring, with a `◀ FOCUS ▶` tag + `kind · 0xHEXID` subtitle. No data floats over the
  node. Minimal = plain dim.
- **Segmented action ring** (`ui/context_menu.rs::render_radial`): the 6 actions
  (fly-to · inspect · trace · isolate · mark · pin) are **arc-segment wedges** evenly
  at 60° from the top, numbered, with the keyboard-cursor / pointer-hovered wedge
  highlighted; a faint inner tick gauge. Pure geometry: `segment_center_angle`,
  `segment_at`. No floating labels.
- **Minimap** (`ui/minimap.rs`): a live radar — real projected node positions
  (type-coloured) over stable padded-square bounds, a camera **viewport frustum**, a
  **focus marker**, and **click-to-fly** (`minimap_project`/`minimap_unproject` pure +
  inverse). LIVE pill + scale hint + corner brackets.
- **Edges** (`render/edges.rs`): **curved** (quadratic bézier, per-edge-hash bow so
  parallels fan), **per-class coloured** (aligned to the node palette), continuous
  **distance falloff** (`edge_falloff`), **directional gradient**, **weight →
  brightness** (`weight_brightness`, the LineList thickness proxy), and **threat-red**
  for alert-incident edges. Settled→cheap rebuild gate intact.
- **Nodes** (`render/spatial.rs`): the per-type **core silhouette is always on** in
  Standard (a single mesh, no costlier than the sphere) — type reads from shape on
  every tier; the wireframe shell stays tier-gated. Capped labels are de-collided
  (`overlay::decollide_labels`).
- **Layout** (`graph/layout.rs`): a wider bounded-density repulsion reach de-clumps,
  and a degree-aware integration **mass** (`node_mass`) anchors hubs while leaves fan
  (hubs-vs-leaves hierarchy). Still converges + freezes (no idle cost).
- **Panel-layer** (`ui/overlay.rs`, `ui/rail.rs`): `update_ui_layout` is the content_rect
  authority (rail + top strip + inspector column reserved); panels anchor via
  `corner_anchor` / `constrain_to(content_rect)` so no two share a corner. **Middle-
  ellipsis** (`middle_truncate`) on every long path/label, full value on hover.
- **Chrome** (`ui/gits.rs`, `ui/entity_card.rs`, `ui/rail.rs`, `ui/hud.rs`): the entity
  card is a three-block readout (identity / state / connections) with a type glyph,
  hex-id, live dot, segmented meter and a clickable connection list; the rail has
  painter-drawn vector icons + active accent bar + severity badge; "screen" panels get
  a faint **scanline** sheen + corner brackets; telemetry is a tidy 2-line readout.
- **Binding additions:** every new overlay anchors via `overlay::corner_anchor`/
  `place_card` + `constrain_to(content_rect)` (never a bare screen corner); long text
  uses `middle_truncate`; node-anchored labels pass through `decollide_labels`;
  per-frame animation stays focus-only / visible-only (no idle cost).
