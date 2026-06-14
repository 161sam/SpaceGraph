use spacegraph_core::Msg;

#[derive(Debug, Clone)]
pub struct Incoming {
    pub stream: String,
    pub kind: IncomingKind,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum IncomingKind {
    Connected,
    Disconnected,
    Identity(Msg),
    Snapshot(Msg),
    Event(Msg),
    /// Agent → viewer FS search results (protocol v4).
    SearchResponse(Msg),
    Other(Msg),
    Error(String),
}

impl Incoming {
    pub fn connected(stream: String) -> Self {
        Self {
            stream,
            kind: IncomingKind::Connected,
        }
    }

    pub fn disconnected(stream: String) -> Self {
        Self {
            stream,
            kind: IncomingKind::Disconnected,
        }
    }

    pub fn identity(stream: String, msg: Msg) -> Self {
        Self {
            stream,
            kind: IncomingKind::Identity(msg),
        }
    }

    pub fn snapshot(stream: String, msg: Msg) -> Self {
        Self {
            stream,
            kind: IncomingKind::Snapshot(msg),
        }
    }

    pub fn event(stream: String, msg: Msg) -> Self {
        Self {
            stream,
            kind: IncomingKind::Event(msg),
        }
    }

    pub fn search_response(stream: String, msg: Msg) -> Self {
        Self {
            stream,
            kind: IncomingKind::SearchResponse(msg),
        }
    }

    pub fn other(stream: String, msg: Msg) -> Self {
        Self {
            stream,
            kind: IncomingKind::Other(msg),
        }
    }

    pub fn error(stream: String, msg: String) -> Self {
        Self {
            stream,
            kind: IncomingKind::Error(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spacegraph_core::Msg;

    #[test]
    fn incoming_constructors_set_stream_and_kind() {
        assert!(matches!(
            Incoming::connected("a".into()).kind,
            IncomingKind::Connected
        ));
        assert!(matches!(
            Incoming::disconnected("a".into()).kind,
            IncomingKind::Disconnected
        ));
        assert!(matches!(
            Incoming::error("a".into(), "boom".into()).kind,
            IncomingKind::Error(_)
        ));
        let inc = Incoming::event("agent".into(), Msg::RequestSnapshot);
        assert_eq!(inc.stream, "agent");
        assert!(matches!(inc.kind, IncomingKind::Event(_)));
    }

    #[test]
    fn wire_msg_roundtrips_through_the_codec_serde() {
        // The UDS reader frames serde_json bytes; a Msg must survive the
        // encode -> decode the ingest performs (PROTOCOL_VERSION 4, no wire change).
        let m = Msg::RequestSnapshot;
        let bytes = serde_json::to_vec(&m).expect("encode");
        let back: Msg = serde_json::from_slice(&bytes).expect("decode");
        assert!(matches!(back, Msg::RequestSnapshot));
    }
}
