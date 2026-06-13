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
) {
    let vis = &st.spatial.vis_cache;
    let lod_active = st.cfg.lod_active(vis.len());
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
                let c = theme::edge_color(EdgeKindClass::from_kind(&edge.kind)).to_linear();
                let col = [c.red * mul, c.green * mul, c.blue * mul, 1.0];
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
