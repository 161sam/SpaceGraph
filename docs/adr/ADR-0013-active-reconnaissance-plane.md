# ADR-0013 — Active reconnaissance plane (dual-plane within SpaceGraph)

**Status:** Accepted — 2026-06-14
**Deciders:** Sam (161sam)
**Supersedes:** ADR-0004 §O-7 (egress ownership). ADR-0004's *two-plane*
discipline and the agent's read-only guarantee are **retained and re-scoped**, not
discarded.
**Implemented by:** ROADMAP Track E (Active Reconnaissance). Authored now; per-
capability detail authored at each Track-E phase.

## Context

SpaceGraph's direction is changing deliberately. It extends from a passive
monitoring/observability tool into a **Monitor + Recon + Red-Team workspace** —
a professional pentest tool intended for **commercial monetization as an ESN
product**. To make the "visualize the internet" capability real and to be useful
for red-team operations, SpaceGraph gains its own **active, aggressive scanner**
that reproduces Shodan-class capability (discovery, port scanning, service/banner
fingerprinting, TLS/cert inspection, OS detection, CVE correlation, a searchable
index).

This reverses ADR-0004's O-7 ("egress stays Smolit-side; SpaceGraph never gains an
egress path; SpaceGraph stays a safe read-only admin tool"). The reversal is
deliberate and owner-decided. The job of this ADR is to do it *cleanly* — to gain
the active capability without throwing away the property that made the monitoring
side trustworthy, and to build the operator/legal controls in from day one.

## Decision

### 1. SpaceGraph spans two planes internally; the line moves
The old line was *passive vs active*. The new line is **intelligence/recon
(SpaceGraph) vs system-action (Smolit)**:

- **Passive observation plane** — `spacegraph-agent`, read-only host collection.
  **Its read-only / no-egress / no-exec guarantee is PRESERVED** — you can still
  run SpaceGraph as a safe monitor on a production or client host.
- **Active reconnaissance plane** — a new `spacegraph-scanner` crate: discovery +
  scanning + fingerprinting, with egress, scope-gated.

Both gather *intelligence*. **The scanner does not exploit or modify targets** —
exploitation is a distinct, more sensitive capability deferred to a separate later
decision (Nebula / Smolit territory), not built under Track E. SpaceGraph's role
extends to *offensive reconnaissance*, not offensive *action*; system-action
(remediation, AdminBot) stays the Smolit plane.

