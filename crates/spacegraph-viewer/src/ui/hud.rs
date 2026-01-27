use bevy::prelude::Res;
use bevy_egui::{egui, EguiContexts};

use crate::graph::{GraphState, ViewMode};

pub fn hud_overlay(mut contexts: EguiContexts, st: Res<GraphState>) {
    egui::Area::new("hud".into())
        .fixed_pos(egui::pos2(10.0, 10.0))
        .show(contexts.ctx_mut(), |ui| {
            ui.group(|ui| {
                ui.label(format!("FPS: {:.0}", st.perf.fps));
                ui.label(format!(
                    "Visible: {} nodes / {} edges",
                    st.perf.visible_nodes, st.perf.visible_edges
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
            });
        });
}
