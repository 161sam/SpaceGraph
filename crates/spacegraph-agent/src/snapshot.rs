use anyhow::Result;
use spacegraph_core::{Msg, Node, NodeId, Edge, EdgeKind};

pub fn build_snapshot() -> Result<Msg> {
    // TODO: procfs scan + fd edges + users/groups
    let nodes = vec![
        (NodeId("user:0".into()), Node::User { uid: 0, name: "root".into() })
    ];
    let edges: Vec<Edge> = vec![];
    Ok(Msg::Snapshot { nodes, edges })
}
