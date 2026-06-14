# MP-E1 — Scanner crate + scope model + discovery + port scan

**Mode:** **NOT auto-mode.** This is the active/offensive plane — SpaceGraph's first
egress component. Requires hard-stops, a scope-policy review, and per-capability
review. Do **not** run it autonomously against anything but the designated local
test surface.
**Repo root:** `/home/dev/SpaceGraph`
**Branch:** `feat/scanner-discovery-portscan`
**Authoritative specs:** ADR-0013 (active recon plane; O-7'/O-11), ADR-0014
(scanner architecture — author it as part of this MP), ROADMAP Track E1 + §5 + §6.
**Estimated size:** L.

---

## Non-negotiable safety rails (read first)

1. **Scope is a hard gate.** The scanner MUST refuse to do *any* network activity
   without an explicit `Scope`. No default-to-scan. This is enforced in code and
   asserted in tests — not a convention.
2. **CI/dev scans target ONLY:** `127.0.0.0/8`, a locally-spawned test listener,
   and the RFC5737 documentation ranges `192.0.2.0/24` / `198.51.100.0/24` /
   `203.0.113.0/24` (which route nowhere). **Never** scan real third-party or
   internet hosts as part of this MP. Tests that need "a target" use a localhost
   listener.
3. **No exploitation.** This MP builds *reconnaissance* only — discovery + port
   scan. No payloads, no exploit code, no auth-bypass, no access attempts. Audited
   in close-out (grep the tree).
4. **Every scan is audited** — an append-only record (timestamp, scope,
   authorization ref, target count, what was probed). Built in P2, used everywhere.
5. **Separate crate.** All of this lives in `crates/spacegraph-scanner`. **Do not**
   add egress, raw sockets, or `child_process` to `crates/spacegraph-agent` — its
   read-only/no-exec guarantee is preserved (O-7').

---

## Mission

Stand up the active reconnaissance plane: a new `spacegraph-scanner` crate that,
**given an explicit scope**, performs host discovery + SYN port scanning at a
controlled rate, audits each scan, and emits the discovered surface to the viewer
— rendered as outward-facing apertures (the D0 aperture vocabulary, mirrored). No
fingerprinting, no OS detect, no CVE, no index (those are E2–E4). No exploitation,
ever.

## Pre-approved decisions (execute; do not re-litigate)

1. **Separate crate `crates/spacegraph-scanner`** (ADR-0013). Workspace member.
2. **Native Rust** discovery + scan via raw sockets (`pnet` or equivalent;
   `CAP_NET_RAW`). No shelling to nmap/masscan in this phase (wrapping is an E2+
   option for fingerprinting, not needed here).
3. **`Scope` is first-class and mandatory:** `{ targets: Vec<Cidr>, mode:
   {OwnAuthorized | Arbitrary}, roe: RoeMeta { engagement_id, authorization_ref,
   owner }, rate: RateLimit }`. The scanner constructor/run path takes a `Scope`
   or returns an error — there is no scan without one.
4. **Both modes exist, default is `OwnAuthorized`.** `Arbitrary` is selectable but
   requires the RoE fields populated and emits a distinct audit marker. The tool
   does not block `Arbitrary`; it records it. (O-11 — the operator owns the call.)
5. **Rate-limited by default** (configurable pps cap) — protects targets and the
   operator; no unbounded blasting.
6. **Audit is append-only** and never silently dropped (a failed audit write aborts
   the scan, it does not proceed unlogged).
7. **Viewer integration is minimal here:** discovered hosts → `RemoteHost` nodes,
   open ports → outward apertures (reuse ADR-0012's `aperture_style`, pointed
   outward). **No `spacegraph-core` wire bump** — discovered infra rides
   `RemoteHost` + enrichment for now; full `Entity`-class integration is E6/D4.

## Explicitly out of scope (reject if tempted)

- Service/banner fingerprinting, TLS/cert, OS detection, CVE correlation, the
  searchable index, reporting, evasion (E2–E5).
- **Any exploitation / access / payload capability** — not now, not as a "stub".
- Any change to `spacegraph-agent` (egress/raw-socket/exec).
- Any `spacegraph-core` wire bump.
- Scanning anything outside the local test surface in CI/dev.

---

## Architecture & file paths

- `crates/spacegraph-scanner/` — new crate (add to the workspace `Cargo.toml`
  members). Modules:
  - `scope.rs` — `Scope`, `Cidr` (reuse the agent's `Cidr` parse logic by
    extracting it to a shared spot or re-implementing minimally; do not depend the
    scanner on the agent), `RoeMeta`, `RateLimit`, `Mode`. `Scope::validate()`.
  - `audit.rs` — append-only scan audit record + writer (a failed write aborts).
  - `discovery.rs` — host liveness (ICMP echo / ARP for local / TCP-SYN to a
    common port), pure parse/decision fns where possible.
  - `portscan.rs` — SYN scan, rate-limited, returns `{host, open_ports}`.
  - `engine.rs` — ties scope → discovery → portscan → results; refuses without a
    valid `Scope`; writes audit.
  - `contract.rs` — the result type emitted to the viewer (the scanner's own data
    contract; not the agent wire).
  - `fixtures/` — committed fixtures (a parsed packet sample, a CIDR set, an
    expected-result for the local-listener test).
- `crates/spacegraph-viewer/src/...` — consume the scanner contract; render
  discovered hosts as `RemoteHost` + **outward** apertures (reuse
  `render::spatial::aperture_style` from D0, oriented outward); a minimal "recon"
  surface (discovered hosts beyond the perimeter). Behind a config toggle.
- `docs/adr/ADR-0014-spacegraph-scanner-architecture.md` — author it (engine
  shape, the `Scope` object, native-vs-wrap boundary, the viewer contract, the
  RFC5737 test posture). Per the WoW "ADR per decision."

**Boundary:** the scanner is a standalone crate; the viewer consumes its contract.
The scanner does not touch `spacegraph-agent`.

## Phases & gates (each: implement → test (local surface only) → `fmt`/`clippy`/`test` → RUNLOG)

**P1 — Crate + `Scope` + the hard gate.** The crate, `scope.rs`, and an `engine`
entrypoint that **refuses without a valid `Scope`**.
*Gate:* `Scope::validate` tests (empty targets → error; `Arbitrary` without RoE →
error; valid → ok); the engine returns an error when constructed/run without a
scope (the hard gate, asserted). No network in this phase.

**P2 — Audit.** Append-only record; a failed write aborts the scan.
*Gate:* an audit record is written per scan attempt with scope + mode marker; a
simulated write failure aborts (asserted); `Arbitrary` mode emits its distinct
marker.

**P3 — Discovery.** Host liveness against the local surface.
*Gate:* discovery against a localhost listener + the loopback range returns the
expected live/!live set; rate limit honoured; **only loopback/RFC5737/local
listener touched** (the test harness asserts no other destination).

**P4 — Port scan.** SYN scan, rate-limited.
*Gate:* SYN scan against a localhost listener on known-open + known-closed ports
returns the correct open set; rate cap honoured; integration test binds its own
listener — **no external target**.

**P5 — Viewer surface.** Discovered hosts → `RemoteHost` + outward apertures;
config toggle; a minimal recon view.
*Gate:* the scanner contract → rendered `RemoteHost`s with outward apertures (assert
on the mapping fn); no wire bump; Minimal-degrades; existing viewer tests green.

**P6 — Close-out.** Author `ADR-0014`. Update `docs/ROADMAP.md` Track-E1 status,
`docs/ACCEPTANCE.md` (E1 criteria incl. the scope-gate + RFC5737 posture),
`docs/recon/CODE_INVENTORY.md` (new crate), `docs/perf/RUNLOG.md`. **CONSUMERS/
licensing note:** flag in RUNLOG that ADR-0015 (license + EULA) is release-blocking.
*Gate:* full `test --workspace` green; clean `clippy`; the audited negatives below.

## Quality gates (every commit, non-negotiable)

- `cargo fmt --check` · `cargo clippy --workspace --all-targets -- -D warnings` ·
  `cargo test --workspace`.
- No `unwrap`/`expect` in the scan/IO paths (a scan must fail safe, never panic
  mid-probe).
- **Audited negatives (assert in close-out):**
  - the scanner refuses without a `Scope` (the hard gate);
  - **no exploitation/payload/access code** anywhere in the crate (grep);
  - `spacegraph-agent` unchanged — no egress / raw socket / `child_process` added;
  - no `spacegraph-core` wire bump;
  - tests reference only `127.0.0.0/8` / a bound localhost listener / RFC5737 —
    **no real external destination** (grep the test sources).
- Conventional commits, English, imperative. **No AI-authorship markers.** Naming
  hygiene (`Scope`, `discovery`, `portscan` — no `enhanced`/`v2`/`pro`).

## Test posture

All network tests bind a **localhost listener** the test itself controls, or use
loopback/RFC5737 destinations. Pure fns (CIDR membership, scope validation, packet
parse, the contract→render mapping) are unit-tested with committed fixtures. **No
test scans a real host.** GPU/visual confirmation documented in RUNLOG.

## Stop-and-Show (pause, RUNLOG note, surface to Sam)

- **Before P3** (first actual network capability): pause for Sam to confirm the
  `Scope` model + the RFC5737/localhost test posture are what he wants — this is
  the scope-policy review checkpoint.
- If raw sockets / `CAP_NET_RAW` require a privilege/setcap setup decision beyond
  the dev box → surface it (don't silently run privileged).
- If anything pulls toward fingerprinting/exploitation/index → **stop** (E2+ / out
  of scope).
- If discovered infra seems to need a new node kind / wire change → **stop** (reuse
  `RemoteHost`; full `Entity` integration is E6/D4).

## BLOCKED discipline

If genuinely blocked, write `BLOCKED.md`: phase, blocker, the ADR/ROADMAP clause in
tension, 1–2 options + recommendation. **Never** relax the scope gate, the no-exec
agent guarantee, or the local-test-only posture to get unblocked.

## Done

- `spacegraph-scanner` crate: `Scope` hard-gate, append-only audit, native
  discovery + rate-limited SYN scan, a viewer contract; discovered hosts rendered
  as `RemoteHost` + outward apertures.
- Scope gate, audit, both-modes (default OwnAuthorized), rate-limit, contract→
  render mapping all tested; **all network tests local-surface only**.
- `ADR-0014` authored; `ROADMAP`/`ACCEPTANCE`/`CODE_INVENTORY`/`RUNLOG` updated;
  ADR-0015 (license/EULA) flagged release-blocking.
- Audited negatives green: no-scope-refusal, no-exploitation, agent-unchanged,
  no-wire-bump, no-external-target.
- `spacegraph-agent` and `spacegraph-core` untouched.
- Branch `feat/scanner-discovery-portscan` ready for review (NOT auto-merged —
  Track E lands on review, per phase).
