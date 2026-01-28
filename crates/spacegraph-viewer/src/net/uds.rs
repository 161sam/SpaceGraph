use crate::net::Incoming;
use crossbeam_channel::Sender;
use futures_util::{SinkExt, StreamExt};
use spacegraph_core::Msg;
use tokio::net::UnixStream;
use tokio::sync::watch;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

#[derive(Clone)]
pub struct ReaderHandle {
    shutdown: watch::Sender<bool>,
}

impl ReaderHandle {
    pub fn shutdown(&self) {
        let _ = self.shutdown.send(true);
    }
}

pub fn spawn_reader(stream_name: String, sock_path: String, tx: Sender<Incoming>) -> ReaderHandle {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(async move {
            run(stream_name, sock_path, tx.clone(), shutdown_rx).await;
        });
    });

    ReaderHandle {
        shutdown: shutdown_tx,
    }
}

async fn run(
    stream_name: String,
    sock_path: String,
    tx: Sender<Incoming>,
    mut shutdown: watch::Receiver<bool>,
) {
    let stream = match tokio::select! {
        _ = shutdown.changed() => {
            return;
        }
        res = UnixStream::connect(&sock_path) => res,
    } {
        Ok(stream) => stream,
        Err(err) => {
            let _ = tx.send(Incoming::error(
                stream_name.clone(),
                format!("connect UDS {sock_path}: {err}"),
            ));
            let _ = tx.send(Incoming::disconnected(stream_name.clone()));
            return;
        }
    };

    let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

    let _ = tx.send(Incoming::connected(stream_name.clone()));

    // Send hello (agent tolerates anything)
    let hello = Msg::Hello {
        version: "0.1.0".into(),
    };
    let hello_bytes = match serde_json::to_vec(&hello) {
        Ok(bytes) => bytes,
        Err(err) => {
            let _ = tx.send(Incoming::error(
                stream_name.clone(),
                format!("encode hello: {err}"),
            ));
            let _ = tx.send(Incoming::disconnected(stream_name.clone()));
            return;
        }
    };
    if let Err(err) = framed.send(hello_bytes.into()).await {
        let _ = tx.send(Incoming::error(
            stream_name.clone(),
            format!("send hello: {err}"),
        ));
        let _ = tx.send(Incoming::disconnected(stream_name.clone()));
        return;
    }

    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                break;
            }
            frame = framed.next() => {
                match frame {
                    Some(Ok(bytes)) => {
                        match serde_json::from_slice::<Msg>(&bytes) {
                            Ok(m) => {
                                let inc = match &m {
                                    Msg::Identity { .. } => Incoming::identity(stream_name.clone(), m),
                                    Msg::Snapshot { .. } => Incoming::snapshot(stream_name.clone(), m),
                                    Msg::Event { .. } => Incoming::event(stream_name.clone(), m),
                                    _ => Incoming::other(stream_name.clone(), m),
                                };
                                let _ = tx.send(inc);
                            }
                            Err(e) => {
                                let _ = tx.send(Incoming::error(
                                    stream_name.clone(),
                                    format!("decode error: {e}"),
                                ));
                            }
                        }
                    }
                    Some(Err(err)) => {
                        let _ = tx.send(Incoming::error(
                            stream_name.clone(),
                            format!("stream error: {err}"),
                        ));
                        break;
                    }
                    None => {
                        break;
                    }
                }
            }
        }
    }

    let _ = tx.send(Incoming::disconnected(stream_name.clone()));
}
