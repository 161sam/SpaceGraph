#[cfg(feature = "audio")]
pub mod audio;
pub mod camera;
pub mod capability;
pub mod edges;
pub mod freefly;
pub mod gameplay;
pub mod interaction;
pub mod node_glyph;
pub mod node_icon;
pub mod node_mesh;
pub mod pacing;
pub mod postfx;
pub mod quality;
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
pub use capability::{detect_capability, resolve_detail, DetailCapability, EffectiveDetail};
pub use edges::{setup_edge_mesh, update_edge_mesh, EdgeMesh};
pub use freefly::{fly_camera, FlyCam};
pub use gameplay::{mission_tick, reveal_tick, scan_pulse, Mission, ScanPulse};
pub use interaction::{
    detect_preview_expand, setup_ripple_resources, trigger_alert_ripple, trigger_focus_ripple,
    update_focus_ripples, PreviewExpand, RippleTracker,
};
pub use node_glyph::{
    setup_node_glyph_resources, sync_node_glyphs, NodeGlyphMarker, NodeGlyphResources, NodeGlyphs,
};
pub use node_icon::{
    file_subtype, icon_for, setup_node_icon_resources, sync_node_icons, IconId, NodeIconMarker,
    NodeIconResources, NodeIcons,
};
pub use pacing::{update_frame_pacing, FramePacing};
pub use postfx::{sync_postfx, PostFxPlugin};
pub use quality::{
    adaptive_quality, apply_quality, detect_tier, effective_gates, QualityState, QualityTier,
};
pub use spatial::{
    apply_picked_focus, draw_node_labels, draw_spatial, highlight_style, hover_detection_spatial,
    picking_focus, rotate_node_rings, setup_node_render_resources, sync_node_entities,
    sync_node_rings, DragSelect, HighlightStyle, NodeEntities, NodeRings, RebuildNodeEntities,
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
