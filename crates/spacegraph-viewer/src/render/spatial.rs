use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::time::Instant;

use crate::app::events::Picked;
use crate::graph::interner::NodeIndex;
use crate::graph::model::{edge_class_name, AggEdgeKey, EdgeKindClass};
use crate::graph::{GraphState, ViewMode};
use crate::render::theme;
use crate::ui::tooltips::render_tooltip;
use crate::util::config::{LodEdgesMode, VisualTheme};
use crate::util::ids::node_label_short;

#[derive(Component)]
pub struct NodeMarker;

/// Back-reference from a spawned node entity to its `NodeIndex`.
#[derive(Component, Clone, Copy)]
pub struct NodeRef(pub NodeIndex);

/// Number of emissive ramp steps per node type (idle → full recent-activity
/// flash). The renderer picks a step from the glow-decay fraction.
pub const GLOW_LEVELS: usize = 6;

/// Mesh/material handles for node entities, created once at startup so the
/// redraw path never allocates assets (the previous per-frame `meshes.add` /
/// `mats.add` leaked a handle every redraw).
///
/// `standard[kind][level]` is a per-type emissive ramp for the Standard theme
/// (level 0 = idle neon, last = white flash); `minimal_*` reproduce the flat
/// pre-visual-pass look.
#[derive(Resource)]
pub struct NodeRenderResources {
    pub mesh: Handle<Mesh>,
    pub standard: Vec<[Handle<StandardMaterial>; GLOW_LEVELS]>,
    pub minimal_normal: Handle<StandardMaterial>,
    pub minimal_glow: Handle<StandardMaterial>,
}

/// Persistent `NodeIndex → Entity` map for spatial node entities. Lets the
/// renderer spawn on node-add, despawn on remove, and otherwise only mutate
/// `Transform` / material — no per-frame entity churn.
#[derive(Resource, Default)]
pub struct NodeEntities {
    pub map: HashMap<NodeIndex, Entity>,
}

/// Create the cached node mesh/material handles (startup, once).
pub fn setup_node_render_resources(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Sphere::new(0.28));

    let standard: Vec<[Handle<StandardMaterial>; GLOW_LEVELS]> = theme::NodeKind::ALL
        .iter()
        .map(|kind| {
            let base = kind.base_color();
            std::array::from_fn(|level| {
                let t = level as f32 / (GLOW_LEVELS - 1) as f32;
                // Idle nodes glow faintly in their type colour; recent activity
                // ramps the emissive toward a bright white flash that blooms.
                let emis = theme::lerp(base, theme::RECENT_GLOW, t).to_linear();
                let intensity = 1.2 + t * t * 6.0;
                mats.add(StandardMaterial {
                    base_color: base,
                    emissive: LinearRgba::rgb(
                        emis.red * intensity,
                        emis.green * intensity,
                        emis.blue * intensity,
                    ),
                    perceptual_roughness: 0.5,
                    metallic: 0.0,
                    ..default()
                })
            })
        })
        .collect();

    let minimal_normal = mats.add(StandardMaterial::default());
    let minimal_glow = mats.add(StandardMaterial {
        emissive: LinearRgba::rgb(1.0, 1.0, 1.0),
        ..default()
    });

    commands.insert_resource(NodeRenderResources {
        mesh,
        standard,
        minimal_normal,
        minimal_glow,
    });
}

/// Pick the material handle for a node given the active theme and its glow
/// decay. Standard: per-type emissive ramp by recency; Minimal: flat
/// normal/glow (binary), matching the pre-visual-pass look.
fn node_material(
    res: &NodeRenderResources,
    st: &GraphState,
    idx: NodeIndex,
    id: &spacegraph_core::NodeId,
    now: std::time::Instant,
    glow_secs: f32,
) -> Handle<StandardMaterial> {
    let glow_until = st.spatial.glow_until[idx.slot()];
    match st.cfg.visual_theme {
        crate::util::config::VisualTheme::Minimal => {
            if glow_until.is_some() {
                res.minimal_glow.clone()
            } else {
                res.minimal_normal.clone()
            }
        }
        crate::util::config::VisualTheme::Standard => {
            let kind = st
                .model
                .nodes
                .get(id)
                .map(theme::NodeKind::of)
                .unwrap_or(theme::NodeKind::File);
            let ramp = &res.standard[kind.index()];
            let level = match glow_until {
                Some(deadline) if glow_secs > 0.0 && deadline > now => {
                    let frac = (deadline - now).as_secs_f32() / glow_secs;
                    (frac.clamp(0.0, 1.0) * (GLOW_LEVELS - 1) as f32).round() as usize
                }
                _ => 0,
            };
            ramp[level.min(GLOW_LEVELS - 1)].clone()
        }
    }
}

