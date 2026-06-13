use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::time::Instant;

use crate::app::events::Picked;
use crate::graph::interner::NodeIndex;
use crate::graph::model::{edge_class_name, EdgeKindClass};
use crate::graph::{GraphState, ViewMode};
use crate::render::freefly::FlyCam;
use crate::render::node_mesh;
use crate::render::theme;
use crate::ui::tooltips::render_tooltip;
use crate::util::config::VisualTheme;
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
    /// Per-kind solid core mesh (Standard theme), indexed by `NodeKind::index()`.
    pub core_mesh: [Handle<Mesh>; theme::NodeKind::ALL.len()],
    /// Optional per-kind wireframe shell mesh (Standard theme).
    pub shell_mesh: [Option<Handle<Mesh>>; theme::NodeKind::ALL.len()],
    /// Per-kind unlit emissive material for the shell.
    pub shell_mat: [Handle<StandardMaterial>; theme::NodeKind::ALL.len()],
    /// Flat sphere used by the Minimal theme (the pre-geometry look).
    pub minimal_mesh: Handle<Mesh>,
    pub standard: Vec<[Handle<StandardMaterial>; GLOW_LEVELS]>,
    pub minimal_normal: Handle<StandardMaterial>,
    pub minimal_glow: Handle<StandardMaterial>,
}

/// Marker on the wireframe-shell child entity of a node (Standard theme).
#[derive(Component)]
pub struct ShellMarker;

/// Set when the visual theme changes; drains and respawns all node entities once
/// so cores/shells match the new theme. Steady state never sets this.
#[derive(Resource, Default)]
pub struct RebuildNodeEntities(pub bool);

/// Persistent `NodeIndex → Entity` map for spatial node entities. Lets the
/// renderer spawn on node-add, despawn on remove, and otherwise only mutate
/// `Transform` / material — no per-frame entity churn.
#[derive(Resource, Default)]
pub struct NodeEntities {
    pub map: HashMap<NodeIndex, Entity>,
}

/// In-progress box-select drag (LMB press position in viewport coords).
#[derive(Resource, Default)]
pub struct DragSelect {
    pub start: Option<Vec2>,
}

/// Pick radius for ray-sphere selection (bounds the largest core envelope; a
/// bounding-sphere approximation over the per-kind geometry).
const PICK_RADIUS: f32 = 0.5;

/// Nearest positive ray-sphere intersection distance, or `None` if the ray
/// misses. Depth-correct (unlike screen-space distance).
fn ray_sphere_t(origin: Vec3, dir: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let oc = origin - center;
    let a = dir.dot(dir);
    let b = 2.0 * oc.dot(dir);
    let c = oc.dot(oc) - radius * radius;
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 || a == 0.0 {
        return None;
    }
    let sq = disc.sqrt();
    let t0 = (-b - sq) / (2.0 * a);
    let t1 = (-b + sq) / (2.0 * a);
    let t = if t0 > 0.0 { t0 } else { t1 };
    (t > 0.0).then_some(t)
}

/// Create the cached node mesh/material handles (startup, once).
pub fn setup_node_render_resources(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let minimal_mesh = meshes.add(Sphere::new(0.28));

    let core_mesh: [Handle<Mesh>; theme::NodeKind::ALL.len()] =
        std::array::from_fn(|i| meshes.add(node_mesh::node_core(theme::NodeKind::ALL[i])));
    let shell_mesh: [Option<Handle<Mesh>>; theme::NodeKind::ALL.len()] = std::array::from_fn(|i| {
        node_mesh::node_shell(theme::NodeKind::ALL[i]).map(|m| meshes.add(m))
    });
    let shell_mat: [Handle<StandardMaterial>; theme::NodeKind::ALL.len()] =
        std::array::from_fn(|i| {
            let c = theme::NodeKind::ALL[i].base_color().to_linear();
            // Unlit + HDR emissive (> 1.0) so the holographic shell blooms.
            mats.add(StandardMaterial {
                base_color: theme::NodeKind::ALL[i].base_color(),
                emissive: LinearRgba::rgb(c.red * 3.0, c.green * 3.0, c.blue * 3.0),
                unlit: true,
                ..default()
            })
        });

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
        core_mesh,
        shell_mesh,
        shell_mat,
        minimal_mesh,
        standard,
        minimal_normal,
        minimal_glow,
    });
}

/// Core mesh plus an optional (shell mesh, shell material) for a node.
type NodeMeshSet = (
    Handle<Mesh>,
    Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
);

