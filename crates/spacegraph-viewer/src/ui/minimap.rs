//! Top-down minimap / radar (egui overlay) — live and accurate: real projected
//! node positions (type-coloured), a camera viewport frustum, a focus marker, and
//! click-to-fly. Spatial view only.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use bevy_panorbit_camera::PanOrbitCamera;

use crate::graph::{GraphState, ViewMode};
use crate::render::theme::{self, NodeKind};
use crate::ui::overlay::{corner_anchor, layer};
use crate::ui::{egui_color, UiLayout};
use crate::util::config::VisualTheme;

const MINIMAP_SIZE: f32 = 168.0;
const MAX_DOTS: usize = 800;
/// Fractional padding kept around the graph bounds so dots don't touch the edge.
const PAD: f32 = 0.08;

fn to_egui(c: Color) -> egui::Color32 {
    let s = c.to_srgba();
    egui::Color32::from_rgb(
        (s.red * 255.0) as u8,
        (s.green * 255.0) as u8,
        (s.blue * 255.0) as u8,
    )
}

/// Padded, **square** world-XZ bounds (`min_x, min_z, span`) over the placed graph
/// — fixed aspect + computed over the full set so the radar doesn't rubber-band as
/// nodes move or fog toggles. Pure.
pub fn minimap_bounds(points: &[(f32, f32)]) -> Option<(f32, f32, f32)> {
    if points.is_empty() {
        return None;
    }
    let (mut minx, mut minz, mut maxx, mut maxz) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &(x, z) in points {
        minx = minx.min(x);
        minz = minz.min(z);
        maxx = maxx.max(x);
        maxz = maxz.max(z);
    }
    let cx = (minx + maxx) * 0.5;
    let cz = (minz + maxz) * 0.5;
    let half = ((maxx - minx).max(maxz - minz) * 0.5).max(0.5);
    let s = half * (1.0 + PAD);
    Some((cx - s, cz - s, 2.0 * s))
}

/// Project world XZ to a fraction of the minimap rect in `[0,1]²`. Pure.
pub fn minimap_project(x: f32, z: f32, min_x: f32, min_z: f32, span: f32) -> (f32, f32) {
    let s = span.max(f32::EPSILON);
    (
        ((x - min_x) / s).clamp(0.0, 1.0),
        ((z - min_z) / s).clamp(0.0, 1.0),
    )
}

/// Inverse of [`minimap_project`]: a `[0,1]²` fraction back to world XZ (the
/// click-to-fly mapping). Pure.
pub fn minimap_unproject(fx: f32, fz: f32, min_x: f32, min_z: f32, span: f32) -> (f32, f32) {
    (min_x + fx * span, min_z + fz * span)
}

/// World point where the camera ray through viewport position `vp` meets the
/// ground plane `Y=0` (the layout plane), or `None` if the ray is ~parallel or
/// behind — used to project the view frustum onto the radar.
fn ground_hit(camera: &Camera, cam_tf: &GlobalTransform, vp: Vec2) -> Option<Vec3> {
    let ray = camera.viewport_to_world(cam_tf, vp)?;
    let dir = *ray.direction;
    if dir.y.abs() < 1e-5 {
        return None;
    }
    let t = -ray.origin.y / dir.y;
    (t > 0.0).then(|| ray.origin + dir * t)
}

