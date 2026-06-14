//! Corner-anchored GitS HUD panels (MP-UI-GitS, P2) — the controls that used to
//! live in the permanent left dev sidebar, now grouped by rail section and shown
//! in a single floating, GitS-framed panel anchored beside the command rail. Also
//! hosts the modal windows the old `ui_panel` used to dispatch (path editor,
//! agent windows, node search).
//!
//! Every control from the old sidebar is preserved here (see the reachability
//! checklist in `docs/RUNLOG-ui-gits.md`); only the chrome changed.

use bevy::prelude::{Res, ResMut};
use bevy_egui::{egui, EguiContexts};
use std::sync::atomic::Ordering;

use crate::graph::{GraphState, ViewMode};
use crate::render::quality::QualityState;
use crate::render::Mission;
use crate::ui::overlay::layer;
use crate::ui::rail::{RailSection, RailState, RAIL_WIDTH, TOP_OFFSET};
use crate::ui::tokens::color;
use crate::ui::{gits, settings_agents, settings_paths, UiLayout};
use crate::util::config::{self, LodEdgesMode, ViewerConfig, VisualTheme};

/// Render the active rail section's panel (none open → nothing drawn).
pub fn hud_panels(
    mut contexts: EguiContexts,
    rail: Res<RailState>,
    mut st: ResMut<GraphState>,
    mut quality: ResMut<QualityState>,
    mission: Res<Mission>,
) {
    let Some(section) = rail.open else {
        return;
    };
    let standard = st.cfg.visual_theme == VisualTheme::Standard;
    let title = match section {
        RailSection::View => "VIEW / DISPLAY",
        RailSection::Filter => "FILTER",
        RailSection::Alerts => "ALERTS / INCIDENT",
        RailSection::Agents => "AGENTS",
        RailSection::Settings => "SETTINGS",
    };
    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };
    let max_h = (ctx.screen_rect().height() - TOP_OFFSET - 24.0).max(220.0);
    let resp = egui::Window::new(title)
        .order(layer::PANEL)
        .anchor(egui::Align2::LEFT_TOP, [RAIL_WIDTH + 14.0, TOP_OFFSET])
        .frame(gits::panel_frame(standard))
        .resizable(false)
        .collapsible(false)
        .title_bar(false)
        .show(ctx, |ui| {
            ui.set_min_width(300.0);
            ui.set_max_width(340.0);
            ui.label(
                egui::RichText::new(format!("◢ {title}"))
                    .monospace()
                    .strong()
                    .color(if standard { color::ACCENT } else { color::TEXT }),
            );
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(max_h)
                .auto_shrink([false, true])
                .show(ui, |ui| match section {
                    RailSection::View => section_view(ui, &mut st, &mut quality, standard),
                    RailSection::Filter => section_filter(ui, &mut st, standard),
                    RailSection::Alerts => section_alerts(ui, &mut st, &mission, standard),
                    RailSection::Agents => section_agents(ui, &mut st, standard),
                    RailSection::Settings => section_settings(ui, &mut st, standard),
                });
        });
    if let Some(resp) = resp {
        gits::bracket_response(ctx, resp.response.rect, standard);
    }
}

/// Host the modal windows the old sidebar used to dispatch (still self-gated).
pub fn dispatch_windows(
    mut contexts: EguiContexts,
    mut st: ResMut<GraphState>,
    layout: Res<UiLayout>,
) {
    {
        let Some(ctx) = contexts.try_ctx_mut() else {
            return;
        };
        settings_paths::path_editor_window(ctx, st.as_mut(), &layout);
        settings_agents::agent_manager_window(ctx, st.as_mut(), &layout);
        settings_agents::agent_editor_window(ctx, st.as_mut(), &layout);
        settings_agents::agent_command_window(ctx, st.as_mut(), &layout);
    }
    crate::ui::search::search_overlay(contexts, st);
}

// ===========================================================================
// Sections — the widget bodies relocated verbatim from the old `ui_panel`.
// ===========================================================================

