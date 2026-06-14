# ADR-0006 — MITRE ATT&CK detection & coverage dimension

**Status:** Accepted — 2026-06-14
**Deciders:** Sam (161sam)
**Depends on:** ADR-0004 (two-plane), ADR-0005 (rule engine).
**Implemented by:** MP-D1 (tagging); ROADMAP D5 (coverage/posture view).

## Context

A professional detection tool speaks MITRE ATT&CK: every detection maps to a
technique, coverage is measured against the technique matrix, and an attack is
understood as progression through tactics (Initial Access → … → Exfiltration →
Impact). ATT&CK is the industry-standard vocabulary; adopting it makes
SpaceGraph's detections legible to analysts, gives an honest self-assessment of
coverage gaps, and provides the tactic axis the threat-motion vocabulary (D2)
animates along. It is also the single highest-leverage credibility surface for
the project's external (Kickstarter) story.

ATT&CK must not force a `spacegraph-core` wire change (O-8). It rides as
viewer-side metadata.

## Decision

ATT&CK threads through three points, all viewer-side, no wire change:

### 1. Detection tagging (D1 — mandatory now)
Every `Rule` (ADR-0005) declares a `technique: &'static str` (e.g. `"T1021"`) and
a `tactic: Tactic` (a closed enum over the 14 ATT&CK Enterprise tactics). The tag
is folded into the emitted `Node::Alert.signature` (e.g.
`"spacegraph-rule:lateral-movement:T1021"`) and held in the **viewer-side rule
registry** as the canonical `technique ↔ rule` mapping. **Discipline (ROADMAP
§5):** a rule with no ATT&CK mapping fails review.

```
enum Tactic {
    Reconnaissance, ResourceDevelopment, InitialAccess, Execution,
    Persistence, PrivilegeEscalation, DefenseEvasion, CredentialAccess,
    Discovery, LateralMovement, Collection, CommandAndControl,
    Exfiltration, Impact,
}
```

A small static `TECHNIQUES` table (`technique_id → (name, tactic)`) is **vendored,
not fetched** (O-7: no egress) — a curated subset covering the implemented + near-
term rules, extended as rules are added. It is *not* the full ATT&CK corpus; it is
the techniques SpaceGraph can reason about.

### 2. Tactic-phased visualization (D2)
The tactic enum is an ordered kill-chain axis. The threat-motion vocabulary (D2)
keys its per-class motion off the tactic (e.g. `CommandAndControl` → periodic
beacon pulse; `LateralMovement` → traversal sweep; `Exfiltration` → outbound-
weighted flow). A campaign (D3) advancing through tactics is the narrative an
operator reads off the timeline/graph.

### 3. Coverage heatmap + posture (D5)
An ATT&CK-Navigator-style **coverage view** is computed read-only from the rule
registry: for each technique in `TECHNIQUES`, detected (a rule maps to it) vs
undetected → a tactic-grouped heatmap showing gaps. A **posture/exposure score**
combines coverage with observed attack-surface signals (open listeners, unusual
outbound, alert density) over the in-memory graph. Historical posture retention is
`v0.9.0`/OceanData.

## Alternatives considered

- **Fetch the live ATT&CK corpus.** Rejected: violates O-7 (egress). A vendored,
  curated subset is sufficient and keeps SpaceGraph offline-safe.
- **A `technique` field on `Node::Alert` in the wire schema.** Rejected for now
  (O-8): folding the tag into `signature` + the viewer-side registry avoids a
  bump. A first-class field is revisited if/when D4 opens the wire (3→4).
- **Free-text technique strings without a tactic enum.** Rejected: the closed
  tactic enum is what makes the coverage view and the tactic-phased motion
  well-defined and testable.

## Consequences

- D1 ships ATT&CK-tagged detections with no wire change; the rule registry is the
  coverage view's single source of truth.
- The coverage/posture view (D5) and tactic-phased motion (D2) build directly on
  the tag model — no rework.
- Coverage is honest by construction: a technique with no mapped rule shows as a
  gap, which both guides the rule backlog and is a credible external artifact.

## References

- ROADMAP §5 (ATT&CK tagging discipline), Track D1 (tags), D2 (motion), D5
  (coverage/posture).
- ADR-0005 — the `Rule` trait carrying `technique`/`tactic`.
- MITRE ATT&CK Enterprise tactics — the 14-tactic kill-chain the `Tactic` enum
  mirrors (vendored subset, no live fetch).
