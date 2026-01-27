use anyhow::Result;
use procfs::process::Process;
use spacegraph_core::{
    id_file, id_process, id_user, Delta, Edge, EdgeKind, FileKind, Msg, Node, NodeId,
};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::time::Duration;
use tokio::sync::mpsc;

fn list_pids() -> HashSet<i32> {
    let mut set = HashSet::new();
    if let Ok(rd) = std::fs::read_dir("/proc") {
        for ent in rd.flatten() {
            if let Ok(pid) = ent.file_name().to_string_lossy().parse::<i32>() {
                set.insert(pid);
            }
        }
    }
    set
}

fn parse_passwd() -> HashMap<u32, String> {
    let mut map = HashMap::new();
    let content = fs::read_to_string("/etc/passwd").unwrap_or_default();
    for line in content.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 3 {
            if let Ok(uid) = parts[2].parse::<u32>() {
                map.insert(uid, parts[0].to_string());
            }
        }
    }
    map
}

fn file_kind_from_path(p: &str) -> FileKind {
    if p.starts_with("socket:") {
        FileKind::Socket
    } else if p.starts_with("pipe:") {
        FileKind::Pipe
    } else if p.starts_with("/dev/") {
        FileKind::Device
    } else {
        FileKind::Unknown
    }
}

fn inode_for_path(path: &str) -> u64 {
    fs::metadata(path)
        .map(|m| {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                m.ino()
            }
            #[cfg(not(unix))]
            {
                0
            }
        })
        .unwrap_or(0)
}

fn fd_mode_from_flags(flags: i64) -> String {
    // O_ACCMODE = 3
    match flags & 3 {
        0 => "r".to_string(),  // O_RDONLY
        1 => "w".to_string(),  // O_WRONLY
        2 => "rw".to_string(), // O_RDWR
        _ => "?".to_string(),
    }
}

fn fd_flags(pid: i32, fd: i32) -> Option<i64> {
    let p = format!("/proc/{pid}/fdinfo/{fd}");
    let s = fs::read_to_string(p).ok()?;
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("flags:") {
            let v = rest.trim();
            // flags often octal-like (e.g. 0100002)
            if v.starts_with('0') {
                return i64::from_str_radix(v.trim_start_matches('0'), 8).ok();
            } else {
                return v.parse::<i64>().ok();
            }
        }
    }
    None
}

fn add_fd_edges(
    node_id: &str,
    pid: i32,
    proc_id: &NodeId,
    nodes: &mut Vec<(NodeId, Node)>,
    edges: &mut Vec<Edge>,
    seen_nodes: &mut HashSet<NodeId>,
) {
    let fd_dir = format!("/proc/{pid}/fd");
    let entries = match fs::read_dir(fd_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for ent in entries.flatten() {
        let name = ent.file_name();
        let fd: i32 = match name.to_string_lossy().parse() {
            Ok(n) => n,
            Err(_) => continue,
        };

        let target = match fs::read_link(ent.path()) {
            Ok(t) => t.to_string_lossy().to_string(),
            Err(_) => continue,
        };

        let f_id = id_file(node_id, &target);
        if seen_nodes.insert(f_id.clone()) {
            nodes.push((
                f_id.clone(),
                Node::File {
                    path: target.clone(),
                    inode: inode_for_path(&target),
                    kind: file_kind_from_path(&target),
                },
            ));
        }

        let mode = fd_flags(pid, fd).map(fd_mode_from_flags).unwrap_or_else(|| "?".into());
        edges.push(Edge {
            from: proc_id.clone(),
            to: f_id,
            kind: EdgeKind::Opens { fd, mode },
        });
    }
}

fn collect_process_detail(
    node_id: &str,
    passwd: &HashMap<u32, String>,
    pid: i32,
) -> Option<(Vec<(NodeId, Node)>, Vec<Edge>)> {
    let pr = Process::new(pid).ok()?;
    let stat = pr.stat().ok()?;

    let ppid = stat.ppid;

    let uid = pr
        .status()
        .ok()
        .and_then(|st| st.ruid.map(|x| x as u32))
        .unwrap_or(0);

    let exe = pr
        .exe()
        .ok()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "<unknown>".into());

    let cmdline = pr
        .cmdline()
        .ok()
        .map(|v| v.join(" "))
        .unwrap_or_else(|| stat.comm.clone());

    let proc_id = id_process(node_id, pid);

    let mut nodes: Vec<(NodeId, Node)> = Vec::new();
    let mut edges: Vec<Edge> = Vec::new();
    let mut seen_nodes: HashSet<NodeId> = HashSet::new();

    // process node
    nodes.push((
        proc_id.clone(),
        Node::Process {
            pid,
            ppid,
            exe: exe.clone(),
            cmdline,
            uid,
        },
    ));
    seen_nodes.insert(proc_id.clone());

    // user node + edge
    let uname = passwd
        .get(&uid)
        .cloned()
        .unwrap_or_else(|| format!("uid{uid}"));
    let user_id = id_user(node_id, uid);
    if seen_nodes.insert(user_id.clone()) {
        nodes.push((user_id.clone(), Node::User { uid, name: uname }));
    }
    edges.push(Edge {
        from: proc_id.clone(),
        to: user_id,
        kind: EdgeKind::RunsAs,
    });

    // exe as file + edge
    let exe_file_id = id_file(node_id, &exe);
    if seen_nodes.insert(exe_file_id.clone()) {
        nodes.push((
            exe_file_id.clone(),
            Node::File {
                path: exe.clone(),
                inode: inode_for_path(&exe),
                kind: file_kind_from_path(&exe),
            },
        ));
    }
    edges.push(Edge {
        from: proc_id.clone(),
        to: exe_file_id,
        kind: EdgeKind::Execs,
    });

    // fd edges
    add_fd_edges(node_id, pid, &proc_id, &mut nodes, &mut edges, &mut seen_nodes);

    Some((nodes, edges))
}

