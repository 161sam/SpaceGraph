# ADR-0012 — Perimeter & exposure visual model

**Status:** Accepted — 2026-06-14
**Deciders:** Sam (161sam)
**Depends on:** ADR-0004 (two-plane; O-8 wire-stability).
**Implemented by:** ROADMAP D0 (AUTO, near-term, parallel to `v0.5.0`).

## Context

The single most security-relevant property of a host's network surface — **what
is reachable from where** — is invisible in SpaceGraph today. `Node::Socket`
carries `state` ("LISTEN"/"ESTABLISHED"/…) and `local_addr`, but rendering is
flat: a listening socket and an established one are both a blue torus, and a
loopback-only service looks identical to an internet-exposed one. Firewall gating
has no source at all. Meanwhile a detection just spawns a red node; the operator
sees *that* something is wrong, not *where*.

Crucially, the most valuable parts of the perimeter story are **derivable from
data already collected** — so they must not wait behind the `v0.6.0` wire bump
(O-8). This ADR fixes the no-wire visual model; the boundary *region* primitive
and live state are D4/ADR-0008.

## Decision

Four viewer-side derivations, **no wire change, no new data field** (D0):

### 1. Port-state-as-aperture
The Socket torus (the "port aperture") renders per `Socket.state`:
- **LISTEN** → an open, glowing aperture facing outward;
- **ESTABLISHED** → an aperture with an active flow beam to its remote;
- **filtered / gated** → a shuttered aperture with a barrier ring (this state
  arrives only once the D2 firewall source exists; until then the aperture has no
  gated form);
- **closing** (TIME_WAIT/CLOSE_WAIT/…) → dimming.

A pure function `aperture_style(state) -> ApertureStyle` selects the form (unit-
testable, like `render::spatial::highlight_style`).

### 2. Exposure-as-depth
A socket's **reachability** is derived from `local_addr` into a bucket —
`Loopback` (127.0.0.0/8, ::1), `Lan` (RFC1918 / link-local / ULA), `Public`
(everything else) — and drives **radial position**: `Public` listeners on the
outer shell facing the perimeter, `Lan` mid, `Loopback` buried at the core. Attack
surface becomes readable as **silhouette**: a host with many outward-facing
apertures *looks* exposed. A pure `exposure_bucket(local_addr) -> Exposure`
function is the single source of truth.

### 3. Anomaly-as-scene-distortion
An alert/detection **localizes the post-fx** around its subject — a bounded
ripple / local desaturation of the "normal" / a focus-pull — so the eye is drawn
to *where* something is wrong, not merely *that* an alert exists. Reuses the
existing `render::postfx` pass (scanlines/vignette/CA/grain), parameterized by
proximity to the alerted node; bounded in radius/intensity; forced off under
Minimal (`postfx_active`), never clobbering saved config.

### 4. Gateway as a derived node
The default route is read from `/proc/net/route` by the existing `net` source and
emitted as a `RemoteHost` (the gateway address) — **reusing an existing kind, no
new type, no wire bump.** It is the anchor the D4 internet-membrane portal later
attaches to; at D0 it simply appears as the egress hub every outbound
`connects_to` passes near.

All four key off `theme.rs` constants (no ad-hoc `Color::srgb`), and all degrade
to the Minimal theme without changing graph state (visuals never mask truth).

## Alternatives considered

- **Add an `exposure` field to `Node::Socket` on the wire.** Rejected (O-8):
  exposure is a pure function of `local_addr`, derivable viewer-side; no bump
  needed.
- **Wait and fold this into D4.** Rejected: this is high security + visual value
  at low risk, needs no new data, and should not be gated behind the `v0.6.0`
  wire bump just because it is "visual like D4."
- **A distinct alert-glow only (no scene distortion).** Rejected: a red node
  among thousands does not direct attention to *location*; localizing the post-fx
  does.

## Consequences

- D0 ships the no-wire half of the perimeter story now, in parallel with
  `v0.5.0`; the boundary *region* (internet membrane as space) and the firewall
  *gated* aperture form arrive later (D4 / D2).
- Exposure and port-state become first-class visual semantics an operator reads at
  a glance — the core security-legibility win — with zero new attack surface.

## References

- ROADMAP D0, §0.3 (visualization catalog), §5 (visuals never mask truth).
- `crates/spacegraph-core/src/lib.rs` — `Node::Socket { proto, local_addr,
  local_port, state }` (the fields D0 derives from; unchanged).
- `crates/spacegraph-agent/src/sources/net.rs` — `tcp_state_name`, the `LISTEN`/
  `ESTABLISHED` handling, and the `/proc/net/route` read point for the gateway.
- `crates/spacegraph-viewer/src/render/postfx.rs` + `DESIGN_LANGUAGE.md`
  (cyberspace post-process) — the pass the anomaly distortion localizes.
- `crates/spacegraph-viewer/src/render/spatial.rs::highlight_style` — the pure-
  function style-picker pattern `aperture_style`/`exposure_bucket` follow.
