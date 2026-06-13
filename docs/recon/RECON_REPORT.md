# SpaceGraph — Recon, Reconciliation & Roadmap-Readiness Report

Running record of the recon master-prompt. One section per phase. The terminal
deliverable is **Part A/B/C** (Phase 4); this run produces a trustworthy baseline
and a per-roadmap-phase readiness verdict, then **stops** — implementation is
routed by the operator via the spec loop.

---

## Phase 0 — Baseline

- **Session-start SHA:** `2a8aa41` (`v0.4.0` closeout merge).
- **Sync:** `origin/main` synced (0 ahead at start); tag `v0.4.0` present.
- **Roadmap:** `docs/ROADMAP.md` committed as `docs(roadmap): add SpaceGraph
  roadmap v0.2` (no content change; no broken internal cross-refs found — the
  `docs/adr/`, `CONSUMERS.md`, and external-ESN references are intentional
  forward references).
- **Baseline gates:** `cargo fmt --check` ✓, `cargo clippy --workspace
  --all-targets -D warnings` ✓, `cargo test --workspace` ✓ — **123 tests passed**.

Companion artifacts produced by this run: `docs/recon/CODE_INVENTORY.md`
(Phase 1), `docs/recon/DRIFT_MATRIX.md` (Phase 2).

---

## Phase 1 — Ground-truth code inventory

`docs/recon/CODE_INVENTORY.md` covers all seven categories, derived mechanically
from the tree (7-way parallel extraction + deterministic cross-verification of
the two critical categories). Headline flags:

- **Unregistered/dead systems (§2): FLAG LIST EMPTY.** All 38 system-shaped
  `pub fn`s are registered in a Bevy schedule or called by a registered system
  (`search_overlay`←`ui_panel`; `draw_spatial`/`draw_timeline`←`draw_scene`).
  **Anti-regression PASS:** `inspector_overlay` + `legend_overlay` are registered
  (`app/mod.rs:81-82`).
- **Config plumbing (§3): 4 panel-only gaps** — `max_visible_alerts`,
  `repulsion_radius`, `layout_budget_ms`, `visual_theme` are applied + serialized
  + toml-editable but have **no settings-panel control**. The first three are
  internal tuning (toml-only defensible); **`visual_theme` is user-facing** (the
  Standard/Minimal switch) → carried as a FINDING for Phase 2/3.
- **UI keybindings (§6):** all keybindings are documented in help; no orphaned
  overlay. Minor info gaps (context-menu actions, hover tooltips not enumerated).
- Core: `PROTOCOL_VERSION=3`, 6 `Node` / 7 `EdgeKind` variants. Agent: 4
  `EventSource`s (fs/proc/net/suricata_eve). Tests: **123** (core 2 / agent 26 /
  viewer 95).

---

## Phase 2 — Doc drift matrix

`docs/recon/DRIFT_MATRIX.md` cross-checks 9 doc targets against the inventory
(9-way parallel audit + deterministic re-verification of high-impact rows).
**107 rows: CONSISTENT 62 · STALE 23 · UNDOCUMENTED 13 · OVERCLAIM 7 · NAMING 2.**

- **Anti-regression (CONSISTENT):** every v0.4.0 deliverable verified present
  **and registered/reachable**; `inspector_overlay`/`legend_overlay` registered.
- **In-scope reconciliations (Phase 3):** README (Container→real kinds, `docs/`
  cross-ref paths, audio path, roadmap repoint), AGENTS.md (phase-order/role pins
  → roadmap pointer; module-boundary cells reworded to enforced reality),
  ARCH_VIEWER.md (complete module lists + Tree mode + baseline), ACCEPTANCE.md
  (bench numbers + Stand/Tag), DESIGN_LANGUAGE.md (mesh-edges rewrite + retitle +
  Socket/network-edge colour rows + theme-via-toml), RUNLOG (mesh-edges note),
  populate empty `ARCHITECTURE.md`/`GRAPH_SCHEMA.md`, archive superseded
  `ROADMAP_v0.2.0.md`.
- **Findings (NOT edited — carried to Part A/B):** F1 `visual_theme` has no in-app
  selector; F2 ROADMAP §1 stale post-v0.4.0 (roadmap edits out of scope); F3
  ACCEPTANCE lacks lane-timeline/Tree-view criteria; F4 Blueprint v0.2.0 plan
  shapes diverge (vision doc).
