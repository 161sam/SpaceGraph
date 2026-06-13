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

---

## Phase 3 — Reconciliation (docs↔code only)

Applied the in-scope reconciliations from the drift matrix (doc edits +
populate-empty + archive-superseded). **No code, no dependency, no external repo,
no behaviour change** — git diff is docs-only.

- **README:** "Container" → real node kinds; bare cross-refs → `docs/…`; audio
  path → `crates/spacegraph-viewer/assets/audio/`; roadmap section repointed to
  `docs/ROADMAP.md` (multi-node marked delivered).
- **AGENTS.md:** role version-pins dropped; frozen v0.1.8→v0.2.0 sequence → pointer
  to `docs/ROADMAP.md`; module-boundary cells reworded to the enforced reality
  (GraphModel-no-UI; graph may use bevy math/ECS types; ui via GraphState API);
  `ROADMAP_v0.2.0.md` ref → `docs/ROADMAP.md`.
- **ARCH_VIEWER.md:** completed render/ui/graph/util/net module lists; added the
  Tree view mode; baseline marker → v0.4.0.
- **ACCEPTANCE.md:** `force_step` numbers → 2.20/8.28 (v0.4.0); reconciliation
  Stand/Tag → 2026-06-13 / v0.4.0.
- **DESIGN_LANGUAGE.md:** rewrote the stale "edges are gizmos, mesh deferred"
  status (edges are a batched HDR mesh now) + retitled to v0.4.0; added Socket
  node colour + the network/alert edge-colour rows; theme "via viewer.toml".
- **RUNLOG.md:** appended a forward note that the deferred mesh-polyline edges
  shipped (historical Phase-5 entry left intact).
- **ARCHITECTURE.md / GRAPH_SCHEMA.md:** populated the two 1-byte placeholder docs
  from the inventory/core types.
- **Archived** superseded `docs/ROADMAP_v0.2.0.md` →
  `docs/archive/2026-06-13-superseded-by-roadmap/` (banner added; refs repointed).

**Re-verify:** all reconciled drift categories now show zero drift (grep sweep
clean). **Gate 3:** `fmt --check` ✓, `test --workspace` ✓ — **123 tests
(delta 0:** no dead system needed wiring — the only dead-code class,
inspector/legend, was already fixed in v0.4.0**)**. Scope guard: diff touches only
`*.md` (no `.rs`, no `Cargo.*`, no other repo).

**Not reconciled (carried as findings, by design):** ROADMAP §1 staleness
(roadmap edits out of scope), ACCEPTANCE lane-timeline/Tree-view criteria,
`visual_theme` in-app selector, Blueprint v0.2.0 plan-shape divergence.

---

# Roadmap-Readiness Report  *(terminal deliverable)*

This is the Phase-4 handoff. Each roadmap phase below carries a verdict; the operator routes the **specs to be written** (Part C) before any implementation MP runs. No roadmap feature is implemented by this recon run.

## Part A — Baseline summary

**Committed/created this run:** `docs/ROADMAP.md` (roadmap v0.2);
`docs/recon/{CODE_INVENTORY,DRIFT_MATRIX,RECON_REPORT}.md`; populated the two
1-byte placeholders `docs/ARCHITECTURE.md` + `docs/GRAPH_SCHEMA.md`; archived the
superseded `docs/ROADMAP_v0.2.0.md` → `docs/archive/`.

**Inventory headline (`CODE_INVENTORY.md`):** 3 crates (core / agent / viewer);
viewer ~53 modules. **Unregistered-systems flag EMPTY** — every system-shaped fn
is registered or called by a registered system; `inspector_overlay` +
`legend_overlay` registered (the prior-run dead-code bug stays fixed). Config: all
44 `ViewerConfig` fields applied + serialized, **4 panel-only gaps** (`visual_theme`
notable). 4 `EventSource`s; `PROTOCOL_VERSION=3`; **123 tests**.

**Drift (`DRIFT_MATRIX.md`):** 107 rows — CONSISTENT 62 · STALE 23 · UNDOCUMENTED
13 · OVERCLAIM 7 · NAMING 2. v0.4.0 deliverables all CONSISTENT (present +
registered/reachable).

**Reconciliations applied (Phase 3, docs-only, gates green, test delta 0):** README
+ AGENTS cross-refs/version-pins/boundary wording; ARCH_VIEWER module lists + Tree
mode + baseline; ACCEPTANCE bench numbers + Stand/Tag; DESIGN_LANGUAGE mesh-edges
rewrite + Socket/network edge colours + theme-via-toml; RUNLOG mesh-edges note;
populate ARCHITECTURE/GRAPH_SCHEMA; archive superseded roadmap.

**FINDINGS left unfixed (no code bug found — anti-regression clean):**
- **F1** `visual_theme` has no in-app selector (panel gap). *Rec:* add a theme
  selector in v0.5.0 (the UX-shell phase).
- **F2** ROADMAP §1 "stands today" is stale post-v0.4.0 (per-type geometry,
  in-world interaction, multi-node shipped) + `GLOW_LEVELS` attribution +
  reference to a `CC_MASTERPROMPT_…v0.4.0…md` not in the repo. *Rec:* operator
  refreshes ROADMAP §1 (roadmap content edits are out of scope for this run).
