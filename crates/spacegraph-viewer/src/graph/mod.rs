//! The viewer's graph layer. The **pure** canonical-state pipeline now lives in
//! the headless `spacegraph-graph` crate (ADR-0001 / MP-v0.6.0 P2) and is
//! re-exported here so existing `crate::graph::*` call sites resolve unchanged;
//! the viewer renders *over* that core. The modules below are the viewer-side
//! pieces that remain Bevy/ECS-coupled (state, layout, render housekeeping) until
//! the P4 `GraphCore` extraction.

// Re-export the headless pure pipeline under the historical `crate::graph::*` path.
pub use spacegraph_graph::{correlation, coverage, explain, model, posture};

pub mod gc;
pub mod grid;
pub mod interner;
pub mod layout;
pub mod metrics;
pub mod namespace;
pub mod query;
/// Viewer-side Bevy integration over [`spacegraph_graph::rules`] (re-exports the
/// pure engine + adds the budgeted `Update` detection system over `GraphState`).
pub mod rules;
pub mod state;
pub mod synthetic;
pub mod timeline;
pub mod tree;

pub use layout::update_layout_or_timeline;
pub use metrics::tick_housekeeping;
pub use state::{GraphState, ViewMode};
pub use timeline::TimelineEvtKind;
