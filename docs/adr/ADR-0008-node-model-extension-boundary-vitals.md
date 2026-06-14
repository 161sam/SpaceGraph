# ADR-0008 — Node-model extension + boundary primitive + AI-fabric + vitals

**Status:** Accepted — 2026-06-14
**Deciders:** Sam (161sam)
**Depends on:** ADR-0004 (two-plane; O-8 wire-stability, O-10 extensibility).
**Implemented by:** ROADMAP D4 (behind `v0.6.0`). Authored now, built at the phase.

## Context

SpaceGraph's `Node` is a closed Rust enum of six primitives. To represent the
real world — VMs, containers (Docker/LXD/Qubes), namespaces, pods, AI agents, AI
models, and classes not yet imagined — and to make virtualization/perimeter
topology *legible*, three needs converge, and all three require the wire opened:

1. **Open-ended types.** Adding `Node::Vm`, `Node::Container`, `Node::Agent`, …
   one-by-one means a `PROTOCOL_VERSION` bump per type — exactly the schema churn
   O-8 forbids.
2. **Boundaries as space.** A process runs *inside* a container *inside* a VM *on*
   a host, reachable *through* a gateway *to* the internet. A flat node-edge graph
   renders containment poorly; the perimeter ("the internet and access to it")
   needs a boundary primitive, not more point-nodes.
3. **Live state.** The numbers `top`/`htop`/`ss`/`df` show (CPU/RSS/load/disk/
   throughput) are node/host *attributes* SpaceGraph does not yet carry.

O-8 wants **one** well-designed bump after `v0.6.0`, not 3→4→5→6. This ADR
designs that single bump so it serves all three needs and leaves the model
permanently extensible.

## Decision

A single `PROTOCOL_VERSION` 3→4 change (at D4, behind `v0.6.0`), documented
migration, `Hello`-mismatch reject intact. It introduces:

### 1. Closed-core + open-extension node model (O-10)
The six core kinds stay **first-class enum variants, hand-designed** (the quality
bar). A generic variant carries the long tail as *data*:

```
Node::Entity { class: ClassId, attrs: Map<String, Value> }
```

New classes (VM, Container, Pod, Namespace, Agent, Model, BSD-jail, gVisor, …) are
**registered**, not coded: the agent declares a class once (id + `VisualHint`,
below) via a registration message; the wire then carries `Entity` generically.
**Adding the next class needs no further wire change.** New edge kinds:
`Contains`/`RunsIn` (containment) and `Invokes`/`Reasons`/`Proposes`/`ToolCall`
(AI). `Inference`/`ToolCall` are **edges/events**, never nodes (bound graph
growth).

> The precise `Value`/attrs encoding and the registration message shape are this
> ADR's implementation concern at D4; the binding decision here is the
> *closed-core + registered-open-class* shape, not the serialization detail.

### 2. Derived-visual function (the disciplined form of "procedural generation")
An extension class declares a small `VisualHint`:
- `family` — a coarse archetype (Compute / Data / Identity / Network / Boundary /
  AI / Threat) → base geometry + palette band.
- a few `axis` scalars (e.g. privilege, exposure, ephemerality) → modulate within
  the archetype (size, shell density, ring presence).
- the stable `class_id` hash → a deterministic accent (hue rotation within the
  band, facet count) so distinct classes in one family are
  **distinguishable-but-evidently-related**.

The renderer **derives a cached mesh/material per class** (computed once, cached
exactly like `NodeRenderResources` caches per-kind today — never per-node,
never per-frame). **Hand-authored overrides** are registered for the classes that
matter (VM, Container get bespoke geometry); the long tail gets the derived form.
Under Minimal, derived classes fall back to the flat sphere + label.

**Procedural means deterministic derivation from semantics within an envelope —
not generative randomness.** Binding rule: derived appearance must encode *real*
semantics, be reproducible (an operator learns the language), stay within the
family envelope, and never replace the hand-designed primitives.

### 3. Boundary / containment render primitive — one primitive for all boundaries
`Contains`/`RunsIn` defines parent→child; the renderer draws children inside a
translucent **boundary hull** of the parent. **The same primitive serves:**
- **the internet membrane** — the outermost boundary; everything not-local is
  "outside"; the gateway/default-route/NAT is its **portal**; exposure (D0) reads
  as depth relative to it.
