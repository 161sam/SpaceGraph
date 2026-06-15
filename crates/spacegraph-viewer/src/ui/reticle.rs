//! Lock-on targeting reticle + in-world readout (Standard theme).
//!
//! Projects the hovered / focused / selected node positions to screen and draws
//! animated corner brackets framing them; the selection also gets a leader-lined
//! monospace readout box. Distance-faded micro-tags label the nearest nodes
//! (capped). Standard-only — `render::spatial::highlight_style` suppresses the
//! gizmo bubbles here and keeps them under Minimal. The bracket sweep animates
//! off the egui clock and never feeds back into graph state (visual-only).

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use spacegraph_core::NodeId;
use std::time::Instant;

use crate::graph::{namespace, GraphState, ViewMode};
use crate::render::spatial::{highlight_style, HighlightStyle};
use crate::render::theme;
use crate::ui::egui_color;
use crate::ui::overlay;
use crate::util::ids::node_label_long;

/// World radius around the camera within which micro-tags may appear.
const MICRO_TAG_RADIUS: f32 = 45.0;

pub fn reticle_overlay(
    mut contexts: EguiContexts,
    st: Res<GraphState>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
) {
    if st.ui.view_mode != ViewMode::Spatial
        || highlight_style(st.cfg.visual_theme) != HighlightStyle::Reticle
    {
        return;
    }
    let Ok((camera, cam_tf)) = cam_q.get_single() else {
        return;
    };

    let ctx = contexts.ctx_mut();
    let t = ctx.input(|i| i.time) as f32;
    let pulse = 0.5 + 0.5 * (t * 3.0).sin();
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("lockon_reticle"),
    ));
    let project = |pos: Vec3| {
        camera
            .world_to_viewport(cam_tf, pos)
            .map(|s| egui::pos2(s.x, s.y))
    };

    // Corner brackets for hovered / focus / selected.
    for (maybe, color, base) in [
        (&st.ui.hovered, theme::RETICLE_HOVER, 20.0_f32),
        (&st.ui.focus, theme::RETICLE_FOCUS, 30.0),
        (&st.ui.selected, theme::RETICLE_SELECT, 25.0),
    ] {
        let Some(id) = maybe else {
            continue;
        };
        let Some(pos) = st.spatial.position_of(id) else {
            continue;
        };
        let Some(c) = project(pos) else {
            continue;
        };
        draw_brackets(&painter, c, base + 4.0 * pulse, egui_color(color));
    }

    // Leader-lined readout for the selection — placed *beside* the node (P1
    // `place_card`, edge-aware), suppressed in Focus Mode (the radial owns the
    // node region) and when the selection is also the hovered node (the hover
    // readout covers it). This breaks the concentric readout pile-up.
    if let Some(id) = &st.ui.selected {
        let suppress = st.ui.focus_mode.is_some() || st.ui.hovered.as_ref() == Some(id);
        if !suppress {
            if let Some(pos) = st.spatial.position_of(id) {
                if let Some(c) = project(pos) {
                    draw_readout(&painter, &readout_lines(&st, id), c, screen);
                }
            }
        }
    }

    // Distance-faded micro-tags on the nearest nodes (capped).
    if st.cfg.micro_tags {
        // When the radial HUD is open (Focus Mode), keep its action ring clear:
        // drop any micro-tag whose anchor lands inside the ring footprint around the
        // focused subject (those neighbours are named in the entity card, not floated
        // across the ring). `focus_mode` is set only when the radial is shown.
        let ring_guard = st
            .ui
            .focus_mode
            .as_ref()
            .and_then(|fid| st.spatial.position_of(fid))
            .and_then(&project);
        let cam = cam_tf.translation();
        let nodes: Vec<(NodeId, Vec3)> = st
            .spatial
            .placed_positions()
            .filter(|(id, _)| st.spatial.vis_cache.contains(*id) && st.is_visible_rendered(id))
            .map(|(id, p)| (id.clone(), p))
            .collect();
        for (id, alpha) in nearest_micro_tags(&nodes, cam, MICRO_TAG_RADIUS, st.cfg.micro_tag_max) {
            if Some(&id) == st.ui.selected.as_ref() || Some(&id) == st.ui.hovered.as_ref() {
                continue; // already reticled
            }
            let Some(pos) = st.spatial.position_of(&id) else {
                continue;
            };
            let Some(c) = project(pos) else {
                continue;
            };
            if let Some(rc) = ring_guard {
                if rc.distance(c) <= crate::ui::context_menu::RING_OUTER_R + 12.0 {
                    continue; // inside the radial ring footprint — keep it unobstructed
                }
            }
            let a = (alpha * 180.0) as u8;
            painter.text(
                egui::pos2(c.x + 8.0, c.y),
                egui::Align2::LEFT_CENTER,
                micro_label(&id),
                egui::FontId::monospace(10.0),
                egui::Color32::from_rgba_unmultiplied(180, 210, 240, a),
            );
        }
    }
}

