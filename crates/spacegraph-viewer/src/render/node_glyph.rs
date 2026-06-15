//! Gate-glyph node LOD layer (v0.5.0, spec §3.3) — a billboarded concentric-ring
//! "gate" glyph on every visible node in the Standard theme. Together with the
//! v0.4.1 face icon (the centre) it forms the GitS gate unit.
//!
//! Built as a shared `LineList` ring mesh (the `edges.rs`/shell pattern), unlit
//! emissive so it blooms, per-kind material (instanced), camera-faced by a cheap
//! per-frame rotation copy. **No WGSL, no new dependency.**
//!
//! Node-representation matrix (theme × tier):
//! - `Minimal` → flat sphere at all tiers, **no glyph** (untouched baseline).
//! - `Standard` → glyph on every node; the 3D per-type silhouette is the *near*
//!   LOD at Medium/High (`gates.silhouettes`), suppressed at Potato/Low and at the
//!   far band so the glyph is the **primary** representation.

use std::collections::HashMap;
use std::f32::consts::TAU;

use bevy::prelude::*;
use bevy::render::mesh::PrimitiveTopology;
use bevy::render::render_asset::RenderAssetUsages;

use spacegraph_core::Node;

use crate::graph::interner::NodeIndex;
use crate::graph::{GraphState, ViewMode};
use crate::render::theme;
use crate::util::config::VisualTheme;

const KIND_COUNT: usize = theme::NodeKind::ALL.len();
/// Beyond this camera distance the silhouette drops out and the glyph is primary
/// even at Medium/High (the far LOD band).
pub const FAR_DIST: f32 = 45.0;
/// HDR emissive multiplier so the ring blooms in the Standard theme.
const GLYPH_GLOW: f32 = 4.0;

/// The glyph layer is drawn only in the Standard theme (Minimal stays flat).
pub fn glyph_layer_active(theme: VisualTheme) -> bool {
    theme == VisualTheme::Standard
}

/// Gate-ring colour for a node: the per-type base colour, or the alert **severity**
/// ramp (low = amber, medium = orange, high/critical = red) for alerts. Pure +
/// unit-tested — the single source of truth the shared materials are built from.
pub fn ring_color(kind: theme::NodeKind, severity: Option<&str>) -> Color {
    match (kind, severity) {
        (theme::NodeKind::Alert, Some(sev)) => theme::alert_severity_color(sev),
        (theme::NodeKind::Alert, None) => theme::ALERT,
        (k, _) => k.base_color(),
    }
}

/// Alert severity → shared-material index (low / medium / high+).
fn severity_index(severity: &str) -> usize {
    match severity {
        "low" => 0,
        "medium" => 1,
        _ => 2, // high / critical / unknown
    }
}

/// Whether the 3D per-type silhouette should render for a node at `dist` from the
/// camera: Standard + the tier permits silhouettes + within the near band. When
/// false the gate-glyph is the primary representation (Potato/Low, or the far LOD).
pub fn silhouette_active(
    theme: VisualTheme,
    gates_silhouettes: bool,
    dist: f32,
    far_dist: f32,
) -> bool {
    theme == VisualTheme::Standard && gates_silhouettes && dist <= far_dist
}

/// Marker on a gate-glyph entity.
#[derive(Component)]
pub struct NodeGlyphMarker;

/// Back-reference from a glyph entity to its node.
#[derive(Component)]
pub struct GlyphRef(pub NodeIndex);

/// Persistent glyph entities, one per visible node (spawn on add / despawn on
/// remove — no per-frame churn).
#[derive(Resource, Default)]
pub struct NodeGlyphs {
    pub map: HashMap<NodeIndex, Entity>,
}

