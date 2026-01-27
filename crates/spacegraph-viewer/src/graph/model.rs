use spacegraph_core::{Edge, EdgeKind, Node, NodeId};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

#[derive(Default)]
pub struct GraphModel {
    pub nodes: HashMap<NodeId, Node>,
    pub edges: HashSet<Edge>,
    pub last_seen: HashMap<NodeId, Instant>,
}

pub fn node_kind_lane(n: &Node) -> f32 {
    // Y lanes (simple & readable)
    match n {
        Node::Process { .. } => 8.0,
        Node::User { .. } => 0.0,
        Node::File { .. } => -8.0,
    }
}

pub fn edge_kind_name(k: &EdgeKind) -> &'static str {
    match k {
        EdgeKind::Opens { .. } => "opens",
        EdgeKind::Execs => "execs",
        EdgeKind::RunsAs => "runs_as",
    }
}

pub fn edge_explain(k: &EdgeKind) -> String {
    match k {
        EdgeKind::Opens { fd, mode } => format!("process opened file (fd={fd}, mode={mode})"),
        EdgeKind::Execs => "process execs file (exe)".to_string(),
        EdgeKind::RunsAs => "process runs as user (uid)".to_string(),
    }
}
