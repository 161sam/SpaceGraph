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

### v0.5.0 — GitS UX-Shell, Radial Command HUD & Quality Tiers (Track A)

Alle Gates struktur-/pure-fn-geprüft (kein GPU/Pi in CI; FPS-Capture lokal, siehe
RUNLOG). Beide Invarianten bleiben grün: Registered-Systems (kein UI/Render-System
verwaist) und Determinismus (`force_step`/`visible_set_capped` deterministisch);
alle v0.5.0-Zusätze sind render/UI oder reine Prädikate → determinismus-exempt.

- **WP-0 Quality-Tiers** — ✓ `detect_tier` (Pi V3D/llvmpipe → Potato, discrete →
  High, GL-iGPU → Low), adaptive State-Machine (3 s ab / 10 s auf + Margin →
  keine Oszillation, nie unter Potato, Cap bei Base), `effective_gates` (Minimal →
  günstigster Pfad), `take_dirty` (genau eine Rekonfiguration pro Wechsel),
  `[quality]`-Config-Roundtrip.
- **WP-1 Shell/Reskin/Selektoren** — ✓ `[shell]`-Roundtrip; Registered-Systems
  nach dem Refactor erneut **leer** verifiziert (inkl. `apply_egui_theme`);
  Control-Inventory (alle Vorbestände erhalten, Tuning unter Technician); Theme +
  Tier Selektoren persistieren; Minimal behält flachen Look (eigener Visuals-Pfad).
- **WP-2 Gate-Glyphs** — ✓ ein Glyph je sichtbarem Knoten (Standard), typ-gefärbt;
  LOD-Selektion pure-fn (`glyph_layer_active`/`silhouette_active`); Potato/Low →
  Silhouette unterdrückt (Glyph primär); Minimal → kein Glyph; kein Steady-State-
  Churn.
- **WP-3 Radial-HUD** — ✓ Zustands-Transitionen (open/switch/rotate-wrap/page-clamp),
  Command→`CtxAct`, Pfad-Indexierung + sortiert-eindeutige Nachbarn; rendert
  panik-frei headless (Camera + Fokus).
- **WP-4 Ripples + Rand-Frame** — ✓ Ripple-Lebenszyklus (Fokus + Alert → abklingen
  → despawn, kein Churn); Rand-Frame liest Global-State panik-frei.
- **WP-5 Palette + Query-DSL** — ✓ Parser (valide/Negation/`deg:>N`/`recent:Nm`/
  malformed→Fehler), Prädikat-Treffer/-Fehlschläge (rein + deterministisch),
  Fuzzy-Match; Filter ersetzt durch Query-DSL mit entfernbaren Chips.
- **Module/Boundaries** — ✓ Query-Prädikat ist Plain-Data in `graph/` (kein Bevy);
  Glyph-/HUD-/Radial-State in `render/`/`ui/`; Graph-Wahrheit unberührt.

### F3 — Lane-Timeline & Tree-View Acceptance (Recon-Finding, hier ergänzt)

Diese in v0.3.x/v0.4.0 implementierten Modi hatten keine expliziten Gates (Recon
F3). Nachgetragen:

- **Lane-Timeline** — Events in Lanes je Entität (pid/Pfad) gruppiert,
  deterministisch bei Pause/Scrub; Hover-Tooltip mit echten Metadaten; Klick auf
  Event → Select → (im Spatial) Jump. Window/X-Scale/Connectors konfigurierbar;
  Events gecappt (Ringpuffer). Determinismus: gleiche Events → gleiche Lanes.
- **Tree-View** — Filesystem-Hierarchie mit Collapse/Expand, datei-LOD per Zoom
  (`tree_file_zoom_threshold`), „Fit to view"; keine ID-Kollisionen; Wechsel
  Spatial⇄Tree⇄Timeline markiert das Layout dirty (eine Neuberechnung, kein Churn).

### v0.5.1 — GitS Focus & Polish (Track A, viewer-local)

