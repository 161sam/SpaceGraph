# ADR-0009 — Threat-motion vocabulary + purple-team origin

**Status:** Accepted — 2026-06-14
**Deciders:** Sam (161sam)
**Depends on:** ADR-0004 (two-plane; O-8 wire-stability, no-exec), ADR-0005/0006
(detection + ATT&CK tactic enum).
**Implemented by:** MP-D2-core.

## Context

D1 gave the viewer ATT&CK-tagged detections (a `Tactic` per detection). D2-core
adds the first read-only external security-tool source (Nebula, the ESN red-team
tool) and two viewer-side legibility layers — all with **no `spacegraph-core`
change** (O-8) and **no exec/egress** in the agent (O-7'):

1. an operator should read *what kind* of activity is moving from across the scene
   (a beacon looks different from lateral movement);
2. an operator must not confuse **authorized red-team** activity with a **real**
   threat in the same scene.

## Decision

### 1. Threat-motion keyed off ATT&CK tactic
Each attack class gets a distinct motion selected by its **tactic** (the
ADR-0006 enum), via a pure classifier `render::motion::motion_style(Tactic) ->
MotionStyle` (mirroring `aperture_style`/`highlight_style`). Mapping: C2 → beacon
pulse, lateral movement → traversal sweep, exfiltration → outbound flow,
credential access → rapid flash, execution/impact → worm-spread; others default to
a calm pulse. Motion constants (`speed`, `amplitude`) live with the style — **no
ad-hoc magic numbers in render**. Motion is a **Standard-only** cue:
`motion_style_themed` forces `Static` under Minimal (motion never changes graph
truth). The per-frame animation that consumes the style is render-only; the
classifier is the unit-tested core.

### 2. Purple-team origin — a viewer-side field, no wire change
A node/edge is classified `Observed` | `RedTeam` by
`render::motion::origin_of(stream) -> Origin`, derived from the **emitting stream**
(the per-connection namespace), not a wire field. The no-wire mechanism: the
Nebula log source is deployed as its **own agent stream** named `nebula-*` /
`red-team-*`; the viewer marks every entity from such a stream as red-team (a
`[red-team]` tag in the inspector; a distinct styling envelope in Standard).
Rationale: sources *within* one agent share a namespace, so origin must come from
the stream identity, and a stream-name convention keeps it wire-stable (O-8).
Minimal degrades to neutral.

### 3. Nebula source — observe only, existing kinds, assumed schema
A `nebula` `EventSource` (`spacegraph-agent`) **tails** Nebula's engagement log
(`~/.local/share/nebula/logs`, A.5) read-only and emits **existing** kinds
(`RemoteHost` targets; `ConnectsTo` for lateral hops) — mirroring `suricata_eve`.
SpaceGraph **observes** Nebula; it never launches an engagement (O-9). **No exec,
no egress, no new node/edge kind, no wire bump.**

**Schema is assumed, not verified (A.5).** Nebula's real log schema is not
verifiable from the build host. The parser targets a documented JSONL assumption
(one targeted event per line: `event`, optional `src`, `target`, …), and the
committed fixture (`sources/fixtures/nebula.jsonl`) **is** that contract.
`parse_nebula_event` is the single place to adjust when the schema is verified on
the operator's host. This is surfaced to the operator (RUNLOG) rather than blocking
the phase, the same posture as Suricata/locate host-verification.

## Alternatives considered

- **Per-signature motion.** Rejected: the tactic axis (ADR-0006) is the closed,
  testable vocabulary; per-signature motion is unbounded and not legible.
- **A wire `origin`/`red_team` field on nodes.** Rejected (O-8): origin is
  derivable viewer-side from the emitting stream; no bump needed.
- **Build the Nebula parser to a guessed schema with no flag.** Rejected: A.5
  mandates verifying the schema; the assumption is documented + fixture-pinned +
  surfaced, and isolated to one function, so a mismatch is a localized fix.
- **A new `Engagement`/`RedHost` node kind.** Rejected for D2 (O-8): reuse
  `RemoteHost` + the viewer-side origin field; a distinct kind would ride D4.

## Consequences

- Motion + origin build directly on the D1 tactic model — no rework; D5 coverage
  and D3 campaigns inherit the same vocabulary.
- The Nebula source is the first external red-team feed; its schema assumption is
  the one operator-host verification item (A.5).
- No wire change, no new kind, agent stays read-only/no-exec (O-7'/O-8 hold).

## References

- ROADMAP D2, §0.3, §5; Appendix A.5 (Nebula log location/schema-verify).
- ADR-0006 — the `Tactic` enum the motion keys off.
- `crates/spacegraph-viewer/src/render/motion.rs` — `motion_style`, `origin_of`.
- `crates/spacegraph-agent/src/sources/nebula.rs` + `fixtures/nebula.jsonl`.
- `crates/spacegraph-agent/src/sources/suricata_eve.rs` — the tail/parse/fixture
  pattern this source mirrors.
