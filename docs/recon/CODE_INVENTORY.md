# SpaceGraph — Ground-Truth Code Inventory

Mechanically derived from the actual source tree at `2a8aa41` (v0.4.0), not from
memory. Produced by a 7-way parallel extraction (fresh-eyes agents reading the
tree) and **cross-verified deterministically** by the recon driver for the two
critical categories:

- **Schedule registration (§2):** the registered-system set was extracted from
  `app/mod.rs`, the system-shaped `pub fn` set by grep, and the diff computed
  independently — agreeing with the agent: **flag list empty**.
- **Config plumbing (§3):** every `ViewerConfig` field independently confirmed
  applied (`apply_viewer_config` + `set_demo_mode`/`sync_agent_endpoints`) and
  serialized (no `#[serde(skip)]`).

---

## 1. Crates & modules

The workspace (`Cargo.toml`) has three crates: `spacegraph-core`,
`spacegraph-agent`, `spacegraph-viewer`.

### spacegraph-core
| module | purpose |
|---|---|
| `src/lib.rs` | Shared wire protocol: `PROTOCOL_VERSION` (=3), `Node`/`Edge`/`EdgeKind`/`FileKind`/`Delta`/`Msg`/`Capabilities`/`NodeIdentity`/`NodeId`, and id constructors (`id_process`/`id_user`/`id_file`/`id_socket`/`id_remote_host`/`id_alert`). |

### spacegraph-agent (`sources/` subdir; no app/graph/render/ui/util)
| module | purpose |
|---|---|
| `src/main.rs` | Entry point; wires config, path_policy, snapshot, sources, server. |
| `src/config.rs` | Manual CLI/config parsing: `AgentMode`, `AgentConfig`, `parse_args`, default include/exclude sets. |
| `src/path_policy.rs` | `PathPolicy` include/exclude (`normalize`/`is_excluded`/`is_included`/`should_watch`). |
| `src/snapshot.rs` | Initial snapshot from `/etc/passwd` + procfs (`build_snapshot`/`parse_passwd`). |
| `src/server.rs` | Async UDS server (`run`): `UnixListener` + `LengthDelimitedCodec`; non-Unix stub. |
| `src/watch_fs.rs` | FS watcher (`spawn`, `notify`) emitting file-node `Delta`s, gated by `PathPolicy`. |
| `src/watch_proc.rs` | Process watcher (`spawn`) polling procfs → process/user/file nodes + edges. |
| `src/sources/mod.rs` | `EventSource` trait + `FsSource`/`ProcSource` (wrap watch_fs/watch_proc). |
| `src/sources/net.rs` | `NetSource`: process↔socket↔remote-host topology from procfs, diff-based. |
| `src/sources/suricata_eve.rs` | `SuricataEveSource`: tails EVE JSON → `Alert` nodes, 5-tuple correlation. |

### spacegraph-viewer (subdirs app/ graph/ net/ render/ ui/ util/)
**roots:** `lib.rs` (library surface for benches/tests), `main.rs` (boot: `--demo-load`, crossbeam net channel, `DefaultPlugins`+`EguiPlugin`+`SpaceGraphViewerPlugin`), `benches/layout.rs` (criterion: `force_step`, `visible_set_capped` @500/1000/2000/5000).

