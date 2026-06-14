# Integration Report — Filesystem Search & Index (`feature/fs-search`)

**For the operator.** This branch implements the FS-search feature (Track-A:
viewer + agent + wire protocol; **no ESN**) per `docs/spec_fs_search_index.md`.
It ran **in parallel** with the *v0.5.1 Focus & Polish* run (`sg-focus` /
`feature/v0.5.1-focus-polish`). Integrate serially.

> ## ⚠️ PROTOCOL BUMP — `PROTOCOL_VERSION` 3 → **4**
> The wire protocol changed. New `Msg` variants (`SearchRequest`,
> `SearchResponse`, `MaterialiseRequest`) and a new `Capabilities.fs_search`
> flag. **Backward compatible by design:** the handshake now accepts the window
> `MIN_COMPATIBLE_PROTOCOL..=PROTOCOL_VERSION` (= `3..=4`) instead of strict
> equality, and gates the feature on the `fs_search` capability — a v3 peer
> connects and runs graph-only, never rejected. Rebuild **both** the agent and
> the viewer from this branch so they agree on v4. (A *deployed* old v3 agent
> binary still rejects a v4 viewer's `Hello(4)` — that is the old binary's
> behaviour, unfixable from here; rebuild it.)

## Branch & status

- **Branch:** `feature/fs-search`, branched from `main` at `ee863b9` (the v0.5.0
  release line). Tip after the Phase 4 merge = `git rev-parse feature/fs-search`.
- **NOT merged to `main`, NOT pushed, NOT tagged.** Left ready for serial
  integration by the operator.
- One sub-branch per phase, each merged `--no-ff` into `feature/fs-search`:
  - Phase 1 `feat/fs-search-wp0-protocol` — WP-0 protocol + handshake.
  - Phase 2 `feat/fs-search-wp1-index` — WP-1 agent index.
  - Phase 3 `feat/fs-search-wp2-viewer` — WP-2 viewer integration.
  - Phase 4 `feat/fs-search-wp3-config-docs` — WP-3 config + docs + this report.
- **Gates green** at every phase: `cargo fmt`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo test --workspace`. Final counts:
  **core 6, agent 44, viewer 163 + 3** (workspace was 188 at v0.5.0).

## Constraints honoured

- **No new Cargo dependency.** `std::process` for the locate shell-out (behind
  the mockable `LocateBackend` trait), `std::fs` for the walker, in-house
  ranking, `libc::access` for readability. The only `Cargo.toml` change is
  enabling the **`sync`** feature on the viewer's existing `tokio` dep (see
  shared files).
- **Security (spec §5):** `index/mod.rs::path_allowed` is the single chokepoint —
  in `User` mode an excluded **or** unreadable path is never returned; excludes
  win over privilege; `full_system` beyond the user's readable set needs
  `Privileged`. Read-only indexing, no execution. Asserted by tests.
- **index ≠ graph:** results never add nodes; only a *picked* result materialises
  (a single bounded `File` node). Asserted by tests.
- Module boundaries kept (`net`/`graph`/agent-index isolated); naming hygiene;
  archive-not-delete; conventional commits; no AI-authorship markers.

## Shared files touched (parallel-run collision surface)

All edits are **additive** (append a block / a struct / a doc section; no
reorganisation), so a serial merge with the v0.5.1 run is trivial.

| File | Addition |
|---|---|
| `crates/spacegraph-viewer/src/util/config.rs` | New `SearchConfig` struct + `ViewerConfig::search` field (`#[serde(default)]`) + `search_config_roundtrip` test. Nothing existing changed. |
| `README.md` (repo root) | Two additive blocks: an FS-search bullet under "UX & Analyse", and three `[search]` rows in the Viewer-Defaults table. |
| `docs/DESIGN_LANGUAGE.md` | Appended a `v0.5.2 — Filesystem search (IN GRAPH vs ON DISK)` section at the end. |
| `docs/ACCEPTANCE.md` | Inserted a `v0.5.2 — Filesystem-Search & Index` section **before** the final "Definition Release-fähig" block. |
| `docs/perf/RUNLOG.md` | Appended a `# FS-Search & Index MP` umbrella with one subsection per phase at the end. |
| `crates/spacegraph-viewer/Cargo.toml` | Added `"sync"` to the `tokio` feature list (already used by `net/uds.rs` via workspace unification; now explicit). Not in the MP's shared-file list, but flagged in case the v0.5.1 run also edits it. |

`viewer.toml` is **runtime-generated** (`config::save`), not a tracked file, so
there is nothing to merge there — the `[search]` block is now emitted
automatically by the new config field.

## New files

- `crates/spacegraph-agent/src/index/mod.rs` — `FsIndex` facade, `path_allowed`
  (security chokepoint), `materialise`, `IndexSource`.
- `crates/spacegraph-agent/src/index/locate.rs` — `detect_locate`, `LocateBackend`
  trait, `SystemLocate`, output parser.
- `crates/spacegraph-agent/src/index/walker.rs` — builtin walker (build/query/
  inotify-incremental).
- `crates/spacegraph-agent/src/index/rank.rs` — in-house tiered ranker.
- `docs/spec_fs_search_index.md` — the binding spec (committed in Phase 0).
- `docs/recon/INTEGRATION_fs-search.md` — this report.

## Other (non-shared) modified files

- **core:** `spacegraph-core/src/lib.rs` — `PROTOCOL_VERSION` 4,
  `MIN_COMPATIBLE_PROTOCOL`/`protocol_compatible`, search messages,
  `Capabilities.fs_search`, `fs_search_available`.
- **agent:** `config.rs` (`--index-source` flag + `index_source`), `main.rs`
  (build the index; thread the walker), `server.rs` (bidirectional read +
  dispatch search/materialise), `sources/mod.rs` + `watch_fs.rs` (feed inotify
  events into the walker).
- **viewer:** `graph/state.rs` (FS search state + methods + apply hooks),
  `net/protocol.rs` (`SearchResponse` incoming), `net/uds.rs` (outbound channel
  + `SearchResponse`), `app/mod.rs` (`pump_outbound` + per-stream outbound
  sender), `ui/search.rs` (merged `IN GRAPH` / `ON DISK` surface).
- **docs:** `GRAPH_SCHEMA.md` (mirrors the v4 contract).

## Host verification (not covered by headless tests)

The locate shell-out is mocked in tests; **real `plocate` behaviour is verified
on the operator's host**. Suggested smoke test (a host with `plocate` and a
running v4 agent):

1. Start the agent (`--mode user`); confirm the log line `FS index: using system
   locate` (or `builtin walker` if no locate binary).
2. In the viewer, `Ctrl+P`, type a filename that is **not** in the graph; confirm
   `ON DISK` rows appear after the debounce.
3. Pick an `ON DISK` row; confirm a `File` node materialises and the camera flies
   to it.
4. Confirm an excluded path (e.g. under `/proc`) never appears in results.

## Deviations (full list in `docs/perf/RUNLOG.md`)

1. **`--index-source` agent CLI flag** — the `[search] index_source` config is
   viewer-side but the index is agent-side and Track-A has no config-push
   message; the flag gives operator control agent-side. The viewer's
   `index_source` is therefore advisory in v1 (agent auto-detects).
2. **`tokio` `sync` feature** on the viewer — not a new dependency; makes an
   already-relied-upon feature explicit so the viewer builds standalone.
