use crate::graph::{GraphState, ViewMode};
use crate::ui::{settings_agents, settings_paths, UiLayout};
use crate::util::config::{self, LodEdgesMode, ViewerConfig};
use bevy::prelude::ResMut;
use bevy_egui::{egui, EguiContexts};
use std::sync::atomic::Ordering;

pub fn ui_panel(
    mut contexts: EguiContexts,
    mut st: ResMut<GraphState>,
    mut layout: ResMut<UiLayout>,
) {
    let ctx = contexts.ctx_mut();
    let resp = egui::SidePanel::left("panel").show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("SpaceGraph");
            ui.vertical(|ui| {
                section_header(ui, "Status");
                ui.label(format!("nodes: {}", st.model.nodes.len()));
                ui.label(format!(
                    "edges: raw {} / agg {}",
                    st.model.edges.len(),
                    st.model.agg_edge_count()
                ));
            });

            ui.separator();
            ui.vertical(|ui| {
                section_header(ui, "Agents");
                let active = st.net.active_connection_count();
                if active == 0 {
                    ui.label("0 Agents connected");
                } else if active == 1 {
                    ui.label("1 Agent connected");
                } else {
                    ui.label(format!("{active} Agents connected"));
                }
                if ui.button("Manage Agents…").clicked() {
                    st.ui.show_agent_manager = true;
                }
            });

            ui.separator();
            ui.vertical(|ui| {
                section_header(ui, "View");
                ui.horizontal(|ui| {
                    ui.label("Mode:");
                    let mut changed = false;
                    changed |= ui
                        .selectable_value(&mut st.ui.view_mode, ViewMode::Spatial, "Spatial")
                        .clicked();
                    changed |= ui
                        .selectable_value(&mut st.ui.view_mode, ViewMode::Tree, "Tree")
                        .clicked();
                    changed |= ui
                        .selectable_value(&mut st.ui.view_mode, ViewMode::Timeline, "Timeline")
                        .clicked();
                    if changed {
                        st.spatial.dirty_layout = true;
                        st.needs_redraw.store(true, Ordering::Relaxed);
                    }
                });
                if st.ui.view_mode == ViewMode::Tree && ui.button("Fit to view").clicked() {
                    st.ui.fit_to_view = true;
                }
                if st.ui.view_mode == ViewMode::Tree {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Tree").strong());
                    let mut show_files = st.ui.tree_show_files;
                    if ui.checkbox(&mut show_files, "Show files").changed() {
                        st.ui.tree_show_files = show_files;
                        st.spatial.dirty_layout = true;
                        st.needs_redraw.store(true, Ordering::Relaxed);
                    }
                    ui.label(format!(
                        "Files auto-show when zoom ≥ {:.3}",
                        st.ui.tree_file_zoom_threshold
                    ));
                }
                let demo_allowed = st.net.active_connection_count() == 0
                    && (st.model.nodes.is_empty() || st.demo_loaded);
                let mut demo_mode = st.cfg.demo_mode;
                if ui
                    .add_enabled(
                        demo_allowed || demo_mode,
                        egui::Checkbox::new(&mut demo_mode, "Demo Mode"),
                    )
                    .changed()
                {
                    st.set_demo_mode(demo_mode);
                }
                if !demo_allowed && !demo_mode {
                    ui.label("Demo mode requires no active agents and an empty graph.");
                }

                if st.ui.view_mode == ViewMode::Timeline {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Timeline / Feynman").strong());
                    let mut paused = st.timeline.pause;
                    ui.checkbox(&mut paused, "Pause");
                    if paused != st.timeline.pause {
                        st.set_timeline_pause(paused);
                    }

                    let mut w = st.timeline.window.as_secs() as i32;
                    ui.horizontal(|ui| {
                        ui.label("Window (s)");
                        ui.add(egui::Slider::new(&mut w, 5..=240));
                    });
                    st.timeline.window = std::time::Duration::from_secs(w as u64);

                    ui.horizontal(|ui| {
                        ui.label("X scale");
                        ui.add(egui::Slider::new(&mut st.timeline.scale, 0.05..=1.5));
                    });
                    if paused {
                        let window_secs = st.timeline.window.as_secs_f32().max(0.1);
                        ui.horizontal(|ui| {
                            ui.label("Scrub (s)");
                            ui.add(egui::Slider::new(
                                &mut st.timeline.scrub_seconds,
                                0.0..=window_secs,
                            ));
                        });
                        if ui.button("Reset scrub").clicked() {
                            st.timeline.scrub_seconds = 0.0;
                        }
                        st.timeline.scrub_seconds =
                            st.timeline.scrub_seconds.clamp(0.0, window_secs);
                    }
                    ui.label(format!("events buffered: {}", st.timeline.events.len()));
                    ui.label("Worldlines: drawn for visible-set (capped).");
                    ui.label("Hover an event vertex/edge → tooltip.");

                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Selection").strong());
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

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.checkbox(&mut st.ui.show_3d, "3D");
                    ui.checkbox(&mut st.ui.show_edges, "Edges");
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut st.cfg.show_agg_edges, "Agg edges");
                    ui.checkbox(&mut st.cfg.show_raw_edges, "Raw edges");
                });
            });

            ui.separator();
            ui.vertical(|ui| {
                section_header(ui, "Filtering");
                ui.label("Filter (substring):");
                ui.text_edit_singleline(&mut st.ui.filter);

                ui.add_space(6.0);
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
            });

            ui.separator();
            ui.vertical(|ui| {
                section_header(ui, "Performance");
                ui.add(
                    egui::Slider::new(&mut st.cfg.max_visible_nodes, 200..=10_000)
                        .text("max visible nodes"),
                );
                ui.add(
                    egui::Slider::new(&mut st.cfg.progressive_nodes_per_frame, 50..=4000)
                        .text("progressive/frame"),
                );
            });

            ui.separator();
            ui.vertical(|ui| {
                section_header(ui, "LOD / Rendering");
                ui.checkbox(&mut st.cfg.lod_enabled, "Enable LOD");
                ui.add(
                    egui::Slider::new(&mut st.cfg.lod_threshold_nodes, 500..=20_000)
                        .text("LOD threshold"),
                );
                egui::ComboBox::from_label("LOD edges")
                    .selected_text(match st.cfg.lod_edges_mode {
                        LodEdgesMode::Off => "Off",
                        LodEdgesMode::FocusOnly => "Focus only",
                        LodEdgesMode::All => "All",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut st.cfg.lod_edges_mode, LodEdgesMode::Off, "Off");
                        ui.selectable_value(
                            &mut st.cfg.lod_edges_mode,
                            LodEdgesMode::FocusOnly,
                            "Focus only",
                        );
                        ui.selectable_value(&mut st.cfg.lod_edges_mode, LodEdgesMode::All, "All");
                    });
            });

            ui.separator();
            ui.vertical(|ui| {
                section_header(ui, "Layout (Spatial)");
                ui.checkbox(&mut st.cfg.layout_force, "Force layout");
                ui.add(egui::Slider::new(&mut st.cfg.link_distance, 1.0..=20.0).text("link dist"));
                ui.add(egui::Slider::new(&mut st.cfg.repulsion, 0.0..=120.0).text("repulsion"));
                ui.add(egui::Slider::new(&mut st.cfg.damping, 0.80..=0.999).text("damping"));
                ui.add(egui::Slider::new(&mut st.cfg.max_step, 0.05..=2.0).text("max step"));
            });

            ui.separator();
            ui.vertical(|ui| {
                section_header(ui, "Glow");
                let mut ms = st.cfg.glow_duration.as_millis() as i32;
                ui.add(egui::Slider::new(&mut ms, 100..=3000).text("glow ms"));
                st.cfg.glow_duration = std::time::Duration::from_millis(ms as u64);
            });

            ui.separator();
            ui.vertical(|ui| {
                section_header(ui, "GC");
                ui.checkbox(&mut st.cfg.gc_enabled, "enabled");
                let mut ttl = st.cfg.gc_ttl.as_secs() as i32;
                ui.add(egui::Slider::new(&mut ttl, 1..=600).text("orphan TTL (s)"));
                st.cfg.gc_ttl = std::time::Duration::from_secs(ttl as u64);
            });

            ui.separator();
            ui.vertical(|ui| {
                section_header(ui, "Search");
                ui.label("Ctrl+P opens search overlay. ? toggles help.");
                if ui.button("Open Search (Ctrl+P)").clicked() {
                    st.ui.search_open = true;
                }
            });

            ui.separator();
            ui.vertical(|ui| {
                section_header(ui, "Settings");
                if ui.button("Edit Paths…").clicked() {
                    st.open_path_editor();
                }
                if ui.button("Save Settings").clicked() {
                    let cfg = st.viewer_config();
                    if let Err(err) = config::save(&cfg) {
                        eprintln!("failed to save settings: {err}");
                    }
                }
                if ui.button("Reset Defaults").clicked() {
                    let defaults = ViewerConfig::default();
                    st.apply_viewer_config(&defaults);
                }
            });

            ui.separator();
            ui.vertical(|ui| {
                section_header(ui, "Actions");
                if ui.button("Clear graph").clicked() {
                    st.clear();
                }
            });
        });
    });

    let panel_rect = resp.response.rect;
    let screen = ctx.screen_rect();
    let content_rect = egui::Rect::from_min_max(
        egui::pos2(panel_rect.max.x, screen.min.y),
        egui::pos2(screen.max.x, screen.max.y),
    );
    layout.panel_rect = panel_rect;
    layout.content_rect = content_rect;

    settings_paths::path_editor_window(ctx, st.as_mut(), &layout);
    settings_agents::agent_manager_window(ctx, st.as_mut(), &layout);
    settings_agents::agent_editor_window(ctx, st.as_mut(), &layout);
    settings_agents::agent_command_window(ctx, st.as_mut(), &layout);
    super::search::search_overlay(contexts, st);
}

fn section_header(ui: &mut egui::Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(title).strong());
}
