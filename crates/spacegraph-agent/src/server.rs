use anyhow::Result;
use spacegraph_core::{protocol_compatible, Msg, PROTOCOL_VERSION};
use std::sync::Arc;

use crate::index::FsIndex;

#[cfg(unix)]
use anyhow::Context;
#[cfg(unix)]
use futures_util::{SinkExt, StreamExt};
#[cfg(unix)]
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(unix)]
use tokio::net::UnixListener;
#[cfg(unix)]
use tokio_util::codec::{Framed, LengthDelimitedCodec};

#[cfg(unix)]
pub async fn run(
    sock_path: &str,
    identity_msg: Msg,
    snapshot_msg: Msg,
    snapshot_node_events: Vec<Msg>,
    bus_tx: tokio::sync::broadcast::Sender<Msg>,
    index: Arc<FsIndex>,
) -> Result<()> {
    let listener =
        UnixListener::bind(sock_path).with_context(|| format!("bind UDS {sock_path}"))?;
    let active_clients = AtomicUsize::new(0);
    let (snapshot_nodes_count, snapshot_edges_count) = match &snapshot_msg {
        Msg::Snapshot { nodes, edges } => (nodes.len(), edges.len()),
        _ => (0, 0),
    };

    // Restrict perms: 0600
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(sock_path, std::fs::Permissions::from_mode(0o600));
    }

    tracing::info!(uds_path = %sock_path, "listening");

    loop {
        let (stream, _) = listener.accept().await?;
        let mut framed = Framed::new(stream, LengthDelimitedCodec::new());
        let client_count = active_clients.fetch_add(1, Ordering::SeqCst) + 1;
        tracing::info!(count = client_count, "client_connected");

        // Per-connection receiver
        let mut bus_rx = bus_tx.subscribe();

        // Expect optional hello/request; reject only an *incompatible* peer.
        // A v3 viewer stays compatible (graph-only) — never break it silently.
        if let Some(Ok(bytes)) = framed.next().await {
            if let Ok(Msg::Hello { protocol, .. }) = serde_json::from_slice::<Msg>(&bytes) {
                if !protocol_compatible(protocol) {
                    tracing::warn!(
                        client_protocol = protocol,
                        expected = PROTOCOL_VERSION,
                        "protocol_incompatible: closing client"
                    );
                    active_clients.fetch_sub(1, Ordering::SeqCst);
                    continue;
                }
            }
        }

        // Send hello + identity + snapshot
        framed
            .send(
                serde_json::to_vec(&Msg::Hello {
                    version: env!("CARGO_PKG_VERSION").into(),
                    protocol: PROTOCOL_VERSION,
                })?
                .into(),
            )
            .await?;
        framed
            .send(serde_json::to_vec(&identity_msg)?.into())
            .await?;
        framed
            .send(serde_json::to_vec(&snapshot_msg)?.into())
            .await?;
        for msg in snapshot_node_events.iter() {
            framed.send(serde_json::to_vec(msg)?.into()).await?;
        }
        tracing::info!(
            nodes = snapshot_nodes_count,
            edges = snapshot_edges_count,
            "sent_snapshot"
        );

        // Split so we can read client requests (FS search/materialise, v4) and
        // forward bus deltas over the same connection without a borrow conflict.
        let (mut sink, mut stream) = framed.split();

        // Stream deltas + serve client requests.
        loop {
            tokio::select! {
                inbound = stream.next() => match inbound {
                    Some(Ok(bytes)) => match serde_json::from_slice::<Msg>(&bytes) {
                        Ok(Msg::SearchRequest(req)) => {
                            // The walker scan is blocking — run it off the async
                            // worker.
                            let idx = Arc::clone(&index);
                            match tokio::task::spawn_blocking(move || idx.search(&req)).await {
                                Ok(resp) => {
                                    let out = serde_json::to_vec(&Msg::SearchResponse(resp))?;
                                    if sink.send(out.into()).await.is_err() {
                                        break;
                                    }
                                }
                                Err(err) => tracing::warn!(error = %err, "search task failed"),
                            }
                        }
                        Ok(Msg::MaterialiseRequest(req)) => {
                            // Only a picked, permitted path materialises (bounded).
                            for delta in index.materialise(&req) {
                                let out = serde_json::to_vec(&Msg::Event { delta })?;
                                if sink.send(out.into()).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Ok(Msg::Ping) => {
                            if sink.send(serde_json::to_vec(&Msg::Pong)?.into()).await.is_err() {
                                break;
                            }
                        }
                        Ok(_) => {} // ignore other client messages (re-sent Hello, etc.)
                        Err(err) => tracing::debug!(error = %err, "decode client frame"),
                    },
                    Some(Err(err)) => {
                        tracing::debug!(error = %err, "client stream error");
                        break;
                    }
                    None => break, // client closed
                },
                bus = bus_rx.recv() => match bus {
                    Ok(msg) => {
                        if sink.send(serde_json::to_vec(&msg)?.into()).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                },
            }
        }
        let client_count = active_clients.fetch_sub(1, Ordering::SeqCst) - 1;
        tracing::info!(count = client_count, "client_disconnected");
    }
}

#[cfg(not(unix))]
pub async fn run(
    _sock_path: &str,
    _identity_msg: Msg,
    _snapshot_msg: Msg,
    _snapshot_node_events: Vec<Msg>,
    _bus_tx: tokio::sync::broadcast::Sender<Msg>,
    _index: Arc<FsIndex>,
) -> Result<()> {
    anyhow::bail!("UDS server is only supported on unix platforms")
}
