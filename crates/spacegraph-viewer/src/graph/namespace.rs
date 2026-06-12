//! Stream namespacing for multi-node graphs (v0.2.0).
//!
//! Each connected stream is a namespace. Incoming `NodeId`s are made globally
//! unique by prefixing them with the stream key, so two agents that emit the
//! same local id (e.g. the same pid) never collide and namespaces are **never
//! merged**. The graph stays keyed by `NodeId`; the prefix encodes the origin
//! (the blueprint's "string prefix" option for `Gid { node, local }`).

use spacegraph_core::{Edge, NodeId};

/// Separator between the stream key and the local id. SOH (`\u{1}`) never
/// appears in real paths / ids.
pub const NS_SEP: char = '\u{1}';

/// A stream/agent namespace key (the viewer's per-connection identity).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeKey(pub String);

/// Build a globally-unique id by prefixing a local id with its stream key.
pub fn globalize(stream: &str, local: &NodeId) -> NodeId {
    NodeId(format!("{stream}{NS_SEP}{}", local.0))
}

/// Globalize both endpoints of an edge.
pub fn globalize_edge(stream: &str, e: &Edge) -> Edge {
    Edge {
        from: globalize(stream, &e.from),
        to: globalize(stream, &e.to),
        kind: e.kind.clone(),
    }
}

/// The origin stream key of a global id, or `None` if the id is not namespaced
/// (e.g. demo / synthetic graphs).
pub fn origin(id: &NodeId) -> Option<&str> {
    id.0.split_once(NS_SEP).map(|(stream, _)| stream)
}

/// The local-id portion (after the stream prefix); the whole id if unprefixed.
pub fn local_part(id: &NodeId) -> &str {
    id.0.split_once(NS_SEP)
        .map(|(_, local)| local)
        .unwrap_or(&id.0)
}

/// Prefix that all of a stream's global ids start with.
pub fn prefix(stream: &str) -> String {
    format!("{stream}{NS_SEP}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nid(s: &str) -> NodeId {
        NodeId(s.to_string())
    }

    #[test]
    fn globalize_is_origin_addressable() {
        let g = globalize("hostA", &nid("local:process:pid:42"));
        assert_eq!(origin(&g), Some("hostA"));
        assert_eq!(local_part(&g), "local:process:pid:42");
        assert!(g.0.starts_with(&prefix("hostA")));
    }

    #[test]
    fn distinct_streams_never_collide() {
        let local = nid("local:process:pid:42");
        let a = globalize("a", &local);
        let b = globalize("b", &local);
        assert_ne!(a, b, "same local id under two streams must differ");
        assert_eq!(origin(&a), Some("a"));
        assert_eq!(origin(&b), Some("b"));
    }

    #[test]
    fn unprefixed_id_has_no_origin() {
        let id = nid("demo:process:pid:1");
        assert_eq!(origin(&id), None);
        assert_eq!(local_part(&id), "demo:process:pid:1");
    }
}
