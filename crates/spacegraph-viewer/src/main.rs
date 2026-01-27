mod net;
mod state;

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use crossbeam_channel::Receiver;
use state::{GraphState, Incoming, NodeMarker, ViewMode};
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::time::Instant;

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
                pump_network,
                ui_panel,
                hud_overlay,
                tick_housekeeping,
                hover_detection,
                picking_focus,
                apply_picked_focus,
                update_layout_or_timeline,
                draw_scene,
                apply_jump_to,
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

fn pump_network(mut st: ResMut<GraphState>, rx: Res<NetRx>) {
    for msg in rx.0.try_iter().take(100_000) {
        st.apply(msg);
    }
}

fn tick_housekeeping(time: Res<Time>, mut st: ResMut<GraphState>) {
    let dt = time.delta_seconds().max(0.0001);
    st.fps = 1.0 / dt;

    st.tick_glow();
    st.tick_metrics(Instant::now());
    st.tick_gc();

    // timeline housekeeping
    st.tick_timeline();
}

fn ui_panel(mut contexts: EguiContexts, mut st: ResMut<GraphState>) {
    egui::SidePanel::left("left").show(contexts.ctx_mut(), |ui| {
        ui.heading("SpaceGraph");
        ui.label(format!("nodes: {}", st.nodes.len()));
        ui.label(format!("edges: {}", st.edges.len()));
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("View:");
            ui.selectable_value(&mut st.view_mode, ViewMode::Spatial, "Spatial");
            ui.selectable_value(&mut st.view_mode, ViewMode::Timeline, "Timeline");
        });

        if st.view_mode == ViewMode::Timeline {
            ui.add_space(6.0);
            ui.heading("Timeline / Feynman");
            ui.horizontal(|ui| {
                let paused = st.timeline_pause;
                let mut p = paused;
                ui.checkbox(&mut p, "Pause");
                if p != paused {
                    st.set_timeline_pause(p);
                }
            });
            let mut w = st.timeline_window.as_secs() as i32;
            ui.add(egui::Slider::new(&mut w, 5..=240).text("window (s)"));
            st.timeline_window = std::time::Duration::from_secs(w as u64);

            ui.add(egui::Slider::new(&mut st.timeline_scale, 0.05..=1.5).text("x scale"));
            ui.label(format!("events buffered: {}", st.timeline_events.len()));
        }

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
            ui.add(egui::Slider::new(&mut st.focus_hops, 1..=10));
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
        ui.add(egui::Slider::new(&mut st.max_visible_nodes, 200..=10_000).text("max visible nodes"));
        ui.add(egui::Slider::new(&mut st.progressive_nodes_per_frame, 50..=4000).text("progressive/frame"));

        ui.add_space(8.0);
        ui.separator();
        ui.heading("Layout (Spatial)");
        ui.checkbox(&mut st.layout_force, "Force layout");
        ui.add(egui::Slider::new(&mut st.link_distance, 1.0..=20.0).text("link dist"));
        ui.add(egui::Slider::new(&mut st.repulsion, 0.0..=120.0).text("repulsion"));
        ui.add(egui::Slider::new(&mut st.damping, 0.80..=0.999).text("damping"));
        ui.add(egui::Slider::new(&mut st.max_step, 0.05..=2.0).text("max step"));

        ui.add_space(8.0);
        ui.separator();
        ui.heading("Glow");
        let mut ms = st.glow_duration.as_millis() as i32;
        ui.add(egui::Slider::new(&mut ms, 100..=3000).text("glow ms"));
        st.glow_duration = std::time::Duration::from_millis(ms as u64);

        ui.add_space(8.0);
        ui.separator();
        ui.heading("GC");
        ui.checkbox(&mut st.gc_enabled, "enabled");
        let mut ttl = st.gc_ttl.as_secs() as i32;
        ui.add(egui::Slider::new(&mut ttl, 1..=600).text("orphan TTL (s)"));
        st.gc_ttl = std::time::Duration::from_secs(ttl as u64);

        ui.add_space(10.0);
        ui.separator();
        ui.heading("Search");
        ui.label("Ctrl+P opens search overlay.");
        if ui.button("Open Search (Ctrl+P)").clicked() {
            st.search_open = true;
        }

        ui.add_space(10.0);
        ui.separator();
        if ui.button("Clear graph").clicked() {
            st.clear();
        }
    });
}

