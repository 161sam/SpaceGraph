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
            ui.label(egui::RichText::new("Navigation").strong());
            ui.label("Left-click — Select node");
            ui.label("Right-drag — Orbit camera");
            ui.label("Middle-drag — Pan camera");
            ui.label("Scroll — Zoom");
            ui.label("F — Fly to / lock on selected");
            ui.label("V — Free-fly (pilot): WASD/QE move, mouse look, Shift boost");
            ui.separator();
            ui.label(egui::RichText::new("Shortcuts").strong());
            ui.label("Ctrl+P — Search");
            ui.label("Esc — Clear selection/focus, close overlays");
            ui.label("Space — Pause timeline");
            ui.label("T — Toggle view (Spatial/Tree/Timeline)");
            ui.label("? — Toggle help");
        });
}
