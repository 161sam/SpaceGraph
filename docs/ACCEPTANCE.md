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

## Status-Reconciliation (Stand 2026-06-13, Tags bis v0.4.0)

Abgleich der Gates mit dem tatsächlichen Stand nach der Perf-/Hardening-Arbeit
(Phasen 0–4). „Automatisch verifiziert“ = via `cargo test`/`cargo bench`/
`cargo clippy`; „lokal/GPU“ = braucht laufende App mit Display.

- **Code Quality** — ✓ automatisch verifiziert (`fmt`, `clippy -D warnings`,
  `test`; keine `unwrap()` in `render/`).
- **Performance** — ✓ benchmark-enforced: `force_step` **2.20 ms @2000 / 8.28 ms
  @5000** (v0.4.0-Re-Messung nach dem Pin-Clamp; siehe `docs/perf/RUNLOG.md`
  Phase 6 — Ausgangsbaseline war 2.19 / 7.57); neighbour-only Grid (kein
  O(N²)/O(E_total) im Frame); Layout + Render arbeiten auf capped Sets;
  Edge-Aggregation aktiv.
- **v0.1.8** — ✓ Module vollständig (`app/ net/ graph/ render/ ui/ util/`).
- **v0.1.9** — ✓ Indizes + Aggregation; Explain-BFS (Cap) liefert Pfade.
- **v0.1.10** — ✓ Timeline-Determinismus-Tests; Klick-Selektion vorhanden.
- **v0.1.11** — Viewer > 2000 Nodes: ✓ (Grid + persistente Entities, Steady
  State ohne Entity-Churn — Headless-Test). Settings persistent: ✓ (Roundtrip-
  Test). **FPS-Gate (≥60 @2000 / ≥30 @5000) lokal/GPU zu bestätigen** (Umgebung
  headless) — die strukturellen Garantien dafür sind getestet.
- **v0.2.0** — ✓ Multi-Node: Stream-Namespacing per Prefix (keine ID-Kollision,
  kein Auto-Merge — getestet), per-Stream Snapshot-Replace, Streams einzeln
  deaktivierbar (`enabled`-Flag, Headless-Test), Tooltips zeigen Node-Origin,
  `PROTOCOL_VERSION`-Handshake (Agent/Viewer lehnen Mismatch ab). Zwei-Agenten-
  Livebetrieb lokal zu bestätigen. Tag `v0.2.0`.

### v0.3.x — Network layer & Threat-Viz

- **Phase 7 (Network)** — ✓ `EventSource`-Trait als Erweiterungspunkt;
  `NetSource` parst procfs zu Socket/RemoteHost-Topologie; **Diff-basiert →
  beschränkte Event-Rate** (Test: stabiler Graph ⇒ 0 Deltas); CIDR-Filter,
  Loopback-Collapse. Pure-Parsing/Build/Diff getestet (Fixture). Live-Demo +
  Steady-State < 5 Events/s lokal zu bestätigen.
- **Phase 8 (Threat-Viz)** — ✓ `Alert`-Node + `alerts_on`-Edge; Suricata-EVE-
  Source (`--eve-file`), 5-Tuple-Korrelation (Hit/Miss getestet), Severity→Farbe,
  Alert-Cap-Eviction (getestet), Alerts immer sichtbar (LOD-unabhängig),
  Alerts-Panel. `PROTOCOL_VERSION`=3. Tag `v0.3.0-alpha.1`. EVE-Replay-Demo +
  Screenshot lokal zu erstellen (`docs/media/`).
- **Teilweise/Deferred:** Timeline-Alert-Vertices erscheinen als NodeUpsert-
  Events (rote Einfärbung pro Node noch offen); Mesh-Edges (statt Gizmos);
  rDNS-Lookups (Hook vorhanden).

Die `force_step`-Baseline-/Verlaufszahlen stehen in `docs/perf/BASELINE.md`
und `docs/perf/RUNLOG.md`.

### v0.4.0 — Node Detail & In-World Interaction

Strukturelle Gates (headless via `cargo test`/`naga`/`clippy` automatisch
verifiziert; FPS/Pixel-Optik **lokal/GPU** per Capture-Anleitung in
`docs/perf/RUNLOG.md`, da Umgebung headless):

- **Per-Typ-Geometrie** — ✓ jede `NodeKind` spawnt mit ihrem `core_mesh`
  (Handle-Gleichheit getestet); Shell-Child nur für RemoteHost/Alert (Standard);
  Theme-Switch = genau **ein** Entity-Rebuild; Steady State ohne Entity-Churn;
  Minimal nutzt die flache Kugel.
