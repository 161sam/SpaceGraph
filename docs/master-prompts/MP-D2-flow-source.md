# MP-D2-flow — Traffic-flow source (conntrack) + traffic-as-flow viz

**Mode:** AUTO (Track D, read-only flow source + viewer-side, no wire change).
**Repo root:** `/home/dev/SpaceGraph`
**Branch:** `feat/flow-source`
**Depends on:** the existing edge-pulse render (extends it).
**Specs:** ROADMAP D2 (sibling source) + §0.3 (traffic-as-flow).
**Estimated size:** M–L.

## Mission
Make traffic volume/direction legible: connection edges carry a flow beam whose
weight tracks bytes/s — exfil = a fat outbound beam through the perimeter, a beacon
= a thin rhythmic pulse. Read-only, no exec, eBPF stays deferred.

## Pre-approved decisions
1. **Flow data from conntrack** (read-only): netlink `ct` or `/proc/net/nf_conntrack`
   for per-connection byte/packet counters; **eBPF stays deferred** (roadmap). If a
   netlink-conntrack crate is used, **it is a new top-level dependency, approved by
   accepting this MP** — pin + document. `/proc/net/nf_conntrack` needs no dep.
2. Lives in `spacegraph-agent` as a read-only source — no exec, no egress.
3. Derive a **per-edge rate** (bytes/s, direction) and **extend the existing edge
   pulse** into a weighted flow beam — **no wire change** (rate is a viewer-side
   field over existing `connects_to` edges; emit counters via the existing edge
   stats path if it fits, else a viewer-side rolling rate).
4. Degrades to Minimal (flow beam → plain edge).

## Out of scope
eBPF. Any wire bump. Per-packet capture / DPI (this is counters, not payloads). Any
exec.

## File paths
- `crates/spacegraph-agent/src/sources/flow.rs` — conntrack read + per-conn counter
  parse (+ `fixtures/`).
- `crates/spacegraph-viewer/src/render/` — beam weighting from rate (thickness/
  brightness/particle-rate ∝ bytes/s; outbound-weighted for exfil).
- `crates/spacegraph-viewer/src/render/theme.rs` — flow-beam constants.

## Phases & gates
- **P1 Flow source.** Read conntrack counters → per-connection byte/rate. *Gate:*
  conntrack fixture → expected per-conn counters; **no exec, no egress** (audited);
  read-only.
- **P2 Rate derivation.** Per-edge rolling rate (bytes/s) + direction. *Gate:*
  rate computation unit-tested from a fixture counter series.
- **P3 Viz.** Beam weight ∝ rate; outbound-weighted exfil look; thin rhythmic
  beacon. *Gate:* weight mapping unit-tested; Minimal → plain edge; no wire.
- **P4 Close-out.** Update ACCEPTANCE (D2-flow), CODE_INVENTORY (+ any dep),
  DESIGN_LANGUAGE (flow viz), RUNLOG. *Gate:* `fmt`/`clippy`/`test --workspace`.

## Quality gates (every commit)
Standard set; **audited: no exec, no agent egress, no `spacegraph-core` wire bump**;
any dep pinned + documented; no AI-authorship markers; naming hygiene.

## Stop-and-Show
If per-edge rate can't ride a viewer-side field and seems to need an edge-schema/wire
change → stop (that would be a wire bump; reconsider scope). Confirm the conntrack
read path (netlink vs `/proc`) before adding any dependency.

## Done
Read-only conntrack flow source; per-edge rate; weighted flow-beam viz; no
exec/egress/wire-bump; docs updated. Branch ready for review.
