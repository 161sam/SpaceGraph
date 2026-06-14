# MP-v0.6.0-P1 — Headless-core extraction + MCP server + ESN admission

**Mode:** Autonomous execution on a branch. Not offensive/mutating (no egress, no
scanner, no AdminBot) — the risk here is *breaking the viewer*, managed by
compile-green-every-step + behavior preservation.

> **Merge-policy amendment (2026-06-14, authorized by Sam):** the original
> "**NEVER auto-merged**" stance for v0.6.0 is **superseded** — Sam authorized
> **auto-merge-on-green** for *all* phases P1–P6, with the `v0.6.0` tag on `main`.
> Each phase merges to `main` `--no-ff` (local; no push) once the quality gates are
> green, mirroring the D0–D5 `AUTO` convention. The carve-out rationale (external
> ESN contract → human pre-merge review) is explicitly waived. Recorded in
> ADR-0001 and memory `spacegraph-auto-band-state`.
**Repo root:** `/home/dev/SpaceGraph`
**Branch:** continue on **`feat/mcp-provider`** (P0 landed at `f194c7f`: Reality-Check
passed, crux decided = headless-core extraction; ADR-0001 + RUNLOG persisted).
**Specs:** ROADMAP v0.6.0 (Track B) + §2; ADR-0001 (P0 decision — amend with the
implementation); O-7' (read-only MCP), O-8 (no `spacegraph-core` wire bump).
**Estimated size:** XL. This is a focused-session refactor; pace it.

## Mission
Execute the P0 crux decision: extract a **headless graph core** (`spacegraph-graph`,
no Bevy) that owns the graph model, agent-UDS ingest, the D1/D3/D5 pipeline, and
read-only queries; refactor the viewer to render *over* that core; then add a
standalone read-only **`spacegraph-mcp`** stdio server that hosts the core and
exposes the read-only tools to the ESN hub. Tag `v0.6.0`.

## The cardinal rule of this refactor
**The workspace compiles and `cargo test --workspace` is green after every commit.**
No big-bang. No broken intermediate state on the branch. Move the least-coupled
things first; let the compiler find every call site; preserve behavior. Each
extraction step is its own commit (bisectable/revertible). **All 275 tests stay
green and move with their code — coverage is preserved or grows, never lost.**

## Invariants (audited at every phase close)
- `PROTOCOL_VERSION` stays **4** — **no `spacegraph-core` wire bump** (the MCP has
  its own contract, O-8).
- `spacegraph-agent` **untouched** — read-only / no-exec / no-egress preserved.
- The MCP server exposes **read-only tools only** — **no action/mutating tools**
  (O-7').
- The viewer **renders identically** — Minimal equivalence holds; the D0–D5 visual
  phases (aperture/exposure/anomaly/gateway, threat-motion, campaigns, coverage,
  posture) still work. Behavior-preserving refactor.
- No scanner / AdminBot / exploitation code anywhere.

## Phases (each: implement → workspace compiles → `fmt`/`clippy`/`test --workspace` green → commit → RUNLOG)

**P1 — `spacegraph-graph` skeleton.** Create the crate (no Bevy deps), add to the
workspace `members`. Empty/placeholder.
*Gate:* workspace builds + tests green (nothing moved yet).

**P2 — Move the pure pipeline.** Move `GraphModel` + the already-pure D1/D3/D5 cores
(`rules`/`correlation`/`coverage`/`posture`/`explain` — built as pure fns in their
MPs) into `spacegraph-graph`, with their tests. The viewer depends on
`spacegraph-graph` and uses them.
*Gate:* workspace compiles; all moved tests green in their new home; viewer still
builds + renders; no behavior change.

**P3 — Move the UDS ingest.** Move the agent-UDS client (`net/uds.rs`) + protocol
decode into `spacegraph-graph` (both the viewer and the MCP server ingest). The core
gains an "ingest agent stream → graph" capability.
*Gate:* compiles; the viewer's ingest path still works (decode/round-trip tests
green); no wire change (still v4).

