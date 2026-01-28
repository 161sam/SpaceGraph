use crate::net::Incoming;
use anyhow::{Context, Result};
use crossbeam_channel::Sender;
use futures_util::{SinkExt, StreamExt};
use spacegraph_core::Msg;
use tokio::net::UnixStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

pub fn spawn_reader(sock_path: String, tx: Sender<Incoming>) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(async move {
            if let Err(e) = run(sock_path.clone(), tx.clone()).await {
                let _ = tx.send(Incoming::error(sock_path.clone(), format!("{e:?}")));
                let _ = tx.send(Incoming::disconnected(sock_path.clone()));
            }
        });
    });
}

async fn run(sock_path: String, tx: Sender<Incoming>) -> Result<()> {
    let stream = UnixStream::connect(&sock_path)
        .await
        .with_context(|| format!("connect UDS {sock_path}"))?;

    let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

    let _ = tx.send(Incoming::connected(sock_path.clone()));

    // Send hello (agent tolerates anything)
    let hello = Msg::Hello {
        version: "0.1.0".into(),
    };
    framed.send(serde_json::to_vec(&hello)?.into()).await?;

    while let Some(frame) = framed.next().await {
        let bytes = frame?;
        match serde_json::from_slice::<Msg>(&bytes) {
            Ok(m) => {
                let inc = match &m {
                    Msg::Identity { .. } => Incoming::identity(sock_path.clone(), m),
                    Msg::Snapshot { .. } => Incoming::snapshot(sock_path.clone(), m),
                    Msg::Event { .. } => Incoming::event(sock_path.clone(), m),
                    _ => Incoming::other(sock_path.clone(), m),
                };
                let _ = tx.send(inc);
            }
            Err(e) => {
                let _ = tx.send(Incoming::error(
                    sock_path.clone(),
                    format!("decode error: {e}"),
                ));
            }
        }
    }

    let _ = tx.send(Incoming::disconnected(sock_path.clone()));
    Ok(())
}
