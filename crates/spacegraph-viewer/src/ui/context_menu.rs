//! Radial in-world context menu (right-click a node).
//!
//! Opened by `picking_focus` (sets `ui.context_menu = Some((node, screen_pos))`),
//! rendered here as an egui popup at that position. Actions are deferred via the
//! `CtxAct` enum and applied through `apply_context_action` (unit-tested) to
//! avoid borrowing `GraphState` mutably while the egui closure is live.

use bevy::prelude::{Camera, GlobalTransform, Query, ResMut, Resource};
use bevy_egui::{egui, EguiContexts};
use spacegraph_core::NodeId;
use std::sync::atomic::Ordering;

use crate::graph::GraphState;
use crate::ui::tokens::color;

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

// ===========================================================================
// Radial command HUD (v0.5.0, spec §3.4) — the keyboard-driven evolution of the
// mouse context menu. Reuses `CtxAct` / `ACTIONS` / `apply_context_action`.
// ===========================================================================

/// Which ring is active for keyboard navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ring {
    /// Inner ring: fixed command verbs.
    Commands,
    /// Outer ring: the focused node's neighbours (paths), paged.
    Paths,
}

/// Open radial HUD state (kept in a UI-side resource, not graph truth).
#[derive(Debug, Clone)]
pub struct RadialState {
    pub focused: NodeId,
    pub active_ring: Ring,
    /// Highlighted slot within the active ring.
    pub cursor: usize,
    /// Page of the outer (paths) ring.
    pub path_page: usize,
}

/// Outer-ring page size.
pub const PATHS_PER_PAGE: usize = 9;

impl RadialState {
    pub fn open(focused: NodeId) -> Self {
        Self {
            focused,
            active_ring: Ring::Commands,
            cursor: 0,
            path_page: 0,
        }
    }

    /// Toggle Commands ⇄ Paths (resets the cursor).
    pub fn switch_ring(&mut self) {
        self.active_ring = match self.active_ring {
            Ring::Commands => Ring::Paths,
            Ring::Paths => Ring::Commands,
        };
        self.cursor = 0;
    }

    /// Rotate the cursor around the active ring with wrap-around.
    pub fn rotate(&mut self, delta: i32, ring_len: usize) {
        if ring_len == 0 {
            self.cursor = 0;
            return;
        }
        let n = ring_len as i32;
        self.cursor = (((self.cursor as i32 + delta) % n + n) % n) as usize;
    }

    /// Page the outer ring, clamped to `[0, pages-1]` (resets the cursor).
    pub fn page(&mut self, delta: i32, total_paths: usize) {
        let pages = total_paths.div_ceil(PATHS_PER_PAGE).max(1);
        let p = (self.path_page as i32 + delta).clamp(0, pages as i32 - 1);
        self.path_page = p as usize;
        self.cursor = 0;
    }
}

/// Inner-ring command at `slot`.
pub fn command_at(slot: usize) -> Option<CtxAct> {
    ACTIONS.get(slot).map(|(a, _)| *a)
}

pub fn command_count() -> usize {
    ACTIONS.len()
}

/// Outer-ring neighbour at `(page, slot)`.
pub fn path_at(neighbors: &[NodeId], page: usize, slot: usize) -> Option<&NodeId> {
    neighbors.get(page * PATHS_PER_PAGE + slot)
}

