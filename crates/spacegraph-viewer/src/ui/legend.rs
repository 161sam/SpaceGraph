//! Colour legend — maps the neon palette to node and edge semantics.
//!
//! The Standard/Neon theme encodes meaning purely in colour, which is opaque to
//! newcomers. This overlay spells it out: node types, edge classes and the alert
//! severity ramp, each with its swatch. Toggle `L`.

use std::sync::atomic::Ordering;

use bevy::prelude::ResMut;
use bevy_egui::{egui, EguiContexts};

use crate::graph::model::{edge_class_name, EdgeKindClass};
use crate::graph::GraphState;
use crate::render::theme::{self, NodeKind};
use crate::ui::{egui_color, gits};
use crate::util::config::VisualTheme;

const NODE_LABELS: [(NodeKind, &str); 6] = [
    (NodeKind::Process, "Process"),
    (NodeKind::File, "File"),
    (NodeKind::User, "User"),
    (NodeKind::Socket, "Socket"),
    (NodeKind::RemoteHost, "Remote host"),
    (NodeKind::Alert, "Alert"),
];

const EDGE_CLASSES: [EdgeKindClass; 7] = [
    EdgeKindClass::Opens,
    EdgeKindClass::Execs,
    EdgeKindClass::RunsAs,
    EdgeKindClass::OwnsSocket,
    EdgeKindClass::ConnectsTo,
    EdgeKindClass::ListensOn,
    EdgeKindClass::AlertsOn,
];

const SEVERITIES: [&str; 3] = ["low", "medium", "high"];

fn swatch(ui: &mut egui::Ui, color: bevy::prelude::Color, label: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("⬤").color(egui_color(color)));
        ui.label(label);
    });
}

pub fn legend_overlay(mut contexts: EguiContexts, mut st: ResMut<GraphState>) {
    if !st.ui.legend_open {
        return;
    }

    let standard = st.cfg.visual_theme == VisualTheme::Standard;
    let mut open = st.ui.legend_open;
    let ctx = contexts.ctx_mut();
    let resp = egui::Window::new("Legend")
        .id(egui::Id::new("color_legend"))
        .open(&mut open)
        .resizable(false)
        .frame(gits::panel_frame(standard))
        .show(ctx, |ui| {
            ui.label(egui::RichText::new("Nodes").strong());
            for (kind, label) in NODE_LABELS {
                swatch(ui, kind.base_color(), label);
            }

            ui.separator();
            ui.label(egui::RichText::new("Edges").strong());
            for class in EDGE_CLASSES {
                swatch(ui, theme::edge_color(class), edge_class_name(class));
            }

            ui.separator();
            ui.label(egui::RichText::new("Alert severity").strong());
            for sev in SEVERITIES {
                swatch(ui, theme::alert_severity_color(sev), sev);
            }
        });
    if let Some(r) = resp {
        gits::bracket_response(ctx, r.response.rect, standard);
    }

    if open != st.ui.legend_open {
        st.ui.legend_open = open;
        st.needs_redraw.store(true, Ordering::Relaxed);
    }
}
