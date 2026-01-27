use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct NodeId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Node {
    Process { pid: i32, ppid: i32, exe: String, cmdline: String, uid: u32 },
    File { path: String, inode: u64, kind: FileKind },
    User { uid: u32, name: String },
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EdgeKind {
    Opens { fd: i32, mode: String }, // "r" | "w" | "rw"
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
#[serde(tag = "type", content = "data")]
pub enum Msg {
    Hello { version: String },
    RequestSnapshot,
    Snapshot { nodes: Vec<(NodeId, Node)>, edges: Vec<Edge> },
    Event { delta: Delta },
    Ping,
    Pong,
}
