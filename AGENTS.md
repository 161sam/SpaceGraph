# SpaceGraph – AGENTS.md

Dieses Dokument definiert **verbindliche Regeln, Rollen und Arbeitsweisen**
für alle automatisierten oder halbautomatisierten Agenten (z.B. Codex),
die am SpaceGraph-Repository arbeiten.

Ziel ist **sichere, nachvollziehbare, inkrementelle Entwicklung**
ohne Architekturbrüche oder unbeabsichtigte Verhaltensänderungen.

---

## 1. Grundprinzipien (nicht verhandelbar)

1. **Architecture first**
   - Keine Feature-Implementierung ohne Einhaltung der Architektur (`ARCH_VIEWER.md`).
   - Modularisierung hat Vorrang vor neuen Features.

2. **Truth before visuals**
   - Graph-Konsistenz, Korrektheit und Erklärbarkeit sind wichtiger als Optik.
   - Kein UI-Feature darf inkonsistente Graph-Zustände verschleiern.

3. **Determinismus**
   - Gleiche Events → gleicher Graph-Zustand.
   - Keine zufälligen IDs, keine impliziten Merges.

4. **Performance by design**
   - Keine O(E)- oder O(N²)-Operationen in Frame-Updates.
   - Alle teuren Operationen nur auf **capped visible sets**.

5. **Small, reversible steps**
   - Jede Änderung muss isoliert, testbar und rücksetzbar sein.
   - Keine „großen Würfe“ ohne explizite Freigabe.

---

## 2. Agenten-Rollen

### 2.1 Refactor-Agent
**Erlaubt:**
- Dateien verschieben
- Module anlegen
- Code aufteilen
- Imports/Visibility anpassen

**Verboten:**
- Logik ändern
- Algorithmen optimieren
- Neue Features einbauen

**Ziel:**  
Struktur ändern, Verhalten identisch lassen.

---

### 2.2 Feature-Agent
**Erlaubt:**
- Neue Module gemäß Roadmap implementieren
- Tests hinzufügen
- Performance-Verbesserungen innerhalb definierter Grenzen

**Verpflichtend:**
- Vorher prüfen: Abhängigkeiten/Phasenreihenfolge in `docs/ROADMAP.md`
- Architektur-Regeln einhalten
- Tests ergänzen oder erweitern

---

### 2.3 Fix-/Hardening-Agent
**Erlaubt:**
- Bugfixes
- Panic-Removal
- Clippy-/Lint-Fixes
- Guardrails (Caps, Early-Returns)

**Nicht erlaubt:**
- Feature-Erweiterungen
- API-Änderungen ohne Diskussion

---

## 3. Arbeitsregeln für Agenten

### 3.1 Reihenfolge einhalten
Agenten **müssen** die in `docs/ROADMAP.md` festgelegte chronologische
Versions-Leiter einhalten (aktuell: v0.3.x → v0.4.0 *(ausgeliefert)* → v0.5.0 →
v0.6.0 → … → v0.9.0; §4 dort). Phasen werden nicht übersprungen oder
vorgezogen; Reihenfolge-Änderungen erfordern eine Begründung in `docs/adr/`.

Ein Agent darf **nicht**:
- Features einer späteren Phase einbauen, bevor die laufende Phase abgeschlossen ist
- eine ESN-Fabric-Phase (v0.6.0+) ohne ihren Reality-Check-Gate beginnen

---

### 3.2 Keine impliziten Architekturentscheidungen
Agenten dürfen **keine neuen Architekturentscheidungen treffen**, z.B.:

❌ stilles Zusammenführen von Graphen  
❌ Heuristiken für Node-Identität  
❌ „Quick fixes“ mit globalem State  

Alle Architekturänderungen müssen explizit angefordert werden.

---

### 3.3 Modulgrenzen sind verbindlich

| Modul     | Darf nicht wissen von |
|----------|------------------------|
| `render/` | `net/`, Raw Events |
| `ui/`     | `GraphModel`-Interna (Zugriff nur über die `GraphState`-API) |
| `graph/`  | UI-/Render-States — `GraphModel` kennt kein Bevy/UI; `graph/` darf bevy-math/ECS-Resource-Typen (`Vec3`, `Res`/`ResMut`, `Time`) nutzen |
| `net/`    | Graph-Struktur |

Verstöße gelten als **Fehler**, nicht als Optimierung. (Die maßgebliche
Formulierung der Grenzen steht in `docs/ARCH_VIEWER.md`.)

---

## 4. Tests & Qualitätspflichten

### 4.1 Tests sind Pflicht bei:
- neuen Datenstrukturen
- Aggregation / Indizes
- Timeline-Logik
- GC / TTL-Logik

Mindestens:
- ein positiver
- ein negativer Test

---

### 4.2 Qualitäts-Gates (vor jedem Commit)

Agenten müssen sicherstellen:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
````

Ein Agent darf **keinen Commit vorschlagen**, wenn eines davon fehlschlägt.

---

## 5. Commit-Regeln

### 5.1 Commit-Größe

* Maximal **1 Thema pro Commit**
* Keine Misch-Commits (Refactor + Feature)

### 5.2 Commit-Nachrichten

Format:

```
<type>(<scope>): <kurze beschreibung>

Beispiele:
refactor(viewer): split main.rs into app/ui/render modules
feat(timeline): add worldline lifespan and scrub support
fix(gc): prevent orphan file nodes from reappearing
```

---

## 6. Umgang mit Unsicherheit

Wenn ein Agent unsicher ist:

* **nicht raten**
* **nicht implizit entscheiden**
* stattdessen:

  * Annahmen explizit auflisten
  * Rückfrage formulieren

Beispiel:

> „Für Edge-Aggregation gibt es zwei mögliche Key-Strategien … bitte auswählen.“

---

## 7. Abbruchbedingungen

Ein Agent **muss abbrechen**, wenn:

* eine Änderung gegen `ARCH_VIEWER.md` verstößt
* Verhalten nicht sicher reproduzierbar ist
* Tests nicht eindeutig formulierbar sind

In diesem Fall:

* Arbeit stoppen
* Problem präzise beschreiben
* keine halb-fertigen Patches liefern

---

## 8. Zielbild

Agenten arbeiten an SpaceGraph nicht als „Code-Generatoren“,
sondern als **präzise, konservative System-Entwickler**.

> **Stabilität, Nachvollziehbarkeit und Wahrheit haben Vorrang vor Geschwindigkeit.**