/// Keep the persistent node entities in sync with the visible graph: spawn new
/// nodes, despawn departed ones, and for everything else mutate only the
/// `Transform` and material handle. In LOD / tree / timeline modes (gizmo or
/// no entity rendering) all node entities are despawned.
pub fn sync_node_entities(
    mut commands: Commands,
    st: Res<GraphState>,
    res: Res<NodeRenderResources>,
    mut entities: ResMut<NodeEntities>,
    mut q: Query<(&mut Transform, &mut Handle<StandardMaterial>), With<NodeMarker>>,
) {
    let vis = &st.spatial.vis_cache;
    let entity_mode = st.ui.view_mode == ViewMode::Spatial && !st.cfg.lod_active(vis.len());

    if !entity_mode {
        if !entities.map.is_empty() {
            for (_, entity) in entities.map.drain() {
                commands.entity(entity).despawn_recursive();
            }
        }
        return;
    }

    // Despawn entities whose node left the visible set.
    entities.map.retain(|&idx, &mut entity| {
        let keep = st.spatial.index_visible(idx, vis);
        if !keep {
            commands.entity(entity).despawn_recursive();
        }
        keep
    });

    // Spawn missing nodes; update Transform + material for existing ones.
    let now = std::time::Instant::now();
    let glow_secs = st.cfg.glow_duration.as_secs_f32();
    for id in vis.iter() {
        let Some(idx) = st.spatial.index_of(id) else {
            continue;
        };
        if !st.spatial.placed[idx.slot()] {
            continue;
        }
        let pos = st.spatial.positions[idx.slot()];
        let material = node_material(&res, &st, idx, id, now, glow_secs);

        if let Some(&entity) = entities.map.get(&idx) {
            if let Ok((mut tf, mut handle)) = q.get_mut(entity) {
                tf.translation = pos;
                if *handle != material {
                    *handle = material;
                }
            }
        } else {
            let entity = commands
                .spawn((
                    PbrBundle {
                        mesh: res.mesh.clone(),
                        material,
                        transform: Transform::from_translation(pos),
                        ..default()
                    },
                    NodeMarker,
                    NodeRef(idx),
                ))
                .id();
            entities.map.insert(idx, entity);
        }
    }
}

