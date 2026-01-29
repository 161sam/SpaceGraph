use smallvec::SmallVec;
use spacegraph_core::{Edge, EdgeKind, Node, NodeId};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

pub type EdgeRef = Edge;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeKindClass {
    Opens,
    Execs,
    RunsAs,
}

impl EdgeKindClass {
    pub fn from_kind(kind: &EdgeKind) -> Self {
        match kind {
            EdgeKind::Opens { .. } => Self::Opens,
            EdgeKind::Execs => Self::Execs,
            EdgeKind::RunsAs => Self::RunsAs,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AggEdgeKey {
    pub from: NodeId,
    pub to: NodeId,
    pub class: EdgeKindClass,
}

impl AggEdgeKey {
    pub fn new(edge: &Edge) -> Self {
        Self {
            from: edge.from.clone(),
            to: edge.to.clone(),
            class: EdgeKindClass::from_kind(&edge.kind),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EdgeStats {
    pub count: u64,
    #[allow(dead_code)]
    pub first_ts: Instant,
    pub last_ts: Instant,
}

#[derive(Debug, Clone)]
pub struct AggEdge {
    pub key: AggEdgeKey,
    pub stats: EdgeStats,
    pub last_kind: EdgeKind,
    pub live_count: usize,
}

#[derive(Default)]
pub struct GraphModel {
    pub nodes: HashMap<NodeId, Node>,
    pub edges: HashSet<Edge>,
    pub last_seen: HashMap<NodeId, Instant>,
    adj: HashMap<NodeId, SmallVec<[EdgeRef; 8]>>,
    agg: HashMap<AggEdgeKey, AggEdge>,
}

pub fn edge_kind_name(k: &EdgeKind) -> &'static str {
    match k {
        EdgeKind::Opens { .. } => "opens",
        EdgeKind::Execs => "execs",
        EdgeKind::RunsAs => "runs_as",
    }
}

pub fn edge_class_name(k: EdgeKindClass) -> &'static str {
    match k {
        EdgeKindClass::Opens => "opens",
        EdgeKindClass::Execs => "execs",
        EdgeKindClass::RunsAs => "runs_as",
    }
}

pub fn edge_explain(k: &EdgeKind) -> String {
    match k {
        EdgeKind::Opens { fd, mode } => format!("process opened file (fd={fd}, mode={mode})"),
        EdgeKind::Execs => "process execs file (exe)".to_string(),
        EdgeKind::RunsAs => "process runs as user (uid)".to_string(),
    }
}

impl GraphModel {
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.edges.clear();
        self.last_seen.clear();
        self.adj.clear();
        self.agg.clear();
    }

    pub fn load_snapshot(&mut self, nodes: Vec<(NodeId, Node)>, edges: Vec<Edge>, now: Instant) {
        self.nodes = nodes.into_iter().collect();
        self.edges = edges.into_iter().collect();
        self.last_seen.clear();
        for id in self.nodes.keys() {
            self.last_seen.insert(id.clone(), now);
        }
        self.rebuild_indices(now);
    }

    pub fn upsert_node(&mut self, id: NodeId, node: Node, now: Instant) {
        self.nodes.insert(id.clone(), node);
        self.last_seen.insert(id, now);
    }

    pub fn remove_node(&mut self, id: &NodeId) -> Vec<Edge> {
        self.nodes.remove(id);
        self.last_seen.remove(id);
        let mut removed = Vec::new();
        if let Some(edges) = self.adj.get(id).cloned() {
            for edge in edges {
                if self.remove_edge(&edge) {
                    removed.push(edge);
                }
            }
        }
        self.adj.remove(id);
        removed
    }

    pub fn upsert_edge(&mut self, edge: Edge, now: Instant) {
        let inserted = self.edges.insert(edge.clone());
        if inserted {
            self.insert_adj(&edge);
        }
        self.update_agg_on_upsert(&edge, now, inserted);
    }

    pub fn remove_edge(&mut self, edge: &Edge) -> bool {
        let removed = self.edges.remove(edge);
        if removed {
            self.remove_adj(edge);
            self.update_agg_on_remove(edge);
        }
        removed
    }

    pub fn edges_for_node(&self, id: &NodeId) -> impl Iterator<Item = &Edge> + '_ {
        self.adj
            .get(id)
            .into_iter()
            .flat_map(|edges| edges.iter())
            .filter_map(|edge| {
                debug_assert!(self.edges.contains(edge));
                self.edges.get(edge)
            })
    }

    pub fn neighbors<'a>(&'a self, id: &'a NodeId) -> impl Iterator<Item = NodeId> + 'a {
        self.edges_for_node(id).map(move |edge| {
            if &edge.from == id {
                edge.to.clone()
            } else {
                edge.from.clone()
            }
        })
    }

    pub fn agg_edges(&self) -> impl Iterator<Item = &AggEdge> + '_ {
        self.agg.values()
    }

    pub fn agg_edge_count(&self) -> usize {
        self.agg.len()
    }

    fn rebuild_indices(&mut self, now: Instant) {
        self.adj.clear();
        self.agg.clear();
        let edges: Vec<Edge> = self.edges.iter().cloned().collect();
        for edge in edges {
            self.insert_adj(&edge);
            self.update_agg_on_upsert(&edge, now, true);
        }
    }

    fn insert_adj(&mut self, edge: &Edge) {
        self.adj
            .entry(edge.from.clone())
            .or_default()
            .push(edge.clone());
        self.adj
            .entry(edge.to.clone())
            .or_default()
            .push(edge.clone());
    }

    fn remove_adj(&mut self, edge: &Edge) {
        self.remove_adj_entry(&edge.from, edge);
        self.remove_adj_entry(&edge.to, edge);
    }

    fn remove_adj_entry(&mut self, id: &NodeId, edge: &Edge) {
        let Some(list) = self.adj.get_mut(id) else {
            return;
        };
        list.retain(|e| e != edge);
        if list.is_empty() {
            self.adj.remove(id);
        }
    }

    fn update_agg_on_upsert(&mut self, edge: &Edge, now: Instant, inserted: bool) {
        let key = AggEdgeKey::new(edge);
        let entry = self.agg.entry(key.clone()).or_insert_with(|| AggEdge {
            key,
            stats: EdgeStats {
                count: 0,
                first_ts: now,
                last_ts: now,
            },
            last_kind: edge.kind.clone(),
            live_count: 0,
        });
        entry.stats.count += 1;
        entry.stats.last_ts = now;
        entry.last_kind = edge.kind.clone();
        if inserted {
            entry.live_count += 1;
        }
    }

    fn update_agg_on_remove(&mut self, edge: &Edge) {
        let key = AggEdgeKey::new(edge);
        let Some(entry) = self.agg.get_mut(&key) else {
            return;
        };
        if entry.live_count > 0 {
            entry.live_count -= 1;
        }
        if entry.live_count == 0 {
            self.agg.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spacegraph_core::FileKind;
    use std::time::Duration;

    fn node_file(path: &str) -> Node {
        Node::File {
            path: path.to_string(),
            inode: 1,
            kind: FileKind::Regular,
        }
    }

    #[test]
    fn adjacency_returns_edges_for_node() {
        let mut model = GraphModel::default();
        let now = Instant::now();
        let a = NodeId("a".to_string());
        let b = NodeId("b".to_string());
        let c = NodeId("c".to_string());
        model.upsert_node(a.clone(), node_file("/a"), now);
        model.upsert_node(b.clone(), node_file("/b"), now);
        model.upsert_node(c.clone(), node_file("/c"), now);

        let e1 = Edge {
            from: a.clone(),
            to: b.clone(),
            kind: EdgeKind::Execs,
        };
        let e2 = Edge {
            from: c.clone(),
            to: a.clone(),
            kind: EdgeKind::RunsAs,
        };
        model.upsert_edge(e1.clone(), now + Duration::from_secs(1));
        model.upsert_edge(e2.clone(), now + Duration::from_secs(2));

        let edges: HashSet<Edge> = model.edges_for_node(&a).cloned().collect();
        assert_eq!(edges.len(), 2);
        assert!(edges.contains(&e1));
        assert!(edges.contains(&e2));
    }

    #[test]
    fn aggregation_coalesces_edges_and_counts_events() {
        let mut model = GraphModel::default();
        let now = Instant::now();
        let a = NodeId("a".to_string());
        let b = NodeId("b".to_string());
        model.upsert_node(a.clone(), node_file("/a"), now);
        model.upsert_node(b.clone(), node_file("/b"), now);

        let e = Edge {
            from: a.clone(),
            to: b.clone(),
            kind: EdgeKind::Opens {
                fd: 3,
                mode: "r".to_string(),
            },
        };
        model.upsert_edge(e.clone(), now);
        model.upsert_edge(e.clone(), now + Duration::from_secs(1));
        model.upsert_edge(e.clone(), now + Duration::from_secs(2));

        assert_eq!(model.agg.len(), 1);
        let agg = model.agg.values().next().unwrap();
        assert_eq!(agg.stats.count, 3);
        assert_eq!(agg.live_count, 1);
        assert_eq!(agg.key.class, EdgeKindClass::Opens);
    }
}
