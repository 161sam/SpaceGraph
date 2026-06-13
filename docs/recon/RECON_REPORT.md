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
