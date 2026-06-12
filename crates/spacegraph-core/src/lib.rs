use serde::{Deserialize, Serialize};

/// Wire-protocol version. Bumped whenever the message schema changes
/// (multi-node handshake, network nodes, alerts). Agent and viewer exchange it
/// in the `Hello` handshake and reject mismatches.
///
/// v1: multi-node handshake. v2: network nodes (`Socket`, `RemoteHost`).
/// v3: alert nodes (`Alert`) + `alerts_on` edges.
pub const PROTOCOL_VERSION: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct NodeId(pub String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Node {
    Process {
        pid: i32,
        ppid: i32,
        exe: String,
        cmdline: String,
        uid: u32,
    },
    File {
        path: String,
        inode: u64,
        kind: FileKind,
    },
    User {
        uid: u32,
        name: String,
    },
    /// A network socket owned by a process (network layer, v0.3.x).
    Socket {
        proto: String, // "tcp" | "tcp6" | "udp" | "udp6"
        local_addr: String,
        local_port: u16,
        state: String, // "LISTEN" | "ESTABLISHED" | ...
    },
    /// A remote endpoint a socket connects to.
    RemoteHost {
        addr: String,
        rdns: Option<String>,
    },
    /// A security alert (e.g. from Suricata EVE) correlated to a connection.
    Alert {
        source: String,    // "suricata"
        signature: String, // rule message
        severity: String,  // "low" | "medium" | "high"
        ts: String,        // ISO timestamp (display)
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileKind {
    Regular,
    Dir,
    Socket,
    Pipe,
    Device,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "type", content = "data")]
pub enum EdgeKind {
    Opens {
        fd: i32,
        mode: String,
    }, // "r" | "w" | "rw" | "?"
    Execs,
    RunsAs,
    /// process → socket (the process holds this socket fd).
    OwnsSocket,
    /// socket → remote_host (an established connection).
    ConnectsTo,
    /// process → socket (a listening socket).
    ListensOn,
    /// alert → socket | process | remote_host (the alerted entity).
    AlertsOn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Delta {
    BatchBegin { id: u64 },
    BatchEnd { id: u64 },
    UpsertNode { id: NodeId, node: Node },
    RemoveNode { id: NodeId },
    UpsertEdge { edge: Edge },
    RemoveEdge { edge: Edge },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    pub procfs: bool,
    pub fd_edges: bool,
    pub fs_notify: bool,
    pub proc_poll: bool,
    pub ebpf: bool,
    pub cloud: bool,
    pub windows: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeIdentity {
    pub node_id: String,
    pub hostname: String,
    pub platform: String,
    pub arch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Msg {
    Hello {
        version: String,
        /// Wire-protocol version; receiver rejects on mismatch with
        /// [`PROTOCOL_VERSION`].
        #[serde(default)]
        protocol: u32,
    },
    Identity {
        ident: NodeIdentity,
        caps: Capabilities,
    },
    RequestSnapshot,
    Snapshot {
        nodes: Vec<(NodeId, Node)>,
        edges: Vec<Edge>,
    },
    Event {
        delta: Delta,
    },
    Ping,
    Pong,
}

/// Build globally unique IDs (scope = node_id).
pub fn id_process(node_id: &str, pid: i32) -> NodeId {
    NodeId(format!("{node_id}:process:pid:{pid}"))
}
pub fn id_user(node_id: &str, uid: u32) -> NodeId {
    NodeId(format!("{node_id}:user:{uid}"))
}
pub fn id_file(node_id: &str, path: &str) -> NodeId {
    // MVP: use raw path. Later you can hash/normalize for privacy.
    NodeId(format!("{node_id}:file:{path}"))
}
pub fn id_socket(node_id: &str, proto: &str, local_addr: &str, local_port: u16) -> NodeId {
    NodeId(format!(
        "{node_id}:socket:{proto}:{local_addr}:{local_port}"
    ))
}
pub fn id_remote_host(node_id: &str, addr: &str) -> NodeId {
    NodeId(format!("{node_id}:remote:{addr}"))
}
pub fn id_alert(node_id: &str, key: &str) -> NodeId {
    NodeId(format!("{node_id}:alert:{key}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_roundtrips_protocol() {
        let m = Msg::Hello {
            version: "x".to_string(),
            protocol: PROTOCOL_VERSION,
        };
        let s = serde_json::to_string(&m).expect("serialize");
        match serde_json::from_str::<Msg>(&s).expect("deserialize") {
            Msg::Hello { protocol, .. } => assert_eq!(protocol, PROTOCOL_VERSION),
            other => panic!("expected Hello, got {other:?}"),
        }
    }

    #[test]
    fn legacy_hello_without_protocol_defaults_to_zero() {
        // A pre-v0.2.0 Hello has no `protocol` field; it must decode to 0 so the
        // receiver detects the mismatch against PROTOCOL_VERSION.
        let legacy = r#"{"type":"Hello","data":{"version":"0.1.0"}}"#;
        match serde_json::from_str::<Msg>(legacy).expect("deserialize legacy") {
            Msg::Hello { protocol, .. } => assert_eq!(protocol, 0),
            other => panic!("expected Hello, got {other:?}"),
        }
        assert_ne!(0, PROTOCOL_VERSION, "current protocol must reject legacy 0");
    }
}