**P4 — Decouple `GraphState` (the bulk).** Extract the graph data + state-mutation +
query logic from the viewer's `GraphState` into a core type (e.g. `GraphCore` in
`spacegraph-graph`) that owns the graph + ingest + pipeline + read-only queries
(`topology_stats`, `node`, `alerts`, `campaigns`, `explain_path`, `coverage`,
`posture`). `GraphState` becomes a thin Bevy `Resource` wrapping a `GraphCore` +
**only** render/ui fields (`needs_redraw`, spatial layout, ui state). Rewire all
viewer call sites through the core. **Iterative, compiler-driven — small commits.**
*Gate:* workspace compiles; **ALL tests green**; **viewer renders identically**
(Minimal equivalence + D0–D5 phases — documented GPU capture in RUNLOG); the core's
read-only queries are unit-tested headless (fixture graph → typed result).
**→ Natural review milestone:** after P4 the headless core is stable and the
extraction is done. **STOP and report** — Sam may want to review the extraction
before the MCP crate. Resume on his go.

**P5 — `spacegraph-mcp` crate.** New crate = the stdio binary the hub spawns by
`command`. It instantiates `GraphCore` **headless** (ingests from the agent over UDS
independently) and exposes the read-only tools over MCP: topology stats, node query,
alert feed, explain-path (+ campaigns/coverage/posture if in scope). Per-tool
**contract tests** (fixture graph → typed result). **No action tools** (O-7').
*Gate:* `tools/list` + `tools/call` smoke green; per-tool contract test green;
**read-only only** (audited — no mutating tool present); the binary runs headless
(no Bevy).

**P6 — Admission + close-out.** `INTERFACE_INVENTORY.md` **Tier-3** row +
`CONSUMERS.md` provider entry; **live-smoke against the hub** (`esn_mcp_server_register`,
documented); auth **L1** (loopback/UDS, JWT bearer where used; RS256/JWKS is L3, out
of scope). Amend **ADR-0001** with the implementation; update ACCEPTANCE (v0.6.0),
CODE_INVENTORY (two new crates), RUNLOG. **Tag `v0.6.0`.**
*Gate:* full `test --workspace` green; `PROTOCOL_VERSION` still 4; read-only only;
registration smoke documented; auth path tested.

## Quality gates (every commit)
`fmt --check` · `clippy --workspace --all-targets -D warnings` · `test --workspace`;
no `unwrap`/`expect` in render/IPC/MCP paths; **audited negatives:** no wire bump
(stays 4), agent untouched, MCP read-only only, no scanner/AdminBot/exploitation
code; conventional commits, English; **no AI-authorship markers**; naming hygiene
(`spacegraph-graph`, `GraphCore` — no `enhanced`/`v2`/`core2`); existing-code-first;
archive-not-delete.

## Stop-and-Show (pause, RUNLOG note, surface to Sam)
- **P4 decoupling forks:** if "what stays Bevy vs. moves to core" needs a design
  call (e.g. a field that's ambiguously render-vs-state) → surface with a
  recommendation.
- **Circular dependency** between crates emerges → stop; restructure deliberately.
- **The UDS client can't cleanly move** (entangled with Bevy) → surface.
- **Behavior can't be preserved** (a render regression not trivially fixable) →
  stop; do not paper over it.
- **MCP independent-ingest problem** (the agent can't serve a second consumer) →
  surface.
- **After P4** (extraction milestone): stop for Sam's optional review before P5.
- Any temptation toward an **action/mutating MCP tool** or a **wire bump** → stop
  (both are out of bounds: O-7'/O-8).

## BLOCKED discipline
If genuinely blocked, write `BLOCKED.md`: phase, blocker, the ADR/ROADMAP clause in
tension, 1–2 options + recommendation. **Never** relax behavior-preservation, the
no-wire-bump rule, the read-only-MCP rule, or the agent guarantee to get unblocked.
Never leave the branch in a non-compiling state to "come back to it."

## Done
- `spacegraph-graph` (headless): `GraphModel` + agent-UDS ingest + D1/D3/D5 pipeline
  + read-only queries, unit-tested headless.
- Viewer refactored to render over the core; `GraphState` a thin Bevy resource;
  **renders identically** (Minimal + D0–D5 verified); all tests green.
- `spacegraph-mcp`: standalone read-only stdio server hosting the core; the 4 (+)
  read-only tools with per-tool contract tests; **no action tools**.
- Tier-3 admission row + CONSUMERS entry + documented hub live-smoke + L1 auth;
  ADR-0001 amended; docs updated; **tag `v0.6.0`**.
- `PROTOCOL_VERSION` still 4; agent untouched; no offensive/mutating code.
- Branch ready for **Sam's review** — **not auto-merged**. The offensive/mutating
  boundary (Track C / E / D4 / D6 / F) remains untouched.