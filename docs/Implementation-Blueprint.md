# Repo-Blueprint: Zielstruktur (Viewer)

```
crates/spacegraph-viewer/src/
  main.rs                     # nur boot + plugin add + setup camera/light
  app/
    mod.rs                    # register systems, ordering
    resources.rs              # NetRx, config load/save, handles
    events.rs                 # bevy events: Picked, etc.
  net/
    mod.rs                    # net public API
    uds.rs                    # current UDS reader (spawn_reader)
    protocol.rs               # Incoming + stream tagging + handshake parse
  graph/
    mod.rs
    model.rs                  # GraphModel: nodes/edges + indices + aggregation
    state.rs                  # GraphState wrapper (substates)
    layout.rs                 # force layout + progressive init
    gc.rs                     # orphan gc, ttl, last_seen
    metrics.rs                # fps/event_rate + counters
    timeline.rs               # ringbuffer + mapping + hover selection
    explain.rs                # why-connected path + label helpers
  render/
    mod.rs
    spatial.rs                # draw nodes/edges + spatial tooltips
    timeline.rs               # worldlines + events + timeline tooltips
    camera.rs                 # jump/focus helpers
  ui/
    mod.rs
    panel.rs                  # left panel
    hud.rs                    # hud overlay
    search.rs                 # ctrl+p overlay + model
    help.rs                   # shortcuts overlay
    tooltips.rs               # shared tooltip render helpers
  util/
    mod.rs
    ids.rs                    # stable hash helpers, node labels
    config.rs                 # viewer.toml load/save
```

---

# v0.1.8 — Move-only Refactor + State split (no behavior change)

## Ziel

* `main.rs` wird klein (Boot + Wiring).
* Logik verteilt in Module, aber **funktional identisch**.

## Arbeitspakete

### A) `main.rs` -> minimal boot

**Neu/Update:**

* `src/main.rs`

  * enthält nur:

    * `mod app;`
    * `App::new().add_plugins(...).add_plugins(EguiPlugin).add_systems(Startup, setup).add_plugins(app::SpaceGraphViewerPlugin)`
    * `setup()` (camera/light)
* `src/app/mod.rs`

  * `pub struct SpaceGraphViewerPlugin; impl Plugin for ...`
  * registriert Systems in sinnvoller Order:

    1. net pump
    2. ticks
    3. ui
    4. input (picking)
    5. update layout/timeline
    6. render

### B) Net entkoppeln

**Neu:**

* `src/net/mod.rs` re-export
* `src/net/uds.rs`

  * enthält bisherigen `spawn_reader(sock_path, tx)` Code
* `src/net/protocol.rs`

  * Typen:

    * `pub enum Incoming { ... }` (wie jetzt)
    * (noch ohne stream_id; kommt v0.2.0)

### C) GraphState split (aber intern noch “thin”)

**Neu:**

* `src/graph/state.rs`

  * `pub struct GraphState { pub model, pub spatial, pub timeline, pub ui, pub perf, pub cfg, needs_redraw }`
  * Methoden, die bisher in state.rs sind, bleiben API-kompatibel durch delegieren.
* `src/graph/model.rs`

  * vorerst: nodes/edges (ohne neue indices)
* `src/graph/layout.rs`, `gc.rs`, `metrics.rs`, `timeline.rs`

  * verschieben bestehende Funktionen rein (noch ohne neue features)

### D) Render & UI split

**Neu:**

* `src/render/spatial.rs`

  * `draw_spatial(...)` aus `main.rs`
* `src/render/timeline.rs`

  * `draw_timeline(...)` aus `main.rs`
* `src/render/camera.rs`

  * `apply_jump_to(...)`
* `src/ui/panel.rs`, `hud.rs`, `search.rs`, `tooltips.rs`

  * `ui_panel`, `hud_overlay`, `search_overlay`, tooltip rendering

### E) Tests (MVP)

**Neu:**

* `crates/spacegraph-viewer/tests/` oder `src/graph/*` `#[cfg(test)]`

  * `timeline_trims()`
  * `timeline_caps()`
  * `gc_orphan_removal()`
  * `search_stable_hits()`

