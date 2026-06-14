# MP-ORCH — Roadmap auto-execution (auto-safe band → hard stop at offensive/mutating)

**Repo root:** `/home/dev/SpaceGraph`
**Mode:** **AUTO for the defined auto-safe band only.** This MP autonomously runs
the read-only / viewer-side band of the roadmap and **HARD-STOPS** at the
offensive/mutating line. It MUST NOT enter the NOT-AUTO tracks in auto-mode.
**Authoritative specs:** ROADMAP v0.5 (§3 tracks, §4 sequencing, §5/§6), all ADRs
(0004–0015).
**Estimated size:** XL (spans several phases; each phase has its own gates).

---

## THE AUTO BOUNDARY (non-negotiable — read first)

This MP runs **only** the auto-safe band:

```
finish v0.5.0 (running)  →  recon  →  D0 · D1 · D2 · D3 · D5  →  v0.6.0 (read-only MCP)  →  HARD STOP
```

It **MUST NOT** enter, in auto-mode, any of:
- **Track C** — v0.7.0 AdminBot, v0.7.x SOAR playbooks, v0.8.0 ABrain, v0.9.0
  OceanData (system mutation / data egress; `approver ≠ requester`; NOT-AUTO).
- **Track E** — the scanner, any phase (active egress / aggressive scanning;
  scope-gated; NOT-AUTO; CI scans only loopback/RFC5737 under human review).
- **D4** — node-model extension + boundary + vitals (wire bump 3→4 + external
  Reality-Check; supervised).
- **D6** — incident/case (depends on v0.9.0).
- **Track F** — exploitation (most sensitive; separate later track).

**Why this boundary is load-bearing, not bureaucratic:** auto-running the scanner
means an autonomous agent conducting active/aggressive scans; auto-mode on AdminBot
means an AI both approving *and* executing system mutations, which breaks the entire
`Decision → Review → Approval → Execution → Audit` spine; exploitation in auto-mode
is categorically out. These phases each get their own human-supervised master-prompt
(MP-E1 exists; v0.7.0/D4/F are reserved). **At the boundary, STOP and hand back to
Sam — do not proceed.**

---

## Phase 0 — Verify prerequisites & land governance docs

1. **Current CC sessions must be finished/merged and the tree clean.** If any
   in-flight session left the working tree dirty or the build broken, **STOP** and
   report — do not build on an unstable base.
2. **Verify the v0.5 governance docs are in `docs/*`** (ROADMAP v0.5, ADR-0004…
   ADR-0015, the existing MPs). If they were left staged (e.g. a `docs/files…`
   folder), place them now — `git mv` to targets, **archive** the superseded
   ROADMAP to `docs/archive/<date>-<reason>/` (never delete), then verify.
3. `cargo build --workspace` + `cargo test --workspace` green on the base.
**Gate:** clean tree, governance docs in place, base builds/tests green. Commit any
doc placement as its own commit. **STOP** if the base is unstable.

## Phase 1 — Full recon (read-only audit)

Reproduce the RECON_REPORT discipline. Read every governing doc (ROADMAP, all ADRs,
ARCH/ARCH_VIEWER, AGENTS, ACCEPTANCE, DESIGN_LANGUAGE, GRAPH_SCHEMA, CODE_INVENTORY,
RUNLOG) and **check the actual code state against them.** Specifically:
- Reconcile **docs ↔ code drift** — what the v0.5 roadmap intends vs. what the code
  + the just-finished CC sessions actually did. List divergences.
- Confirm the auto-safe band's preconditions hold (e.g. D1 needs the `GraphModel`
  primitives; v0.6.0 needs the `GraphState` projection).
- Produce a fresh `docs/recon/RECON_REPORT.md` (or dated successor) with a per-phase
  readiness verdict for the auto-safe band.
