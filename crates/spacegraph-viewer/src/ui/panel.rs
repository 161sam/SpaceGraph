use bevy::prelude::ResMut;
use bevy_egui::{egui, EguiContexts};
use std::sync::atomic::Ordering;

use crate::graph::{GraphState, ViewMode};

pub fn ui_panel(mut contexts: EguiContexts, mut st: ResMut<GraphState>) {
    egui::SidePanel::left("left").show(contexts.ctx_mut(), |ui| {
        ui.heading("SpaceGraph");
        ui.label(format!("nodes: {}", st.model.nodes.len()));
        ui.label(format!(
            "edges: raw {} / agg {}",
            st.model.edges.len(),
            st.model.agg_edge_count()
        ));
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("View:");
            ui.selectable_value(&mut st.ui.view_mode, ViewMode::Spatial, "Spatial");
            ui.selectable_value(&mut st.ui.view_mode, ViewMode::Timeline, "Timeline");
        });

        if st.ui.view_mode == ViewMode::Timeline {
            ui.add_space(6.0);
            ui.heading("Timeline / Feynman");
            let mut paused = st.timeline.pause;
            ui.checkbox(&mut paused, "Pause");
            if paused != st.timeline.pause {
                st.set_timeline_pause(paused);
            }

            let mut w = st.timeline.window.as_secs() as i32;
            ui.add(egui::Slider::new(&mut w, 5..=240).text("window (s)"));
            st.timeline.window = std::time::Duration::from_secs(w as u64);

            ui.add(egui::Slider::new(&mut st.timeline.scale, 0.05..=1.5).text("x scale"));
            if paused {
                let window_secs = st.timeline.window.as_secs_f32().max(0.1);
                ui.add(
                    egui::Slider::new(&mut st.timeline.scrub_seconds, 0.0..=window_secs)
                        .text("scrub (s)"),
                );
                if ui.button("Reset scrub").clicked() {
                    st.timeline.scrub_seconds = 0.0;
                }
                st.timeline.scrub_seconds = st.timeline.scrub_seconds.clamp(0.0, window_secs);
            }
            ui.label(format!("events buffered: {}", st.timeline.events.len()));
            ui.label("Worldlines: drawn for visible-set (capped).");
            ui.label("Hover an event vertex/edge → tooltip.");

            ui.add_space(6.0);
            ui.heading("Selection");
            if let Some(id) = st.ui.selected_a.as_ref() {
                ui.label(format!("A: {}", st.node_label_with_id(id)));
            } else {
                ui.label("A: (none)");
            }
            if let Some(id) = st.ui.selected_b.as_ref() {
                ui.label(format!("B: {}", st.node_label_with_id(id)));
            } else {
                ui.label("B: (none)");
            }
            let jump_enabled = st.ui.selected_a.is_some();
            if ui
                .add_enabled(jump_enabled, egui::Button::new("Jump to Spatial"))
                .clicked()
            {
                if let Some(id) = st.ui.selected_a.clone() {
                    st.ui.view_mode = ViewMode::Spatial;
                    st.request_jump(id);
                }
            }
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.checkbox(&mut st.ui.show_3d, "3D");
            ui.checkbox(&mut st.ui.show_edges, "Edges");
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut st.cfg.show_agg_edges, "Agg edges");
            ui.checkbox(&mut st.cfg.show_raw_edges, "Raw edges");
        });

        ui.add_space(8.0);
        ui.label("Filter (substring):");
        ui.text_edit_singleline(&mut st.ui.filter);

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Focus hops:");
            ui.add(egui::Slider::new(&mut st.ui.focus_hops, 1..=10));
        });

        if let Some(f) = &st.ui.focus {
            ui.label(format!("Focus: {}", f.0));
            if ui.button("Clear focus").clicked() {
                st.ui.focus = None;
                st.needs_redraw.store(true, Ordering::Relaxed);
            }
        } else {
            ui.label("Focus: (none) — click a node");
        }

        ui.add_space(8.0);
        ui.separator();
        ui.heading("Performance");
        ui.add(
            egui::Slider::new(&mut st.cfg.max_visible_nodes, 200..=10_000)
                .text("max visible nodes"),
        );
        ui.add(
            egui::Slider::new(&mut st.cfg.progressive_nodes_per_frame, 50..=4000)
                .text("progressive/frame"),
        );

        ui.add_space(8.0);
        ui.separator();
        ui.heading("Layout (Spatial)");
        ui.checkbox(&mut st.cfg.layout_force, "Force layout");
        ui.add(egui::Slider::new(&mut st.cfg.link_distance, 1.0..=20.0).text("link dist"));
        ui.add(egui::Slider::new(&mut st.cfg.repulsion, 0.0..=120.0).text("repulsion"));
        ui.add(egui::Slider::new(&mut st.cfg.damping, 0.80..=0.999).text("damping"));
        ui.add(egui::Slider::new(&mut st.cfg.max_step, 0.05..=2.0).text("max step"));

        ui.add_space(8.0);
        ui.separator();
        ui.heading("Glow");
        let mut ms = st.cfg.glow_duration.as_millis() as i32;
        ui.add(egui::Slider::new(&mut ms, 100..=3000).text("glow ms"));
        st.cfg.glow_duration = std::time::Duration::from_millis(ms as u64);

        ui.add_space(8.0);
        ui.separator();
        ui.heading("GC");
        ui.checkbox(&mut st.cfg.gc_enabled, "enabled");
        let mut ttl = st.cfg.gc_ttl.as_secs() as i32;
        ui.add(egui::Slider::new(&mut ttl, 1..=600).text("orphan TTL (s)"));
        st.cfg.gc_ttl = std::time::Duration::from_secs(ttl as u64);

        ui.add_space(10.0);
        ui.separator();
        ui.heading("Search");
        ui.label("Ctrl+P opens search overlay.");
        if ui.button("Open Search (Ctrl+P)").clicked() {
            st.ui.search_open = true;
        }

        ui.add_space(10.0);
        ui.separator();
        if ui.button("Clear graph").clicked() {
            st.clear();
        }
    });

    super::search::search_overlay(contexts, st);
}
