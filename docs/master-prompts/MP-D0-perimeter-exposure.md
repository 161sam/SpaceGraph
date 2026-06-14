# MP-D0 — Perimeter & exposure visual pass (+ governance-doc placement)

**Mode:** AUTO (Track D, viewer-side + one read-only agent read, no wire change,
no publish, no egress).
**Repo root:** `/home/dev/SpaceGraph`
**Branch:** `feat/perimeter-exposure-visual`
**Parallel to / independent of:** `MP-v0.5.0` (Track A) and `MP-D1` (rule engine)
— D0 touches socket rendering + the `net` source + post-fx; D1 touches
`graph/rules.rs`. No shared files; either order, or concurrently.
**Authoritative specs:** ADR-0012 (perimeter & exposure visual model), ADR-0004
(two-plane; O-8 wire-stability, no-exec), ROADMAP D0 + §0.3 + §5.
**Estimated size:** M.

---

## Phase 0 — Place the staged governance docs (do this FIRST, own commit)

The new ROADMAP + ADRs + MPs were downloaded into a staging folder under `docs/`
(`docs/files(23)/`, or whatever the exact name is — confirm by listing). Move them
to their correct locations, archive the superseded ROADMAP (archive-not-delete),
and verify a clean tree before any implementation.

```bash
cd /home/dev/SpaceGraph
ls -la docs/                      # locate the staging folder
ls -la "docs/files(23)"           # confirm the 7 staged files (adapt name if different)

# 1. Archive the superseded ROADMAP (never delete)
mkdir -p docs/archive/2026-06-14-roadmap-v0.2
git mv docs/ROADMAP.md docs/archive/2026-06-14-roadmap-v0.2/ROADMAP.md

# 2. New ROADMAP -> docs/
git mv "docs/files(23)/ROADMAP.md" docs/ROADMAP.md

# 3. ADRs -> docs/adr/
mkdir -p docs/adr
for f in ADR-0004-security-analytics-two-plane-architecture.md \
         ADR-0005-graph-native-detection-rule-engine.md \
         ADR-0006-attack-coverage-dimension.md \
         ADR-0008-node-model-extension-boundary-vitals.md \
         ADR-0012-perimeter-exposure-visual-model.md; do
  git mv "docs/files(23)/$f" "docs/adr/$f"
done

# 4. Master prompts -> docs/master-prompts/
mkdir -p docs/master-prompts
git mv "docs/files(23)/MP-D1-detection-rule-engine.md" docs/master-prompts/MP-D1-detection-rule-engine.md

# 5. Save THIS prompt too (the one you are executing)
#    -> docs/master-prompts/MP-D0-perimeter-exposure.md

# 6. Remove the now-empty staging folder
rmdir "docs/files(23)" 2>/dev/null || true
```

**Phase-0 gate:** `git status` shows the 7 docs at their new paths + the archived
old ROADMAP + this MP saved; the staging folder is gone; no file deleted (only
moved/archived). Commit as `docs: place v0.4 roadmap + security ADRs, archive
v0.2 roadmap`. **Then** start the implementation phases below.

**Stop-and-Show:** if the staging folder name or contents differ from the above
(e.g. extra/missing files), **list what is actually there and stop** — do not
guess placements for unexpected files.

---

## Mission (implementation)

Make the most security-relevant invisible properties legible, with no new data and
no wire change: **port state** (open/established/gated/closing as aperture forms),
**exposure** (loopback/LAN/internet as radial depth → attack surface as
silhouette), **anomaly locality** (a detection distorts the scene where it is),
and the **gateway** as a derived egress hub. All keyed off `theme.rs`, all degrade
to Minimal without changing graph truth.

## Pre-approved decisions (do NOT re-litigate — execute)

1. **No wire change** (O-8). Everything derives from `Node::Socket` fields already
   present (`state`, `local_addr`) and reuses `Node::RemoteHost` for the gateway.
   If anything seems to need a `spacegraph-core` change — **stop**.
2. **No exec, no egress** (O-7). The gateway comes from reading `/proc/net/route`
   in the existing `net` source — never a shell-out. No outbound network.