fn section_view(
    ui: &mut egui::Ui,
    st: &mut GraphState,
    quality: &mut QualityState,
    standard: bool,
) {
    gits::section_header(ui, "Status", standard);
    ui.label(format!("nodes: {}", st.core.model.nodes.len()));
    ui.label(format!(
        "edges: raw {} / agg {}",
        st.core.model.edges.len(),
        st.core.model.agg_edge_count()
    ));

    gits::section_header(ui, "View", standard);
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
    let demo_allowed =
        st.net.active_connection_count() == 0 && (st.core.model.nodes.is_empty() || st.demo_loaded);
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
        let mut show_connectors = st.timeline.show_connectors;
        if ui
            .checkbox(&mut show_connectors, "Show connectors")
            .changed()
        {
            st.timeline.show_connectors = show_connectors;
            st.needs_redraw.store(true, Ordering::Relaxed);
        }
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
            st.timeline.scrub_seconds = st.timeline.scrub_seconds.clamp(0.0, window_secs);
        }
        ui.label(format!("events buffered: {}", st.timeline.events.len()));
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

    gits::section_header(ui, "Display", standard);
    theme_tier_selectors(ui, st, quality);
    if ui
        .button(if st.ui.legend_open {
            "Hide legend (L)"
        } else {
            "Show legend (L)"
        })
        .clicked()
    {
        st.ui.legend_open = !st.ui.legend_open;
        st.needs_redraw.store(true, Ordering::Relaxed);
    }
}

fn section_filter(ui: &mut egui::Ui, st: &mut GraphState, standard: bool) {
    gits::section_header(ui, "Filter (query-DSL)", standard);
    ui.text_edit_singleline(&mut st.ui.filter);
    ui.label(
        egui::RichText::new("e.g. type:process deg:>3 -name:bash recent:5m")
            .weak()
            .small(),
    );
    match crate::graph::query::parse_query(&st.ui.filter) {
        Err(e) => {
            ui.colored_label(color::SEV_HIGH, format!("⚠ {}", e.message));
        }
        Ok(_) => {
            let toks: Vec<String> = st.ui.filter.split_whitespace().map(String::from).collect();
            let mut remove: Option<usize> = None;
            ui.horizontal_wrapped(|ui| {
                for (i, tok) in toks.iter().enumerate() {
                    if ui.small_button(format!("{tok} ✕")).clicked() {
                        remove = Some(i);
                    }
                }
            });
            if let Some(i) = remove {
                let kept: Vec<&String> = toks
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, t)| t)
                    .collect();
                st.ui.filter = kept
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
            }
        }
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Focus hops:");
        ui.add(egui::Slider::new(&mut st.ui.focus_hops, 1..=10));
    });

    if let Some(f) = st.ui.focus.clone() {
        ui.label(format!("Focus: {}", f.0));
        if ui.button("Clear focus").clicked() {
            st.ui.focus = None;
            st.needs_redraw.store(true, Ordering::Relaxed);
        }
    } else {
        ui.label("Focus: (none) — click a node");
    }
}

fn section_alerts(ui: &mut egui::Ui, st: &mut GraphState, mission: &Mission, standard: bool) {
    gits::section_header(ui, "Incident Hunt", standard);
    ui.label(format!("Score: {}", mission.score));
    if mission.active {
        ui.label(
            egui::RichText::new(format!("▶ {}", mission.signature))
                .color(egui::Color32::from_rgb(255, 120, 60)),
        );
    }
    if !mission.last_message.is_empty() {
        ui.label(&mission.last_message);
    }
    ui.label("Press M to investigate the next alert.");

    gits::section_header(ui, "Alerts", standard);
    let (low, med, high) = st.alert_severity_counts();
    ui.horizontal(|ui| {
        ui.colored_label(color::SEV_LOW, format!("low {low}"));
        ui.colored_label(color::SEV_MED, format!("med {med}"));
        ui.colored_label(color::SEV_HIGH, format!("high {high}"));
    });
    if low + med + high == 0 {
        ui.label("no alerts");
    } else {
        let recent: Vec<(spacegraph_core::NodeId, String, String)> = st
            .alerts_newest_first()
            .take(10)
            .filter_map(|id| match st.core.model.nodes.get(id) {
                Some(spacegraph_core::Node::Alert {
                    signature,
                    severity,
                    ..
                }) => Some((id.clone(), severity.clone(), signature.clone())),
                _ => None,
            })
            .collect();
        let mut jump_to = None;
        for (id, sev, sig) in recent {
            if ui
                .selectable_label(false, format!("[{sev}] {sig}"))
                .clicked()
            {
                jump_to = Some(id);
            }
        }
        if let Some(id) = jump_to {
            st.ui.focus = Some(id.clone());
            st.ui.selected = Some(id.clone());
            st.ui.view_mode = ViewMode::Spatial;
            st.request_jump(id);
            st.needs_redraw.store(true, Ordering::Relaxed);
        }
    }
}