fn draw_brackets(painter: &egui::Painter, c: egui::Pos2, h: f32, color: egui::Color32) {
    let arm = h * 0.4;
    let stroke = egui::Stroke::new(1.6, color);
    for (sx, sy) in [(-1.0_f32, -1.0_f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let corner = egui::pos2(c.x + sx * h, c.y + sy * h);
        painter.line_segment([corner, egui::pos2(corner.x - sx * arm, corner.y)], stroke);
        painter.line_segment([corner, egui::pos2(corner.x, corner.y - sy * arm)], stroke);
    }
}

fn draw_readout(painter: &egui::Painter, lines: &[String], anchor: egui::Pos2, vp: egui::Rect) {
    if lines.is_empty() {
        return;
    }
    let line_h = 15.0;
    let size = egui::vec2(240.0, line_h * lines.len() as f32 + 8.0);
    let box_min = overlay::place_card(anchor, overlay::NODE_HALF_PX, size, vp, overlay::CARD_GAP);
    let rect = egui::Rect::from_min_size(box_min, size);
    // Leader line from the node to the box's nearest vertical edge.
    let leader_target = if rect.min.x >= anchor.x {
        egui::pos2(rect.min.x, rect.center().y)
    } else {
        egui::pos2(rect.max.x, rect.center().y)
    };
    painter.line_segment(
        [anchor, leader_target],
        egui::Stroke::new(1.0, egui_color(theme::RETICLE_SELECT)),
    );
    painter.rect_filled(
        rect,
        3.0,
        egui::Color32::from_rgba_unmultiplied(6, 14, 24, 200),
    );
    painter.rect_stroke(
        rect,
        3.0,
        egui::Stroke::new(1.0, egui_color(theme::RETICLE_SELECT)),
    );
    let mut y = box_min.y + 4.0;
    for line in lines {
        painter.text(
            egui::pos2(box_min.x + 6.0, y),
            egui::Align2::LEFT_TOP,
            line,
            egui::FontId::monospace(12.0),
            egui::Color32::from_rgb(200, 230, 255),
        );
        y += line_h;
    }
}

fn readout_lines(st: &GraphState, id: &NodeId) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(n) = st.core.model.nodes.get(id) {
        out.extend(node_label_long(n));
    }
    if st.spatial.node_glow(id).is_some_and(|d| d > Instant::now()) {
        out.push("◉ recent activity".to_string());
    }
    out
}

/// A compact label for a micro-tag: the last segment of the local id.
fn micro_label(id: &NodeId) -> String {
    let local = namespace::local_part(id);
    let tail = local.rsplit(['/', ':']).next().unwrap_or(local);
    tail.chars().take(16).collect()
}

/// The nearest nodes to the camera within `radius`, capped to `max`, each with a
/// distance-based alpha in `[0,1]` (1 = closest). Pure + bounded for testing.
pub fn nearest_micro_tags(
    nodes: &[(NodeId, Vec3)],
    cam: Vec3,
    radius: f32,
    max: usize,
) -> Vec<(NodeId, f32)> {
    let r2 = radius * radius;
    let mut within: Vec<(f32, &NodeId)> = nodes
        .iter()
        .filter_map(|(id, p)| {
            let d2 = p.distance_squared(cam);
            (d2 <= r2).then_some((d2, id))
        })
        .collect();
    within.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    within.truncate(max);
    within
        .into_iter()
        .map(|(d2, id)| {
            let alpha = (1.0 - d2.sqrt() / radius).clamp(0.0, 1.0);
            (id.clone(), alpha)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::config::VisualTheme;

    #[test]
    fn highlight_style_maps_theme() {
        assert_eq!(
            highlight_style(VisualTheme::Standard),
            HighlightStyle::Reticle
        );
        assert_eq!(
            highlight_style(VisualTheme::Minimal),
            HighlightStyle::Bubbles
        );
    }

    #[test]
    fn micro_tag_cap_and_radius_respected() {
        let cam = Vec3::ZERO;
        // 50 nodes inside the radius, 5 well outside.
        let mut nodes: Vec<(NodeId, Vec3)> = (0..50)
            .map(|i| {
                (
                    NodeId(format!("in{i}")),
                    Vec3::new(i as f32 * 0.1, 0.0, 0.0),
                )
            })
            .collect();
        for i in 0..5 {
            nodes.push((
                NodeId(format!("out{i}")),
                Vec3::new(100.0 + i as f32, 0.0, 0.0),
            ));
        }
        let tags = nearest_micro_tags(&nodes, cam, 45.0, 24);
        assert_eq!(tags.len(), 24, "capped to max");
        assert!(
            tags.iter().all(|(id, _)| id.0.starts_with("in")),
            "only in-radius nodes"
        );
        // Closest first → first alpha is the largest.
        assert!(tags[0].1 >= tags[23].1);
    }

    #[test]
    fn reticle_overlay_runs_without_panic_headless() {
        // `EguiContexts` needs `EguiUserTextures` just to fetch its params; with
        // no camera entity the system early-returns before any egui drawing.
        let mut app = App::new();
        app.init_resource::<bevy_egui::EguiUserTextures>()
            .insert_resource(GraphState::default())
            .add_systems(Update, reticle_overlay);
        app.update();
    }
}
