# MP-v0.6.0 — SpaceGraph MCP server (read-only) + ESN admission

**Mode:** Auto-capable **with mandatory checkpoints** (a Reality-Check + a design
crux to resolve before tool code). **Never auto-merged** — this touches an external
ESN contract (the orchestrator hub).
**Repo root:** `/home/dev/SpaceGraph`
**Branch:** `feat/mcp-provider`
**Specs:** ROADMAP v0.6.0 (Track B) + §2 (contract map), ADR-0001 (MCP surface —
author at this phase). This is the **keystone** — D4, Track C, Track E6 depend on it.
**Estimated size:** L.

## Mission
Expose the graph as `mcp__spacegraph__*` (read-only) and join the ESN fabric as the
7th Hexa-Repo member (Tier 3). Read-only tools only — no action tools (those are
Track C, AdminBot).

## Phase 0 — Reality-Check + resolve the canonical-state crux (BEFORE tool code)

1. **Reality-Check-Gate.** Read the orchestrator hub registration shape
   (`mcp_proxy/`, how stdio-MCP servers are proxied as `mcp__<server>__*`) and the
   `INTERFACE_INVENTORY.md` admission-row format. Record the check in RUNLOG. If the
   live hub contract differs from the roadmap's assumption → **STOP and report.**
2. **Resolve the canonical-state-access crux.** Decide how an **out-of-process stdio
   MCP server reads the in-process `GraphState`.** Options: (a) the MCP server runs
   **in-process** with the viewer (same binary, shares state) and the hub spawns a
   thin stdio shim; (b) a **separate process** reads a snapshot/IPC the viewer
   publishes; (c) the **agent** exposes a read API the MCP server consumes. **This
   is a design decision — STOP-and-Show with a recommendation before writing tool
   code.** (The roadmap mandates resolving it first.)
**Gate:** Reality-Check recorded; the canonical-state approach chosen and confirmed
by Sam. Do not proceed to P1 until resolved.

## Pre-approved decisions (post-crux)
1. New crate `crates/spacegraph-mcp`. **Read-only tools only**: topology stats, node
   query, alert feed, explain-path. **No action/mutating tools** (audited).
2. Contract test per tool (fixture graph → typed result).
3. `INTERFACE_INVENTORY.md` row + **Tier 3**; `CONSUMERS.md` provider entry.
4. Auth posture L1 (loopback/UDS, JWT bearer where used; RS256/JWKS is L3, out of
   scope).

## Out of scope
Action tools / AdminBot (Track C). The Cockpit embed (`v0.6.x`). The AI-fabric MCP
tap (D4). Any mutating capability.

## File paths
- `crates/spacegraph-mcp/` — the new crate; read-only tool implementations;
  `fixtures/` for contract tests. (Plus whatever the P0 crux decision dictates for
  state access — e.g. an in-process shim vs a snapshot reader.)
- `INTERFACE_INVENTORY.md` — the admission row (Tier 3).
- `CONSUMERS.md` — the provider entry (§3 format).
- `docs/adr/ADR-0001-mcp-provider-surface.md` — author it (incl. the crux
  resolution).

## Phases & gates
- **P0** (above) — Reality-Check + crux. **Hard checkpoint.**
- **P1 Crate + read-only tools.** Implement the four read-only tools over the
  chosen state-access path. *Gate:* `tools/list` + `tools/call` smoke green.
- **P2 Contract tests.** Per-tool fixture → typed result. *Gate:* each tool's
  contract test green; **no action tool present** (audited).
- **P3 Admission.** `INTERFACE_INVENTORY.md` row (Tier 3) + `CONSUMERS.md` entry +
  live-smoke against the hub (documented). *Gate:* registration smoke documented;
  auth token path tested.
- **P4 Close-out.** Author ADR-0001; update ACCEPTANCE (v0.6.0), CODE_INVENTORY,
  RUNLOG. *Gate:* `fmt`/`clippy`/`test --workspace`.

## Quality gates (every commit)
Standard set; no `unwrap`/`expect` in IPC paths; **read-only only — no mutating/
action tool anywhere** (audited); **never auto-merged** (external contract — Sam
reviews); conventional commits, English; no AI-authorship markers; naming hygiene.

## Stop-and-Show
- **P0 always** (Reality-Check mismatch; the canonical-state crux decision).
- If any tool would need to *mutate* state → stop (read-only only).
- If the hub registration contract can't be satisfied at L1 → surface (don't reach
  for L3/remote transport).

## Done
`spacegraph-mcp` read-only tools over the resolved state-access path; per-tool
contract tests; Tier-3 admission row + CONSUMERS entry; live-smoke documented;
ADR-0001 authored. **No action tools.** Branch ready for **Sam's review** (not
auto-merged).
