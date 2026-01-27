use bevy::prelude::Event;
use spacegraph_core::NodeId;

#[derive(Event)]
pub struct Picked(pub NodeId);