**Gate:** recon report written; readiness verdict per auto-safe phase. **STOP and
report** if recon finds the code materially violates a roadmap premise or a locked
decision (O-7'/O-8/O-10/O-11), or if a finished CC session changed the architecture
in a way the roadmap doesn't reflect — that needs Sam, not an auto-fix.

## Phase 2 — Execute the auto-safe band (dependency order)

Each phase: use its dedicated MP where one exists, else the roadmap phase spec +
its ADR; own branch; own gates; RUNLOG entry per phase. Order:

1. **D0** — perimeter & exposure → **MP-D0** (ADR-0012). AUTO, no wire.
2. **D1** — rule engine + ATT&CK → **MP-D1** (ADR-0004/0005/0006). AUTO, no wire.
3. **D2** — threat-motion + Nebula source (+ firewall/flow sources) → roadmap D2 +
   ADR-0009. AUTO, no wire, no exec in the sources (netlink read-only).
4. **D3** — multi-stage correlation → roadmap D3 + ADR-0007. AUTO, viewer-internal.
5. **D5** — ATT&CK coverage + posture → roadmap D5 + ADR-0006. AUTO, after the
   D1/D2/D3 rule corpus exists.
6. **v0.6.0** — MCP server (read-only) + ESN admission → roadmap v0.6.0 (Track B) +
   ADR-0001 (author it at this phase). **Reality-Check-Gate first** (read the
   orchestrator hub registration shape). **Resolve the canonical-state-access crux**
   (how the out-of-process MCP server reads the in-process `GraphState`) — if the
   resolution is non-obvious or needs a design decision, **STOP-and-Show** before
   writing tool code (the roadmap mandates resolving it first). Read-only tools
   only; no action tools.

For each AUTO phase, honour that phase's own Stop-and-Show conditions and audited
negatives (e.g. no `child_process`/exec, no agent egress, no `spacegraph-core` wire
bump in D0–D3/D5; the wire stays at PROTOCOL_VERSION 3 — **any need for a wire bump
means you have wandered into D4: STOP**).

**Merge policy (confirm with Sam if unset):** default is **branch-ready-for-review
per phase**, not auto-merge — auto-merge-on-green is acceptable only for the
pure-viewer AUTO phases (D0/D1/D2/D3/D5) if Sam enables it; **v0.6.0 (external
contract) is never auto-merged.**

**Gate (band):** each phase's gates green; `fmt`/`clippy`/`test --workspace` green
throughout; RUNLOG complete; ACCEPTANCE updated per phase.

## Phase 3 — HARD STOP at the offensive/mutating boundary

After v0.6.0, **STOP.** Do not enter Track C, Track E, D4, D6, or Track F. Produce a
final status report: what landed (with gates), the fresh recon delta, and the
explicit next steps — the NOT-AUTO phases, each requiring Sam's per-phase supervised
master-prompt (MP-E1 exists for E1; v0.7.0 / D4 / F reserved). Hand back to Sam.

---

## Quality gates (every commit, every phase)

- `cargo fmt --check` · `cargo clippy --workspace --all-targets -- -D warnings` ·
  `cargo test --workspace`.
- No `unwrap`/`expect` in render/IPC paths.
- **Audited negatives (per the auto-safe band):** the **agent stays read-only /
  no-egress / no-exec** (O-7'); **no `spacegraph-core` wire bump** (O-8 — would be
  D4); no scanner code (that is Track E, NOT in this MP); no AdminBot/action code
  (Track C). Assert at each phase's close-out.
- Conventional commits, English, imperative. **No AI-authorship markers.** Naming
  hygiene; existing-code-first; archive-not-delete.

## Stop-and-Show (mandatory pauses)

- Phase 0: unstable base / dirty tree from current sessions.
- Phase 1: recon finds docs↔code divergence violating a locked decision, or a
  finished session changed the architecture.
- Any phase: a need for a `spacegraph-core` wire bump (→ that's D4, stop).
- v0.6.0: the canonical-state-access crux needs a design decision; or the
  orchestrator Reality-Check surfaces a contract mismatch.
- **The boundary (Phase 3): always stop — never auto-enter Track C / E / D4 / D6 /
  F.**

## BLOCKED discipline

If genuinely blocked, write `BLOCKED.md`: phase, blocker, the ADR/ROADMAP clause in
tension, 1–2 options + recommendation. **Never** relax O-7'/O-8/O-10/O-11, the agent
no-exec guarantee, or the auto-boundary to get unblocked.

## Done

- Current sessions verified merged; governance docs in `docs/*`; old ROADMAP
  archived; base green.
- Fresh recon report with per-phase readiness.
- Auto-safe band landed: D0, D1, D2, D3, D5, and v0.6.0 (read-only MCP + ESN
  admission), each with gates green, RUNLOG + ACCEPTANCE updated.
- `spacegraph-core` still at PROTOCOL_VERSION 3 (no wire bump); agent still
  read-only/no-exec; no scanner/AdminBot/exploitation code.
- **Clean hard stop at the offensive/mutating boundary**, with a status report and
  the handoff list of NOT-AUTO phases for Sam's supervised MPs.