3. **Pure functions for the semantics**, mirroring
   `render::spatial::highlight_style`: `aperture_style(state) -> ApertureStyle` and
   `exposure_bucket(local_addr) -> Exposure { Loopback | Lan | Public }`. These are
   the single sources of truth and are unit-tested.
4. **Theme split:** aperture forms + anomaly distortion are **Standard-only**
   (Minimal keeps the flat torus + plain alert). **Exposure-as-depth is
   informational placement and applies in both themes** (it positions truth, it is
   not decoration).
5. **Exposure-depth refines the existing shell**, it does not rewrite layout:
   sockets get a target shell band (`Public` outer · `Lan` mid · `Loopback` core)
   fed as a soft constraint through the existing `progressive_prepare` shell
   factor. Deeper layout surgery is out of scope — if needed, **stop**.
6. **Anomaly distortion extends the existing post pass**, it is not a new render
   pass: feed the projected screen position(s) + intensity of a bounded set of
   alerts (top-N by severity/recency) as uniforms to `render::postfx`; the WGSL
   ramps a local ripple/desaturation by proximity. Bounded count, Minimal-off.
7. **Gated aperture form is defined but dormant** — the "filtered/gated" state only
   arrives once the D2 firewall source exists. Implement the form; it simply has no
   input yet. Do not build a firewall source here (that is a separate D2 MP).

## Explicitly out of scope (reject if tempted)

- Any `spacegraph-core` change / `PROTOCOL_VERSION` bump.
- The firewall source, the traffic-flow source (separate D2 MPs).
- The internet-membrane *region* / boundary hull (that is D4 / ADR-0008 —
  needs the boundary primitive + the wire bump). D0 ships the gateway as a *node*
  and exposure as *depth*, not the membrane as a region.
- New node/edge kinds, vitals, AI-fabric (all D4).
- Any `child_process`/exec, any outbound network.

---

## Architecture & file paths

- `crates/spacegraph-viewer/src/render/theme.rs` — new constants: aperture state
  tints (open/established/closing), barrier-ring colour, exposure tints
  (loopback/lan/public), gateway accent. No ad-hoc `Color::srgb` in render code.
- `crates/spacegraph-viewer/src/render/spatial.rs` — `aperture_style(state)`;
  render the Socket aperture per style (open torus / flow-beam / shuttered+barrier
  / dimmed), reusing the cached mesh/material handle pattern
  (`NodeRenderResources`) — no per-frame asset allocation; `exposure_bucket` +
  the socket shell-band target feeding `progressive_prepare`.
- `crates/spacegraph-viewer/src/render/postfx.rs` (+ `assets/shaders/cyberspace_post.wgsl`)
  — an "anomaly focus" uniform set (bounded alert screen-positions + intensity);
  the shader ramps a localized effect by proximity; forced off under Minimal
  (`postfx_active`), saved config untouched.
- `crates/spacegraph-agent/src/sources/net.rs` — parse `/proc/net/route`, extract
  the default route (destination `00000000`), emit the gateway IP as a
  `Node::RemoteHost` (existing kind) with a `connects_to`/derived linkage so it
  reads as the egress hub. Pure parse fn + committed fixture (mirror
  `parse_net_table` / the `/proc/net/{tcp,udp}` fixtures).
- `crates/spacegraph-viewer/src/util/config.rs` — `aperture_by_state` (default on),
  `exposure_depth` (default on), `anomaly_focus` + intensity (default on),
  persisted; follow the 4-way config discipline (struct+Default, serialize no
  `serde(skip)`, `apply_viewer_config` round-trip).
- `crates/spacegraph-viewer/src/ui/*` — surface the exposure bucket + socket state
  in the existing inspector tooltip (render only — do not fork the inspector).

**Boundaries (enforced):** `render/` reads the graph via the `GraphState` API,
never `net/`; the `net` source change is parse-only + emits an existing kind.

## Phases & gates (each: implement → test → `fmt`/`clippy`/`test` green → RUNLOG)

**P1 — Port-state-as-aperture.** `aperture_style(state)` + per-style Socket render
(cached handles).
*Gate:* `aperture_style` pure-fn test for LISTEN/ESTABLISHED/TIME_WAIT/CLOSE_WAIT/
the gated form; Minimal keeps the flat torus (assert via the theme-style picker).

