use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use std::collections::HashSet;
use std::sync::atomic::Ordering;

use crate::app::events::Picked;
use crate::graph::model::{edge_class_name, AggEdgeKey};
use crate::graph::{GraphState, ViewMode};
use crate::ui::tooltips::render_tooltip;
use crate::util::config::LodEdgesMode;

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
        st.ui.selected_a = Some(id.clone());
        st.ui.selected_b = None;
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
    let lod_active = st.cfg.lod_active(vis.len());
    if st.spatial.lod_active != lod_active {
        st.spatial.lod_active = lod_active;
        st.needs_redraw.store(true, Ordering::Relaxed);
    }

    // Tooltip
    let hovered = st.ui.hovered.clone();
    let selected = st.ui.selected.clone();
    if let Some(hid) = hovered.as_ref() {
        let pos = contexts
            .ctx_mut()
            .input(|i| i.pointer.hover_pos().unwrap_or(egui::pos2(0.0, 0.0)))
            + egui::vec2(14.0, 14.0);

        let mut lines = st.node_tooltip_lines(hid);
        if let Some(selected) = selected.as_ref() {
            if selected != hid {
                lines.push("why connected:".to_string());
                match st.explain_path_cached(selected, hid, &vis) {
                    Some(path) if path.is_empty() => {
                        lines.push("same node".to_string());
                    }
                    Some(path) => {
                        for step in path {
                            let from = st.node_label_with_id(&step.from);
                            let to = st.node_label_with_id(&step.to);
                            lines.push(format!(
                                "{} --[{}]--> {}",
                                from,
                                edge_class_name(step.class),
                                to
                            ));
                        }
                    }
                    None => lines.push("no path within depth cap".to_string()),
                }
            }
        }
        render_tooltip(contexts.ctx_mut(), "tooltip_spatial", pos, lines);
    }

    if st.needs_redraw.swap(false, Ordering::Relaxed) {
        for (e, _) in query.iter_mut() {
            commands.entity(e).despawn_recursive();
        }

        if !lod_active {
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
    }

    if lod_active {
        let marker = 0.35;
        for id in vis.iter() {
            let Some(pos) = st.spatial.positions.get(id).cloned() else {
                continue;
            };
            let color = if st.node_is_glowing(id) {
                Color::WHITE
            } else {
                Color::srgb(0.7, 0.7, 0.95)
            };
            gizmos.line(
                pos + Vec3::new(-marker, 0.0, 0.0),
                pos + Vec3::new(marker, 0.0, 0.0),
                color,
            );
            gizmos.line(
                pos + Vec3::new(0.0, -marker, 0.0),
                pos + Vec3::new(0.0, marker, 0.0),
                color,
            );
            gizmos.line(
                pos + Vec3::new(0.0, 0.0, -marker),
                pos + Vec3::new(0.0, 0.0, marker),
                color,
            );
        }
    }

    if st.ui.show_edges {
        let edges_mode = if lod_active {
            st.cfg.lod_edges_mode
        } else {
            LodEdgesMode::All
        };

        let mut focus_nodes = HashSet::new();
        if let Some(id) = st.ui.focus.clone() {
            if vis.contains(&id) {
                focus_nodes.insert(id);
            }
        }
        if let Some(id) = st.ui.selected.clone() {
            if vis.contains(&id) {
                focus_nodes.insert(id);
            }
        }
        if let Some(id) = st.ui.selected_a.clone() {
            if vis.contains(&id) {
                focus_nodes.insert(id);
            }
        }
        if let Some(id) = st.ui.selected_b.clone() {
            if vis.contains(&id) {
                focus_nodes.insert(id);
            }
        }

        match edges_mode {
            LodEdgesMode::Off => {}
            LodEdgesMode::FocusOnly => {
                if st.cfg.show_agg_edges && !focus_nodes.is_empty() {
                    let mut agg_keys = HashSet::new();
                    for id in focus_nodes.iter() {
                        for edge in st.model.edges_for_node(id) {
                            if !st.edge_visible(edge, &vis) {
                                continue;
                            }
                            agg_keys.insert(AggEdgeKey::new(edge));
                        }
                    }
                    for key in agg_keys {
                        let (Some(a), Some(b)) = (
                            st.spatial.positions.get(&key.from),
                            st.spatial.positions.get(&key.to),
                        ) else {
                            continue;
                        };
                        gizmos.line(*a, *b, Color::srgb(0.8, 0.8, 1.0));
                    }
                }
                if st.cfg.show_raw_edges && !focus_nodes.is_empty() {
                    let mut raw_edges = HashSet::new();
                    for id in focus_nodes.iter() {
                        for edge in st.model.edges_for_node(id) {
                            if !st.edge_visible(edge, &vis) {
                                continue;
                            }
                            raw_edges.insert(edge.clone());
                        }
                    }
                    for edge in raw_edges {
                        let (Some(a), Some(b)) = (
                            st.spatial.positions.get(&edge.from),
                            st.spatial.positions.get(&edge.to),
                        ) else {
                            continue;
                        };
                        if st.edge_is_glowing(&edge) {
                            gizmos.line(*a, *b, Color::srgb(1.0, 1.0, 1.0));
                        }
                        gizmos.line(*a, *b, Color::WHITE);
                    }
                }
            }
            LodEdgesMode::All => {
                if st.cfg.show_agg_edges {
                    for edge in st.model.agg_edges() {
                        if !vis.contains(&edge.key.from) || !vis.contains(&edge.key.to) {
                            continue;
                        }
                        let (Some(a), Some(b)) = (
                            st.spatial.positions.get(&edge.key.from),
                            st.spatial.positions.get(&edge.key.to),
                        ) else {
                            continue;
                        };
                        gizmos.line(*a, *b, Color::srgb(0.8, 0.8, 1.0));
                    }
                }
                if st.cfg.show_raw_edges {
                    for id in vis.iter() {
                        for edge in st.model.edges_for_node(id) {
                            if &edge.from != id {
                                continue;
                            }
                            if !st.edge_visible(edge, &vis) {
                                continue;
                            }
                            let (Some(a), Some(b)) = (
                                st.spatial.positions.get(&edge.from),
                                st.spatial.positions.get(&edge.to),
                            ) else {
                                continue;
                            };
                            if st.edge_is_glowing(edge) {
                                gizmos.line(*a, *b, Color::srgb(1.0, 1.0, 1.0));
                            }
                            gizmos.line(*a, *b, Color::WHITE);
                        }
                    }
                }
            }
        }
    }
}