pub fn spawn(node_id: &str, tx: mpsc::Sender<Msg>) -> Result<()> {
    let node_id = node_id.to_string();

    tokio::spawn(async move {
        let mut prev = list_pids();
        let mut batch_id: u64 = 1;
        let mut passwd = parse_passwd();

        loop {
            tokio::time::sleep(Duration::from_millis(750)).await;

            // refresh passwd occasionally (cheap, keeps usernames accurate)
            if batch_id % 80 == 0 {
                passwd = parse_passwd();
            }

            let cur = list_pids();

            let new_pids: Vec<i32> = cur.difference(&prev).copied().collect();
            let gone_pids: Vec<i32> = prev.difference(&cur).copied().collect();

            if new_pids.is_empty() && gone_pids.is_empty() {
                prev = cur;
                continue;
            }

            // IMPORTANT: BatchBegin first
            let _ = tx
                .send(Msg::Event {
                    delta: Delta::BatchBegin { id: batch_id },
                })
                .await;

            // handle new pids with detail refresh
            for pid in new_pids {
                if let Some((nodes, edges)) = collect_process_detail(&node_id, &passwd, pid) {
                    for (id, node) in nodes {
                        let _ = tx
                            .send(Msg::Event {
                                delta: Delta::UpsertNode { id, node },
                            })
                            .await;
                    }
                    for edge in edges {
                        let _ = tx
                            .send(Msg::Event {
                                delta: Delta::UpsertEdge { edge },
                            })
                            .await;
                    }
                } else {
                    // fallback minimal node if /proc vanished quickly
                    let id = id_process(&node_id, pid);
                    let node = Node::Process {
                        pid,
                        ppid: 0,
                        exe: "<unknown>".into(),
                        cmdline: "<new>".into(),
                        uid: 0,
                    };
                    let _ = tx
                        .send(Msg::Event {
                            delta: Delta::UpsertNode { id, node },
                        })
                        .await;
                }
            }

            // handle gone pids
            for pid in gone_pids {
                let id: NodeId = id_process(&node_id, pid);
                let _ = tx
                    .send(Msg::Event {
                        delta: Delta::RemoveNode { id },
                    })
                    .await;
            }

            // IMPORTANT: BatchEnd last
            let _ = tx
                .send(Msg::Event {
                    delta: Delta::BatchEnd { id: batch_id },
                })
                .await;

            batch_id = batch_id.wrapping_add(1);
            prev = cur;
        }
    });

    Ok(())
}
