//! Agent-UDS ingest (headless): the length-delimited JSON wire client that
//! connects to a `spacegraph-agent` Unix socket, performs the `Hello`/protocol
//! handshake (`PROTOCOL_VERSION` — unchanged, O-8), and decodes the agent stream
//! into [`Incoming`] events delivered over a crossbeam channel. No Bevy/render
//! dependency — both the viewer and `spacegraph-mcp` ingest through this.

pub mod protocol;
pub mod uds;

pub use protocol::{Incoming, IncomingKind};
pub use uds::{spawn_reader, ReaderHandle};
