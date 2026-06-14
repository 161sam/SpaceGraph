//! Focus entity card (MP-UI-GitS, P3) — a framed GitS readout for the focused
//! node: a type silhouette, the per-kind identity fields, origin and connection
//! count, plus the Fly-to / Pin-compare actions. Corner-anchored (bottom-right,
//! clear of the centered node, the minimap and the focus preview) via the P1
//! layer model, so it never overlaps the node. Minimal → a plain flat card.

use bevy::prelude::ResMut;
use bevy_egui::{egui, EguiContexts};
use std::sync::atomic::Ordering;

use crate::graph::{namespace, GraphState};
use crate::render::theme::NodeKind;
use crate::ui::context_menu::{apply_context_action, CtxAct};
use crate::ui::gits;
use crate::ui::overlay::layer;
use crate::ui::tokens::color;
use crate::util::config::VisualTheme;

enum CardAct {
    Focus,
    Pin,
    ClearPin,
    Ctx(CtxAct),
}

/// Draw the focus entity card (only while Focus Mode is active).
pub fn entity_card_overlay(mut contexts: EguiContexts, mut st: ResMut<GraphState>) {
    let Some(id) = st.ui.focus_mode.clone() else {
        return;
    };
    let Some(node) = st.core.model.nodes.get(&id).cloned() else {
        return;
    };
    let standard = st.cfg.visual_theme == VisualTheme::Standard;
    let kind = NodeKind::of(&node);
    let fields = st.node_tooltip_lines(&id);
    let degree = st.core.model.degree(&id);
    let origin = namespace::origin(&id).unwrap_or("local").to_string();
    let pinned_self = st.ui.compare_pin.as_ref() == Some(&id);
    let marked = st.ui.marked.contains(&id);

    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };
    let mut act: Option<CardAct> = None;
    let resp = egui::Window::new("entity_card")
        .order(layer::PANEL)
        .anchor(egui::Align2::RIGHT_BOTTOM, [-14.0, -14.0])
        .frame(gits::panel_frame(standard))
        .resizable(false)
        .collapsible(false)
        .title_bar(false)
        .show(ctx, |ui| {
            ui.set_min_width(238.0);
            ui.set_max_width(300.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(silhouette(kind))
                        .size(24.0)
                        .color(if standard { color::ACCENT } else { color::TEXT }),
                );
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("◢ ENTITY")
                            .monospace()
                            .small()
                            .color(color::TEXT_DIM),
                    );
                    ui.label(egui::RichText::new(kind_name(kind)).monospace().strong());
                });
            });
            ui.separator();
            for line in &fields {
                ui.add(
                    egui::Label::new(egui::RichText::new(line).monospace().size(11.0))
                        .wrap_mode(egui::TextWrapMode::Truncate),
                )
                .on_hover_text(line);
            }
            ui.label(
                egui::RichText::new(format!("origin: {origin}"))
                    .monospace()
                    .size(11.0)
                    .color(color::TEXT_DIM),
            );
            ui.label(
                egui::RichText::new(format!("connections: {degree}"))
                    .monospace()
                    .size(11.0)
                    .color(color::TEXT_DIM),
            );
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
            ui.horizontal(|ui| {
                if ui.button("Isolate").clicked() {
                    act = Some(CardAct::Ctx(CtxAct::Isolate));
                }
                if ui.button("Trace").clicked() {
                    act = Some(CardAct::Ctx(CtxAct::Trace));
                }
                if ui.button(if marked { "Unmark" } else { "Mark" }).clicked() {
                    act = Some(CardAct::Ctx(CtxAct::ToggleMark));
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
        Some(CardAct::Ctx(a)) => apply_context_action(&mut st, &id, a),
        None => {}
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
