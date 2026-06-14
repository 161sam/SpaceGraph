//! `spacegraph-graph` — the headless canonical-state core for SpaceGraph.
//!
//! Owns the graph model, the D1/D3/D5 detection pipeline, exposure
//! classification, and (from P3/P4) agent-UDS ingest + read-only queries — with
//! **no Bevy or render/GUI dependency**. Both consumers build on it: the viewer
//! renders *over* this core, and `spacegraph-mcp` hosts it headless to serve the
//! read-only ESN tool surface. See ADR-0001.
//!
//! Extraction status (MP-v0.6.0):
//! - P2 (done) — the pure pipeline: [`model`], [`rules`] (engine), [`correlation`]
//!   (campaigns), [`coverage`], [`posture`], [`explain`], [`exposure`].
//! - P3 — agent-UDS ingest moves in here.
//! - P4 — `GraphCore` (graph + ingest + pipeline + read-only queries) is
//!   extracted; the viewer's `GraphState` becomes a thin Bevy `Resource` wrapper.

pub mod correlation;
pub mod coverage;
pub mod explain;
pub mod exposure;
pub mod graph_core;
pub mod model;
pub mod net;
pub mod posture;
pub mod rules;

pub use graph_core::GraphCore;
