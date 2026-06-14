//! egui GitS reskin (v0.5.0, spec §3.1) — embeds the OFL fonts and builds the
//! `egui::Visuals`/`Style` from the design tokens. The reskin restyles all panels
//! (appearance only, behaviour unchanged). `VisualTheme::Minimal` keeps the plain
//! egui look (no GitS chrome), per the behavioural-equivalence baseline.

use bevy_egui::egui::{self, FontData, FontDefinitions, FontFamily};
use bevy_egui::EguiContexts;

use crate::graph::GraphState;
use crate::ui::tokens::{color, font, space};
use crate::util::config::VisualTheme;

// Committed OFL font assets (embedded like the audio WAVs — one-time asset
// addition, not a runtime/build network dependency).
const INTER: &[u8] = include_bytes!("../../assets/fonts/inter/Inter-Regular.ttf");
const JETBRAINS: &[u8] =
    include_bytes!("../../assets/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf");
const SPACE_GROTESK: &[u8] =
    include_bytes!("../../assets/fonts/space-grotesk/SpaceGrotesk-Regular.ttf");

/// Install the embedded fonts: Inter → proportional body, JetBrains Mono →
/// monospace, Space Grotesk → a named "headers" family.
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert(font::BODY.to_string(), FontData::from_static(INTER));
    fonts
        .font_data
        .insert(font::MONO.to_string(), FontData::from_static(JETBRAINS));
    fonts.font_data.insert(
        font::HEADER.to_string(),
        FontData::from_static(SPACE_GROTESK),
    );

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, font::BODY.to_string());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, font::MONO.to_string());
    fonts
        .families
        .entry(FontFamily::Name(font::HEADER.into()))
        .or_default()
        .insert(0, font::HEADER.to_string());

    ctx.set_fonts(fonts);
}

/// GitS dark visuals from the tokens (Standard theme).
pub fn gits_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.override_text_color = Some(color::TEXT);
    v.window_fill = color::BG;
    v.panel_fill = color::BG;
    v.extreme_bg_color = color::BG;
    v.faint_bg_color = color::SURFACE;
    v.window_stroke = egui::Stroke::new(1.0, color::LINE);
    v.selection.bg_fill = color::ACCENT.linear_multiply(0.35);
    v.selection.stroke = egui::Stroke::new(1.0, color::ACCENT);
    v.hyperlink_color = color::ACCENT;

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = color::SURFACE;
    w.noninteractive.weak_bg_fill = color::SURFACE;
    w.noninteractive.bg_stroke = egui::Stroke::new(1.0, color::LINE);
    w.noninteractive.fg_stroke = egui::Stroke::new(1.0, color::TEXT_DIM);
    w.inactive.bg_fill = color::SURFACE;
    w.inactive.weak_bg_fill = color::SURFACE;
    w.inactive.bg_stroke = egui::Stroke::new(1.0, color::LINE);
    w.inactive.fg_stroke = egui::Stroke::new(1.0, color::TEXT);
    w.hovered.bg_fill = color::SURFACE_HI;
    w.hovered.weak_bg_fill = color::SURFACE_HI;
    w.hovered.bg_stroke = egui::Stroke::new(1.0, color::ACCENT);
    w.hovered.fg_stroke = egui::Stroke::new(1.0, color::TEXT);
    w.active.bg_fill = color::SURFACE_HI;
    w.active.weak_bg_fill = color::SURFACE_HI;
    w.active.bg_stroke = egui::Stroke::new(1.0, color::ACCENT);
    w.active.fg_stroke = egui::Stroke::new(1.0, color::ACCENT);

    // Flat, segmented look — minimal rounding.
    let r = egui::Rounding::same(2.0);
    w.noninteractive.rounding = r;
    w.inactive.rounding = r;
    w.hovered.rounding = r;
    w.active.rounding = r;
    v
}

fn gits_style() -> egui::Style {
    let mut s = egui::Style::default();
    s.spacing.item_spacing = egui::vec2(space::MD, space::SM);
    s.spacing.button_padding = egui::vec2(space::MD, space::SM);
    s.spacing.window_margin = egui::Margin::same(space::MD);
    s.visuals = gits_visuals();
    s
}

/// Install fonts once and apply the egui chrome theme, swapping on `VisualTheme`
/// change. Standard → the GitS reskin; Minimal → plain egui dark (behavioural-
/// equivalence baseline). Runs in `Update` so the egui context exists (it is not
/// yet created during `Startup`).
pub fn apply_egui_theme(
    mut contexts: EguiContexts,
    st: Res<GraphState>,
    mut fonts_done: Local<bool>,
    mut last: Local<Option<VisualTheme>>,
) {
    if *fonts_done && *last == Some(st.cfg.visual_theme) {
        return;
    }
    let Some(ctx) = contexts.try_ctx_mut() else {
        return; // context not ready yet — retry next frame
    };
    if !*fonts_done {
        install_fonts(ctx);
        *fonts_done = true;
    }
    match st.cfg.visual_theme {
        VisualTheme::Standard => ctx.set_style(gits_style()),
        VisualTheme::Minimal => ctx.set_style(egui::Style::default()),
    }
    *last = Some(st.cfg.visual_theme);
}

use bevy::prelude::{Local, Res};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gits_visuals_use_accent_selection() {
        let v = gits_visuals();
        assert_eq!(v.selection.stroke.color, color::ACCENT);
        assert_eq!(v.panel_fill, color::BG);
        // Distinct from the plain default (Minimal path).
        assert_ne!(v.panel_fill, egui::Visuals::dark().panel_fill);
    }
}
