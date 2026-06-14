# MP-D1 — Graph-native detection rule engine + ATT&CK tagging

**Mode:** AUTO (Track D, viewer-side, read-only, no wire change, no publish).
**Branch:** `feat/detection-rule-engine`
**Parallel to:** `MP-v0.5.0` (Track A) — no shared files; rebase onto whatever
`v0.5.0` lands.
**Authoritative specs:** ADR-0004 (two-plane architecture), ADR-0005 (rule
engine), ADR-0006 (ATT&CK dimension), ROADMAP Track D1 + §5.
**Estimated size:** L.

---

## Mission

Give the viewer its own graph-native detection: a compiled-matcher rule engine in
`graph/rules.rs` that runs over the canonical `GraphModel`, emits detections as
first-class `Node::Alert` (`source = "spacegraph-rule"`), and tags every detection
with a MITRE ATT&CK technique + tactic. Ship 3 first rules that match on existing
graph data, surfaced through the existing alert inbox with the matched "why"
subgraph and a click-to-focus. No new collector, no `spacegraph-core` change, no
egress, no publish.

---

## Pre-approved decisions (do NOT re-litigate — execute)

1. **Compiled Rust matchers, not a DSL.** A `Rule` trait per ADR-0005. No
   data-driven rule language in this MP.
2. **Viewer-side.** The engine lives in `crates/spacegraph-viewer/src/graph/` and
   reads `GraphModel`. Nothing agent-side. No new `EventSource`.
3. **Detections reuse `Node::Alert`.** Emit through `GraphState::note_alert` with
   `source = "spacegraph-rule"`. **No new node/edge kind. No `PROTOCOL_VERSION`
   bump** (O-8). If you find yourself wanting a `Detection` kind — stop, that is a
   separate post-`v0.6.0` decision; reuse `Alert`.
4. **Budgeted `Update` system after layout.** Schedule after
   `update_layout_or_timeline`, carry a `detection_budget_ms` config mirroring the
   layout budget. No per-frame full-graph rescan — use `GraphModel`'s prebuilt
   `adj`/degree/`AggEdge`/`EdgeStats` indices and an interval/dirty cadence.
5. **Stable de-dup id + re-arm.** `id_alert(subject, "{rule_id}|{subgraph_key}")`.
   Same subgraph across ticks → one alert. Match clears then recurs → new alert.
6. **ATT&CK tag is mandatory per rule** (`technique` + `tactic` enum), folded into
   `Alert.signature` and held in the rule registry. A rule with no mapping does
   not compile into the registry (enforce structurally if cheap, else a test).
7. **Vendored ATT&CK subset, no fetch** (O-7). A static `TECHNIQUES` table
   covering only the implemented + near-term techniques.
8. **First 3 rules, existing data only:** lateral-movement (`T1021`,
   LateralMovement), suspicious new listener (`T1571`/`T1071`, CommandAndControl),
   beaconing candidate (`T1071`, CommandAndControl). Exact predicates per ADR-0005.

## Explicitly out of scope (reject if tempted)

- New `EventSource`s (Nebula/auditd/eBPF/Zeek/Falco) — each is its own MP (D2+).
- Any `spacegraph-core` change / `PROTOCOL_VERSION` bump.
- Multi-stage correlation / campaign object (D3).
- ATT&CK coverage heatmap / posture score (D5) — *tagging only* here.
- Any MCP publish surface (Track B) or AdminBot/remediation path (Track C).
- Any outbound network call, any `child_process`/exec.

---

## Architecture & file paths

**New:**
- `crates/spacegraph-viewer/src/graph/rules.rs` — the `Rule` trait, `Detection`,
  `Severity`/`Tactic` enums, the `RuleRegistry`, the 3 rules, the vendored
  `TECHNIQUES` table, and the pure `evaluate_rules(model) -> Vec<Detection>`
  function (the unit-testable core).
- `crates/spacegraph-viewer/src/graph/rules/fixtures/` — committed fixture graphs
  (positive + negative per rule), mirroring `sources/fixtures/suricata_eve.jsonl`
  usage.

**Edit (existing-code-first):**
- `crates/spacegraph-viewer/src/graph/mod.rs` — register the `rules` module.
- `crates/spacegraph-viewer/src/app/*` (plugin/schedule) — add the budgeted
  `run_detection_rules` `Update` system after `update_layout_or_timeline`; wire a
  `DetectionState` resource if needed (registry + last-run cadence + dirty flag).
- `crates/spacegraph-viewer/src/graph/state.rs` — the system calls existing
  `note_alert`; add only a thin producer entrypoint if one is missing (no new
  alert plumbing — reuse `alert_order`/cap/eviction).
- `crates/spacegraph-viewer/src/util/config.rs` — add `detection_enabled`
  (default `true`) + `detection_budget_ms` + `detection_interval_ms`, persisted;
  follow the existing 4-way config discipline (struct+Default, serialize no
  `serde(skip)`, `apply_viewer_config` round-trip).
- `crates/spacegraph-viewer/src/ui/*` (alert/inspector panel) — show the ATT&CK
  technique/tactic on a `spacegraph-rule` alert and its matched subgraph; click →
  focus the subject (reuse the existing alert-click jump). **Render only — do not
  fork the inbox.**

**Boundaries (enforced, per ARCH_VIEWER):** `rules.rs` reads `GraphModel` only;
it must not touch `net/` or raw events; it emits via the `GraphState` API. Keep
`evaluate_rules` a pure function of `&GraphModel` so it is unit-testable without
Bevy/ECS.

