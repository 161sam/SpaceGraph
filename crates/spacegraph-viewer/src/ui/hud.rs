use bevy::prelude::Res;
use bevy_egui::{egui, EguiContexts};
use std::time::Instant;

use crate::graph::{GraphState, ViewMode};
use crate::render::quality::QualityState;
use crate::ui::gits;
use crate::ui::tokens::color;
use crate::ui::{UiLayout, HUD_EDGE_PADDING};
use crate::util::config::VisualTheme;

/// HUD rand-frame (v0.5.0, spec §3.6): edge-hugging corner brackets carrying live
/// global state (agents, alert counts by severity, mode, FPS, active tier). Drawn
/// with the egui painter at the viewport margins so the centre stays the
/// visualisation. Panic-free headless (`try_ctx_mut`).
pub fn hud_frame_overlay(
    mut contexts: EguiContexts,
    st: Res<GraphState>,
    quality: Res<QualityState>,
) {
    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };
    draw_hud_frame(ctx, &st, quality.effective.as_str());
}

fn draw_hud_frame(ctx: &egui::Context, st: &GraphState, tier: &str) {
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("hud_frame"),
    ));
    let accent = egui::Color32::from_rgba_unmultiplied(
        color::ACCENT.r(),
        color::ACCENT.g(),
        color::ACCENT.b(),
        150,
    );
    let stroke = egui::Stroke::new(1.5, accent);
    let m = 6.0; // margin
    let l = 26.0; // bracket arm length
    let r = screen.shrink(m);
    // Four corner brackets (the GitS "rand-frame").
    for (corner, dx, dy) in [
        (r.left_top(), 1.0, 1.0),
        (r.right_top(), -1.0, 1.0),
        (r.left_bottom(), 1.0, -1.0),
        (r.right_bottom(), -1.0, -1.0),
    ] {
        painter.line_segment([corner, egui::pos2(corner.x + dx * l, corner.y)], stroke);
        painter.line_segment([corner, egui::pos2(corner.x, corner.y + dy * l)], stroke);
    }

    // Live global-state strip along the top edge.
    let (low, med, high) = st.alert_severity_counts();
    let mode = match st.ui.view_mode {
        ViewMode::Spatial => "SPATIAL",
        ViewMode::Tree => "TREE",
        ViewMode::Timeline => "TIMELINE",
    };
    let line = format!(
        "◈ AGENTS {}  ·  ALERTS {low}/{med}/{high}  ·  {mode}  ·  {:.0} FPS  ·  TIER {}",
        st.net.active_connection_count(),
        st.perf.fps,
        tier.to_uppercase(),
    );
    painter.text(
        egui::pos2(screen.center().x, r.top() + 10.0),
        egui::Align2::CENTER_CENTER,
        line,
        egui::FontId::monospace(12.0),
        color::TEXT,
    );
}

pub fn hud_overlay(mut contexts: EguiContexts, st: Res<GraphState>, layout: Res<UiLayout>) {
    let ctx = contexts.ctx_mut();
    let screen = ctx.screen_rect();
    let content_rect = if layout.content_rect.width() > 0.0 && layout.content_rect.height() > 0.0 {
        layout.content_rect
    } else {
        screen
    };
    // P2: anchor the debug telemetry to the content area's bottom-left, clear of
    // the command rail and the HUD panels (which open from the top-left).
    let left = (content_rect.min.x + HUD_EDGE_PADDING).max(screen.min.x + HUD_EDGE_PADDING);
    let standard = st.cfg.visual_theme == VisualTheme::Standard;

    egui::Area::new("hud".into())
        .order(crate::ui::overlay::layer::PANEL)
        .anchor(
            egui::Align2::LEFT_BOTTOM,
            egui::vec2(left, -HUD_EDGE_PADDING),
        )
        .show(ctx, |ui| {
            // P4: GitS readout frame (Standard) / plain (Minimal), consistent with
            // the HUD panels.
            gits::panel_frame(standard).show(ui, |ui| {
                ui.label(
                    egui::RichText::new("◢ TELEMETRY")
                        .monospace()
                        .small()
                        .color(if standard {
                            color::ACCENT
                        } else {
                            color::TEXT_DIM
                        }),
                );
                let now = Instant::now();
                let mut snapshot_seen = false;
                let mut live_seen = false;
                let mut last_activity: Option<Instant> = None;
                for stream in st.net.streams.values() {
                    if let Some(ts) = stream.last_snapshot_at {
                        snapshot_seen = true;
                        if last_activity.is_none_or(|last| ts > last) {
                            last_activity = Some(ts);
                        }
                    }
                    if let Some(ts) = stream.last_event_at {
                        live_seen = true;
                        if last_activity.is_none_or(|last| ts > last) {
                            last_activity = Some(ts);
                        }
                    }
                }
                let last_label = last_activity
                    .map(|ts| format!("{:.1}s ago", now.duration_since(ts).as_secs_f32()))
                    .unwrap_or_else(|| "—".to_string());
                ui.label(format!("FPS: {:.0}", st.perf.fps));
                ui.label(format!(
                    "Visible: {} nodes / {} edges",
                    st.perf.visible_nodes, st.perf.visible_edges
                ));
                ui.label(format!(
                    "Edges (raw/agg): {} / {}",
                    st.perf.visible_raw_edges, st.perf.visible_agg_edges
                ));
                ui.label(format!("Event rate: {:.1}/s", st.perf.event_rate));
                ui.label(format!("Total msgs: {}", st.perf.event_total));
                if let Some(id) = st.spatial.last_batch_id {
                    ui.label(format!("Last batch: {}", id));
                }
                ui.label(format!(
                    "Data flow: snapshot: {} | live: {} | last: {}",
                    if snapshot_seen { "yes" } else { "no" },
                    if live_seen { "yes" } else { "no" },
                    last_label
                ));
                ui.label(format!(
                    "Mode: {}",
                    match st.ui.view_mode {
                        ViewMode::Spatial => "Spatial",
                        ViewMode::Tree => "Tree",
                        ViewMode::Timeline => "Timeline",
                    }
                ));
                if st.snapshot_loaded
                    && !st.live_events_seen
                    && !st.core.model.nodes.is_empty()
                    && !st.cfg.demo_mode
                {
                    ui.label("Initial snapshot (no live events yet)");
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hud_frame_draws_without_panic() {
        // Real standalone egui context (no bevy_egui) — exercises the painter.
        let st = GraphState::default();
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            draw_hud_frame(ctx, &st, "medium");
        });
    }
}
