# SpaceGraph — Architecture (workspace overview)

Top-level map of the Cargo workspace. The **binding viewer architecture** (module
boundaries, system order, multi-node, pin/interaction state) lives in
[`ARCH_VIEWER.md`](ARCH_VIEWER.md); the **graph/wire schema** in
[`GRAPH_SCHEMA.md`](GRAPH_SCHEMA.md); a full module map in
[`recon/CODE_INVENTORY.md`](recon/CODE_INVENTORY.md).

## Crates

- **`spacegraph-core`** — shared wire protocol & types (`Node`/`Edge`/`EdgeKind`/
  `Delta`/`Msg`, `PROTOCOL_VERSION = 4` (`MIN_COMPATIBLE = 3`), id constructors).
  No Bevy, no IO.
- **`spacegraph-agent`** — read-only host collectors behind the `EventSource`
  trait (`fs`, `proc`, `net` procfs sockets, `suricata_eve` alerts); serves a UDS
  stream of `Msg`. `AgentMode::{User, Privileged}` gates *which paths are read*.
- **`spacegraph-viewer`** — Bevy + egui renderer. Subsystems: `net/` (UDS client),
  `graph/` (canonical `GraphState` truth + force layout + timeline), `render/`
  (spatial/edges/geometry/post-fx/camera), `ui/` (panels/overlays/shortcuts),
  `util/` (config/ids), wired by `app/SpaceGraphViewerPlugin`.

## Data flow

```
Agent(s) → UDS (Msg, PROTOCOL_VERSION-checked) → net/ → Incoming
       → graph/ (GraphState, stream-namespaced) → capped projection
       → render/ (Spatial / Tree / Timeline) → ui/ overlays
```

## Boundaries (enforced)

`render/` never touches `net/` or raw events; `GraphModel` knows no UI/render
(the `graph/` substate may use bevy-math/ECS-resource types); `ui/` reads the
graph via the `GraphState` API, not `GraphModel` internals; `net/` knows no graph
structure. See `ARCH_VIEWER.md` for the authoritative rules.

## ESN fabric (roadmap)

SpaceGraph is moving into the ESN fabric as provider (MCP surface) and consumer
(AdminBot / ABrain / OceanData) — see [`ROADMAP.md`](ROADMAP.md).
