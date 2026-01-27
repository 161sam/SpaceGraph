use spacegraph_core::Node;
use std::hash::{Hash, Hasher};

pub fn stable_u32(s: &str) -> u32 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    (h.finish() & 0xFFFF_FFFF) as u32
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
    }
}