### 2. The scanner is a separate crate (`spacegraph-scanner`)
**Not welded into the agent** — this is correct engineering independent of safety:
- **Deployment:** the read-only agent runs *on* watched hosts (incl. prod, incl.
  a client's box during an engagement); the scanner runs *from* an ops box / VPS /
  red-team infra, *against* targets. Different locations and lifecycles.
- **Blast radius:** a compromised read-only agent leaks observation; a compromised
  scanner is a weapon. Separation bounds the damage.
- **Dependencies / privilege:** the scanner pulls raw-socket / packet-crafting
  libraries (`pnet` / raw sockets) and needs `CAP_NET_RAW`; the agent must not
  carry that surface.

The viewer visualizes **both planes in one scene** — the defended estate (inside-
out) and the scanned target surface / internet (outside-in). The two-plane
discipline of ADR-0004 does not die; it **moves inside SpaceGraph**.

### 3. Scope / authorization model — both modes, scope-gated
A first-class **`Scope`** object: target CIDR sets + rules-of-engagement metadata
(engagement id, authorization reference, owner) + a mode flag + rate/aggressiveness
controls. **Both scan modes are supported (the new decision):**
- **own/authorized** (the default) — your estate + RoE-defined engagement targets;
- **internet-wide / arbitrary** — an explicit, operator-owned, **audited** opt-in
  mode, with rate-limiting + abuse/opt-out handling.

The scanner **refuses to run without an explicit scope** (hard gate). Every scan
is audited (what, when, against which scope, under which authorization). The
operator owns the arbitrary/third-party call. **Scope-gating is not a safety-nanny
restriction** — it is how red teams operate (RoE), it protects the operator from
accidental out-of-scope scanning mid-engagement (the cardinal red-team sin), and
for a commercial product it is a liability control. The tool always *knows,
records, and audits* its scope; it does not hide the arbitrary mode, nor does it
make it the default.

> **Legal reality (operator-owned, stated once):** scanning third-party
> infrastructure without authorization is unlawful in most jurisdictions —
> directly under German law §202 StGB (tooling) and unauthorized-access statutes
> (use). The arbitrary mode exists because the operator may have lawful grounds
> (their own ranges, contracted engagements, research with standing); the
> responsibility for that determination is the operator's, and the audit trail
> exists to evidence it.

### 4. Native-core + optional tool-wrap
Implement discovery / fast-scan / fingerprinting **natively in Rust** (control,
performance, no dependency on installed binaries). Optionally **wrap mature tools**
(zgrab2, nmap NSE) where reimplementing is wasteful — the scanner is the sanctioned
active component, so the no-exec rule does **not** bind it. The **agent keeps
no-exec.** The native-vs-wrap boundary per capability is decided at its Track-E
phase.

### 5. Wire / data model
O-8 is unchanged for the **agent** wire. The scanner has its **own data contract**
to the viewer. Discovered infrastructure (scanned host, service, cert, vuln) is
naturally **`Entity`-class under the O-10 extension model** → full graph
integration rides **D4** (the extension carrier + derived visuals). Initial viz
(before D4) reuses `RemoteHost` + enrichment annotations (the mirrored D0
aperture/exposure vocabulary, turned outward) — no agent wire bump.

### 6. Commercial / licensing (now live, reserved)
As a monetized ESN product with an offensive component, **licensing** (dual-
license, the Cockpit `ADR-053` AGPL+Commercial precedent) and an **authorized-use
EULA** become live, **release-blocking** decisions — the existing `LICENSE` TODO
must resolve before commercial release. The offensive capability + monetization
raises the scope/audit/EULA story to a *liability* concern (cf. how commercial
red-team tooling controls distribution and vetting). This is a Sam/Johanna
business decision; its ADR is reserved.

## Alternatives considered

- **Weld the scanner into the agent.** Rejected: deployment, blast-radius, and
  dependency reasons above; would also forfeit the still-valuable read-only
  monitor.
- **Keep SpaceGraph read-only; scan from Smolit (the old O-7).** Rejected: the
  owner's direction is that SpaceGraph itself owns the recon plane and is the
  red-team workspace.
- **Build exploitation now.** Deferred: meaningfully larger and more sensitive
  than recon; a separate later decision, kept out of Track E so the scanner build
  stays bounded and Shodan-class ("what Shodan does" = recon, not exploitation).
- **No scope object / unconstrained by default.** Rejected: even "both modes" needs
  an explicit, audited scope; defaulting to arbitrary forfeits the operator
  protection and the commercial liability control, and is not how professional
  tooling works.

## Consequences

- A new **Track E — Active Reconnaissance** (multi-phase): scanner crate + scope
  model → discovery/port-scan → fingerprint/TLS → OS/CVE → searchable index →
  red-team/reporting/engagement → full Entity-class viz integration (rides D4).
- The **agent's read-only guarantee is preserved**; SpaceGraph can still be the
  safe monitor. The no-exec rule now binds the agent, not the scanner.
- The unified scene shows defended estate + recon surface together — the "visualize
  the internet" capability becomes real (scoped, the practical red-team form;
  internet-wide possible as the audited operator-owned mode).
- Track E is **security-sensitive and NOT auto-mode** — each phase its own master-
  prompt with hard-stops; the scope gate is a hard code requirement; CI/dev tests
  scan only loopback / RFC5737 documentation ranges, never real third-party
  targets.
- Licensing + EULA become release-blocking for the commercial product.

## References

- ADR-0004 (the O-7 it supersedes; the two-plane discipline it re-scopes; the
  agent read-only guarantee it preserves).
- ROADMAP Track E, §0.1 (dual-plane-within-SpaceGraph), §6 (O-7'/O-11).
- ADR-0008 / O-10 — the `Entity` extension model the scanner's discovered infra
  rides (D4 dependency for full graph integration).
- ADR-0012 — the D0 aperture/exposure vocabulary, mirrored outward for scanned
  remote-host surfaces.
- `crates/spacegraph-agent/src/sources/mod.rs` — the read-only collector plane the
  scanner is deliberately *separate* from.
- ESN-Cockpit `ADR-053` — the dual-license (AGPL + Commercial) precedent for the
  reserved licensing decision.
