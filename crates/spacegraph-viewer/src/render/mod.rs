#[cfg(feature = "audio")]
pub mod audio;
pub mod camera;
pub mod edges;
pub mod freefly;
pub mod gameplay;
pub mod pacing;
pub mod spatial;
pub mod theme;
pub mod timeline;

use bevy::prelude::*;
use bevy_egui::EguiContexts;

use crate::graph::{GraphState, ViewMode};
use crate::ui::UiLayout;

#[cfg(feature = "audio")]
pub use audio::{audio_triggers, setup_audio, AudioAssets};
pub use camera::{apply_jump_to, setup_scene, sync_visual_theme, update_tree_zoom};
pub use edges::{setup_edge_mesh, update_edge_mesh, EdgeMesh};
pub use freefly::{fly_camera, FlyCam};
pub use gameplay::{mission_tick, reveal_tick, scan_pulse, Mission, ScanPulse};
pub use pacing::{update_frame_pacing, FramePacing};
pub use spatial::{
    apply_picked_focus, draw_node_labels, draw_spatial, hover_detection_spatial, picking_focus,
    setup_node_render_resources, sync_node_entities, DragSelect, NodeEntities,
};
pub use timeline::draw_timeline;

#[allow(clippy::too_many_arguments)]
pub fn draw_scene(
    st: ResMut<GraphState>,
    gizmos: Gizmos,
    contexts: EguiContexts,
    layout: Res<UiLayout>,
    windows: Query<&Window>,
    buttons: Res<ButtonInput<MouseButton>>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
) {
    match st.ui.view_mode {
        ViewMode::Spatial | ViewMode::Tree => draw_spatial(st, gizmos, contexts),
        ViewMode::Timeline => draw_timeline(st, gizmos, contexts, layout, windows, buttons, cam_q),
    }
}
