//! Agent-UDS ingest now lives in the headless core ([`spacegraph_graph::net`],
//! ADR-0001 / MP-v0.6.0 P3); re-exported here so the existing `crate::net::*`
//! call sites (app wiring, `GraphState` ingest) resolve unchanged.

pub use spacegraph_graph::net::{
    protocol, spawn_reader, uds, Incoming, IncomingKind, ReaderHandle,
};
