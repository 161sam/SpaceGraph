# SpaceGraph — Visual Design Language

Binding reference for the viewer's visual identity. The single source of truth
for colours is `crates/spacegraph-viewer/src/render/theme.rs`; this document
explains the intent. Aesthetic target: **"Ghost in the Shell" cyberspace** —
dark space, emissive neon elements, additive glow (HDR + bloom), fine grid,
data-dense but legible. Original design, no copied assets.

## Themes

Selectable via `cfg.visual_theme` (`viewer.toml` / settings panel):

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
| Host / Container | violet | `theme::HOST` (Phase 7) |
| Alert / threat | red | `theme::ALERT` (Phase 8) |

Edge classes:

| Edge | Colour | Const |
|---|---|---|
| `opens` | green | `theme::EDGE_OPENS` |
| `execs` | cyan | `theme::EDGE_EXECS` |
| `runs_as` | amber | `theme::EDGE_RUNS_AS` |

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

## Implementation status (Phase 5)

Implemented: themes + `theme.rs` palette, HDR + bloom camera, per-type emissive
node ramps with decay, dark-space background + floor grid, recent-activity edge
pulse, capped billboard labels, timeline palette.

Deviation (see `docs/perf/RUNLOG.md`): edges are drawn as **HDR gizmos coloured
by class**, not yet mesh polylines. Gizmo lines render reliably; the
mesh-polyline upgrade (full bloom participation, alpha/animation) is the planned
next visual iteration and is deferred because it cannot be validated in this
headless build. Screenshots for the Phase 5 gate are produced locally with
`cargo run -p spacegraph-viewer -- --demo-load 2000`.
