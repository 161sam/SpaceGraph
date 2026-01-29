pub mod camera;
pub mod spatial;
pub mod timeline;

use bevy::prelude::*;
use bevy_egui::EguiContexts;

use crate::graph::{GraphState, ViewMode};
use crate::ui::UiLayout;

pub use camera::{apply_jump_to, setup_scene, update_tree_zoom};
pub use spatial::{apply_picked_focus, draw_spatial, hover_detection_spatial, picking_focus};
pub use timeline::draw_timeline;

#[allow(clippy::too_many_arguments)]
pub fn draw_scene(
    commands: Commands,
    st: ResMut<GraphState>,
    meshes: ResMut<Assets<Mesh>>,
    mats: ResMut<Assets<StandardMaterial>>,
    query: Query<(Entity, &spatial::NodeMarker)>,
    gizmos: Gizmos,
    contexts: EguiContexts,
    layout: Res<UiLayout>,
    windows: Query<&Window>,
    buttons: Res<ButtonInput<MouseButton>>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
) {
    match st.ui.view_mode {
        ViewMode::Spatial | ViewMode::Tree => {
            draw_spatial(commands, st, meshes, mats, query, gizmos, contexts)
        }
        ViewMode::Timeline => draw_timeline(st, gizmos, contexts, layout, windows, buttons, cam_q),
    }
}
