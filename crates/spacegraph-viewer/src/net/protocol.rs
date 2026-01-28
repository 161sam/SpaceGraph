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
