pub mod camera;
pub mod spatial;
pub mod timeline;

use bevy::prelude::*;
use bevy_egui::EguiContexts;

use crate::graph::{GraphState, ViewMode};

pub use camera::{apply_jump_to, setup_scene};
pub use spatial::{apply_picked_focus, draw_spatial, hover_detection_spatial, picking_focus};
pub use timeline::draw_timeline;

pub fn draw_scene(
    mut commands: Commands,
    mut st: ResMut<GraphState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(Entity, &spatial::NodeMarker)>,
    mut gizmos: Gizmos,
    mut contexts: EguiContexts,
    windows: Query<&Window>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
) {
    match st.ui.view_mode {
        ViewMode::Spatial => draw_spatial(commands, st, meshes, mats, query, gizmos, contexts),
        ViewMode::Timeline => draw_timeline(st, gizmos, contexts, windows, cam_q),
    }
}
