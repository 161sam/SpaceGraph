use bevy::prelude::Res;
use bevy_egui::{egui, EguiContexts};
use std::time::Instant;

use crate::graph::{GraphState, ViewMode};
use crate::ui::{UiLayout, HUD_EDGE_PADDING, HUD_FALLBACK_Y_OFFSET, HUD_MIN_CONTENT_W};

pub fn hud_overlay(mut contexts: EguiContexts, st: Res<GraphState>, layout: Res<UiLayout>) {
    let ctx = contexts.ctx_mut();
    let screen = ctx.screen_rect();
    let content_rect = if layout.content_rect.width() > 0.0 && layout.content_rect.height() > 0.0 {
        layout.content_rect
    } else {
        screen
    };
    let mut x = content_rect.min.x + HUD_EDGE_PADDING;
    let mut y = content_rect.min.y + HUD_EDGE_PADDING;
    if content_rect.width() < HUD_MIN_CONTENT_W {
        x = screen.min.x + HUD_EDGE_PADDING;
        y = screen.min.y + HUD_EDGE_PADDING + HUD_FALLBACK_Y_OFFSET;
    }

    egui::Area::new("hud".into())
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(x, y))
        .show(ctx, |ui| {
            ui.group(|ui| {
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
                    if st.ui.view_mode == ViewMode::Spatial {
                        "Spatial"
                    } else {
                        "Timeline"
                    }
                ));
                if st.snapshot_loaded
                    && !st.live_events_seen
                    && !st.model.nodes.is_empty()
                    && !st.cfg.demo_mode
                {
                    ui.label("Initial snapshot (no live events yet)");
                }
            });
        });
}
