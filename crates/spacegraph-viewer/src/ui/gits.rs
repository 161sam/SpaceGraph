//! GitS chrome helpers (MP-UI-GitS, P2) — builds the holographic panel look
//! (translucent dark fill, thin neon stroke, corner brackets, monospace headers)
//! from the existing `tokens`. This is **not** a parallel theme: Standard renders
//! the GitS chrome; Minimal returns the plain egui equivalent so the chrome
//! degrades flat. Used by the command rail, the HUD panels and (P3) the entity card.

use bevy_egui::egui;

use crate::ui::tokens::{alpha, color, radius, space, stroke_w};

/// Translucent GitS panel fill (the 3D scene reads faintly through the chrome).
pub fn panel_fill() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(
        color::SURFACE.r(),
        color::SURFACE.g(),
        color::SURFACE.b(),
        alpha::PANEL_FILL,
    )
}

/// Frame for a HUD panel / window. Standard → GitS holographic frame; Minimal →
/// the plain egui popup frame (flat, opaque), so the chrome degrades cleanly.
pub fn panel_frame(standard: bool) -> egui::Frame {
    if standard {
        egui::Frame::none()
            .fill(panel_fill())
            .stroke(egui::Stroke::new(stroke_w::HAIR, color::LINE))
            .inner_margin(egui::Margin::same(space::MD))
            .rounding(radius::PANEL)
    } else {
        egui::Frame::popup(&egui::Style::default())
    }
}

/// A GitS section header: a monospace, accent-tinted label with a little space
/// above. Replaces the old `panel::section_header`.
pub fn section_header(ui: &mut egui::Ui, title: &str, standard: bool) {
    ui.add_space(space::SM);
    let text = egui::RichText::new(format!("◢ {title}"))
        .monospace()
        .strong()
        .size(13.0);
    let text = if standard {
        text.color(color::ACCENT)
    } else {
        text
    };
    ui.label(text);
}

/// Paint GitS corner brackets just outside `rect` (Standard only) — the device-
/// schematic accent that frames a panel without a heavy border.
pub fn draw_brackets(painter: &egui::Painter, rect: egui::Rect, arm: f32) {
    let accent = egui::Color32::from_rgba_unmultiplied(
        color::ACCENT.r(),
        color::ACCENT.g(),
        color::ACCENT.b(),
        alpha::BRACKET,
    );
    let stroke = egui::Stroke::new(stroke_w::FRAME, accent);
    let r = rect.expand(2.0);
    for (corner, dx, dy) in [
        (r.left_top(), 1.0_f32, 1.0_f32),
        (r.right_top(), -1.0, 1.0),
        (r.left_bottom(), 1.0, -1.0),
        (r.right_bottom(), -1.0, -1.0),
    ] {
        painter.line_segment([corner, egui::pos2(corner.x + dx * arm, corner.y)], stroke);
        painter.line_segment([corner, egui::pos2(corner.x, corner.y + dy * arm)], stroke);
    }
}

/// Faint horizontal scanlines over a panel rect — the GitS "screen" texture.
/// Bounded line count; very low alpha so it reads as a CRT sheen over content.
fn draw_scanlines(painter: &egui::Painter, rect: egui::Rect) {
    let col = egui::Color32::from_rgba_unmultiplied(
        color::ACCENT.r(),
        color::ACCENT.g(),
        color::ACCENT.b(),
        alpha::SCANLINE,
    );
    let stroke = egui::Stroke::new(1.0, col);
    let mut y = rect.top() + 3.0;
    while y < rect.bottom() {
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            stroke,
        );
        y += 3.0;
    }
}

/// Frame the rect of a just-shown "screen" panel with corner brackets + a faint
/// scanline sheen (call with the window/area response rect). No-op under Minimal.
pub fn bracket_response(ctx: &egui::Context, rect: egui::Rect, standard: bool) {
    if !standard {
        return;
    }
    let painter = ctx.layer_painter(egui::LayerId::new(
        crate::ui::overlay::layer::PANEL,
        egui::Id::new("gits_brackets")
            .with(rect.min.x as i32)
            .with(rect.min.y as i32),
    ));
    draw_scanlines(&painter, rect);
    draw_brackets(&painter, rect, 14.0);
}
