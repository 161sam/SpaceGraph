use crate::net::Incoming;
use crossbeam_channel::Sender;
use futures_util::{SinkExt, StreamExt};
use spacegraph_core::{protocol_compatible, Msg, PROTOCOL_VERSION};
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

pub fn spawn_reader(
    stream_name: String,
    sock_path: String,
    tx: Sender<Incoming>,
    outbound_rx: tokio::sync::mpsc::Receiver<Msg>,
) -> ReaderHandle {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(async move {
            run(stream_name, sock_path, tx.clone(), shutdown_rx, outbound_rx).await;
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
    mut outbound_rx: tokio::sync::mpsc::Receiver<Msg>,
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

    // Send hello with our protocol version.
    let hello = Msg::Hello {
        version: env!("CARGO_PKG_VERSION").into(),
        protocol: PROTOCOL_VERSION,
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
            out = outbound_rx.recv() => {
                match out {
                    Some(msg) => {
                        // Viewer → agent request (FS SearchRequest / Materialise).
                        match serde_json::to_vec(&msg) {
                            Ok(bytes) => {
                                if framed.send(bytes.into()).await.is_err() {
                                    break;
                                }
                            }
                            Err(err) => {
                                let _ = tx.send(Incoming::error(
                                    stream_name.clone(),
                                    format!("encode outbound: {err}"),
                                ));
                            }
                        }
                    }
                    None => {
                        // The outbound sender was dropped (stream torn down).
                        break;
                    }
                }
            }
            frame = framed.next() => {
                match frame {
                    Some(Ok(bytes)) => {
                        match serde_json::from_slice::<Msg>(&bytes) {
                            Ok(m) => {
                                let inc = match &m {
                                    Msg::Hello { protocol, .. } if !protocol_compatible(*protocol) => {
                                        let _ = tx.send(Incoming::error(
                                            stream_name.clone(),
                                            format!(
                                                "protocol incompatible: agent v{protocol}, viewer v{PROTOCOL_VERSION}"
                                            ),
                                        ));
                                        let _ = tx.send(Incoming::disconnected(stream_name.clone()));
                                        return;
                                    }
                                    Msg::Identity { .. } => Incoming::identity(stream_name.clone(), m),
                                    Msg::Snapshot { .. } => Incoming::snapshot(stream_name.clone(), m),
                                    Msg::Event { .. } => Incoming::event(stream_name.clone(), m),
                                    Msg::SearchResponse(_) => {
                                        Incoming::search_response(stream_name.clone(), m)
                                    }
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
