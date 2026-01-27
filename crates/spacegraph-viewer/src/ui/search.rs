use bevy::prelude::ResMut;
use bevy_egui::{egui, EguiContexts};

use crate::graph::GraphState;

// Ctrl+P search overlay
pub fn search_overlay(mut contexts: EguiContexts, mut st: ResMut<GraphState>) {
    let ctx = contexts.ctx_mut();

    if !st.ui.search_open {
        return;
    }

    egui::Window::new("Search / Jump (Ctrl+P)")
        .collapsible(false)
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Query:");
                let resp = ui.text_edit_singleline(&mut st.ui.search_query);
                if resp.changed() {
                    st.recompute_search_hits(30);
                }
                if ui.button("Close (Esc)").clicked() {
                    st.ui.search_open = false;
                }
            });

            ui.separator();
            ui.label("Hits:");
            ui.add_space(4.0);

            let mut picked: Option<spacegraph_core::NodeId> = None;
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    for id in st.ui.search_hits.iter() {
                        let label = if let Some(node) = st.model.nodes.get(id) {
                            match node {
                                spacegraph_core::Node::File { path, .. } => {
                                    format!("file: {} ({})", path, id.0)
                                }
                                spacegraph_core::Node::Process { cmdline, pid, .. } => {
                                    format!("proc: pid={pid} {} ({})", cmdline, id.0)
                                }
                                spacegraph_core::Node::User { name, uid } => {
                                    format!("user: {name} uid={uid} ({})", id.0)
                                }
                            }
                        } else {
                            id.0.clone()
                        };
                        if ui.selectable_label(false, label).clicked() {
                            picked = Some(id.clone());
                        }
                    }
                });

            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Some(first) = st.ui.search_hits.first() {
                    picked = Some(first.clone());
                }
            }

            if let Some(id) = picked {
                st.request_jump(id.clone());
                st.ui.selected = Some(id);
                st.ui.search_open = false;
            }
        });
}
