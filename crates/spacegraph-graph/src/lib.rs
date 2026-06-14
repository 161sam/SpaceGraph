//! `spacegraph-graph` — the headless canonical-state core for SpaceGraph.
//!
//! Owns the graph model, agent-UDS ingest, the D1/D3/D5 detection pipeline, and
//! read-only queries — with **no Bevy or render/GUI dependency**. Both consumers
//! build on it: the viewer renders *over* this core, and `spacegraph-mcp` hosts
//! it headless to serve the read-only ESN tool surface. See ADR-0001.
//!
//! This crate is populated by the MP-v0.6.0 extraction:
//! - P2 — the pure pipeline (`model`, `rules`, `correlation`, `coverage`,
//!   `posture`, `explain`, queries) moves in here with its tests.
//! - P3 — the agent-UDS ingest (`net`) moves in here.
//! - P4 — `GraphCore` (graph + ingest + pipeline + read-only queries) is
//!   extracted; the viewer's `GraphState` becomes a thin Bevy `Resource` wrapper.
//!
//! P1 is the skeleton only — nothing is moved yet.