pub fn minimap(
    mut contexts: EguiContexts,
    mut st: ResMut<GraphState>,
    layout: Res<UiLayout>,
    cam_q: Query<(&Camera, &GlobalTransform, Option<&PanOrbitCamera>)>,
) {
    if st.ui.view_mode != ViewMode::Spatial {
        return;
    }

    // --- gather everything from state up front (no `st` borrow inside the egui
    // closure, so the click can mutate it afterwards) ---
    let bounds_pts: Vec<(f32, f32)> = st
        .spatial
        .placed_positions()
        .map(|(_, p)| (p.x, p.z))
        .collect();
    let Some((min_x, min_z, span)) = minimap_bounds(&bounds_pts) else {
        return;
    };
    let dots: Vec<(f32, f32, egui::Color32)> = st
        .spatial
        .placed_positions()
        .filter(|(id, _)| st.is_visible_rendered(id))
        .take(MAX_DOTS)
        .map(|(id, p)| {
            let c = st
                .core
                .model
                .nodes
                .get(id)
                .map(|n| to_egui(NodeKind::of(n).base_color()))
                .unwrap_or(egui::Color32::GRAY);
            (p.x, p.z, c)
        })
        .collect();
    let focus_xz: Option<(f32, f32)> = st
        .ui
        .focus_mode
        .clone()
        .or_else(|| st.ui.selected.clone())
        .and_then(|id| st.spatial.position_of(&id))
        .map(|p| (p.x, p.z));
    let standard = st.cfg.visual_theme == VisualTheme::Standard;

    // Camera frustum (4 ground-plane corners), camera position, look pivot.
    let (frustum, cam_xz, pivot_xz) = match cam_q.get_single() {
        Ok((cam, cam_tf, pan)) => {
            let corners = cam.logical_viewport_rect().map(|r| {
                [
                    r.min,
                    Vec2::new(r.max.x, r.min.y),
                    r.max,
                    Vec2::new(r.min.x, r.max.y),
                ]
                .map(|v| ground_hit(cam, cam_tf, v).map(|w| (w.x, w.z)))
            });
            let t = cam_tf.translation();
            let pivot = pan.map(|p| (p.target_focus.x, p.target_focus.z));
            (corners, Some((t.x, t.z)), pivot)
        }
        Err(_) => (None, None, None),
    };

    let ctx = contexts.ctx_mut();
    let content = if layout.content_rect.width() > 0.0 && layout.content_rect.height() > 0.0 {
        layout.content_rect
    } else {
        ctx.screen_rect()
    };
    let anchor = corner_anchor(
        content,
        egui::Align2::RIGHT_TOP,
        egui::vec2(MINIMAP_SIZE, MINIMAP_SIZE),
        egui::vec2(12.0, 12.0),
    );

    let mut fly_to: Option<Vec3> = None;
    egui::Area::new("minimap".into())
        .order(layer::PANEL)
        .fixed_pos(anchor)
        .show(ctx, |ui| {
            let (resp, painter) =
                ui.allocate_painter(egui::vec2(MINIMAP_SIZE, MINIMAP_SIZE), egui::Sense::click());
            let rect = resp.rect;
            // Opaque radar background.
            painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(6, 12, 20));
            painter.rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(1.0, egui_color(theme::GRID_LINE)),
            );

            let project = |x: f32, z: f32| -> egui::Pos2 {
                let (fx, fz) = minimap_project(x, z, min_x, min_z, span);
                egui::pos2(
                    rect.left() + fx * rect.width(),
                    rect.top() + fz * rect.height(),
                )
            };

            // Frustum quad (or a heading line fallback when a corner misses Y=0).
            let frustum_col = egui::Color32::from_rgba_unmultiplied(43, 179, 168, 150);
            if let Some(corners) = frustum {
                let pts: Vec<egui::Pos2> = corners
                    .into_iter()
                    .filter_map(|c| c.map(|(x, z)| project(x, z)))
                    .collect();
                if pts.len() == 4 {
                    for i in 0..4 {
                        painter.line_segment(
                            [pts[i], pts[(i + 1) % 4]],
                            egui::Stroke::new(1.2, frustum_col),
                        );
                    }
                } else if let (Some((cx, cz)), Some((px, pz))) = (cam_xz, pivot_xz) {
                    painter.line_segment(
                        [project(cx, cz), project(px, pz)],
                        egui::Stroke::new(1.2, frustum_col),
                    );
                }
            }

            // Type-coloured node dots (real projected positions).
            for (x, z, c) in &dots {
                painter.circle_filled(project(*x, *z), 1.4, *c);
            }

            // Camera position marker.
            if let Some((cx, cz)) = cam_xz {
                painter.circle_stroke(
                    project(cx, cz),
                    3.0,
                    egui::Stroke::new(1.4, egui::Color32::WHITE),
                );
            }

            // Focus / selection marker (distinct cyan ring + crosshair).
            if let Some((fx, fz)) = focus_xz {
                let p = project(fx, fz);
                let cyan = egui::Color32::from_rgb(52, 214, 200);
                painter.circle_stroke(p, 5.0, egui::Stroke::new(1.6, cyan));
                painter.line_segment(
                    [egui::pos2(p.x - 7.0, p.y), egui::pos2(p.x + 7.0, p.y)],
                    egui::Stroke::new(1.0, cyan),
                );
                painter.line_segment(
                    [egui::pos2(p.x, p.y - 7.0), egui::pos2(p.x, p.y + 7.0)],
                    egui::Stroke::new(1.0, cyan),
                );
            }

            // LIVE pill + corner brackets (Standard) and a world-span scale hint.
            if standard {
                painter.text(
                    egui::pos2(rect.left() + 6.0, rect.top() + 5.0),
                    egui::Align2::LEFT_TOP,
                    "● LIVE",
                    egui::FontId::monospace(9.0),
                    egui::Color32::from_rgb(111, 224, 111),
                );
                draw_brackets(&painter, rect, egui_color(theme::RETICLE_FOCUS));
            }
            painter.text(
                egui::pos2(rect.left() + 6.0, rect.bottom() - 5.0),
                egui::Align2::LEFT_BOTTOM,
                format!("⟷ {span:.0}"),
                egui::FontId::monospace(9.0),
                egui::Color32::from_rgb(136, 184, 178),
            );

            // Click-to-fly: map the click back to a world XZ target.
            if resp.clicked() {
                if let Some(p) = resp.interact_pointer_pos() {
                    let fx = ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                    let fz = ((p.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
                    let (wx, wz) = minimap_unproject(fx, fz, min_x, min_z, span);
                    fly_to = Some(Vec3::new(wx, 0.0, wz));
                }
            }
        });

    if let Some(target) = fly_to {
        st.request_jump_pos(target);
        st.needs_redraw
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Four corner brackets framing `rect` (the radar's GitS frame).
fn draw_brackets(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let a = 10.0;
    let s = egui::Stroke::new(1.2, color);
    for (c, sx, sy) in [
        (rect.left_top(), 1.0, 1.0),
        (rect.right_top(), -1.0, 1.0),
        (rect.left_bottom(), 1.0, -1.0),
        (rect.right_bottom(), -1.0_f32, -1.0_f32),
    ] {
        painter.line_segment([c, egui::pos2(c.x + sx * a, c.y)], s);
        painter.line_segment([c, egui::pos2(c.x, c.y + sy * a)], s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_are_square_padded_and_stable() {
        let pts = vec![(0.0, 0.0), (10.0, 4.0), (-2.0, 8.0)];
        let (min_x, min_z, span) = minimap_bounds(&pts).unwrap();
        for &(x, z) in &pts {
            let (fx, fz) = minimap_project(x, z, min_x, min_z, span);
            assert!((0.0..=1.0).contains(&fx) && (0.0..=1.0).contains(&fz));
        }
        assert!(
            span > 12.0,
            "span includes padding around the 12-wide extent"
        );
        assert!(minimap_bounds(&[]).is_none());
    }

    #[test]
    fn project_unproject_roundtrip() {
        let (min_x, min_z, span) = (-10.0, -10.0, 40.0);
        for &(x, z) in &[(0.0, 0.0), (5.0, -3.0), (-7.5, 9.0)] {
            let (fx, fz) = minimap_project(x, z, min_x, min_z, span);
            let (rx, rz) = minimap_unproject(fx, fz, min_x, min_z, span);
            assert!(
                (rx - x).abs() < 1e-3 && (rz - z).abs() < 1e-3,
                "roundtrip ({x},{z}) → ({rx},{rz})"
            );
        }
    }
}