- **Lock-on-Reticle** — ✓ `highlight_style(theme)` pure-fn (Standard=Reticle,
  Minimal=Bubbles); Micro-Tags gecappt (`nearest_micro_tags`).
- **Orbital-Ringe** — ✓ Qualifikation per `degree ≥ ring_min_degree || Alert`
  (O(1)-Adjazenz); je ein `RingMarker`-Child, kein Steady-State-Churn; Minimal
  ohne Ringe.
- **Interaktion** — ✓ Pin-State ist reine Graph-Wahrheit (kein Bevy-ECS-Typ);
  `force_step` hält Pins fest **und deterministisch** (zwei Läufe identisch);
  Slot-Reuse löscht den Pin; `ray_segment_dist` Hit/Miss; Kontextmenü-Mapping
  getestet.
- **Post-FX** — ✓ WGSL validiert via `naga` (`wgsl_postfx_validates`);
  `PostFxPlugin` baut headless ohne Panic; Minimal erzwingt aus
  (`postfx_active`); Config-Roundtrip. Render-Graph-Pfad zusätzlich live auf
  Vulkan ohne wgpu-Validation-Error verifiziert.
- **Modulgrenzen** — ✓ neuer Visual-/Interaktionsstate in `render/`/`ui/`,
  Pin-State als Plain-Data in `graph/` (siehe `ARCH_VIEWER.md`).
- **Layout-Benchmarks** — `force_step` weiterhin innerhalb der Gates trotz
  Pin-Clamp im Integrate-Loop (Zahlen in `RUNLOG.md`, Phase-6-Eintrag).

### v0.4.1 — Detailed Interactive Nodes (Track A, viewer-local)

Zwei-Ebenen-Detailmodell, GPU-skaliert, ohne Layout-Regression — alle Gates
struktur-/pure-fn-geprüft (kein GPU/Pi in CI; FPS-Capture lokal, siehe RUNLOG).

- **Capability-Gate** — ✓ `detect_capability` (V3D/VideoCore/llvmpipe/GLES → Low,
  discrete → High, integrated → Mid), `resolve_detail` (Low → Bild-Decode aus,
  Panels ≤ 1, text-only), `[node_detail]`-Config-Roundtrip. Vorläufer von
  v0.5.0-`QualityTier`.
- **Level-1-Icons** — ✓ *ein* geteilter Atlas-Handle, von allen Glyph-Materialien
  referenziert (`icons_share_one_atlas_and_quad_set`); Subtyp-Mapping pure-fn;
  ein Icon je sichtbarem Knoten (Standard), keiner in Minimal; kein
  Steady-State-Churn.
- **Level-2-Vorschau** — ✓ Panel-Cap erzwungen (`decode_set_respects_panel_cap`);
  LRU insert/evict + Recency-Bump; Decode als Task gespawnt, nicht inline
  (`requests_spawn_a_task_not_inline_decode`); Oversize-Bild übersprungen /
  Oversize-Text gekürzt / verbotener Pfad nicht gelesen; nicht dekodierbares
  Format → Karte; Low → text-only; kein Re-Decode bei stabilem Fokus
  (`stable_focus_has_no_redecode_churn`).
- **Interaktion** — ✓ Vorschau öffnet bei Fokus / schließt bei Clear; Hover ist
  reiner Display-Peek (kein Read); Fokus-Ripple spawnt nur bei Fokuswechsel,
  klingt ab und despawnt (gecappt, Minimal aus); Doppelklick toggelt Expand.
- **Off-thread-Decode** — `bevy/multi_threaded` aktiviert, damit der
  (vorab-genehmigte) `AsyncComputeTaskPool` Decode wirklich nebenläufig ausführt;
  Determinismus-Gate danach grün re-verifiziert.
- **Modulgrenzen** — ✓ Icon-/Preview-State in `render/`/`ui/`; Graph-Wahrheit
  unberührt; rein visuell → determinismus-exempt.
- **Layout-Benchmarks** — `force_step` unverändert innerhalb der Gates (dieser
  Pass ist render-seitig; Zahlen in `RUNLOG.md`, v0.4.1-Closeout).

---

## Definition „Release-fähig“

Ein Release gilt als fertig, wenn:
- alle Gates erfüllt sind
- kein bekannter Crash reproduzierbar ist
- Architekturregeln eingehalten sind
