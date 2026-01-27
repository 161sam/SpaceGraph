mod app;
mod graph;
mod net;
mod render;
mod ui;
mod util;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;

use crate::app::resources::NetRx;

fn sock_path() -> String {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        format!("{dir}/spacegraph.sock")
    } else {
        "/tmp/spacegraph.sock".to_string()
    }
}

fn main() {
    let (tx, rx) = crossbeam_channel::unbounded();
    net::spawn_reader(sock_path(), tx);

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
        .add_plugins(app::SpaceGraphViewerPlugin)
        .run();
}
