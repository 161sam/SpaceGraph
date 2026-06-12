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
- **Game-Navigation:** Orbit/Pan/Zoom-Kamera (bevy_panorbit_camera) —
  Rechts-Drag = Orbit, Mittel-Drag = Pan, Scroll = Zoom, Linksklick = Auswahl,
  `F` = sanftes Fly-to/Lock-on; Hover-/Selektions-Highlight-Bubbles
- Focus Mode (N-Hop Subgraph)
- Hover-Tooltips mit Kontext
- „Why connected?“ Erklärung
- Glow bei neuen/aktuellen Events
- **Edges als Mesh-Polylines** (ein gebatchter `LineList` mit Per-Vertex-HDR-
  Farben) → volle Bloom-Teilnahme im Standard/Neon-Theme; LOD-Edge-Modus
  (Off/Focus/All) bleibt wirksam

### Timeline / Feynman Mode
- Zeitachse (Vergangenheit → Jetzt)
- Worldlines für Nodes
- Event-Vertices (Node/Edge Upsert/Remove)
- Hover-Tooltips mit Event-Details
- Pause & Replay (Scrub)
- Klick auf Event → Auswahl / Jump

### Netzwerk-Layer (v0.3.x)
- Agent-Source `net` (procfs `/proc/net/{tcp,tcp6,udp,udp6}` + inode→pid):
  Prozess → Socket → RemoteHost-Topologie
- Diff-basiert (nur Änderungen → beschränkte Event-Rate), Poll-Intervall,
  CIDR-Filter (`--net-include`/`--net-exclude`), Loopback-Collapse

### Threat-Viz (v0.3.x)
- Suricata-EVE-Ingestion (`--eve-file`): `alert`-Events → `Alert`-Nodes,
  5-Tuple-Korrelation zu RemoteHost/Socket (geteilte IDs)
- Severity-Farben (low=amber, medium=orange, high=rot), Alerts immer sichtbar
  (cap `max_visible_alerts`, älteste evict), Alerts-Panel (Counts + Jump)

### Gameplay / Exploration
- **Free-Fly „Pilot"-Modus** (`V`): WASD/QE bewegen, Maus-Look, Shift = Boost
- **Box-Select** (Links-Drag) für Multi-Auswahl; tiefengenaues Ray-Picking
- **Scan-Puls** (`G`): expandierende Welle vom Kamerastandpunkt lässt
  getroffene Nodes aufglühen (aktives Erkunden)
- **Incident-Hunt** (`M`): wähle den alarmierten Host → Score (schneller = mehr),
  Anzeige im „Incident Hunt"-Panel
- **Minimap/Radar** (Top-Down-Overlay) mit Kamera-Marker
- **Fog-of-war** (`O`): Layout/Placement laufen auf der vollen Projektion, aber
  nur erkundete Nodes werden gerendert. Reveal durch Annähern der Kamera,
  Scan-Puls oder Fly-to; Alarme bleiben immer sichtbar. Default: aus.
- _Geplant:_ Audio (Hum/Blip/Klaxon — `audio`-Feature vorhanden, braucht Assets)

### UX & Analyse
- Ctrl+P Search & Jump
- HUD (FPS, Eventrate, Visible Nodes)
- Filter (Substring)
- Konfigurierbare Caps & Performance-Grenzen

### Agent Event-Sources (Erweiterungspunkt)
Collectors implementieren das `EventSource`-Trait (`agent/src/sources/`):
`fs`, `proc`, `net`, `suricata_eve`. **Geplant** als weitere
`EventSource`-Implementierungen: eBPF, auditd, Zeek, Falco (Erweiterungspunkt
dokumentiert, noch nicht implementiert).

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
`/run/user/$(id -u)/spacegraph.sock` (falls verfügbar) oder `/tmp/spacegraph.sock`.

Zum Testen ohne Agent seedet der Viewer mit `--demo-load <n>` einen
deterministischen synthetischen Graphen (`n` Nodes, ~`2n` Edges) statt sich zu
verbinden — nützlich für Performance- und Layout-Smoke-Tests:

```bash
cargo run -p spacegraph-viewer -- --demo-load 2000
```

### ⚙️ Viewer-Defaults (Erststart)

Beim ersten Start (ohne `viewer.toml`) gelten diese Defaults; alle sind im
Settings-Panel bzw. in `viewer.toml` änderbar:

| Setting | Default | Bedeutung |
|---|---:|---|
| `max_visible_nodes` | 3000 | Cap der sichtbaren Nodes (konnektivitätserhaltend, behält Edge-Endpunkte) |
| `lod_threshold_nodes` | 2500 | Ab hier Level-of-Detail: Punkte statt voller Spheres |
| `show_edges` / `show_agg_edges` | an / an | Edges sind by default sichtbar (aggregiert) |
| `show_raw_edges` | aus | Roh-Edges nur auf Wunsch/Focus |
| `repulsion` / `repulsion_radius` | 400 / 8 | Force-Layout: Spread und Repulsion-Cutoff (Grid-Zellgröße) |
| `layout_budget_ms` | 6 | Per-Frame-Zeitbudget fürs Layout (Pass über Frames teilbar) |
| `link_distance` | 6 | Ziel-Kantenlänge (Spring-Ruhelänge) |
| `lod_enabled` / `layout_force` | an / an | LOD bzw. Force-Layout aktiv |
| `visual_theme` | `standard` | `standard` = Neon/HDR+Bloom („Ghost in the Shell"), `minimal` = flach (Accessibility/Perf) |

Die Farb-/Design-Sprache ist in `docs/DESIGN_LANGUAGE.md` festgelegt
(Quelle der Wahrheit für Farben: `render/theme.rs`).

So sieht ein Erststart einen sinnvollen, gespreizten Graphen, Edges inklusive;
LOD greift erst bei großen Graphen (> 2500 sichtbare Nodes).

### ✅ Diagnose: Agent-UDS prüfen

```bash
ss -xlpn | rg spacegraph
```

```bash
ls -la /run/user/$(id -u) | rg spacegraph
```

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
