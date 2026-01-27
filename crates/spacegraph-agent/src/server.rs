use anyhow::Result;
use spacegraph_core::Msg;
use tokio::net::UnixListener;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use futures_util::{SinkExt, StreamExt};

pub async fn run(
    sock_path: &str,
    snapshot: Msg,
    mut rx: tokio::sync::mpsc::Receiver<Msg>,
) -> Result<()> {
    let listener = UnixListener::bind(sock_path)?;
    println!("spacegraph-agent listening on {}", sock_path);

    loop {
        let (stream, _addr) = listener.accept().await?;
        println!("viewer connected");

        let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

        // Expect Hello, then allow snapshot request
        if let Some(Ok(bytes)) = framed.next().await {
            let msg: Msg = serde_json::from_slice(&bytes)?;
            println!("recv: {:?}", std::mem::discriminant(&msg));
        }

        // Send Hello back (optional)
        let hello = Msg::Hello { version: "0.1.0".into() };
        framed.send(serde_json::to_vec(&hello)?.into()).await?;

        // Simple: immediately send snapshot
        framed.send(serde_json::to_vec(&snapshot)?.into()).await?;

        // Stream deltas
        while let Some(msg) = rx.recv().await {
            framed.send(serde_json::to_vec(&msg)?.into()).await?;
        }
    }
}