/// The mesh + optional shell for a node given the active theme.
fn node_meshes(
    res: &NodeRenderResources,
    theme: crate::util::config::VisualTheme,
    kind: theme::NodeKind,
) -> NodeMeshSet {
    match theme {
        crate::util::config::VisualTheme::Minimal => (res.minimal_mesh.clone(), None),
        crate::util::config::VisualTheme::Standard => {
            let i = kind.index();
            let shell = res.shell_mesh[i]
                .clone()
                .map(|m| (m, res.shell_mat[i].clone()));
            (res.core_mesh[i].clone(), shell)
        }
    }
}

/// Resolve a node's kind (defaulting to File for an unknown id).
fn node_kind(st: &GraphState, id: &spacegraph_core::NodeId) -> theme::NodeKind {
    st.model
        .nodes
        .get(id)
        .map(theme::NodeKind::of)
        .unwrap_or(theme::NodeKind::File)
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
#[allow(clippy::type_complexity)]
pub fn sync_node_entities(
    mut commands: Commands,
    st: Res<GraphState>,
    res: Res<NodeRenderResources>,
    mut entities: ResMut<NodeEntities>,
    mut rebuild: ResMut<RebuildNodeEntities>,
    mut q: Query<
        (
            &mut Transform,
            &mut Handle<StandardMaterial>,
            &mut Handle<Mesh>,
        ),
        With<NodeMarker>,
    >,
) {
    let vis = &st.spatial.vis_cache;
    let entity_mode = st.ui.view_mode == ViewMode::Spatial && !st.cfg.lod_active(vis.len());

    if !entity_mode {
        if !entities.map.is_empty() {
            for (_, entity) in entities.map.drain() {
                commands.entity(entity).despawn_recursive();
            }
        }
        rebuild.0 = false;
        return;
    }

    // Theme switch → drain once so cores/shells match the new theme; the spawn
    // loop below repopulates this same frame. Steady state never sets this flag.
    if rebuild.0 {
        for (_, entity) in entities.map.drain() {
            commands.entity(entity).despawn_recursive();
        }
        rebuild.0 = false;
    }

    // Despawn entities whose node left the visible set or is fogged.
    entities.map.retain(|&idx, &mut entity| {
        let keep = st.spatial.index_visible(idx, vis)
            && st
                .spatial
                .interner
                .resolve(idx)
                .map(|id| st.is_visible_rendered(id))
                .unwrap_or(false);
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
        if !st.spatial.placed[idx.slot()] || !st.is_visible_rendered(id) {
            continue;
        }
        let pos = st.spatial.positions[idx.slot()];
        let material = node_material(&res, &st, idx, id, now, glow_secs);
        let kind = node_kind(&st, id);
        let (mesh, shell) = node_meshes(&res, st.cfg.visual_theme, kind);

        if let Some(&entity) = entities.map.get(&idx) {
            if let Ok((mut tf, mut handle, mut mesh_h)) = q.get_mut(entity) {
                tf.translation = pos;
                if *handle != material {
                    *handle = material;
                }
                if *mesh_h != mesh {
                    *mesh_h = mesh;
                }
            }
        } else {
            let entity = commands
                .spawn((
                    PbrBundle {
                        mesh: mesh.clone(),
                        material,
                        transform: Transform::from_translation(pos),
                        ..default()
                    },
                    NodeMarker,
                    NodeRef(idx),
                ))
                .id();
            if let Some((shell_mesh, shell_mat)) = shell {
                commands.entity(entity).with_children(|p| {
                    p.spawn((
                        PbrBundle {
                            mesh: shell_mesh,
                            material: shell_mat,
                            ..default()
                        },
                        ShellMarker,
                    ));
                });
            }
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

    let Some(ray) = camera.viewport_to_world(cam_tf, cursor) else {
        st.ui.hovered = None;
        return;
    };
    let dir = *ray.direction;
    let mut best: Option<(f32, spacegraph_core::NodeId)> = None;
    for (id, pos) in st.spatial.placed_positions() {
        if !st.is_visible_rendered(id) {
            continue;
        }
        if let Some(t) = ray_sphere_t(ray.origin, dir, pos, PICK_RADIUS) {
            if best.as_ref().map(|(bt, _)| t < *bt).unwrap_or(true) {
                best = Some((t, id.clone()));
            }
        }
    }
    st.ui.hovered = best.map(|(_, id)| id);
}

/// Threshold (viewport px) above which an LMB drag becomes a box-select
/// instead of a single click.
const DRAG_THRESHOLD: f32 = 5.0;

#[allow(clippy::too_many_arguments)]
pub fn picking_focus(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
    mut contexts: EguiContexts,
    mut st: ResMut<GraphState>,
    mut out: EventWriter<Picked>,
    mut drag: ResMut<DragSelect>,
    fly: Res<FlyCam>,
) {
    if st.ui.view_mode == ViewMode::Timeline || fly.active {
        drag.start = None;
        return;
    }
    let egui_pointer = contexts.ctx_mut().wants_pointer_input();
    let Ok(window) = windows.get_single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, cam_tf)) = cam_q.get_single() else {
        return;
    };

    if buttons.just_pressed(MouseButton::Left) && !egui_pointer {
        drag.start = Some(cursor);
    }

    // While dragging far enough, draw the selection rectangle.
    if buttons.pressed(MouseButton::Left) {
        if let Some(start) = drag.start {
            if (cursor - start).length() > DRAG_THRESHOLD {
                let rect = egui::Rect::from_two_pos(
                    egui::pos2(start.x, start.y),
                    egui::pos2(cursor.x, cursor.y),
                );
                let painter = contexts.ctx_mut().layer_painter(egui::LayerId::new(
                    egui::Order::Background,
                    egui::Id::new("box_select"),
                ));
                painter.rect_filled(
                    rect,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(80, 180, 255, 24),
                );
                painter.rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 200, 255)),
                );
            }
        }
    }

    if buttons.just_released(MouseButton::Left) {
        let Some(start) = drag.start.take() else {
            return;
        };
        if (cursor - start).length() <= DRAG_THRESHOLD {
            // Click → single ray-pick (depth-correct, nearest sphere hit).
            if let Some(ray) = camera.viewport_to_world(cam_tf, cursor) {
                let dir = *ray.direction;
                let mut best: Option<(f32, spacegraph_core::NodeId)> = None;
                for (id, pos) in st.spatial.placed_positions() {
                    if !st.is_visible_rendered(id) {
                        continue;
                    }
                    if let Some(t) = ray_sphere_t(ray.origin, dir, pos, PICK_RADIUS) {
                        if best.as_ref().map(|(bt, _)| t < *bt).unwrap_or(true) {
                            best = Some((t, id.clone()));
                        }
                    }
                }
                if let Some((_, picked)) = best {
                    out.send(Picked(picked));
                }
            }
        } else {
            // Drag → box-select: every placed node whose screen point is in the
            // rectangle.
            let rect = Rect::from_corners(start, cursor);
            let mut selected = HashSet::new();
            for (id, pos) in st.spatial.placed_positions() {
                if !st.is_visible_rendered(id) {
                    continue;
                }
                if let Some(screen) = camera.world_to_viewport(cam_tf, pos) {
                    if rect.contains(screen) {
                        selected.insert(id.clone());
                    }
                }
            }
            st.ui.multi_selected = selected;
            st.needs_redraw.store(true, Ordering::Relaxed);
        }
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

/// How single-node selection/hover/focus is shown. Standard uses the in-world
/// egui reticle (`ui::reticle`) and suppresses gizmo bubbles; Minimal keeps the
/// gizmo bubbles for pre-visual-pass parity. Multi-select bubbles show in both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightStyle {
    Bubbles,
    Reticle,
}

