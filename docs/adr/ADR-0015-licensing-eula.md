# ADR-0015 — Licensing & EULA (commercial ESN product)

**Status:** Accepted — 2026-06-14 (counsel-cleared; binding text held by counsel,
vault sync pending)
**Deciders:** Sam (161sam) + legal counsel
**Resolves:** the `LICENSE` TODO; the release-blocking flag in ADR-0013 §6.
**Relates:** ESN-Cockpit `ADR-053` (dual-license precedent).

## Context

SpaceGraph is being monetized as a commercial ESN product (ADR-0013) and now
carries an active reconnaissance plane (Track E) and a planned, separately-licensed
exploitation capability via Smolit/AdminBot (a future track). A monetized product
with offensive capability needs a licensing model and an authorized-use EULA before
commercial release. Counsel has resolved the licensing question; this ADR records
the adopted model. The binding legal text lives with counsel and will be synced
into the vault / `LICENSE` + `EULA` files; this ADR is the decision record, not the
legal instrument.

## Decision

1. **Dual-license** the codebase (the Cockpit `ADR-053` pattern): an open-source
   license for the core + a separate **commercial license** for paid use — the
   model proposed in ADR-0013 §6, adopted.
2. **Authorized-use EULA** governs the offensive/recon capability: the scanner
   (Track E) and any future exploitation capability may only be operated against
   targets the licensee is authorized to test (own assets / contracted
   engagements / lawful standing). The EULA carries the RoE / authorized-use terms
   that the in-product `Scope` gate + audit trail (O-11) operationalize.
3. **Premium tier for offensive depth.** The **exploitation / full red-team
   capability is a separately-purchased add-on / premium tier — not in the normal
   subscription**. The recon plane's tiering is per the commercial license; the
   exploitation tier is gated above it (the future track's monetization).
4. **Binding text is counsel's.** The specific license identifiers, commercial
   terms, and EULA wording are authored/owned by counsel; this repo's `LICENSE`,
   `LICENSE-COMMERCIAL`, and `EULA` files are populated from that text once the
   vault syncs. Do not invent or paraphrase legal terms in code or docs.

## Consequences

- The `LICENSE` TODO and the ADR-0013 §6 release blocker are **resolved** — no
  longer blocking.
- The offensive capability's responsible-use story is now three-layered:
  **technical** (the `Scope` hard-gate + audit, O-11), **contractual** (the
  authorized-use EULA), and **commercial** (premium-tier gating for exploitation).
- Headers / `LICENSE*` / `EULA` files are populated on vault sync; until then this
  ADR is the authority that the question is resolved.

## References

- ADR-0013 §3/§6 (scope/authorization; the release-blocking licensing flag this
  resolves).
- ESN-Cockpit `ADR-053` (dual-license AGPL + Commercial precedent).
- O-11 (the in-product scope/audit gate the EULA's authorized-use terms rest on).
