# Recon Report — Auto-safe band readiness (MP-ORCH)

**Date:** 2026-06-14 · **HEAD:** `aab221b` · **Branch:** `main`
**Driver:** MP-ORCH Phase 1 (read-only audit of the auto-safe band).
**Predecessor:** `docs/recon/RECON_REPORT.md` (pinned at `2a8aa41`, v0.4.0) +
`docs/recon/DRIFT_MATRIX.md`. This is the dated successor required by MP-ORCH
Phase 1; it audits the band against the code as it stands **after** the v0.5.0 /
v0.5.1 / v0.5.2 merges.

**Resolution (2026-06-14, post-stop):** Sam decided to **ratify** `PROTOCOL_VERSION
4` (amend O-8 + the D4 plan; author **ADR-0016**) and **restore O-7'** by replacing
the agent's `locate` shell-out with the builtin walker (no exec). Landed on branch
`chore/reconcile-wire-v4-and-agent-noexec` (review-gated). After review, the
auto-safe band resumes from D0. The verdict below is the original Phase-1 finding.

**Verdict (headline):** **STOP-and-Show.** The base is green, but two locked
decisions that MP-ORCH treats as inviolable for the auto-safe band — **O-7'
(agent no-exec)** and **O-8 (wire-stability: PROTOCOL_VERSION stays 3 until D4,
deferred behind v0.6.0)** — are **already violated by merged code** (v0.5.2
"Filesystem Search & Index", commit `ed2f5ce`), and the authoritative ROADMAP
v0.5 does **not** reflect that change. Per MP-ORCH Phase 1 this is Sam's call,
not an auto-fix. The auto-safe band's own Done criteria ("`spacegraph-core` still
at PROTOCOL_VERSION 3"; "agent still read-only/no-exec") are therefore
**unsatisfiable against the current baseline** and cannot be auto-executed.

---

## 1. Base state (Phase 0 gate) — PASS

- Working tree: **docs-only dirt** — `MP-ORCH-*.md` modified + six untracked
  dedicated MPs (`MP-D2-core`, `MP-D2-firewall`, `MP-D2-flow`, `MP-D3`, `MP-D5`,
  `MP-v0.6.0`) in `docs/master-prompts/`. **No code changes** in the tree. This
  is the "governance docs left staged" situation Phase 0.2 anticipates, not an
  unstable in-flight session.
- Gates (workspace, at `aab221b`): `cargo fmt --check` ✓ · `cargo build` ✓ ·
  `cargo test` ✓ (**186 passing, 0 failed**) · `cargo clippy --all-targets
  -D warnings` ✓.
- Governance docs present: ROADMAP v0.5; ADRs 0004/0005/0006/0008/0012/0013/0015;
  MP-D0, MP-D1, MP-E1 committed (`aab221b`). Superseded ROADMAP archived
  (`docs/archive/2026-06-13-superseded-by-roadmap/ROADMAP_v0.2.0.md`).
- **Not committed by this run** — see §5.

> Minor note (non-blocking): the `ed2f5ce` merge message claims "236 tests"; the
> current workspace run reports 186 passing. Likely a doctest/binary-bucketing
> difference. Flagged for the next doc-reconcile, not a gate failure.

## 2. The blocker — merged v0.5.2 broke two locked invariants the roadmap still asserts

Commit `ed2f5ce` — *"Merge: v0.5.2 — Filesystem Search & Index (wire protocol
v4, agent index, ON DISK search UI)"* — touched `spacegraph-core/src/lib.rs`
(+175) and added `spacegraph-agent/src/index/{mod,locate,rank,walker}.rs`.

### 2a. O-8 violated — wire already at PROTOCOL_VERSION 4

- **Code:** `crates/spacegraph-core/src/lib.rs:13` → `pub const PROTOCOL_VERSION:
  u32 = 4;` (`MIN_COMPATIBLE_PROTOCOL = 3`); test at `lib.rs:281` asserts
  `PROTOCOL_VERSION == 4`.
- **Roadmap (authoritative, says the opposite):** the 3→4 bump is "the **single**
  sanctioned bump", reserved for **D4**, **deferred behind v0.6.0** — ROADMAP
  lines 32, 371, 457–462, 502, 504, 619, 659 (O-8) and the O-8 row at line 720;
  ADR-0004 §O-8; ADR-0005/0006/0008/0012 all justify designs by "no wire bump
  (O-8)". ROADMAP lines **208 & 213 still describe the live wire as
  `PROTOCOL_VERSION = 3`.**
- **MP-ORCH Done criterion (now unmeetable):** "`spacegraph-core` still at
  PROTOCOL_VERSION 3 (no wire bump)".
- **Significance:** D4 was framed as "the one phase that crosses the wire
  boundary," opening the extension model + boundary + vitals **together** under a
  single bump. v0.5.2 **spent that bump early**, for a different purpose
  (filesystem-search request/response messages — no new `Node`/`EdgeKind`
  variant was added; the 6 Node / 7 EdgeKind set is unchanged). What "protocol 4"
  now means, and what D4 is allowed to do next, is an open architectural question.

### 2b. O-7' violated — the agent now execs (`child_process`)

- **Code:** `crates/spacegraph-agent/src/index/locate.rs` — `SystemLocate`
  (production, **not** `#[cfg(test)]`) implements `LocateBackend` by shelling out:
  `locate.rs:6 use std::process::Command;` → `locate.rs:81 Command::new(self.kind
  .binary())`. Module doc: *"The real system locate backend (shells out via
  `std::process`)."* (The `std::process::id()` calls in `index/mod.rs`,
  `path_policy.rs`, `walker.rs` are PID-for-tempdir only — not exec.)
