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

#### Layout-Benchmarks (benchmark-enforced, ab v0.1.x Perf-Arbeit)

Gemessen via `cargo bench -p spacegraph-viewer` (criterion, bench-Profil) gegen
deterministische synthetische Graphen. Maschinen-Specs und Baseline-Zahlen in
`docs/perf/BASELINE.md`, Verlauf in `docs/perf/RUNLOG.md`.

- `force_step` bei **2000 Nodes < 4 ms** pro Step.
- `force_step` bei **5000 Nodes < 12 ms** pro Step.
- Layout ist deterministisch: gleiche Seeds → identische Positionen nach K
  Steps (`force_step_is_deterministic`).
- Repulsion ist neighbour-only über ein Uniform-Grid (kein O(N²) im Frame).
- Per-Frame Layout-Budget (`layout_budget_ms`, Default 6 ms); ein Pass wird bei
  Überschreitung über Frames fortgesetzt (Cursor), ohne das Ergebnis zu ändern.

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

## Status-Reconciliation (Stand 2026-06-12, Tag v0.1.11)

Abgleich der Gates mit dem tatsächlichen Stand nach der Perf-/Hardening-Arbeit
(Phasen 0–4). „Automatisch verifiziert“ = via `cargo test`/`cargo bench`/
`cargo clippy`; „lokal/GPU“ = braucht laufende App mit Display.

- **Code Quality** — ✓ automatisch verifiziert (`fmt`, `clippy -D warnings`,
  `test`; keine `unwrap()` in `render/`).
- **Performance** — ✓ benchmark-enforced: `force_step` 2.19 ms @2000 / 7.57 ms
  @5000; neighbour-only Grid (kein O(N²)/O(E_total) im Frame); Layout +
  Render arbeiten auf capped Sets; Edge-Aggregation aktiv.
- **v0.1.8** — ✓ Module vollständig (`app/ net/ graph/ render/ ui/ util/`).
- **v0.1.9** — ✓ Indizes + Aggregation; Explain-BFS (Cap) liefert Pfade.
- **v0.1.10** — ✓ Timeline-Determinismus-Tests; Klick-Selektion vorhanden.
- **v0.1.11** — Viewer > 2000 Nodes: ✓ (Grid + persistente Entities, Steady
  State ohne Entity-Churn — Headless-Test). Settings persistent: ✓ (Roundtrip-
  Test). **FPS-Gate (≥60 @2000 / ≥30 @5000) lokal/GPU zu bestätigen** (Umgebung
  headless) — die strukturellen Garantien dafür sind getestet.
- **v0.2.0** — offen (Phase 6).

Die `force_step`-Baseline-/Verlaufszahlen stehen in `docs/perf/BASELINE.md`
und `docs/perf/RUNLOG.md`.

---

## Definition „Release-fähig“

Ein Release gilt als fertig, wenn:
- alle Gates erfüllt sind
- kein bekannter Crash reproduzierbar ist
- Architekturregeln eingehalten sind
