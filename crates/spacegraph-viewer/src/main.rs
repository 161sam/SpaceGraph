mod net;
mod state;

use bevy::prelude::*;
use bevy_egui::{EguiPlugin, egui};
use state::{GraphState, NetEvent};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        .insert_resource(GraphState::default())
        .add_event::<NetEvent>()
        .add_systems(Startup, setup)
        .add_systems(Update, (ui_panel, apply_net_events, render_graph_placeholder))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera3dBundle::default());
    // spawn net reader task (writes Bevy Events)
    net::spawn_reader("/tmp/spacegraph.sock");
}

fn ui_panel(mut contexts: bevy_egui::EguiContexts, mut state: ResMut<GraphState>) {
    egui::SidePanel::left("left").show(contexts.ctx_mut(), |ui| {
        ui.heading("SpaceGraph");
        ui.label(format!("nodes: {}", state.nodes.len()));
        ui.label(format!("edges: {}", state.edges.len()));
        ui.separator();
        ui.checkbox(&mut state.show_3d, "3D Ansicht");
        ui.text_edit_singleline(&mut state.filter);
    });
}

fn apply_net_events(mut ev: EventReader<NetEvent>, mut state: ResMut<GraphState>) {
    for e in ev.read() {
        state.apply(e);
    }
}

fn render_graph_placeholder() {
    // TODO: draw points/lines (Bevy meshes) from state
}