Alle Gates struktur-/pure-fn-geprüft (kein GPU/Pi in CI; FPS-Capture lokal, siehe
RUNLOG). Beide Invarianten grün: Registered-Systems (Focus-Systeme registriert,
kein verwaister Systemshape) und Determinismus — `force_step` **byte-unverändert**
ggü. v0.5.0 (Funktion aus beiden Revisionen extrahiert → IDENTICAL); alle Zusätze
sind render/UI → determinismus-exempt. 208 Tests grün.

- **Phase 1 Bugfixes** — ✓ Face-Icon: Atlas ist echte Alpha-Cutout-Maske (bimodal
  0/255, Asset-Test) + `AlphaMode::Mask` mit explizitem Nearest-Sampler (scharfer
  Schnitt statt gefülltem Quad); Billboard auf Knoten-Envelope geklammert
  (`icon_half_extent`, pure-fn). ScrollArea-ID-Kollision im Paths-Dialog behoben
  (`id_source(title)`, eindeutig + stabil).
- **Phase 2 Gate-Ring + Radial-HUD** — ✓ Ring-`LineList` mit Tick-Marks (eine
  geteilte Mesh je Knoten — strukturell, keine Per-Node-Allokation); Typ/Severity-
  Farbe (`ring_color`, pure-fn; Alert-Rampe low/med/high); Radial-HUD-Backing-Disc
  (Lesbarkeit), rendert panik-frei.
