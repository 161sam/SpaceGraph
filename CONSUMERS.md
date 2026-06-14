# CONSUMERS.md — SpaceGraph cross-repo relationships

Per the ESN convention (ROADMAP §3): one entry per cross-repo relationship. This
records who SpaceGraph talks to and the contract on each edge. SpaceGraph joins the
fabric as the 7th Hexa-Repo member (Tier 3) at `v0.6.0` (O-5, ADR-0001).

## 1. Purpose

SpaceGraph is a **read-only security-graph provider**: it builds a canonical graph
of host/network/detection state and exposes it to the ESN fabric. It is a provider,
not an actor — no mutating/offensive capability crosses any edge (O-7').

## 2. Consumed by SpaceGraph (upstream)

| Provider | Contract | Transport | Notes |
|---|---|---|---|
| `spacegraph-agent` | `Msg`/`Delta`, `PROTOCOL_VERSION = 4` | Unix domain socket (length-delimited JSON) | Local collector. **Read-only / no-exec / no-egress** (ADR-0004 O-7'). Both the viewer and `spacegraph-mcp` are UDS *clients*. No wire bump at v0.6.0 (O-8). |

SpaceGraph consumes no other ESN service at `v0.6.0`.

## 3. Provided by SpaceGraph (downstream)

### `mcp__spacegraph__*` — read-only MCP provider (ESN orchestrator hub)

- **Consumer:** the ESN orchestrator hub (and, through it, every fabric consumer —
  D4's AI-fabric tap, the Cockpit embed v0.6.x, the consumer phases).
- **Producer:** `crates/spacegraph-mcp` — a standalone **stdio** binary the hub
  spawns by `command` and proxies as `mcp__spacegraph__<tool>`.
- **Contract:** MCP (JSON-RPC 2.0 over stdio, protocol revision `2024-11-05`).
  Tool result schema is **owned by `spacegraph-mcp`** (JSON), distinct from the
  agent wire (ADR-0001 §3). This contract does **not** touch `PROTOCOL_VERSION`.
- **Tools (read-only, O-7'):** `topology_stats`, `node`, `alerts`, `explain_path`,
  `campaigns`, `coverage`, `posture`. **No action/mutating tools.**
- **Admission:** a hub registry row via `esn_mcp_server_register`
  (`server_id: "spacegraph"`, `transport: "stdio"`, `command: <spacegraph-mcp>`).
  See `docs/INTERFACE_INVENTORY.md` for the Tier-3 row.
- **Auth posture — L1:** the hub spawns the server **locally over stdio** (no
  network listener; the binary opens no TCP/HTTP port). Its only outbound socket is
  the **loopback Unix domain socket** to the local agent. JWT bearer applies where
  the hub injects it; **RS256/JWKS (L3) is out of scope** at v0.6.0.

## Status

- **Live-smoke (2026-06-14):** registered `spacegraph` with the live hub via
  `esn_mcp_server_register`, confirmed it in `esn_mcp_server_list` (registry count
  4 → 5, row present with the read-only surface), then removed it (transient smoke;
  permanent registration awaits Sam's review of the `v0.6.0` branch).
