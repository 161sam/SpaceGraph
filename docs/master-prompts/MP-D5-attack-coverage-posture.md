# MP-D5 — ATT&CK coverage heatmap + posture score

**Mode:** AUTO (Track D, viewer-side, read-only, no wire change, no egress).
**Repo root:** `/home/dev/SpaceGraph`
**Branch:** `feat/attack-coverage-posture`
**Depends on:** **D1** (rule registry + `technique`/`tactic` tags); benefits from
D2/D3 (a richer corpus).
**Specs:** ROADMAP D5, ADR-0006 (the tag model + coverage plan).
**Estimated size:** M.

## Mission
"How well am I covered / how exposed am I." An ATT&CK-Navigator-style coverage
heatmap from the rule registry (detected vs undetected techniques → gaps) and a
deterministic posture/exposure score. Read-only; no egress.

## Pre-approved decisions
1. **Coverage from the rule registry** (the D1/ADR-0006 `technique ↔ rule` map):
   for each technique in the **vendored** `TECHNIQUES` table, detected (a rule maps
   to it) vs undetected → a tactic-grouped heatmap. **No live ATT&CK fetch** (O-7'
   — the lookup is local; only the agent/scanner touch the network, and not here).
2. **Posture/exposure score** = coverage + observed attack-surface (open listeners,
   unusual outbound, alert density) over the in-memory graph; **deterministic** over
   a fixture graph.
3. No wire change (read-only computation over the registry + graph).

## Out of scope
Historical posture retention (`v0.9.0`/OceanData). Any egress for the lookup. Any
wire bump.

## File paths
- `crates/spacegraph-viewer/src/graph/coverage.rs` — coverage computation (pure fn
  over the registry).
- `crates/spacegraph-viewer/src/graph/posture.rs` — the score (pure fn over graph +
  coverage).
- `crates/spacegraph-viewer/src/ui/` — the Navigator-style heatmap view (tactic-
  grouped, detected/undetected).

## Phases & gates
- **P1 Coverage.** Detected/undetected per technique from the registry. *Gate:*
  every rule maps to a technique (asserted); the view lists detected vs undetected;
  pure-fn test.
- **P2 Heatmap view.** Tactic-grouped coverage UI. *Gate:* renders the tactic
  grouping; existing UI tests green.
- **P3 Posture score.** Deterministic score over a fixture graph. *Gate:* same
  fixture → same score (deterministic); components (coverage / surface / density)
  unit-tested.
- **P4 Close-out.** Update ACCEPTANCE (D5), CODE_INVENTORY, RUNLOG, DESIGN_LANGUAGE
  (the coverage/posture view). *Gate:* `fmt`/`clippy`/`test --workspace`.

## Quality gates (every commit)
Standard set; **no egress for the coverage/CVE lookup** (vendored, audited); no
`spacegraph-core` wire bump; no AI-authorship markers; naming hygiene.

## Stop-and-Show
If a meaningful posture score seems to need data not in the graph (would need a new
source) → surface (don't add a source here). If coverage needs the registry exposed
differently → coordinate with D1's registry shape.

## Done
Coverage heatmap from the registry (detected/undetected per technique) + a
deterministic posture score; read-only, no egress, no wire bump; docs updated.
Branch ready for review.
