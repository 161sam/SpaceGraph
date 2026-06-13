# SpaceGraph Viewer – Architektur

Dieses Dokument beschreibt die **verbindliche Architektur**
des SpaceGraph Viewers (seit v0.1.8; aktueller Stand v0.4.0).

---

## High-Level Übersicht

```

Agent(s)
↓ Events
Net Layer
↓ Incoming (stream-tagged)
Graph Core
↓ Projection (capped)
Render (Spatial / Tree / Timeline)
↓
UI (Panel, HUD, Search, Tooltips, Inspector, Reticle, Context-Menu, …)

```

---

## Modulübersicht

### app/
**Verantwortung:** Bevy Wiring & System Order

- Plugin-Registrierung
- System-Reihenfolge
- Globale Resources & Events

---

### net/
**Verantwortung:** Datenaufnahme

- `uds.rs`: UDS-Client-Reader (`spawn_reader`, framed, version-geprüft) + `ReaderHandle`
- `protocol.rs`: `Incoming` / `IncomingKind` (Connected/Disconnected/Identity/Snapshot/Event/Error)
- (später TCP); Stream-Verwaltung; Protokoll-Normalisierung

Keine Graph-Logik.

---

### graph/
**Verantwortung:** Wahrheit des Systems

#### model.rs
- Nodes, Edges
- Indizes & Aggregation
- Keine UI- oder Renderlogik

#### state.rs
- Orchestriert Substates:
  - `GraphModel`
  - `SpatialState`
  - `TimelineState`
  - `UiState`
  - `PerfState`
  - `CfgState`

#### layout.rs
- Force-Layout
- Progressive Initialisierung

#### timeline.rs
- Event-Ringbuffer
- Zeitabbildung
- Worldline-Lebensdauer

#### explain.rs
- „Why connected?“
- Pfadsuche (BFS, capped)

#### gc.rs
- Orphan Removal
- TTL-Logik

#### Weitere graph/-Module
- `interner.rs`: `NodeId` → dichter `NodeIndex` (Slot-Reuse) für Hot-Paths
- `grid.rs`: Uniform-Grid für Neighbour-Repulsion (kein O(N²))
- `metrics.rs`: Per-Frame-Housekeeping (`tick_housekeeping`/`tick_metrics`)
- `tree.rs`: Dateisystem-Hierarchie-Layout (`ViewMode::Tree`)
- `namespace.rs`: Multi-Stream-Namespacing (Prefix pro Stream)
- `synthetic.rs`: deterministischer Demo-Graph (`--demo-load`)

---

### render/
**Verantwortung:** Darstellung, keine Logik

- `spatial.rs`: Nodes, Picking, Selektion, Orbital-Ringe
- `node_mesh.rs`: per-Typ-Geometrie (Core + Wireframe-Shell)
- `edges.rs`: aggregierte Edges als gebatchtes HDR-`LineList`-Mesh
- `theme.rs`: Farb-Source-of-Truth
- `camera.rs`: Setup (HDR+Bloom), Jump/Focus, `update_tree_zoom`, Theme-Sync
- `freefly.rs`: Free-Fly-Pilotmodus (`V`)
- `gameplay.rs`: Scan-Puls + Incident-Hunt-Mission
- `pacing.rs`: reaktives Frame-Pacing (`Last`)
- `postfx.rs`: Cyberspace-Post-FX (Render-Graph-Node, Standard)
- `timeline.rs`: Timeline-View-Rendering
- `audio.rs`: UI-Soundeffekte (Feature `audio`)

---

### ui/
**Verantwortung:** Interaktion

- `panel.rs`: Sidebar · `hud.rs`: FPS/Counters · `search.rs`: Ctrl+P
- `tooltips.rs`: Shared Tooltip · `help.rs`: Shortcut-Overlay
- `inspector.rs` (`I`) · `legend.rs` (`L`) · `minimap.rs`
- `context_menu.rs`: Radial-Rechtsklick-Menü · `reticle.rs`: Lock-on-Reticle
- `shortcuts.rs`: globale Tastatur-Shortcuts
- `settings_agents.rs` / `settings_paths.rs`: Agent-/Pfad-Editor-Fenster
- `layout.rs`: `UiLayout` (Panel-/Content-Rects)

---

### util/
**Verantwortung:** Infrastruktur

