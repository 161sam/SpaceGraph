//! Top-down minimap / radar (egui overlay) — a quick spatial overview with the
//! camera position marked.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::graph::{GraphState, ViewMode};
use crate::render::theme::NodeKind;
use crate::ui::overlay::{corner_anchor, layer};
use crate::ui::UiLayout;

const MINIMAP_SIZE: f32 = 160.0;
const MAX_DOTS: usize = 800;

fn to_egui(c: Color) -> egui::Color32 {
    let s = c.to_srgba();
    egui::Color32::from_rgb(
        (s.red * 255.0) as u8,
        (s.green * 255.0) as u8,
        (s.blue * 255.0) as u8,
    )
}

pub fn minimap(
    mut contexts: EguiContexts,
    st: Res<GraphState>,
    layout: Res<UiLayout>,
    cam_q: Query<&Transform, With<Camera>>,
) {
    if st.ui.view_mode != ViewMode::Spatial {
        return;
    }
    let ctx = contexts.ctx_mut();
    // Anchor top-right within the content rect (clears the rail, top strip and the
    // inspector column) via the shared corner rule — no longer the bare screen edge.
    let content = if layout.content_rect.width() > 0.0 && layout.content_rect.height() > 0.0 {
        layout.content_rect
    } else {
        ctx.screen_rect()
    };
    let pos = corner_anchor(
        content,
        egui::Align2::RIGHT_TOP,
        egui::vec2(MINIMAP_SIZE, MINIMAP_SIZE),
        egui::vec2(12.0, 12.0),
    );
    egui::Area::new("minimap".into())
        .order(layer::PANEL)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            let (resp, painter) =
                ui.allocate_painter(egui::vec2(MINIMAP_SIZE, MINIMAP_SIZE), egui::Sense::hover());
            let rect = resp.rect;
            painter.rect_filled(
                rect,
                4.0,
                egui::Color32::from_rgba_unmultiplied(8, 14, 22, 190),
            );

            // Bounds over placed nodes (X/Z top-down).
            let mut min = Vec2::splat(f32::INFINITY);
            let mut max = Vec2::splat(f32::NEG_INFINITY);
            for (id, p) in st.spatial.placed_positions() {
                if !st.is_visible_rendered(id) {
                    continue;
                }
                let xz = Vec2::new(p.x, p.z);
                min = min.min(xz);
                max = max.max(xz);
            }
            if !min.is_finite() {
                return;
            }
            let span = (max - min).max(Vec2::splat(1.0));
            let to_screen = |x: f32, z: f32| {
                let nx = (x - min.x) / span.x;
                let nz = (z - min.y) / span.y;
                egui::pos2(
                    rect.left() + nx * rect.width(),
                    rect.top() + nz * rect.height(),
                )
            };

            for (i, (id, p)) in st.spatial.placed_positions().enumerate() {
                if i >= MAX_DOTS {
                    break;
                }
                if !st.is_visible_rendered(id) {
                    continue;
                }
                let color = st
                    .core
                    .model
                    .nodes
                    .get(id)
                    .map(|n| to_egui(NodeKind::of(n).base_color()))
                    .unwrap_or(egui::Color32::GRAY);
                painter.circle_filled(to_screen(p.x, p.z), 1.3, color);
            }

            // Camera marker.
            if let Ok(tf) = cam_q.get_single() {
                painter.circle_stroke(
                    to_screen(tf.translation.x, tf.translation.z),
                    4.0,
                    egui::Stroke::new(1.5, egui::Color32::WHITE),
                );
            }
        });
}
