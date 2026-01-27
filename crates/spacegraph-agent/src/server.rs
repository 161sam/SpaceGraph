use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use spacegraph_core::Msg;
use tokio::net::UnixListener;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

pub async fn run(
    sock_path: &str,
    identity_msg: Msg,
    snapshot_msg: Msg,
    bus_tx: tokio::sync::broadcast::Sender<Msg>,
) -> Result<()> {
    let listener = UnixListener::bind(sock_path).with_context(|| format!("bind UDS {sock_path}"))?;

    // Restrict perms: 0600
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(sock_path, std::fs::Permissions::from_mode(0o600));
    }

    eprintln!("spacegraph-agent listening on {sock_path}");

    loop {
        let (stream, _) = listener.accept().await?;
        let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

        // Per-connection receiver
        let mut bus_rx = bus_tx.subscribe();

        // Expect optional hello/request, but MVP is tolerant.
        if let Some(Ok(bytes)) = framed.next().await {
            let _ = serde_json::from_slice::<Msg>(&bytes); // ignore errors
        }

        // Send hello + identity + snapshot
        framed.send(serde_json::to_vec(&Msg::Hello { version: "0.1.0".into() })?.into()).await?;
        framed.send(serde_json::to_vec(&identity_msg)?.into()).await?;
        framed.send(serde_json::to_vec(&snapshot_msg)?.into()).await?;

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
    }
}
