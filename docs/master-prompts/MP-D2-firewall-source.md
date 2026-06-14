# MP-D2-firewall — nftables source (netlink) + firewall-as-gate viz

**Mode:** AUTO (Track D, read-only netlink source + viewer-side, no wire change).
**Repo root:** `/home/dev/SpaceGraph`
**Branch:** `feat/firewall-source`
**Depends on:** **D0** (the gated-aperture form exists but is dormant until this
source feeds it — ADR-0012 §1).
**Specs:** ROADMAP D2 (sibling source), ADR-0012 (the gated aperture it activates).
**Estimated size:** M–L (nftables netlink parsing is intricate).

## Mission
Read the host firewall ruleset and make gating legible: ports/sockets blocked or
filtered render as D0's shuttered aperture + barrier ring; the firewall becomes a
visible gate. Read-only, no exec.

## Pre-approved decisions
1. **nftables via read-only netlink (NFNL)** — **never** shell out to `nft` /
   `iptables` (that is exec, forbidden). New top-level dependency **`rustables`**
   (or `nftnl`) for the netlink read — **this is a new top-level dependency,
   approved by accepting this MP**; pin it, document it.
2. Lives in `spacegraph-agent` as a read-only source — preserves the agent
   no-exec/read-only guarantee (netlink read is a kernel query, not a process).
3. Derive a **per-port/socket gated state** (`allowed` | `filtered` | `blocked`)
   from the ruleset; feed D0's `aperture_style` gated form + a barrier ring. **No
   wire change** (the gated state is a viewer-side derivation; sockets already
   exist). Blocked-*attempt* flares are a flow/log concern (out of scope here).
4. Degrades to Minimal (gated form → flat torus).

## Out of scope
Blocked-connection-attempt flares (needs flow/log data — MP-D2-flow or later). Any
`nft`/`iptables` exec. Any wire bump. Writing firewall rules (read-only only).

## File paths
- `crates/spacegraph-agent/src/sources/firewall.rs` — netlink ruleset read + parse
  (+ committed `fixtures/` of a parsed ruleset sample).
- `crates/spacegraph-viewer/src/render/spatial.rs` — activate the gated aperture +
  barrier ring from the firewall-derived state (reuse `aperture_style`).
- `crates/spacegraph-viewer/src/render/theme.rs` — barrier-ring constant (if not
  already added in D0).
- Workspace `Cargo.toml` — the pinned netlink dependency.

## Phases & gates
- **P1 Netlink read + parse.** Read the ruleset via `rustables`; parse to a
  per-port/socket allow/filter/block model. *Gate:* ruleset fixture → expected
  gated states; **no `nft`/`iptables` exec anywhere** (audited); netlink read-only.
- **P2 Gated-state derivation.** Map ruleset → `aperture_style` gated form per
  socket. *Gate:* derivation unit-tested from a fixture ruleset + socket set.
- **P3 Viz.** Shuttered aperture + barrier ring on filtered/blocked ports. *Gate:*
  renders per gated state; Minimal → flat torus; no wire.
- **P4 Close-out.** Update ACCEPTANCE (D2-firewall), CODE_INVENTORY (new source +
  dep), DESIGN_LANGUAGE (barrier viz), RUNLOG.
  *Gate:* `fmt`/`clippy`/`test --workspace` green.

## Quality gates (every commit)
Standard set; **audited: no `nft`/`iptables`/`child_process` exec, no agent egress,
no `spacegraph-core` wire bump**; the netlink dep pinned + documented; no
AI-authorship markers; naming hygiene.

## Stop-and-Show
Before adding the netlink dependency, confirm it's `rustables`/`nftnl` (not a wrapper
that shells out). If the netlink ruleset model can't yield a clean per-socket gated
state without a graph/model change → stop and surface.

## Done
Read-only nftables netlink source; per-port gated state; shuttered-aperture +
barrier viz (activates D0); no exec/egress/wire-bump; dep pinned; docs updated.
Branch ready for review.