// Spatial hover only (timeline has its own hover picking based on events)
pub fn hover_detection_spatial(
    windows: Query<&Window>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
    mut contexts: EguiContexts,
    mut st: ResMut<GraphState>,
) {
    if st.ui.view_mode == ViewMode::Timeline {
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
    for (id, pos) in st.spatial.placed_positions() {
        let Some(screen) = camera.world_to_viewport(cam_tf, pos) else {
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
    if st.ui.view_mode == ViewMode::Timeline {
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
    for (id, pos) in st.spatial.placed_positions() {
        let Some(screen) = camera.world_to_viewport(cam_tf, pos) else {
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
        if st.ui.view_mode == ViewMode::Tree {
            st.toggle_tree_dir(id);
        }
        st.ui.focus = Some(id.clone());
        st.ui.selected = Some(id.clone());
        st.ui.selected_a = Some(id.clone());
        st.ui.selected_b = None;
        st.needs_redraw.store(true, Ordering::Relaxed);
    }
}

pub fn draw_spatial(mut st: ResMut<GraphState>, mut gizmos: Gizmos, mut contexts: EguiContexts) {
    // Node entities are managed by `sync_node_entities`; this system only draws
    // immediate-mode overlays (tooltips, edge/LOD/tree gizmos) over the visible
    // set published by the layout step.
    let vis: HashSet<_> = st.spatial.vis_cache.clone();
    let lod_active = st.cfg.lod_active(vis.len());
    if st.spatial.lod_active != lod_active {
        st.spatial.lod_active = lod_active;
        st.needs_redraw.store(true, Ordering::Relaxed);
    }

    // Scene dressing: faint floor grid (Standard theme, spatial view).
    if st.cfg.visual_theme == VisualTheme::Standard && st.ui.view_mode == ViewMode::Spatial {
        draw_floor_grid(&mut gizmos);
    }

    // Selection / hover highlight bubbles (lock-on feedback). Drawn in any LOD
    // state; only the few picked nodes, so cost is negligible.
    let highlights = [
        (st.ui.hovered.clone(), Color::srgb(0.90, 0.95, 1.0), 0.50),
        (st.ui.selected.clone(), Color::srgb(0.25, 0.95, 1.0), 0.62),
        (st.ui.focus.clone(), Color::srgb(0.20, 1.0, 0.85), 0.72),
    ];
    for (maybe_id, color, radius) in highlights {
        if let Some(id) = maybe_id {
            if let Some(pos) = st.spatial.position_of(&id) {
                gizmos.sphere(pos, Quat::IDENTITY, radius, color);
            }
        }
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

    if lod_active {
        let marker = 0.35;
        for id in vis.iter() {
            let Some(pos) = st.spatial.position_of(id) else {
                continue;
            };
            // Alerts always stand out (severity colour) even under LOD.
            let color = match st.model.nodes.get(id) {
                Some(spacegraph_core::Node::Alert { severity, .. }) => {
                    theme::alert_severity_color(severity)
                }
                _ if st.node_is_glowing(id) => Color::WHITE,
                _ => Color::srgb(0.7, 0.7, 0.95),
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

    if st.ui.view_mode == ViewMode::Tree {
        let indicator_color = Color::srgb(0.9, 0.9, 0.9);
        let size = 0.35;
        let offset = Vec3::new(-0.6, 0.0, 0.0);
        for id in st.spatial.tree_dir_children.iter() {
            if !vis.contains(id) {
                continue;
            }
            let Some(pos) = st.spatial.position_of(id) else {
                continue;
            };
            let base = pos + offset;
            if st.tree_dir_is_expanded(id) {
                let a = base + Vec3::new(-size, size * 0.6, 0.0);
                let b = base + Vec3::new(size, size * 0.6, 0.0);
                let c = base + Vec3::new(0.0, -size * 0.6, 0.0);
                gizmos.line(a, b, indicator_color);
                gizmos.line(b, c, indicator_color);
                gizmos.line(c, a, indicator_color);
            } else {
                let a = base + Vec3::new(-size * 0.5, size, 0.0);
                let b = base + Vec3::new(-size * 0.5, -size, 0.0);
                let c = base + Vec3::new(size, 0.0, 0.0);
                gizmos.line(a, b, indicator_color);
                gizmos.line(b, c, indicator_color);
                gizmos.line(c, a, indicator_color);
            }
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
                            st.spatial.position_of(&key.from),
                            st.spatial.position_of(&key.to),
                        ) else {
                            continue;
                        };
                        gizmos.line(a, b, theme::edge_color(key.class));
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
                            st.spatial.position_of(&edge.from),
                            st.spatial.position_of(&edge.to),
                        ) else {
                            continue;
                        };
                        if st.edge_is_glowing(&edge) {
                            gizmos.line(a, b, Color::srgb(1.0, 1.0, 1.0));
                        }
                        gizmos.line(a, b, Color::WHITE);
                    }
                }
            }
            LodEdgesMode::All => {
                if st.cfg.show_agg_edges {
                    // Iterate the visible nodes' adjacency (bounded by the capped
                    // set), de-duplicating by aggregated-edge key — instead of an
                    // O(E_total) scan over every aggregated edge in the model.
                    let mut seen = HashSet::new();
                    for id in vis.iter() {
                        for edge in st.model.edges_for_node(id) {
                            if &edge.from != id || !st.edge_visible(edge, &vis) {
                                continue;
                            }
                            let class = EdgeKindClass::from_kind(&edge.kind);
                            if !seen.insert(AggEdgeKey::new(edge)) {
                                continue;
                            }
                            let (Some(a), Some(b)) = (
                                st.spatial.position_of(&edge.from),
                                st.spatial.position_of(&edge.to),
                            ) else {
                                continue;
                            };
                            gizmos.line(a, b, theme::edge_color(class));
                        }
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
                                st.spatial.position_of(&edge.from),
                                st.spatial.position_of(&edge.to),
                            ) else {
                                continue;
                            };
                            if st.edge_is_glowing(edge) {
                                gizmos.line(a, b, Color::srgb(1.0, 1.0, 1.0));
                            }
                            gizmos.line(a, b, Color::WHITE);
                        }
                    }
                }
            }
        }

        // Recent-activity pulse: a bright dot travels along each glowing edge
        // as its glow decays (Standard theme).
        if st.cfg.visual_theme == VisualTheme::Standard {
            let now = Instant::now();
            let dur = st.cfg.glow_duration.as_secs_f32().max(0.001);
            for (edge, deadline) in st.spatial.glow_edges.iter() {
                if *deadline <= now || !vis.contains(&edge.from) || !vis.contains(&edge.to) {
                    continue;
                }
                let (Some(a), Some(b)) = (
                    st.spatial.position_of(&edge.from),
                    st.spatial.position_of(&edge.to),
                ) else {
                    continue;
                };
                let remaining = (*deadline - now).as_secs_f32();
                let t = (1.0 - remaining / dur).clamp(0.0, 1.0);
                let class = EdgeKindClass::from_kind(&edge.kind);
                gizmos.sphere(a.lerp(b, t), Quat::IDENTITY, 0.18, theme::edge_color(class));
            }
        }
    }
}

/// Billboarded labels for the focused / hovered / selected nodes only (capped),
/// projected to screen via egui. Never labels every node at once.
pub fn draw_node_labels(
    mut contexts: EguiContexts,
    st: Res<GraphState>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
) {
    if st.ui.view_mode == ViewMode::Timeline {
        return;
    }
    let Ok((camera, cam_tf)) = cam_q.get_single() else {
        return;
    };

    const CAP: usize = 6;
    let mut targets: Vec<spacegraph_core::NodeId> = Vec::new();
    for id in [
        &st.ui.hovered,
        &st.ui.selected,
        &st.ui.focus,
        &st.ui.selected_a,
        &st.ui.selected_b,
    ]
    .into_iter()
    .flatten()
    {
        if !targets.contains(id) {
            targets.push(id.clone());
        }
    }
    targets.truncate(CAP);
    if targets.is_empty() {
        return;
    }

    let ctx = contexts.ctx_mut();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("node_labels"),
    ));
    for id in targets {
        let Some(pos) = st.spatial.position_of(&id) else {
            continue;
        };
        let Some(screen) = camera.world_to_viewport(cam_tf, pos) else {
            continue;
        };
        let label = st
            .model
            .nodes
            .get(&id)
            .map(node_label_short)
            .unwrap_or_else(|| id.0.clone());
        painter.text(
            egui::pos2(screen.x + 10.0, screen.y - 6.0),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(13.0),
            egui::Color32::from_rgb(200, 230, 255),
        );
    }
}

