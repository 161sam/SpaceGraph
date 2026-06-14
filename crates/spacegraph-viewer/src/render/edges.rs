//! Mesh-based edge rendering.
//!
//! Aggregated edges are drawn as a single batched `LineList` mesh with an unlit
//! material and **per-vertex HDR colours**, so the lines write bright values to
//! the HDR target and participate in bloom (unlike gizmo lines). Raw edges + the
//! activity pulse stay as gizmos in `spatial.rs`.
//!
//! Two cost controls: the vertex buffers are **reused** across frames (no
//! per-frame allocation), and a **fingerprint** of everything that affects edge
//! geometry/colour (besides moving node positions) lets the rebuild be skipped
//! entirely when nothing changed. The mesh is world-space, so pure camera moves,
//! hover, free-fly and the scan pulse — which redraw the frame but don't move
//! edges — cost nothing here once the layout has settled.

use bevy::prelude::*;
use bevy::render::mesh::{PrimitiveTopology, VertexAttributeValues};
use bevy::render::render_asset::RenderAssetUsages;
use spacegraph_core::NodeId;
use std::collections::HashSet;

use crate::graph::model::{AggEdgeKey, EdgeKindClass};
use crate::graph::{GraphState, ViewMode};
use crate::render::theme;
use crate::util::config::{LodEdgesMode, VisualTheme};

/// Camera-position quantization cell (world units) for the edge-LOD fingerprint:
/// the mesh rebuilds only when the camera crosses a cell boundary, not on every
/// micro-move — so the "settled → cheap" property is preserved while distance LOD
/// stays responsive.
const EDGE_LOD_CELL: f32 = 12.0;

/// Edge render level-of-detail by camera distance + focus state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeLod {
    /// Draw at full brightness.
    Full,
    /// Draw dimmed (less HDR → less bloom + overdraw).
    Dim,
    /// Do not draw (fewer vertices, less overdraw).
    Cull,
}

/// Classify an edge for rendering — the v0.5.1 edge-perf lever. In **Focus Mode**
/// (`focus_cull` on), edges not incident to the focused node are culled (the strong
/// reduction that makes the focused subgraph the subject); incident edges stay
/// Full. Outside Focus Mode, edges **dim** past `near` and **cull** past `far`
/// (discrete distance bands, so a small camera move doesn't reclassify). Pure +
/// unit-tested. `force_step` (layout truth) is untouched — this is render-side only.
pub fn edge_lod(
    mid_dist: f32,
    near: f32,
    far: f32,
    focus_incident: bool,
    focus_cull: bool,
) -> EdgeLod {
    if focus_cull {
        return if focus_incident {
            EdgeLod::Full
        } else {
            EdgeLod::Cull
        };
    }
    if mid_dist <= near {
        EdgeLod::Full
    } else if mid_dist <= far {
        EdgeLod::Dim
    } else {
        EdgeLod::Cull
    }
}

/// Quantize a camera position into an integer cell for the rebuild fingerprint.
fn cam_cell(pos: Vec3) -> (i32, i32, i32) {
    (
        (pos.x / EDGE_LOD_CELL).floor() as i32,
        (pos.y / EDGE_LOD_CELL).floor() as i32,
        (pos.z / EDGE_LOD_CELL).floor() as i32,
    )
}

/// Everything (besides moving node positions) that changes the edge mesh. When
/// the layout is settled and this is unchanged, the rebuild is skipped.
#[derive(PartialEq)]
struct EdgeFingerprint {
    spatial: bool,
    show: bool,
    standard_theme: bool,
    lod_active: bool,
    lod_mode: LodEdgesMode,
    fog: bool,
    revealed_len: usize,
    vis_len: usize,
    focus: Option<NodeId>,
    selected: Option<NodeId>,
    sel_a: Option<NodeId>,
    sel_b: Option<NodeId>,
    // Only affects rendering (via `is_visible_rendered`) while fog is on.
    hovered: Option<NodeId>,
    // Focus Mode subject (drives focus-mode edge culling) + the quantized camera
    // cell (drives distance LOD) — v0.5.1 edge-perf inputs.
    focus_mode: Option<NodeId>,
    cam_quant: (i32, i32, i32),
}

