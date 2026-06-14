# ADR-0005 — Graph-native detection rule engine

**Status:** Accepted — 2026-06-14
**Deciders:** Sam (161sam)
**Depends on:** ADR-0004 (two-plane architecture; detection placement).
**Implemented by:** MP-D1.

## Context

SpaceGraph today has no detection of its own — alerts arrive only from external
sources (Suricata, via `sources/suricata_eve.rs`). `graph/explain.rs` does a
single-pair BFS (`shortest_path` over `EdgeKindClass`), not pattern matching. To
become a detection plane (ROADMAP Track D1, layer 3), the viewer must synthesize
detections from its own graph topology. ADR-0004 fixes that this runs viewer-side
and emits `Node::Alert` (wire-stable per O-8). This ADR fixes *how*.

## Decision

### Representation: compiled matchers, not a DSL
Rules are **compiled Rust matchers** implementing a small `Rule` trait, not a
data-driven DSL. Rationale: existing-code-first / no speculative generality — the
first rules are few and match directly on the prebuilt `GraphModel` primitives. A
query-DSL already exists for *navigation* (`v0.5.0`); a *detection* DSL is
deferred until the rule corpus justifies it.

```
trait Rule {
    fn id(&self) -> &'static str;          // stable rule id (de-dup key prefix)
    fn technique(&self) -> &'static str;   // ATT&CK technique id (ADR-0006)
    fn tactic(&self) -> Tactic;            // ATT&CK tactic enum (ADR-0006)
    fn severity(&self) -> Severity;        // reuse low/medium/high
    fn evaluate(&self, model: &GraphModel) -> Vec<Detection>;
}
```

A `Detection` carries the matched subject `NodeId`, the supporting subgraph
(node/edge ids, for rendering the "why"), and a stable de-dup key.

### Placement: a budgeted Update system after layout
The engine runs in a **new `Update` system scheduled after**
`update_layout_or_timeline`, reading the canonical `GraphModel` (reusing its
prebuilt `adj` adjacency, `AggEdge`/`EdgeStats` `first_ts`/`last_ts`/`count`
indices, and `EdgeKindClass`). It carries a **time budget** mirroring
`layout_budget_ms`: detection cost may never stall a frame. **No per-frame
full-graph rescan** — rules use the O(1) adjacency/degree accessors and the
incremental edge index; a dirty-set / interval cadence bounds work on large
graphs (bench scales 500/1000/2000/5000, `benches/layout.rs`).

### Emission: detections → first-class `Node::Alert`
A new detection emits a `Node::Alert { source: "spacegraph-rule", signature:
<rule id + ATT&CK>, severity, ts }` plus an `alerts_on` edge to its subject,
through the **existing** `GraphState::note_alert` / `alert_order` plumbing
(cap + eviction via `max_visible_alerts`, severity counts, timeline lane). This
reuses all alert triage/render/cap machinery and needs **no wire change** (O-8) —
the alert is produced *in the viewer*, not received over UDS.

### De-dup / re-arm
Each detection has a **stable id** (`id_alert(subject, "{rule_id}|{subgraph_key}")`)
so the same subgraph across ticks yields **one** alert, not N. A rule **re-arms**
when its match clears and recurs (a fresh occurrence is a fresh detection).
De-dup interacts with `max_visible_alerts` eviction exactly as Suricata alerts do.

### First rules (existing graph data only — no new collector, per O-8/O-9)
1. **Lateral-movement candidate** (`T1021`, *Lateral Movement*): a `Process` with
   an `execs` child shell **and** a new `connects_to` a `RemoteHost` **and** a
   correlated `Alert` in window.
2. **Suspicious new listener** (`T1571`/`T1071`, *Command and Control*): a new
   `Socket` in `listens_on` state on an unusual port owned by an unexpected
   process.
3. **Beaconing candidate** (`T1071`, *Command and Control*): repeated
   `connects_to` the same `RemoteHost` at a regular cadence (from `EdgeStats`
   `count` + timestamps).

## Alternatives considered

- **Data-driven DSL first.** Rejected: speculative generality before a corpus
  exists; navigation DSL already covers the query need.
- **Agent-side detection.** Rejected by ADR-0004 — keeps the agent dumb/read-only;
  the canonical graph lives in the viewer.
- **A separate `Detection` node kind.** Rejected for D1: reusing `Node::Alert`
  avoids a wire bump (O-8) and inherits triage/cap/timeline for free. A distinct
  kind is revisited only if rendering clarity demands it (then under O-8).
- **Per-frame full rescan.** Rejected: violates the performance discipline;
  budgeted incremental evaluation over the existing indices is mandatory.

## Consequences

- D1 is fully AUTO and wire-stable; it lands alongside `v0.5.0`.
- The rule registry's technique↔rule mapping is the single source of truth for the
  ATT&CK coverage view (D5, ADR-0006).
- Detections feed multi-stage correlation (D3) and, downstream, SOAR playbook
  proposals (`v0.7.x`) — but a detection only ever **proposes**; remediation is
  gated Smolit-side.

## References

- ROADMAP Track D1, §5 (ATT&CK tagging discipline, quality gates).
- `crates/spacegraph-viewer/src/graph/model.rs` — `GraphModel`, `adj`, `AggEdge`,
  `EdgeStats`, `EdgeKindClass`.
- `crates/spacegraph-viewer/src/graph/state.rs` — `note_alert`, `alert_order`,
  `max_visible_alerts`, `alert_severity_counts`.
- `crates/spacegraph-viewer/src/graph/explain.rs` — the BFS primitive rules
  extend from; the fixture/test posture (`shortest_path` tests) D1 mirrors.
- `crates/spacegraph-agent/src/sources/suricata_eve.rs` — the pure-parse +
  committed-fixture test pattern D1's rule tests follow.
