use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct NodeId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    Opens { fd: i32, mode: String }, // "r" | "w" | "rw" | "?"
    Execs,
    RunsAs,
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
    Hello { version: String },
    Identity { ident: NodeIdentity, caps: Capabilities },
    RequestSnapshot,
    Snapshot { nodes: Vec<(NodeId, Node)>, edges: Vec<Edge> },
    Event { delta: Delta },
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
