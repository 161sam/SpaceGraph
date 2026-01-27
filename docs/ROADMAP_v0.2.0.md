# SpaceGraph – Roadmap bis v0.2.0

Diese Roadmap definiert die verbindliche Entwicklung von **v0.1.8 bis v0.2.0**.
Ziel ist ein stabiler, wahrheitsgetreuer, modularer Viewer mit Multi-Node-Fähigkeit.

---

## Leitprinzipien

1. **Truthiness vor Optik**
   - Graph muss erklärbar, konsistent und deterministisch sein.
2. **Performance by Design**
   - Keine O(E)-Scans in Hotpaths.
   - Alle teuren Operationen nur auf capped Visible-Sets.
3. **Architektur zuerst**
   - Modularisierung vor neuen Großfeatures.
4. **Viewer-first**
   - Multi-Node zuerst im Viewer, kein Hub bis v0.3.x.

---

## v0.1.8 – Refactor Foundation (Move-only)

### Ziel
Codebasis modularisieren, ohne Verhalten zu ändern.

### Deliverables
- Aufteilung von `main.rs` in:
  - `app/`, `graph/`, `render/`, `ui/`, `net/`, `util/`
- Einführung von `GraphState` mit Substates
- Keine neuen Features
- Erste Unit-Tests (Timeline, GC, Search)

### Akzeptanz
- Verhalten identisch zu v0.1.7
- `cargo fmt`, `cargo clippy -D warnings`, `cargo test` grün

---

## v0.1.9 – Truth Graph Core

### Ziel
Graph wird schnell, stabil und erklärbar.

### Deliverables
- Adjazenz-Indizes (keine Edge-Scans mehr)
- Edge-Aggregation mit Stats
- „Why connected?“ via BFS (max depth, capped)
- Echte Node-Labels in Tooltips

### Akzeptanz
- Edge-Anzahl sinkt bei Event-Stürmen messbar
- Explain-Pfad < 50 ms bei capped Graph

---

## v0.1.10 – Timeline „Feynman-Grade“

### Ziel
Timeline wird ein echtes Analysewerkzeug.

### Deliverables
- Worldlines mit Lebensdauer
- Event-Icons + Batch-Bänder
- Pause + Scrub (Replay)
- Klick auf Event → Select → Jump to Spatial

### Akzeptanz
- Timeline deterministisch bei Pause/Scrub
- Tooltips zeigen echte Metadaten

---

## v0.1.11 – UX & Performance Hardening

### Ziel
Viewer fühlt sich stabil und fertig an.

### Deliverables
- LOD (Points statt Meshes bei großen Graphen)
- Konsistente Shortcuts
- Persistente Viewer-Config (`viewer.toml`)
- Help-Overlay
- Panic-freie Renderpfade

### Akzeptanz
- Viewer bleibt bedienbar >2000 Nodes
- Start/Stop immer ohne Fehler

---

## v0.2.0 – Multi-Node Viewer

### Ziel
Cluster- & Cloud-ready Viewer ohne Hub.

### Deliverables
- Multi-Stream Net-Layer
- Node/Stream-Namespace
- Connections Panel
- Merged Projection View
- Timeline mit Node-Origin

### Akzeptanz
- Mehrere Agenten parallel
- Keine ID-Kollisionen
- Streams einzeln aktivierbar/deaktivierbar
