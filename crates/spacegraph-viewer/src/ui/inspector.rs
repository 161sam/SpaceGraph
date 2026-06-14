//! Node inspector — a detail panel for the current selection.
//!
//! Shows the selected node's type, fields, origin and fog state, lists its
//! connections (colour-coded by edge class, click to navigate), and can pin a
//! node to explain *why* two nodes are connected (shortest path). Toggle `I`.

use std::collections::HashSet;
use std::sync::atomic::Ordering;

use bevy::prelude::ResMut;
use bevy_egui::{egui, EguiContexts};
use spacegraph_core::NodeId;

use crate::graph::model::{edge_class_name, EdgeKindClass};
use crate::graph::GraphState;
use crate::render::theme;
use crate::ui::egui_color;
use crate::util::ids::node_label_short;

/// Deferred mutation requested from inside the egui closure (applied after, to
/// avoid borrowing `GraphState` both mutably and immutably at once).
enum Act {
    Select(NodeId),
    Focus(NodeId),
    Pin(NodeId),
    ClearPin,
}

pub fn inspector_overlay(mut contexts: EguiContexts, mut st: ResMut<GraphState>) {
    if !st.ui.inspector_open {
        return;
    }
    let Some(id) = st.ui.selected.clone().or_else(|| st.ui.focus.clone()) else {
        return;
    };
    let Some(node) = st.core.model.nodes.get(&id).cloned() else {
        return;
    };

    // --- gather display data (immutable) ---
    let title = node_label_short(&node);
    let detail = st.node_tooltip_lines(&id);
    let fog = st.cfg.fog_of_war;
    let revealed = st.is_visible_rendered(&id);

    // Connections, de-duplicated by neighbour, sorted by label.
    let mut neighbors: Vec<(EdgeKindClass, NodeId, String)> = Vec::new();
    {
        let mut seen: HashSet<NodeId> = HashSet::new();
        for edge in st.core.model.edges_for_node(&id) {
            let other = if edge.from == id {
                edge.to.clone()
            } else {
                edge.from.clone()
            };
            if other == id || !seen.insert(other.clone()) {
                continue;
            }
            let class = EdgeKindClass::from_kind(&edge.kind);
            let label = st
                .core
                .model
                .nodes
                .get(&other)
                .map(node_label_short)
                .unwrap_or_else(|| other.0.clone());
            neighbors.push((class, other, label));
        }
    }
    neighbors.sort_by(|a, b| a.2.cmp(&b.2));
    let neighbor_count = neighbors.len();

    // "Why connected" path from the pinned anchor to this node.
    let pin = st.ui.compare_pin.clone();
    let compare_lines: Option<Vec<String>> = match pin.clone() {
        Some(from) if from != id => {
            let vis = st.spatial.vis_cache.clone();
            match st.explain_path_cached(&from, &id, &vis) {
                Some(path) if path.is_empty() => Some(vec!["same node".to_string()]),
                Some(path) => Some(
                    path.iter()
                        .map(|s| {
                            format!(
                                "{} --[{}]--> {}",
                                st.node_label_with_id(&s.from),
                                edge_class_name(s.class),
                                st.node_label_with_id(&s.to),
                            )
                        })
                        .collect(),
                ),
                None => Some(vec!["no path within depth cap".to_string()]),
            }
        }
        _ => None,
    };
    let pinned_self = pin.as_ref() == Some(&id);

    // --- render (docked right panel, spec §3.2) ---
    if !st.cfg.shell.right_open {
        return;
    }
    let mut act: Option<Act> = None;
    let right_width = st.cfg.shell.right_width;
    let resp = egui::SidePanel::right("node_inspector")
        .resizable(true)
        .default_width(right_width)
        .width_range(220.0..=600.0)
        .show(contexts.ctx_mut(), |ui| {
            // Truncate/wrap every dynamic field so a long value (e.g. a process
            // cmdline used as a label) can't force the panel wider than the user
            // dragged it — the blow-up that made the resize snap back.
            ui.add(
                egui::Label::new(egui::RichText::new(format!("🔍 {title}")).heading())
                    .wrap_mode(egui::TextWrapMode::Truncate),
            )
            .on_hover_text(&title);
            for line in &detail {
                ui.add(egui::Label::new(line).wrap_mode(egui::TextWrapMode::Wrap));
            }
            if fog {
                ui.separator();
                ui.label(if revealed {
                    "fog: revealed"
                } else {
                    "fog: hidden (explore to reveal)"
                });
            }

            ui.separator();
            ui.label(egui::RichText::new(format!("connections ({neighbor_count})")).strong());
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for (class, nid, label) in &neighbors {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("⬤")
                                    .color(egui_color(theme::edge_color(*class))),
                            );
                            if ui
                                .add(
                                    egui::Button::new(label)
                                        .wrap_mode(egui::TextWrapMode::Truncate),
                                )
                                .on_hover_text(format!("{label}\n{}", edge_class_name(*class)))
                                .clicked()
                            {
                                act = Some(Act::Select(nid.clone()));
                            }
                        });
                    }
                });

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Fly-to (F)").clicked() {
                    act = Some(Act::Focus(id.clone()));
                }
                if pinned_self {
                    if ui.button("Unpin").clicked() {
                        act = Some(Act::ClearPin);
                    }
                } else if ui.button("Pin compare").clicked() {
                    act = Some(Act::Pin(id.clone()));
                }
            });

            if let Some(lines) = &compare_lines {
                ui.separator();
                ui.label(egui::RichText::new("why connected").strong());
                for line in lines {
                    ui.add(egui::Label::new(line).wrap_mode(egui::TextWrapMode::Wrap));
                }
                if ui.button("clear compare").clicked() {
                    act = Some(Act::ClearPin);
                }
            }
        });

    // Persist the dragged width (mirrors the left rail in `ui_panel`) so it
    // survives "Save Settings" and no longer snaps back to the default.
    st.cfg.shell.right_width = resp.response.rect.width();

    // --- apply deferred state changes ---
    let mut changed = false;
    match act {
        Some(Act::Select(nid)) => {
            st.reveal(&nid);
            st.ui.selected = Some(nid.clone());
            st.ui.focus = Some(nid);
            changed = true;
        }
        Some(Act::Focus(fid)) => {
            st.ui.focus = Some(fid.clone());
            st.request_jump(fid);
            changed = true;
        }
        Some(Act::Pin(pid)) => {
            st.ui.compare_pin = Some(pid);
            changed = true;
        }
        Some(Act::ClearPin) => {
            st.ui.compare_pin = None;
            changed = true;
        }
        None => {}
    }
    if changed {
        st.needs_redraw.store(true, Ordering::Relaxed);
    }
}
