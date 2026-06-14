# MP-D2-core — Threat-motion vocabulary + Nebula source + purple-team origin

**Mode:** AUTO (Track D, viewer-side + one read-only file source, no wire change).
**Repo root:** `/home/dev/SpaceGraph`
**Branch:** `feat/threat-motion-and-nebula`
**Depends on:** **D1** (ATT&CK `tactic` tags on detections drive the motion).
**Specs:** ROADMAP D2 + §0.3, ADR-0006 (tactic vocabulary), ADR-0009 (purple-team
origin — author at this phase).
**Estimated size:** M.

## Mission

Give each attack class a distinct motion keyed off its ATT&CK tactic, add the first
external security-tool source (Nebula, read-only log tail), and disambiguate
authorized pentest activity from real threats in one scene. No new collector beyond
Nebula; no wire change.

## Pre-approved decisions

1. **Motion keys off `tactic`** (the D1/ADR-0006 enum), not per-signature. New
   `theme.rs` motion constants — no ad-hoc `Color::srgb`/magic numbers.
2. **Nebula source = read-only log tail** of `~/.local/share/nebula/logs` (the
   `suricata_eve` pattern: pure parse + committed fixture). It emits **existing**
   node/edge kinds. Lives in `spacegraph-agent` — preserves the agent's
   read-only/no-exec guarantee (reads a file Nebula wrote; no exec, no egress).
3. **Purple-team origin** = a viewer-side field (`observed` | `red_team`) derived
   from the source stream (Nebula stream → `red_team`); **no wire change**.
4. Everything degrades to Minimal (motion off → static; origin styling → neutral).

## Out of scope
Firewall/flow sources (sibling MPs MP-D2-firewall / MP-D2-flow). Any wire bump. Any
new node/edge kind. Launching Nebula (offensive — we only observe its logs; O-9).

## File paths
- `crates/spacegraph-viewer/src/render/` — per-tactic motion: C2 (`CommandAndControl`)
  periodic beacon pulse · `LateralMovement` traversal sweep · `Exfiltration`
  outbound-weighted flow · brute-force rapid edge flashes · worm-spread along edges.
- `crates/spacegraph-viewer/src/render/theme.rs` — motion constants.
- `crates/spacegraph-agent/src/sources/nebula.rs` — log-tail parse (+ `fixtures/`).
- `crates/spacegraph-viewer/src/render/spatial.rs` (or theme) — origin styling
  (`red_team` distinct edge/node treatment vs `observed`).
- `docs/adr/ADR-0009-purple-team-origin.md` — author it.

## Phases & gates
- **P1 Threat-motion.** Per-tactic motion selector (pure fn `tactic → MotionStyle`).
  *Gate:* each tactic maps to a motion; Minimal → static; unit test on the selector.
- **P2 Nebula source.** Parse `~/.local/share/nebula/logs` → existing kinds.
  *Gate:* committed fixture → expected nodes/edges + count/severity asserted; **no
  exec, no egress** (audited); diff-stable.
- **P3 Purple-team origin.** Derive `observed`/`red_team` from the stream; distinct
  styling. *Gate:* origin unit-tested from a fixture stream; Minimal → neutral.
- **P4 Close-out.** Author ADR-0009; update DESIGN_LANGUAGE (motion + origin +
  constants), ACCEPTANCE (D2-core), CODE_INVENTORY, RUNLOG.
  *Gate:* `fmt`/`clippy`/`test --workspace` green.

## Quality gates (every commit)
`fmt --check` · `clippy --workspace --all-targets -D warnings` · `test --workspace`;
no `unwrap`/`expect` in render/IPC; **no `child_process`/exec, no agent egress, no
`spacegraph-core` wire bump** (audited); conventional commits, English; **no
AI-authorship markers**; naming hygiene.

## Stop-and-Show
Verify the Nebula log schema before parsing (A.5) — if it differs from assumption,
surface. Any need for a new kind / wire change → stop (reuse existing kinds).

## Done
Per-tactic motion, Nebula read-only source, purple-team origin styling — all with
fixtures/unit tests, Minimal-degrading, no exec/egress/wire-bump; ADR-0009 authored;
docs updated. Branch ready for review.
