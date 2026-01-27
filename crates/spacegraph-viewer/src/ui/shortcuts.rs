use bevy::prelude::ResMut;
use bevy_egui::{egui, EguiContexts};
use std::sync::atomic::Ordering;

use crate::graph::{GraphState, ViewMode};

pub fn handle_shortcuts(mut contexts: EguiContexts, mut st: ResMut<GraphState>) {
    let ctx = contexts.ctx_mut();
    let esc_pressed = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let wants_keyboard = ctx.wants_keyboard_input();

    if esc_pressed {
        let mut changed = false;
        if st.ui.search_open {
            st.ui.search_open = false;
            changed = true;
        }
        if st.ui.help_open {
            st.ui.help_open = false;
            changed = true;
        }
        if st.ui.focus.is_some() {
            st.ui.focus = None;
            changed = true;
        }
        if st.ui.selected.is_some()
            || st.ui.selected_a.is_some()
            || st.ui.selected_b.is_some()
            || st.ui.hovered.is_some()
        {
            st.ui.selected = None;
            st.ui.selected_a = None;
            st.ui.selected_b = None;
            st.ui.hovered = None;
            changed = true;
        }
        if changed {
            st.needs_redraw.store(true, Ordering::Relaxed);
        }
    }

    if wants_keyboard {
        return;
    }

    if ctx.input(|i| i.key_pressed(egui::Key::P) && i.modifiers.ctrl) {
        st.ui.search_open = true;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Questionmark)) {
        st.ui.help_open = !st.ui.help_open;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::F)) {
        if let Some(id) = st.ui.selected.clone().or_else(|| st.ui.selected_a.clone()) {
            st.ui.focus = Some(id);
            st.needs_redraw.store(true, Ordering::Relaxed);
        }
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Space)) && st.ui.view_mode == ViewMode::Timeline {
        let pause = !st.timeline.pause;
        st.set_timeline_pause(pause);
    }
    if ctx.input(|i| i.key_pressed(egui::Key::T)) {
        st.ui.view_mode = match st.ui.view_mode {
            ViewMode::Spatial => ViewMode::Timeline,
            ViewMode::Timeline => ViewMode::Spatial,
        };
        st.needs_redraw.store(true, Ordering::Relaxed);
    }
}