fn hud_overlay(mut contexts: EguiContexts, st: Res<GraphState>) {
    egui::Area::new("hud")
        .fixed_pos(egui::pos2(10.0, 10.0))
        .show(contexts.ctx_mut(), |ui| {
            ui.group(|ui| {
                ui.label(format!("FPS: {:.0}", st.fps));
                ui.label(format!("Visible: {} nodes / {} edges", st.visible_nodes, st.visible_edges));
                ui.label(format!("Event rate: {:.1}/s", st.event_rate));
                ui.label(format!("Total msgs: {}", st.event_total));
                if let Some(id) = st.last_batch_id {
                    ui.label(format!("Last batch: {}", id));
                }
                ui.label(format!(
                    "Mode: {}",
                    if st.view_mode == ViewMode::Spatial { "Spatial" } else { "Timeline" }
                ));
            });
        });
}

// Hover (spatial only, based on projected positions)
fn hover_detection(
    windows: Query<&Window>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
    mut contexts: EguiContexts,
    mut st: ResMut<GraphState>,
) {
    if st.view_mode != ViewMode::Spatial {
        st.hovered = None;
        return;
    }

    let Ok(window) = windows.get_single() else { return; };
    let Some(cursor) = window.cursor_position() else { st.hovered = None; return; };
    let Ok((camera, cam_tf)) = cam_q.get_single() else { return; };

    if contexts.ctx_mut().wants_pointer_input() {
        return;
    }

    let mut best: Option<(f32, spacegraph_core::NodeId)> = None;
    for (id, pos) in st.positions.iter() {
        let Some(screen) = camera.world_to_viewport(cam_tf, *pos) else { continue; };
        let d = screen.distance(cursor);
        if d < 18.0 {
            if best.as_ref().map(|(bd, _)| d < *bd).unwrap_or(true) {
                best = Some((d, id.clone()));
            }
        }
    }
    st.hovered = best.map(|(_, id)| id);
}

// Ctrl+P search overlay (same as v0.1.5)
fn search_overlay(mut contexts: EguiContexts, mut st: ResMut<GraphState>) {
    let ctx = contexts.ctx_mut();

    if ctx.input(|i| i.key_pressed(egui::Key::P) && i.modifiers.ctrl) {
        st.search_open = true;
    }
    if !st.search_open {
        return;
    }

    egui::Window::new("Search / Jump (Ctrl+P)")
        .collapsible(false)
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Query:");
                let resp = ui.text_edit_singleline(&mut st.search_query);
                if resp.changed() {
                    st.recompute_search_hits(30);
                }
                if ui.button("Close (Esc)").clicked() {
                    st.search_open = false;
                }
            });

            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                st.search_open = false;
            }

            ui.separator();
            ui.label("Hits:");
            ui.add_space(4.0);

            let mut picked: Option<spacegraph_core::NodeId> = None;
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for id in st.search_hits.iter() {
                    let label = if let Some(node) = st.nodes.get(id) {
                        match node {
                            spacegraph_core::Node::File { path, .. } => format!("file: {} ({})", path, id.0),
                            spacegraph_core::Node::Process { cmdline, pid, .. } => format!("proc: pid={pid} {} ({})", cmdline, id.0),
                            spacegraph_core::Node::User { name, uid } => format!("user: {name} uid={uid} ({})", id.0),
                        }
                    } else {
                        id.0.clone()
                    };
                    if ui.selectable_label(false, label).clicked() {
                        picked = Some(id.clone());
                    }
                }
            });

            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Some(first) = st.search_hits.first() {
                    picked = Some(first.clone());
                }
            }

            if let Some(id) = picked {
                st.request_jump(id.clone());
                st.selected = Some(id);
                st.search_open = false;
            }
        });
}

