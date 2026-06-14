# ADR-0007 — Multi-stage correlation / campaign model

**Status:** Accepted — 2026-06-14
**Deciders:** Sam (161sam)
**Depends on:** ADR-0005 (detections as `Node::Alert`), ADR-0006 (ATT&CK tactic
enum), ADR-0004 (O-8 wire-stability).
**Implemented by:** MP-D3.

## Context

D1 produces isolated detections (`spacegraph-rule` alerts, each ATT&CK-tagged). An
attack, however, is a *sequence* — lateral movement *then* C2 *then* exfiltration.
The operator needs to see the **campaign**, not N disconnected red dots. This ADR
fixes the correlation model. It must not force a `spacegraph-core` change (O-8), so
campaigns are **derived viewer-side** from the existing alerts + graph topology.

## Decision

### Campaign = linked detections spanning a tactic progression
A **campaign** is a set of **≥2 detections** whose subjects are the **same node or
graph-adjacent nodes**, and which span **≥2 distinct ATT&CK tactics** (a kill-chain
progression). Two single-tactic alerts are *not* a campaign (no progression);
detections on unconnected subjects are *not* one campaign (no link).

`correlate(&GraphModel) -> Vec<Campaign>` is the pure aggregation core
(`graph/correlation.rs`), unit-tested without ECS:

1. collect `spacegraph-rule` detections as `(alert, subject, tactic)` — subject is
   the `alerts_on` target, tactic is parsed from the signature via the vendored
   technique table (ADR-0006);
2. **union** subjects that are equal or graph-adjacent (an edge between them) into
   components (union-find);
3. a component with **≥2 distinct tactics** is a `Campaign` carrying its subjects,
   alerts, the tactics in kill-chain order, and a **stable key** (sorted subjects +
   the tactic progression) for de-dup across ticks.

### Viewer-internal — no wire type, no `Campaign` node kind
Campaigns are a **derived view** over already-ingested alerts; there is **no new
wire message and no `Campaign` core kind** (O-8). A first-class published
`Campaign` object is **deferred** behind a wire bump — revisited only if a campaign
must cross the wire (e.g. ABrain reasoning over campaigns at `v0.8.0`).

### Render + timeline (visual layer)
The highlighted path through a campaign's subgraph and its lane on the timeline
consume `correlate`; per the project test posture, the GPU/visual confirmation is
documented in RUNLOG, while `correlate` itself is the unit-tested source of truth
(also surfaced as campaign membership in the inspector).

### De-dup / re-arm
The stable campaign `key` makes the same campaign across ticks one entry (mirrors
the D1 detection de-dup); a campaign clears when its detections clear (re-arm flows
from the underlying detections' re-arm, ADR-0005).

## Alternatives considered

- **A first-class `Campaign` node on the wire now.** Rejected (O-8): derivable
  viewer-side; a wire kind is deferred until a published campaign object is needed.
- **Correlate only by exact shared subject.** Rejected: misses cross-host
  progressions (lateral movement → exfil on the next host); graph-adjacency links
  them, which is the whole point of a *graph*-native correlator.
- **Time-window-only correlation.** Rejected as the primary key: viewer-produced
  detections carry no reliable wire timestamp; subject/adjacency + tactic
  progression is robust and testable. (A temporal window can refine grouping later
  without a model change.)

## Consequences

- D3 is fully AUTO and wire-stable; campaigns feed the timeline narrative and,
  downstream, ABrain reasoning (`v0.8.0`) and SOAR proposals (`v0.7.x`) — but a
  campaign only ever *describes*; remediation stays gated Smolit-side.
- The correlation key model is reusable by D5's posture score (campaign density is
  an attack-surface signal).

## References

- ROADMAP Track D3, §5.
- ADR-0005 (`Node::Alert` detections), ADR-0006 (`Tactic`).
- `crates/spacegraph-viewer/src/graph/correlation.rs` — `correlate`, `Campaign`.
- `crates/spacegraph-viewer/src/graph/rules.rs` — the technique→tactic source.
