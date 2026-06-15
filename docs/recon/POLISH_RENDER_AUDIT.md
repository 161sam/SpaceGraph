# MP-UI-GitS-polish — Render-vs-Claim Audit (real GPU, forced tier)

**Verified build:** `feat/ui-gits-polish` HEAD, debug binary, run on the **real GPU**
(`Intel(R) HD Graphics 520 (SKL GT2)`, IntegratedGpu, **Vulkan** backend, Mesa 25.2) on
`DISPLAY :0`. Evidence shots in `docs/media/gits/audit/`.

## The tier-detection discovery (why the prior captures lied)
The Intel HD 520 is a real GPU but **weak** — `render::quality::detect_tier` auto-classifies
it as **`Potato`**, and `adaptive_quality` then steps the *effective* tier down further to
hold FPS. So the prior `afterp*` captures rendered at **TIER POTATO**, which gates **off**
exactly the tier-dependent 3D layer: **HDR bloom, post-FX, orbital rings, wireframe shells**.
Green pure-fn tests + a Potato capture = the fidelity gaps were invisible.

**To audit the real render** the config must force it: `[quality] tier = "high"` **and**
`adaptive = false` (tier alone is overridden back down by the adaptive stepper). With both,
the HUD reads `TIER HIGH` and bloom/post-FX/rings/shells render — see
`audit-HIGH-focus.png` / `audit-HIGH-default.png`.

## Per-feature verdict (at `feat/ui-gits-polish` HEAD, forced TIER HIGH)

| # | Claimed gap | Verdict at HEAD | Evidence / root cause |
|---|---|---|---|
| 1 | Segmented ring not shown; old overlapping labels, numbering 1,6,2,5,4 | **Already correct** | `audit-HIGH-focus.png`: 6 numbered wedges 1–6 clockwise (fly-to·inspect·trace·isolate·mark·pin), no floating overlap. `render_radial` is the only radial painter; the old float-label path was replaced in P1, no fallback. |
| 2 | Large focus **sphere/eye** dominates ("full wireframe sphere at some angles") | **Not present at HEAD** | `focus_core` (the P5 gyroscopic-rings + octahedron — *that* is the wireframe sphere) is reverted to `legacy/render/focus_core.rs` and fully unwired (grep: only a doc-comment remains). What renders on the focused node at HEAD is the much smaller **gate-glyph** concentric rings + indicator ring (a subtle "target", not a sphere). |
| 3 | Entity card still the old **flat** card | **Already correct** | `audit-HIGH-focus.png`: 3-block card (IDENTITY/STATE/CONNECTIONS) + type glyph + hex-id + live dot + degree meter + clickable connections. `entity_card_overlay` is the only card, rewritten in P7, no fallback. |
| 4 | Telemetry/preview **overlap** in focus | **Already correct** | `node_preview_overlay` early-returns in Focus Mode (P2); the focus shot shows telemetry alone bottom-left, no preview. |
| 5 | Focus header **kind mislabel** (`file` for a `/socket` node) | **Cannot occur at HEAD** | `theme::NodeKind::of` maps `Node::Socket → Socket` exactly; `focus.rs` reads it directly. (The demo has **no** socket nodes, so a socket can't even be focused here — this claim is from real data on a different build.) |
| 6 | Stray **vertical axis line** | **Not reproduced** | Not visible in `audit-HIGH-default.png`; no axis gizmo in the focus/scene path. |
| 7 | Layout collapses to a **central cluster** | **Spread at HEAD** | `audit-HIGH-default.png`: nodes distributed, not a central blob (P6 + the spread defaults). |
| 8 | Minimap dots + frustum | **Correct** | type-coloured dots + frustum + focus marker visible. |
| 9 | Per-type colours / **monochrome green** | **Partly real (structural)** | See below — the one genuine residual. |

## Conclusion
**8 of 9 claimed gaps are already fixed at `feat/ui-gits-polish` HEAD.** Every one of them
(old radial, big wireframe sphere, flat card, telemetry/preview overlap, central-cluster
layout) is the exact state of **`main` / the merged MP-UI-GitS overhaul** — which still has
`focus_core`, the float-label radial, the flat card, the overlap, and the pre-P6 layout.
**The strong hypothesis is that the reviewed binary was `main` (or a stale build that didn't
pick up the branch), not this branch.** The forced-HIGH captures here render the correct
new chrome.

## The one genuine residual — "monochrome green" overview
Not a bug in the colour code: `node_glyph::ring_color` and the cores use the correct per-kind
`base_color()`. It is structural —
- the synthetic demo is **~59% File + ~40% Process + ~1% User, with no Socket/RemoteHost/Alert
  nodes** (`graph/synthetic.rs`), so the only colours present are green / teal / a few amber;
- after P5 the MP palette puts **Process = `#2bb3a8` (teal)** right next to **File = `#6fe06f`
  (green)** — both green-family — and HIGH-tier **bloom** brightens them toward the same
  green-white, so File and Process are not distinguishable at the overview zoom.

Real agent data (sockets blue, hosts violet, alerts red) *would* show the full range; the demo
just lacks those kinds. **Decision needed (Sam):** nudge Process toward a clearer cyan (a small
deviation from the MP `#2bb3a8` toward the named "cyan") to separate it from File green, and/or
seed a few Socket/Host/Alert nodes into the demo so the palette is visible in the review shot —
vs. accept the green-family palette as approved.

## Open decisions surfaced (Stop-and-Show)
1. **Which binary did the review use?** If it was `main`, this branch already resolves the
   report — re-verify on `feat/ui-gits-polish` (forced `tier=high, adaptive=false` for the
   full-fidelity look on a weak GPU). If it genuinely was this branch, I need the exact build/
   commit to find the discrepancy (none exists in the source).
2. **Monochrome green** — palette nudge + demo seeding, or accept (above).
3. **Focus gate-glyph** — at HEAD-HIGH the focused node shows the gate-glyph concentric rings
   (a subtle target, *not* the big sphere). Keep, or suppress on the focus subject for the
   mockup's single-thin-ring look?