fn section_agents(ui: &mut egui::Ui, st: &mut GraphState, standard: bool) {
    gits::section_header(ui, "Agents", standard);
    let active = st.net.active_connection_count();
    match active {
        0 => ui.label("0 Agents connected"),
        1 => ui.label("1 Agent connected"),
        n => ui.label(format!("{n} Agents connected")),
    };
    if ui.button("Manage Agents…").clicked() {
        st.ui.show_agent_manager = true;
    }
}

fn section_settings(ui: &mut egui::Ui, st: &mut GraphState, standard: bool) {
    gits::section_header(ui, "Search", standard);
    ui.label("Ctrl+P opens the command palette. ? toggles help.");
    if ui.button("Open Search (Ctrl+P)").clicked() {
        st.ui.search_open = true;
    }

    gits::section_header(ui, "Settings", standard);
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

    gits::section_header(ui, "Actions", standard);
    if ui.button("Clear graph").clicked() {
        st.clear();
    }

    ui.separator();
    let mut tech_open = st.cfg.shell.technician_open;
    ui.checkbox(&mut tech_open, "⚙ Technician (tuning)");
    st.cfg.shell.technician_open = tech_open;
    if tech_open {
        technician_controls(ui, st, standard);
    }
}

fn technician_controls(ui: &mut egui::Ui, st: &mut GraphState, standard: bool) {
    gits::section_header(ui, "Performance", standard);
    ui.add(
        egui::Slider::new(&mut st.cfg.max_visible_nodes, 200..=10_000).text("max visible nodes"),
    );
    ui.add(
        egui::Slider::new(&mut st.cfg.progressive_nodes_per_frame, 50..=4000)
            .text("progressive/frame"),
    );

    gits::section_header(ui, "LOD / Rendering", standard);
    ui.checkbox(&mut st.cfg.lod_enabled, "Enable LOD");
    ui.add(egui::Slider::new(&mut st.cfg.lod_threshold_nodes, 500..=20_000).text("LOD threshold"));
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

    gits::section_header(ui, "Layout (Spatial)", standard);
    let mut changed = false;
    changed |= ui
        .checkbox(&mut st.cfg.layout_force, "Force layout")
        .changed();
    changed |= ui
        .add(egui::Slider::new(&mut st.cfg.link_distance, 1.0..=20.0).text("link dist"))
        .changed();
    changed |= ui
        .add(egui::Slider::new(&mut st.cfg.repulsion, 0.0..=120.0).text("repulsion"))
        .changed();
    changed |= ui
        .add(egui::Slider::new(&mut st.cfg.damping, 0.80..=0.999).text("damping"))
        .changed();
    changed |= ui
        .add(egui::Slider::new(&mut st.cfg.max_step, 0.05..=2.0).text("max step"))
        .changed();
    if changed {
        st.spatial.layout_settled = false;
        st.spatial.settle_streak = 0;
        st.needs_redraw.store(true, Ordering::Relaxed);
    }

    gits::section_header(ui, "Glow", standard);
    let mut ms = st.cfg.glow_duration.as_millis() as i32;
    ui.add(egui::Slider::new(&mut ms, 100..=3000).text("glow ms"));
    st.cfg.glow_duration = std::time::Duration::from_millis(ms as u64);

    gits::section_header(ui, "Gameplay", standard);
    ui.checkbox(&mut st.cfg.fog_of_war, "Fog of war (O)");
    ui.add(egui::Slider::new(&mut st.cfg.reveal_radius, 10.0..=200.0).text("reveal radius"));
    ui.add(egui::Slider::new(&mut st.cfg.scan_speed, 10.0..=300.0).text("scan speed"));
    ui.add(egui::Slider::new(&mut st.cfg.scan_max, 50.0..=1500.0).text("scan range"));
    ui.add(egui::Slider::new(&mut st.cfg.fly_speed, 2.0..=120.0).text("fly speed"));
    ui.add(egui::Slider::new(&mut st.cfg.fly_boost, 1.0..=12.0).text("fly boost"));
    ui.add(egui::Slider::new(&mut st.cfg.fly_sensitivity, 0.0005..=0.01).text("look sens"));
    ui.checkbox(&mut st.cfg.micro_tags, "Micro-tags (Standard)");
    ui.add(egui::Slider::new(&mut st.cfg.micro_tag_max, 0..=128).text("micro-tag max"));
    ui.checkbox(&mut st.cfg.node_rings, "Orbital rings (Standard)");
    ui.add(egui::Slider::new(&mut st.cfg.ring_min_degree, 1..=20).text("ring min degree"));
    ui.add(egui::Slider::new(&mut st.cfg.edge_pick_threshold, 0.05..=0.6).text("edge pick dist"));

    gits::section_header(ui, "Post-FX (Standard)", standard);
    ui.checkbox(&mut st.cfg.postfx.enabled, "Enabled");
    ui.add(egui::Slider::new(&mut st.cfg.postfx.scanline, 0.0..=0.5).text("scanline"));
    ui.add(egui::Slider::new(&mut st.cfg.postfx.vignette, 0.0..=1.0).text("vignette"));
    ui.add(egui::Slider::new(&mut st.cfg.postfx.aberration, 0.0..=2.0).text("aberration"));
    ui.add(egui::Slider::new(&mut st.cfg.postfx.grain, 0.0..=0.3).text("grain"));

    gits::section_header(ui, "Audio", standard);
    ui.checkbox(&mut st.cfg.audio_enabled, "Enabled");
    ui.add(egui::Slider::new(&mut st.cfg.audio_volume, 0.0..=1.0).text("volume"));
    if cfg!(not(feature = "audio")) {
        ui.label(egui::RichText::new("(build with --features audio)").weak());
    }

    gits::section_header(ui, "GC", standard);
    ui.checkbox(&mut st.cfg.gc_enabled, "enabled");
    let mut ttl = st.cfg.gc_ttl.as_secs() as i32;
    ui.add(egui::Slider::new(&mut ttl, 1..=600).text("orphan TTL (s)"));
    st.cfg.gc_ttl = std::time::Duration::from_secs(ttl as u64);
}

