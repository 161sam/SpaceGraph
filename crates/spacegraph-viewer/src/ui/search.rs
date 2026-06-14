use bevy::prelude::ResMut;
use bevy_egui::{egui, EguiContexts};
use std::time::{Duration, Instant};

use crate::graph::state::SearchSource;
use crate::graph::GraphState;

/// Per-section result cap shown in the overlay.
const SECTION_HITS: usize = 30;
/// Debounce before an `ON DISK` agent query is sent (spec §4, ~120 ms).
const FS_DEBOUNCE: Duration = Duration::from_millis(120);
/// Agent result cap requested per query.
const FS_LIMIT: u32 = 200;
/// Whether to opt into the full-system scope (D-2). Off by default.
const FS_FULL_SYSTEM: bool = false;

// Ctrl+P search overlay — merged `IN GRAPH` (instant) + `ON DISK` (async agent
// index) results. Picking an `ON DISK` hit materialises it and flies to it.
pub fn search_overlay(mut contexts: EguiContexts, mut st: ResMut<GraphState>) {
    let ctx = contexts.ctx_mut();

    if !st.ui.search_open {
        return;
    }

    let now = Instant::now();
    // Fire the debounced agent query if its window has elapsed.
    st.maybe_issue_fs_query(now, FS_DEBOUNCE, FS_LIMIT, FS_FULL_SYSTEM);

    let fs_available = st.fs_search_available();
    let truncated = st.fs.truncated;

    egui::Window::new("Search / Jump (Ctrl+P)")
        .collapsible(false)
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Query:");
                let resp = ui.text_edit_singleline(&mut st.ui.search_query);
                if resp.changed() {
                    st.note_search_query_changed(now, SECTION_HITS);
                }
                if ui.button("Close (Esc)").clicked() {
                    st.ui.search_open = false;
                }
            });

            if fs_available {
                ui.label(
                    egui::RichText::new("IN GRAPH (loaded) · ON DISK (filesystem index)").weak(),
                );
            } else {
                ui.label(
                    egui::RichText::new("IN GRAPH only — agent filesystem search unavailable")
                        .weak(),
                );
            }
            ui.separator();

            let rows = st.merged_search_results(SECTION_HITS);
            let mut pick_graph: Option<spacegraph_core::NodeId> = None;
            let mut pick_disk: Option<usize> = None;

            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for row in &rows {
                        let (tag, color) = match row.source {
                            SearchSource::InGraph(_) => {
                                ("IN GRAPH", egui::Color32::from_rgb(120, 220, 160))
                            }
                            SearchSource::OnDisk(_) => {
                                ("ON DISK ", egui::Color32::from_rgb(120, 180, 240))
                            }
                        };
                        let resp = ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(tag).monospace().color(color));
                            ui.selectable_label(false, &row.label).clicked()
                        });
                        if resp.inner {
                            match &row.source {
                                SearchSource::InGraph(id) => pick_graph = Some(id.clone()),
                                SearchSource::OnDisk(i) => pick_disk = Some(*i),
                            }
                        }
                    }
                    if rows.is_empty() {
                        ui.weak("no matches");
                    }
                });

            if truncated {
                ui.weak("(disk results capped — refine the query)");
            }

            // Enter picks the first row (graph hits come first).
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Some(first) = rows.first() {
                    match &first.source {
                        SearchSource::InGraph(id) => pick_graph = Some(id.clone()),
                        SearchSource::OnDisk(i) => pick_disk = Some(*i),
                    }
                }
            }

            if let Some(id) = pick_graph {
                st.request_jump(id.clone());
                st.ui.selected = Some(id);
                st.ui.search_open = false;
            } else if let Some(i) = pick_disk {
                // Materialise the picked path; the camera flies to it once the
                // agent streams the node in (see apply_delta).
                st.pick_fs_result(i);
                st.ui.search_open = false;
            }
        });
}
