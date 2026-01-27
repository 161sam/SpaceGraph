# SpaceGraph – Acceptance & Quality Gates

Dieses Dokument definiert, wann ein Release als „fertig“ gilt.

---

## Allgemeine Gates (alle Versionen)

### Code Quality
- `cargo fmt`
- `cargo clippy -D warnings`
- `cargo test`
- Keine `unwrap()` in Renderpfaden

---

### Performance
- Keine O(E)-Scans in Frame-Updates
- Timeline & Layout arbeiten nur auf capped Sets
- Event-Coalescing & Aggregation aktiv

---

### Stabilität
- Kein Panic bei:
  - leerem Graph
  - reconnect
  - schnellem Event-Sturm
- Viewer startet immer mit validen Defaults

---

### UX
- Jeder Modus hat:
  - Exit (Esc)
  - Help (?)
- Tooltips zeigen:
  - Name + ID
  - Kontext („why connected?“)

---

## Versionsspezifische Acceptance

### v0.1.8
- Verhalten identisch zu v0.1.7
- Modularisierung vollständig

---

### v0.1.9
- Explain-Pfad liefert Ergebnis < 50 ms
- Edge-Aggregation reduziert Edge-Anzahl sichtbar

---

### v0.1.10
- Timeline deterministisch bei Pause/Scrub
- Klick auf Event selektiert Node(s)

---

### v0.1.11
- Viewer bedienbar > 2000 Nodes
- Settings persistent

---

### v0.2.0
- Mehrere Streams gleichzeitig
- Keine ID-Kollisionen
- Streams einzeln deaktivierbar
- Tooltips zeigen Node-Origin

---

## Definition „Release-fähig“

Ein Release gilt als fertig, wenn:
- alle Gates erfüllt sind
- kein bekannter Crash reproduzierbar ist
- Architekturregeln eingehalten sind