fn picking_focus(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
    mut contexts: EguiContexts,
    st: Res<GraphState>,
    mut out: EventWriter<Picked>,
) {
    if st.view_mode != ViewMode::Spatial {
        return;
    }
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    if contexts.ctx_mut().wants_pointer_input() {
        return;
    }

    let Ok(window) = windows.get_single() else { return; };
    let Some(cursor) = window.cursor_position() else { return; };
    let Ok((camera, cam_tf)) = cam_q.get_single() else { return; };

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
        st.selected = Some(id.clone());
        st.needs_redraw.store(true, Ordering::Relaxed);
    }
}

fn update_layout_or_timeline(time: Res<Time>, mut st: ResMut<GraphState>) {
    // search overlay must run with egui context -> we do it in draw_scene
    match st.view_mode {
        ViewMode::Spatial => {
            let vis: HashSet<_> = st.visible_set_capped();
            let mut ecount = 0usize;
            for e in st.edges.iter() {
                if st.edge_visible(e, &vis) {
                    ecount += 1;
                }
            }
            st.set_visible_counts(vis.len(), ecount);

            st.progressive_prepare(&vis);
            let dt = time.delta_seconds().min(0.033);
            st.force_step(&vis, dt);
        }
        ViewMode::Timeline => {
            // In timeline mode we don’t force-layout; counts are still useful:
            let vis: HashSet<_> = st.visible_set_capped();
            let mut ecount = 0usize;
            for e in st.edges.iter() {
                if st.edge_visible(e, &vis) {
                    ecount += 1;
                }
            }
            st.set_visible_counts(vis.len(), ecount);
        }
    }
}

fn draw_scene(
    mut commands: Commands,
    mut st: ResMut<GraphState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(Entity, &NodeMarker)>,
    mut gizmos: Gizmos,
    mut contexts: EguiContexts,
) {
    // overlay UI
    search_overlay(contexts, st);

    match st.view_mode {
        ViewMode::Spatial => draw_spatial(commands, st, meshes, mats, query, gizmos, contexts),
        ViewMode::Timeline => draw_timeline(st, gizmos, contexts),
    }
}

fn draw_spatial(
    mut commands: Commands,
    mut st: ResMut<GraphState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(Entity, &NodeMarker)>,
    mut gizmos: Gizmos,
    mut contexts: EguiContexts,
) {
    let vis: HashSet<_> = st.visible_set_capped();

    // Tooltip
    if let Some(hid) = &st.hovered {
        egui::Area::new("tooltip")
            .fixed_pos(
                contexts
                    .ctx_mut()
                    .input(|i| i.pointer.hover_pos().unwrap_or(egui::pos2(0.0, 0.0)))
                    + egui::vec2(14.0, 14.0),
            )
            .show(contexts.ctx_mut(), |ui| {
                ui.group(|ui| {
                    for line in st.node_tooltip_lines(hid) {
                        ui.label(line);
                    }
                    ui.separator();
                    ui.label("why connected (first 8 edges):");
                    for e in st.edges_for_node(hid).into_iter().take(8) {
                        ui.label(st.explain_edge(&e));
                    }
                });
            });
    }

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
            if st.edge_is_glowing(e) {
                gizmos.line(*a, *b, Color::rgb(1.0, 1.0, 1.0));
            }
            gizmos.line(*a, *b, Color::WHITE);
        }
    }
}