- **Phase 3 Edge-Perf** — ✓ `edge_lod` (Full/Dim/Cull) pure-fn: fern → gedimmt/
  gecullt; Focus-Mode → nicht-inzidente Kanten gecullt; Kamera-Zelle quantisiert
  (Rebuild bleibt „settled→cheap"); `force_step` byte-unverändert; `[edge_lod]`-
  Roundtrip; 3-Klassen-FPS-Capture im RUNLOG (Ziel: kein Regress).
- **Phase 4 Focus Mode (Headliner)** — ✓ Enter/Exit-Transitionen (`enter_focus`/
  `exit_focus`); Layout friert bei Fokus / taut bei Exit (`layout_frozen`,
  reversibel — Determinismus grün); Pfad-Dive re-zentriert Fokus
  (`dive_to_neighbor`); **kein Per-Node-Aufwand** (`focus_overlay` ohne `Commands`,
  Struktur-Test); rendert panik-frei headless (Fokus + Camera); Minimal → schlichtes
  Dim+Zentrum (keine Ringe/Arcs/DoF). High-Tier-DoF dokumentiert **deferred**
  (Dim-only ausgeliefert — kein Blocker, §1.4).
- **Module/Boundaries** — ✓ Focus-State als reversibler UI-Zustand (`ui.focus_mode`),
  Graph-Wahrheit unberührt; Render/UI-Systeme determinismus-exempt.

---

## v0.5.2 — Filesystem-Search & Index (Track-A)

Eigenständiges Feature (Viewer + Agent + Wire-Protokoll; kein ESN), Spezifikation
`docs/spec_fs_search_index.md`. **Leitprinzip `index ≠ graph`:** der Index ist das
durchsuchbare Universum, der Graph bleibt bounded — nur ein *gepicktes* Ergebnis
wird zum Node.

### Protokoll & Handshake (WP-0)
- `PROTOCOL_VERSION` **3 → 4**; ein v3-Peer bleibt kompatibel
  (`protocol_compatible` akzeptiert `3..=4`) — **kein stilles Brechen von v3**.
- FS-Search wird per Capability ausgehandelt: nur wenn der Agent `fs_search`
  annonciert. Ein v3-Agent (Cap default `false`) → Viewer deaktiviert FS-Search
  **ohne Panic**, Graph-Suche funktioniert weiter.

### Agent-Index (WP-1)
- **D-1**: bevorzugt System-`plocate`/`locate`/`mlocate` (erkannt, hinter
  mockbarem Trait); sonst Builtin-Walker (gecachte Pfadliste, inotify-inkrementell).
- **Ranking**: exact > prefix > path-substring > fuzzy; Ties nach Recency
  (mtime), dann Pfadtiefe; Result-Cap mit `truncated`-Flag.
- **Security (§5, Test-erzwungen)**: im `User`-Modus wird ein **excluded oder
  unreadable** Pfad **nie** zurückgegeben; Excludes schlagen Privileg; `full_system`
  jenseits des lesbaren Sets erfordert `Privileged`. Privilegierte Suche bleibt
  read-only.

### Viewer-Integration (WP-2)
- Ctrl+P/Palette: Graph-Treffer **instant** (`IN GRAPH`), Agent-Treffer **async**
  gemerged (`ON DISK`), sichtbar unterschieden; Query **debounced**.
- Pick `ON DISK` → `MaterialiseRequest` → Agent emittiert Node(s) über den
  Delta-Stream → Node hinzugefügt + Fly-to. **Nur gepickte Ergebnisse
  materialisieren** — nie der ganze Result-Set.

### Config & Kompatibilität (WP-3)
- `[search]`-Block in `viewer.toml` (`index_source`, `full_system`, `result_limit`,
  `debounce_ms`), additiv, round-trip; eine Config **ohne** `[search]` lädt per
  Default (rückwärtskompatibel).
- **Gates**: `cargo fmt` / `clippy -D warnings` / `cargo test` grün; keine neue
  Cargo-Dependency; Modulgrenzen (`net`/`graph`/Agent-Index isoliert) gewahrt.

## D0 — Perimeter & Exposure (ADR-0012, AUTO, no wire)

- **Port-state-as-aperture (P1):** `aperture_style(state)` ist reine Funktion
  (Unit-Test: LISTEN→Open, ESTABLISHED→Active, FILTERED→Shuttered,
  TIME_WAIT/CLOSE_WAIT/SYN_SENT→Closing). Standard tönt idle Sockets per Apertur;
  Minimal behält den flachen Torus. Cached Material-Handles (keine Per-Frame-Alloc).
- **Exposure-as-depth (P2):** `exposure_bucket(local_addr)` reine Funktion
  (Loopback/LAN/Public inkl. `0.0.0.0`/`::`→Public, RFC1918/link-local/ULA→LAN);
  `shell_factor` ordnet Public außen … Loopback Kern; via `progressive_prepare`,
  gilt in beiden Themes; per Toggle abschaltbar.
- **Anomaly-as-distortion (P3):** `select_focus_alerts` reine Funktion
  (severity→recency, count-bounded ≤ `MAX_ALERT_FOCUS`); Post-FX-Ramp lokalisiert
  um Top-N-Alerts (screen-projiziert); unter Minimal aus (`postfx_active`); WGSL
  naga-validiert. GPU-Look in RUNLOG dokumentiert.
- **Gateway-Node (P4):** `parse_default_gateway` reine Funktion (Default-Route
  `00000000`, little-endian); Gateway als `Node::RemoteHost` (bestehende Art,
  **kein Wire-Bump**), diff-stabil; `/proc/net/route` read-only Parse (kein
  exec/egress).
- **Config & Inspector (P5):** `[socket_display]` (aperture/exposure/anomaly +
  intensity) round-trip; Exposure-Bucket im Inspector-Tooltip.
- **Gates / Audited Negatives:** `fmt`/`clippy -D`/`test --workspace` grün
  (243 Tests); **kein `spacegraph-core`-Change / PROTOCOL_VERSION bleibt 4**;
  **kein `child_process`/exec, kein Egress im Agent**; Minimal-Äquivalenz gewahrt.

---

## Definition „Release-fähig“

Ein Release gilt als fertig, wenn:
- alle Gates erfüllt sind
- kein bekannter Crash reproduzierbar ist
- Architekturregeln eingehalten sind
