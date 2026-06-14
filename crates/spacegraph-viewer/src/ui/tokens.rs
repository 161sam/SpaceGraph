//! SpaceGraph design tokens (v0.5.0, spec §3.1) — the house token *semantics*
//! (palette roles, spacing, font roles) mirrored into egui. Parity with the
//! Smolitux house style, not a component import (D-1). The 3D scene palette lives
//! in `render::theme`; this module is the egui-chrome counterpart.

use bevy_egui::egui::Color32;

/// Colour roles for the GitS egui chrome (dark, neon-on-black, cyan accent).
pub mod color {
    use super::Color32;

    /// Deepest background (window/panel base).
    pub const BG: Color32 = Color32::from_rgb(6, 10, 16);
    /// Raised surface (panels, group frames).
    pub const SURFACE: Color32 = Color32::from_rgb(12, 18, 26);
    /// Slightly raised (hovered widgets, headers).
    pub const SURFACE_HI: Color32 = Color32::from_rgb(20, 30, 42);
    /// Hairline separators / inactive strokes.
    pub const LINE: Color32 = Color32::from_rgb(38, 54, 70);
    /// Primary accent — cyan (active strokes, selection, corner brackets).
    pub const ACCENT: Color32 = Color32::from_rgb(60, 200, 220);
    /// Secondary accent — green (ok/connected).
    pub const ACCENT_GREEN: Color32 = Color32::from_rgb(90, 230, 150);
    /// Body text.
    pub const TEXT: Color32 = Color32::from_rgb(200, 224, 235);
    /// Dimmed/secondary text.
    pub const TEXT_DIM: Color32 = Color32::from_rgb(120, 145, 160);

    /// Severity ramp (alerts).
    pub const SEV_LOW: Color32 = Color32::from_rgb(220, 180, 70);
    pub const SEV_MED: Color32 = Color32::from_rgb(230, 140, 60);
    pub const SEV_HIGH: Color32 = Color32::from_rgb(235, 80, 80);
}

/// Spacing scale (px) — a small 4-based ramp.
pub mod space {
    pub const XS: f32 = 2.0;
    pub const SM: f32 = 4.0;
    pub const MD: f32 = 8.0;
    pub const LG: f32 = 12.0;
    pub const XL: f32 = 16.0;
}

/// Font-family role names registered in `theme_egui::setup_fonts`.
pub mod font {
    /// Body / proportional (Inter).
    pub const BODY: &str = "inter";
    /// UI monospace (JetBrains Mono).
    pub const MONO: &str = "jetbrains";
    /// Headers / display (Space Grotesk) — an egui `FontFamily::Name`.
    pub const HEADER: &str = "space_grotesk";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_are_distinct() {
        // A few key roles must not collide (catches copy-paste token errors).
        let roles = [
            color::BG,
            color::SURFACE,
            color::SURFACE_HI,
            color::LINE,
            color::ACCENT,
            color::ACCENT_GREEN,
            color::TEXT,
        ];
        for (i, a) in roles.iter().enumerate() {
            for b in &roles[i + 1..] {
                assert_ne!(a, b, "token colour roles must be distinct");
            }
        }
        assert_ne!(font::BODY, font::MONO);
    }
}