## Dateien geändert/neu (v0.1.8)

* NEW: `src/app/*`, `src/net/*`, `src/graph/*`, `src/render/*`, `src/ui/*`, `src/util/*`
* UPDATE: `src/main.rs`
* UPDATE/RENAME: `src/state.rs` -> aufgeteilt, dann löschen/leer lassen

## DoD v0.1.8

* Verhalten unverändert (spatial+timeline laufen)
* `cargo fmt && cargo clippy -D warnings && cargo test` grün

---

# v0.1.9 — Indices + Aggregation + Explain (Truth Core)

## Ziel

* keine Edge-Scans in Hotpaths
* Kantenexplosion stoppt
* why-connected zeigt Pfadkette

## Arbeitspakete

### A) GraphModel Indizes

**Update: `src/graph/model.rs`**

* Datenstrukturen:

  * `nodes: HashMap<NodeId, Node>`
  * `edges_raw: Vec<Edge>` (oder HashSet + stable vec)
  * `adj: HashMap<NodeId, SmallVec<[EdgeRef; 8]>>` (EdgeRef = index+direction)
* API:

  * `neighbors(id) -> impl Iterator<Item=NodeId>`
  * `edges_for_node(id) -> impl Iterator<Item=&Edge>`

### B) Edge Aggregation

**Update: `src/graph/model.rs`**

* `EdgeKey { from, to, kind_class }`
* `AggEdge { from, to, kind_class, stats: EdgeStats, last_kind_payload }`
* `AggStore: HashMap<EdgeKey, AggEdge>`
* Toggle in cfg: `cfg.show_raw_edges: bool`

**Update: `src/graph/state.rs`**

* `apply_delta()` schreibt in raw + aggregated

### C) Explain: why-connected BFS

**Neu: `src/graph/explain.rs`**

* `fn shortest_path(model, a, b, max_depth, vis_set) -> Option<Vec<PathStep>>`
* `PathStep { from, to, edge_kind_class }`
* Tooltip nutzt:

  * Node labels: file path / cmdline / user name
  * Edge explanation: opens/execs/runs_as

### D) Tooltips “real labels”

**Update: `util/ids.rs`**

* `fn node_label(node: &Node) -> String` (short)
* `fn node_label_long(node: &Node) -> Vec<String>`

### E) Tests v0.1.9

* adjacency correctness
* aggregation merges correctly
* BFS path returns expected chain (small synthetic graph)

## DoD v0.1.9

* HUD zeigt deutlich weniger edges (bei edit storms)
* „Why connected?“ liefert Pfad (wenn vorhanden) < 50ms bei cap

---

# v0.1.10 — Timeline „Feynman-Grade“ (scrub, lifespan, batch bands)

## Ziel

* Timeline ist ein echtes Analyse-Tool (Pause+Scrub+Click+Jump)

## Arbeitspakete

### A) TimelineState erweitert

**Update: `src/graph/timeline.rs`**

* `TimelineState { events: VecDeque<TimelineEvt>, pause, frozen_now, window, scale, scrub: f32 }`
* `timeline_now()` = frozen_now - scrub_offset
* `Scrub slider`: `scrub_seconds` 0..window

### B) Worldline Lifespan

**Update: `src/graph/timeline.rs`**

* `NodeLife { first_seen, last_seen, removed_at: Option<Instant> }`
* On `UpsertNode`: set first_seen if not exists, update last_seen
* On `RemoveNode`: set removed_at
* Worldline draw uses `[max(first_seen, now-window) .. min(removed_at/now, now)]`

### C) Batch visualization

**Update: timeline**

* Store `BatchSpan { id, start_ts, end_ts }` in timeline state (ringbuffer)
* Render as band (rect or two vertical lines) in `render/timeline.rs`

### D) Timeline click-select + jump

**Update: `render/timeline.rs`**

* hover picking already: add click detection on nearest event point
* set `ui.selected_a`, `ui.selected_b` (two-node selection)
* add UI button "Jump to Spatial" (switch mode + focus selected)

### E) Tests v0.1.10

* scrub time mapping correct
* lifespan worldline interval correct
* batch span open/close correct