- **Roadmap (authoritative, forbids it):** "Hard rule: no `child_process`/exec
  **anywhere in the tree** (audited)" (lines 208–209); O-7' plane invariant
  (644–652) — the agent's "read-only / no-egress / **no-exec** guarantee"; "State
  via the source, not the tool … never via subprocess (that is
  `child_process`/exec, forbidden)" (671–674); D4 vitals "via procfs/sysfs … never
  via subprocess spawn (no-exec)" (494); O-7' row (719). No `locate` carve-out
  exists anywhere in the governance docs.
- **MP-ORCH Done criterion (now false):** "agent still read-only/no-exec".
- **BLOCKED discipline forbids the auto-fix:** "Never relax O-7'/O-8 … the agent
  no-exec guarantee … to get unblocked."

### 2c. Roadmap doesn't represent the merged FS-search feature as its own phase

The ROADMAP's only "searchable index" entries are **E4 — "Searchable recon index
(the 'Shodan search')"** (lines 574, 628), which is part of **Track E (the
scanner / active-recon track — NOT-AUTO)** and is about scanned-host
intelligence, not a local *filesystem* index. The v0.5.2 *filesystem* search/
index that actually merged has no corresponding roadmap phase. So the merged
session changed the architecture (wire + agent exec + a new agent subsystem) in a
way the roadmap does not reflect — the exact Phase 1 STOP trigger.

## 3. Per-phase readiness (auto-safe band)

Structural preconditions are otherwise **met** — the blocker is the baseline
governance drift (§2), which gates every phase via MP-ORCH's global audited
negatives.

| Phase | Structural preconditions | Verdict |
|---|---|---|
| **D0** perimeter/exposure (MP-D0, ADR-0012) | `Node::Socket`/`RemoteHost` present; viewer-side; no wire | **READY but BLOCKED** by §2 (global no-exec/no-wire audit unmeetable) |
| **D1** rule engine + ATT&CK (MP-D1) | `GraphModel` (`graph/model.rs:67`), `GraphState` (`graph/state.rs:645`), `Node::Alert`, graph/query subsystem present | **READY but BLOCKED** |
| **D2-core** motion + Nebula + origin (MP-D2-core, ADR-0009 to author) | `render/theme.rs` present; `sources/suricata_eve.rs` pattern to clone; depends on D1 | **READY but BLOCKED** — and Nebula source lands in `spacegraph-agent`, the same crate currently in O-7' breach |
| **D2-firewall / D2-flow** (netlink/conntrack sources) | new netlink deps; read-only `/proc`/netlink | **BLOCKED** — these pin new agent deps into a crate that already violates no-exec; do not extend it until O-7' is resolved |
| **D3** correlation (MP-D3, ADR-0007 to author) | `graph/timeline.rs` + `render/timeline.rs`; viewer-internal | **READY but BLOCKED** |
| **D5** ATT&CK coverage/posture (MP-D5) | depends on D1/D2/D3 corpus | **BLOCKED** (downstream) |
| **v0.6.0** read-only MCP (MP-v0.6.0, ADR-0001 to author) | `GraphState` exists (canonical-state crux has a target); never auto-merged regardless | **BLOCKED** — also independently needs its P0 Reality-Check + canonical-state design decision (Sam) |

## 4. Options for reconciliation (Sam's decision)

1. **Ratify reality + reconcile docs, then resume (recommended for the wire).**
   Author/extend an ADR recording that v0.5.2 spent the 3→4 bump; amend O-8 and
   the D4 plan (D4 no longer "the bump" — it rides protocol 4). Fix ROADMAP lines
   208/213 (and the O-8 narrative) to state `PROTOCOL_VERSION = 4`. The wire is
   already tagged/shipped (v0.5.2) — reverting it is not realistic.
2. **Restore O-7' literally (recommended for the exec).** Replace `SystemLocate`'s
   shell-out with a **no-exec** implementation (read the `plocate`/`mlocate` DB
   directly, or drop the locate backend in favour of the existing read-only
   walker), so "no `child_process`/exec anywhere" holds again. *Alternative:* a
   tightly-scoped ADR carve-out explicitly permitting the read-only local `locate`
   query — but this weakens an audited invariant and should be a deliberate,
   documented choice, not a default.
3. **Do nothing / proceed anyway:** rejected — MP-ORCH forbids relaxing
   O-7'/O-8 to get unblocked, and the band's Done criteria would be unmeetable.

**Recommendation:** (1) for the wire + (2)-replace for the exec. Both are doc/code
reconciliation of the *existing* baseline and should land as their own
review-gated change **before** any auto-safe-band phase begins, so the band runs
against a truthful, invariant-holding baseline.

## 5. What this run deliberately did NOT do

- **No commits.** The six untracked MPs and the MP-ORCH edit encode the same
  now-false premises ("no wire bump", "no-exec"); committing them before the §4
  reconciliation would bake in stale governance. Left for Sam to land together
  with the reconciliation.
- **Did not enter Phase 2.** No D0–D5 / v0.6.0 code. Auto-boundary intact: no
  Track C / Track E / D4 / D6 / Track F code.
- **Did not relax any locked decision** to proceed.