- **F3** ACCEPTANCE has no criterion for the lane-based timeline (PR #31) or the
  Tree view (PRs #29/#30). *Rec:* add or explicitly scope-out criteria.
- **F4** Implementation-Blueprint v0.2.0 plan shapes (NetManager/`ui/connections.rs`/
  per-node graphs/`TimelineEvt` node_key) diverge from the shipped string-prefix +
  `settings_agents` design. *Rec:* vision doc — revisit only if per-stream
  solo/telemetry UX is wanted.

## Part B — Per-phase readiness

### v0.5.0 — UX/UI shell + ESN house-standard alignment

- **Builds on (code that exists today):**
    - ui/panel.rs — single egui::SidePanel::left("panel") + ad-hoc egui::Window overlays; the only existing layout anchor to evolve into a dockable shell
    - ui/layout.rs — UiLayout rects helper (355B); current rectangle/region bookkeeping the dockable layout would replace/extend
    - ui/search.rs (search_overlay, Ctrl+P) + graph/state.rs recompute_search_hits(limit) (state.rs:1456-1502, substring/lowercase match over UiState.search_query → search_hits) — the substring filter the command palette + query-DSL replace
    - graph/state.rs UiState (state.rs:241-281: search_open/inspector_open/legend_open/view_mode + search_query/search_hits) — open-state flags the shell + palette extend
    - graph/state.rs alert surface — alert_order: VecDeque<NodeId> (state.rs:549), note_alert (1098), alert_severity_counts (1114), CfgState.max_visible_alerts (478) — the retained-alert model the native alert inbox/triage builds on
    - util/config.rs ViewerConfig + load_or_default()/save() TOML round-trip (config.rs:113,336,350; 44 fields, no #[serde(skip)]) — the persistence path for shell layout + saved views + palette/theme choice
    - util/config.rs VisualTheme enum {Standard,Minimal} (config.rs:49-52) + ViewerConfig.visual_theme (144) — extend with colourblind-safe palette variant; currently no in-app selector (CODE_INVENTORY §3 gap, FINDING)
    - render/theme.rs — hardcoded Color::srgb palette constants (PROCESS/FILE/USER/SOCKET/HOST/ALERT… + edge/timeline colours) as the single colour source of truth to re-express as ESN design tokens
    - ui/shortcuts.rs (handle_shortcuts) — keybinding dispatch the command palette + new shell toggles hook into
    - ui/help.rs (help_overlay) — the existing first-run/help surface the first-run tour extends
    - ui/hud.rs (hud_overlay) — existing status overlay the status/health bar evolves from
    - app/mod.rs SpaceGraphViewerPlugin::build — egui overlay systems registered in Update group 1 (ui_panel/help/hud/inspector/legend/reticle/context_menu/minimap); the registration site a dockable shell reorganizes
- **Gaps (code missing for this phase):**
    - No docking framework: egui_dock is NOT a workspace dependency (checked crates/*/Cargo.toml); layout today is one SidePanel::left + ad-hoc egui::Window. Dockable IDE-shell (left rail + bottom timeline + right inspector; pin/tile) is greenfield and constrained by bevy_egui = 0.28 (viewer Cargo.toml:35), which pins a compatible egui and therefore a compatible egui_dock version (or a hand-rolled dock).
    - No command-palette infrastructure: Ctrl+P (ui/search.rs) is node-search only; there is no action/command registry, no navigation/agent/settings command vocabulary, and no fuzzy matcher (recompute_search_hits is substring/lowercase). Mirroring Cockpit's Cmd+K is named but the command set + fuzzy ranking are unspecified.
    - No query-DSL: the substring filter (state.rs:1456) has no grammar, no tokenizer, no chip model. The DSL surface (type:/host:/sev:/recent:) → predicate over GraphModel is greenfield; the exact key set, value domains, and chip UX are unanchored.
    - Alert inbox triage model missing: state.rs has alert_order/note_alert/alert_severity_counts but NO ack/dismiss/mute/triage-state per alert. The referenced source `to_integrate/notification_system` does NOT exist in the tree (confirmed: no to_integrate dir, no *notif* source) — the roadmap calls it an 'idea' to port natively, so there is no code to reuse, only a concept.
    - No saved-views / bookmarks model: ViewerConfig persists single-valued settings but has no named-view collection (camera pose + filter + view_mode + selection bundle) schema.
    - No design-token / typography infrastructure: render/theme.rs is hardcoded Color::srgb constants only; no font assets and no egui font registration in Cargo.toml (only bevy_egui 0.28; no egui_extras/font crate). Inter / Space Grotesk / JetBrains Mono adoption is greenfield, and egui's theming is limited vs. the React Smolitux token system (O-1).
    - No colourblind-safe palette: VisualTheme is binary {Standard,Minimal}; a third (or palette-axis) colourblind-safe option does not exist; theme.rs colours are not parameterized.
    - No in-app theme selector: visual_theme is persisted+applied but has no panel control (CODE_INVENTORY §3, carried FINDING) — v0.5.0 must add the selector the shell needs.
    - ESN house-standard sources NOT locally available: the Portfolio-MVP / Smolitux-UI token definitions are not in the SpaceGraph tree and the shared INTERFACE_INVENTORY.md is NOT-LOCALLY-AVAILABLE — token values/semantics must be pinned by the spec author (O-1 governs the depth).
- **SpaceGraph docs to touch:**
    - /home/dev/SpaceGraph/docs/ROADMAP.md (resolve O-1 in §6; update v0.5.0 §3 block status)
    - /home/dev/SpaceGraph/docs/adr/ (new ADR resolving O-1: token/typography parity vs deeper Smolitux alignment)
    - /home/dev/SpaceGraph/docs/ACCEPTANCE.md (add v0.5.0 gates: layout persistence round-trip, palette/DSL parse tests, Minimal-equivalence)
    - /home/dev/SpaceGraph/README.md (controls list: command palette, query-DSL chips, alert inbox, saved views, shell layout, theme/palette selector)
    - /home/dev/SpaceGraph/docs/perf/RUNLOG.md (v0.5.0 section; visual-capture / shell-layout local-capture steps)
    - /home/dev/SpaceGraph/AGENTS.md (if new UX conventions or token map need recording)
- **External ESN contracts to reality-check:**
    - `Smolitux-UI / Portfolio-MVP design tokens (typography Inter/Space Grotesk/JetBrains Mono + three-brand token semantics)` — **NOT_LOCALLY_AVAILABLE** — Not present in the SpaceGraph tree; not in the ESN local checkout map. v0.5.0 is token/typography/interaction-convention PARITY only (O-1 recommendation: parity, since Bevy/egui cannot consume React components). Spec author must pin exact token values/semantics or scope to a self-defined egui token subset. No code dependency — alignment is one-way adoption.
    - `Cockpit Cmd+K command palette pattern (interaction-convention reference)` — **NOT_LOCALLY_AVAILABLE** — Cockpit repo present at EcoSphereNetwork but the Cmd+K palette pattern is referenced only as a convention to mirror; no contract is consumed. Pure UX parity, no wire/data dependency. Spec author may pin the command-vocabulary convention but it is not binding.
    - `INTERFACE_INVENTORY.md (ESN component / Hexa-Repo map)` — **NOT_LOCALLY_AVAILABLE** — Shared inventory not found by name in the local checkout. Not required for v0.5.0 implementation (no fabric membership at this phase — that is v0.6.0); listed only because the 'ESN family' framing references it. No blocking dependency for Track A.
- **Verdict:** SPEC-REQUIRED
- **If SPEC-REQUIRED, the spec must specify:**
    * design questions to resolve:
        - O-1 resolution: token/typography parity only, or deeper Smolitux alignment? (roadmap recommends parity; ADR required before build).
        - Docking approach: adopt egui_dock (which version is compatible with bevy_egui 0.28 / its pinned egui?) or hand-roll a dock from SidePanel/TopBottomPanel/Window? This gates the whole shell.
        - Command-palette scope: which command categories (actions, navigation, agents, settings) and which concrete commands ship in v0.5.0? Does it subsume Ctrl+P node-search or sit alongside it?
        - Fuzzy-matching algorithm/library for palette + search (e.g. subsequence/fzf-style vs the current substring) and its ranking.
        - Query-DSL grammar: exact key set (type/host/sev/recent + others?), value domains (which severities, which time units), boolean/combination semantics, and error/partial-input behaviour.
        - Alert-inbox triage model: which per-alert states (new/ack/dismissed/muted), do they persist, and how do they interact with the existing max_visible_alerts eviction (alert_order VecDeque)?
        - Saved-views semantics: what does a 'view' capture (camera pose + active DSL filter + view_mode + pinned/selected set + theme?), naming/overwrite/delete UX, and storage location (in viewer.toml vs a sibling file).
        - Colourblind-safe palette design: a third VisualTheme variant, or a palette axis orthogonal to Standard/Minimal? Which deficiency types (deuteran/protan/tritan) are targeted and what are the exact colour values?
        - Shell layout persistence: serialize the full dock tree, or a fixed set of pane visibility/size fields? How does it interact with ViewerConfig (extend ViewerConfig vs a separate layout file)?
        - First-run tour mechanism: in-egui overlay steps vs help-overlay extension; trigger/dismiss/persistence of 'seen' state.
        - Status/health bar contents: what signals (agent connection state, FPS, node/edge counts, alert counts via alert_severity_counts) and placement within the new shell.
    * data shapes / wire formats / tool schemas:
        - Query-DSL token + AST shape (e.g. enum DslPredicate { Type(NodeKind), Host(String), Severity(Sev), RecentWithin(Duration), ... }) and the chip representation.
        - Command-palette command descriptor (id, label, category, keybind hint, action closure/enum) and the command registry shape.
        - SavedView struct (name + camera transform + serialized filter/DSL string + view_mode + selection/pin set) and its serde representation for TOML/JSON persistence.
        - Alert-inbox entry shape: per-alert triage state attached to existing Alert nodes / alert_order (new map NodeId → TriageState) and its serde shape if persisted.
        - Shell layout serialization shape (dock tree or pane-config struct) and how it extends or sits beside ViewerConfig (config.rs ViewerConfig, no #[serde(skip)] convention).
        - Design-token table: named tokens → egui values (Color32, font family/size, spacing) mapping the three-brand semantics into egui's theming surface.
        - Colourblind-safe palette: the concrete per-NodeKind / per-EdgeKind Color set parallel to render/theme.rs constants.
    * decision points (with a recommendation):
        - egui_dock dependency add vs hand-rolled dock (bevy_egui 0.28 compatibility is the hard constraint to verify before committing).
        - Font loading mechanism: egui_extras / direct font bytes registration into the egui FontDefinitions; license/redistribution of Inter / Space Grotesk / JetBrains Mono and where the .ttf assets live (assets/fonts/).
        - Whether saved views + shell layout extend ViewerConfig (single viewer.toml) or live in a separate persisted file (avoid bloating the tuning config).
        - Whether the command palette replaces or augments the existing Ctrl+P search overlay (and the keybind: keep Ctrl+P vs add Cmd/Ctrl+K to mirror Cockpit).
        - Whether colourblind-safe is a new VisualTheme variant (touches every theme-gated system: geometry/reticle/rings/post-FX/audio) or a palette-only overlay that leaves geometry behaviour unchanged.
        - Scope cut for v0.5.0: all six deliverables in one tag vs phased sub-versions (large UX surface — sequencing should be set in the MP).
    * test & gate strategy (incl. headless):
        - Shell layout persists round-trip: serialize → deserialize → identical layout (unit test, mirrors existing ViewerConfig round-trip test in util/config.rs tests).
        - Command palette: parse input → resolved command/action (unit tests over the command registry + fuzzy matcher, table-driven).
        - Query-DSL: parse query string → predicate, then predicate → expected node/edge set over a fixture GraphModel (parse-then-apply unit tests; cover malformed/partial input).
        - Saved views: save → load → applied state matches (round-trip + apply unit test).
        - Alert inbox triage: ack/dismiss/mute transitions + interaction with max_visible_alerts eviction (unit test over alert_order/note_alert).
        - Minimal-equivalence preserved: assert Minimal theme behaviour unchanged after token/typography work (regression test, extends render/theme.rs theme tests).
        - Colourblind palette: assert palette completeness (every NodeKind/EdgeKind has a colour) and Standard-geometry-behaviour unchanged if palette-only.
        - Headless build gates: fmt --check, clippy --workspace --all-targets -D warnings, test --workspace all green; GPU/visual capture documented in RUNLOG (never a CI stop).
    * security / threat-model needs:
        - Saved views / shell layout / triage state persisted to local config files only (loopback/local, no network) — no new transport, no secrets; honour the existing no-network Track-A posture.
        - No mutating actions and no command execution: the command palette is navigation/view/settings only at v0.5.0 — system-action commands belong to v0.7.0 (AdminBot, NOT auto-mode). The spec must forbid any action command that touches the host here.
        - Font assets: verify redistribution licenses (Inter/Space Grotesk/JetBrains Mono are OFL but confirm) before vendoring into the repo.
- **Auto-mode suitability:** AUTO — Track A, no ESN dependency at implementation time, fully viewer-side and read-only: dockable shell, command palette (navigation/view/settings only — NO host actions), query-DSL over the in-memory graph, native alert inbox/triage, saved views, design tokens/typography. Nothing mutates the host or touches a security boundary; all persistence is local config files. No command-execution or action surface lands here (those are v0.7.0 / NOT_AUTO). Gates are unit-testable headless (round-trip, parse→action, query→predicate) with GPU/visual capture documented, not a stop. SPEC_REQUIRED only because the UX surface is large with O-1 open and many unanchored shapes — but the work itself is auto-safe.
- **Estimated size:** L

### v0.6.0 — SpaceGraph MCP server (read-only) + formal ESN admission

- **Builds on (code that exists today):**
    - graph/state.rs:539 — GraphState (#[derive(Resource)]): the canonical in-process projection the viewer renders; read accessors already present: alert_severity_counts (:1114), alerts_newest_first (:1129), explain_path_cached (:1517), reveal/visibility helpers, alert_order VecDeque. This is the projection v0.6.0 tools must read.
    - graph/model.rs:67 — GraphModel with the exact query precursors the read-only tools need: neighbors (:183) → spacegraph.neighbors; degree (:179) + agg_edges (:198) + agg_edge_count (:202) → spacegraph.topology_summary hubs/counts; edges_for_node (:167) → get_node detail; node map → query_graph.
    - graph/explain.rs:14 — shortest_path(model, a, b, max_depth, allowed) BFS already implements the why-connected path → spacegraph.explain_path is a thin wrapper (also cached via GraphState::explain_path_cached state.rs:1517).
    - spacegraph-core/src/lib.rs — shared wire types Node{6}/Edge/EdgeKind{7}/FileKind/NodeId + id constructors + serde derives; the only crate already shared across the workspace and the natural home for any MCP-facing typed result schema.
    - net/protocol.rs Incoming/IncomingKind + net/uds.rs spawn_reader (framed, version-checked, PROTOCOL_VERSION=3) — existing ingest path that fills GraphState via GraphState::apply (state.rs:947); relevant if the MCP server is fed the same delta stream rather than sharing ECS memory.
    - graph/interner.rs (NodeId→dense NodeIndex) + graph/state.rs index_of/is_visible_rendered — id resolution + visibility predicates a typed-filter query_graph would reuse.
    - Tests: 123 inline #[cfg(test)] unit tests incl. explain 2, model 2, state 15 (no tests/ dirs) — the existing test idiom; contract tests (fixture graph → typed result) are a new harness, not present.
- **Gaps (code missing for this phase):**
    - No MCP runtime anywhere: zero rmcp / json-rpc / tools/list / tools/call / stdio-server code in the tree (grep clean across all crates); crate crates/spacegraph-mcp does not exist; workspace Cargo.toml has only the 3 existing members.
    - ARCHITECTURE BLOCKER — canonical-state access: GraphState is #[derive(Resource)] (state.rs:538), an in-process Bevy ECS resource, and GraphModel lives ONLY in spacegraph-viewer (model.rs:67), NOT in spacegraph-core. A stdio MCP server is a separate process; 'reads the same GraphState projection' (roadmap §3) has no mechanism in-tree. Requires a decision: (a) extract a shared graph/state crate consumable by both viewer and mcp, (b) run the MCP server in-process inside the viewer over a shared snapshot/Arc, or (c) feed the MCP process the same delta stream and rebuild a parallel projection (the roadmap explicitly forbids 'parallel logic'). This is the single largest unspecified design point.
    - No auth plumbing: no jsonwebtoken/JWT/bearer dependency or code anywhere; the roadmap mandates 'auth token path tested' and L1 posture (bearer, omit aud/iss) but the token verification surface, secret source, and where it sits relative to stdio transport are entirely unspecified in-tree.
    - No typed result schemas for the 6 tools: filter grammar for query_graph (typed filter → node/edge set), the shape of get_node / neighbors / explain_path / list_alerts / topology_summary JSON payloads, pagination/result caps, and id encoding (NodeId is a String wrapper) are undefined. core lib.rs serializes wire Node/Edge but no MCP-facing tool-result types exist.
    - No docs/adr/ directory and no CONSUMERS.md exist — ADR-0001 (MCP surface as canonical external read API) and the CONSUMERS.md orchestrator-hub provider entry are net-new files.
    - No contract-test harness: tests are inline #[cfg(test)] only, no tests/ dirs; the gate ('each tool has a contract test: fixture graph → expected typed result') needs a new fixture+golden pattern not present.
    - Hub registration shape unknown locally: no orchestrator-registration code in-tree and the registration descriptor/manifest the orchestrator mcp_proxy expects (how a stdio server is added so it proxies as mcp__spacegraph__*) is not pinned — Appendix A.3 flags this as to-verify.
    - O-5 formal-admission deliverable is cross-repo and unactionable from this tree: the shared INTERFACE_INVENTORY.md (where SpaceGraph's row + Hepta map + Tier 3 go) is NOT in the local checkout (see external_contracts).
- **SpaceGraph docs to touch:**
    - /home/dev/SpaceGraph/docs/adr/0001-mcp-surface-canonical-read-api.md (new — ADR-0001, the canonical-state access mechanism decision must be recorded here)
    - /home/dev/SpaceGraph/CONSUMERS.md (new — §3 orchestrator-hub provider entry)
    - /home/dev/SpaceGraph/docs/ROADMAP.md (mark v0.6.0 status; reconcile §3 'reads the same GraphState' claim with the chosen access mechanism)
    - /home/dev/SpaceGraph/docs/ACCEPTANCE.md (add v0.6.0 gates: tools/list+tools/call smoke, per-tool contract test, auth token path, live-hub smoke)
    - /home/dev/SpaceGraph/docs/perf/RUNLOG.md (record the Reality-Check-Gate reads + live-smoke against the orchestrator hub per §5)
    - /home/dev/SpaceGraph/Cargo.toml (add crates/spacegraph-mcp workspace member)
    - cross-repo (NOT local): shared INTERFACE_INVENTORY.md — SpaceGraph row + Hexa→Hepta extension + Tier 3 (O-5); handled by a cross-repo MP, not in this tree
- **External ESN contracts to reality-check:**
    - `orchestrator MCP-hub proxy (mcp_proxy/ + ADR-0001-mcp-hub-proxy.md, mcp__<server>__<tool> naming)` — **VERIFIED** — Present at /home/dev/ESN/EcoSphereNetwork/esn-orchestrator/{src/esn_orchestrator/mcp_proxy, docs/adr/0001-mcp-hub-proxy.md}. Spec author must read it to pin the stdio-server REGISTRATION shape (Appendix A.3) — not derivable from SpaceGraph's tree. A live mcp__esn-orchestrator__* channel is also connected for verification.
    - `ABrain MCP_V2_INTERFACE.md (capability-first tool shape — the design template)` — **VERIFIED** — Present at /home/dev/ESN/Modularium/ABrain/docs/architecture/MCP_V2_INTERFACE.md (3291 bytes). Read as the binding template for capability-first tool naming and the 'thin interface over canonical state, no parallel logic' principle.
    - `Auth posture (auth.md / bearer / omit aud,iss — L1)` — **VERIFIED** — LabOS auth.md at /home/dev/LabOS/docs/modules/auth.md + ESN auth-posture closeout at esn-orchestrator/ops/closeouts/mp-auth-posture-closeout-2026-05-31.md present. Pins L1 token rules. Note: Cockpit ADR-068 auth-posture itself is ON-BRANCH only (see below); use the closeout + LabOS auth.md as the verified surface.
    - `shared INTERFACE_INVENTORY.md (ESN component / Hexa→Hepta repo map — O-5 admission target)` — **NOT_LOCALLY_AVAILABLE** — Not found by name in the local ESN checkout. The O-5 deliverable (add SpaceGraph row, extend to 7th member, assign Tier 3) cannot be authored from this tree; spec must defer it to a cross-repo MP and pin the inventory's row schema once located.
    - `Cockpit MP70.5 'Phase-3-Bündelung' (mcp__<server>__<tool> Sam-decision naming source)` — **NOT_LOCALLY_AVAILABLE** — The Cockpit ROADMAP section naming the decision is not in the local checkout; however the naming convention itself is corroborated by the VERIFIED orchestrator ADR-0001 + live mcp__esn-orchestrator__* tool names, which is sufficient to pin tool naming.
    - `Cockpit ADR-066 orchestrator-hub-integration / ADR-068 auth-posture` — **ON_BRANCH** — Present only on branches feat/adr-068-amend-l1-attach and feat/auth-posture-adr — NOT in default checkout. Not required for v0.6.0 (ADR-066 belongs to v0.6.x Cockpit-tab); treat as not-locally-available until merged.
- **Verdict:** SPEC-REQUIRED
- **If SPEC-REQUIRED, the spec must specify:**
    * design questions to resolve:
        - CANONICAL-STATE ACCESS (the crux): how does an out-of-process stdio MCP server read the in-process Bevy Resource GraphState (state.rs:538) without parallel logic? Choose: (a) extract a shared graph crate (GraphModel/GraphState read-side) out of spacegraph-viewer into a new lib consumable by spacegraph-mcp; (b) host the MCP server in-process inside the viewer over a shared snapshot/Arc<RwLock>; (c) feed the MCP process the same delta stream and rebuild a read-only projection. The roadmap forbids (c)'s 'parallel logic' — resolve explicitly.
        - Process topology: is spacegraph-mcp a standalone binary launched independently, or spawned by / embedded in the viewer? Determines memory sharing with GraphState vs independent ingest, and what the orchestrator registers.
        - rmcp server-side version pin + stdio transport setup, and whether it mirrors the tool-daemon's client-side rmcp usage (roadmap cites 'ecosystem consistency' but the tool-daemon code is not in this tree to copy).
        - Hub registration descriptor: the exact manifest/registration shape the orchestrator mcp_proxy expects for a stdio server to be proxied as mcp__spacegraph__* (read from esn-orchestrator/mcp_proxy + ADR-0001).
        - Read-consistency under per-frame mutation: define point-in-time-snapshot vs live read for a tool call and how needs_redraw AtomicBool / cross-thread reads interact.
        - O-5 admission split: confirm v0.6.0 ships SpaceGraph-side code only and defers the cross-repo INTERFACE_INVENTORY.md / Hepta / Tier-3 edits to a separate cross-repo MP (that file is not locally available).
    * data shapes / wire formats / tool schemas:
        - query_graph typed-filter grammar (type/host/severity/recency, mirroring the v0.5.0 query-DSL) + returned node/edge-set JSON shape + result caps/pagination.
        - get_node result: per-type detail payload for all 6 Node variants (Process/File/User/Socket/RemoteHost/Alert), with redaction policy for sensitive fields (cmdline/uid/path/addr/rdns).
        - neighbors result: node list + edge list with EdgeKind + direction; depth/limit params.
        - explain_path result: PathStep serialization from graph/explain.rs, max_depth default, allowed-set semantics.
        - list_alerts result: severity filter + ordering (reuse alerts_newest_first :1129 / alert_severity_counts :1114) + Alert payload (source/signature/severity/ts).
        - topology_summary result: counts-by-type, hub selection criterion (model.degree :179 / agg_edges :198), outer-ring / edge-class breakdown.
        - NodeId encoding across the MCP boundary (String newtype) — external id format; interner NodeIndex must never be exposed (process-local).
        - Home for MCP-facing result types: new serde types in spacegraph-core or the mcp crate, distinct from wire Node/Edge, vs documented reuse of core types.
    * decision points (with a recommendation):
        - Where shared read-side graph types live if extraction is chosen (new crate name vs widening spacegraph-core) — honour naming hygiene (no v2/enhanced suffixes per AGENTS.md).
        - rmcp exact version + workspace Cargo.toml dependency pin.
        - Auth enforcement point for a stdio transport that has no HTTP headers: validate bearer at SpaceGraph, or trust the hub as auth authority — resolve against auth.md + the orchestrator auth-posture closeout.
        - Tool naming confirmed against ABrain MCP_V2 capability-first convention and the mcp__<server>__<tool> proxy form.
        - Whether v0.6.0 scope includes the in-process-vs-standalone viewer refactor or treats the MCP server as a read-only consumer of an extracted snapshot.
    * test & gate strategy (incl. headless):
        - New contract-test harness: fixture GraphState/GraphModel → tool call → golden typed result, one per tool (6 min); decide inline #[cfg(test)] vs a new tests/ integration dir (none exist today).
        - tools/list + tools/call smoke test (rmcp server lists the 6 tools and returns a successful call).
        - Auth token path test only if auth applies at the SpaceGraph layer (gated on the auth decision); else document hub-owned auth.
        - Live-smoke (documented local capture, not a CI stop): register against the running orchestrator hub, confirm proxied as mcp__spacegraph__*, call one tool end-to-end.
        - Workspace gates per §5: fmt --check, clippy --workspace --all-targets -D warnings, test --workspace; no unwrap/expect in the new IPC crate.
        - Anti-regression: extracting graph types must not break the 95 viewer tests (state 15, model 2, explain 2, spatial 15).
    * security / threat-model needs:
        - Machine-checkable read-only invariant: the MCP crate exposes no mutating path into GraphState and no action tools (deferred to v0.7.0).
        - Auth posture L1: bearer-where-used, minters omit aud/iss, loopback/UDS only; resolve whether stdio-proxied calls carry a token and pin the secret source (one secret per node).
        - Field-level exposure review: get_node / query_graph surface cmdline, uid, file paths, socket addrs, rdns — set redaction defaults for an externally-consumable surface.
        - Result-size / DoS bounds: caps on query_graph result sets, explain_path max_depth, neighbors depth — protect the viewer frame loop if the server shares the ECS thread.
        - Trust boundary: document SpaceGraph trusts the orchestrator hub as auth/registration authority and exposes nothing needing shared code or foreign subprocess spawn (Appendix A.4).
- **Auto-mode suitability:** AUTO — v0.6.0 is read-only by design (no mutating actions, no command execution; action tools are deferred to v0.7.0). Roadmap §4 explicitly classifies v0.6.0 as 'read-only/low-risk and auto-capable' and v0.7.0 as the one non-auto phase. Provider-side surface over already-read-only GraphState. AUTO applies, but the auth/redaction surface and the cross-process state-access refactor warrant Stop-and-Show at the design boundary, and auto-mode does not waive the SPEC_REQUIRED verdict — the spec lands first.
- **Estimated size:** L

### v0.6.x — Cockpit tab (embed)

- **Builds on (code that exists today):**
    - v0.6.0 SpaceGraph MCP surface (crates/spacegraph-mcp) — NOT YET BUILT: crates/ holds only spacegraph-core, spacegraph-agent, spacegraph-viewer (CODE_INVENTORY §1). v0.6.x consumes the read-only tools v0.6.0 produces (spacegraph.query_graph/get_node/neighbors/explain_path/list_alerts/topology_summary, ROADMAP §3 v0.6.0). This phase cannot start until that surface exists.
    - graph/explain.rs `shortest_path` why-connected BFS — the canonical source behind spacegraph.explain_path, surfaced as jump-to-node deep links (CODE_INVENTORY §1 graph/).
    - graph/state.rs GraphState (+ SpatialState/TimelineState/UiState/ViewMode) — the canonical projection the MCP surface and any REST summary must read; topology stats come from graph/model.rs (degree/agg_edge) and metrics.rs (CODE_INVENTORY §1 graph/).
    - spacegraph-core/src/lib.rs NodeId + id_* constructors — the id vocabulary a deep-link must carry to focus the native viewer (CODE_INVENTORY §4).
    - render/camera.rs apply_jump_to — the existing focus-on-node mechanism a deep-link must invoke when the native window launches (CODE_INVENTORY §1 render/).
    - util/agent_command.rs build_agent_command + ui/settings_agents.rs 'Command…' launch-string helper — existing precedent for constructing a viewer launch invocation, relevant to 'launch native window' (CODE_INVENTORY §1 util/, §1 ui/).
- **Gaps (code missing for this phase):**
    - No MCP/REST surface in the SpaceGraph tree to consume: crates/spacegraph-mcp does not exist (CODE_INVENTORY §1 lists only core/agent/viewer). The phase's deliverable ('a tabs/spacegraph/ consuming SpaceGraph's MCP/REST') has no upstream to consume yet.
    - No REST surface anywhere in SpaceGraph — viewer net/ is a UDS client (net/uds.rs spawn_reader, net/protocol.rs Incoming), agent server.rs is a UDS server (LengthDelimitedCodec). 'MCP/REST' in the deliverable is undefined: ROADMAP v0.6.0 specifies MCP only, no REST endpoint is planned in any inventory module (CODE_INVENTORY §1 net/, §1 spacegraph-agent).
    - No SpaceGraph CONSUMERS.md and no SpaceGraph docs/adr/ directory exist (neither found in the tree). The gate's 'Cockpit CONSUMERS.md §3 gains a SpaceGraph entry' is cross-repo (Cockpit side), and there is no SpaceGraph-side provider doc to point back to.
    - No deep-link / URL-scheme handling anywhere in the viewer: main.rs boot accepts only --demo-load; the only focus path is render/camera.rs apply_jump_to driven internally. There is no inbound 'focus node X' entrypoint for an external launcher to invoke (CODE_INVENTORY §1 roots, render/).
    - Cockpit-side work lives in the ESN-Cockpit repo, not SpaceGraph — the tabs/spacegraph/ deliverable (React panel + manifest.toml) is authored in a foreign repo; nothing in the SpaceGraph inventory covers it. Cross-repo MP needed (ROADMAP §5 hard-pin discipline).
- **SpaceGraph docs to touch:**
    - /home/dev/SpaceGraph/CONSUMERS.md (new — provider entry pointing at the MCP/REST surface Cockpit consumes; §3 relationship per ROADMAP §5)
    - /home/dev/SpaceGraph/docs/adr/ (new ADR if the embed mechanism / deep-link scheme is a SpaceGraph-local decision; ROADMAP §5 'ADR per decision')
    - /home/dev/SpaceGraph/docs/ROADMAP.md (resolve O-2 once the embed mechanism is locked; §6 decisions table)
    - /home/dev/SpaceGraph/docs/perf/RUNLOG.md (Reality-Check entry recording the ADR-066 read + Cockpit tab-manifest verification, per ROADMAP §5)
    - Cockpit-side (foreign repo, ESN-Cockpit): tabs/spacegraph/manifest.toml + React panel, and ESN-Cockpit/CONSUMERS.md §3 — out of the SpaceGraph tree, tracked via a cross-repo MP
- **External ESN contracts to reality-check:**
    - `Cockpit ADR-066 orchestrator-hub-integration (the named embed/tab template)` — **NOT_LOCALLY_AVAILABLE** — ON-BRANCH only. The file docs/adr/ADR-066-orchestrator-hub-integration.md is NOT in the ESN-Cockpit default checkout (which has only ADR-002-plugin-tab-system.md and ADR-059_thru_065_RESERVED.md). It exists in git history (commits dd364a0, 99ab442 'mark ADR-066 Accepted') and on branches feat/adr-068-amend-l1-attach / feat/auth-posture-adr. Per the recon rule, treat as not available until checked out/merged. Spec author must pin the orchestrator-tab pattern from it.
    - `Cockpit tab-manifest / plugin-tab system (ADR-002)` — **VERIFIED** — In ESN-Cockpit default checkout: docs/adr/ADR-002-plugin-tab-system.md + tabs/<name>/manifest.toml schema parsed by crates/plugin-loader/src/manifest.rs into TabManifest (fields: name, display_name, icon?, description?, actions[] allowlist, backend_module?, [capabilities] requires_desktop/requires_container_stack/requires_auth). Living templates: tabs/agent-actions, tabs/system-info, tabs/n8n, tabs/flowise. NOTE: the roadmap names 'tab-manifest/Phase-3-Bündelung MP70.5' which the recon marks NOT-LOCALLY-AVAILABLE — but the concrete manifest mechanism IS verified here via ADR-002.
    - `orchestrator MCP-hub proxy (mcp__<server>__<tool> proxying)` — **VERIFIED** — esn-orchestrator/src/esn_orchestrator/mcp_proxy/{tool_proxy.py,client_pool.py} + docs/adr/0001-mcp-hub-proxy.md present; live mcp__esn-orchestrator__* channel connected. This is how the tab would reach SpaceGraph's tools once SpaceGraph registers — but SpaceGraph's MCP server (v0.6.0) does not exist yet.
    - `Cockpit auth posture (ADR-068 / auth-posture-adr) for tab→MCP bearer path` — **ON_BRANCH** — ON-BRANCH only: branches feat/adr-068-amend-l1-attach, feat/auth-posture-adr. Pins the L1 bearer / token-attach posture the React panel uses to call the hub. Not in default checkout. ESN auth-posture closeout exists at esn-orchestrator/ops/closeouts/mp-auth-posture-closeout-2026-05-31.md as a secondary anchor.
    - `ABrain MCP_V2_INTERFACE.md (capability-first tool shape, design template for v0.6.0 tools the tab renders)` — **VERIFIED** — Modularium/ABrain/docs/architecture/MCP_V2_INTERFACE.md present — defines the tool/result shape the Cockpit panel will render. Relevant indirectly (it shapes v0.6.0, which this tab consumes).
    - `Shared INTERFACE_INVENTORY.md (Hexa/Hepta-Repo map for the Cockpit consumer relationship)` — **NOT_LOCALLY_AVAILABLE** — Not found by name in the ESN tree. O-5 formal admission (the SpaceGraph row) is a v0.6.0 deliverable; the Cockpit-consumer relationship recorded at v0.6.x assumes that row exists. Spec author must pin where/how the consumer edge is recorded.
- **Verdict:** SPEC-REQUIRED
- **If SPEC-REQUIRED, the spec must specify:**
    * design questions to resolve:
        - O-2 (open in ROADMAP §6): the exact embed mechanism for a native Bevy window. Roadmap recommends 'thin React panel over MCP + launch native window (no iframe)' — the spec must lock this and define the launch mechanism: does the React tab shell out to a spacegraph-viewer binary, send a request to a resident local launcher, or use a desktop OS handler? Cockpit is Tauri; pin who owns process spawn (ADR-066 §A.4 says no foreign-subprocess-spawn — so spawn must be host-side, not from Cockpit's process).
        - Hard dependency ordering: v0.6.x consumes the v0.6.0 MCP surface which is NOT BUILT (only core/agent/viewer crates exist). Spec must state v0.6.0 is a prerequisite and either gate v0.6.x behind it or scope a stub.
        - 'MCP/REST' ambiguity: ROADMAP v0.6.0 plans MCP only and the tree has no REST surface. Spec must decide whether the tab reads purely via the orchestrator hub (mcp__spacegraph__*) or whether a REST/HTTP read endpoint must additionally be specified (and if so, where it is built — that would be net v0.6.0 scope, not v0.6.x).
        - Deep-link contract: how 'jump-to-node' carries a NodeId (spacegraph-core id_* vocabulary) from the React panel into a launched/running viewer, and how the viewer focuses it (render/camera.rs apply_jump_to currently has no external entrypoint). Define URL scheme / arg / IPC and whether it focuses an existing window or always launches a new one.
        - Which Cockpit ADR-066 orchestrator-tab pattern details are binding — it is ON-BRANCH, so the spec author must check out the branch and transcribe the exact tab→hub call shape, action-bus usage, and ConfirmationLayer expectations rather than guessing.
    * data shapes / wire formats / tool schemas:
        - Tab manifest fields for tabs/spacegraph/manifest.toml against the verified ADR-002 TabManifest schema: name, display_name, icon, description, actions[] (read-only — no mutating actions at v0.6.x), backend_module hint, and [capabilities] (requires_desktop true since the 3D viewer is native; requires_auth per the ON-BRANCH auth posture).
        - The exact shape of the MCP results the panel renders: alert feed (from spacegraph.list_alerts — severity-filtered Alert nodes {source,signature,severity,ts}, per core lib.rs Node::Alert), and topology summary (from spacegraph.topology_summary — counts/hubs/outer-ring). These are v0.6.0 outputs the spec must reference once v0.6.0 pins them.
        - Deep-link payload schema: the NodeId form (spacegraph-core NodeId + which id_* constructor namespace) plus any host/stream qualifier (graph/namespace.rs multi-stream id prefixing) so a link is unambiguous across agents.
        - Launch-invocation shape for the native viewer: argument/flag added to viewer main.rs (currently only --demo-load) to accept an initial focus target.
    * decision points (with a recommendation):
        - Whether to build any SpaceGraph-side REST endpoint at all, or consume exclusively through the orchestrator MCP hub (affects whether net/ or a new crate is touched).
        - Whether the native window is spawned fresh each deep-link or a single resident viewer is focused (IPC channel choice — reuse agent-style UDS vs a new local control socket).
        - Whether v0.6.x ships against a real v0.6.0 MCP surface or is explicitly blocked until v0.6.0 tags (recommend block; the phase has nothing to consume otherwise).
        - Where the consumer relationship is recorded given INTERFACE_INVENTORY.md is not locally available: SpaceGraph CONSUMERS.md (new) + Cockpit CONSUMERS.md §3 are the concrete anchors.
    * test & gate strategy (incl. headless):
        - Cockpit-side (foreign repo): tab renders alert feed + topology summary from the MCP surface — fixture MCP responses → rendered panel assertion (mirrors ROADMAP v0.6.x gate).
        - Deep-link round-trip: a deep-link string → viewer launches/focuses the target node — assert apply_jump_to receives the decoded NodeId (SpaceGraph-side unit test on the new inbound focus entrypoint).
        - Manifest validity: tabs/spacegraph/manifest.toml parses under Cockpit's plugin-loader manifest.rs TabManifest (parse test in Cockpit's crate).
        - No mutating action in the manifest actions[] allowlist (read-only gate — assert vocabulary is the v0.6.0 read tools only).
        - CONSUMERS.md §3 entry present (Cockpit side) and a reciprocal SpaceGraph CONSUMERS.md provider entry — doc-presence check.
        - Reality-Check recorded in RUNLOG (ADR-066 + tab-manifest read), per ROADMAP §5.
    * security / threat-model needs:
        - Auth path for the React tab → orchestrator hub → mcp__spacegraph__*: L1 posture (loopback/UDS, JWT bearer, minters omit aud/iss per ROADMAP §2/§5) — pinned from the ON-BRANCH Cockpit ADR-068 auth-posture, which must be checked out to confirm the token-attach mechanism.
        - Read-only enforcement: v0.6.x exposes only read tools; no action vocabulary in the tab (mutating actions are v0.7.0 AdminBot, NOT auto-mode). The manifest actions[] allowlist must contain no mutating entries.
        - Process-boundary discipline (ROADMAP §A.4 / ADR-066): Cockpit must consume over a process/network boundary only — no shared code, no Cockpit-side subprocess spawn of the SpaceGraph viewer. The native-window launch must be host-side, with the spec pinning who owns the spawn and how the boundary is preserved.
        - Deep-link input validation: an external focus target (NodeId) is untrusted input into the viewer — must be validated against the interner/known nodes (graph/interner.rs) and not used to trigger any side effect beyond camera focus.
- **Auto-mode suitability:** AUTO — Read-only / viewer-side and provider-surface consumption: the tab renders an alert feed + topology summary from the MCP surface and adds a camera-focus deep-link (render/camera.rs apply_jump_to). No mutating actions, no system editing — those are explicitly deferred to v0.7.0 (AdminBot, NOT_AUTO). Per ROADMAP §4, v0.6.x sits in the read-only band before the NOT-auto AdminBot phase. AUTO is conditional on the spec keeping the action vocabulary read-only and the auth path L1-loopback. Note: actual implementation is cross-repo (the tab is authored in ESN-Cockpit), and it is hard-blocked on v0.6.0 (the MCP surface) which is not yet built — so 'auto-ready' applies to the work shape, not to the readiness to start.
- **Estimated size:** M

### v0.7.0 — AdminBot integration (security-critical)

- **Builds on (code that exists today):**
    - spacegraph-agent UDS server (crates/spacegraph-agent/src/server.rs `run`): tokio UnixListener + tokio_util LengthDelimitedCodec (u32-length-prefix framing) + serde_json — the framing shape is the closest existing analogue to AdminBot-wire's 'u32-prefix + JSON', and 0600 socket perms are already set. BUT it is listen-only and has no SO_PEERCRED, no 64 KiB cap.
    - spacegraph-viewer net client (crates/spacegraph-viewer/src/net/uds.rs `spawn_reader`/`run`): the existing connect-out UDS pattern (tokio UnixStream::connect, Framed+LengthDelimitedCodec, Hello/protocol-version handshake, watch-channel shutdown, crossbeam Sender bridge to the Bevy ECS) — the template a new outbound AdminBot-wire client would mirror. It currently speaks the SpaceGraph `Msg` protocol, not AdminBot-wire.
    - Viewer action surface (crates/spacegraph-viewer/src/ui/context_menu.rs): the `CtxAct` enum + `apply_context_action` (closed, deferred-action vocabulary, unit-tested as a pure mapping) — the existing in-world right-click action pattern that the Approval-object UI can extend. Today every CtxAct is a graph-state mutation only (Focus/Isolate/Trace/TogglePin/ToggleMark/Inspect); none touch the host.
    - Multi-select state (crates/spacegraph-viewer/src/graph/state.rs `UiState.multi_selected`, drag-select in render/spatial.rs) — the substrate for the 'multi-select N nodes → one approval → AdminBot dispatch' batch-action flow.
    - v0.4.0 reticle/readout (crates/spacegraph-viewer/src/ui/reticle.rs `reticle_overlay`, gated Spatial+Standard): the lock-on + micro-tag in-world readout the roadmap says to reuse for rendering the per-node audit trail in-scene.
    - Bevy plugin schedule wiring (crates/spacegraph-viewer/src/app/mod.rs `SpaceGraphViewerPlugin::build`) and net-command plumbing (app/resources.rs `NetRx`/`NetTx`, app/mod.rs `process_net_commands`/`pump_network`) — the integration points where an approval-state resource, an outbound-request channel, and audit-event ingestion systems would be registered.
    - spacegraph-core wire types (crates/spacegraph-core/src/lib.rs): Node::{Process,Socket,RemoteHost,...} and id constructors (id_process/id_socket/id_remote_host) — the node identities a closed AdminBot action vocabulary (kill/signal process, restart unit, drop socket/conntrack, block IP) must map targets from.
- **Gaps (code missing for this phase):**
    - No AdminBot-wire IPC client exists at all — grep across crates/ for adminbot/peercred/action_request/capability/whitelist returns zero hits. The proposed `crates/spacegraph-adminbot-client` crate is fully greenfield (not in the 3-crate workspace: spacegraph-core/-agent/-viewer).
    - No SO_PEERCRED peer-credential verification anywhere. The agent server (server.rs) accepts connections with `let (stream, _) = listener.accept()` — discarding the peer addr and never reading SO_PEERCRED. AdminBot-wire mandates SO_PEERCRED; this must be built.
    - No 64 KiB frame cap. LengthDelimitedCodec is used with `::new()` defaults (no `max_frame_length` set) in both server.rs and net/uds.rs — AdminBot-wire's 64 KiB cap is not enforced.
    - No outbound/client direction for the host-local agent. O-3 says the agent speaks AdminBot-wire directly, but spacegraph-agent (main.rs/server.rs) only LISTENS for viewers; it never connects out to a peer daemon. The whole agent→AdminBot client path is new.
    - No Approval object / Decision→Review→Approval→Execution→Audit state machine. No types, no `approver != requester` enforcement, no two-step gate. The only action concept (`CtxAct`) is a single-click graph mutation. Entirely new domain.
    - No closed AdminBot action vocabulary in SpaceGraph. `CtxAct` is viewer-local; there is no enum/registry of whitelisted AdminBot capabilities (process.snapshot, journal.query, kill/signal, restart unit, drop socket, block IP) nor their request/response shapes.
    - No audit trail / correlation_id threading. No audit store, no correlation_id field in core types, no in-scene audit rendering. The reticle exists but has no audit data to show.
    - No JWT/bearer auth or rmcp dependency in any Cargo.toml (grep of crates/*/Cargo.toml for jwt/jsonwebtoken/rmcp is empty). If AdminBot-wire requires a bearer token under the L1 posture, the token mint/attach path is unbuilt.
    - No CONSUMERS.md, no docs/adr/ADR-0002 (Actions via AdminBot). No `docs/adr/` directory entries for SpaceGraph-local ADRs yet; ADR-0001 (MCP, v0.6.0) and ADR-0002 are both unwritten.
    - No threat-model or per-action test scaffolding (tests are inline #[cfg(test)] only; no tests/ dirs, no #[tokio::test]); the 'approve-and-execute-is-two-steps' and per-action threat-model test sets are net-new.
    - The binding approval contract (OceanData OPERATOR_APPROVAL_ARCHITECTURE.md) is NOT in the local checkout — so the exact Approval object shape SpaceGraph must mirror is currently unknowable without checking out the OceanData branch.
- **SpaceGraph docs to touch:**
    - /home/dev/SpaceGraph/docs/adr/ADR-0002-actions-via-adminbot.md (new — 'Actions via AdminBot, not a native channel')
    - /home/dev/SpaceGraph/docs/adr/ (one new ADR per onboarded action, starting with the first read-class action)
    - /home/dev/SpaceGraph/docs/CONSUMERS.md (new or §3 update — AdminBot consumer entry)
    - /home/dev/SpaceGraph/docs/ROADMAP.md (mark v0.7.0 progress; record Reality-Check in RUNLOG)
    - /home/dev/SpaceGraph/docs/perf/RUNLOG.md (record the v0.7.0 Reality-Check-Gate + live-smoke against AdminBot)
    - /home/dev/SpaceGraph/docs/ACCEPTANCE.md (add approval-object / two-step / no-native-exec acceptance gates)
    - a per-action threat-model doc set under docs/ (one per onboarded action, OceanData PR13.x style)
- **External ESN contracts to reality-check:**
    - `Smolit-Assistant ADR-0005 adminbot-safety-boundary (capability-whitelist, approval-default, AdminBot-wire UDS u32-prefix+JSON, SO_PEERCRED, 64 KiB cap)` — **VERIFIED** — Present at /home/dev/ESN/Modularium/Smolit-Assistant/docs/adr/ADR-0005-adminbot-safety-boundary.md. Spec author must read it to pin the exact AdminBot-wire frame layout, the SO_PEERCRED check semantics, the capability-whitelist shape, and the approval-default rule. NOT deep-read here.
    - `OceanData OPERATOR_APPROVAL_ARCHITECTURE.md (binding Decision→Review→Approval→Execution→Audit discipline)` — **ON_BRANCH** — Not in OceanData default checkout (confirmed absent under /home/dev/ESN/Modularium/OceanData/docs); lives on branch docs/operator-approval-architecture. This is the binding approval-object contract SpaceGraph must mirror — must be checked out/merged before the Approval object can be designed to spec.
    - `OceanData PR13.x approval domain + context_query_contract.md (one-action-at-a-time onboarding discipline, approval domain types)` — **ON_BRANCH** — On branches feat/pr13-1-approval-domain and docs/fa1-context-query-wire-contract. Referenced as the per-action onboarding template; not locally available in default tree.
    - `Smolit-Assistant ADR-0008 outbound-tool-surface / ADR-0009 tool-daemon-integration (Appendix B AdminBot-pattern references)` — **NOT_LOCALLY_AVAILABLE** — Recon map lists ADR-0008/0009 as not found by name. ADR-0005/0006 are present but 0008/0009 (the tool-daemon outbound-surface pattern the roadmap cites as the action-universe precedent) are not. Spec author must source them cross-repo.
    - `AdminBot adminbot_status + adminbot_action_request axes (the live action surface + binding action vocabulary, Assumption A.2)` — **NOT_LOCALLY_AVAILABLE** — Smolit_AdminBot repo is present in the ESN tree but the wire-level adminbot_status/adminbot_action_request axis definitions and the concrete action vocabulary were not surfaced by the recon map as a named contract file. The exact request/response JSON for the first read-class action (process.snapshot / journal.query) must be pinned from the AdminBot repo at Reality-Check time.
    - `Auth posture (auth.md / ADR-068 smolit-stack-auth-posture, L1 loopback/UDS bearer, omit aud/iss)` — **ON_BRANCH** — ESN auth-posture closeout present at esn-orchestrator/ops/closeouts/mp-auth-posture-closeout-2026-05-31.md (VERIFIED) and a LabOS auth.md exists, but the authoritative ADR-068 smolit-stack-auth-posture is on branches feat/adr-068-amend-l1-attach / feat/auth-posture-adr. Whether AdminBot-wire requires a bearer token (and its mint/omit-aud/iss rules) must be pinned from the auth-posture ADR.
    - `esn-orchestrator MCP-hub proxy + live mcp__esn-orchestrator__* knowledge channel` — **VERIFIED** — Present (ADR 0001-mcp-hub-proxy.md + live MCP server). Not directly required for v0.7.0 (AdminBot is a direct IPC peer per O-3, not hub-proxied), but available as a knowledge channel to resolve the NOT_LOCALLY_AVAILABLE/ON_BRANCH contracts above without re-searching.
- **Verdict:** SPEC-REQUIRED
- **If SPEC-REQUIRED, the spec must specify:**
    * design questions to resolve:
        - Which is the FIRST onboarded action — process.snapshot or journal.query — and what is its exact AdminBot adminbot_action_request request/response JSON (read-class)?
        - Does SpaceGraph's host-local AGENT speak AdminBot-wire (per O-3 literal reading: agent is the direct IPC peer), or does the VIEWER hold the client? The agent is currently listen-only; this decides which crate gains the outbound client and where the Approval object/audit lives relative to the agent↔viewer split.
        - How is `approver != requester` actually enforced in a single-operator desktop tool — second OS identity, second peer over SO_PEERCRED, a separate approver role/credential, or a deliberate two-human gate? The roadmap mandates it but the mechanism is undefined.
        - Is the Approval object SpaceGraph-local, or must it be byte-for-byte the OceanData OPERATOR_APPROVAL_ARCHITECTURE object (which is ON_BRANCH and unread)? Reuse vs. mirror decision blocks the data shape.
        - Does AdminBot-wire require a JWT bearer token under L1, and if so what is the mint path (omit aud/iss, one secret per node) — or is SO_PEERCRED alone the auth?
        - How is the 'no native command execution anywhere in the tree' invariant audited/enforced (deny-list lint, grep gate in CI, architectural test)?
        - How is the audit trail persisted (in-memory only for v0.7.0, or already to OceanData — which is v0.9.0)? And how is correlation_id generated and threaded from Decision through Audit?
        - What is the batch-action semantics — does one approval cover N heterogeneous actions across N nodes, or N homogeneous actions; and is it one AdminBot dispatch or N?
    * data shapes / wire formats / tool schemas:
        - AdminBot-wire frame: confirm u32 length prefix endianness + 64 KiB max_frame_length + JSON body (LengthDelimitedCodec must be configured, not ::new() defaults).
        - adminbot_action_request payload schema for the first action (fields, capability id, target ref, args) and its response/status schema (incl. adminbot_status).
        - SO_PEERCRED expectation: which uid/gid/pid the AdminBot peer must present and how SpaceGraph validates it.
        - Approval object schema: Decision / Review / Approval / Execution / Audit record fields, state-transition rules, requester id, approver id, timestamps, correlation_id.
        - Closed AdminBot capability/action vocabulary enum (the whitelist) with the SpaceGraph Node→target mapping for each (process kill/signal, unit restart, socket/conntrack drop, IP block).
        - Audit-event record shape rendered in-scene via the reticle (what fields, severity, link to node).
        - Bearer-token claim set IF auth is required (sub, exp, omit aud/iss per L1).
    * decision points (with a recommendation):
        - First action selection (process.snapshot vs journal.query) and the second (first mutating) action.
        - Client placement: new crate `spacegraph-adminbot-client` consumed by agent vs by viewer.
        - Build the AdminBot-wire framing fresh in spacegraph-adminbot-client vs lift the net/uds.rs Framed pattern (explicitly NOT the esn-daemon-ipc shared crate — that is a documented future extraction, not built here).
        - Approval object: adopt OceanData's verbatim vs SpaceGraph-local mirror.
        - approver!=requester enforcement mechanism (per design question).
        - Whether v0.7.0 ships read-class only, or read-class + exactly one mutating action (roadmap implies onboard read first, then one mutating action per PR).
    * test & gate strategy (incl. headless):
        - Read-class action round-trips through the full Approval object (Decision→Review→Approval→Execution→Audit) with an audit record — contract test against a fixture/mock AdminBot peer.
        - 'Approve and execute is two steps' asserted in tests — a single call/click cannot reach Execution; approval is its own object transition.
        - approver != requester rejected when equal — negative test.
        - Per-action threat-model test set, one per onboarded action (OceanData PR13.x discipline).
        - AdminBot-wire framing tests: 64 KiB cap enforced (oversize frame rejected), SO_PEERCRED mismatch rejected.
        - 'No native command execution anywhere in the tree' audited — CI gate / architectural test that no std::process::Command / exec path exists in the action path.
        - Live-smoke documented (not a CI stop) against a real AdminBot peer; correlation_id threaded end-to-end.
        - Capability-whitelist: an action outside the closed vocabulary is rejected before dispatch — negative test.
    * security / threat-model needs:
        - SO_PEERCRED peer-credential verification on every AdminBot-wire connection (currently absent).
        - 64 KiB frame cap to bound memory (LengthDelimitedCodec max_frame_length, currently default/unbounded).
        - 0600 socket perms (already done for the agent listener; must carry to any new socket).
        - Capability-whitelist enforcement — closed action vocabulary, no generic command runner; reject-by-default.
        - Two-step Decision→Approval gate with approver!=requester; no single-click execute.
        - Mandatory correlation_id-threaded audit for every action, including denials.
        - No native command execution in the tree (audited) — SpaceGraph only emits adminbot_action_request, never runs a command.
        - L1 auth posture: loopback/UDS only, bearer where required, minters omit aud/iss, one secret per node; RS256/JWKS explicitly out of scope.
        - One action onboarded at a time, each with its own threat-model + PR + tests (no bulk action enablement).
- **Auto-mode suitability:** NOT-AUTO — Roadmap mandate plus security/mutation profile.
- **Estimated size:** L

### v0.8.0 — ABrain reasoning

- **Builds on (code that exists today):**
    - v0.7.0 AdminBot approval layer (Decision→Review→Approval→Execution→Audit) — ABrain action_intents are surfaced as PROPOSED AdminBot actions and run through this gate (ROADMAP §3 v0.8.0); NOT YET BUILT — no approval/intent/correlation_id scaffolding exists in the tree (grep over crates/ returns 0 hits)
    - crates/spacegraph-adminbot-client (v0.7.0) — the action sink that proposed intents target; NOT YET BUILT (crates/ holds only spacegraph-core, spacegraph-agent, spacegraph-viewer)
    - crates/spacegraph-mcp (v0.6.0) — fabric membership + the graph-read tool shape (query_graph/explain_path/topology_summary) that defines the 'graph slice' sent to ABrain as context; NOT YET BUILT
    - graph/explain.rs — shortest_path/PathStep (CODE_INVENTORY §1) is the existing why-connected BFS that produces an attack-path/graph-slice for the reasoning context
    - graph/state.rs GraphState + ViewMode (state.rs:233) and graph/model.rs GraphModel upsert/remove — the canonical projection a graph slice is extracted from and where new hypothesis/note nodes would be inserted
    - spacegraph-core/src/lib.rs Node enum (6 variants, lib.rs:16) + EdgeKind enum (7 variants, lib.rs:73) + Delta (lib.rs:92) + PROTOCOL_VERSION=3 (lib.rs:9) — must gain hypothesis/note annotation kinds, a protocol-version bump, and matching render/theme/legend coverage
    - render/theme.rs (colour source of truth) + render/node_mesh.rs (per-type geometry) + ui/legend.rs + ui/reticle.rs/ui/inspector.rs — where new annotation node/edge kinds are 'rendered distinctly' and reasoning output is surfaced
    - ABrain MCP_V2_INTERFACE.md (VERIFIED local: /home/dev/ESN/Modularium/ABrain/docs/architecture/MCP_V2_INTERFACE.md) — capability-first tool shape / run_plan+explain surface the abrain-client adapts to
- **Gaps (code missing for this phase):**
    - No crates/spacegraph-abrain-client exists — the entire reasoning adapter (MCP rmcp client OR HTTP POST /text/generate, version-tagged) is greenfield
    - Transport decision (MCP run_plan vs HTTP /text/generate on 127.0.0.1:8788/UDS) is unresolved locally: ABrain ADR-0003-native-api-for-text-generation is ON_BRANCH only (feat/adr-0003-appliance-posture), not in the default checkout — the deciding ADR cannot be read
    - The v0.7.0 approval layer that v0.8.0 depends on does not exist yet: zero matches for approval/Approval/action_intent/correlation_id/adminbot in crates/ (verified by grep). v0.8.0 cannot route 'proposed' intents through a gate that isn't built
    - No action_intent / Approval object data type exists anywhere — the shape an ABrain intent maps INTO (and how a mutation-implying intent is detected and paused) is undefined
    - Graph slice → reasoning request: there is no serializer turning GraphState/explain.rs output into an ABrain-consumable context payload; the slice shape, node/edge projection, size bounds and redaction posture are unspecified
    - No hypothesis/note node or edge kinds in spacegraph-core (Node has 6 variants, EdgeKind has 7); adding annotation kinds is a wire-protocol change (PROTOCOL_VERSION bump) touching core + agent compat + every match site (render/theme/node_mesh/legend, graph/model)
    - correlation_id is threaded into 'the audit trail' (ROADMAP §3 v0.8.0) but no audit trail and no correlation_id field exist in the tree (0 grep hits) — both are inherited-from-v0.7.0 unknowns
    - No docs/adr/ directory and no CONSUMERS.md exist yet — ADR (adapter/version-pin) and the ABrain consumer CONSUMERS.md §3 entry must be created from scratch
    - OceanData OPERATOR_APPROVAL_ARCHITECTURE.md — the binding approval discipline the intents flow through — is NOT in the default tree (ON_BRANCH docs/operator-approval-architecture); the gate semantics can't be hard-pinned locally
    - Auth posture for the ABrain call (L1 loopback/UDS bearer, minters omit aud/iss per auth.md) is unspecified for the abrain-client; auth.md is not in the SpaceGraph tree (LabOS auth.md + esn-orchestrator closeout exist out-of-repo)
- **SpaceGraph docs to touch:**
    - /home/dev/SpaceGraph/docs/ROADMAP.md (v0.8.0 block + §6 decisions if transport/annotation choices get pinned)
    - /home/dev/SpaceGraph/docs/adr/ (new dir) — ADR 'ABrain reasoning consumed via MCP/HTTP adapter; intents proposed, never executed' (SpaceGraph-local numbering follows ADR-0003=OceanData per §5)
    - /home/dev/SpaceGraph/CONSUMERS.md (does not exist) — §3 ABrain consumer entry, version-pin recorded
    - /home/dev/SpaceGraph/docs/ACCEPTANCE.md (gate: fixture alert cluster → ABrain call → rendered reasoning + proposed-not-executed actions; mutation intents pause at approval)
    - /home/dev/SpaceGraph/docs/perf/RUNLOG.md (Reality-Check-Gate record: ABrain MCP_V2 + ADR-0003 read; adapter version-pin; live-smoke note)
    - spacegraph-core wire-protocol doc / inline (new hypothesis/note kinds + PROTOCOL_VERSION bump rationale)
- **External ESN contracts to reality-check:**
    - `ABrain MCP_V2_INTERFACE.md (capability-first tool shape; run_plan/explain)` — **VERIFIED** — Locally present: /home/dev/ESN/Modularium/ABrain/docs/architecture/MCP_V2_INTERFACE.md. Design template for the abrain-client tool calls. Spec author must pin exact tool names/args/return shape for run_plan + explain and the action_intents return field.
    - `ABrain ADR-0003-native-api-for-text-generation (HTTP /text/generate vs MCP run_plan decision; appliance posture)` — **ON_BRANCH** — Branch feat/adr-0003-appliance-posture — NOT in default checkout. This is the transport-choice ADR the Reality-Check-Gate names; cannot be read locally. Spec must pin chosen transport, endpoint (127.0.0.1:8788/UDS), provider-only/no-execute invariant, action_intents schema. Treat as NOT-LOCALLY-AVAILABLE until merged/checked out.
    - `OceanData OPERATOR_APPROVAL_ARCHITECTURE.md (binding approval discipline intents flow through)` — **ON_BRANCH** — Branch docs/operator-approval-architecture — NOT in default tree. v0.8.0 routes ABrain intents through the v0.7.0 approval layer derived from this. Spec must pin the approval object shape, approver≠requester rule, and mutation-pause semantics from it once available.
    - `Smolit-Assistant ADR-0005-adminbot-safety-boundary (AdminBot action vocabulary / capability-whitelist intents map to)` — **VERIFIED** — Local: Modularium/Smolit-Assistant/docs/adr/. Bounds which AdminBot actions an ABrain intent may propose. Relevant transitively via v0.7.0; spec must confirm the intent→adminbot_action_request mapping uses this closed vocabulary.
    - `esn-orchestrator MCP-hub proxy + live mcp__abrain__* tools (run_plan/run_task/explain/list_pending_approvals/approve/reject)` — **VERIFIED** — Hub proxy verified in tree (EcoSphereNetwork/esn-orchestrator/src/esn_orchestrator/mcp_proxy + ADR 0001) and a live MCP server exposes mcp__esn-orchestrator__mcp__abrain__* incl. abrain_run_plan/abrain_explain/abrain_list_pending_approvals/abrain_approve/abrain_reject — a queryable reference for the real tool surface, but the wire contract must still be pinned from MCP_V2_INTERFACE + ADR-0003.
    - `Auth posture (auth.md L1 loopback/UDS bearer, omit aud/iss) for the abrain-client call` — **NOT_LOCALLY_AVAILABLE** — No auth.md in SpaceGraph tree; ESN auth.md not under default ESN checkout (LabOS auth.md + esn-orchestrator ops/closeouts/mp-auth-posture-closeout-2026-05-31.md exist out-of-repo). Spec must pin the bearer/token path for the ABrain endpoint.
- **Verdict:** SPEC-REQUIRED
- **If SPEC-REQUIRED, the spec must specify:**
    * design questions to resolve:
        - Transport: MCP (rmcp client → abrain.run_plan/abrain.explain) vs HTTP POST /text/generate (127.0.0.1:8788/UDS)? Decide per ABrain ADR-0003 (ON_BRANCH — must be read once merged) and record the version-pin.
        - Does v0.8.0 proceed before v0.7.0's approval layer + spacegraph-adminbot-client land, or is it hard-blocked on them? (Neither exists today — grep confirms 0 scaffolding.) If sequenced after, spec must reference the v0.7.0 Approval object type by name.
        - How is a graph slice selected and bounded for the reasoning context — from explain.rs shortest_path (attack-path), from an alert cluster, or from selection? What max node/edge cap and redaction posture apply (no data-lake dump)?
        - How are returned action_intents detected as mutation-implying vs read-class, so mutation intents always pause at approval (ROADMAP gate)? Where does that classification live?
        - Are hypothesis/note added as new Node variants + EdgeKind variants in spacegraph-core (wire-protocol change, PROTOCOL_VERSION bump, agent-compat) OR as viewer-only annotation overlays not crossing the wire? This decides blast radius across core/agent/render/theme/legend.
        - Is the reasoning call viewer-initiated (operator triggers 'explain this cluster') only, or can it run automatically? (Auto reasoning that proposes mutating actions raises the auto-mode posture.)
        - correlation_id provenance: minted by SpaceGraph at request time and threaded ABrain→intent→approval→audit, or inherited from v0.7.0's audit trail? (No correlation_id exists in tree.)
    * data shapes / wire formats / tool schemas:
        - ABrain request payload: exact graph-slice context schema (node/edge projection from GraphState — which fields of the 6 Node / 7 EdgeKind variants are sent), the prompt/task framing, and version tag.
        - ABrain response schema: reasoning/explanation text + action_intents array — exact field set per intent (target node id, proposed AdminBot capability/action name, args, severity/risk class, rationale).
        - Intent → adminbot_action_request mapping: how an ABrain action_intent is translated into the v0.7.0 PROPOSED action object (must use AdminBot's closed capability-whitelist, not a free command).
        - New annotation node kinds (hypothesis, note): field shape (id constructor, linked target node id(s), source=ABrain, confidence?, ts, correlation_id) and the new EdgeKind(s) linking annotation→subject (e.g. Annotates/Hypothesizes).
        - Audit-trail record shape carrying correlation_id from reasoning request through proposed intent to approval/execution (inherited from v0.7.0 — reference, not redefine).
        - Adapter version-pin record format (which ABrain contract version / endpoint the build is pinned to).
    * decision points (with a recommendation):
        - MCP vs HTTP transport (gated on ABrain ADR-0003, ON_BRANCH).
        - hypothesis/note as core wire types (PROTOCOL_VERSION bump) vs viewer-only annotations.
        - Sequencing: block v0.8.0 on v0.7.0 approval-layer + adminbot-client completion, or build the abrain-client adapter standalone first with intents stubbed to the (future) gate.
        - Graph-slice source: explain.rs path / alert-cluster / manual selection (or all three) and the size cap.
        - ABrain endpoint binding (loopback HTTP port vs UDS) and auth token path.
        - Whether reasoning is operator-triggered only (recommended for NOT_AUTO posture) vs allowing background reasoning.
    * test & gate strategy (incl. headless):
        - Fixture alert cluster → mocked ABrain call → assert rendered reasoning + proposed (NOT executed) actions (ROADMAP gate).
        - Assert every mutation-implying intent pauses at the approval object and is never auto-executed (mirrors v0.7.0 'approve and execute is two steps').
        - Contract test of the abrain-client adapter against a recorded ABrain MCP_V2 / HTTP fixture (request/response round-trip), with the version-pin asserted.
        - Unit test: graph slice serializer (GraphState/explain.rs output → request payload) respects node/edge caps and redaction.
        - Core round-trip test for new hypothesis/note node+edge kinds across Delta/Msg serialization if they cross the wire (PROTOCOL_VERSION bump covered).
        - Audit assertion: correlation_id threads request→intent→approval→audit (depends on v0.7.0 audit trail existing).
        - Negative test: no native command execution introduced anywhere (tree audit, per v0.7.0 invariant).
    * security / threat-model needs:
        - Preserve the ABrain invariant: SpaceGraph consumes reasoning, ABrain proposes only, AdminBot + the approval layer execute — no auto-execution path from a returned intent (ROADMAP Notes).
        - Every mutating action_intent must enter the v0.7.0 Decision→Review→Approval→Execution→Audit gate with approver≠requester; the abrain-client may never call AdminBot directly.
        - Treat ABrain output as untrusted: validate/whitelist intents against AdminBot's closed capability vocabulary before they become proposed actions; reject intents referencing unknown nodes or non-whitelisted capabilities.
        - L1 auth posture for the ABrain endpoint: loopback/UDS, JWT bearer where used, minters omit aud/iss (auth.md — not locally present, must be pinned); one secret per node.
        - Bound the outbound graph slice: redaction-default, max_items/size caps, no full-graph or sensitive-field dump to the reasoning provider.
        - correlation_id threaded through the audit trail for every reasoning request and resulting proposed action (forensic traceability).
- **Auto-mode suitability:** NOT-AUTO — v0.8.0 produces action_intents that become PROPOSED AdminBot (mutating) actions routed through the security-critical v0.7.0 approval layer, which the roadmap mandates as NOT auto-mode (§3, §4). Although the ABrain call itself is read-shaped, the deliverable's purpose is to feed the mutating-action path and it adds new core wire types (hypothesis/note, PROTOCOL_VERSION bump). Output is untrusted LLM-proposed actions that must be human-gated. It inherits the NOT_AUTO posture of the action spine it plugs into.
- **Estimated size:** L

### v0.9.0 — OceanData history sink + context

- **Builds on (code that exists today):**
    - spacegraph-viewer `graph/timeline.rs` — the in-memory ringbuffer this phase scrubs past: `TimelineState` (`events: VecDeque<TimelineEvt>` capped at `max_events=20_000`, `window: Duration` 60s default, `node_life: HashMap<NodeId,NodeLife>{first_seen,last_seen,removed_at}`, `batch_spans: VecDeque<BatchSpan>`); `trim()` discards events older than the window — the exact data lost today and the recall surface a sink must restore.
    - spacegraph-viewer `graph/state.rs` `GraphState::apply_delta` (lines ~1190-1325) — the single choke point where every `Delta` (BatchBegin/End, Upsert/RemoveNode, Upsert/RemoveEdge) is applied with an `Instant` ts and pushed to the timeline via `push_timeline_at`; the natural tap point to mirror deltas to an OceanData sink.
    - spacegraph-viewer `graph/state.rs` `GraphState::apply` / `IncomingKind::{Snapshot,Event}` and `globalize_delta` — already namespaces ids per stream (`namespace::globalize`); the sink must record the namespaced/globalized form to round-trip multi-stream sessions.
    - spacegraph-core `src/lib.rs` — the wire types a sink serializes: `Delta`/`Node`/`Edge`/`EdgeKind` are `Serialize+Deserialize` (serde `tag=type,content=data`), `NodeId(String)`, `PROTOCOL_VERSION=3`. NOTE: `Delta` carries NO timestamp field — ts is assigned viewer-side as `Instant::now()` in `apply_delta`; `Instant` is not serializable, so a sink needs a wall-clock mapping (only `Node::Alert.ts` is an ISO string today).
    - spacegraph-viewer `util/config.rs` `ViewerConfig` (serde, toml round-trip, `load_or_default`/`save`, per-field `#[serde(default)]` fns) + `AgentEndpoint`/`AgentEndpointKind::UdsPath` — the established pattern for adding an opt-in, default-off OceanData provider config block (UDS/loopback endpoint, caps), mirrored after the agent-endpoint precedent.
    - spacegraph-viewer `net/uds.rs` (`spawn_reader` framed version-checked UDS client, `LengthDelimitedCodec`) and `net/protocol.rs` — the existing UDS client transport precedent for a loopback/UDS context-provider client (read-side SPI) and any UDS sink transport.
    - spacegraph-viewer `graph/state.rs` `TimelineState` scrub/freeze controls (`effective_now`, `scrub_seconds`, `frozen_now`, `set_timeline_pause`) and `render/timeline.rs`/`draw_timeline` — the scrubbable-timeline UI that history recall feeds; the gate ('round-trips back into a scrubbable timeline') is asserted against this surface.
    - ROADMAP O-decisions: O-4 (asset/audit vs dedicated time-series sink) is OPEN; v0.9.0 sits after v0.7.0 approval layer + v0.8.0 ABrain in the §4 ladder; auth posture L1 (loopback/UDS, JWT bearer, omit aud/iss) from §2/§5 and `auth.md`.
- **Gaps (code missing for this phase):**
    - O-4 is explicitly OPEN (ROADMAP §6): history persistence model = OceanData asset/audit (`/assets`, `/audit/events`) vs a dedicated time-series sink. Roadmap recommendation is 'start asset/audit, revisit', but the choice drives the entire sink wire shape, query shape, and recall path — unresolved design decision → cannot AUTO.
    - No sink/recall code exists anywhere in the tree. CODE_INVENTORY shows three crates (core/agent/viewer); no `spacegraph-oceandata-client` crate, no HTTP client, no persistence module. The phase introduces a wholly new crate + new IPC family per the consumer-adapter pattern (cf. proposed v0.7.0 `spacegraph-adminbot-client`, v0.8.0 `spacegraph-abrain-client`).
    - `Delta` has no timestamp and the timeline uses monotonic `Instant` (not serializable, no wall-clock anchor). A sink/recall round-trip needs a persisted wall-clock ts per event and a mapping back to the `Instant`-based `TimelineState` — a data-shape gap that must be designed (only `Node::Alert.ts` ISO string exists today).
    - Recall/scrub-past-ringbuffer has no read path: `TimelineState::trim` permanently drops events past `window`/`max_events`; nothing reloads historical events into `events`/`node_life`/`batch_spans`. The 'scrub back into a scrubbable timeline' gate requires a new backfill mechanism into these structures — not present.
    - OceanData sink surface (`/assets`, `/audit/events`) and the binding approval discipline `OPERATOR_APPROVAL_ARCHITECTURE.md` are ON-BRANCH only (NOT locally checked out) — the actual wire shapes for the write/sink side cannot be pinned from the working tree.
    - OceanData-side context wire contract (`context_query_contract.md`, OceanData `ADR-0004 §6` authoritative server form) is ON-BRANCH only / NOT locally available. The locally-VERIFIED ADR-0006 is the Smolit-Assistant CONSUMER-side SPI conceptual model, explicitly NOT the OceanData wire contract and explicitly Proposed/docs-only with no implementing code — so the server-side request/response shape SpaceGraph must speak is not authoritatively available.
    - `correlation_id` is not yet a real wire/audit field (ADR-0006 §4.8: 'behauptet kein bestehendes Wire-Feld'; adopted only once AUDIT_CORRELATION_ID_SPEC FA-1 lands). v0.8.0 already threads correlation_id into audit; the cross-version threading contract for sink/context audit entries is unpinned.
    - Auth path is unpinned: ADR-0006 allows `auth_mode = local_peer | bearer` with bearer tokens only from a 0600 secret store. SpaceGraph has no secret-store/bearer plumbing today (config is plaintext toml). The 'auth path tested' gate has no implementation substrate.
    - No `CONSUMERS.md` exists in the SpaceGraph tree (Track B/C roadmap deliverable); ADR-0003 (SpaceGraph OceanData) does not exist. The relationship-registration deliverables have no current files to extend.
- **SpaceGraph docs to touch:**
    - /home/dev/SpaceGraph/docs/adr/ADR-0003-oceandata-history-and-context.md (new; SpaceGraph-local ADR numbering per ROADMAP §5)
    - /home/dev/SpaceGraph/docs/adr/ (new ADR resolving O-4: asset/audit vs dedicated time-series sink)
    - /home/dev/SpaceGraph/CONSUMERS.md (new or extended; §3 OceanData consumer entry — history sink + context-provider)
    - /home/dev/SpaceGraph/docs/ROADMAP.md (mark O-4 resolved in §6; v0.9.0 RUNLOG/Reality-Check record per §5)
    - /home/dev/SpaceGraph/docs/ACCEPTANCE.md (add v0.9.0 gates: session round-trips sink→scrubbable timeline; SPI honours local_only + caps; auth path)
    - /home/dev/SpaceGraph/docs/perf/RUNLOG.md (Reality-Check-Gate entry: which OceanData branches/contracts were read at implementation time)
    - /home/dev/SpaceGraph/AGENTS.md / README (controls + new config block for the opt-in, default-off OceanData provider)
- **External ESN contracts to reality-check:**
    - `OceanData Context-Provider SPI — consumer-side conceptual model (Smolit-Assistant ADR-0006-oceandata-context-provider-spi.md)` — **VERIFIED** — Locally present at Modularium/Smolit-Assistant/docs/adr/. CRITICAL: this is the Smolit-Assistant SPI form, explicitly NOT the OceanData wire contract and explicitly Proposed/docs-only with no implementing code. Pins the candidate shapes SpaceGraph should mirror: ContextQueryRequest {contract_version, request_id, query, context_scope(user|project|session|system), purpose(assistant_context|action_planning|debug), max_items, redaction(local_only|external_safe), include_provenance}; ContextQueryResponse {status(ok|refused|error), items[id,title,summary,source,sensitivity,provenance{kind,uri,timestamp}], error{code,message}}; ops query_context/fetch_context_summary/list_available_contexts/inspect_context_item_metadata; defaults default-off, local_only, allow_sensitive=false, allow_external_forwarding=false, max_items<=max_items_default, transport unix|loopback_http, auth local_peer|bearer(0600 store).
    - `OceanData context_query_contract.md + OceanData ADR-0004 §6 (authoritative server-side wire form)` — **ON_BRANCH** — context_query_contract.md on branch docs/fa1-context-query-wire-contract (+ PR13.x approval domain on feat/pr13-1-approval-domain). NOT in default checkout → the authoritative OceanData-side request/response wire the SPI sits over is not verifiable locally. Must be checked out/merged before the read-side adapter can be hard-pinned.
    - `OceanData HTTP sink surface (/assets, /audit/events) + OPERATOR_APPROVAL_ARCHITECTURE.md` — **ON_BRANCH** — OPERATOR_APPROVAL_ARCHITECTURE.md on branch docs/operator-approval-architecture. NOT in default checkout → the asset/audit sink wire shapes and the binding approval discipline are not verifiable locally. These define the entire sink (write) side once O-4 picks asset/audit.
    - `Auth posture (auth.md / ADR-068 smolit-stack-auth-posture / esn-orchestrator auth-posture closeout)` — **ON_BRANCH** — Cockpit ADR-068 on branches feat/adr-068-amend-l1-attach, feat/auth-posture-adr (NOT-LOCALLY-AVAILABLE as a doc). A LabOS auth.md exists (/home/dev/LabOS/docs/modules/auth.md) and an ESN auth-posture closeout exists at esn-orchestrator/ops/closeouts/mp-auth-posture-closeout-2026-05-31.md; ESN-Auth repo present. L1 posture (loopback/UDS, bearer, omit aud/iss) is summarized in the SpaceGraph ROADMAP §2/§5 but the binding ADR-068 is not in the SpaceGraph-reachable default tree.
    - `AUDIT_CORRELATION_ID_SPEC (referenced by ADR-0006 §4.8/§9)` — **NOT_LOCALLY_AVAILABLE** — Referenced by the VERIFIED ADR-0006 as the gate for adopting correlation_id, but the spec file itself is not present in the default checkout. correlation_id is therefore not yet a real wire/audit field; the cross-version audit-threading contract is unpinned.
- **Verdict:** SPEC-REQUIRED
- **If SPEC-REQUIRED, the spec must specify:**
    * design questions to resolve:
        - Resolve O-4: persist SpaceGraph deltas via OceanData asset/audit model (/assets, /audit/events) or a dedicated time-series sink? This single choice determines the sink wire shape, the recall query shape, and whether deltas map to assets vs audit events vs TS points.
        - Where is the sink tap: mirror at `GraphState::apply_delta` (post-globalize, after ts assignment) vs a separate buffered/batched exporter task off the net pump? Define sync vs async, backpressure, and whether sink failure ever blocks the viewer (it must not).
        - How does history recall re-enter the timeline: a new backfill API that loads historical events into `TimelineState.{events,node_life,batch_spans}` past `trim`'s window, or a separate read-only history view? Define how monotonic `Instant`-based scrub coexists with wall-clock-anchored historical events.
        - Per ADR-0006: must SpaceGraph implement a `local_static_context` provider first (to verify the SPI contract without an OceanData dependency) before `oceandata_context`? Confirm the spike-provider-first sequencing.
        - Is the context-read path strictly read-only and decoupled from the sink-write path (separate crates/config/capabilities), per ADR-0006's separate-axis principle?
        - Does any context-read result ever feed an action path (v0.7.0 AdminBot / v0.8.0 ABrain)? If so, correlation_id becomes mandatory and approval/audit rules escalate — define the boundary explicitly.
    * data shapes / wire formats / tool schemas:
        - Persisted event record for the sink: wall-clock timestamp (UTC ISO-8601) per `Delta` — currently `Delta` has NO ts and the viewer uses non-serializable `Instant`; define the ts capture point and format, plus how it maps back to `TimelineState` on recall.
        - Serialized form of `Delta`/`Node`/`Edge` for the sink: reuse spacegraph-core serde (tag=type,content=data, PROTOCOL_VERSION=3) or an OceanData-native asset/audit envelope? Pin the field mapping (namespaced NodeId, batch ids, edge kinds).
        - Session/recording identity: a session id + stream namespace recorded so multi-stream globalized ids round-trip; recall key shape (time range, session, host/stream filter).
        - ContextQueryRequest fields SpaceGraph emits (from ADR-0006 §7.1): contract_version, request_id, query, context_scope, purpose, max_items, redaction, include_provenance — pin SpaceGraph's defaults (context_scope, purpose) and the graph-slice→query mapping.
        - ContextQueryResponse handling (ADR-0006 §7.2): items[id,title,summary,source,sensitivity,provenance{kind,uri,timestamp}] + status(ok|refused|error) + error{code,message}; define how items render in-scene (the roadmap's cross-source context) and that provenance.uri is treated redacted-by-default.
        - Provider config block in `ViewerConfig`: provider_kind, enabled(default false), endpoint(unix/loopback only), transport, auth_mode, max_items_default, max_summary_chars, allow_sensitive(false), allow_external_forwarding(false) — mirror ADR-0006 §6.1 with serde `#[serde(default)]` per existing config convention.
    * decision points (with a recommendation):
        - O-4 sink model (asset/audit vs time-series) — must be ADR'd before any sink code.
        - Default-off enforcement for BOTH sink and context-read (ADR-0006: default-off, no default-chain, no auto-add of OceanData endpoints at first run).
        - Caps enforcement: max_items <= max_items_default (reject as too_many_results), max_summary_chars cutoff, redaction=local_only default, allow_sensitive=false default, allow_external_forwarding=false default.
        - Whether the history sink is opt-in too (privacy: SpaceGraph deltas include file paths, process cmdlines, sockets — sending these to OceanData is a data-egress decision needing explicit operator opt-in).
        - Adapter version-tagging: record contract_version / pinned OceanData branch+commit per ROADMAP §5 hard-pin discipline (no invented adapters).
    * test & gate strategy (incl. headless):
        - Fixture session round-trip: synthetic delta stream → sink serialize → recall deserialize → backfill into TimelineState → assert events/node_life/batch_spans reconstruct and the timeline scrubs to a pre-window timestamp (the ROADMAP gate).
        - Caps/redaction unit tests: a context query with max_items>default is rejected; redaction defaults to local_only; external_safe/allow_sensitive refused unless explicitly enabled (assert refused/error codes per ADR-0006 §7/§12).
        - Default-off test: with no opt-in config, zero sink writes and zero context calls occur (assert no network/UDS activity).
        - Auth-path test: bearer token sourced only from a 0600 secret store (not plaintext toml); local_peer/SO_PEERCRED path on UDS — assert the token never appears in serialized config.
        - Provider-config round-trip test (mirror existing `viewer_config_roundtrip_save_load`) for the new OceanData block.
        - Live-smoke (documented, not a CI stop): against a real OceanData once the ON-BRANCH contracts are merged — record in RUNLOG per Reality-Check-Gate.
        - no-data-leak audit test: assert sink/audit never serializes raw query strings for sensitivity>=private, no tokens, no full provenance.uri (ADR-0006 §9 'Niemals erfasst').
    * security / threat-model needs:
        - Loopback/UDS only — endpoint must reject remote URLs while transport ∈ {unix, loopback_http} (ADR-0006 §6.1).
        - Default-off + opt-in for both egress (sink) and ingest (context); no first-run auto-add of OceanData endpoints.
        - Redaction local_only default; external forwarding hard-blocked (allow_external_forwarding=false) absent a Privacy/Redaction gate that does not exist yet.
        - Bearer tokens only from a 0600 secret store; no plaintext tokens in viewer.toml (SpaceGraph has no secret store today — must be built).
        - Data-egress review: SpaceGraph deltas carry sensitive host data (paths, cmdlines, sockets, alerts); sink is an exfiltration surface and needs explicit operator consent + audit of what was sent.
        - Audit discipline: never log raw query strings for sensitivity>=private, full summaries, plaintext provenance.uri, or tokens (ADR-0006 §9).
        - correlation_id threading once a context read feeds an action path (mandatory then; not a wire field yet — gate on AUDIT_CORRELATION_ID_SPEC).
        - Sink failures must never block or crash the viewer render/IPC paths (no unwrap/expect per ROADMAP §5 quality gate).
- **Auto-mode suitability:** NOT-AUTO — Security-sensitive cross-repo data egress + ingest. The sink ships SpaceGraph deltas (file paths, process cmdlines, sockets, alerts) out to OceanData — a real exfiltration surface requiring explicit operator opt-in, redaction defaults, and a 0600 bearer secret store SpaceGraph does not yet have. The read side is bounded/capped/default-off context with refused/error policy modes. Even though the context-read is conceptually read-only, the egress sink and the auth/secret-store work make this not safe viewer-side auto work; it must follow the hard-pin + Reality-Check discipline (the same posture as the other Track C consumer phases), and its core contracts are not even locally available yet.
- **Estimated size:** L

### Track D — Security-analytics depth (parallel)

- **Builds on (code that exists today):**
    - graph/explain.rs: BFS shortest_path(model,a,b,max_depth,allowed) over EdgeKindClass — the existing graph-traversal primitive a topology rule engine extends from (multi-hop pattern match instead of A→B path).
    - graph/model.rs: GraphModel with prebuilt adjacency (adj: HashMap<NodeId,SmallVec>), edges_for_node/neighbors/degree (O(1)), and the AggEdge/EdgeStats first_ts/last_ts/count index — the canonical in-memory graph a rule engine reads; EdgeKindClass is the typed edge vocabulary rules match on.
    - graph/state.rs: GraphState alert plumbing — note_alert/alert_order (VecDeque), max_visible_alerts cap+eviction (default 200), alert_severity_counts, alerts_newest_first — the existing path for ingesting/capping/triaging Alert nodes that graph-native detections would become first-class members of.
    - spacegraph-core/src/lib.rs: Node::Alert{source,signature,severity,ts} + EdgeKind::AlertsOn + id_alert(node_id,key) — the wire/type the detection engine reuses to emit detections as alert nodes (source could be e.g. "spacegraph-rule" instead of "suricata").
    - spacegraph-agent sources/mod.rs: EventSource trait (name()+start(self,node_id,tx)) with FsSource/ProcSource/NetSource/SuricataEveSource impls; main.rs builds Vec<Box<dyn EventSource>> and fans onto the broadcast bus — the documented, uniform extension point each new collector (eBPF/auditd/Zeek/Falco) plugs into, exactly as the trait doc comment states.
    - sources/suricata_eve.rs: end-to-end precedent for a new EventSource — pure parse fns (parse_eve_alert/build_alert_graph/alert_deltas) + thin tail loop, fixture-tested, 5-tuple correlation to id_remote_host — the template for auditd/Zeek/Falco sources and for CVE/attack-surface enrichment emitting node metadata.
    - graph/namespace.rs: per-stream id prefixing (globalize/globalize_edge/origin/local_part/prefix, NS_SEP) keeping multi-agent streams collision-free — the substrate fleet/cross-host work builds on (origin() already recovers the host of any id).
    - sources/net.rs: NetSource process↔socket↔remote-host topology (diff-based) — supplies the ConnectsTo/ListensOn/OwnsSocket edges and RemoteHost nodes that both topology rules (new outbound socket) and fleet stitching (RemoteHost→another monitored host) operate over.
- **Gaps (code missing for this phase):**
    - No rule/detection engine exists: grep for rule/detect/lateral/heuristic in viewer/ finds only hover-detection, edge-detection and frame-pacing — nothing in graph/. There is no multi-node/multi-edge pattern matcher, no rule type, no rule registry, no detection→Alert emission path. explain.rs only does single A→B BFS; it is not a pattern engine.
    - Alerts are produced only externally (agent-side SuricataEveSource); the viewer has no path to synthesize an Alert node from its own graph state. note_alert/alert_order assume alerts arrive as Deltas over the wire — emitting a locally-derived detection as a first-class alert node needs a new in-viewer producer + id scheme (no spacegraph-rule source string, no local id_alert call site in the viewer).
    - No CVE / package-version / open-port-baseline surface anywhere: grep across agent+core finds zero package/cve/nmap/baseline. Node::Process carries exe/cmdline but no package or version; there is no node/edge kind nor any field to attach a CVE tag or attack-surface annotation. nmap/CVE enrichment has no collector and no data model.
    - No additional EventSources beyond fs/proc/net/suricata_eve. Capabilities.ebpf is hardcoded false in main.rs; there is no eBPF/auditd/Zeek/Falco source, no Cargo feature for them, and (per the roadmap) eBPF is its own MP.
    - Fleet/cross-host stitching is absent: namespace.rs explicitly states namespaces are "never merged" — there is no logic to recognize that one host's RemoteHost{addr} equals another monitored host's identity and stitch a cross-host edge, no host-grouping/fleet-overview model, no fleet UI. The substrate (origin/prefix) exists but the join does not.
    - No publish path for findings: Track D's detections/enrichment have no MCP surface (Track B v0.6.0 not built — no spacegraph-mcp crate) and no approval layer (Track C v0.7.0 not built). Publishing findings as actionable items has no downstream contract yet.
    - No persistence beyond the in-memory ringbuffer (roadmap §1 gap #4) — detections/enrichment computed now are not retained for forensics until Track D's OceanData sink (v0.9.0) exists.
- **SpaceGraph docs to touch:**
    - docs/ROADMAP.md (Track D §3 block — record which sub-area's MP is being specced and its sequencing vs v0.6.0/v0.7.0)
    - docs/adr/ (new SpaceGraph-local ADR for the detection-rule engine design — rule representation, where it runs, how detections become alert nodes; per ROADMAP §5 ADR-per-decision)
    - docs/adr/ (separate ADR per new EventSource family — eBPF/auditd/Zeek/Falco — each its own MP per the roadmap; and an ADR for the CVE/attack-surface enrichment data model)
    - docs/ACCEPTANCE.md (gates for rule-engine fixtures and any new source)
    - docs/recon/CODE_INVENTORY.md (add new graph/rules module + any new agent source once landed)
    - CONSUMERS.md (only when Track D actually publishes via the Track B MCP surface / Track C approval layer — a §3 entry per the WoW)
    - spacegraph-core CHANGELOG / PROTOCOL_VERSION note (if detections-as-alerts or CVE tags require new Node/Edge/field → wire schema bump, since v3 already gates on protocol)
- **External ESN contracts to reality-check:**
    - `orchestrator MCP-hub proxy (Track B v0.6.0 publish path)` — **VERIFIED** — Local at EcoSphereNetwork/esn-orchestrator/{src/esn_orchestrator/mcp_proxy, docs/adr/0001-mcp-hub-proxy.md}; live mcp__esn-orchestrator__* channel. Not a Track D dep until findings are published; the spacegraph-mcp surface it would proxy does not exist yet (Track B unbuilt).
    - `ABrain MCP_V2_INTERFACE.md (capability-first tool shape, reference for any analytics tool surface)` — **VERIFIED** — Local at Modularium/ABrain/docs/architecture/MCP_V2_INTERFACE.md. Only relevant if Track D analytics are exposed as MCP tools via Track B; not a hard dep for viewer-side detection work.
    - `OceanData OPERATOR_APPROVAL_ARCHITECTURE.md (Track C approval discipline for actionable findings)` — **ON_BRANCH** — On branch docs/operator-approval-architecture — NOT in default checkout, treat as not-locally-available. Needed only when a detection drives a mutating remediation through Track C (v0.7.0).
    - `OceanData context_query_contract.md + PR13.x approval domain (forensic history sink for retained detections)` — **ON_BRANCH** — On branches docs/fa1-context-query-wire-contract and feat/pr13-1-approval-domain — not locally available. Relevant only at Track D persistence/forensics (aligns with v0.9.0), not for in-memory detection.
    - `Smolit-Assistant ADR-0005-adminbot-safety-boundary.md (action vocabulary if a finding triggers remediation)` — **VERIFIED** — Local at Modularium/Smolit-Assistant/docs/adr/. Only engaged when Track D publishes an actionable finding through the Track C AdminBot path; pure analytics has no AdminBot dep.
    - `auth posture (auth.md / L1 bearer, omit aud/iss) for any published surface` — **VERIFIED** — LabOS auth.md at /home/dev/LabOS/docs/modules/auth.md + esn-orchestrator closeout ops/closeouts/mp-auth-posture-closeout-2026-05-31.md. Applies only to the publish surface (Track B), not to viewer-side detection logic. Cockpit ADR-068 auth-posture is ON_BRANCH (feat/auth-posture-adr).
    - `INTERFACE_INVENTORY.md / Hexa-Repo map (formal fabric membership)` — **NOT_LOCALLY_AVAILABLE** — Shared INTERFACE_INVENTORY.md not found by name in the ESN tree. Track D does not need it; it is a Track B (v0.6.0 O-5 admission) concern.
- **Verdict:** SPEC-REQUIRED
- **If SPEC-REQUIRED, the spec must specify:**
    * design questions to resolve:
        - Scope: Track D is four distinct deliverables (graph-native detection rule engine; new EventSources eBPF/auditd/Zeek/Falco; CVE/attack-surface enrichment; fleet/cross-host stitching) — the roadmap mandates each EventSource is its own MP and eBPF its own rabbit hole. Which ONE sub-area does this spec cover? (They cannot share one MP.)
        - Rule engine seat: does the detection rule engine run in the viewer (graph/rules — reading the canonical GraphModel, matching the existing explain.rs/EdgeKindClass primitives) or agent-side? Roadmap says "small rule engine in graph/" → viewer-side; confirm and pin the module path (e.g. graph/rules.rs) so it reads the same GraphState projection (no parallel graph).
        - Rule representation: hardcoded Rust matchers vs a small data-driven DSL (which would echo the v0.5.0 Query-DSL). The roadmap example (process spawns shell + new outbound socket + alert = lateral-movement) is a temporal multi-edge pattern over EdgeKindClass — specify pattern grammar, temporal window semantics (using AggEdge.first_ts/last_ts), and how 'new' (recency) is defined.
        - Detection lifecycle: when a rule fires, is the detection a transient overlay or a durable Alert node? If durable, does it flow through note_alert/alert_order (sharing the max_visible_alerts cap with external alerts) or a separate detection store? How are detections de-duplicated / re-armed when the matched subgraph persists across frames?
        - Fleet stitching (if that sub-area): namespace.rs explicitly never merges namespaces — specify the cross-host identity match (RemoteHost{addr} of host A ≡ NodeIdentity/hostname of host B), what new cross-host edge kind represents the stitch, and whether stitching mutates the graph or is a render-only join (to preserve the never-merge invariant).
        - CVE/enrichment (if that sub-area): native/safe enrichment with no child_process exec — specify the data source (offline CVE DB? package-manager query? passive port-vs-baseline) and that it is read-only; the roadmap forbids `child_process exec`-style nmap.
        - Publish boundary: detections are viewer-side and need no ESN dep UNTIL published — confirm this spec stops at in-scene/in-viewer rendering and explicitly defers MCP exposure (Track B v0.6.0) and any remediation (Track C v0.7.0).
    * data shapes / wire formats / tool schemas:
        - Rule definition shape: a Rule { id, name, severity, pattern (node-kind + EdgeKindClass sequence), temporal_window } — concrete struct/serde form; whether rules are config-loaded (extend util/config.rs ViewerConfig) or compiled-in.
        - Detection→Alert mapping: if detections become Node::Alert, fix source (e.g. "spacegraph-rule"), signature (rule name), severity vocabulary (reuse low/medium/high), ts, and the id_alert key scheme (must be stable for de-dup across frames; current key for suricata is timestamp|signature|5-tuple).
        - Whether new Node/Edge kinds or fields are needed (e.g. a Detection node distinct from Alert, a CVE tag field on Process, a cross-host stitch EdgeKind). Any of these is a spacegraph-core wire change → PROTOCOL_VERSION bump from 3 and a documented schema migration.
        - New EventSource output shape (if eBPF/auditd/Zeek/Falco): which existing Node/Edge kinds it emits vs whether it needs new ones; its Capabilities flag (e.g. set caps.ebpf=true, currently hardcoded false in main.rs); its CLI flag(s) mirroring --eve-file.
        - Enrichment annotation shape: how a CVE tag / package-version / open-port-vs-baseline attaches to a node without a parallel store (field on Node vs a sidecar annotation map keyed by NodeId).
    * decision points (with a recommendation):
        - Pick exactly one Track D sub-area for this MP (rule engine is the highest-leverage, no-new-collector start and uses existing primitives — recommend it first; eBPF deliberately deferred per roadmap).
        - Rule engine: data-driven DSL vs compiled matchers (recommend compiled matchers for the first 1-2 rules, DSL deferred — matches existing-code-first/no-speculative-generality discipline).
        - Detections as Node::Alert reusing existing cap/triage plumbing vs a separate Detection kind (reusing Alert avoids a wire bump; a distinct kind gives clearer rendering — trade-off must be decided).
        - Whether any change touches spacegraph-core (and thus PROTOCOL_VERSION) — prefer keeping Track D viewer-only/no-wire-change for the first MP to stay AUTO-safe and avoid agent/viewer version coupling.
        - Fleet stitching as render-only join (preserves namespace never-merge) vs graph-level merge (recommend render-only).
        - Where the rule engine runs in the schedule: a new Update system after update_layout_or_timeline / on the canonical GraphState, with a budget (mirroring layout_budget_ms) so detection cost cannot stall the frame.
    * test & gate strategy (incl. headless):
        - Fixture-graph → expected-detections unit tests, mirroring suricata_eve's pure-function fixture style: build a GraphModel with the lateral-movement subgraph, run the rule, assert the detection (and a negative fixture that must NOT fire). This is the same test posture as explain.rs shortest_path tests.
        - De-dup/re-arm test: same subgraph across two ticks yields one detection, not N; detection cap/eviction interaction with max_visible_alerts asserted.
        - If a new EventSource: pure parse fn + committed fixture file (exactly as sources/fixtures/suricata_eve.jsonl is used in fixture_file_yields_three_alerts), plus a count/severity assertion.
        - Config round-trip test if rules/enrichment add ViewerConfig fields (the inventory §3 4-way discipline: struct+Default, serialize no serde(skip), apply_viewer_config round-trip).
        - Workspace gates per ROADMAP §5: fmt --check, clippy --workspace --all-targets -D warnings, test --workspace; no unwrap/expect in render/IPC paths.
        - Performance: rule engine runs under a documented budget on the layout-bench scales (500/1000/2000/5000 nodes, benches/layout.rs) — no per-frame full-graph rescan; reuse adjacency/AggEdge indices.
        - Negative/safety gate for enrichment: assert no child_process/exec anywhere in the tree (the same audited-no-native-execution posture the roadmap applies to Track C).
    * security / threat-model needs:
        - Enrichment must be read-only and native — explicit gate that CVE/nmap/attack-surface enrichment performs NO command execution (no child_process exec), matching the roadmap's native/safe mandate; offline/passive data sources only.
        - Detections are advisory until published — they must not auto-trigger any AdminBot action; any remediation path is deferred to Track C v0.7.0 (NOT_AUTO) and goes through the Decision→Review→Approval→Execution→Audit shape.
        - New EventSources keep the agent's read-only posture (EventSource collectors observe, never act); eBPF/auditd require elevated capability — pin the AgentMode/privilege story and that caps are advertised honestly (Capabilities flags), with the privileged-without-root warning path (main.rs already warns).
        - Wire-schema changes (if any) must bump PROTOCOL_VERSION and keep the Hello mismatch-reject behavior intact (legacy decodes to 0 and is rejected) — no silent schema drift between agent and viewer.
        - Cross-host/fleet data stays L1 loopback/UDS posture; no remote transport introduced by Track D (RS256/JWKS is L3, out of scope per §5).
- **Auto-mode suitability:** AUTO — The core Track D analytics work is viewer-side and read-only: a detection rule engine in graph/ reading the canonical GraphModel, and passive enrichment/fleet rendering — none of it executes anything on the host or mutates external state, so it is AUTO-safe like Track A. Two explicit carve-outs that flip to NOT_AUTO: (1) if a sub-area requires a spacegraph-core wire change (PROTOCOL_VERSION bump / agent collector with elevated eBPF/auditd capability), that crosses the agent privilege boundary and should be specced/reviewed rather than auto-run; (2) the moment Track D PUBLISHES findings, it inherits its downstream track's mode — Track B MCP (AUTO-capable, read-only) and Track C approval/remediation (NOT_AUTO by roadmap mandate, v0.7.0 hard-stop). The recommended first MP (compiled-matcher rule engine, detections as Alert nodes, no wire change, no publish) is fully AUTO; eBPF and any remediation are deliberately out of that AUTO scope.
- **Estimated size:** L

## Part C — Consolidated handoff

| Phase | Verdict | Auto-mode | Size | Headline blocker |
|---|---|---|---|---|
| v0.5.0 — UX/UI shell + ESN house-standard alignment | SPEC-REQUIRED | AUTO | L | O-1 resolution: token/typography parity only, or deeper Smolitux alignment? (roadmap recom |
| v0.6.0 — SpaceGraph MCP server (read-only) + formal ESN admission | SPEC-REQUIRED | AUTO | L | CANONICAL-STATE ACCESS (the crux): how does an out-of-process stdio MCP server read the in |
| v0.6.x — Cockpit tab (embed) | SPEC-REQUIRED | AUTO | M | O-2 (open in ROADMAP §6): the exact embed mechanism for a native Bevy window. Roadmap reco |
| v0.7.0 — AdminBot integration (security-critical) | SPEC-REQUIRED | NOT-AUTO | L | Which is the FIRST onboarded action — process.snapshot or journal.query — and what is its  |
| v0.8.0 — ABrain reasoning | SPEC-REQUIRED | NOT-AUTO | L | Transport: MCP (rmcp client → abrain.run_plan/abrain.explain) vs HTTP POST /text/generate  |
| v0.9.0 — OceanData history sink + context | SPEC-REQUIRED | NOT-AUTO | L | Resolve O-4: persist SpaceGraph deltas via OceanData asset/audit model (/assets, /audit/ev |
| Track D — Security-analytics depth (parallel) | SPEC-REQUIRED | AUTO | L | Scope: Track D is four distinct deliverables (graph-native detection rule engine; new Even |

**Recommended chronological start point:** **v0.5.0** (next rung on the ladder
after the shipped v0.4.0; Track-A, auto-safe, no ESN dependency — gets the
operator shell + ESN house-standard alignment in before fabric work). Then the
keystone **v0.6.0** (MCP server + formal admission) which every consumer phase
(v0.6.x/v0.7.0/v0.8.0) depends on. **v0.7.0 (AdminBot) is the single NOT-AUTO,
hard-stop phase.** Track D runs opportunistically in parallel (publishes via the
v0.6.0 MCP surface + v0.7.0 approval layer).

**Cross-cutting prerequisite for v0.6.0+ (fabric phases):** several binding
contracts are NOT in the default local checkout — `INTERFACE_INVENTORY.md` (the
Hexa-Repo/admission map) is absent entirely, and OceanData approval-arch +
context-query-contract, ABrain ADR-0003, and Cockpit ADR-066/068 live on
**unmerged branches**. Each fabric phase's Reality-Check-Gate must first obtain
these (check out the branch / wait for merge / query the live `esn-orchestrator`
MCP) — do not let a spec proceed on guessed contracts.

### Specs to be written (operator checklist — union of SPEC-REQUIRED phases)

- [ ] **SPEC: v0.5.0 — UX/UI shell + ESN house-standard alignment** — AUTO, size L. Must pin: O-1 resolution: token/typography parity only, or deeper Smolitux alignment? (roadmap recommends parity; ADR required bef
- [ ] **SPEC: v0.6.0 — SpaceGraph MCP server (read-only) + formal ESN admission** — AUTO, size L. Must pin: CANONICAL-STATE ACCESS (the crux): how does an out-of-process stdio MCP server read the in-process Bevy Resource GraphSt
- [ ] **SPEC: v0.6.x — Cockpit tab (embed)** — AUTO, size M. Must pin: O-2 (open in ROADMAP §6): the exact embed mechanism for a native Bevy window. Roadmap recommends 'thin React panel over 
- [ ] **SPEC: v0.7.0 — AdminBot integration (security-critical)** — NOT-AUTO, size L. Must pin: Which is the FIRST onboarded action — process.snapshot or journal.query — and what is its exact AdminBot adminbot_action
- [ ] **SPEC: v0.8.0 — ABrain reasoning** — NOT-AUTO, size L. Must pin: Transport: MCP (rmcp client → abrain.run_plan/abrain.explain) vs HTTP POST /text/generate (127.0.0.1:8788/UDS)? Decide p
- [ ] **SPEC: v0.9.0 — OceanData history sink + context** — NOT-AUTO, size L. Must pin: Resolve O-4: persist SpaceGraph deltas via OceanData asset/audit model (/assets, /audit/events) or a dedicated time-seri
- [ ] **SPEC: Track D — Security-analytics depth (parallel)** — AUTO, size L. Must pin: Scope: Track D is four distinct deliverables (graph-native detection rule engine; new EventSources eBPF/auditd/Zeek/Falc

*(All 7 phases are SPEC-REQUIRED — none is auto-implementable straight from the roadmap detail. Full per-spec contents in Part B.)*
