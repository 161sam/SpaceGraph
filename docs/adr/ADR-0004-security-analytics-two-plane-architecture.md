# ADR-0004 — Security-analytics two-plane architecture

**Status:** Accepted — 2026-06-14
**Deciders:** Sam (161sam)
**Context window:** SpaceGraph ROADMAP v0.3 (§0.1, §0.2, §5); supersedes nothing,
grounds Track D, `v0.7.x`, and ADR-0005/0006/0007/0008/0009/0010/0011.

## Context

SpaceGraph is moving from a read-only observability tool toward a full
professional admin + cyber-security workspace (SIEM-class detection, ATT&CK
mapping, SOAR response, pentest visualization). A naive build would bolt active
capabilities — threat-intel lookups, CVE enrichment, host scanning, exploitation,
remediation — directly into SpaceGraph. That would destroy the property that
makes SpaceGraph trustworthy: its agent is **strictly read-only** (`AgentMode`
gates *which paths are read*, never write/execute) with an audited "no
`child_process`/exec in the tree" rule, and the viewer has **no outbound
network**.

The ESN fabric already provides the active half: AdminBot (privileged, gated
system actions), ABrain (reasoning), Smolit-Assistant (operator + pentest +
egress driver), OceanData (forensic store + audit), the orchestrator (MCP hub +
internet-facing execution). The question is not "how does SpaceGraph become a
SIEM/SOAR/pentest tool" but "where does each capability live."

## Decision

Adopt a **two-plane split** as the binding organizing principle:

- **SpaceGraph = the observability & detection cortex** ("eyes"): passive
  collection, graph-native normalization, detection/correlation, ATT&CK mapping,
  the spatial/temporal live workspace, coverage/posture, a read-only MCP provider
  surface. **It never acquires an egress path and never executes a system
  action.**
- **Smolit-Stack = the action, reasoning, egress & retention muscle** ("hands &
  brain"): AdminBot (actions + SOAR), ABrain (reasoning), Smolit-Assistant
  (operator + pentest + egress), OceanData (store + audit), orchestrator (hub +
  internet-facing execution).

The two planes connect over the MCP fabric and the AdminBot approval spine
(`Decision → Review → Approval → Execution → Audit`). The full professional tool
is the **fabric**; SpaceGraph is its safe plane within it.

Three consequences are locked as decisions:

### O-7 — Egress / enrichment ownership: Smolit-side only
SpaceGraph never makes an outbound-network call. Threat-intel, CVE lookup,
IP/domain/hash reputation, and neural malware verdicts (e.g. Nebula DAP) are
computed Smolit-side and flow back to SpaceGraph as **read-only node
annotations** over the fabric. Any PR adding a network client to the agent or
viewer is rejected by review. Rationale: a sink/lookup is an exfiltration and
attack surface; keeping SpaceGraph egress-free is the core safety guarantee of an
admin tool that watches a production host.

### O-8 — Wire-stability: governed `spacegraph-core` bumps
**Updated by ADR-0016.** The single sanctioned `PROTOCOL_VERSION` bump (3→4) was
spent by **v0.5.2 (FS-search, commit `ed2f5ce`)** for the search/materialise
messages — *not* by D4 as originally planned here. `PROTOCOL_VERSION = 4` is now
the baseline (`MIN_COMPATIBLE_PROTOCOL = 3`; the `Hello`-mismatch reject is
intact). No further `spacegraph-core` schema or `PROTOCOL_VERSION` change without
governance review. Security-analytics work (Track D, pre-D4) **reuses existing
node/edge kinds**: detections emit as `Node::Alert`; ATT&CK technique/tactic and
purple-team origin ride as **viewer-side fields / source strings**, not wire
types. D4's node-model extension (`Entity`, new `EdgeKind`s, vitals) is evaluated
when D4 is designed — additively over protocol 4 where the `MIN_COMPATIBLE` scheme
allows, else a governed bump.
Rationale: a wire change couples agent and viewer versions and crosses the
agent/viewer privilege boundary; governing bumps (rather than forbidding them)
keeps Track D auto-safe and avoids unintentional schema churn before the fabric's
read shape is fixed.

### O-9 — Passive-until-gated: no scan/probe trigger before v0.7.0
SpaceGraph does not *trigger* scans, probes, or exploitation before the `v0.7.0`
AdminBot approval layer exists. External tools (Suricata, ClamAV, Nebula, nmap)
run **independently**; SpaceGraph ingests their output via the `EventSource`
pattern (`suricata_eve` is the precedent). When triggering arrives, it is a
**mutating AdminBot action** onboarded one-at-a-time under the spine — never a
native SpaceGraph capability.

### Detection placement (binding for ADR-0005)
Detection runs **viewer-side** over the canonical in-memory `GraphModel`, not
agent-side. Detections become first-class `Node::Alert` (reusing the existing
cap/triage/timeline plumbing), with `source = "spacegraph-rule"`. This is what
keeps D1 wire-stable (O-8) and auto-safe.

## Alternatives considered

- **Build egress/enrichment into SpaceGraph.** Rejected: destroys the read-only
  safety guarantee; duplicates a capability the fabric already owns.
- **Run detection agent-side.** Rejected: the agent is the privilege-sensitive
  collector; keeping it dumb and read-only is the security boundary. Detection
  belongs where the canonical graph lives (the viewer).
- **One monolith that does collection→response.** Rejected: that is precisely the
  architecture the ESN fabric exists to avoid; SpaceGraph consumes AdminBot, it
  does not become a new privileged daemon.

## Consequences

- Track D1/D2/D3/D5 are AUTO and wire-stable; D4 (wire bump) waits for `v0.6.0`;
  remediation/scan-trigger/playbooks are Smolit-side and gated (`v0.7.0`/`v0.7.x`).
- SpaceGraph's value proposition sharpens: the one thing the fabric lacks — a
  graph-native, spatial-temporal detection + visualization cortex — with zero new
  attack surface.
- A "purple-team" view becomes possible: authorized pentest activity (observed
  from Smolit/Nebula output) and real attacker activity render in one scene,
  disambiguated by origin (ADR-0009), because both arrive as read-only streams.

## References

- ROADMAP §0.1 (two-plane split), §0.2 (capability-layer map), §5 (binding
  discipline), §6 (decisions O-7/O-8/O-9).
- `crates/spacegraph-agent/src/sources/mod.rs` — `EventSource` trait (the passive
  collection extension point).
- `crates/spacegraph-core/src/lib.rs` — `Node::Alert` + `id_alert` (the type
  detections reuse).
- `Smolit-Assistant ADR-0005` (AdminBot safety boundary), `OceanData
  OPERATOR_APPROVAL_ARCHITECTURE.md` (approval discipline) — the Smolit-side
  contracts the action plane hard-pins.
