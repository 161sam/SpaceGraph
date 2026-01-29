use anyhow::Result;
use spacegraph_core::Msg;

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

        // Expect optional hello/request, but MVP is tolerant.
        if let Some(Ok(bytes)) = framed.next().await {
            let _ = serde_json::from_slice::<Msg>(&bytes); // ignore errors
        }

        // Send hello + identity + snapshot
        framed
            .send(
                serde_json::to_vec(&Msg::Hello {
                    version: "0.1.0".into(),
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

        // Stream deltas
        loop {
            match bus_rx.recv().await {
                Ok(msg) => {
                    if framed.send(serde_json::to_vec(&msg)?.into()).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
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
) -> Result<()> {
    anyhow::bail!("UDS server is only supported on unix platforms")
}
