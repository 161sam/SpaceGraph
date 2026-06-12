use spacegraph_core::Node;

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
    }
}