/// Shared gate-glyph resources: one ring mesh + per-kind emissive materials +
/// the alert severity ramp. All shared/instanced — **no per-node allocation**.
#[derive(Resource)]
pub struct NodeGlyphResources {
    pub ring_mesh: Handle<Mesh>,
    pub mat: [Handle<StandardMaterial>; KIND_COUNT],
    /// Alert severity ramp materials (low / medium / high), shared + instanced.
    pub alert_mat: [Handle<StandardMaterial>; 3],
}

/// The gate glyph as a single shared `LineList`: a centre dot, two concentric
/// gate arcs, and outer **tick-marks** (the "gate" graduations — longer at the
/// four cardinals). XY plane, billboarded at runtime. One mesh for every node.
fn ring_mesh() -> Mesh {
    let segs = 48u32;
    let mut pos: Vec<[f32; 3]> = Vec::new();
    // Concentric gate arcs: centre dot + two rings.
    for &r in &[0.07_f32, 0.34, 0.50] {
        for i in 0..segs {
            let a0 = i as f32 / segs as f32 * TAU;
            let a1 = (i + 1) as f32 / segs as f32 * TAU;
            pos.push([r * a0.cos(), r * a0.sin(), 0.0]);
            pos.push([r * a1.cos(), r * a1.sin(), 0.0]);
        }
    }
    // Tick-marks radiating from the outer ring — the gate's registration marks
    // (every 6th is longer, marking the cardinals).
    let ticks = 24u32;
    for i in 0..ticks {
        let a = i as f32 / ticks as f32 * TAU;
        let (c, s) = (a.cos(), a.sin());
        let r1 = if i % 6 == 0 { 0.62 } else { 0.56 };
        pos.push([0.50 * c, 0.50 * s, 0.0]);
        pos.push([r1 * c, r1 * s, 0.0]);
    }
    Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
}

/// An unlit, HDR-emissive glyph material so the ring blooms in the Standard theme.
fn glyph_material(color: Color) -> StandardMaterial {
    let c = color.to_linear();
    StandardMaterial {
        base_color: color,
        emissive: LinearRgba::rgb(
            c.red * GLYPH_GLOW,
            c.green * GLYPH_GLOW,
            c.blue * GLYPH_GLOW,
        ),
        unlit: true,
        ..default()
    }
}

pub fn setup_node_glyph_resources(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let ring_mesh = meshes.add(ring_mesh());
    let mat =
        std::array::from_fn(|i| mats.add(glyph_material(theme::NodeKind::ALL[i].base_color())));
    let alert_mat = std::array::from_fn(|i| {
        let sev = ["low", "medium", "high"][i];
        mats.add(glyph_material(theme::alert_severity_color(sev)))
    });
    commands.insert_resource(NodeGlyphResources {
        ring_mesh,
        mat,
        alert_mat,
    });
}

