//! Radial in-world context menu (right-click a node).
//!
//! Opened by `picking_focus` (sets `ui.context_menu = Some((node, screen_pos))`),
//! rendered here as an egui popup at that position. Actions are deferred via the
//! `CtxAct` enum and applied through `apply_context_action` (unit-tested) to
//! avoid borrowing `GraphState` mutably while the egui closure is live.

use bevy::prelude::ResMut;
use bevy_egui::{egui, EguiContexts};
use spacegraph_core::NodeId;
use std::sync::atomic::Ordering;

use crate::graph::GraphState;

/// A context-menu action. Mapping to state changes is `apply_context_action`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CtxAct {
    Focus,
    TogglePin,
    Isolate,
    Trace,
    ToggleMark,
    Inspect,
}

const ACTIONS: [(CtxAct, &str); 6] = [
    (CtxAct::Focus, "Fly-to"),
    (CtxAct::Isolate, "Isolate subgraph"),
    (CtxAct::Trace, "Trace connections"),
    (CtxAct::TogglePin, "Pin / Unpin"),
    (CtxAct::ToggleMark, "Mark / Unmark"),
    (CtxAct::Inspect, "Inspect"),
];

/// Apply a context action to graph state. Pure mapping (no egui) — unit-tested.
pub fn apply_context_action(st: &mut GraphState, id: &NodeId, act: CtxAct) {
    match act {
        CtxAct::Focus => {
            st.ui.focus = Some(id.clone());
            st.ui.selected = Some(id.clone());
            st.request_jump(id.clone());
        }
        CtxAct::Isolate => {
            st.ui.focus = Some(id.clone());
            st.ui.selected = Some(id.clone());
            st.ui.multi_selected.clear();
            st.spatial.dirty_layout = true;
        }
        CtxAct::Trace => {
            st.ui.compare_pin = Some(id.clone());
            st.ui.selected = Some(id.clone());
        }
        CtxAct::TogglePin => {
            if st.is_pinned(id) {
                st.clear_pin(id);
            } else if let Some(pos) = st.spatial.position_of(id) {
                st.set_pin(id, pos);
            }
        }
        CtxAct::ToggleMark => {
            if !st.ui.marked.remove(id) {
                st.ui.marked.insert(id.clone());
            }
        }
        CtxAct::Inspect => {
            st.ui.selected = Some(id.clone());
            st.ui.inspector_open = true;
        }
    }
    st.needs_redraw.store(true, Ordering::Relaxed);
}

pub fn context_menu_overlay(mut contexts: EguiContexts, mut st: ResMut<GraphState>) {
    let Some((id, pos)) = st.ui.context_menu.clone() else {
        return;
    };

    let mut chosen: Option<CtxAct> = None;
    let mut close = false;
    let pinned = st.is_pinned(&id);
    let marked = st.ui.marked.contains(&id);
    let ctx = contexts.ctx_mut();

    let area = egui::Area::new(egui::Id::new("context_menu"))
        .fixed_pos(egui::pos2(pos.x, pos.y))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(180.0);
                ui.label(egui::RichText::new("Node actions").strong());
                for (act, base_label) in ACTIONS {
                    let label = match act {
                        CtxAct::TogglePin if pinned => "Unpin",
                        CtxAct::TogglePin => "Pin",
                        CtxAct::ToggleMark if marked => "Unmark",
                        CtxAct::ToggleMark => "Mark",
                        _ => base_label,
                    };
                    if ui.button(label).clicked() {
                        chosen = Some(act);
                    }
                }
            });
        });

    // Click outside the popup → dismiss.
    if ctx.input(|i| i.pointer.any_click()) && !area.response.contains_pointer() {
        close = true;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }

    if let Some(act) = chosen {
        apply_context_action(&mut st, &id, act);
        close = true;
    }
    if close {
        st.ui.context_menu = None;
        st.needs_redraw.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::GraphState;
    use bevy::prelude::Vec3;

    fn state_with_node() -> (GraphState, NodeId) {
        let mut st = GraphState::default();
        let id = NodeId("n".to_string());
        st.model.nodes.insert(id.clone(), process_node());
        let idx = st.spatial.intern(&id);
        st.spatial.set_position(idx, Vec3::ZERO);
        (st, id)
    }

    fn process_node() -> spacegraph_core::Node {
        spacegraph_core::Node::Process {
            pid: 1,
            ppid: 0,
            exe: "x".to_string(),
            cmdline: "x".to_string(),
            uid: 0,
        }
    }

    #[test]
    fn toggle_pin_round_trips() {
        let (mut st, id) = state_with_node();
        assert!(!st.is_pinned(&id));
        apply_context_action(&mut st, &id, CtxAct::TogglePin);
        assert!(st.is_pinned(&id), "pin set");
        apply_context_action(&mut st, &id, CtxAct::TogglePin);
        assert!(!st.is_pinned(&id), "pin cleared");
    }

    #[test]
    fn toggle_mark_round_trips() {
        let (mut st, id) = state_with_node();
        apply_context_action(&mut st, &id, CtxAct::ToggleMark);
        assert!(st.ui.marked.contains(&id));
        apply_context_action(&mut st, &id, CtxAct::ToggleMark);
        assert!(!st.ui.marked.contains(&id));
    }

    #[test]
    fn focus_trace_inspect_set_expected_state() {
        let (mut st, id) = state_with_node();
        apply_context_action(&mut st, &id, CtxAct::Focus);
        assert_eq!(st.ui.focus.as_ref(), Some(&id));

        apply_context_action(&mut st, &id, CtxAct::Trace);
        assert_eq!(st.ui.compare_pin.as_ref(), Some(&id));

        st.ui.inspector_open = false;
        apply_context_action(&mut st, &id, CtxAct::Inspect);
        assert!(st.ui.inspector_open);
        assert_eq!(st.ui.selected.as_ref(), Some(&id));
    }
}
