# SpaceGraph Viewer – Architektur

Dieses Dokument beschreibt die **verbindliche Architektur**
des SpaceGraph Viewers ab v0.1.8.

---

## High-Level Übersicht

```

Agent(s)
↓ Events
Net Layer
↓ Incoming (stream-tagged)
Graph Core
↓ Projection (capped)
Render (Spatial / Timeline)
↓
UI (Panel, HUD, Search, Tooltips)

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

- UDS / später TCP
- Stream-Verwaltung
- Protokoll-Normalisierung

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

---

### render/
**Verantwortung:** Darstellung, keine Logik

- spatial.rs: Nodes, Edges, Picking
- timeline.rs: Worldlines, Events, Hover
- camera.rs: Jump / Focus

---

### ui/
**Verantwortung:** Interaktion

- panel.rs: Sidebar
- hud.rs: FPS, Counters
- search.rs: Ctrl+P
- tooltips.rs: Shared Tooltip Rendering
- help.rs: Shortcut Overlay

---

### util/
**Verantwortung:** Infrastruktur

- config.rs: viewer.toml
- ids.rs: Labels, Stable Hashes

---

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
