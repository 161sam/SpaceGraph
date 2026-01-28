pub mod help;
pub mod hud;
pub mod layout;
pub mod panel;
pub mod search;
pub mod settings_agents;
pub mod settings_paths;
pub mod shortcuts;
pub mod tooltips;

pub const HUD_EDGE_PADDING: f32 = 10.0;
pub const HUD_MIN_CONTENT_W: f32 = 200.0;
pub const HUD_FALLBACK_Y_OFFSET: f32 = 220.0;

pub use help::help_overlay;
pub use hud::hud_overlay;
pub use layout::UiLayout;
pub use panel::ui_panel;
pub use shortcuts::handle_shortcuts;