/// Handle to the shared edge line mesh plus reusable scratch buffers and the
/// last-built fingerprint.
#[derive(Resource)]
pub struct EdgeMesh {
    pub handle: Handle<Mesh>,
    positions: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    last: Option<EdgeFingerprint>,
}

#[derive(Component)]
pub struct EdgeMeshTag;

/// Create the batched edge mesh + its unlit (bloom-capable) material (startup).
pub fn setup_edge_mesh(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let mut mesh = Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
    let handle = meshes.add(mesh);

    let material = mats.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        ..default()
    });
    commands.spawn((
        PbrBundle {
            mesh: handle.clone(),
            material,
            ..default()
        },
        EdgeMeshTag,
    ));
    commands.insert_resource(EdgeMesh {
        handle,
        positions: Vec::new(),
        colors: Vec::new(),
        last: None,
    });
}

/// Rebuild the edge line mesh from the currently rendered aggregated edges —
/// only when node positions may have moved or an input changed.
pub fn update_edge_mesh(
    st: Res<GraphState>,
    mut edge_mesh: ResMut<EdgeMesh>,
    mut meshes: ResMut<Assets<Mesh>>,
    cam_q: Query<&GlobalTransform, With<Camera>>,
) {
    let vis = &st.spatial.vis_cache;
    let lod_active = st.cfg.lod_active(vis.len());
    let cam_pos = cam_q.get_single().ok().map(|t| t.translation());
    let fp = EdgeFingerprint {
        spatial: st.ui.view_mode == ViewMode::Spatial,
        show: st.ui.show_edges && st.cfg.show_agg_edges,
        standard_theme: st.cfg.visual_theme == VisualTheme::Standard,
        lod_active,
        lod_mode: st.cfg.lod_edges_mode,
        fog: st.cfg.fog_of_war,
        revealed_len: st.revealed.len(),
        vis_len: vis.len(),
        focus: st.ui.focus.clone(),
        selected: st.ui.selected.clone(),
        sel_a: st.ui.selected_a.clone(),
        sel_b: st.ui.selected_b.clone(),
        hovered: st.cfg.fog_of_war.then(|| st.ui.hovered.clone()).flatten(),
        focus_mode: st.ui.focus_mode.clone(),
        cam_quant: cam_pos.map(cam_cell).unwrap_or_default(),
    };

    // While the force layout is moving, node positions change every frame and the
    // mesh must follow; otherwise only an input change warrants a rebuild.
    let layout_moving = fp.spatial && st.cfg.layout_force && !st.spatial.layout_settled;
    if !layout_moving && edge_mesh.last.as_ref() == Some(&fp) {
        return;
    }

    let edges_mode = if lod_active {
        st.cfg.lod_edges_mode
    } else {
        LodEdgesMode::All
    };
    let mul = if fp.standard_theme { 2.5_f32 } else { 1.0 };

    // Build into the reused scratch buffers.
    let EdgeMesh {
        positions, colors, ..
    } = &mut *edge_mesh;
    positions.clear();
    colors.clear();

    // Edge-LOD inputs (v0.5.1): focus-mode culling vs distance dim/cull bands.
    let focus_cull_active = fp.focus_mode.is_some() && st.cfg.edge_lod.focus_cull;
    let near = st.cfg.edge_lod.near_dist;
    let far = st.cfg.edge_lod.far_dist;
    let dim_factor = st.cfg.edge_lod.far_dim;

    if fp.spatial && fp.show && edges_mode != LodEdgesMode::Off {
        // Focus-only restricts to edges incident to the focus/selection; a full
        // sweep walks the whole visible set.
        let mut sources: Vec<&NodeId> = Vec::new();
        let focus_only = edges_mode == LodEdgesMode::FocusOnly;
        if focus_only {
            for id in [
                &st.ui.focus,
                &st.ui.selected,
                &st.ui.selected_a,
                &st.ui.selected_b,
            ]
            .into_iter()
            .flatten()
            {
                if vis.contains(id) && st.is_visible_rendered(id) {
                    sources.push(id);
                }
            }
        } else {
            sources.extend(vis.iter().filter(|id| st.is_visible_rendered(id)));
        }

        let mut seen: HashSet<AggEdgeKey> = HashSet::new();
        for id in sources {
            for edge in st.model.edges_for_node(id) {
                if !focus_only && &edge.from != id {
                    continue;
                }
                if !vis.contains(&edge.from)
                    || !vis.contains(&edge.to)
                    || !st.is_visible_rendered(&edge.from)
                    || !st.is_visible_rendered(&edge.to)
                {
                    continue;
                }
                if !seen.insert(AggEdgeKey::new(edge)) {
                    continue;
                }
                let (Some(a), Some(b)) = (
                    st.spatial.position_of(&edge.from),
                    st.spatial.position_of(&edge.to),
                ) else {
                    continue;
                };
                // Edge LOD: cull distant / non-focused edges, dim the mid band.
                let lod = {
                    let mid = (a + b) * 0.5;
                    let mid_dist = cam_pos.map(|cp| mid.distance(cp)).unwrap_or(0.0);
                    let incident = fp
                        .focus_mode
                        .as_ref()
                        .is_some_and(|f| &edge.from == f || &edge.to == f);
                    edge_lod(mid_dist, near, far, incident, focus_cull_active)
                };
                if lod == EdgeLod::Cull {
                    continue;
                }
                let bright = if lod == EdgeLod::Dim { dim_factor } else { 1.0 };
                let c = theme::edge_color(EdgeKindClass::from_kind(&edge.kind)).to_linear();
                let col = [
                    c.red * mul * bright,
                    c.green * mul * bright,
                    c.blue * mul * bright,
                    1.0,
                ];
                positions.push(a.to_array());
                positions.push(b.to_array());
                colors.push(col);
                colors.push(col);
            }
        }
    }

    // Blit the scratch into the mesh's own buffers in place (reused capacity);
    // `get_mut` marks the asset changed so it re-uploads to the GPU.
    let handle = edge_mesh.handle.clone();
    if let Some(mesh) = meshes.get_mut(&handle) {
        if let Some(VertexAttributeValues::Float32x3(p)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        {
            p.clear();
            p.extend_from_slice(&edge_mesh.positions);
        }
        if let Some(VertexAttributeValues::Float32x4(c)) = mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR)
        {
            c.clear();
            c.extend_from_slice(&edge_mesh.colors);
        }
    }

    edge_mesh.last = Some(fp);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_lod_distance_bands() {
        let (near, far) = (70.0, 160.0);
        // Not focus mode: near → Full, mid band → Dim, far → Cull.
        assert_eq!(edge_lod(10.0, near, far, false, false), EdgeLod::Full);
        assert_eq!(edge_lod(70.0, near, far, false, false), EdgeLod::Full); // inclusive
        assert_eq!(edge_lod(120.0, near, far, false, false), EdgeLod::Dim);
        assert_eq!(edge_lod(200.0, near, far, false, false), EdgeLod::Cull);
    }

    #[test]
    fn edge_lod_focus_mode_culls_non_incident() {
        let (near, far) = (70.0, 160.0);
        // Focus-cull on: incident stays Full regardless of distance; others culled.
        assert_eq!(edge_lod(5.0, near, far, true, true), EdgeLod::Full);
        assert_eq!(edge_lod(5.0, near, far, false, true), EdgeLod::Cull);
        assert_eq!(
            edge_lod(300.0, near, far, true, true),
            EdgeLod::Full,
            "an incident edge is never distance-culled in focus mode"
        );
    }

    #[test]
    fn cam_cell_quantizes_position() {
        // Nearby points share a cell; crossing a boundary changes it (so the
        // rebuild fingerprint is stable under small camera moves).
        assert_eq!(
            cam_cell(Vec3::new(1.0, 1.0, 1.0)),
            cam_cell(Vec3::new(5.0, 2.0, 3.0))
        );
        assert_ne!(
            cam_cell(Vec3::ZERO),
            cam_cell(Vec3::new(EDGE_LOD_CELL + 0.1, 0.0, 0.0))
        );
    }
}
