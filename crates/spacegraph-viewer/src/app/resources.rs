use bevy::prelude::Resource;
use crossbeam_channel::{Receiver, Sender};

use crate::net::Incoming;

#[derive(Resource)]
pub struct NetRx(pub Receiver<Incoming>);

#[derive(Resource, Clone)]
pub struct NetTx(pub Sender<Incoming>);
