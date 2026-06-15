//! Focus entity card (MP-UI-GitS-polish, P7) — the framed GitS readout for the
//! focused node, in three blocks: **identity** (type glyph + hex id + per-kind
//! fields), **state** (origin · degree meter · severity), and **connections** (the
//! de-duped, per-class-coloured, clickable neighbour list). Corner-anchored
//! bottom-right within the content rect so it never overlaps the node or the other
//! panels. Primary actions (Fly-to / Pin) live here; the rest are on the radial
//! ring. Minimal → a plain flat card.

use bevy::prelude::{Res, ResMut};
use bevy_egui::{egui, EguiContexts};
use std::collections::HashSet;
use std::sync::atomic::Ordering;

use spacegraph_core::{Node, NodeId};

use crate::graph::model::EdgeKindClass;
use crate::graph::{namespace, GraphState};
use crate::render::theme::{self, NodeKind};
use crate::ui::overlay::{layer, middle_truncate};
use crate::ui::tokens::color;
use crate::ui::{egui_color, gits, UiLayout};
use crate::util::config::VisualTheme;
use crate::util::ids::{node_label_short, short_hex_id};

/// How many neighbour rows the card lists before "+N more".
const MAX_CONN_ROWS: usize = 6;

enum CardAct {
    Focus,
    Pin,
    ClearPin,
    /// Click a connection row → re-centre focus on that neighbour (graph traversal).
    Dive(NodeId),
}

/// Draw the focus entity card (only while Focus Mode is active).
pub fn entity_card_overlay(
    mut contexts: EguiContexts,
    mut st: ResMut<GraphState>,
    layout: Res<UiLayout>,
) {
    let Some(id) = st.ui.focus_mode.clone() else {
        return;
    };
    let Some(node) = st.core.model.nodes.get(&id).cloned() else {
        return;
    };
    let standard = st.cfg.visual_theme == VisualTheme::Standard;
    let kind = NodeKind::of(&node);
    let accent = if standard {
        kind_color(kind)
    } else {
        color::TEXT
    };
    let hexid = short_hex_id(&id);

    // --- identity fields (skip the redundant `kind:` line; header carries it) ---
    let fields: Vec<String> = st
        .node_tooltip_lines(&id)
        .into_iter()
        .filter(|l| !l.starts_with("kind:"))
        .collect();
    let degree = st.core.model.degree(&id);
    let origin = namespace::origin(&id).unwrap_or("local").to_string();
    let severity = match &node {
        Node::Alert { severity, .. } => Some(severity.clone()),
        _ => None,
    };

    // --- connections: de-duped by neighbour, per-class, sorted by label ---
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
    let conn_total = neighbors.len();

    let pinned_self = st.ui.compare_pin.as_ref() == Some(&id);

    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };
    let content = if layout.content_rect.width() > 0.0 && layout.content_rect.height() > 0.0 {
        layout.content_rect
    } else {
        ctx.screen_rect()
    };

    let mut act: Option<CardAct> = None;
    let resp = egui::Window::new("entity_card")
        .order(layer::PANEL)
        .anchor(egui::Align2::RIGHT_BOTTOM, [-14.0, -14.0])
        .constrain_to(content)
        .frame(gits::panel_frame(standard))
        .resizable(false)
        .collapsible(false)
        .title_bar(false)
        .show(ctx, |ui| {
            ui.set_min_width(248.0);
            ui.set_max_width(308.0);

            // ── Header: type glyph · ENTITY · kind · hex id · live dot ──
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(silhouette(kind))
                        .size(22.0)
                        .color(accent),
                );
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("◢ ENTITY")
                                .monospace()
                                .small()
                                .color(color::TEXT_DIM),
                        );
                        ui.label(
                            egui::RichText::new(kind_name(kind))
                                .monospace()
                                .strong()
                                .color(accent),
                        );
                    });
                    ui.label(
                        egui::RichText::new(&hexid)
                            .monospace()
                            .small()
                            .color(color::TEXT_DIM),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("●").small().color(color::ACCENT_HI));
                });
            });
            ui.separator();

            // ── IDENTITY ──
            gits::section_header(ui, "IDENTITY", standard);
            for line in &fields {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(middle_truncate(line, 42))
                            .monospace()
                            .size(11.0),
                    )
                    .wrap_mode(egui::TextWrapMode::Truncate),
                )
                .on_hover_text(line);
            }

            // ── STATE ──
            ui.add_space(2.0);
            gits::section_header(ui, "STATE", standard);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("origin {origin}"))
                        .monospace()
                        .size(11.0)
                        .color(color::TEXT_DIM),
                );
                if let Some(sev) = &severity {
                    ui.label(
                        egui::RichText::new(format!("· sev {sev}"))
                            .monospace()
                            .size(11.0)
                            .color(egui_color(theme::alert_severity_color(sev))),
                    );
                }
            });
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("degree {degree}"))
                        .monospace()
                        .size(11.0)
                        .color(color::TEXT_DIM),
                );
                meter(ui, degree.min(8), 8, accent);
            });

            // ── CONNECTIONS ──
            ui.add_space(2.0);
            gits::section_header(ui, &format!("CONNECTIONS · {conn_total}"), standard);
            for (class, nid, label) in neighbors.iter().take(MAX_CONN_ROWS) {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("●")
                            .small()
                            .color(egui_color(theme::edge_color(*class))),
                    );
                    if ui
                        .add(
                            egui::Label::new(
                                egui::RichText::new(middle_truncate(label, 26))
                                    .monospace()
                                    .size(11.0),
                            )
                            .sense(egui::Sense::click()),
                        )
                        .on_hover_text(format!("{label}\n{}", class_name(*class)))
                        .clicked()
                    {
                        act = Some(CardAct::Dive(nid.clone()));
                    }
                });
            }
            if conn_total > MAX_CONN_ROWS {
                ui.label(
                    egui::RichText::new(format!("+ {} more…", conn_total - MAX_CONN_ROWS))
                        .monospace()
                        .small()
                        .color(color::TEXT_DIM),
                );
            }

            // ── Primary actions (the rest live on the radial ring) ──
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Fly-to (F)").clicked() {
                    act = Some(CardAct::Focus);
                }
                if pinned_self {
                    if ui.button("Unpin").clicked() {
                        act = Some(CardAct::ClearPin);
                    }
                } else if ui.button("Pin compare").clicked() {
                    act = Some(CardAct::Pin);
                }
            });
        });
    if let Some(resp) = resp {
        gits::bracket_response(ctx, resp.response.rect, standard);
    }

    match act {
        Some(CardAct::Focus) => {
            st.request_jump(id.clone());
            st.needs_redraw.store(true, Ordering::Relaxed);
        }
        Some(CardAct::Pin) => {
            st.ui.compare_pin = Some(id.clone());
            st.needs_redraw.store(true, Ordering::Relaxed);
        }
        Some(CardAct::ClearPin) => {
            st.ui.compare_pin = None;
            st.needs_redraw.store(true, Ordering::Relaxed);
        }
        Some(CardAct::Dive(other)) => {
            // Re-centre Focus Mode on the clicked neighbour (graph traversal).
            st.reveal(&other);
            st.ui.focus_mode = Some(other.clone());
            st.ui.selected = Some(other.clone());
            st.request_jump(other);
            st.needs_redraw.store(true, Ordering::Relaxed);
        }
        None => {}
    }
}

