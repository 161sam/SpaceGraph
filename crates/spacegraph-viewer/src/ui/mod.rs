pub mod command_palette;
pub mod context_menu;
pub mod focus;
pub mod gits;
pub mod help;
pub mod hud;
pub mod hud_panels;
pub mod inspector;
pub mod layout;
pub mod legend;
pub mod minimap;
pub mod node_preview;
pub mod overlay;
pub mod rail;
pub mod reticle;
pub mod search;
pub mod settings_agents;
pub mod settings_paths;
pub mod shortcuts;
pub mod theme_egui;
pub mod tokens;
pub mod tooltips;

pub const HUD_EDGE_PADDING: f32 = 10.0;

pub use command_palette::command_palette_overlay;
pub use context_menu::{context_menu_overlay, radial_hud, RadialMenu, RadialState};
pub use focus::{enter_focus, exit_focus, focus_double_click, focus_overlay};
pub use help::help_overlay;
pub use hud::{hud_frame_overlay, hud_overlay};
pub use hud_panels::{dispatch_windows, hud_panels};
pub use inspector::inspector_overlay;
pub use layout::UiLayout;
pub use legend::legend_overlay;
pub use minimap::minimap;
pub use node_preview::{
    node_preview_overlay, poll_preview_decodes, update_preview_requests, PreviewState,
};
pub use rail::{command_rail, update_ui_layout, RailState};
pub use reticle::reticle_overlay;
pub use shortcuts::handle_shortcuts;
pub use theme_egui::apply_egui_theme;

/// Convert a Bevy `Color` to an egui `Color32` (sRGB, opaque) for UI swatches.
pub fn egui_color(c: bevy::prelude::Color) -> bevy_egui::egui::Color32 {
    let s = c.to_srgba();
    bevy_egui::egui::Color32::from_rgb(
        (s.red * 255.0).round() as u8,
        (s.green * 255.0).round() as u8,
        (s.blue * 255.0).round() as u8,
    )
}