---

## Phases & gates (each phase: implement → test → `fmt`/`clippy`/`test` green → RUNLOG entry)

**P1 — Skeleton + registry + ATT&CK model.** `Rule` trait, `Detection`,
`Severity`, `Tactic` (14-tactic enum), `TECHNIQUES` table, empty `RuleRegistry`,
pure `evaluate_rules`. Config fields + round-trip.
*Gate:* `TECHNIQUES` completeness test (every entry has name+tactic); config
round-trip test; registry compiles with zero rules.

**P2 — Rule 1 (lateral-movement, `T1021`).** Predicate per ADR-0005 over
`adj`/`EdgeKindClass`. Positive + negative fixtures.
*Gate:* fixture-graph → exactly the expected detection; negative fixture → none;
de-dup across two evaluations → one; re-arm after clear → new.

**P3 — Rules 2 & 3 (listener `T1571`/`T1071`, beaconing `T1071`).** Beaconing uses
`EdgeStats` `count`+timestamps for cadence. Fixtures each.
*Gate:* per-rule positive/negative fixtures pass; combined fixture with all 3
firing yields exactly 3 distinct detections with correct technique/tactic/severity.

**P4 — Schedule integration + emission.** The budgeted `Update` system; emit via
`note_alert` with `source="spacegraph-rule"` and the stable id; honor
`detection_enabled`/budget/interval.
*Gate:* a headless system test (or a deterministic harness) shows a seeded graph
produces the expected `Alert` nodes through `alert_order`; cap/eviction
interaction with `max_visible_alerts` asserted (mirror
`alert_cap_evicts_oldest`); detection respects the disabled flag (zero alerts when
off).

**P5 — UI surface.** Technique/tactic + matched subgraph on the alert; click →
focus. Minimal-theme parity preserved.
*Gate:* the alert panel renders the ATT&CK tag for a `spacegraph-rule` alert
(assert on the formatting fn); existing alert tests still green; Minimal
equivalence unchanged.

**P6 — Perf + close-out.** Confirm no per-frame full rescan: the engine runs on
the interval/dirty cadence within budget on `benches/layout.rs` scales
(500/1000/2000/5000). Update `docs/ACCEPTANCE.md` (D1 criteria),
`docs/recon/CODE_INVENTORY.md` (new `graph/rules` module), `docs/DESIGN_LANGUAGE.md`
only if a new `theme.rs` constant was added (detections reuse `ALERT` severity
ramp — likely none).
*Gate:* documented budget note in `docs/perf/RUNLOG.md`; full `test --workspace`
green; clean `clippy`.

---

## Quality gates (every commit, non-negotiable)

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- No `unwrap`/`expect` in render/IPC paths (the detection system runs in
  `Update` — treat it as a hot path; return/skip on degenerate data, never panic).
- **Audited negative:** grep the tree — no `child_process`/`std::process::Command`
  spawn, no outbound network client added (O-7). Assert in close-out.
- Conventional commits, English, imperative. **No AI-authorship markers** (no
  `Co-Authored-By: Claude`, no `Generated with…`, no emoji authorship).
- Naming hygiene: no `enhanced`/`advanced`/`v2`/`pro` suffixes. `rules.rs`,
  `Rule`, `Detection`, `RuleRegistry` — descriptive names only.

## Test posture (headless)

Mirror the established patterns: pure-function fixture tests like
`sources/suricata_eve.rs::fixture_file_yields_three_alerts` and
`graph/explain.rs` `shortest_path` tests. Every rule gets a committed positive
fixture *and* a negative fixture that must not fire. De-dup/re-arm and
cap-eviction are explicit unit tests. GPU/visual confirmation is documented in
RUNLOG, never a CI stop.

## Stop-and-Show (pause, write `RUNLOG.md` note, surface to Sam)

- If any rule's correct predicate appears to require data **not** present in the
  current graph (would need a new collector) — **stop**, do not add a collector;
  report which rule and what data is missing.
- If de-dup correctness depends on a `GraphModel` change or a new index — **stop**
  and surface; do not bump the wire or restructure the model unilaterally.
- If emitting a detection cleanly seems to need a new node/edge kind — **stop**;
  reuse `Alert` is mandatory (O-8). Report the friction.
- At the phase boundary **P3 → P4** (engine proven on fixtures, before wiring into
  the live schedule): pause for a Sam look if anything diverged from ADR-0005.

## BLOCKED discipline

If genuinely blocked, write `BLOCKED.md` at repo root with: the phase, the exact
blocker, the ADR/ROADMAP clause in tension, and 1–2 options with a recommendation.
Do not work around a hard-stop by relaxing O-7/O-8/O-9.

## Done

- `graph/rules.rs` with the `Rule` trait, registry, vendored `TECHNIQUES`, and 3
  ATT&CK-tagged rules; budgeted `Update` system emitting `spacegraph-rule` alerts;
  UI shows technique/tactic + subgraph + click-to-focus.
- All fixtures (positive + negative per rule), de-dup/re-arm, cap-eviction,
  disabled-flag, config round-trip, perf-budget tests green.
- `fmt`/`clippy`/`test --workspace` green; audited no-exec/no-egress.
- `ACCEPTANCE.md`, `CODE_INVENTORY.md`, `RUNLOG.md` updated.
- No `spacegraph-core` change; no new collector; no publish.
- Branch `feat/detection-rule-engine` ready for review (not merged — Track D
  lands on review, like every phase).