pub fn highlight_style(theme: VisualTheme) -> HighlightStyle {
    match theme {
        VisualTheme::Standard => HighlightStyle::Reticle,
        VisualTheme::Minimal => HighlightStyle::Bubbles,
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

    // Single-node lock-on feedback. Standard draws an in-world egui reticle
    // (`ui::reticle`) and suppresses these gizmo bubbles; Minimal keeps them.
    if highlight_style(st.cfg.visual_theme) == HighlightStyle::Bubbles {
        let highlights = [
            (st.ui.hovered.clone(), theme::RETICLE_HOVER, 0.50),
            (st.ui.selected.clone(), theme::RETICLE_SELECT, 0.62),
            (st.ui.focus.clone(), theme::RETICLE_FOCUS, 0.72),
        ];
        for (maybe_id, color, radius) in highlights {
            if let Some(id) = maybe_id {
                if let Some(pos) = st.spatial.position_of(&id) {
                    gizmos.sphere(pos, Quat::IDENTITY, radius, color);
                }
            }
        }
    }
    // Box-selected nodes (both themes).
    for id in st.ui.multi_selected.iter() {
        if let Some(pos) = st.spatial.position_of(id) {
            gizmos.sphere(pos, Quat::IDENTITY, 0.55, Color::srgb(0.30, 0.90, 1.0));
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
            if !st.is_visible_rendered(id) {
                continue;
            }
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
        // Aggregated edges are drawn by the mesh edge renderer
        // (`update_edge_mesh`) so they participate in bloom. Raw edges are an
        // opt-in debugging overlay and stay as gizmos here.
        if st.cfg.show_raw_edges {
            for id in vis.iter() {
                if !st.is_visible_rendered(id) {
                    continue;
                }
                for edge in st.model.edges_for_node(id) {
                    if &edge.from != id
                        || !st.edge_visible(edge, &vis)
                        || !st.is_visible_rendered(&edge.to)
                    {
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
            core_mesh: std::array::from_fn(|i| Handle::weak_from_u128(1000 + i as u128)),
            shell_mesh: std::array::from_fn(|i| {
                matches!(
                    theme::NodeKind::ALL[i],
                    theme::NodeKind::RemoteHost | theme::NodeKind::Alert
                )
                .then(|| Handle::weak_from_u128(2000 + i as u128))
            }),
            shell_mat: std::array::from_fn(|i| Handle::weak_from_u128(3000 + i as u128)),
            minimal_mesh: Handle::weak_from_u128(99),
            standard: (0..theme::NodeKind::ALL.len())
                .map(|_| ramp.clone())
                .collect(),
            minimal_normal: Handle::default(),
            minimal_glow: Handle::default(),
        }
    }

    /// A graph state with a single placed, visible node of the given kind.
    fn one_node_state(node: spacegraph_core::Node, theme: VisualTheme) -> GraphState {
        use spacegraph_core::NodeId;
        let mut gs = GraphState::default();
        gs.cfg.visual_theme = theme;
        gs.cfg.max_visible_nodes = 16;
        gs.cfg.progressive_nodes_per_frame = 16;
        let id = NodeId("n".to_string());
        gs.model.nodes.insert(id, node);
        let vis = gs.visible_set_capped();
        gs.progressive_prepare(&vis);
        gs.spatial.vis_cache = vis;
        gs
    }

    /// The single node entity's mesh handle and whether it has a shell child.
    fn node_mesh_and_shell(app: &mut App) -> (Handle<Mesh>, bool) {
        let world = app.world_mut();
        let mut q = world.query_filtered::<(&Handle<Mesh>, Option<&Children>), With<NodeMarker>>();
        let (mesh, child_entities) = {
            let (mesh, children) = q.iter(world).next().expect("one node entity");
            (
                mesh.clone(),
                children
                    .map(|c| c.iter().copied().collect::<Vec<_>>())
                    .unwrap_or_default(),
            )
        };
        let has_shell = child_entities
            .iter()
            .any(|&e| world.get::<ShellMarker>(e).is_some());
        (mesh, has_shell)
    }

    fn run_sync(gs: GraphState) -> App {
        let mut app = App::new();
        app.insert_resource(gs)
            .insert_resource(NodeEntities::default())
            .insert_resource(RebuildNodeEntities::default())
            .insert_resource(dummy_render_resources())
            .add_systems(Update, sync_node_entities);
        app.update();
        app
    }

    fn process_node() -> spacegraph_core::Node {
        spacegraph_core::Node::Process {
            pid: 1,
            ppid: 0,
            exe: "x".to_string(),
            cmdline: "x".to_string(),
            uid: 0,
        }
    }

    #[test]
    fn ray_sphere_hits_and_misses() {
        // Ray from origin along -Z hits a sphere centred 5 ahead.
        let t = ray_sphere_t(Vec3::ZERO, Vec3::NEG_Z, Vec3::new(0.0, 0.0, -5.0), 0.5);
        assert!(t.is_some());
        assert!((t.unwrap() - 4.5).abs() < 1e-3);
        // Sphere off to the side → miss.
        assert!(ray_sphere_t(Vec3::ZERO, Vec3::NEG_Z, Vec3::new(5.0, 0.0, -5.0), 0.5).is_none());
        // Sphere behind the ray → no positive hit.
        assert!(ray_sphere_t(Vec3::ZERO, Vec3::NEG_Z, Vec3::new(0.0, 0.0, 5.0), 0.5).is_none());
    }

    #[test]
    fn spawns_one_entity_per_visible_node() {
        let mut app = App::new();
        app.insert_resource(graph_state(200))
            .insert_resource(NodeEntities::default())
            .insert_resource(RebuildNodeEntities::default())
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
            .insert_resource(RebuildNodeEntities::default())
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
            core_mesh: std::array::from_fn(|_| Handle::default()),
            shell_mesh: std::array::from_fn(|_| None),
            shell_mat: std::array::from_fn(|_| Handle::default()),
            minimal_mesh: Handle::default(),
            standard: (0..theme::NodeKind::ALL.len())
                .map(|_| ramp.clone())
                .collect(),
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
            .insert_resource(RebuildNodeEntities::default())
            .insert_resource(dummy_render_resources())
            .add_systems(Update, sync_node_entities);
        app.update();
        assert_eq!(node_entities(&mut app).len(), 0);
    }

    fn all_kind_nodes() -> [(spacegraph_core::Node, theme::NodeKind); 6] {
        use spacegraph_core::{FileKind, Node};
        [
            (process_node(), theme::NodeKind::Process),
            (
                Node::File {
                    path: "/x".to_string(),
                    inode: 1,
                    kind: FileKind::Regular,
                },
                theme::NodeKind::File,
            ),
            (
                Node::User {
                    uid: 1,
                    name: "u".to_string(),
                },
                theme::NodeKind::User,
            ),
            (
                Node::Socket {
                    proto: "tcp".to_string(),
                    local_addr: "0.0.0.0".to_string(),
                    local_port: 80,
                    state: "LISTEN".to_string(),
                },
                theme::NodeKind::Socket,
            ),
            (
                Node::RemoteHost {
                    addr: "1.2.3.4".to_string(),
                    rdns: None,
                },
                theme::NodeKind::RemoteHost,
            ),
            (
                Node::Alert {
                    source: "s".to_string(),
                    signature: "sig".to_string(),
                    severity: "high".to_string(),
                    ts: "t".to_string(),
                },
                theme::NodeKind::Alert,
            ),
        ]
    }

    #[test]
    fn standard_theme_uses_core_mesh_per_kind() {
        let res = dummy_render_resources();
        for (node, kind) in all_kind_nodes() {
            let mut app = run_sync(one_node_state(node, VisualTheme::Standard));
            let (mesh, _) = node_mesh_and_shell(&mut app);
            assert_eq!(
                mesh,
                res.core_mesh[kind.index()],
                "{kind:?} spawns with its per-kind core mesh"
            );
        }
    }

    #[test]
    fn shell_child_present_only_for_shelled_kinds_in_standard() {
        for (node, kind) in all_kind_nodes() {
            let mut app = run_sync(one_node_state(node, VisualTheme::Standard));
            let (_, has_shell) = node_mesh_and_shell(&mut app);
            let want = matches!(kind, theme::NodeKind::RemoteHost | theme::NodeKind::Alert);
            assert_eq!(has_shell, want, "{kind:?} shell child presence (Standard)");
        }
    }

    #[test]
    fn minimal_theme_uses_sphere_mesh_and_no_shell() {
        let res = dummy_render_resources();
        for (node, _) in all_kind_nodes() {
            let mut app = run_sync(one_node_state(node, VisualTheme::Minimal));
            let (mesh, has_shell) = node_mesh_and_shell(&mut app);
            assert_eq!(mesh, res.minimal_mesh, "Minimal uses the flat sphere mesh");
            assert!(!has_shell, "Minimal has no shell child");
        }
    }

    #[test]
    fn theme_switch_triggers_exactly_one_rebuild() {
        let mut app = App::new();
        app.insert_resource(graph_state(100))
            .insert_resource(NodeEntities::default())
            .insert_resource(RebuildNodeEntities::default())
            .insert_resource(dummy_render_resources())
            .add_systems(Update, sync_node_entities);
        app.update();
        let before = node_entities(&mut app);
        assert!(!before.is_empty());

        // Simulate a theme switch: one drain + respawn, flag cleared.
        app.world_mut().resource_mut::<RebuildNodeEntities>().0 = true;
        app.update();
        let after = node_entities(&mut app);
        assert_eq!(
            after.len(),
            before.len(),
            "node count preserved across rebuild"
        );
        assert!(
            before.is_disjoint(&after),
            "rebuild respawns entities (new ids)"
        );
        assert!(
            !app.world().resource::<RebuildNodeEntities>().0,
            "rebuild flag cleared after one rebuild"
        );

        // No further churn once the flag is clear.
        let steady = node_entities(&mut app);
        app.update();
        assert_eq!(steady, node_entities(&mut app), "no churn after rebuild");
    }
}
