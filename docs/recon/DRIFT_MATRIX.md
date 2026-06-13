# SpaceGraph — Doc↔Code Drift Matrix

Cross-check of every doc against `CODE_INVENTORY.md` + the actual source, at
`2a8aa41` (v0.4.0). Produced by a 9-way parallel doc audit; the high-impact rows
were re-verified deterministically by the recon driver.

**Tally (107 rows):** CONSISTENT 62 · STALE 23 · UNDOCUMENTED 13 · OVERCLAIM 7 ·
NAMING 2.

Categories: **OVERCLAIM** (doc asserts behaviour the code lacks / doesn't run),
**UNDOCUMENTED** (real behaviour with no doc coverage), **STALE** (gate / cross-ref
/ version / status no longer true), **NAMING** (hygiene), **CONSISTENT** (verified
true & current). Action = in-scope reconciliation (Phase 3), `FINDING` (design /
behaviour — not edited this run), or `leave` (out of scope / correct).

---

## A. Anti-regression — v0.4.0 deliverables (CONSISTENT, asserted)

> **All v0.4.0 deliverables are present AND registered/reachable** (inventory §2):
> per-type geometry (`render/node_mesh.rs`, in the strict-order chain),
> lock-on reticle + micro-tags (`ui/reticle.rs`, registered group 1), orbital
> rings (`sync_node_rings`+`rotate_node_rings`), grab-to-pin / edge-picking /
> radial context menu (`render/spatial.rs` + `ui/context_menu.rs`, registered),
> cyberspace post-FX (`render/postfx.rs` + `PostFxPlugin` render-graph node),
> mesh edges (`render/edges.rs`). **`inspector_overlay` + `legend_overlay` are
> registered (`app/mod.rs:81-82`)** — the prior-run dead-code bug stays fixed.
> README documents all of these correctly (11 CONSISTENT rows). RUNLOG (10/11
> CONSISTENT) and ACCEPTANCE (12 CONSISTENT) match the shipped state.

---

## B. Actionable drift (non-CONSISTENT)

### README.md
| # | Cat | Location | True state | Action |
|---|---|---|---|---|
| R1 | OVERCLAIM | §Kernideen L28 "Nodes: …, Container" | `Node` has no `Container` variant (only Process/File/User/Socket/RemoteHost/Alert, core lib.rs) | **EDIT**: replace "Container" → "Sockets, Remote-Hosts, Alerts" |
| R2 | STALE | L159/300/336 bare `ARCH_VIEWER.md`/`ACCEPTANCE.md` refs | files live under `docs/` | **EDIT**: prefix `docs/` |
| R3 | STALE | L114 "Assets: `assets/audio/*.wav`" | assets are at `crates/spacegraph-viewer/assets/audio/` | **EDIT**: correct path |
| R4 | STALE | §Roadmap L277-282 "v0.2.0 Multi-Node … *future*"; refs `ROADMAP_v0.2.0.md` | multi-node/multi-agent shipped (namespace.rs, settings_agents.rs); canonical roadmap is `docs/ROADMAP.md` | **EDIT**: repoint to `docs/ROADMAP.md`, note multi-node delivered |

### AGENTS.md
| # | Cat | Location | True state | Action |
|---|---|---|---|---|
| AG1 | STALE | §3.1 L88 `v0.1.8 → … → v0.2.0` sequence | tree at v0.4.0; v0.2.0 shipped | **EDIT**: point phase order to `docs/ROADMAP.md` |
| AG2 | STALE | §2.1/§2.2 role version pins "(v0.1.8)"/"(v0.1.9+)" | phases long complete | **EDIT**: make roles phase-agnostic |
| AG3 | OVERCLAIM | §3.3 boundary `graph/ \| Bevy, UI` (darf nicht wissen von) | graph/ uses bevy math + ECS `Resource`/`Res`/`Time` (state.rs/layout.rs/metrics.rs); only **GraphModel** must avoid UI | **EDIT**: reword to match ARCH_VIEWER (GraphModel no UI; graph may use bevy math/ECS-resource types) |
| AG4 | OVERCLAIM | §3.3 boundary `ui/ \| Graph-Interna` | ui/ uses `GraphState` + `graph::model` types by design | **EDIT**: reword to "ui reads via GraphState API, not GraphModel internals; the enforced rule is graph→UI ignorance" |
| AG5 | STALE | L62 "Abhängigkeiten in `ROADMAP_v0.2.0.md`" | superseded by `docs/ROADMAP.md` | **EDIT**: repoint |

### docs/ARCH_VIEWER.md
| # | Cat | Location | True state | Action |
|---|---|---|---|---|
| AV1 | UNDOCUMENTED | render/ lists only spatial/timeline/camera | also node_mesh, edges, theme, freefly, gameplay, pacing, postfx, audio | **EDIT**: add 8 render/ entries |
| AV2 | UNDOCUMENTED | ui/ lists only panel/hud/search/tooltips/help | also inspector, legend, minimap, context_menu, reticle, shortcuts, settings_agents, settings_paths, layout | **EDIT**: add 9 ui/ entries |
| AV3 | UNDOCUMENTED | graph/ misses interner/grid/metrics/tree/synthetic | all present (inventory §1) | **EDIT**: add entries |
| AV4 | UNDOCUMENTED | util/ lists only config/ids | also agent_command.rs | **EDIT**: add entry |
| AV5 | UNDOCUMENTED | net/ has no file list | protocol.rs, uds.rs | **EDIT**: add file list |
| AV6 | STALE | High-level diagram + render/ "Spatial / Timeline" | third mode `ViewMode::Tree` (tree.rs, update_tree_zoom) | **EDIT**: add Tree mode |
| AV7 | STALE | Header L4 "ab v0.1.8" | doc now covers v0.2/v0.3/v0.4 | **EDIT**: bump baseline to v0.4.0 |

### docs/ACCEPTANCE.md
| # | Cat | Location | True state | Action |
|---|---|---|---|---|
| AC1 | STALE | Status-Reconciliation Performance "2.19 ms@2000 / 7.57 ms@5000" | post-pin v0.4.0 re-measure = 2.20 / 8.28 (RUNLOG Phase 6) | **EDIT**: update numbers / cite RUNLOG Phase 6 |
| AC2 | STALE | Status-Reconciliation header "(Stand 2026-06-12, Tag v0.1.11)" | doc covers through v0.4.0 | **EDIT**: bump Stand/Tag to v0.4.0 / 2026-06-13 |
| AC3 | UNDOCUMENTED | no criterion for lane-based timeline (PR #31) | timeline_lane_key + render lane grouping shipped | **FINDING** — add or scope-out a Tree/timeline-lane acceptance criterion |
| AC4 | UNDOCUMENTED | no criterion for Tree view (PRs #29/#30) | ViewMode::Tree + collapse/expand + file-LOD shipped | **FINDING** — add or scope-out a Tree-view criterion |
| AC5 | NAMING | "Pin/Mark/Inspect" prose vs `TogglePin`/`ToggleMark` enum | behaviour described correctly; label nuance only | leave (optional prose tweak) |

### docs/DESIGN_LANGUAGE.md
| # | Cat | Location | True state | Action |
|---|---|---|---|---|
| DL1 | STALE | "Implementation status (Phase 5)" deviation: "edges are HDR gizmos … not yet mesh polylines … deferred" | edges ARE a batched HDR `LineList` mesh now (`render/edges.rs`) | **EDIT**: rewrite the deviation — mesh edges shipped |
| DL2 | STALE | same section heading "(Phase 5)" + "Implemented:" list | rest of doc is v0.4.0; list omits geometry/rings/reticle/post-fx | **EDIT**: retitle to v0.4.0, refresh/trim list |
| DL3 | OVERCLAIM | Themes "Selectable via `cfg.visual_theme` (`viewer.toml` / settings panel)" | **no in-app theme selector** (inventory §3) | **EDIT** "via viewer.toml" + **FINDING** (add selector — design) |
| DL4 | UNDOCUMENTED | node colour table omits Socket | `theme::SOCKET` (blue) is a first-class kind w/ torus geometry | **EDIT**: add Socket row |
| DL5 | UNDOCUMENTED | edge colour table omits owns_socket/connects_to/listens_on/alerts_on | all defined in theme.rs `edge_color` | **EDIT**: extend edge table |
| DL6 | STALE | "(Phase 7)"/"(Phase 8)" parentheticals on HOST/ALERT | historical markers; consts in active use | leave (cosmetic) |

### docs/perf/RUNLOG.md
| # | Cat | Location | True state | Action |
|---|---|---|---|---|
| RL1 | UNDOCUMENTED | Phase 5 Deviations "edges are gizmos … mesh-polyline deferred" (historical entry) | mesh edges later shipped (render/edges.rs) | **EDIT**: append a forward note (don't rewrite the historical entry) |

### docs/ROADMAP.md — record-only (roadmap edits out of scope §1.3)
| # | Cat | Location | True state | Action |
|---|---|---|---|---|
| RM1 | STALE | §1 gap 1 "Every node is one `Sphere::new(0.28)` … colour-only … no in-world interaction" | v0.4.0 shipped per-type geometry + interaction | **leave — record**: operator should rewrite §1 gap 1 post-v0.4.0 |
| RM2 | STALE | §1 "`render/theme.rs`, `GLOW_LEVELS=6`" | `GLOW_LEVELS` lives in `render/spatial.rs`, not theme.rs | **leave — record**: attribute fix |
| RM3 | STALE | §3 v0.4.0 ref `CC_MASTERPROMPT_spacegraph_v0.4.0_node-detail-interaction.md` | file not in repo (external MP) | **leave — record (ambiguous target; §1.3 ambiguity rule)** — operator decides restore/repoint |
| RM4 | STALE | header "Status: v0.2 … " / "Changelog v0.1→v0.2" | roadmap-doc version vs product v0.4.0 (no factual error) | leave — record (clarify doc-version wording) |
| RM5 | STALE | header/§5 `docs/adr/` + ADR-0001+ | dir absent (forward-looking, legitimate pre-v0.6.0) | leave (no fix needed) |

### Implementation-Blueprint.md — original v0.1.8→v0.2.0 vision doc (conservative)
| # | Cat | Location | True state | Action |
|---|---|---|---|---|
| IB1 | STALE | target tree `app/resources.rs # … config load/save` | config load/save in `util/config.rs`; resources.rs is NetRx/NetTx only | leave (vision doc; intent met) |
| IB2 | STALE | v0.2.0 §A `StreamId=u32` | stream id is `String` (net/protocol.rs) — doc offered string as a sanctioned fallback | leave (sanctioned alternative chosen) |
| IB3 | OVERCLAIM | v0.2.0 §B `NetManager`/`Connection` | implemented as `NetState.connections: HashMap<String, ReaderHandle>` | leave (functional goal met, different shape) |
| IB4 | OVERCLAIM | v0.2.0 §C/D per-node graphs + `Gid` keying | single `GraphModel` + string-prefix namespacing (namespace.rs) | leave (string-prefix option won) |
| IB5 | OVERCLAIM | v0.2.0 §D `TimelineEvt` node_key tag | `TimelineEvt{ts,kind}` — origin via id-prefix lookup | leave; **FINDING** if per-stream timeline filtering wanted |
| IB6 | UNDOCUMENTED | doc scope ends at v0.2.0 (no post-FX/reticle/etc.) | correct — later work belongs to later docs | leave |
| IB7 | NAMING | milestone tokens scan | clean, no banned suffixes | leave |

### Secondary docs (ARCHITECTURE.md, GRAPH_SCHEMA.md, ROADMAP_v0.2.0.md)
| # | Cat | Location | True state | Action |
|---|---|---|---|---|
| SD1 | UNDOCUMENTED | `docs/ARCHITECTURE.md` | 1-byte placeholder | **EDIT**: populate (workspace map; pointer to ARCH_VIEWER) |
| SD2 | UNDOCUMENTED | `docs/GRAPH_SCHEMA.md` | 1-byte placeholder | **EDIT**: populate from core types (inventory §4) |
| SD3 | STALE | `docs/ROADMAP_v0.2.0.md` | superseded by `docs/ROADMAP.md` | **ARCHIVE** to `docs/archive/<date>-superseded-roadmap/` + repoint README/AGENTS refs |

---

## C. Phase-3 reconciliation plan (in-scope, doc↔code only)

A. **README**: R1 (Container→real kinds), R2 (`docs/` prefixes), R3 (audio path),
   R4 (roadmap→`docs/ROADMAP.md`, multi-node delivered).
B. **AGENTS.md**: AG1/AG2 (phase order/role pins → roadmap pointer), AG3/AG4
   (boundary cells reworded to enforced reality), AG5 (roadmap ref).
C. **ARCH_VIEWER.md**: AV1-AV5 (complete module lists), AV6 (Tree mode), AV7
   (baseline → v0.4.0).
D. **ACCEPTANCE.md**: AC1 (bench numbers), AC2 (Stand/Tag).
E. **DESIGN_LANGUAGE.md**: DL1/DL2 (mesh edges + retitle), DL3 (theme via toml),
   DL4/DL5 (Socket + network edge colour rows).
F. **RUNLOG.md**: RL1 (append mesh-edges-landed note).
G. **GRAPH_SCHEMA.md**: SD2 (populate from core types).
H. **ARCHITECTURE.md**: SD1 (populate workspace map).
I. **Archive** ROADMAP_v0.2.0.md (SD3) → `docs/archive/`, repoint README/AGENTS.

## D. Findings carried to the report (NOT edited this run)

- **F1** — `visual_theme` has no in-app selector (panel gap; DL3 / inventory §3).
  Standard/Minimal is toml-only despite governing geometry/reticle/rings/post-fx.
  *Rec:* add a theme selector in the v0.5.0 UX-shell.
- **F2/RM1-RM5** — ROADMAP §1 "stands today" is stale post-v0.4.0 (geometry,
  interaction, multi-node) + GLOW_LEVELS attribution + missing v0.4.0 MP file +
  doc-version wording. *Rec:* operator refreshes ROADMAP §1 (roadmap edits are
  out of scope for this recon run).
- **F3 (AC3/AC4)** — ACCEPTANCE lacks criteria for the lane-based timeline (PR#31)
  and Tree view (PRs #29/#30). *Rec:* add or explicitly scope-out criteria.
- **F4 (IB3-IB5)** — Implementation-Blueprint v0.2.0 plan shapes (NetManager,
  ui/connections.rs, per-node graphs, TimelineEvt node_key) diverge from the
  shipped string-prefix + settings_agents design. Vision doc; *rec:* only revisit
  if per-stream solo/telemetry UX is wanted.