/// In-app selectors for `VisualTheme` (aesthetic) and `QualityTier` (cost).
/// Relocated verbatim from the old `ui_panel`.
fn theme_tier_selectors(ui: &mut egui::Ui, st: &mut GraphState, quality: &mut QualityState) {
    use crate::render::quality::{parse_tier, QualityTier};

    egui::ComboBox::from_label("Theme")
        .selected_text(match st.cfg.visual_theme {
            VisualTheme::Standard => "Standard",
            VisualTheme::Minimal => "Minimal",
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut st.cfg.visual_theme, VisualTheme::Standard, "Standard");
            ui.selectable_value(&mut st.cfg.visual_theme, VisualTheme::Minimal, "Minimal");
        });

    let tier_label = if st.cfg.quality.tier == "auto" {
        format!("Auto ({})", quality.effective.as_str())
    } else {
        st.cfg.quality.tier.clone()
    };
    egui::ComboBox::from_label("Quality")
        .selected_text(tier_label)
        .show_ui(ui, |ui| {
            for (label, val) in [
                ("Auto (detect)", "auto"),
                ("Potato", "potato"),
                ("Low", "low"),
                ("Medium", "medium"),
                ("High", "high"),
            ] {
                if ui
                    .selectable_label(st.cfg.quality.tier == val, label)
                    .clicked()
                {
                    st.cfg.quality.tier = val.to_string();
                    if let Some(t) = parse_tier(val) {
                        quality.base = t;
                        quality.set_effective(t);
                    }
                }
            }
        });

    if ui
        .checkbox(&mut st.cfg.quality.adaptive, "Adaptive quality")
        .changed()
    {
        quality.adaptive_on = st.cfg.quality.adaptive;
    }
    let _ = QualityTier::ALL;
}
