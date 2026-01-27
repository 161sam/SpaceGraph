use std::collections::{HashMap, HashSet, VecDeque};

use spacegraph_core::NodeId;

use crate::graph::model::{EdgeKindClass, GraphModel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathStep {
    pub from: NodeId,
    pub to: NodeId,
    pub class: EdgeKindClass,
}

pub fn shortest_path(
    model: &GraphModel,
    a: NodeId,
    b: NodeId,
    max_depth: usize,
    allowed: &HashSet<NodeId>,
) -> Option<Vec<PathStep>> {
    if max_depth == 0 {
        return None;
    }
    if a == b {
        return Some(Vec::new());
    }
    if !allowed.contains(&a) || !allowed.contains(&b) {
        return None;
    }

    let mut visited: HashSet<NodeId> = HashSet::new();
    let mut prev: HashMap<NodeId, (NodeId, EdgeKindClass)> = HashMap::new();
    let mut q: VecDeque<(NodeId, usize)> = VecDeque::new();
    visited.insert(a.clone());
    q.push_back((a.clone(), 0));

    while let Some((cur, depth)) = q.pop_front() {
        if depth >= max_depth {
            continue;
        }
        for edge in model.edges_for_node(&cur) {
            let (next, class) = if edge.from == cur {
                (edge.to.clone(), EdgeKindClass::from_kind(&edge.kind))
            } else {
                (edge.from.clone(), EdgeKindClass::from_kind(&edge.kind))
            };
            if !allowed.contains(&next) || visited.contains(&next) {
                continue;
            }
            visited.insert(next.clone());
            prev.insert(next.clone(), (cur.clone(), class));
            if next == b {
                return Some(reconstruct_path(&prev, a.clone(), b));
            }
            q.push_back((next, depth + 1));
        }
    }

    None
}

fn reconstruct_path(
    prev: &HashMap<NodeId, (NodeId, EdgeKindClass)>,
    start: NodeId,
    end: NodeId,
) -> Vec<PathStep> {
    let mut steps = Vec::new();
    let mut cur = end;
    while cur != start {
        let Some((p, class)) = prev.get(&cur) else {
            break;
        };
        steps.push(PathStep {
            from: p.clone(),
            to: cur.clone(),
            class: *class,
        });
        cur = p.clone();
    }
    steps.reverse();
    steps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::model::GraphModel;
    use spacegraph_core::{Edge, EdgeKind, Node};
    use std::time::Instant;

    #[test]
    fn shortest_path_finds_chain() {
        let mut model = GraphModel::default();
        let now = Instant::now();
        let a = NodeId("a".to_string());
        let b = NodeId("b".to_string());
        let c = NodeId("c".to_string());
        model.upsert_node(
            a.clone(),
            Node::User {
                uid: 1,
                name: "a".to_string(),
            },
            now,
        );
        model.upsert_node(
            b.clone(),
            Node::User {
                uid: 2,
                name: "b".to_string(),
            },
            now,
        );
        model.upsert_node(
            c.clone(),
            Node::User {
                uid: 3,
                name: "c".to_string(),
            },
            now,
        );
        model.upsert_edge(
            Edge {
                from: a.clone(),
                to: b.clone(),
                kind: EdgeKind::Execs,
            },
            now,
        );
        model.upsert_edge(
            Edge {
                from: b.clone(),
                to: c.clone(),
                kind: EdgeKind::RunsAs,
            },
            now,
        );

        let allowed = HashSet::from([a.clone(), b.clone(), c.clone()]);
        let path = shortest_path(&model, a.clone(), c.clone(), 4, &allowed).unwrap();
        assert_eq!(path.len(), 2);
        assert_eq!(path[0].from, a);
        assert_eq!(path[1].to, c);
    }

    #[test]
    fn shortest_path_respects_allowed_set() {
        let mut model = GraphModel::default();
        let now = Instant::now();
        let a = NodeId("a".to_string());
        let b = NodeId("b".to_string());
        let c = NodeId("c".to_string());
        model.upsert_node(
            a.clone(),
            Node::User {
                uid: 1,
                name: "a".to_string(),
            },
            now,
        );
        model.upsert_node(
            b.clone(),
            Node::User {
                uid: 2,
                name: "b".to_string(),
            },
            now,
        );
        model.upsert_node(
            c.clone(),
            Node::User {
                uid: 3,
                name: "c".to_string(),
            },
            now,
        );
        model.upsert_edge(
            Edge {
                from: a.clone(),
                to: b.clone(),
                kind: EdgeKind::Execs,
            },
            now,
        );
        model.upsert_edge(
            Edge {
                from: b.clone(),
                to: c.clone(),
                kind: EdgeKind::RunsAs,
            },
            now,
        );

        let allowed = HashSet::from([a.clone(), c.clone()]);
        let path = shortest_path(&model, a, c, 4, &allowed);
        assert!(path.is_none());
    }
}