/// A segmented meter bar: `filled` of `total` cells in `col`, the rest dim.
fn meter(ui: &mut egui::Ui, filled: usize, total: usize, col: egui::Color32) {
    let n = total.max(1);
    let w = ui.available_width().clamp(60.0, 150.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 7.0), egui::Sense::hover());
    let gap = 2.0;
    let seg_w = ((rect.width() - gap * (n as f32 - 1.0)) / n as f32).max(1.0);
    for i in 0..n {
        let x = rect.left() + i as f32 * (seg_w + gap);
        let r =
            egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(seg_w, rect.height()));
        ui.painter()
            .rect_filled(r, 1.0, if i < filled { col } else { color::LINE });
    }
}

/// A compact type silhouette glyph per node kind.
fn silhouette(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Process => "▣",
        NodeKind::File => "▢",
        NodeKind::User => "◉",
        NodeKind::Socket => "◈",
        NodeKind::RemoteHost => "⬢",
        NodeKind::Alert => "⚠",
    }
}

fn kind_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Process => "process",
        NodeKind::File => "file",
        NodeKind::User => "user",
        NodeKind::Socket => "socket",
        NodeKind::RemoteHost => "host",
        NodeKind::Alert => "alert",
    }
}

/// Per-type chrome accent (mirrors the node palette).
fn kind_color(kind: NodeKind) -> egui::Color32 {
    match kind {
        NodeKind::Process => color::PROCESS,
        NodeKind::File => color::FILE,
        NodeKind::User => color::USER,
        NodeKind::Socket => color::SOCKET,
        NodeKind::RemoteHost => color::REMOTEHOST,
        NodeKind::Alert => color::ALERT,
    }
}

/// Short relation tag for a connection row's hover.
fn class_name(class: EdgeKindClass) -> &'static str {
    match class {
        EdgeKindClass::Opens => "opens",
        EdgeKindClass::Execs => "execs",
        EdgeKindClass::RunsAs => "runs-as",
        EdgeKindClass::OwnsSocket => "owns-socket",
        EdgeKindClass::ConnectsTo => "connects-to",
        EdgeKindClass::ListensOn => "listens-on",
        EdgeKindClass::AlertsOn => "alerts-on",
    }
}
