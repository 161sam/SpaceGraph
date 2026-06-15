use spacegraph_core::{Node, NodeId};

/// A stable short hex id for a node — the GitS `0x032D8A40` chip used in the focus
/// subtitle and panel headers so the eye can correlate a node across surfaces.
/// Deterministic FNV-1a over the node-id bytes; display-only, never graph truth.
pub fn short_hex_id(id: &NodeId) -> String {
    let mut h: u32 = 0x811c_9dc5;
    for b in id.0.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    format!("0x{h:08X}")
}

// viewer-side "pretty path" (display only)
pub fn normalize_display_path(p: &str) -> String {
    let mut s = p.replace("/./", "/");
    while s.contains("//") {
        s = s.replace("//", "/");
    }
    let mut parts = Vec::new();
    for part in s.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            x => parts.push(x),
        }
    }
    let out = format!("/{}", parts.join("/"));
    if out == "/" {
        "/".into()
    } else {
        out
    }
}

pub fn node_label_short(node: &Node) -> String {
    match node {
        Node::Process { cmdline, exe, .. } => {
            if !cmdline.is_empty() {
                cmdline.clone()
            } else {
                normalize_display_path(exe)
            }
        }
        Node::File { path, .. } => normalize_display_path(path),
        Node::User { name, .. } => name.clone(),
        Node::Socket {
            proto,
            local_addr,
            local_port,
            ..
        } => format!("{proto} {local_addr}:{local_port}"),
        Node::RemoteHost { addr, rdns } => match rdns {
            Some(name) => format!("{name} ({addr})"),
            None => addr.clone(),
        },
        Node::Alert { signature, .. } => format!("⚠ {signature}"),
    }
}

pub fn node_label_long(node: &Node) -> Vec<String> {
    match node {
        Node::Process {
            pid,
            ppid,
            exe,
            cmdline,
            uid,
        } => vec![
            "kind: process".to_string(),
            format!("pid: {pid} ppid: {ppid} uid: {uid}"),
            format!("exe: {}", normalize_display_path(exe)),
            format!("cmd: {}", cmdline),
        ],
        Node::File { path, inode, kind } => vec![
            "kind: file".to_string(),
            format!("path: {}", normalize_display_path(path)),
            format!("inode: {}", inode),
            format!("filekind: {:?}", kind),
        ],
        Node::User { uid, name } => {
            vec!["kind: user".to_string(), format!("uid: {uid} name: {name}")]
        }
        Node::Socket {
            proto,
            local_addr,
            local_port,
            state,
        } => vec![
            "kind: socket".to_string(),
            format!("proto: {proto}"),
            format!("local: {local_addr}:{local_port}"),
            format!("state: {state}"),
        ],
        Node::RemoteHost { addr, rdns } => vec![
            "kind: remote_host".to_string(),
            format!("addr: {addr}"),
            format!("rdns: {}", rdns.as_deref().unwrap_or("-")),
        ],
        Node::Alert {
            source,
            signature,
            severity,
            ts,
        } => vec![
            "kind: alert".to_string(),
            format!("severity: {severity}"),
            format!("signature: {signature}"),
            format!("source: {source}"),
            format!("ts: {ts}"),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_hex_id_is_stable_and_distinct() {
        let a = NodeId("proc:1234".to_string());
        let b = NodeId("proc:1235".to_string());
        let id_a = short_hex_id(&a);
        assert_eq!(id_a, short_hex_id(&a), "deterministic for the same id");
        assert_ne!(id_a, short_hex_id(&b), "different ids → different chips");
        assert!(
            id_a.starts_with("0x") && id_a.len() == 10,
            "0x + 8 hex digits, got {id_a}"
        );
    }
}
