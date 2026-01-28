use bevy::prelude::Res;
use bevy_egui::{egui, EguiContexts};

use crate::graph::{GraphState, ViewMode};
use crate::ui::{
    HUD_EDGE_PADDING, HUD_FALLBACK_Y_OFFSET, HUD_MIN_CONTENT_W, HUD_PANEL_GAP, PANEL_W,
};

pub fn hud_overlay(mut contexts: EguiContexts, st: Res<GraphState>) {
    let ctx = contexts.ctx_mut();
    let screen = ctx.screen_rect();
    let mut x = screen.min.x + PANEL_W + HUD_PANEL_GAP;
    let mut y = screen.min.y + HUD_EDGE_PADDING;
    if screen.width() < PANEL_W + HUD_MIN_CONTENT_W {
        x = screen.min.x + HUD_EDGE_PADDING;
        y = screen.min.y + HUD_EDGE_PADDING + HUD_FALLBACK_Y_OFFSET;
    }

    egui::Area::new("hud".into())
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(x, y))
        .show(ctx, |ui| {
            ui.group(|ui| {
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
                    "Mode: {}",
                    if st.ui.view_mode == ViewMode::Spatial {
                        "Spatial"
                    } else {
                        "Timeline"
                    }
                ));
                if st.snapshot_loaded && !st.live_events_seen && !st.model.nodes.is_empty() {
                    ui.label("Initial snapshot (no live events yet)");
                }
            });
        });
}
