# ADR-0001 — SpaceGraph MCP provider surface + canonical-state access

**Status:** Accepted — 2026-06-14
**Deciders:** Sam (161sam)
**Depends on:** ADR-0004 (two-plane; O-7' read-only/no-egress, O-8 wire-stability),
ADR-0005/0006/0007 (the viewer-side detections/campaigns/coverage the surface
exposes).
**Implemented by:** MP-v0.6.0 (Track B). **Never auto-merged** — external ESN
contract (Sam reviews).

## Context

v0.6.0 makes SpaceGraph an ESN **provider**: expose the graph as
`mcp__spacegraph__*` (read-only) and join the fabric as the 7th Hexa-Repo member
(Tier 3). It is the keystone — D4's AI-fabric tap, the Cockpit embed (v0.6.x), and
the consumer phases all depend on it. Two P0 questions had to be resolved before
any tool code (the MP-v0.6.0 Phase-0 gate).

### Reality-Check (live hub, read-only — `esn_mcp_server_list`)

The orchestrator hub registers **stdio MCP servers it spawns by `command`** and
proxies their tools as `mcp__<server_id>__<tool>` (confirmed live: `abrain` →
`/opt/venvs/abrain/bin/abrain-mcp`; `sequential-thinking` → `npx … server-
sequential-thinking`; `memory`; `esn-orchestrator`). An admission row is a
registry entry: `{ server_id, transport: "stdio", command, args, env, … }` via
`esn_mcp_server_register`. **This matches the ROADMAP §2 assumption** (`mcp_proxy/`,
`mcp__spacegraph__*`, stdio) — **no contract mismatch.** Implication: the hub
spawns the MCP server as a **separate process**, so it cannot directly read the
viewer's in-process `GraphState`.

### The crux

The MCP tool surface (topology stats, node query, alert feed, explain-path) needs
the **viewer's synthesized canonical state** — D1 detections, D3 campaigns, D5
coverage/posture **live only in the viewer's `GraphState`**, never on the agent
wire. The agent serves raw collector deltas (UDS, `PROTOCOL_VERSION 4`); the
viewer is a UDS *client* that builds the canonical model and runs the
detection/correlation/coverage pipeline. So "the MCP server consumes the agent"
is **insufficient** (it would lack the synthesized state, or have to duplicate the
whole pipeline).

## Decision

### 1. Extract a headless canonical-state core
The canonical state + its pipeline — `GraphModel`, delta ingest from the agent
UDS, D1 detection (`rules`), D3 correlation, D5 coverage/posture — is extracted
into a **headless crate** (`crates/spacegraph-graph`, name TBD at P1) that depends
on **neither Bevy nor any render/GUI code**. It exposes read-only queries
(topology stats, node lookup, alert/detection feed, campaigns, explain-path,
coverage, posture). The **viewer renders over this core**; an MCP/headless service
hosts the same core **without a GUI**. This is the always-on-provider choice: the
MCP surface must not require a running GUI session.

### 2. MCP server = a thin stdio binary hosting the core
`crates/spacegraph-mcp` is a standalone **stdio** binary (the hub's registry
`command`) that hosts the headless core (ingesting from the agent UDS) and exposes
the four **read-only** tools over MCP. **No action/mutating tools** (O-7'; those
are Track C/AdminBot). Per-tool contract test (fixture graph → typed result).

### 3. No `spacegraph-core` wire bump (O-8)
The MCP tool schema is a **new, separate contract** owned by `spacegraph-mcp`
(JSON tool results), distinct from the agent wire (`Msg`/`Delta`,
`PROTOCOL_VERSION 4` untouched) — the O-8 "scanner has its own contract"
precedent. The headless core reuses the existing `spacegraph-core` types unchanged.

### 4. Admission + auth posture
Admission = an `INTERFACE_INVENTORY.md` Tier-3 row + a `CONSUMERS.md` provider
entry, realized as a hub registry row (`server_id: "spacegraph"`,
`transport: "stdio"`, `command: <spacegraph-mcp>`) via `esn_mcp_server_register`.
Auth posture **L1** (loopback/UDS; the hub spawns the server locally over stdio;
JWT bearer where used). RS256/JWKS (L3) is out of scope.

## Alternatives considered

- **Viewer hosts a read-only query-UDS; thin stdio bridge** (simplest). Rejected
  as the v0.6.0 target: ties MCP availability to a running GUI viewer — the
  provider should be always-on. (It remains a valid fallback if the extraction
  proves too large.)
- **Snapshot file/IPC** the viewer publishes; separate MCP process reads it.
  Rejected: staleness + duplicates the query logic (explain-path/node-query) in
  the MCP server.
- **MCP consumes the agent.** Rejected: insufficient — the agent has raw collector
  data only, not the viewer's synthesized detections/campaigns/coverage.

## Consequences

- A clean separation of **canonical state (headless)** from **rendering (viewer)**
  — independently valuable (testability, an always-on provider, future headless
  deployments). Larger P1 refactor: `GraphState`/pipeline are currently
  Bevy-coupled (the extraction is the bulk of v0.6.0).
- D4's AI-fabric MCP tap and the Cockpit embed build on the same core + surface.
- Read-only, no egress, no wire bump — O-7'/O-8 preserved. **Never auto-merged.**

## References

- ROADMAP v0.6.0 (Track B) + §2 (contract map); O-5 (Tier-3 admission), O-7'/O-8.
- MP-v0.6.0 (Phase 0 gate). Live Reality-Check via `esn_mcp_server_list`.
- `crates/spacegraph-viewer/src/graph/{state,model,rules,correlation,coverage,
  posture}.rs` — the pipeline to extract; `net/uds.rs` — the agent-UDS client.
