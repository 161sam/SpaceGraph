mod net;
mod state;

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, egui};
use crossbeam_channel::Receiver;
use state::{GraphState, Incoming};

fn sock_path() -> String {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        format!("{dir}/spacegraph.sock")
    } else {
        "/tmp/spacegraph.sock".to_string()
    }
}

#[derive(Resource)]
struct NetRx(Receiver<Incoming>);

fn main() {
    // Wayland: nothing special needed, but we keep defaults minimal.
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
        .insert_resource(GraphState::default())
        .add_systems(Startup, setup)
        .add_systems(Update, (ui_panel, pump_network, update_layout, draw_graph))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(PointLightBundle {
        point_light: PointLight { intensity: 5000.0, shadows_enabled: true, ..default() },
        transform: Transform::from_xyz(10.0, 20.0, 10.0),
        ..default()
    });

    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 18.0, 28.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
}

fn ui_panel(mut contexts: EguiContexts, mut st: ResMut<GraphState>) {
    egui::SidePanel::left("left").show(contexts.ctx_mut(), |ui| {
        ui.heading("SpaceGraph");
        ui.label(format!("nodes: {}", st.nodes.len()));
        ui.label(format!("edges: {}", st.edges.len()));
        ui.separator();

        ui.horizontal(|ui| {
            ui.checkbox(&mut st.show_3d, "3D");
            ui.checkbox(&mut st.show_edges, "Edges");
        });

        ui.add_space(8.0);
        ui.label("Filter (substring auf NodeId oder File path):");
        ui.text_edit_singleline(&mut st.filter);

        ui.add_space(8.0);
        ui.label("Layout:");
        ui.add(egui::Slider::new(&mut st.radius, 5.0..=60.0).text("radius"));
        ui.add(egui::Slider::new(&mut st.y_spread, 0.0..=20.0).text("y-spread"));

        ui.add_space(8.0);
        if ui.button("Clear").clicked() {
            st.clear();
        }
    });
}

fn pump_network(mut st: ResMut<GraphState>, rx: Res<NetRx>) {
    // Drain messages quickly each frame
    for msg in rx.0.try_iter().take(10_000) {
        st.apply(msg);
    }
}

fn update_layout(mut st: ResMut<GraphState>) {
    st.recompute_positions_if_dirty();
}

fn draw_graph(
    mut commands: Commands,
    mut st: ResMut<GraphState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(Entity, &state::NodeMarker)>,
    mut gizmos: Gizmos,
) {
    // Remove previously spawned node entities if we need a redraw
    if st.needs_redraw.swap(false, std::sync::atomic::Ordering::Relaxed) {
        for (e, _) in query.iter_mut() {
            commands.entity(e).despawn_recursive();
        }

        // Spawn nodes
        let sphere = meshes.add(Sphere::new(0.25));
        let mat = mats.add(StandardMaterial::default());

        for (id, node) in st.nodes.iter() {
            if !st.passes_filter(id, node) {
                continue;
            }
            let pos = st.positions.get(id).cloned().unwrap_or(Vec3::ZERO);

            commands.spawn((
                PbrBundle {
                    mesh: sphere.clone(),
                    material: mat.clone(),
                    transform: Transform::from_translation(pos),
                    ..default()
                },
                state::NodeMarker { id: id.0.clone() },
            ));
        }
    }

    // Draw edges via gizmos (cheap)
    if st.show_edges {
        for e in st.edges.iter() {
            if let (Some(a), Some(b)) = (st.positions.get(&e.from), st.positions.get(&e.to)) {
                if st.passes_edge_filter(e) {
                    gizmos.line(*a, *b, Color::WHITE);
                }
            }
        }
    }
}
