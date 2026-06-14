# MP-D3 — Multi-stage correlation / campaign aggregation

**Mode:** AUTO (Track D, viewer-internal, no wire change).
**Repo root:** `/home/dev/SpaceGraph`
**Branch:** `feat/multi-stage-correlation`
**Depends on:** **D1** (detections exist as `Alert`s with ATT&CK tags).
**Specs:** ROADMAP D3, ADR-0007 (correlation model — author at this phase).
**Estimated size:** M.

## Mission
An attack is a *sequence* of detections, not isolated alerts. Link related
detections into one tracked **campaign**, rendered as a highlighted path through the
graph + a lane on the timeline. Viewer-internal; no new wire type.

## Pre-approved decisions
1. **Viewer-internal aggregation** over already-ingested `Alert`s — link by shared
   subject node / temporal window / ATT&CK **tactic progression**. **No new wire
   type**; a first-class `Campaign` *node* (a wire bump) is **deferred** (revisit
   only if a published campaign object is needed → O-8).
2. Pure aggregation core (`fn correlate(alerts, model) -> Vec<Campaign>`) for
   unit-testing without ECS.
3. Render: a highlighted path linking the campaign's subgraph + a timeline lane.
4. De-dup/re-arm across ticks like D1 detections.

## Out of scope
A `Campaign` wire/core type (deferred). Any wire bump. ABrain reasoning over
campaigns (`v0.8.0`).

## File paths
- `crates/spacegraph-viewer/src/graph/correlation.rs` — the campaign aggregation
  (pure core + `fixtures/`).
- `crates/spacegraph-viewer/src/render/` — campaign path highlight.
- `crates/spacegraph-viewer/src/ui/` (timeline) — campaign lane.

## Phases & gates
- **P1 Aggregation core.** Link detections into campaigns (shared subject / window /
  tactic progression). *Gate:* fixture detection sequence → **one** campaign (not
  N); negative fixture → no chain; de-dup/re-arm across two passes.
- **P2 Render.** Highlighted path through the campaign subgraph. *Gate:* renders the
  linked path; Minimal-degrades.
- **P3 Timeline lane.** Campaign as a lane on the timeline. *Gate:* lane reflects
  the campaign's span; existing timeline tests green.
- **P4 Close-out.** Author ADR-0007; update ACCEPTANCE (D3), CODE_INVENTORY, RUNLOG.
  *Gate:* `fmt`/`clippy`/`test --workspace`.

## Quality gates (every commit)
Standard set; no `unwrap`/`expect` in render/IPC; **no `spacegraph-core` wire bump**
(audited); no AI-authorship markers; naming hygiene.

## Stop-and-Show
If correct correlation seems to need a persisted/wire `Campaign` type → stop (that
is deferred; keep it viewer-internal). If it needs a `GraphModel` change → surface.

## Done
Viewer-internal campaign aggregation with fixtures; highlighted-path + timeline-lane
render; no wire bump; ADR-0007 authored; docs updated. Branch ready for review.
