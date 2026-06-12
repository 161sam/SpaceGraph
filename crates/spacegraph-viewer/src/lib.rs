//! SpaceGraph viewer library surface.
//!
//! The binary (`main.rs`) is a thin boot wrapper around these modules. The
//! library target exists so benchmarks and integration tests can construct
//! [`graph::GraphState`] and exercise hot paths (layout, visibility) without a
//! running Bevy app.

pub mod app;
pub mod graph;
pub mod net;
pub mod render;
pub mod ui;
pub mod util;
