# SpaceGraph

**SpaceGraph** ist ein natives, leichtgewichtiges Visualisierungs- und Analyse-Tool für Unix/Linux-Systeme (später Windows & Cloud), das Systemzustände als **lebendigen Graphen** darstellt.

> Prozesse, Dateien, Nutzer und Ressourcen werden als wechselwirkende Objekte visualisiert –  
> inspiriert von **Feynman-Diagrammen** und dem Unix-Prinzip: *„Everything is a file“*.

---

## ✨ Motivation

Moderne Systeme sind komplex:
- Prozesse öffnen Dateien
- Konfigurationen ändern Verhalten
- Nutzerrechte wirken indirekt
- Cloud- & Cluster-Setups vervielfachen Abhängigkeiten

**SpaceGraph macht diese Wechselwirkungen sichtbar** – nicht als Logfile oder Tabelle, sondern als **räumlich-zeitlichen Graphen**, der sich live verändert.

Ziel ist **Verständnis**, nicht nur Monitoring.

---

## 🧠 Kernideen

- **Graph statt Baum**  
  Kein klassischer Prozessbaum, sondern ein gerichteter Multi-Graph:
  - Nodes: Dateien, Prozesse, User, Hosts, Container
  - Edges: `opens`, `execs`, `runs_as`, …

- **Zeit als Dimension**  
  Neben einer räumlichen Ansicht gibt es einen **Timeline / Feynman Mode**:
  - Worldlines pro Objekt
  - Events als Vertices
  - Ursache–Wirkung sichtbar über Zeit

- **Live + erklärbar**
  - Änderungen erscheinen sofort
  - Tooltips beantworten: *„Warum ist das verbunden?“*

- **Schlank & lokal**
  - Native Viewer (Rust + Bevy)
  - Kein Browser, kein schweres Backend
  - Läuft lokal, später auch verteilt

---

## 🖥️ Features (aktueller Stand)

### Spatial View
- 2D/3D Graphansicht
- Force-Directed Layout
- Focus Mode (N-Hop Subgraph)
- Hover-Tooltips mit Kontext
- „Why connected?“ Erklärung
- Glow bei neuen/aktuellen Events

### Timeline / Feynman Mode
- Zeitachse (Vergangenheit → Jetzt)
- Worldlines für Nodes
- Event-Vertices (Node/Edge Upsert/Remove)
- Hover-Tooltips mit Event-Details
- Pause & Replay (Scrub)
- Klick auf Event → Auswahl / Jump

### UX & Analyse
- Ctrl+P Search & Jump
- HUD (FPS, Eventrate, Visible Nodes)
- Filter (Substring)
- Konfigurierbare Caps & Performance-Grenzen

---

## 🧩 Architektur (Kurzfassung)

