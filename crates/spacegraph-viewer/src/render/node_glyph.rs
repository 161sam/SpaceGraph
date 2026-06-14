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

/// Shared gate-glyph resources: one ring mesh + per-kind emissive materials.
#[derive(Resource)]
pub struct NodeGlyphResources {
    pub ring_mesh: Handle<Mesh>,
    pub mat: [Handle<StandardMaterial>; KIND_COUNT],
}

/// Concentric ring `LineList` (centre dot + two rings), in the XY plane,
/// billboarded at runtime.
fn ring_mesh() -> Mesh {
    let segs = 48u32;
    let mut pos: Vec<[f32; 3]> = Vec::new();
    for &r in &[0.07_f32, 0.34, 0.50] {
        for i in 0..segs {
            let a0 = i as f32 / segs as f32 * TAU;
            let a1 = (i + 1) as f32 / segs as f32 * TAU;
            pos.push([r * a0.cos(), r * a0.sin(), 0.0]);
            pos.push([r * a1.cos(), r * a1.sin(), 0.0]);
        }
    }
    Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
}

pub fn setup_node_glyph_resources(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let ring_mesh = meshes.add(ring_mesh());
    let mat = std::array::from_fn(|i| {
        let kind = theme::NodeKind::ALL[i];
        let c = kind.base_color().to_linear();
        mats.add(StandardMaterial {
            base_color: kind.base_color(),
            emissive: LinearRgba::rgb(
                c.red * GLYPH_GLOW,
                c.green * GLYPH_GLOW,
                c.blue * GLYPH_GLOW,
            ),
            unlit: true,
            ..default()
        })
    });
    commands.insert_resource(NodeGlyphResources { ring_mesh, mat });
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
                .map(|id| st.is_visible_rendered(id))
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
        if !st.spatial.placed[idx.slot()] || !st.is_visible_rendered(id) {
            continue;
        }
        let Some(node) = st.model.nodes.get(id) else {
            continue;
        };
        let kind = theme::NodeKind::of(node);
        let pos = st.spatial.positions[idx.slot()];
        let material = res.mat[kind.index()].clone();

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
