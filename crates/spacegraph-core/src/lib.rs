use serde::{Deserialize, Serialize};

/// Wire-protocol version. Bumped whenever the message schema changes
/// (multi-node handshake, network nodes, alerts). Agent and viewer exchange it
/// in the `Hello` handshake and reject *incompatible* peers (see
/// [`protocol_compatible`]).
///
/// v1: multi-node handshake. v2: network nodes (`Socket`, `RemoteHost`).
/// v3: alert nodes (`Alert`) + `alerts_on` edges.
/// v4: filesystem search/index (`SearchRequest`/`SearchResponse`/
/// `MaterialiseRequest`, `Capabilities::fs_search`). v3 peers stay compatible —
/// they simply never advertise `fs_search`, so the viewer disables FS search.
pub const PROTOCOL_VERSION: u32 = 4;

/// Oldest peer protocol this build still interoperates with. A peer in
/// `MIN_COMPATIBLE_PROTOCOL..=PROTOCOL_VERSION` is accepted; features added in
/// later versions are gated on capability flags ([`Capabilities`]) rather than
/// on the version number, so a v3 agent connects and runs graph-only.
pub const MIN_COMPATIBLE_PROTOCOL: u32 = 3;

/// Whether a peer advertising protocol `peer` can interoperate with this build.
///
/// Backward-compatible by design: instead of rejecting any non-equal version
/// (which would break v3 ⇄ v4), we accept the supported window and negotiate
/// new features via [`Capabilities`]. A newer peer (beyond `PROTOCOL_VERSION`)
/// is rejected — this build cannot assume its schema.
pub fn protocol_compatible(peer: u32) -> bool {
    (MIN_COMPATIBLE_PROTOCOL..=PROTOCOL_VERSION).contains(&peer)
}

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
    /// Agent serves filesystem search/materialise requests (protocol v4+). A v3
    /// agent's `Identity` omits this field, so `#[serde(default)]` decodes it to
    /// `false` and the viewer disables FS search for that stream. This is the
    /// capability half of the handshake negotiation (the version half is
    /// [`protocol_compatible`]).
    #[serde(default)]
    pub fs_search: bool,
}

/// Whether FS search may be offered to the user for a peer that advertised
/// protocol `peer_protocol` and capabilities `caps`. Both must agree: the
/// version must be compatible *and* the agent must advertise `fs_search`.
pub fn fs_search_available(peer_protocol: u32, caps: &Capabilities) -> bool {
    protocol_compatible(peer_protocol) && caps.fs_search
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeIdentity {
    pub node_id: String,
    pub hostname: String,
    pub platform: String,
    pub arch: String,
}

/// One filesystem-index hit (protocol v4). The index is *not* the graph: a hit
/// is a searchable pointer, materialised into a node only when picked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchHit {
    pub path: String,
    pub kind: FileKind,
    pub size: Option<u64>,
    pub mtime: Option<i64>,
    /// Whether the agent user can read this path. In `User` mode an unreadable
    /// (or excluded) path is never returned at all; this flag is informational
    /// for the readable hits a `Privileged` agent may surface.
    pub readable: bool,
}

/// Viewer → agent: query the host-local filesystem index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub limit: u32,
    /// Opt-in widening beyond the root-set scope (D-2). Indexing *beyond the
    /// agent user's readable set* additionally requires the agent to run
    /// `Privileged` (D-3); otherwise only readable paths come back.
    pub full_system: bool,
}

/// Agent → viewer: ranked hits for a [`SearchRequest`]. `truncated` is set when
/// the result cap clipped the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchHit>,
    pub truncated: bool,
}

/// Viewer → agent: materialise this path into the graph. The agent emits the
/// corresponding node(s) via the normal delta stream (`Event`); there is no new
/// node-delivery path. Only *picked* results materialise — never a whole result
/// set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaterialiseRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Msg {
    Hello {
        version: String,
        /// Wire-protocol version; receiver rejects an incompatible peer per
        /// [`protocol_compatible`].
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
    /// FS search (v4). A v3 peer never sends/handles these; they are gated by
    /// the `fs_search` capability so they only flow once both sides agreed.
    SearchRequest(SearchRequest),
    SearchResponse(SearchResponse),
    MaterialiseRequest(MaterialiseRequest),
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

    #[test]
    fn protocol_v4_accepts_v3_and_v4_rejects_others() {
        assert_eq!(PROTOCOL_VERSION, 4);
        // v3 (older) and v4 (this build) interoperate — never break v3 silently.
        assert!(protocol_compatible(3));
        assert!(protocol_compatible(4));
        // Too old (legacy 0/2) and too new (5) are not assumed compatible.
        assert!(!protocol_compatible(0));
        assert!(!protocol_compatible(2));
        assert!(!protocol_compatible(5));
    }

    fn caps_with_fs_search(fs_search: bool) -> Capabilities {
        Capabilities {
            procfs: true,
            fd_edges: true,
            fs_notify: true,
            proc_poll: true,
            ebpf: false,
            cloud: false,
            windows: false,
            fs_search,
        }
    }

    #[test]
    fn fs_search_negotiation_requires_version_and_capability() {
        // A v4 agent that advertises the capability → available.
        assert!(fs_search_available(4, &caps_with_fs_search(true)));
        // A v4 agent that does *not* advertise it (e.g. feature off) → disabled.
        assert!(!fs_search_available(4, &caps_with_fs_search(false)));
        // A v3 agent is compatible but cannot advertise fs_search → disabled.
        assert!(!fs_search_available(3, &caps_with_fs_search(false)));
        // An incompatible version is disabled even if a cap somehow claimed it.
        assert!(!fs_search_available(2, &caps_with_fs_search(true)));
    }

    #[test]
    fn legacy_identity_caps_default_fs_search_to_false() {
        // A v3 agent's Identity has no `fs_search` field; it must decode to false
        // so the viewer disables FS search rather than panicking.
        let v3_identity = r#"{
            "type":"Identity",
            "data":{
                "ident":{"node_id":"n","hostname":"h","platform":"linux","arch":"x86_64"},
                "caps":{"procfs":true,"fd_edges":true,"fs_notify":true,
                        "proc_poll":true,"ebpf":false,"cloud":false,"windows":false}
            }
        }"#;
        match serde_json::from_str::<Msg>(v3_identity).expect("deserialize v3 identity") {
            Msg::Identity { caps, .. } => {
                assert!(!caps.fs_search, "v3 caps must default fs_search to false");
                assert!(!fs_search_available(3, &caps));
            }
            other => panic!("expected Identity, got {other:?}"),
        }
    }

    #[test]
    fn search_messages_roundtrip() {
        let req = Msg::SearchRequest(SearchRequest {
            query: "report".into(),
            limit: 200,
            full_system: false,
        });
        let resp = Msg::SearchResponse(SearchResponse {
            results: vec![SearchHit {
                path: "/home/u/report.pdf".into(),
                kind: FileKind::Regular,
                size: Some(4096),
                mtime: Some(1_700_000_000),
                readable: true,
            }],
            truncated: true,
        });
        let mat = Msg::MaterialiseRequest(MaterialiseRequest {
            path: "/home/u/report.pdf".into(),
        });
        for m in [req, resp, mat] {
            let s = serde_json::to_string(&m).expect("serialize");
            let back: Msg = serde_json::from_str(&s).expect("deserialize");
            // Round-trip preserves the payload (compare via re-serialization).
            assert_eq!(s, serde_json::to_string(&back).expect("reserialize"));
        }
    }
}
