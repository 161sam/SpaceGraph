use anyhow::{Context, Result};
use procfs::process::Process;
use spacegraph_core::{id_file, id_process, id_user, Edge, EdgeKind, FileKind, Node, NodeId};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;

fn parse_passwd() -> Result<HashMap<u32, String>> {
    let content = match fs::read_to_string("/etc/passwd") {
        Ok(content) => content,
        Err(err) if is_permission_denied(&err) => {
            tracing::warn!("skipping /etc/passwd (permission denied)");
            return Ok(HashMap::new());
        }
        Err(err) => return Err(err).context("read /etc/passwd"),
    };
    let mut map = HashMap::new();
    for line in content.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        // name:x:uid:gid:gecos:home:shell
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 3 {
            if let Ok(uid) = parts[2].parse::<u32>() {
                map.insert(uid, parts[0].to_string());
            }
        }
    }
    Ok(map)
}

type SnapshotData = (Vec<(NodeId, Node)>, Vec<Edge>);

fn file_kind_from_path(p: &str) -> FileKind {
    // MVP heuristic (real kind via metadata later)
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

fn is_permission_denied(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::PermissionDenied
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
            // flags often shown as octal (e.g. 0100002)
            if v.starts_with('0') {
                i64::from_str_radix(v.trim_start_matches("0o").trim_start_matches('0'), 8).ok()
            } else {
                v.parse::<i64>().ok()
            }
        } else {
            continue;
        }?;
        // If parse ok:
        // (we already returned via ?)
    }
    None
}

pub fn build_snapshot(node_id: &str) -> Result<SnapshotData> {
    let passwd = parse_passwd().unwrap_or_default();

    let mut nodes: HashMap<NodeId, Node> = HashMap::new();
    let mut edges: HashSet<Edge> = HashSet::new();

    // Users from passwd that appear as process owners will be added on demand.
    // Processes:
    for pr in procfs::process::all_processes()? {
        let pr = match pr {
            Ok(p) => p,
            Err(_) => continue,
        };

        let stat = match pr.stat() {
            Ok(s) => s,
            Err(_) => continue,
        };

        let pid = stat.pid;
        let ppid = stat.ppid;

        let uid = pr.status().ok().map(|st| st.ruid).unwrap_or(0);

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

        nodes.insert(
            proc_id.clone(),
            Node::Process {
                pid,
                ppid,
                exe: exe.clone(),
                cmdline,
                uid,
            },
        );

        // user node + edge
        let uname = passwd
            .get(&uid)
            .cloned()
            .unwrap_or_else(|| format!("uid{uid}"));
        let user_id = id_user(node_id, uid);
        nodes
            .entry(user_id.clone())
            .or_insert(Node::User { uid, name: uname });
        edges.insert(Edge {
            from: proc_id.clone(),
            to: user_id,
            kind: EdgeKind::RunsAs,
        });

        // exe as file node + edge
        let exe_file_id = id_file(node_id, &exe);
        nodes.entry(exe_file_id.clone()).or_insert(Node::File {
            path: exe.clone(),
            inode: inode_for_path(&exe),
            kind: file_kind_from_path(&exe),
        });
        edges.insert(Edge {
            from: proc_id.clone(),
            to: exe_file_id,
            kind: EdgeKind::Execs,
        });

        // fd edges
        add_fd_edges(node_id, &pr, &proc_id, &mut nodes, &mut edges);
    }

    Ok((nodes.into_iter().collect(), edges.into_iter().collect()))
}

fn add_fd_edges(
    node_id: &str,
    pr: &Process,
    proc_id: &NodeId,
    nodes: &mut HashMap<NodeId, Node>,
    edges: &mut HashSet<Edge>,
) {
    let pid = pr.pid();
    let fd_dir = format!("/proc/{pid}/fd");
    let entries = match fs::read_dir(&fd_dir) {
        Ok(e) => e,
        Err(err) if is_permission_denied(&err) => {
            tracing::debug!(path = %fd_dir, "skipping fd dir (permission denied)");
            return;
        }
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
        nodes.entry(f_id.clone()).or_insert(Node::File {
            path: target.clone(),
            inode: inode_for_path(&target),
            kind: file_kind_from_path(&target),
        });

        let mode = fd_flags(pid, fd)
            .map(fd_mode_from_flags)
            .unwrap_or_else(|| "?".into());
        edges.insert(Edge {
            from: proc_id.clone(),
            to: f_id,
            kind: EdgeKind::Opens { fd, mode },
        });
    }
}
