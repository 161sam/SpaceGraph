# SpaceGraph — Graph Schema

The wire/graph schema is defined in `crates/spacegraph-core/src/lib.rs` and is the
contract shared by the agent (producer) and the viewer (consumer). This doc
mirrors that source; `lib.rs` is authoritative.

**`PROTOCOL_VERSION = 3`** — exchanged in the `Hello` handshake; a mismatch is
rejected.

## Nodes (`Node`, tagged `{type, data}`)

| Variant | Fields |
|---|---|
| `Process` | `pid: i32`, `ppid: i32`, `exe: String`, `cmdline: String`, `uid: u32` |
| `File` | `path: String`, `inode: u64`, `kind: FileKind` |
| `User` | `uid: u32`, `name: String` |
| `Socket` | `proto: String`, `local_addr: String`, `local_port: u16`, `state: String` |
| `RemoteHost` | `addr: String`, `rdns: Option<String>` |
| `Alert` | `source: String`, `signature: String`, `severity: String`, `ts: String` |

`FileKind`: `Regular`, `Dir`, `Socket`, `Pipe`, `Device`, `Unknown`.

Node identity is a string `NodeId(String)`, globally unique via the id
constructors `id_process`/`id_user`/`id_file`/`id_socket`/`id_remote_host`/
`id_alert`. Multi-stream graphs prefix ids per stream (`graph/namespace.rs`).

## Edges (`Edge { from: NodeId, to: NodeId, kind: EdgeKind }`)

| `EdgeKind` | Fields | Meaning |
|---|---|---|
| `Opens` | `fd: i32`, `mode: String` | process opened a file |
| `Execs` | — | process execs a file |
| `RunsAs` | — | process runs as a user |
| `OwnsSocket` | — | process owns a socket |
| `ConnectsTo` | — | socket connects to a remote host |
| `ListensOn` | — | process listens on a socket |
| `AlertsOn` | — | alert raised on an entity |

The viewer aggregates parallel edges into an `AggEdge` keyed by
`AggEdgeKey{from,to,class}` (`EdgeKindClass`), with first/last timestamps and
counts (`graph/model.rs`).

## Wire protocol (`Msg`, tagged `{type, data}`)

`Hello{version,protocol}` · `Identity{ident,caps}` · `RequestSnapshot` ·
`Snapshot{nodes,edges}` · `Event{delta}` · `Ping` · `Pong`.

`Delta`: `BatchBegin{id}` · `BatchEnd{id}` · `UpsertNode{id,node}` ·
`RemoveNode{id}` · `UpsertEdge{edge}` · `RemoveEdge{edge}`.

`Capabilities{procfs, fd_edges, fs_notify, proc_poll, ebpf, cloud, windows: bool}`;
`NodeIdentity{node_id, hostname, platform, arch}`.
