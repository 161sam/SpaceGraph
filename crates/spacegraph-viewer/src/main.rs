mod net;
mod state;

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use crossbeam_channel::Receiver;
use state::{GraphState, Incoming, NodeMarker};
use std::collections::HashSet;
use std::sync::atomic::Ordering;

fn sock_path() -> String {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        format!("{dir}/spacegraph.sock")
    } else {
        "/tmp/spacegraph.sock".to_string()
    }
}

#[derive(Resource)]
struct NetRx(Receiver<Incoming>);

#[derive(Event)]
struct Picked(spacegraph_core::NodeId);

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
        .add_event::<Picked>()
        .insert_resource(NetRx(rx))
        .insert_resource(GraphState::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                ui_panel,
                pump_network,
                tick_glow,
                picking_focus,
                apply_picked_focus,
                update_layout_and_forces,
                draw_graph,
            ),
        )
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 5000.0,
            shadows_enabled: true,
            ..default()
        },
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

        // Legend
        ui.horizontal(|ui| {
            ui.label("Legend:");
            ui.label("● node");
            ui.label("— edge");
            ui.label("✨ glowing = recent change");
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.checkbox(&mut st.show_3d, "3D");
            ui.checkbox(&mut st.show_edges, "Edges");
        });

        ui.add_space(8.0);
        ui.label("Filter (substring):");
        ui.text_edit_singleline(&mut st.filter);

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Focus hops:");
            ui.add(egui::Slider::new(&mut st.focus_hops, 1..=8));
        });

        if let Some(f) = &st.focus {
            ui.label(format!("Focus: {}", f.0));
            if ui.button("Clear focus").clicked() {
                st.focus = None;
                st.needs_redraw.store(true, Ordering::Relaxed);
            }
        } else {
            ui.label("Focus: (none) — click a node");
        }

        ui.add_space(8.0);
        ui.separator();
        ui.heading("Performance");
        ui.add(egui::Slider::new(&mut st.max_visible_nodes, 200..=5000).text("max visible nodes"));
        ui.add(egui::Slider::new(&mut st.progressive_nodes_per_frame, 50..=2000).text("progressive/frame"));

        ui.add_space(8.0);
        ui.separator();
        ui.heading("Layout");
        ui.checkbox(&mut st.layout_force, "Force layout");
        ui.add(egui::Slider::new(&mut st.link_distance, 1.0..=20.0).text("link dist"));
        ui.add(egui::Slider::new(&mut st.repulsion, 0.0..=80.0).text("repulsion"));
        ui.add(egui::Slider::new(&mut st.damping, 0.80..=0.999).text("damping"));
        ui.add(egui::Slider::new(&mut st.max_step, 0.05..=1.5).text("max step"));

        ui.add_space(8.0);
        ui.separator();
        ui.heading("Glow");
        let mut ms = st.glow_duration.as_millis() as i32;
        ui.add(egui::Slider::new(&mut ms, 100..=3000).text("glow ms"));
        st.glow_duration = std::time::Duration::from_millis(ms as u64);

        ui.add_space(8.0);
        if ui.button("Clear graph").clicked() {
            st.clear();
        }
    });
}

fn pump_network(mut st: ResMut<GraphState>, rx: Res<NetRx>) {
    for msg in rx.0.try_iter().take(100_000) {
        st.apply(msg);
    }
}

fn tick_glow(mut st: ResMut<GraphState>) {
    st.tick_glow();
}

fn picking_focus(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
    st: Res<GraphState>,
    mut out: EventWriter<Picked>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok(window) = windows.get_single() else { return; };
    let Some(cursor) = window.cursor_position() else { return; };
    let Ok((camera, cam_tf)) = cam_q.get_single() else { return; };

    let mut st_tmp = st.clone(); // read-only, but we need cap calc; cheap clone of small fields? Actually GraphState huge.
    // Avoid cloning: compute visible by iterating nodes/edges directly:
    // We'll approximate: pick among currently positioned nodes (which are already capped/progressive).
    let mut best: Option<(f32, spacegraph_core::NodeId)> = None;

    for (id, pos) in st.positions.iter() {
        let Some(screen) = camera.world_to_viewport(cam_tf, *pos) else { continue; };
        let d = screen.distance(cursor);
        if d < 14.0 {
            if best.as_ref().map(|(bd, _)| d < *bd).unwrap_or(true) {
                best = Some((d, id.clone()));
            }
        }
    }

    if let Some((_, picked)) = best {
        out.send(Picked(picked));
    }
}

fn apply_picked_focus(mut st: ResMut<GraphState>, mut ev: EventReader<Picked>) {
    for Picked(id) in ev.read() {
        st.focus = Some(id.clone());
        st.needs_redraw.store(true, Ordering::Relaxed);
    }
}

fn update_layout_and_forces(time: Res<Time>, mut st: ResMut<GraphState>) {
    let vis: HashSet<_> = st.visible_set_capped();

    // Progressive init: only prepare some nodes each frame
    st.progressive_prepare(&vis);

    let dt = time.delta_seconds().min(0.033);
    st.force_step(&vis, dt);
}

fn draw_graph(
    mut commands: Commands,
    mut st: ResMut<GraphState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(Entity, &NodeMarker)>,
    mut gizmos: Gizmos,
) {
    let vis: HashSet<_> = st.visible_set_capped();

    if st.needs_redraw.swap(false, Ordering::Relaxed) {
        for (e, _) in query.iter_mut() {
            commands.entity(e).despawn_recursive();
        }

        let sphere = meshes.add(Sphere::new(0.28));

        let mat_norm = mats.add(StandardMaterial::default());
        let mut mat_glow = StandardMaterial::default();
        mat_glow.emissive = Color::rgb(1.0, 1.0, 1.0);
        let mat_glow = mats.add(mat_glow);

        for (id, node) in st.nodes.iter() {
            if !vis.contains(id) {
                continue;
            }
            if !st.passes_filter(id, node) {
                continue;
            }
            let Some(pos) = st.positions.get(id).cloned() else { continue; };

            let use_glow = st.node_is_glowing(id);

            commands.spawn((
                PbrBundle {
                    mesh: sphere.clone(),
                    material: if use_glow { mat_glow.clone() } else { mat_norm.clone() },
                    transform: Transform::from_translation(pos),
                    ..default()
                },
                NodeMarker { id: id.0.clone() },
            ));
        }
    }

    if st.show_edges {
        for e in st.edges.iter() {
            if !st.edge_visible(e, &vis) {
                continue;
            }
            let (Some(a), Some(b)) = (st.positions.get(&e.from), st.positions.get(&e.to)) else { continue; };

            // edge glow: draw twice if glowing (white line already; glow via second line slightly offset)
            if st.edge_is_glowing(e) {
                gizmos.line(*a, *b, Color::rgb(1.0, 1.0, 1.0));
            }
            gizmos.line(*a, *b, Color::WHITE);
        }
    }
}
