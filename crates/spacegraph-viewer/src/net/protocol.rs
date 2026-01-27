use spacegraph_core::Msg;

#[derive(Debug, Clone)]
pub enum Incoming {
    Identity(Msg),
    Snapshot(Msg),
    Event(Msg),
    Other(Msg),
    Error(String),
}