**app/** — `mod.rs` (`SpaceGraphViewerPlugin`: resources/events/systems + net wiring), `events.rs` (`Picked(NodeId)`), `resources.rs` (`NetRx`/`NetTx`).

**graph/** — `mod.rs` (aggregator), `model.rs` (`GraphModel`, `EdgeKindClass`, `AggEdgeKey`/`AggEdge`/`EdgeStats`, upsert/remove, `degree`, `agg_edge`), `state.rs` (`GraphState` + `SpatialState`/`TimelineState`/`UiState`/`CfgState`/`ViewMode`), `interner.rs` (`NodeId`→dense `NodeIndex`, slot reuse), `grid.rs` (uniform grid for neighbour repulsion), `layout.rs` (`update_layout_or_timeline`, `force_step`, `visible_set_capped`, `progressive_prepare`, `apply_tree_layout`), `gc.rs` (`tick_glow`/`tick_gc`), `metrics.rs` (`tick_housekeeping`/`tick_metrics`), `timeline.rs` (timeline model + ticks), `tree.rs` (FS-hierarchy layout), `explain.rs` (`shortest_path` why-connected), `namespace.rs` (multi-stream id prefixing), `synthetic.rs` (seeded demo graph).

**net/** — `mod.rs` (aggregator), `protocol.rs` (`Incoming`/`IncomingKind`), `uds.rs` (`spawn_reader` framed version-checked UDS client + `ReaderHandle`).

**render/** — `mod.rs` (aggregator; `audio` feature-gated), `camera.rs` (`setup_scene` HDR+bloom, `sync_visual_theme`, `apply_jump_to`, `update_tree_zoom`), `spatial.rs` (node entities + rings + drag-select + picking + draw), `edges.rs` (batched HDR `LineList` edges, reused buffers + fingerprint), `node_mesh.rs` (per-type geometry), `theme.rs` (colour source of truth), `freefly.rs` (`V` pilot cam), `gameplay.rs` (scan-pulse + incident-hunt `Mission`), `pacing.rs` (reactive frame pacing, `Last`), `postfx.rs` (cyberspace post-FX, WGSL embedded), `audio.rs` (one-shot cues, `audio` feature), `timeline.rs` (`draw_timeline`).

**ui/** — `mod.rs` (aggregator + `egui_color`), `layout.rs` (`UiLayout` rects), `panel.rs` (`ui_panel` left panel), `hud.rs` (`hud_overlay`), `help.rs` (`help_overlay`), `inspector.rs` (`I`), `legend.rs` (`L`), `minimap.rs`, `context_menu.rs` (radial menu + `CtxAct`/`apply_context_action`), `reticle.rs` (lock-on + micro-tags), `search.rs` (`Ctrl+P`), `shortcuts.rs` (`handle_shortcuts`), `settings_agents.rs` (agent manager/editor/command windows), `settings_paths.rs` (path editor window), `tooltips.rs` (`render_tooltip`).

**util/** — `mod.rs`, `config.rs` (`ViewerConfig`, `PostFxConfig`, `VisualTheme`, `LodEdgesMode`, `AgentEndpoint`, `load_or_default`/`save`), `ids.rs` (label helpers), `agent_command.rs` (`build_agent_command`).

---

## 2. Schedule registration & unregistered-system flags  *(CRITICAL)*

Registration lives in `SpaceGraphViewerPlugin::build` (`app/mod.rs:25-129`) +
`render::PostFxPlugin` (`render/postfx.rs:93-128`). `main.rs` adds only
`DefaultPlugins`, `EguiPlugin`, `PanOrbitCameraPlugin`, `PostFxPlugin`.

- **Startup:** `setup_scene`, `setup_node_render_resources`, `setup_edge_mesh`; `seed_demo_load` (if `--demo-load`) / `auto_connect_agents` (else); `setup_audio` (feature `audio`).
- **Update (group 1):** `process_net_commands`, `pump_network`, `tick_housekeeping`, `handle_shortcuts`, `ui_panel`, `help_overlay`, `hud_overlay`, `inspector_overlay`, `legend_overlay`, `reticle_overlay`, `context_menu_overlay`, `minimap`.
- **Update (group 2):** `hover_detection_spatial`, `picking_focus`, `apply_picked_focus`, `update_tree_zoom`, `sync_visual_theme`, `fly_camera`, `scan_pulse`, `mission_tick`, `reveal_tick`, `rotate_node_rings`, `sync_postfx`.
- **Update (chain, strict order):** `update_layout_or_timeline` → `sync_node_entities` → `sync_node_rings` → `update_edge_mesh` → `draw_scene` → `draw_node_labels` → `apply_jump_to`.
- **Last:** `update_frame_pacing`. **Audio Update (feature):** `audio_triggers`.
- **PostFxPlugin:** render-graph node `ViewNodeRunner<PostFxNode>` in `Core3d` between `Tonemapping` and `EndMainPassPostProcessing` + `ExtractComponentPlugin`/`UniformComponentPlugin` (no `add_systems`).

**Diff vs the 38 system-shaped `pub fn`s:** the only three not directly in an
`add_systems` call are **reachable** (called by a registered system):
- `search_overlay` ← invoked by `ui_panel` (`panel.rs:468`).
- `draw_spatial` / `draw_timeline` ← invoked by `draw_scene` (`render/mod.rs:46-47`).

> **FLAG LIST EMPTY — every system-shaped `pub fn` is registered or called by a
> registered system.** **Anti-regression PASS:** `inspector_overlay` +
> `legend_overlay` are registered (`app/mod.rs:81-82`) — the v0.4.0 dead-code bug
> stays fixed. (Test-only `add_systems` inside `#[cfg(test)]` correctly ignored.)

---

## 3. Config plumbing (4-way)

All 44 `ViewerConfig` fields + the 5 nested `PostFxConfig` fields are in the
struct+`Default` (a), serialized via `toml::to_string_pretty` with **no
`#[serde(skip)]`** (b), and round-tripped through `apply_viewer_config` +
`viewer_config` (d). Column (c) — settings-panel control — has **4 gaps**:

| Field | applied + serialized | panel control | gap |
|---|---|---|---|
| `max_visible_alerts` | yes | **no** | panel-only |
| `repulsion_radius` | yes | **no** | panel-only |
| `layout_budget_ms` | yes | **no** | panel-only |
| `visual_theme` | yes | **no** | panel-only — **notable** |

All four are persisted + reloaded + applied; they are simply not editable from
the in-app panel (only via `viewer.toml`). `max_visible_alerts` / `repulsion_radius`
/ `layout_budget_ms` are internal-tuning (toml-only is defensible). **`visual_theme`
is user-facing** — the Standard/Minimal switch governs geometry, reticle, rings,
post-FX and audio, yet there is no in-app selector (panel even shows "(Standard)"
labels). → carried as a FINDING.

---

## 4. Core types  (`spacegraph-core/src/lib.rs`)

- `PROTOCOL_VERSION: u32 = 3` (L9).
- **`Node`** (6): `Process{pid,ppid,exe,cmdline,uid}`, `File{path,inode,kind}`, `User{uid,name}`, `Socket{proto,local_addr,local_port,state}`, `RemoteHost{addr,rdns}`, `Alert{source,signature,severity,ts}`. `FileKind`: Regular/Dir/Socket/Pipe/Device/Unknown.
- **`Edge`** `{from:NodeId,to:NodeId,kind:EdgeKind}`. **`EdgeKind`** (7): `Opens{fd,mode}`, `Execs`, `RunsAs`, `OwnsSocket`, `ConnectsTo`, `ListensOn`, `AlertsOn`.
- **`Delta`** (6): BatchBegin/BatchEnd/UpsertNode/RemoveNode/UpsertEdge/RemoveEdge.
- **`Msg`** (7): `Hello{version,protocol(#serde default)}`, `Identity{ident,caps}`, `RequestSnapshot`, `Snapshot{nodes,edges}`, `Event{delta}`, `Ping`, `Pong`.
- `Capabilities{procfs,fd_edges,fs_notify,proc_poll,ebpf,cloud,windows:bool}`; `NodeIdentity{node_id,hostname,platform,arch}`.

---

## 5. Agent EventSources & CLI

`EventSource` trait (`sources/mod.rs:24-27`: `name()` + `start(self, node_id, tx)`).
Impls: **`FsSource`** ("fs"), **`ProcSource`** ("proc"), **`NetSource`** ("net"),
**`SuricataEveSource`** ("suricata_eve"). `main` builds `Vec<Box<dyn EventSource>>`
= Fs+Proc always, Net unless `--no-net`, Suricata when `--eve-file` set.

CLI (manual parse, no clap, no `--help`/`--version`; unknown arg → error):
`--include <path>`, `--exclude <path>`, `--no-net`, `--net-poll-secs <n>` (def 2,
≥1), `--net-include <cidr>`, `--net-exclude <cidr>`, `--eve-file <path>`,
`--mode <user|privileged>` (def user), `--uds`/`--socket <path>`.

---

## 6. UI surface & keybindings

Overlays/panels (open-state · keybind · in help): `ui_panel` (always · — · n/a);
`help_overlay` (`help_open` · `?`/`Esc` · yes); `hud_overlay` (always); `inspector_overlay`
(`inspector_open`, default on · `I` · yes); `legend_overlay` (`legend_open` · `L` · yes);
`reticle_overlay` (gated Spatial+Standard); `context_menu_overlay` (`context_menu` ·
right-click node · yes "Context menu"); `minimap` (gated Spatial); `search_overlay`
(`search_open` · `Ctrl+P` · yes); settings windows `path_editor`/`agent_manager`/
`agent_editor`/`agent_command` (button-only, no keybind — acceptable).

Behavioural keys: `F` fly-to, `O` fog, `T` view-cycle, `Space` pause, `V` free-fly,
`G` scan, `M` mission, `WASD/QE/Shift` fly-move, `Esc` clear/close. **All
keybindings are listed in help (`ui/help.rs`).**

> **FLAG: no undocumented keybinding; no orphaned overlay** (every open-state
> field has a setter). Minor info gaps (not keybinding bugs): context-menu action
> list and spatial hover-tooltip behaviour are not enumerated in help.

---

## 7. Tests

Inline `#[cfg(test)]` unit tests only (no `tests/` dirs, no `#[tokio::test]`/
`#[ignore]`/`#[should_panic]`). **Workspace total: 123** — `spacegraph-core` **2**,
`spacegraph-agent` **26**, `spacegraph-viewer` **95**.

- **core (2):** lib.rs — Hello protocol round-trip + legacy default.
- **agent (26):** config 4, path_policy 5, watch_fs 4, sources/net 8, sources/suricata_eve 5.
- **viewer (95):** main 3; graph: explain 2, gc 1, grid 3, interner 3, layout 10, model 2, namespace 3, state 15, synthetic 5, timeline 7, tree 4; render: freefly 1, gameplay 1, node_mesh 2, postfx 3, spatial 15, theme 3; ui: context_menu 3, reticle 3, settings_paths 2; util: agent_command 1, config 3.

> **This inventory is the v0.4.0 baseline snapshot.** Current state (post v0.5.x +
> the auto-safe band) is tracked in `RECON_REPORT-2026-06-14-auto-safe-band.md`.
> Workspace total is now **243** (core 6, agent 44, viewer 190 + 3 main).
>
> **D0 additions (ADR-0012):** `render::spatial::{aperture_style, exposure_bucket,
> ApertureStyle, Exposure}` (+ `NodeRenderResources.socket_aperture`);
> `render::postfx::{select_focus_alerts, severity_weight, MAX_ALERT_FOCUS}` (+ the
> anomaly fields on `PostFxSettings`); `sources/net::{parse_default_gateway,
> gateway_node}`; `SocketDisplayConfig` (`[socket_display]`) wired through
> `ViewerConfig`/`CfgState`. No new `Node`/`EdgeKind`; `PROTOCOL_VERSION` unchanged.
</content>