/// Keep gate-glyphs in sync with the visible graph and billboard them to the
/// camera. Standard + spatial + non-LOD only; spawns on add / despawns on remove.
#[allow(clippy::type_complexity)]
pub fn sync_node_glyphs(
    mut commands: Commands,
    st: Res<GraphState>,
    res: Res<NodeGlyphResources>,
    mut glyphs: ResMut<NodeGlyphs>,
    cam_q: Query<&GlobalTransform, With<Camera>>,
    mut q: Query<(&mut Transform, &mut Handle<StandardMaterial>), With<NodeGlyphMarker>>,
) {
    let vis = &st.spatial.vis_cache;
    let active = st.ui.view_mode == ViewMode::Spatial
        && !st.cfg.lod_active(vis.len())
        && glyph_layer_active(st.cfg.visual_theme);

    if !active {
        if !glyphs.map.is_empty() {
            for (_, e) in glyphs.map.drain() {
                commands.entity(e).despawn_recursive();
            }
        }
        return;
    }

    let Ok(cam_tf) = cam_q.get_single() else {
        return;
    };
    let (_, cam_rot, _) = cam_tf.to_scale_rotation_translation();

    glyphs.map.retain(|&idx, &mut e| {
        let keep = st.spatial.index_visible(idx, vis)
            && st
                .spatial
                .interner
                .resolve(idx)
                .map(|id| st.is_visible_rendered(id) && st.ui.focus_mode.as_ref() != Some(id))
                .unwrap_or(false);
        if !keep {
            commands.entity(e).despawn_recursive();
        }
        keep
    });

    for id in vis.iter() {
        let Some(idx) = st.spatial.index_of(id) else {
            continue;
        };
        // Skip the focus subject so its node region stays the clean reticle + single
        // indicator ring (the gate-glyph otherwise reads as a busy "eye" on it).
        if !st.spatial.placed[idx.slot()]
            || !st.is_visible_rendered(id)
            || st.ui.focus_mode.as_ref() == Some(id)
        {
            continue;
        }
        let Some(node) = st.core.model.nodes.get(id) else {
            continue;
        };
        let kind = theme::NodeKind::of(node);
        let pos = st.spatial.positions[idx.slot()];
        // Type colour for most kinds; alerts ramp by severity (shared materials).
        let material = match node {
            Node::Alert { severity, .. } => res.alert_mat[severity_index(severity)].clone(),
            _ => res.mat[kind.index()].clone(),
        };

        if let Some(&e) = glyphs.map.get(&idx) {
            if let Ok((mut tf, mut mat)) = q.get_mut(e) {
                tf.translation = pos;
                tf.rotation = cam_rot; // screen-aligned billboard
                if *mat != material {
                    *mat = material;
                }
            }
        } else {
            let e = commands
                .spawn((
                    PbrBundle {
                        mesh: res.ring_mesh.clone(),
                        material,
                        transform: Transform {
                            translation: pos,
                            rotation: cam_rot,
                            ..default()
                        },
                        ..default()
                    },
                    NodeGlyphMarker,
                    GlyphRef(idx),
                ))
                .id();
            glyphs.map.insert(idx, e);
        }
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

    fn dummy_resources() -> NodeGlyphResources {
        NodeGlyphResources {
            ring_mesh: Handle::weak_from_u128(9000),
            mat: std::array::from_fn(|i| Handle::weak_from_u128(9100 + i as u128)),
            alert_mat: std::array::from_fn(|i| Handle::weak_from_u128(9200 + i as u128)),
        }
    }

    fn glyph_count(app: &mut App) -> usize {
        let mut q = app
            .world_mut()
            .query_filtered::<Entity, With<NodeGlyphMarker>>();
        q.iter(app.world()).count()
    }

    fn spawn_camera(app: &mut App) {
        app.world_mut().spawn((
            Camera::default(),
            GlobalTransform::from_translation(Vec3::new(0.0, 0.0, 120.0)),
        ));
    }

    #[test]
    fn lod_matrix_pure_fns() {
        assert!(glyph_layer_active(VisualTheme::Standard));
        assert!(!glyph_layer_active(VisualTheme::Minimal));
        // Medium/High (silhouettes on), near → silhouette shows.
        assert!(silhouette_active(
            VisualTheme::Standard,
            true,
            10.0,
            FAR_DIST
        ));
        // Far band → glyph primary even at Medium/High.
        assert!(!silhouette_active(
            VisualTheme::Standard,
            true,
            80.0,
            FAR_DIST
        ));
        // Potato/Low (silhouettes off) → glyph primary at any distance.
        assert!(!silhouette_active(
            VisualTheme::Standard,
            false,
            5.0,
            FAR_DIST
        ));
        // Minimal → no silhouette layer (flat sphere path).
        assert!(!silhouette_active(
            VisualTheme::Minimal,
            true,
            5.0,
            FAR_DIST
        ));
    }

    #[test]
    fn ring_color_maps_type_and_severity() {
        use theme::NodeKind;
        assert_eq!(ring_color(NodeKind::Process, None), theme::PROCESS);
        // Non-alert kinds ignore severity.
        assert_eq!(ring_color(NodeKind::File, Some("low")), theme::FILE);
        // Alert ramps by severity.
        assert_eq!(ring_color(NodeKind::Alert, Some("low")), theme::ALERT_LOW);
        assert_eq!(
            ring_color(NodeKind::Alert, Some("medium")),
            theme::ALERT_MEDIUM
        );
        assert_eq!(ring_color(NodeKind::Alert, Some("high")), theme::ALERT_HIGH);
        // Unknown severity → high (red); missing severity → base alert red.
        assert_eq!(
            ring_color(NodeKind::Alert, Some("weird")),
            theme::ALERT_HIGH
        );
        assert_eq!(ring_color(NodeKind::Alert, None), theme::ALERT);
        // severity_index agrees with the alert_mat layout.
        assert_eq!(severity_index("low"), 0);
        assert_eq!(severity_index("medium"), 1);
        assert_eq!(severity_index("critical"), 2);
    }

    #[test]
    fn glyphs_share_one_ring_mesh() {
        // Structural perf proxy: every gate-glyph instances the *same* shared ring
        // mesh handle — no per-node mesh allocation, regardless of node count.
        let mut app = App::new();
        let gs = graph_state(90);
        app.insert_resource(gs)
            .insert_resource(NodeGlyphs::default())
            .insert_resource(dummy_resources())
            .add_systems(Update, sync_node_glyphs);
        spawn_camera(&mut app);
        app.update();
        let shared = app
            .world()
            .resource::<NodeGlyphResources>()
            .ring_mesh
            .clone();
        let mut q = app
            .world_mut()
            .query_filtered::<&Handle<Mesh>, With<NodeGlyphMarker>>();
        let mut n = 0;
        for h in q.iter(app.world()) {
            assert_eq!(*h, shared, "every glyph must share the one ring mesh");
            n += 1;
        }
        assert!(n > 0, "expected gate-glyphs to be spawned");
    }

    #[test]
    fn glyph_per_visible_node_in_standard() {
        let mut app = App::new();
        let gs = graph_state(100);
        let want = gs.spatial.vis_cache.len();
        app.insert_resource(gs)
            .insert_resource(NodeGlyphs::default())
            .insert_resource(dummy_resources())
            .add_systems(Update, sync_node_glyphs);
        spawn_camera(&mut app);
        app.update();
        assert!(want > 0);
        assert_eq!(
            glyph_count(&mut app),
            want,
            "one gate-glyph per visible node"
        );
    }

    #[test]
    fn minimal_theme_has_no_glyphs() {
        let mut app = App::new();
        let mut gs = graph_state(60);
        gs.cfg.visual_theme = VisualTheme::Minimal;
        app.insert_resource(gs)
            .insert_resource(NodeGlyphs::default())
            .insert_resource(dummy_resources())
            .add_systems(Update, sync_node_glyphs);
        spawn_camera(&mut app);
        app.update();
        assert_eq!(glyph_count(&mut app), 0, "Minimal draws no gate-glyphs");
    }

    #[test]
    fn glyphs_have_no_steady_state_churn() {
        let mut app = App::new();
        app.insert_resource(graph_state(120))
            .insert_resource(NodeGlyphs::default())
            .insert_resource(dummy_resources())
            .add_systems(Update, sync_node_glyphs);
        spawn_camera(&mut app);
        app.update();
        let frame1 = {
            let mut q = app
                .world_mut()
                .query_filtered::<Entity, With<NodeGlyphMarker>>();
            q.iter(app.world())
                .collect::<std::collections::HashSet<_>>()
        };
        assert!(!frame1.is_empty());
        app.update();
        let frame2 = {
            let mut q = app
                .world_mut()
                .query_filtered::<Entity, With<NodeGlyphMarker>>();
            q.iter(app.world())
                .collect::<std::collections::HashSet<_>>()
        };
        assert_eq!(frame1, frame2, "steady state must not respawn glyphs");
    }
}
