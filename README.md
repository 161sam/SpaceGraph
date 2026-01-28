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

```

Agent(s)
↓ Events
Net Layer
↓ normalized Incoming
Graph Core
↓ projection (capped)
Render (Spatial / Timeline)
↓
UI (Panel, HUD, Search, Tooltips)

```

- **Agent** sammelt Systemevents (FS, Prozesse, etc.)
- **Viewer** ist strikt getrennt in:
  - Net
  - Graph (Truth)
  - Render
  - UI

Details: siehe `ARCH_VIEWER.md`.

---

## 🧑‍💻 Lokal starten (Dev-Modus)

SpaceGraph besteht im Dev-Modus aus **Agent** (Event-Quelle) und **Viewer**
(Visualisierung). Starte beide Prozesse in getrennten Terminals:

```bash
# Terminal 1: Agent (Events + UDS-Server)
cargo run -p spacegraph-agent

# Terminal 2: Viewer (Bevy UI)
cargo run -p spacegraph-viewer
```

Optional können beim Agent include/exclude Pfade gesetzt werden (Prefix-Matching, d.h.
`/etc` matcht `/etc` und `/etc/ssh/...`):

```bash
spacegraph-agent --include /etc --include /home/dev --exclude /etc/cni
```

Standardmäßig kommunizieren beide über eine Unix-Domain-Socket unter
`$XDG_RUNTIME_DIR/spacegraph.sock` (falls gesetzt) oder `/tmp/spacegraph.sock`.

---

## 📁 Repository-Struktur (Viewer)

```

crates/spacegraph-viewer/
src/
app/        # Bevy wiring
net/        # event ingestion
graph/      # truth & logic
render/     # spatial/timeline rendering
ui/         # panels, overlays, search
util/       # config, helpers

```

Modularisierung ist **kein Nice-to-have**, sondern Kernbestandteil der Roadmap.

Hinweis: Der Viewer baut standardmäßig ohne Audio-Subsystem (kein ALSA erforderlich). Optional kann Audio über das Feature `audio` aktiviert werden, was u.a. `libasound2-dev` voraussetzt.

---

## 🧪 Tests & Checks

Vor jedem Commit müssen die Quality-Gates laufen:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

---

## 🗺️ Roadmap (kurz)

- **v0.1.x**
  - stabile Spatial + Timeline Views
  - erklärbarer Graph
  - Performance & UX Hardening

- **v0.2.0**
  - Multi-Node Viewer
  - mehrere Agenten gleichzeitig
  - Cluster-/Cloud-ready (ohne Hub)

Details: siehe `ROADMAP_v0.2.0.md`.

---

## 🧪 Qualität & Stabilität

SpaceGraph folgt klaren Qualitäts-Gates:
- keine Panics in Renderpfaden
- keine O(E)-Scans im Frame-Loop
- deterministische Graph-Zustände
- Tests für Timeline, GC, Search, Aggregation

Details: siehe `ACCEPTANCE.md`.

---

## 🤖 Arbeiten mit Agenten (Codex etc.)

Dieses Projekt ist **agentenfähig**, aber **nicht agenten-beliebig**.

- Klare Rollen
- Strikte Architekturgrenzen
- Kleine, reversible Schritte
- Keine impliziten Entscheidungen

Regeln: siehe `AGENTS.md`.

---

## 🚧 Status

SpaceGraph ist **early-stage**, aber **architektonisch ernst gemeint**.

- APIs sind noch nicht stabil
- Fokus liegt auf Korrektheit & Verständnis
- Feedback, Diskussionen & Reviews sind willkommen

---

## 🤝 Mitmachen

Wenn du interessiert bist an:
- Systemvisualisierung
- OS-Interna
- Graphen & Zeitmodelle
- Rust / Bevy / Low-Level Events

… dann schau in:
- `ARCH_VIEWER.md`
- `ROADMAP_v0.2.0.md`
- `AGENTS.md`

und öffne gern ein Issue oder eine Diskussion.

---

## 📜 Lizenz

(TODO – voraussichtlich Open Source, Lizenz folgt)

---

**SpaceGraph**  
*Make system interactions visible.*
