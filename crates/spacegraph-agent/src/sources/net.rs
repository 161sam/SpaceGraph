//! Network [`EventSource`]: process ↔ socket ↔ remote-host topology from
//! procfs (`/proc/net/{tcp,tcp6,udp,udp6}` + `/proc/<pid>/fd` inode→pid).
//!
//! Diff-based: each poll computes the current socket graph and emits only the
//! changes, so a stable system produces no event storm. Parsing, CIDR matching
//! and diffing are pure functions (unit-tested with committed fixtures); only
//! `collect` touches the filesystem.

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

use anyhow::Result;
use spacegraph_core::{
    id_process, id_remote_host, id_socket, Delta, Edge, EdgeKind, Msg, Node, NodeId,
};
use tokio::sync::mpsc;

use super::EventSource;

/// A CIDR block for remote-host include/exclude filtering.
#[derive(Debug, Clone, Copy)]
pub struct Cidr {
    base: IpAddr,
    prefix: u8,
}

impl Cidr {
    pub fn parse(s: &str) -> Option<Cidr> {
        let (ip_s, pfx_s) = s.split_once('/')?;
        let base: IpAddr = ip_s.trim().parse().ok()?;
        let prefix: u8 = pfx_s.trim().parse().ok()?;
        let max = if base.is_ipv4() { 32 } else { 128 };
        if prefix > max {
            return None;
        }
        Some(Cidr { base, prefix })
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self.base, ip) {
            (IpAddr::V4(b), IpAddr::V4(i)) => bits_match(&b.octets(), &i.octets(), self.prefix),
            (IpAddr::V6(b), IpAddr::V6(i)) => bits_match(&b.octets(), &i.octets(), self.prefix),
            _ => false,
        }
    }
}

fn bits_match(a: &[u8], b: &[u8], prefix: u8) -> bool {
    let prefix = prefix as usize;
    let full = prefix / 8;
    if a[..full] != b[..full] {
        return false;
    }
    let rem = prefix % 8;
    if rem == 0 {
        return true;
    }
    let mask = 0xFFu8 << (8 - rem);
    (a[full] & mask) == (b[full] & mask)
}

#[derive(Debug, Clone)]
pub struct NetConfig {
    pub poll_interval: Duration,
    pub include: Vec<Cidr>,
    pub exclude: Vec<Cidr>,
    pub collapse_loopback: bool,
}

impl NetConfig {
    /// Build from raw CLI strings; invalid CIDRs are dropped (logged by caller).
    pub fn from_args(poll_secs: u64, include: &[String], exclude: &[String]) -> Self {
        let parse = |xs: &[String]| xs.iter().filter_map(|s| Cidr::parse(s)).collect();
        NetConfig {
            poll_interval: Duration::from_secs(poll_secs.max(1)),
            include: parse(include),
            exclude: parse(exclude),
            collapse_loopback: true,
        }
    }

    /// Whether a remote IP passes the include/exclude filters.
    fn remote_allowed(&self, ip: IpAddr) -> bool {
        if self.exclude.iter().any(|c| c.contains(ip)) {
            return false;
        }
        if self.include.is_empty() {
            return true;
        }
        self.include.iter().any(|c| c.contains(ip))
    }
}

/// A parsed row of a `/proc/net/{tcp,udp}*` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocketRow {
    pub proto: String,
    pub local_ip: IpAddr,
    pub local_port: u16,
    pub remote_ip: IpAddr,
    pub remote_port: u16,
    pub state: String,
    pub inode: u64,
}

/// Map a Linux TCP state hex code to its name.
pub fn tcp_state_name(hex: &str) -> &'static str {
    match hex {
        "01" => "ESTABLISHED",
        "02" => "SYN_SENT",
        "03" => "SYN_RECV",
        "04" => "FIN_WAIT1",
        "05" => "FIN_WAIT2",
        "06" => "TIME_WAIT",
        "07" => "CLOSE",
        "08" => "CLOSE_WAIT",
        "09" => "LAST_ACK",
        "0A" => "LISTEN",
        "0B" => "CLOSING",
        _ => "UNKNOWN",
    }
}

fn parse_hex_port(hex: &str) -> Option<u16> {
    u16::from_str_radix(hex, 16).ok()
}

fn parse_addr_v4(hex: &str) -> Option<IpAddr> {
    if hex.len() != 8 {
        return None;
    }
    let raw = u32::from_str_radix(hex, 16).ok()?;
    Some(IpAddr::V4(Ipv4Addr::from(raw.to_le_bytes())))
}

