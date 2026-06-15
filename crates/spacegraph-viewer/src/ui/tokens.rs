//! SpaceGraph design tokens (v0.5.0, spec §3.1) — the house token *semantics*
//! (palette roles, spacing, font roles) mirrored into egui. Parity with the
//! Smolitux house style, not a component import (D-1). The 3D scene palette lives
//! in `render::theme`; this module is the egui-chrome counterpart.

use bevy_egui::egui::Color32;

/// Colour roles for the GitS egui chrome — the MP-UI-GitS-polish palette,
/// reconciled with `render::theme` so chrome accents match the 3D node palette.
pub mod color {
    use super::Color32;

    /// Deepest background `#05090c` (window/panel base).
    pub const BG: Color32 = Color32::from_rgb(5, 9, 12);
    /// Raised surface `#08171c` (panels, group frames).
    pub const SURFACE: Color32 = Color32::from_rgb(8, 23, 28);
    /// Slightly raised (hovered widgets, headers).
    pub const SURFACE_HI: Color32 = Color32::from_rgb(14, 36, 42);
    /// Hairline separators / inactive strokes `#1d4a4c`.
    pub const LINE: Color32 = Color32::from_rgb(29, 74, 76);
    /// Primary accent — cyan `#2bb3a8` (active strokes, selection, corner brackets,
    /// Process). Also the focus colour family.
    pub const ACCENT: Color32 = Color32::from_rgb(43, 179, 168);
    /// Brighter cyan `#34d6c8` — active / focused / hovered emphasis.
    pub const ACCENT_HI: Color32 = Color32::from_rgb(52, 214, 200);
    /// Secondary accent — green `#6fe06f` (ok/connected, File).
    pub const ACCENT_GREEN: Color32 = Color32::from_rgb(111, 224, 111);
    /// Bright body text `#cfe9e5`.
    pub const TEXT: Color32 = Color32::from_rgb(207, 233, 229);
    /// Dimmed/secondary text `#88b8b2`.
    pub const TEXT_DIM: Color32 = Color32::from_rgb(136, 184, 178);

    // --- Per-type semantic accents (mirror `render::theme` node colours) ---
    pub const FILE: Color32 = ACCENT_GREEN; // #6fe06f
    pub const PROCESS: Color32 = Color32::from_rgb(43, 176, 208); // #2bb0d0 (cyan, distinct from File green)
    pub const SOCKET: Color32 = Color32::from_rgb(95, 168, 255); // #5fa8ff
    pub const USER: Color32 = Color32::from_rgb(245, 185, 66); // #f5b942
    pub const REMOTEHOST: Color32 = Color32::from_rgb(176, 155, 251); // #b09bfb

    /// Severity ramp (alerts): low = amber, medium = orange, high = red `#ff5d5d`.
    pub const SEV_LOW: Color32 = USER;
    pub const SEV_MED: Color32 = Color32::from_rgb(252, 146, 60);
    pub const SEV_HIGH: Color32 = Color32::from_rgb(255, 93, 93);
    pub const ALERT: Color32 = SEV_HIGH;
}

/// Spacing scale (px) — a small 4-based ramp.
pub mod space {
    pub const XS: f32 = 2.0;
    pub const SM: f32 = 4.0;
    pub const MD: f32 = 8.0;
    pub const LG: f32 = 12.0;
    pub const XL: f32 = 16.0;
}

/// Corner rounding (px) per surface role — flat, segmented GitS look.
pub mod radius {
    /// HUD panels / rail (nearly square).
    pub const PANEL: f32 = 2.0;
    /// Readout cards / entity card.
    pub const CARD: f32 = 3.0;
}

/// Stroke widths (px).
pub mod stroke_w {
    /// Hairline separators / inactive frames.
    pub const HAIR: f32 = 1.0;
    /// Active frame / corner brackets.
    pub const FRAME: f32 = 1.5;
}

/// Alpha (0–255) for translucent GitS surfaces drawn over the 3D scene, so the
/// graph reads faintly through the holographic chrome.
pub mod alpha {
    /// HUD panel / rail fill.
    pub const PANEL_FILL: u8 = 232;
    /// Backing scrim behind a readout (radial disc, etc.).
    pub const SCRIM: u8 = 170;
    /// Corner-bracket accent.
    pub const BRACKET: u8 = 150;
    /// Scanline sheen over a "screen" panel (very faint CRT texture).
    pub const SCANLINE: u8 = 13;
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
            color::ACCENT_HI,
            color::ACCENT_GREEN,
            color::TEXT,
            color::TEXT_DIM,
            color::SOCKET,
            color::USER,
            color::REMOTEHOST,
            color::SEV_HIGH,
        ];
        for (i, a) in roles.iter().enumerate() {
            for b in &roles[i + 1..] {
                assert_ne!(a, b, "token colour roles must be distinct");
            }
        }
        assert_ne!(font::BODY, font::MONO);
    }
}
