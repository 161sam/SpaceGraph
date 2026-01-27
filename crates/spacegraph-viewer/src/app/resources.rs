use bevy::prelude::Resource;
use crossbeam_channel::Receiver;

use crate::net::Incoming;

#[derive(Resource)]
pub struct NetRx(pub Receiver<Incoming>);