fn parse_addr_v6(hex: &str) -> Option<IpAddr> {
    if hex.len() != 32 {
        return None;
    }
    let mut bytes = [0u8; 16];
    for w in 0..4 {
        let word = u32::from_str_radix(&hex[w * 8..w * 8 + 8], 16).ok()?;
        bytes[w * 4..w * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    Some(IpAddr::V6(Ipv6Addr::from(bytes)))
}

fn parse_endpoint(field: &str, is_v6: bool) -> Option<(IpAddr, u16)> {
    let (addr, port) = field.split_once(':')?;
    let ip = if is_v6 {
        parse_addr_v6(addr)?
    } else {
        parse_addr_v4(addr)?
    };
    Some((ip, parse_hex_port(port)?))
}

/// Parse a `/proc/net/{tcp,tcp6,udp,udp6}` table body (skips the header line).
pub fn parse_net_table(content: &str, proto: &str, is_v6: bool) -> Vec<SocketRow> {
    let mut rows = Vec::new();
    let is_tcp = proto.starts_with("tcp");
    for line in content.lines().skip(1) {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 10 {
            continue;
        }
        let Some((local_ip, local_port)) = parse_endpoint(f[1], is_v6) else {
            continue;
        };
        let Some((remote_ip, remote_port)) = parse_endpoint(f[2], is_v6) else {
            continue;
        };
        let state = if is_tcp {
            tcp_state_name(f[3]).to_string()
        } else {
            String::new()
        };
        let Ok(inode) = f[9].parse::<u64>() else {
            continue;
        };
        rows.push(SocketRow {
            proto: proto.to_string(),
            local_ip,
            local_port,
            remote_ip,
            remote_port,
            state,
            inode,
        });
    }
    rows
}

/// Parse the inode out of a `/proc/<pid>/fd/<n>` symlink target
/// (`socket:[12345]` → `12345`).
pub fn parse_socket_inode(link_target: &str) -> Option<u64> {
    let rest = link_target.strip_prefix("socket:[")?;
    let num = rest.strip_suffix(']')?;
    num.parse().ok()
}

/// Scan `/proc/<pid>/fd` to map socket inodes to owning pids (best-effort).
fn inode_pid_map() -> HashMap<u64, i32> {
    let mut map = HashMap::new();
    let Ok(proc_dir) = std::fs::read_dir("/proc") else {
        return map;
    };
    for entry in proc_dir.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|s| s.parse::<i32>().ok()) else {
            continue;
        };
        let fd_dir = entry.path().join("fd");
        let Ok(fds) = std::fs::read_dir(&fd_dir) else {
            continue;
        };
        for fd in fds.flatten() {
            if let Ok(target) = std::fs::read_link(fd.path()) {
                if let Some(inode) = target.to_str().and_then(parse_socket_inode) {
                    map.entry(inode).or_insert(pid);
                }
            }
        }
    }
    map
}

/// Display address for a remote endpoint, collapsing loopback if configured.
fn remote_label(ip: IpAddr, collapse_loopback: bool) -> String {
    if collapse_loopback && ip.is_loopback() {
        "localhost".to_string()
    } else {
        ip.to_string()
    }
}

/// Build the socket/remote-host subgraph from parsed rows + inode→pid map.
pub fn build_graph(
    rows: &[SocketRow],
    inode_pid: &HashMap<u64, i32>,
    node_id: &str,
    cfg: &NetConfig,
) -> (HashMap<NodeId, Node>, HashSet<Edge>) {
    let mut nodes = HashMap::new();
    let mut edges = HashSet::new();

    for row in rows {
        // Only sockets we can attribute to a process.
        let Some(&pid) = inode_pid.get(&row.inode) else {
            continue;
        };
        if row.inode == 0 {
            continue;
        }

        let local_addr = row.local_ip.to_string();
        let sock_id = id_socket(node_id, &row.proto, &local_addr, row.local_port);
        let proc_id = id_process(node_id, pid);
        let listening = row.state == "LISTEN";

        nodes.insert(
            sock_id.clone(),
            Node::Socket {
                proto: row.proto.clone(),
                local_addr,
                local_port: row.local_port,
                state: row.state.clone(),
            },
        );
        edges.insert(Edge {
            from: proc_id,
            to: sock_id.clone(),
            kind: if listening {
                EdgeKind::ListensOn
            } else {
                EdgeKind::OwnsSocket
            },
        });

        // Established connections get a remote host + connects_to edge.
        let has_remote = row.remote_port != 0
            && !row.remote_ip.is_unspecified()
            && (row.state == "ESTABLISHED"
                || row.state == "CLOSE_WAIT"
                || row.state == "TIME_WAIT");
        if has_remote && cfg.remote_allowed(row.remote_ip) {
            let label = remote_label(row.remote_ip, cfg.collapse_loopback);
            let remote_id = id_remote_host(node_id, &label);
            nodes.insert(
                remote_id.clone(),
                Node::RemoteHost {
                    addr: label,
                    rdns: None,
                },
            );
            edges.insert(Edge {
                from: sock_id,
                to: remote_id,
                kind: EdgeKind::ConnectsTo,
            });
        }
    }

    (nodes, edges)
}

