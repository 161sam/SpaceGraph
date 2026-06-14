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