- `config.rs`: `viewer.toml` (`ViewerConfig`/`PostFxConfig`/`VisualTheme`/`AgentEndpoint`)
- `ids.rs`: Labels, Display-Pfade
- `agent_command.rs`: Agent-CLI-Kommando-String-Builder

---

## Interaktions-/Pin-State (v0.4.0)

- **Grab-to-pin** ist *Graph-Wahrheit*, kein Render-State: `SpatialState.pinned:
  Vec<Option<Vec3>>` (slot-indiziert, **keine Bevy-ECS-Typen**, nur `Vec3`-Math),
  API `GraphState::set_pin/clear_pin/is_pinned/pinned_pos`. `force_step` klemmt
  gepinnte Indizes an ihre `Vec3` (überspringt Integration), sie wirken aber
  weiter als Spring-Endpunkte → Determinismus bleibt (getestet mit Pin).
  `release()`/Slot-Reuse löscht den Pin-Slot.
- **UI-Interaktionsstate** (Hover-Edge, Kontextmenü, Marks) lebt in `UiState`
  (`hovered_edge`, `context_menu`, `marked`). Input/Drawing liegt in `render/`
  bzw. `ui/`; das Kontextmenü mutiert über ein deferred `CtxAct`-Mapping
  (`apply_context_action`, unit-getestet).

## Architekturregeln (verbindlich)

1. **Kein Render-Code greift direkt auf Net oder Raw Events zu**
2. **GraphModel kennt keine UI-States**
3. **Timeline & Spatial teilen sich keine Positionsdaten**
4. **Multi-Node nur über Namespacing, nie durch Heuristik**
5. **Capped Sets sind Pflicht für teure Operationen**

---

## Multi-Node Design (v0.2.0)

- Jeder Stream hat `NodeKey`
- Alle IDs sind `(NodeKey, LocalId)`
- Viewer rendert:
  - Einzelgraph
  - oder Projection über aktive Nodes

Kein automatisches Merge ohne Namespace.

### Implementierung (v0.2.0)

- `graph/namespace.rs`: `(NodeKey, LocalId)` wird als **Prefix-String** kodiert
  (`<stream>\u{1}<local>`), siehe `globalize` / `origin` / `local_part`. Ein
  einziges `GraphModel`, keyed by global eindeutigem `NodeId`. (Blueprint
  erlaubt „string prefix" explizit.)
- Namespacing passiert **nur an der Ingest-Grenze** (`GraphState::apply`):
  Snapshots ersetzen genau ihren Stream-Subgraphen (`remove_stream`), Deltas
  werden pro Stream globalisiert. Kein Merge über Streams.
- Per-Stream `enabled`-Flag filtert die Projektion (`stream_enabled` in
  `visible_set_capped`).
- `PROTOCOL_VERSION` (in `spacegraph-core`) wird im `Hello`-Handshake geprüft;
  Mismatch → Ablehnung mit klarer Fehlermeldung.

## Agent: Event-Sources (v0.3.x)

Der Agent sammelt über das `EventSource`-Trait (`agent/src/sources/mod.rs`) —
der **Erweiterungspunkt** für neue Collectors. Jede Source läuft eigenständig
und schreibt `Msg` auf den Broadcast-Bus.

- `FsSource` (fsnotify), `ProcSource` (procfs) — bestehende Collectors hinter
  dem Trait.
- `NetSource` (`sources/net.rs`): procfs `/proc/net/{tcp,tcp6,udp,udp6}` +
  inode→pid (`/proc/<pid>/fd`) → `Socket`/`RemoteHost`-Nodes + Edges
  (`owns_socket`/`listens_on`/`connects_to`). Diff-basiert (nur Änderungen →
  beschränkte Event-Rate), Poll-Intervall konfigurierbar, CIDR-Filter,
  Loopback-Collapse. rDNS ist best-effort (Hook vorhanden).
- `SuricataEveSource` (`sources/suricata_eve.rs`): tailt eine Suricata-EVE-JSON-
  Datei (`--eve-file`), `alert`-Events → `Alert`-Nodes + `alerts_on`-Edge,
  5-Tuple-Korrelation über geteilte `id_remote_host`/`id_socket`-IDs (Hit =
  existierender RemoteHost; Miss = neu angelegt). Viewer cappt Alerts
  (`max_visible_alerts`, älteste evict) und rendert sie immer (LOD-unabhängig).
- eBPF/auditd/Zeek/Falco sind geplante weitere `EventSource`-Implementierungen
  (Erweiterungspunkt dokumentiert, nicht implementiert).
