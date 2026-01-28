mod app;
mod graph;
mod net;
mod render;
mod ui;
mod util;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;

use crate::app::resources::{NetRx, NetTx};

fn main() {
    let (tx, rx) = crossbeam_channel::unbounded();

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "SpaceGraph (native)".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin)
        .insert_resource(NetRx(rx))
        .insert_resource(NetTx(tx))
        .add_plugins(app::SpaceGraphViewerPlugin)
        .run();
}