/// Unique neighbours of a node (deterministic order) — the outer-ring paths.
pub fn radial_neighbors(st: &GraphState, id: &NodeId) -> Vec<NodeId> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for edge in st.model.edges_for_node(id) {
        let other = if &edge.from == id {
            edge.to.clone()
        } else {
            edge.from.clone()
        };
        if &other != id && seen.insert(other.clone()) {
            out.push(other);
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// The open radial HUD (None = closed). UI state, deliberately outside
/// `GraphState` (module boundary: graph truth stays Bevy/UI-free).
#[derive(Resource, Default)]
pub struct RadialMenu(pub Option<RadialState>);

/// Number of selectable slots in the currently-active ring.
fn active_ring_len(state: &RadialState, neighbor_count: usize) -> usize {
    match state.active_ring {
        Ring::Commands => command_count(),
        Ring::Paths => {
            let start = state.path_page * PATHS_PER_PAGE;
            neighbor_count.saturating_sub(start).min(PATHS_PER_PAGE)
        }
    }
}

/// Keyboard-driven radial command HUD: input + screen-space render. Tier-
/// independent (egui), determinism-exempt. Uses `try_ctx_mut` so it is panic-free
/// headless (no egui context → no draw).
pub fn radial_hud(
    mut contexts: EguiContexts,
    mut st: ResMut<GraphState>,
    mut radial: ResMut<RadialMenu>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
) {
    let Some(state) = radial.0.clone() else {
        return;
    };
    if !st.model.nodes.contains_key(&state.focused) {
        radial.0 = None;
        return;
    }
    let neighbors = radial_neighbors(&st, &state.focused);
    let ring_len = active_ring_len(&state, neighbors.len());
    let total_paths = neighbors.len();
    let screen = cam_q
        .get_single()
        .ok()
        .zip(st.spatial.position_of(&state.focused))
        .and_then(|((cam, tf), p)| cam.world_to_viewport(tf, p));

    let Some(ctx) = contexts.try_ctx_mut() else {
        return; // headless / no context yet
    };

    enum Do {
        Cmd(CtxAct),
        Dive(NodeId),
        Close,
        None,
    }
    let mut next = state.clone();
    let mut act = Do::None;
    ctx.input(|i| {
        use egui::Key;
        if i.key_pressed(Key::Escape) {
            act = Do::Close;
        }
        if i.key_pressed(Key::Tab) || i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::ArrowDown) {
            next.switch_ring();
        }
        if i.key_pressed(Key::ArrowLeft) {
            next.rotate(-1, ring_len);
        }
        if i.key_pressed(Key::ArrowRight) {
            next.rotate(1, ring_len);
        }
        if i.key_pressed(Key::OpenBracket) {
            next.page(-1, total_paths);
        }
        if i.key_pressed(Key::CloseBracket) {
            next.page(1, total_paths);
        }
        for (n, key) in [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ]
        .into_iter()
        .enumerate()
        {
            if i.key_pressed(key) && n < ring_len {
                next.cursor = n;
            }
        }
        if i.key_pressed(Key::Enter) {
            match next.active_ring {
                Ring::Commands => {
                    if let Some(a) = command_at(next.cursor) {
                        act = Do::Cmd(a);
                    }
                }
                Ring::Paths => {
                    if let Some(nid) = path_at(&neighbors, next.path_page, next.cursor) {
                        act = Do::Dive(nid.clone());
                    }
                }
            }
        }
    });

    if let Some(screen) = screen {
        render_radial(ctx, screen, &next, &neighbors, &st);
    }

    match act {
        Do::Cmd(a) => {
            apply_context_action(&mut st, &state.focused, a);
            radial.0 = None;
        }
        Do::Dive(nid) => {
            st.reveal(&nid);
            st.ui.focus = Some(nid.clone());
            st.ui.selected = Some(nid.clone());
            st.request_jump(nid.clone());
            radial.0 = Some(RadialState::open(nid)); // keyboard graph traversal
        }
        Do::Close => {
            radial.0 = None;
            st.needs_redraw.store(true, Ordering::Relaxed);
        }
        Do::None => radial.0 = Some(next),
    }
}

/// Draw the two concentric rings (egui painter) at the focused node's projected
/// position, with numbered slots, active-ring/cursor highlight, and a centre
/// identity label.
fn render_radial(
    ctx: &egui::Context,
    screen: bevy::math::Vec2,
    state: &RadialState,
    neighbors: &[NodeId],
    st: &GraphState,
) {
    use std::f32::consts::TAU;
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("radial_hud"),
    ));
    let c = egui::pos2(screen.x, screen.y);
    let accent = egui::Color32::from_rgba_unmultiplied(
        color::ACCENT.r(),
        color::ACCENT.g(),
        color::ACCENT.b(),
        160,
    );
    let amber = egui::Color32::from_rgb(240, 190, 90);
    let dim = egui::Color32::from_rgba_unmultiplied(
        color::LINE.r(),
        color::LINE.g(),
        color::LINE.b(),
        200,
    );
    let inner_r = 58.0_f32;
    let outer_r = 104.0_f32;

    painter.circle_stroke(
        c,
        inner_r,
        egui::Stroke::new(
            if state.active_ring == Ring::Commands {
                2.5
            } else {
                1.0
            },
            if state.active_ring == Ring::Commands {
                accent
            } else {
                dim
            },
        ),
    );
    painter.circle_stroke(
        c,
        outer_r,
        egui::Stroke::new(
            if state.active_ring == Ring::Paths {
                2.5
            } else {
                1.0
            },
            if state.active_ring == Ring::Paths {
                accent
            } else {
                dim
            },
        ),
    );

    // Inner ring: command slots.
    let cmds = command_count().max(1);
    for (i, (_, label)) in ACTIONS.iter().enumerate() {
        let ang = (i as f32 / cmds as f32) * TAU - TAU / 4.0;
        let p = egui::pos2(c.x + ang.cos() * inner_r, c.y + ang.sin() * inner_r);
        let hot = state.active_ring == Ring::Commands && state.cursor == i;
        painter.text(
            p,
            egui::Align2::CENTER_CENTER,
            format!("{} {label}", i + 1),
            egui::FontId::monospace(11.0),
            if hot { amber } else { color::TEXT },
        );
    }

    // Outer ring: neighbour paths for the current page.
    let start = state.path_page * PATHS_PER_PAGE;
    let page: Vec<&NodeId> = neighbors.iter().skip(start).take(PATHS_PER_PAGE).collect();
    let slots = page.len().max(1);
    for (i, nid) in page.iter().enumerate() {
        let ang = (i as f32 / slots as f32) * TAU - TAU / 4.0;
        let p = egui::pos2(c.x + ang.cos() * outer_r, c.y + ang.sin() * outer_r);
        let hot = state.active_ring == Ring::Paths && state.cursor == i;
        let label = st.node_label_with_id(nid);
        painter.text(
            p,
            egui::Align2::CENTER_CENTER,
            format!("{} {}", i + 1, label.chars().take(18).collect::<String>()),
            egui::FontId::monospace(10.0),
            if hot { amber } else { color::TEXT_DIM },
        );
    }

    // Centre identity readout.
    painter.text(
        c,
        egui::Align2::CENTER_CENTER,
        st.node_label_with_id(&state.focused)
            .chars()
            .take(22)
            .collect::<String>(),
        egui::FontId::monospace(11.0),
        color::ACCENT,
    );
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

    // ---- Radial HUD state transitions (spec §3.4) ----

    fn nid(s: &str) -> NodeId {
        NodeId(s.to_string())
    }

    #[test]
    fn radial_open_and_ring_switch() {
        let mut s = RadialState::open(nid("n"));
        assert_eq!(s.active_ring, Ring::Commands);
        assert_eq!(s.cursor, 0);
        s.cursor = 3;
        s.switch_ring();
        assert_eq!(s.active_ring, Ring::Paths);
        assert_eq!(s.cursor, 0, "ring switch resets the cursor");
        s.switch_ring();
        assert_eq!(s.active_ring, Ring::Commands);
    }

    #[test]
    fn radial_rotate_wraps_around() {
        let mut s = RadialState::open(nid("n"));
        s.rotate(-1, 6);
        assert_eq!(s.cursor, 5, "left from 0 wraps to last");
        s.rotate(1, 6);
        assert_eq!(s.cursor, 0, "right wraps back to 0");
        s.rotate(0, 0); // empty ring is safe
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn radial_paging_clamps_to_bounds() {
        let mut s = RadialState::open(nid("n"));
        // 20 paths → 3 pages (0,1,2).
        s.page(1, 20);
        assert_eq!(s.path_page, 1);
        s.page(1, 20);
        assert_eq!(s.path_page, 2);
        s.page(1, 20);
        assert_eq!(s.path_page, 2, "clamped at the last page");
        s.page(-9, 20);
        assert_eq!(s.path_page, 0, "clamped at the first page");
    }

    #[test]
    fn radial_commands_map_to_actions() {
        assert_eq!(command_at(0), Some(CtxAct::Focus));
        assert_eq!(command_at(3), Some(CtxAct::TogglePin));
        assert_eq!(command_count(), 6);
        assert_eq!(command_at(99), None);
    }

    #[test]
    fn radial_path_indexing_by_page() {
        let ns: Vec<NodeId> = (0..12).map(|i| nid(&format!("n{i}"))).collect();
        assert_eq!(path_at(&ns, 0, 0), Some(&nid("n0")));
        assert_eq!(
            path_at(&ns, 1, 0),
            Some(&nid("n9")),
            "page 1 slot 0 = index 9"
        );
        assert_eq!(path_at(&ns, 1, 3), None, "index 12 is out of bounds");
    }

    #[test]
    fn radial_neighbors_are_unique_and_sorted() {
        let mut st = GraphState::default();
        st.load_synthetic_graph(60);
        let id = st
            .model
            .nodes
            .keys()
            .find(|id| st.model.edges_for_node(id).next().is_some())
            .cloned()
            .expect("a connected node");
        let ns = radial_neighbors(&st, &id);
        assert!(!ns.is_empty());
        let mut sorted = ns.clone();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(ns, sorted, "neighbours are sorted (deterministic order)");
        let set: std::collections::HashSet<_> = ns.iter().cloned().collect();
        assert_eq!(set.len(), ns.len(), "neighbours are de-duplicated");
    }

    #[test]
    fn radial_render_does_not_panic() {
        // Exercise the painter with a real standalone egui context (no bevy_egui).
        let (st, id) = state_with_node();
        let state = RadialState::open(id);
        let neighbors = vec![nid("a"), nid("b")];
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            render_radial(
                ctx,
                bevy::math::Vec2::new(120.0, 120.0),
                &state,
                &neighbors,
                &st,
            );
        });
    }

    #[test]
    fn radial_hud_runs_without_panic_headless() {
        use bevy::prelude::*;
        let (st, id) = state_with_node();
        let mut app = App::new();
        app.init_resource::<bevy_egui::EguiUserTextures>()
            .insert_resource(st)
            .insert_resource(RadialMenu(Some(RadialState::open(id))))
            .add_systems(Update, radial_hud);
        app.world_mut()
            .spawn((Camera::default(), GlobalTransform::default()));
        app.update(); // camera + focused node + open radial → no panic (try_ctx_mut None)
    }
}
