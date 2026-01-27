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
            if let Err(e) = run(sock_path, tx.clone()).await {
                let _ = tx.send(Incoming::Error(format!("{e:?}")));
            }
        });
    });
}

async fn run(sock_path: String, tx: Sender<Incoming>) -> Result<()> {
    let stream = UnixStream::connect(&sock_path)
        .await
        .with_context(|| format!("connect UDS {sock_path}"))?;

    let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

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
                    Msg::Identity { .. } => Incoming::Identity(m),
                    Msg::Snapshot { .. } => Incoming::Snapshot(m),
                    Msg::Event { .. } => Incoming::Event(m),
                    _ => Incoming::Other(m),
                };
                let _ = tx.send(inc);
            }
            Err(e) => {
                let _ = tx.send(Incoming::Error(format!("decode error: {e}")));
            }
        }
    }
    Ok(())
}