/// Draw a faint floor grid on the XZ plane (scene dressing for the Standard
/// theme). Immediate-mode gizmos with a bounded line count.
fn draw_floor_grid(gizmos: &mut Gizmos) {
    const HALF: i32 = 28;
    const STEP: f32 = 4.0;
    let extent = HALF as f32 * STEP;
    let color = theme::GRID_LINE;
    for i in -HALF..=HALF {
        let o = i as f32 * STEP;
        gizmos.line(Vec3::new(-extent, 0.0, o), Vec3::new(extent, 0.0, o), color);
        gizmos.line(Vec3::new(o, 0.0, -extent), Vec3::new(o, 0.0, extent), color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::GraphState;

    fn graph_state(n: usize) -> GraphState {
        let mut gs = GraphState::default();
        gs.cfg.max_visible_nodes = n + 16;
        gs.cfg.progressive_nodes_per_frame = n + 16;
        gs.load_synthetic_graph(n);
        let vis = gs.visible_set_capped();
        gs.progressive_prepare(&vis);
        gs.spatial.vis_cache = vis;
        gs
    }

    fn node_entities(app: &mut App) -> HashSet<Entity> {
        let mut q = app.world_mut().query_filtered::<Entity, With<NodeMarker>>();
        q.iter(app.world()).collect()
    }

    fn dummy_render_resources() -> NodeRenderResources {
        let ramp: [Handle<StandardMaterial>; GLOW_LEVELS] =
            std::array::from_fn(|_| Handle::default());
        NodeRenderResources {
            mesh: Handle::default(),
            standard: vec![ramp.clone(), ramp.clone(), ramp],
            minimal_normal: Handle::default(),
            minimal_glow: Handle::default(),
        }
    }

    #[test]
    fn spawns_one_entity_per_visible_node() {
        let mut app = App::new();
        app.insert_resource(graph_state(200))
            .insert_resource(NodeEntities::default())
            .insert_resource(dummy_render_resources())
            .add_systems(Update, sync_node_entities);
        app.update();
        let count = node_entities(&mut app).len();
        let want = app.world().resource::<GraphState>().spatial.vis_cache.len();
        assert!(want > 0);
        assert_eq!(count, want, "one node entity per visible node");
    }

    #[test]
    fn steady_state_has_no_entity_churn() {
        let mut app = App::new();
        app.insert_resource(graph_state(300))
            .insert_resource(NodeEntities::default())
            .insert_resource(dummy_render_resources())
            .add_systems(Update, sync_node_entities);

        app.update();
        let frame1 = node_entities(&mut app);
        assert!(!frame1.is_empty());

        // Topology unchanged → the second frame must reuse the exact same
        // entities (mutate Transform/material only), never respawn.
        app.update();
        let frame2 = node_entities(&mut app);
        assert_eq!(
            frame1, frame2,
            "steady state must not spawn/despawn entities"
        );
    }

    #[test]
    fn minimal_theme_uses_flat_materials() {
        use crate::util::config::VisualTheme;
        let ramp: [Handle<StandardMaterial>; GLOW_LEVELS] =
            std::array::from_fn(|_| Handle::default());
        let res = NodeRenderResources {
            mesh: Handle::default(),
            standard: vec![ramp.clone(), ramp.clone(), ramp],
            minimal_normal: Handle::weak_from_u128(11),
            minimal_glow: Handle::weak_from_u128(22),
        };

        let mut gs = graph_state(50);
        gs.cfg.visual_theme = VisualTheme::Minimal;
        let id = gs.spatial.vis_cache.iter().next().cloned().unwrap();
        let idx = gs.spatial.index_of(&id).unwrap();
        let now = std::time::Instant::now();

        // Not glowing → flat normal material (Phase 4 look).
        assert_eq!(
            node_material(&res, &gs, idx, &id, now, 0.9),
            res.minimal_normal
        );

        // Glowing → flat white-emissive glow (Phase 4 look), binary, no ramp.
        gs.spatial
            .set_node_glow(&id, now + std::time::Duration::from_secs(1));
        let idx = gs.spatial.index_of(&id).unwrap();
        assert_eq!(
            node_material(&res, &gs, idx, &id, now, 0.9),
            res.minimal_glow
        );
    }

    #[test]
    fn lod_or_non_spatial_mode_despawns_node_entities() {
        let mut app = App::new();
        let mut gs = graph_state(100);
        gs.ui.view_mode = ViewMode::Timeline; // not entity-rendered
        app.insert_resource(gs)
            .insert_resource(NodeEntities::default())
            .insert_resource(dummy_render_resources())
            .add_systems(Update, sync_node_entities);
        app.update();
        assert_eq!(node_entities(&mut app).len(), 0);
    }
}