## DoD v0.1.10

* Scrub funktioniert deterministisch
* Klick Event → Auswahl sichtbar → Jump nach Spatial möglich

---

# v0.1.11 — UX & Perf Hardening (LOD, settings, help, crashproof)

## Ziel

* fühlt sich „fertig“ an, bleibt performant, keine Panics

## Arbeitspakete

### A) LOD/Instancing

**Update: `render/spatial.rs`**

* if `visible_nodes > lod_threshold`:

  * render nodes as points/gizmos
  * edges optional: only for focus/selected
* optional: bevy instanced meshes (später), MVP: points

### B) Persist Settings

**Neu: `util/config.rs`**

* `ViewerConfig { caps, view_mode, timeline_window, toggles, lod_threshold }`
* load on startup, save on exit or on “Apply” button

### C) Shortcuts + Help Overlay

**Neu/Update: `ui/help.rs`, `ui/mod.rs`**

* `?` toggles help overlay
* consistent shortcuts (Esc/F/Space/T)

### D) Robustness

* handle “empty graph” everywhere
* handle “no camera transform” gracefully
* avoid `unwrap()` in render paths

### E) Tests v0.1.11

* config serialize/deserialize
* lod mode toggling doesn’t panic

## DoD v0.1.11

* Viewer startet/stopt sauber mit config
* UI bleibt bedienbar bei >2000 nodes

---

# v0.2.0 — Multi-Node Streams (Cluster-ready in Viewer)

## Ziel

* mehrere Agenten gleichzeitig, Namespacing + Connections UI + merge projection

## Arbeitspakete

### A) Stream IDs & Node identity

**Update: `net/protocol.rs`**

* `type StreamId = u32`
* `Incoming { stream: StreamId, msg: Msg }`
* Handshake:

  * Agent sendet `Identity { node_id, host, os, arch, version }` (wenn im core existiert; sonst fallback: stream-based node key)

> Falls Agent aktuell keine Identity liefert: v0.2.0 MVP kann `node_key = "stream-<id>"` nutzen, aber Architektur so bauen, dass echte Identity später drop-in ist.

### B) Net: multi-connection support

**Update: `net/mod.rs` & `net/uds.rs`**

* `spawn_reader(path, stream_id, tx)`
* `NetManager` resource:

  * `connections: Vec<Connection { stream_id, label, status, last_seen }>`
  * `add_connection(...)`

### C) Graph: namespacing

**Update: `graph/model.rs`**

* `GlobalId { node: NodeKey, local: NodeId }` ODER string prefix
* Empfehlung: intern struct:

  * `struct NodeKey(String);`
  * `struct Gid { node: NodeKey, local: NodeId }`
* Alle Maps keyed by `Gid`, nicht mehr `NodeId`.

### D) Graph: per-node graphs + projection

**Update: `graph/state.rs`**

* `graphs: HashMap<NodeKey, GraphModel>`
* `projection: ProjectionState`

  * `active_nodes: HashSet<NodeKey>` (from UI toggles)
  * `visible_set_capped()` arbeitet auf projection
* Timeline:

  * merged events with `node_key` tag in `TimelineEvt`

### E) UI: Connections panel

**Neu: `ui/connections.rs`**

* list streams, toggles, solo
* display event rate per stream

### F) Tests v0.2.0

* namespacing collision test:

  * same local pid on two nodes doesn’t collide
* projection filter respects enabled nodes
* multi-stream incoming routes to correct graph

## DoD v0.2.0

* zwei UDS connections parallel möglich (oder simulated)
* disable one stream → graph shrinks, no crash
* tooltips show node origin (node_key)

---

# Sequenz & Abhängigkeiten (damit’s „richtig“ ist)

1. **v0.1.8** muss zuerst (sonst refactor+features parallel = pain)
2. **v0.1.9** indices+aggregation (sonst timeline/multi-node wird langsam)
3. **v0.1.10** timeline upgrade (nutzt explain/labels/indices)
4. **v0.1.11** UX/perf hardening (stabilisiert vor multi-node)
5. **v0.2.0** multi-node (baut auf modularer net/graph/ui)