**P2 — Exposure-as-depth.** `exposure_bucket(local_addr)` + shell-band target.
*Gate:* `exposure_bucket` table test (`127.0.0.1`→Loopback, `::1`→Loopback,
`10.x`/`192.168.x`/`172.16–31.x`/`169.254.x`/`fc00::`→Lan, public v4/v6→Public,
`0.0.0.0`/`::`→Public listener); shell-band mapping pure-fn test; applies in both
themes.

**P3 — Anomaly-as-scene-distortion.** Bounded alert selection + uniform feed +
shader ramp.
*Gate:* the alert-selection fn (top-N, screen-projected) is unit-tested
(count-bounded, picks by severity/recency); distortion off under Minimal; GPU look
documented in RUNLOG (not a CI stop).

**P4 — Gateway node.** `/proc/net/route` parse → default-route gateway →
`RemoteHost` emit.
*Gate:* committed `/proc/net/route` fixture → default route extracted, gateway
`RemoteHost` emitted; non-default routes ignored; **no wire change** (reuses
`RemoteHost`); diff-stable (a stable route table emits nothing after the first
upsert).

**P5 — Config + inspector + close-out.** Toggles + round-trip; exposure/state in
the tooltip. Update `docs/DESIGN_LANGUAGE.md` (aperture/exposure/anomaly + the new
constants), `docs/ACCEPTANCE.md` (D0 criteria), `docs/recon/CODE_INVENTORY.md`,
`docs/perf/RUNLOG.md`.
*Gate:* config round-trip test (mirror `viewer_config_roundtrip_save_load`);
Minimal-equivalence regression green; full `test --workspace` green; clean
`clippy`.

## Quality gates (every commit, non-negotiable)

- `cargo fmt --check` · `cargo clippy --workspace --all-targets -- -D warnings` ·
  `cargo test --workspace`.
- No `unwrap`/`expect` in render/IPC paths (return/skip on degenerate data).
- **Audited negatives:** no `child_process`/`std::process::Command`; no outbound
  network client; no `spacegraph-core` change / `PROTOCOL_VERSION` bump. Assert in
  close-out.
- Conventional commits, English, imperative. **No AI-authorship markers.** Naming
  hygiene: `aperture_style`, `exposure_bucket`, no `enhanced`/`v2`/`pro` suffixes.

## Test posture (headless)

Pure-function fixtures throughout (`aperture_style`, `exposure_bucket`, the route
parse, the alert-selection fn), mirroring `net.rs`'s `parse_net_table` +
`render::spatial::highlight_style` patterns. GPU/visual confirmation is documented
in RUNLOG, never a CI stop.

## Stop-and-Show

- Exposure-depth needing more than the existing `progressive_prepare` shell factor
  (a layout rewrite) → **stop**.
- Anomaly focus needing a new render pass rather than uniforms into the existing
  post pass → **stop**, surface the friction.
- The gateway emit appearing to need a wire change → **stop** (reuse `RemoteHost`
  is mandatory, O-8).
- Phase boundary **P4 → P5**: pause for a Sam look if the gateway linkage diverged
  from ADR-0012.

## BLOCKED discipline

If genuinely blocked, write `BLOCKED.md` at repo root: phase, exact blocker, the
ADR/ROADMAP clause in tension, 1–2 options + a recommendation. Never work around a
hard-stop by relaxing O-7/O-8.

## Done

- Phase 0 complete: v0.4 ROADMAP + ADRs + MPs filed under `docs/*`, v0.2 ROADMAP
  archived, tree clean.
- Aperture-by-state, exposure-as-depth, anomaly distortion, gateway node — all
  with pure-fn fixtures, config round-trip, Minimal-equivalence, no-exec/no-egress/
  no-wire audited.
- `DESIGN_LANGUAGE.md`/`ACCEPTANCE.md`/`CODE_INVENTORY.md`/`RUNLOG.md` updated.
- Branch `feat/perimeter-exposure-visual` ready for review (not merged — Track D
  lands on review).
