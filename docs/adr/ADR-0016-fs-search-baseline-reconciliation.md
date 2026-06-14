# ADR-0016 — FS-search baseline reconciliation: protocol-4 ratification + agent no-exec restoration

**Status:** Accepted — 2026-06-14
**Deciders:** Sam (161sam)
**Amends:** ADR-0004 §O-8 (wire-stability). The O-8 *discipline* is retained and
re-scoped from "defer the one bump" to "govern future bumps".
**Reaffirms:** ADR-0013 / O-7' (the agent's read-only / no-egress / **no-exec**
guarantee).
**Context source:** `docs/recon/RECON_REPORT-2026-06-14-auto-safe-band.md`
(MP-ORCH Phase 1).

## Context

The v0.5.2 "Filesystem Search & Index" feature (merged in commit `ed2f5ce`,
tagged `v0.5.2`) shipped two changes that silently violated locked decisions the
roadmap and the auto-execution master-prompt (MP-ORCH) treat as inviolable, and
the ROADMAP v0.5 did not reflect either:

1. **It spent the single sanctioned wire bump.** `spacegraph-core` went
   `PROTOCOL_VERSION` 3 → 4 to carry the new `SearchRequest` / `SearchResponse` /
   `MaterialiseRequest` messages. ADR-0004 §O-8 had reserved the *one* 3→4 bump
   for **D4** (node-model extension), deferred behind v0.6.0. No `Node`/`EdgeKind`
   variant changed (the 6-Node / 7-EdgeKind set is unchanged); the bump was purely
   the search message family. `MIN_COMPATIBLE_PROTOCOL` stayed 3.
2. **It introduced exec into the agent.** A `SystemLocate` backend shelled out via
   `std::process::Command` to the system `plocate`/`locate`/`mlocate` binary,
   breaking the agent's **no-exec** invariant (O-7', ADR-0013): "no
   `child_process`/exec anywhere in the tree (audited)".

MP-ORCH Phase 1 (recon) caught both as docs↔code drift on locked decisions
(O-7'/O-8) and stopped, because the master-prompt forbids relaxing those
invariants or auto-fixing an architecture change a finished session made. This
ADR records Sam's reconciliation decision.

## Decision

### 1. Ratify `PROTOCOL_VERSION = 4` as the baseline (amends O-8)

The 3→4 bump is **accepted as spent** by v0.5.2. `PROTOCOL_VERSION = 4` is the
canonical wire baseline; `MIN_COMPATIBLE_PROTOCOL = 3` (a v3 peer still negotiates
where compatible, and a pre-handshake/legacy peer that decodes to protocol 0 is
rejected by the `Hello` check — already proven by the v0.5.2 migration). Reverting
the bump is not pursued: v0.5.2 is a tagged release and the ON-DISK search UI
depends on it.

O-8 is **re-scoped from timing to governance**: no further `spacegraph-core`
schema / `PROTOCOL_VERSION` change without an explicit governance review. The
"single bump" framing is retired — the discipline is now "bumps are deliberate and
reviewed," not "there is exactly one, reserved for D4."

### 2. D4 no longer *owns* the bump

D4 (node-model extension + boundary + AI-fabric + vitals) is no longer "the one
phase that crosses the wire boundary." Its deferral behind v0.6.0 now rests on the
**AI-fabric MCP tap needing the provider surface**, not on a pending wire bump.
D4's own schema additions (`Node::Entity`, the new containment/AI `EdgeKind`s, the
vitals message) are evaluated **when D4 is designed**: carried additively over
protocol 4 where the `MIN_COMPATIBLE` scheme allows, otherwise via a governed
bump. This ADR does **not** pre-decide that D4 needs zero wire change — only that
the 3→4 bump it was once allotted is already spent.

### 3. Restore the agent no-exec invariant (reaffirms O-7')

The `SystemLocate` shell-out is **removed**. The filesystem index standardizes on
the in-tree **builtin walker** (already present as the fallback): it walks the
policy-scoped roots into a cached path list and stays fresh via the existing
inotify watches. Consequences:

- `spacegraph-agent` contains **no** `std::process::Command` / `child_process` /
  subprocess spawn (audited; only `std::process::id()` for temp-dir naming
  remains, which is not exec).
- The `IndexSource` selector (`auto`/`plocate`/`builtin`), the `--index-source`
  agent flag, and the viewer `[search] index_source` config are removed — there is
  one source now.
- Search **scope** is the policy root-set applied at walk time (not a system-wide
  index post-filtered). This is security-by-default; an operator widens coverage
  with `--include`. The `path_allowed` post-filter remains the single
  search-time security chokepoint.

## Consequences

- The ROADMAP, ADR-0004 §O-8, and `docs/spec_fs_search_index.md` are updated to
  state `PROTOCOL_VERSION = 4` and the walker-only / no-exec index.
- The MP-ORCH auto-safe band can resume against a truthful, invariant-holding
  baseline: agent **no-exec** holds again, and the audited negative for the band
  becomes "**no further** wire bump" (the wire is at 4, not 3).
- Behavior change for FS-search: results are bounded to the agent's scoped roots
  rather than a host-wide `locate` index; freshness depends on inotify rather than
  the OS `updatedb` cadence.

## Alternatives considered

- **Revert the wire to 3.** Rejected: v0.5.2 is tagged/shipped and the search UI
  rides protocol 4; un-shipping a wire version is disruptive for no safety gain
  (the bump is back-compatible, `MIN_COMPATIBLE = 3`).
- **Keep `SystemLocate` behind an ADR carve-out for read-only local `locate`.**
  Rejected by Sam: it weakens an audited invariant (no-exec anywhere) for a
  capability the builtin walker already covers; the walker keeps O-7' literally
  true with no exec surface.
- **Read the `plocate`/`mlocate` DB directly (no exec).** Rejected: the DB formats
  are proprietary/version-fragile (plocate `frcode`); parsing them is brittle on
  OS upgrades for marginal benefit over the walker.
