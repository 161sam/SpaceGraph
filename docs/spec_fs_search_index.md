# SpaceGraph — Technical Specification: Filesystem Search & Index

**Status:** v0.1 (for review), 2026-06-14 · **Owner:** 161sam (Sam)
**Feeds:** the FS-search implementation MP. **Scope:** Track-A, viewer **+ agent
+ wire protocol** (no ESN integration). Slots as a standalone feature on the
ladder (suggested `v0.5.2`); independent of the v0.5.1 focus/polish pass and
sequenceable either way.

**Problem this solves:** the node search (Ctrl+P / palette) searches only the
*loaded graph* — files appear as nodes only if observed by a collector, so a file
on disk that nothing touched is unfindable. The fix is **not** to load the
filesystem into the graph (that destroys performance), but to make the
filesystem a fast **searchable index** that is queried separately and from which
results are **materialised into nodes on demand**.

**Binding principle: index ≠ graph.** The index is the searchable universe; the
graph stays bounded. Only a *picked* result becomes a node.

> **Reconciliation (2026-06-14, ADR-0016 / O-7').** D-1 originally let the agent
> shell out to the system `plocate`/`locate`/`mlocate` binary. That introduced
> `std::process::Command` into `spacegraph-agent`, breaking the agent's
> **no-exec** invariant (O-7'). The locate backend has been **removed**; the
> index is now the **builtin walker only** (no exec). The wire (protocol 4) and
> the search/materialise messages are unchanged; only the in-agent source is. The
> sections below are updated to reflect the walker-only design.

---

## 1. Resolved decisions (operator-locked)

- **D-1 — Index source: builtin walker (no exec).** The agent walks the scoped
  roots into a cached path list, kept fresh by incremental inotify updates. It
  **never shells out** to a system `locate` binary — the agent's read-only /
  no-exec guarantee (O-7') is preserved. *(Superseded the original "prefer system
  `plocate`/`mlocate`" decision — see the reconciliation note above.)*
- **D-2 — Scope: root-set default, full-system opt-in.** Default to a sensible
  root set (home + common data roots) with noise excludes; `full_system` is an
  explicit opt-in widening to `/`.
- **D-3 — Privilege: privileged full index, coupled to `AgentMode{User,
  Privileged}`.** `User` (default) returns only paths the agent user may read,
  within scope. `Privileged` enables the full-system index — an explicit,
  off-by-default, audited security surface (see §5).

---

## 2. Architecture

The index lives in the **agent** (host-local), separate from the graph. Flow:

```
viewer Ctrl+P/palette
   ├─ (instant) match loaded graph nodes in-memory
   └─ (async)   SearchRequest ─▶ agent ─▶ index ─▶ ranked results ─▶ SearchResponse
                                                                         │
   pick an on-disk result ─▶ MaterialiseRequest ─▶ agent emits node(s) ─▶ fly-to
```

- **Index source (D-1):** at startup the agent builds the **builtin index**: a
  background walker over the scoped roots builds a cached path list, kept fresh
  incrementally via the existing inotify watches. No system `locate` binary is
  consulted and no subprocess is spawned (O-7').
- **Scope/privilege application (D-2/D-3):** scope + privilege are applied **at
  build time** (the walker only descends allowed roots and only records readable
  entries unless privileged) and **re-checked as a post-filter** on every search
  (`path_allowed`, the single security chokepoint), so an excluded/unreadable path
  is never returned in `User` mode.
- **Materialisation:** a picked result triggers the agent to emit that file as a
  node (and optionally its immediate parent/owner context, bounded) into the live
  stream; the viewer flies to it. **Only picked results materialise** — never the
  whole result set.

No new Cargo dependency: `std::fs` for the walker, an in-house substring/trigram
match for ranking. **No `std::process` / no exec.**

---

## 3. Wire protocol

`PROTOCOL_VERSION` **3 → 4**. New messages:

- `SearchRequest { query: String, limit: u32, full_system: bool }`
- `SearchResponse { results: Vec<SearchHit>, truncated: bool }` where
  `SearchHit { path, kind, size: Option<u64>, mtime: Option<i64>, readable: bool }`
- `MaterialiseRequest { path }` → the agent emits the corresponding node(s) via
  the normal delta stream (no new node path).

**Capability handshake:** the existing version handshake negotiates search
support. An older agent (v3) → the viewer **gracefully falls back to graph-only
search** and labels FS search unavailable. A newer viewer never assumes search.

---

## 4. Query behaviour & speed ("super schnell")

- builtin index: query a cached path list with an in-house ranker
  (exact > prefix > path-substring > fuzzy subsequence), **result cap** (default
  200), run on a background agent thread.
- Viewer side: the search box **debounces** (~120 ms), shows graph-node matches
  instantly and merges agent results as they arrive, **visually distinguished**:
  `IN GRAPH` vs `ON DISK`. Picking an `ON DISK` hit materialises + flies to it.
- Ranking ties broken by recency (mtime) then path depth (shallower first).

---

## 5. Security (the privileged full-system index is a real surface)

- Default posture: `User` + root-set scope. Excluded by default: `/proc`, `/sys`,
  `/dev`, `/run`, plus noise (`.cache`, `node_modules`, `.git/objects`, build
  artefacts) — configurable.
- `Privileged` **and** `full_system` must **both** be explicitly enabled to index
  beyond the user's readable set / beyond the root-set. Off by default.
- Privileged search usage is **audited** (logged with query + caller), forward-
  compatible with the ESN audit discipline — a natural tie-in to the later
  AdminBot/approval world, but here it is **read-only** indexing, no execution.
- In `User` mode an excluded/sensitive path is **never** returned — the walker
  does not record it at build time and the search post-filter (`path_allowed`)
  drops it regardless. A test asserts this.
- Reuse/extend the agent's existing `path_policy` (the Include/Exclude UI already
  in the app) as the single source of scope truth.

---

## 6. Work-package breakdown (→ MP phases)

| WP | Title | Deliverables | Depends |
|---|---|---|---|
| **WP-0** | Protocol + handshake | `SearchRequest`/`SearchResponse`/`MaterialiseRequest`, `PROTOCOL_VERSION` 4, capability negotiation + graceful v3 fallback | — |
| **WP-1** | Agent index | builtin walker (cached list, inotify-incremental, no exec); scope/privilege/path-policy filtering; ranking + cap | WP-0 |
| **WP-2** | Viewer integration | async agent query from Ctrl+P/palette; merged `IN GRAPH`/`ON DISK` results; debounce; on-pick materialise + fly-to | WP-0, WP-1 |
| **WP-3** | Config, docs, tag | `[search]` config; README/DESIGN_LANGUAGE/ACCEPTANCE/RUNLOG; tag | all |

**Per-WP acceptance (machine-checkable):**
- WP-0: protocol round-trip; v3-agent handshake → viewer disables FS search
  without panic.
- WP-1: builtin walker build + query (hits/miss, cap, ranking order);
  **scope/privilege post-filter** — `User` mode never returns an excluded or
  unreadable path (security test); builtin incremental update on an inotify event.
- WP-2: graph-node match is instant + merged with async agent hits (logic test on
  a fake agent); debounce; pick `ON DISK` → `MaterialiseRequest` emitted → node
  added + fly-to (test against a fake agent).
- WP-3: `[search]` config round-trip; ACCEPTANCE gains FS-search criteria; tag.

Headless: all logic is unit-testable; the walker builds over temp dirs and the
search post-filter is exercised over a synthetic path universe; no GPU needed. No
local-capture required (this is not a render pass).

---

## 7. Config (`[search]`)
```
[search]
full_system  = false      # D-2 opt-in (requires Privileged for beyond-user paths)
result_limit = 200
debounce_ms  = 120
# scope roots + excludes reuse the existing path_policy Include/Exclude
```
Privilege is **not** a search key — it is governed by the agent's `AgentMode`
(D-3). `full_system` beyond the user's readable set requires `Privileged`.

---

## 8. Open items
- **OS-1:** the builtin walker is kept fresh by inotify upsert/remove events. If
  inotify watches are exhausted (`max_user_watches`) or a create is missed under
  load, the index can lag for those paths until the next periodic rebuild; this is
  logged. Scope is the policy root-set (a `User`-mode agent only indexes its
  admitted roots — security-by-default; widen via `--include`). *(Originally this
  item covered plocate's `updatedb` staleness; superseded by the walker-only
  design — see the reconciliation note.)*
- **OS-2:** remote viewer ↔ agent: today host-local (UDS). FS search assumes the
  agent is on the indexed host (correct). Remote multi-host search would fan
  `SearchRequest` across agents — a natural later extension, out of scope.

---

## Appendix — file map
**New:** `crates/spacegraph-agent/src/index/` (walker.rs, rank.rs; `mod.rs`
`FsIndex`), `crates/spacegraph-core` search message types; `graph`/`net` viewer
search client. *(The original `locate.rs` shell-out backend was removed — O-7'.)*
**Evolved:** `net/protocol.rs` (+messages, version 4), `ui/search.rs` +
`ui/command_palette.rs` (async agent query, merged results, materialise),
`util/config.rs` + `viewer.toml` (`[search]`), agent `path_policy` (scope reuse).
**Docs:** `README.md`, `docs/DESIGN_LANGUAGE.md`, `docs/ACCEPTANCE.md`,
`docs/perf/RUNLOG.md`, `docs/GRAPH_SCHEMA.md` (new message types).