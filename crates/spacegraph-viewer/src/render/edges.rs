//! Mesh-based edge rendering.
//!
//! Aggregated edges are drawn as a single batched `LineList` mesh with an unlit
//! material and **per-vertex HDR colours**, so the lines write bright values to
//! the HDR target and participate in bloom (unlike gizmo lines). The mesh is
//! rebuilt each frame from the rendered edge set (node positions move every
//! frame). Raw edges + the activity pulse stay as gizmos in `spatial.rs`.

use bevy::prelude::*;
use bevy::render::mesh::PrimitiveTopology;
use bevy::render::render_asset::RenderAssetUsages;
use std::collections::HashSet;

use crate::graph::model::{AggEdgeKey, EdgeKindClass};
use crate::graph::{GraphState, ViewMode};
use crate::render::theme;
use crate::util::config::{LodEdgesMode, VisualTheme};

/// Handle to the shared edge line mesh (mutated in place each frame).
#[derive(Resource)]
pub struct EdgeMesh {
    pub handle: Handle<Mesh>,
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
    commands.insert_resource(EdgeMesh { handle });
}

/// Rebuild the edge line mesh from the currently rendered aggregated edges.
pub fn update_edge_mesh(
    st: Res<GraphState>,
    edge_mesh: Res<EdgeMesh>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(mesh) = meshes.get_mut(&edge_mesh.handle) else {
        return;
    };

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();

    let vis = &st.spatial.vis_cache;
    let show = st.ui.view_mode == ViewMode::Spatial && st.ui.show_edges && st.cfg.show_agg_edges;
    // Honour the same LOD edge policy the gizmo path used: on large graphs the
    // configured mode (Off / focus-only / all) applies; otherwise draw all.
    let edges_mode = if st.cfg.lod_active(vis.len()) {
        st.cfg.lod_edges_mode
    } else {
        LodEdgesMode::All
    };
    if show && edges_mode != LodEdgesMode::Off {
        // Standard theme pushes colours into HDR (>1.0) so the lines bloom.
        let mul = if st.cfg.visual_theme == VisualTheme::Standard {
            2.5_f32
        } else {
            1.0
        };

        // In focus-only mode, restrict to edges incident to the focused /
        // selected / compare nodes; otherwise sweep the whole visible set.
        let mut sources: Vec<&spacegraph_core::NodeId> = Vec::new();
        if edges_mode == LodEdgesMode::FocusOnly {
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

        let focus_only = edges_mode == LodEdgesMode::FocusOnly;
        let mut seen: HashSet<AggEdgeKey> = HashSet::new();
        for id in sources {
            for edge in st.model.edges_for_node(id) {
                // For a full sweep we only want each undirected edge once, keyed
                // by `from`; in focus-only we accept edges from either endpoint.
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

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
}