- **VM / Container nesting** — a VM is a region containing container sub-regions
  containing process nodes.
- **trust zones** — privilege/trust regions; escalation = a visible boundary
  crossing; lateral movement = a path across zones.

It also enables **semantic zoom**: collapse containers → VMs → hosts → zones on
zoom-out, expand on zoom-in (the navigation answer for 10k-node scenes).

### 4. Virtualization source
A `virt` `EventSource` (libvirt / Docker / LXD / Qubes qrexec — **read-only**,
no shell-out) emits `Entity` classes (VM/Container/Pod/Namespace) + `Contains`
edges.

### 5. AI-fabric
`Agent`/`Model` as registered classes (or core kinds), sourced **primarily from
the orchestrator MCP tap** (ESN agents already flow through the hub with
`correlation_id`), secondarily from `nebula`/local-inference processes. Renders
agent→model→target with the AI edge kinds.

### 6. Telemetric state & vitals — read the source, not the tool
Per-process CPU/RSS/threads/state from `/proc/<pid>/stat|statm`; a host-vitals
message (load/mem/swap/disk/throughput from `/proc/stat`, `/proc/meminfo`,
`/proc/loadavg`, statvfs, `/proc/net/dev`). **Read via procfs/sysfs the agent
already reads — never by spawning `top`/`htop`/`ss`/`df`** (`child_process`/exec
is forbidden; TUI-scraping is fragile; the tools read the same procfs anyway).
Encoded two ways, both required to *replace* htop rather than complement it:
- **vitality** — CPU/mem/IO drive node pulse-rate/size/instability (a thrashing
  process vibrates, a leaking one swells, a flapping service flickers);
- **numbers** — a live state readout in the inspector + a **system-vitals HUD**
  (the htop header line, in-scene), so an operator drills to exact figures without
  leaving the scene.

The goal is to **subsume each tool's *data* into the one scene, not clone its
UI** — same data, better surface.

## Alternatives considered

- **A `kind: String` on every node (no closed core).** Rejected: loses the
  hand-designed quality of the primitives and invites the gray-sphere-soup the
  design language fought; the hybrid keeps the core first-class.
- **One enum variant per new type, multiple bumps.** Rejected: the schema churn
  O-8 exists to avoid; the registered-class mechanism makes one bump permanent.
- **Generative/random node art.** Rejected: not legible, not learnable; derivation
  must be deterministic and semantic.
- **Spawn `top`/`htop` for state.** Rejected: breaks the no-exec safety rule,
  fragile TUI-scraping, and unnecessary — read procfs/sysfs directly.
- **Containment as styling only (no boundary primitive).** Rejected: the
  perimeter/virtualization use case needs *space* (regions), which point-nodes
  and edges cannot convey; the boundary hull is the feature.

## Consequences

- One wire bump (3→4) opens extensibility, boundaries, AI-fabric, and vitals
  together; thereafter new node classes are data, never further bumps.
- The boundary primitive answers "visualize the internet and access to it" *and*
  VM/Container *and* trust zones with one mechanism.
- D4 is large (L–XL) — the boundary primitive alone is layout + render work — and
  is therefore deferred behind `v0.6.0` and run as its own master-prompt with the
  Reality-Check; it inherits no auto-mode guarantee.
- D0 (perimeter & exposure, ADR-0012) ships the no-wire half of the perimeter
  story now; D4 adds the boundary *region* and live state on the bump.

## References

- ROADMAP D4, §0.3 (visualization catalog), §5 (state-via-source + derived-visual
  discipline), §6 (O-8, O-10), Appendix A.6/A.7.
- ADR-0004 — O-8/O-10 it implements; the no-exec / two-plane invariants.
- `crates/spacegraph-core/src/lib.rs` — `Node`, `EdgeKind`, `PROTOCOL_VERSION`,
  `Capabilities` (the `ebpf`/`cloud`/`windows` flags the registration extends).
- `crates/spacegraph-viewer/src/render/spatial.rs` — `NodeRenderResources` (the
  per-kind handle cache the per-class derived cache mirrors).
- `crates/spacegraph-agent/src/sources/mod.rs` — the `EventSource` trait the
  `virt` source plugs into.
