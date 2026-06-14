# INTERFACE_INVENTORY.md — SpaceGraph (Tier-3 admission row)

> The **canonical** `INTERFACE_INVENTORY.md` is the shared ESN-org Hexa-Repo
> inventory (not in this repo). This file records **SpaceGraph's row** so it can be
> propagated upstream when SpaceGraph is formally admitted (O-5, ADR-0001). Until
> then this is the authoritative source for that row.

## Admission row

| Field | Value |
|---|---|
| Repo | `SpaceGraph` |
| Tier | **3** (provider; promotion to Tier 2 targeted at `v1.0`) |
| Admitted at | `v0.6.0` (7th Hexa-Repo member) |
| Role | Read-only security-graph **provider** |
| Provider surface | `mcp__spacegraph__*` (MCP, stdio, proto `2024-11-05`) |
| Hub registry id | `spacegraph` (`transport: stdio`, `command: <spacegraph-mcp>`) |
| Tools (read-only) | `topology_stats`, `node`, `alerts`, `explain_path`, `campaigns`, `coverage`, `posture` |
| Mutating tools | **none** (O-7') |
| Agent wire | `PROTOCOL_VERSION = 4` (unchanged — O-8) |
| Auth posture | **L1** (loopback/UDS; hub-spawned stdio; JWT bearer where used; RS256/JWKS = L3, out of scope) |
| ADR | ADR-0001 (MCP provider surface + canonical-state access) |

## Hub registry row (for `esn_mcp_server_register`)

```json
{
  "server_id": "spacegraph",
  "display_name": "SpaceGraph (read-only graph provider)",
  "transport": "stdio",
  "command": "<absolute path to the spacegraph-mcp binary>",
  "args": [],
  "is_default_active": false,
  "is_required": false
}
```

Notes:
- `command` is an absolute path to the built `spacegraph-mcp` binary (a release
  build at a stable install path for production; the v0.6.0 live-smoke used the
  workspace `target/debug/spacegraph-mcp`).
- The agent UDS path is given as positional arg 1 or `$SPACEGRAPH_AGENT_UDS`; with
  neither, the server serves an empty graph (so `tools/list` works without an
  agent).
- Registered **dormant** (`is_default_active: false`) until reviewed.

## Live-smoke result (2026-06-14)

`esn_mcp_server_register` → row accepted; `esn_mcp_server_list` → registry count
4 → 5 with the `spacegraph` row present; `esn_mcp_server_remove` → `removed: true`
(transient smoke; permanent registration awaits Sam's review of the `v0.6.0`
branch). End-to-end stdio smoke of the binary itself (`initialize` / `tools/list` /
`tools/call`) is green — see RUNLOG v0.6.0 P5.