// Timeline/Feynman render:
// X = time (past -> left), Y = lane (user/process/file), Z = stable hash
fn draw_timeline(mut st: ResMut<GraphState>, mut gizmos: Gizmos, mut contexts: EguiContexts) {
    // Little legend
    egui::Area::new("timeline_legend")
        .fixed_pos(egui::pos2(10.0, 120.0))
        .show(contexts.ctx_mut(), |ui| {
            ui.group(|ui| {
                ui.label("Timeline/Feynman:");
                ui.label("X = time (past → left), now at x=0");
                ui.label("Y lanes: user=0, proc=+8, file=-8");
                ui.label("Z = stable hash spread");
            });
        });

    let now = st.timeline_now();
    let window = st.timeline_window;
    let scale = st.timeline_scale.max(0.001);

    // Draw axis line (now)
    gizmos.line(Vec3::new(0.0, -10.0, 0.0), Vec3::new(0.0, 10.0, 0.0), Color::WHITE);

    // Draw lane guides
    gizmos.line(Vec3::new(-window.as_secs_f32() * scale, 8.0, -20.0), Vec3::new(0.0, 8.0, 20.0), Color::WHITE);
    gizmos.line(Vec3::new(-window.as_secs_f32() * scale, 0.0, -20.0), Vec3::new(0.0, 0.0, 20.0), Color::WHITE);
    gizmos.line(Vec3::new(-window.as_secs_f32() * scale, -8.0, -20.0), Vec3::new(0.0, -8.0, 20.0), Color::WHITE);

    // Render events
    // For each event: compute x = -(now - ts).seconds * scale
    for ev in st.timeline_events.iter() {
        let age = now.duration_since(ev.ts).as_secs_f32();
        if age > window.as_secs_f32() {
            continue;
        }
        let x = -age * scale;

        match &ev.kind {
            state::TimelineEvtKind::NodeUpsert | state::TimelineEvtKind::NodeRemove => {
                if let Some(a) = &ev.a {
                    let base = st.timeline_pos_for_node(a);
                    let p = Vec3::new(x, base.y, base.z);

                    // draw a tiny cross
                    let s = 0.25;
                    gizmos.line(p + Vec3::new(-s, 0.0, 0.0), p + Vec3::new(s, 0.0, 0.0), Color::WHITE);
                    gizmos.line(p + Vec3::new(0.0, -s, 0.0), p + Vec3::new(0.0, s, 0.0), Color::WHITE);
                }
            }
            state::TimelineEvtKind::EdgeUpsert | state::TimelineEvtKind::EdgeRemove => {
                let (Some(a), Some(b)) = (&ev.a, &ev.b) else { continue; };
                let pa = st.timeline_pos_for_node(a);
                let pb = st.timeline_pos_for_node(b);

                let a3 = Vec3::new(x, pa.y, pa.z);
                let b3 = Vec3::new(x, pb.y, pb.z);

                // draw interaction line at that time slice
                gizmos.line(a3, b3, Color::WHITE);

                // draw a small tick at midpoint to show edge kind changes (optional)
                let mid = (a3 + b3) * 0.5;
                let s = 0.18;
                gizmos.line(mid + Vec3::new(0.0, -s, 0.0), mid + Vec3::new(0.0, s, 0.0), Color::WHITE);

                // If you want, we could show edge kind text via egui hover later.
                let _ = &ev.edge_kind;
            }
            state::TimelineEvtKind::BatchBegin(_) | state::TimelineEvtKind::BatchEnd(_) => {
                // ignore in MVP rendering
            }
        }
    }
}

fn apply_jump_to(
    mut st: ResMut<GraphState>,
    mut cam_q: Query<&mut Transform, With<Camera>>,
) {
    let Some(id) = st.jump_to.take() else { return; };

    // Jump affects spatial; timeline doesn’t need camera jump (yet)
    if st.view_mode != ViewMode::Spatial {
        st.focus = Some(id);
        st.selected = Some(id);
        return;
    }

    let Some(target) = st.positions.get(&id).cloned() else { return; };
    st.focus = Some(id.clone());
    st.selected = Some(id.clone());
    st.needs_redraw.store(true, Ordering::Relaxed);

    let Ok(mut cam_tf) = cam_q.get_single_mut() else { return; };

    let current = cam_tf.translation;
    let dist = (current - target).length().max(6.0);
    let offset = Vec3::new(dist * 0.6, dist * 0.5, dist * 0.9);

    cam_tf.translation = target + offset;
    cam_tf.look_at(target, Vec3::Y);
}