/// Read procfs and build the current network subgraph (blocking).
fn collect(node_id: &str, cfg: &NetConfig) -> (HashMap<NodeId, Node>, HashSet<Edge>) {
    let tables = [
        ("/proc/net/tcp", "tcp", false),
        ("/proc/net/tcp6", "tcp6", true),
        ("/proc/net/udp", "udp", false),
        ("/proc/net/udp6", "udp6", true),
    ];
    let mut rows = Vec::new();
    for (path, proto, is_v6) in tables {
        if let Ok(content) = std::fs::read_to_string(path) {
            rows.extend(parse_net_table(&content, proto, is_v6));
        }
    }
    let inode_pid = inode_pid_map();
    build_graph(&rows, &inode_pid, node_id, cfg)
}

/// Compute the deltas to turn `prev` into `cur` (nodes then edges; upserts
/// before removes). Only changes are emitted → bounded event rate.
pub fn diff(
    prev_nodes: &HashMap<NodeId, Node>,
    cur_nodes: &HashMap<NodeId, Node>,
    prev_edges: &HashSet<Edge>,
    cur_edges: &HashSet<Edge>,
) -> Vec<Delta> {
    let mut out = Vec::new();
    for (id, node) in cur_nodes {
        if prev_nodes.get(id) != Some(node) {
            out.push(Delta::UpsertNode {
                id: id.clone(),
                node: node.clone(),
            });
        }
    }
    for edge in cur_edges {
        if !prev_edges.contains(edge) {
            out.push(Delta::UpsertEdge { edge: edge.clone() });
        }
    }
    for edge in prev_edges {
        if !cur_edges.contains(edge) {
            out.push(Delta::RemoveEdge { edge: edge.clone() });
        }
    }
    for id in prev_nodes.keys() {
        if !cur_nodes.contains_key(id) {
            out.push(Delta::RemoveNode { id: id.clone() });
        }
    }
    out
}

/// The network event source.
pub struct NetSource {
    pub config: NetConfig,
}

impl EventSource for NetSource {
    fn name(&self) -> &'static str {
        "net"
    }

    fn start(self: Box<Self>, node_id: String, tx: mpsc::Sender<Msg>) -> Result<()> {
        let cfg = self.config;
        tokio::spawn(run(cfg, node_id, tx));
        Ok(())
    }
}

