use bevy::prelude::ResMut;
use bevy_egui::{egui, EguiContexts};

use crate::graph::GraphState;

pub fn help_overlay(mut contexts: EguiContexts, st: ResMut<GraphState>) {
    if !st.ui.help_open {
        return;
    }

    egui::Window::new("Help / Shortcuts")
        .collapsible(false)
        .resizable(false)
        .show(contexts.ctx_mut(), |ui| {
            ui.label("Ctrl+P — Search");
            ui.label("Esc — Clear selection/focus, close overlays");
            ui.label("F — Focus selected");
            ui.label("Space — Pause timeline");
            ui.label("T — Toggle view (Spatial/Tree/Timeline)");
            ui.label("? — Toggle help");
        });
}
