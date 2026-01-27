use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use std::collections::HashSet;
use std::sync::atomic::Ordering;

use crate::app::events::Picked;
use crate::graph::{GraphState, ViewMode};
use crate::ui::tooltips::render_tooltip;

#[derive(Component)]
pub struct NodeMarker;

// Spatial hover only (timeline has its own hover picking based on events)
pub fn hover_detection_spatial(
    windows: Query<&Window>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
    mut contexts: EguiContexts,
    mut st: ResMut<GraphState>,
) {
    if st.ui.view_mode != ViewMode::Spatial {
        st.ui.hovered = None;
        return;
    }

    let Ok(window) = windows.get_single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        st.ui.hovered = None;
        return;
    };
    let Ok((camera, cam_tf)) = cam_q.get_single() else {
        return;
    };

    if contexts.ctx_mut().wants_pointer_input() {
        return;
    }

    let mut best: Option<(f32, spacegraph_core::NodeId)> = None;
    for (id, pos) in st.spatial.positions.iter() {
        let Some(screen) = camera.world_to_viewport(cam_tf, *pos) else {
            continue;
        };
        let d = screen.distance(cursor);
        if d < 18.0 && best.as_ref().map(|(bd, _)| d < *bd).unwrap_or(true) {
            best = Some((d, id.clone()));
        }
    }
    st.ui.hovered = best.map(|(_, id)| id);
}

pub fn picking_focus(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
    mut contexts: EguiContexts,
    st: Res<GraphState>,
    mut out: EventWriter<Picked>,
) {
    if st.ui.view_mode != ViewMode::Spatial {
        return;
    }
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    if contexts.ctx_mut().wants_pointer_input() {
        return;
    }

    let Ok(window) = windows.get_single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, cam_tf)) = cam_q.get_single() else {
        return;
    };

    let mut best: Option<(f32, spacegraph_core::NodeId)> = None;
    for (id, pos) in st.spatial.positions.iter() {
        let Some(screen) = camera.world_to_viewport(cam_tf, *pos) else {
            continue;
        };
        let d = screen.distance(cursor);
        if d < 14.0 && best.as_ref().map(|(bd, _)| d < *bd).unwrap_or(true) {
            best = Some((d, id.clone()));
        }
    }
    if let Some((_, picked)) = best {
        out.send(Picked(picked));
    }
}

pub fn apply_picked_focus(mut st: ResMut<GraphState>, mut ev: EventReader<Picked>) {
    for Picked(id) in ev.read() {
        st.ui.focus = Some(id.clone());
        st.ui.selected = Some(id.clone());
        st.needs_redraw.store(true, Ordering::Relaxed);
    }
}

pub fn draw_spatial(
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
    if let Some(hid) = &st.ui.hovered {
        let pos = contexts
            .ctx_mut()
            .input(|i| i.pointer.hover_pos().unwrap_or(egui::pos2(0.0, 0.0)))
            + egui::vec2(14.0, 14.0);

        let mut lines = st.node_tooltip_lines(hid);
        lines.push("why connected (first 8 edges):".to_string());
        for e in st.edges_for_node(hid).into_iter().take(8) {
            lines.push(st.explain_edge(&e));
        }
        render_tooltip(contexts.ctx_mut(), "tooltip_spatial", pos, lines);
    }

    if st.needs_redraw.swap(false, Ordering::Relaxed) {
        for (e, _) in query.iter_mut() {
            commands.entity(e).despawn_recursive();
        }

        let sphere = meshes.add(Sphere::new(0.28));
        let mat_norm = mats.add(StandardMaterial::default());
        let mat_glow = mats.add(StandardMaterial {
            emissive: Color::srgb(1.0, 1.0, 1.0).into(),
            ..default()
        });

        for (id, node) in st.model.nodes.iter() {
            if !vis.contains(id) {
                continue;
            }
            if !st.passes_filter(id, node) {
                continue;
            }
            let Some(pos) = st.spatial.positions.get(id).cloned() else {
                continue;
            };
            let use_glow = st.node_is_glowing(id);

            commands.spawn((
                PbrBundle {
                    mesh: sphere.clone(),
                    material: if use_glow {
                        mat_glow.clone()
                    } else {
                        mat_norm.clone()
                    },
                    transform: Transform::from_translation(pos),
                    ..default()
                },
                NodeMarker,
            ));
        }
    }

    if st.ui.show_edges {
        for e in st.model.edges.iter() {
            if !st.edge_visible(e, &vis) {
                continue;
            }
            let (Some(a), Some(b)) = (
                st.spatial.positions.get(&e.from),
                st.spatial.positions.get(&e.to),
            ) else {
                continue;
            };
            if st.edge_is_glowing(e) {
                gizmos.line(*a, *b, Color::srgb(1.0, 1.0, 1.0));
            }
            gizmos.line(*a, *b, Color::WHITE);
        }
    }
}