async fn run(cfg: NetConfig, node_id: String, tx: mpsc::Sender<Msg>) {
    let mut prev_nodes: HashMap<NodeId, Node> = HashMap::new();
    let mut prev_edges: HashSet<Edge> = HashSet::new();
    let mut batch_id: u64 = 0;

    loop {
        let node_id2 = node_id.clone();
        let cfg2 = cfg.clone();
        let collected = tokio::task::spawn_blocking(move || collect(&node_id2, &cfg2)).await;
        let (cur_nodes, cur_edges) = match collected {
            Ok(v) => v,
            Err(_) => {
                tokio::time::sleep(cfg.poll_interval).await;
                continue;
            }
        };

        let deltas = diff(&prev_nodes, &cur_nodes, &prev_edges, &cur_edges);
        if !deltas.is_empty() {
            batch_id = batch_id.wrapping_add(1);
            if tx
                .send(Msg::Event {
                    delta: Delta::BatchBegin { id: batch_id },
                })
                .await
                .is_err()
            {
                return;
            }
            for delta in deltas {
                if tx.send(Msg::Event { delta }).await.is_err() {
                    return;
                }
            }
            let _ = tx
                .send(Msg::Event {
                    delta: Delta::BatchEnd { id: batch_id },
                })
                .await;
        }

        prev_nodes = cur_nodes;
        prev_edges = cur_edges;
        tokio::time::sleep(cfg.poll_interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TCP_FIXTURE: &str = "\
  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 100001 1 0000000000000000 100 0 0 10 0
   1: 0100007F:C350 0101A8C0:01BB 01 00000000:00000000 00:00000000 00000000  1000        0 100002 1 0000000000000000 20 0 0 10 0
";

    #[test]
    fn parses_socket_inode() {
        assert_eq!(parse_socket_inode("socket:[12345]"), Some(12345));
        assert_eq!(parse_socket_inode("anon_inode:[eventfd]"), None);
        assert_eq!(parse_socket_inode("/dev/null"), None);
    }

    #[test]
    fn parses_v4_address_little_endian() {
        // 0100007F → 127.0.0.1 ; 1F90 → 8080
        let (ip, port) = parse_endpoint("0100007F:1F90", false).unwrap();
        assert_eq!(ip, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
        assert_eq!(port, 8080);
    }

    #[test]
    fn parses_tcp_table() {
        let rows = parse_net_table(TCP_FIXTURE, "tcp", false);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].state, "LISTEN");
        assert_eq!(rows[0].local_port, 8080);
        assert_eq!(rows[0].inode, 100001);
        assert_eq!(rows[1].state, "ESTABLISHED");
        assert_eq!(rows[1].remote_ip, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
        assert_eq!(rows[1].remote_port, 443);
    }

    #[test]
    fn build_graph_links_process_socket_remote() {
        let rows = parse_net_table(TCP_FIXTURE, "tcp", false);
        let mut inode_pid = HashMap::new();
        inode_pid.insert(100001u64, 4242);
        inode_pid.insert(100002u64, 4242);
        let cfg = NetConfig::from_args(2, &[], &[]);
        let (nodes, edges) = build_graph(&rows, &inode_pid, "host", &cfg);

        // 2 sockets + 1 remote host (loopback established collapses? remote is
        // 192.168.1.1, not loopback) = 3 nodes.
        let sockets = nodes
            .values()
            .filter(|n| matches!(n, Node::Socket { .. }))
            .count();
        let remotes = nodes
            .values()
            .filter(|n| matches!(n, Node::RemoteHost { .. }))
            .count();
        assert_eq!(sockets, 2);
        assert_eq!(remotes, 1);

        assert!(edges.iter().any(|e| e.kind == EdgeKind::ListensOn));
        assert!(edges.iter().any(|e| e.kind == EdgeKind::ConnectsTo));
    }

    #[test]
    fn socket_without_pid_is_skipped() {
        let rows = parse_net_table(TCP_FIXTURE, "tcp", false);
        let cfg = NetConfig::from_args(2, &[], &[]);
        let (nodes, edges) = build_graph(&rows, &HashMap::new(), "host", &cfg);
        assert!(nodes.is_empty());
        assert!(edges.is_empty());
    }

    #[test]
    fn diff_is_empty_for_stable_graph() {
        let rows = parse_net_table(TCP_FIXTURE, "tcp", false);
        let mut inode_pid = HashMap::new();
        inode_pid.insert(100001u64, 4242);
        inode_pid.insert(100002u64, 4242);
        let cfg = NetConfig::from_args(2, &[], &[]);
        let (n1, e1) = build_graph(&rows, &inode_pid, "host", &cfg);
        let (n2, e2) = build_graph(&rows, &inode_pid, "host", &cfg);
        assert!(
            diff(&n1, &n2, &e1, &e2).is_empty(),
            "unchanged graph must emit no events"
        );
    }

    #[test]
    fn diff_reports_added_and_removed() {
        let rows = parse_net_table(TCP_FIXTURE, "tcp", false);
        let mut inode_pid = HashMap::new();
        inode_pid.insert(100001u64, 4242);
        inode_pid.insert(100002u64, 4242);
        let cfg = NetConfig::from_args(2, &[], &[]);
        let (full_n, full_e) = build_graph(&rows, &inode_pid, "host", &cfg);
        let empty_n = HashMap::new();
        let empty_e = HashSet::new();

        let added = diff(&empty_n, &full_n, &empty_e, &full_e);
        assert!(added.iter().any(|d| matches!(d, Delta::UpsertNode { .. })));
        let removed = diff(&full_n, &empty_n, &full_e, &empty_e);
        assert!(removed
            .iter()
            .any(|d| matches!(d, Delta::RemoveNode { .. })));
    }

    #[test]
    fn cidr_filters_remote_hosts() {
        let c = Cidr::parse("10.0.0.0/8").unwrap();
        assert!(c.contains("10.1.2.3".parse().unwrap()));
        assert!(!c.contains("192.168.1.1".parse().unwrap()));

        let cfg = NetConfig {
            poll_interval: Duration::from_secs(2),
            include: vec![],
            exclude: vec![Cidr::parse("192.168.0.0/16").unwrap()],
            collapse_loopback: true,
        };
        assert!(!cfg.remote_allowed("192.168.1.1".parse().unwrap()));
        assert!(cfg.remote_allowed("8.8.8.8".parse().unwrap()));
    }
}
