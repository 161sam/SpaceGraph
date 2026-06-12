# docs/media — screenshots & recordings

Kickstarter-grade visuals for the SpaceGraph viewer. These are captured locally
(the CI/headless build has no GPU); the renderer that produces them is in place
as of Phase 5 (`feat/visual-design-pass`).

## Phase 5 — Visual pass (to capture)

Run the viewer with the synthetic demo load and the Standard theme (default),
then capture:

```bash
cargo run -p spacegraph-viewer -- --demo-load 2000
```

- `spatial-2k.png` — Spatial view, ~2000 nodes, neon nodes + bloom over dark
  space with the floor grid.
- `focus-mode.png` — a focused node (press `F` on a selection) showing its
  neighbourhood, edge colours by class, and the recent-activity pulse.
- `timeline.png` — Timeline view (`T`) with the shared palette.

## Phase 8 — Threat viz (to capture)

- `alerts.png` — red alert nodes/edges attached to live connections (replayed
  EVE file). See Phase 8 in `docs/perf/RUNLOG.md`.

Keep large recordings out of git history where possible (link instead); commit
only the still PNGs needed for the README / Kickstarter page.
