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

// Ring/menu order follows the MP-UI-GitS-polish mockup exactly: fly-to · inspect ·
// trace · isolate · mark · pin (clockwise from the top). `command_at`/the keyboard
// 1–6 mapping read this order directly.
const ACTIONS: [(CtxAct, &str); 6] = [
    (CtxAct::Focus, "Fly-to"),
    (CtxAct::Inspect, "Inspect"),
    (CtxAct::Trace, "Trace connections"),
    (CtxAct::Isolate, "Isolate subgraph"),
    (CtxAct::ToggleMark, "Mark / Unmark"),
    (CtxAct::TogglePin, "Pin / Unpin"),
];

/// Short uppercase verb for a segmented action-ring wedge (the focus HUD), distinct
/// from the descriptive context-menu labels above.
fn ring_label(act: CtxAct) -> &'static str {
    match act {
        CtxAct::Focus => "FLY-TO",
        CtxAct::Inspect => "INSPECT",
        CtxAct::Trace => "TRACE",
        CtxAct::Isolate => "ISOLATE",
        CtxAct::ToggleMark => "MARK",
        CtxAct::TogglePin => "PIN",
    }
}

// ---- Segmented action-ring geometry (focus HUD, pure + unit-tested) ----
/// Inner/outer radius of the action-ring band (screen px around the focused node).
pub const RING_INNER_R: f32 = 46.0;
pub const RING_OUTER_R: f32 = 80.0;
/// Fraction of each `1/count` slot left empty as the divider gap between wedges.
const RING_GAP_FRAC: f32 = 0.16;

/// Centre angle (radians, egui screen space: +x right, +y **down** ⇒ +angle is
/// clockwise) of action segment `slot` of `count`, with slot 0 centred at the top.
pub fn segment_center_angle(slot: usize, count: usize) -> f32 {
    use std::f32::consts::TAU;
    let count = count.max(1);
    -TAU / 4.0 + (slot as f32) * (TAU / count as f32)
}

/// Which action segment a pointer at `p` is over, given the ring centre `c`. Returns
/// `None` outside the band or inside a divider gap. Pure — the hover/hit math the
/// segmented ring asserts.
pub fn segment_at(c: egui::Pos2, p: egui::Pos2, count: usize) -> Option<usize> {
    use std::f32::consts::TAU;
    let count = count.max(1);
    let d = c.distance(p);
    if !(RING_INNER_R..=RING_OUTER_R).contains(&d) {
        return None;
    }
    let ang = (p.y - c.y).atan2(p.x - c.x);
    let step = TAU / count as f32;
    let slot = (((ang - (-TAU / 4.0)) / step)
        .round()
        .rem_euclid(count as f32)) as usize;
    let center = segment_center_angle(slot, count);
    let mut diff = (ang - center).rem_euclid(TAU);
    if diff > TAU / 2.0 {
        diff -= TAU;
    }
    let half_drawn = (step / 2.0) * (1.0 - RING_GAP_FRAC);
    (diff.abs() <= half_drawn).then_some(slot)
}

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
    for edge in st.core.model.edges_for_node(id) {
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

/// Path dive: re-centre Focus Mode on a neighbour (keyboard graph traversal). The
/// camera eases onward and the dim/freeze/edge-cull follow the new subject; the
/// radial re-opens on it. Pure state change (unit-tested).
pub fn dive_to_neighbor(st: &mut GraphState, radial: &mut RadialMenu, nid: NodeId) {
    st.reveal(&nid);
    st.ui.context_menu = None; // keep the radial the sole node-region overlay (P1)
    st.ui.focus = Some(nid.clone());
    st.ui.selected = Some(nid.clone());
    st.ui.focus_mode = Some(nid.clone());
    st.request_jump(nid.clone());
    radial.0 = Some(RadialState::open(nid));
}

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
    if !st.core.model.nodes.contains_key(&state.focused) {
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
        render_radial(ctx, screen, &next, &neighbors);
    }

    match act {
        Do::Cmd(a) => {
            apply_context_action(&mut st, &state.focused, a);
            radial.0 = None;
        }
        Do::Dive(nid) => dive_to_neighbor(&mut st, &mut radial, nid),
        Do::Close => {
            radial.0 = None;
            st.ui.focus_mode = None; // closing the radial exits Focus Mode
            st.needs_redraw.store(true, Ordering::Relaxed);
        }
        Do::None => radial.0 = Some(next),
    }
}

/// Draw the **segmented action ring** (egui painter) around the focused node's
/// projected position: 6 numbered arc-segment wedges evenly at 60°, the
/// keyboard-cursor / pointer-hovered wedge highlighted brighter, a faint inner tick
/// gauge, and — only while the Paths ring is active — faint positional ticks for the
/// page's neighbours (their names live in the entity card, never floating over the
/// node). Replaces the old concentric floating-label rings (the overlap bug).
fn render_radial(
    ctx: &egui::Context,
    screen: bevy::math::Vec2,
    state: &RadialState,
    neighbors: &[NodeId],
) {
    use std::f32::consts::TAU;
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("radial_hud"),
    ));
    let c = egui::pos2(screen.x, screen.y);
    let accent = color::ACCENT;
    let rgba = |a: u8| egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), a);
    let band_dim = rgba(40);
    let band_hot = rgba(140);
    let outline = rgba(190);
    let dim = egui::Color32::from_rgba_unmultiplied(
        color::LINE.r(),
        color::LINE.g(),
        color::LINE.b(),
        170,
    );

    // Backing disc so the wedges + labels read against a busy graph (drawn first).
    painter.circle_filled(
        c,
        RING_OUTER_R + 10.0,
        egui::Color32::from_rgba_unmultiplied(4, 10, 18, 150),
    );

    // Faint inner tick gauge (decorative GitS dial inside the wedge band).
    let gauge_r = RING_INNER_R - 7.0;
    for i in 0..24 {
        let a = (i as f32 / 24.0) * TAU;
        painter.line_segment(
            [
                egui::pos2(
                    c.x + a.cos() * (gauge_r - 2.0),
                    c.y + a.sin() * (gauge_r - 2.0),
                ),
                egui::pos2(c.x + a.cos() * gauge_r, c.y + a.sin() * gauge_r),
            ],
            egui::Stroke::new(1.0, dim),
        );
    }

    // Inner ring: the 6 command slots as segmented arc wedges (even 60°, gapped).
    let count = command_count();
    let step = TAU / count as f32;
    let half = (step / 2.0) * (1.0 - RING_GAP_FRAC);
    let mid_r = (RING_INNER_R + RING_OUTER_R) / 2.0;
    let band_w = RING_OUTER_R - RING_INNER_R - 4.0;
    let hovered = ctx
        .pointer_hover_pos()
        .and_then(|p| segment_at(c, p, count));
    for (slot, (act, _)) in ACTIONS.iter().enumerate() {
        let ca = segment_center_angle(slot, count);
        let hot =
            (state.active_ring == Ring::Commands && state.cursor == slot) || hovered == Some(slot);
        arc_band(
            &painter,
            c,
            mid_r,
            ca - half,
            ca + half,
            band_w,
            if hot { band_hot } else { band_dim },
        );
        if hot {
            arc_band(
                &painter,
                c,
                RING_INNER_R + 2.0,
                ca - half,
                ca + half,
                1.5,
                outline,
            );
            arc_band(
                &painter,
                c,
                RING_OUTER_R - 2.0,
                ca - half,
                ca + half,
                1.5,
                outline,
            );
        }
        painter.text(
            egui::pos2(c.x + ca.cos() * mid_r, c.y + ca.sin() * mid_r),
            egui::Align2::CENTER_CENTER,
            format!("{} {}", slot + 1, ring_label(*act)),
            egui::FontId::monospace(if hot { 11.0 } else { 10.0 }),
            if hot {
                egui::Color32::WHITE
            } else {
                color::TEXT
            },
        );
    }

    // Outer ring: faint positional ticks for the page's neighbours, only while the
    // Paths ring is active (names live in the entity card — no floating labels).
    if state.active_ring == Ring::Paths {
        let start = state.path_page * PATHS_PER_PAGE;
        let page = neighbors.iter().skip(start).take(PATHS_PER_PAGE).count();
        let slots = page.max(1);
        let pr = RING_OUTER_R + 12.0;
        for i in 0..page {
            let a = segment_center_angle(i, slots);
            let (dx, dy) = (a.cos(), a.sin());
            let hot = state.cursor == i;
            painter.line_segment(
                [
                    egui::pos2(c.x + dx * pr, c.y + dy * pr),
                    egui::pos2(c.x + dx * (pr + 9.0), c.y + dy * (pr + 9.0)),
                ],
                egui::Stroke::new(if hot { 2.5 } else { 1.0 }, if hot { accent } else { dim }),
            );
        }
    }
}

/// A thick arc band (the filled-segment look) from `a0` to `a1` at radius `r`.
fn arc_band(
    painter: &egui::Painter,
    c: egui::Pos2,
    r: f32,
    a0: f32,
    a1: f32,
    width: f32,
    color: egui::Color32,
) {
    let steps = 16;
    let pts: Vec<egui::Pos2> = (0..=steps)
        .map(|i| {
            let a = a0 + (a1 - a0) * (i as f32 / steps as f32);
            egui::pos2(c.x + a.cos() * r, c.y + a.sin() * r)
        })
        .collect();
    painter.add(egui::Shape::line(pts, egui::Stroke::new(width, color)));
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
        st.core.model.nodes.insert(id.clone(), process_node());
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
        // MP/mockup order: fly-to · inspect · trace · isolate · mark · pin.
        assert_eq!(command_at(0), Some(CtxAct::Focus));
        assert_eq!(command_at(1), Some(CtxAct::Inspect));
        assert_eq!(command_at(2), Some(CtxAct::Trace));
        assert_eq!(command_at(3), Some(CtxAct::Isolate));
        assert_eq!(command_at(4), Some(CtxAct::ToggleMark));
        assert_eq!(command_at(5), Some(CtxAct::TogglePin));
        assert_eq!(command_count(), 6);
        assert_eq!(command_at(99), None);
    }

    #[test]
    fn segment_centers_are_evenly_spaced_from_the_top() {
        use std::f32::consts::TAU;
        let n = command_count();
        assert!(
            (segment_center_angle(0, n) - (-TAU / 4.0)).abs() < 1e-5,
            "slot 0 sits at the top"
        );
        let step = TAU / n as f32;
        for s in 1..n {
            let d = segment_center_angle(s, n) - segment_center_angle(s - 1, n);
            assert!(
                (d - step).abs() < 1e-5,
                "slot {s} is one even step from its predecessor"
            );
        }
    }

    #[test]
    fn segment_at_hits_centres_and_misses_holes_and_gaps() {
        use std::f32::consts::TAU;
        let c = egui::pos2(0.0, 0.0);
        let n = command_count();
        let mid_r = (RING_INNER_R + RING_OUTER_R) / 2.0;
        for s in 0..n {
            let a = segment_center_angle(s, n);
            let p = egui::pos2(c.x + a.cos() * mid_r, c.y + a.sin() * mid_r);
            assert_eq!(
                segment_at(c, p, n),
                Some(s),
                "the centre of wedge {s} hits it"
            );
        }
        // Inner hole and beyond the outer radius are not on the ring.
        assert_eq!(segment_at(c, egui::pos2(1.0, 0.0), n), None);
        assert_eq!(segment_at(c, egui::pos2(500.0, 0.0), n), None);
        // The divider gap between two wedges is not clickable.
        let gap = segment_center_angle(0, n) + (TAU / n as f32) / 2.0;
        let p = egui::pos2(c.x + gap.cos() * mid_r, c.y + gap.sin() * mid_r);
        assert_eq!(segment_at(c, p, n), None, "divider gaps are inert");
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
            .core
            .model
            .nodes
            .keys()
            .find(|id| st.core.model.edges_for_node(id).next().is_some())
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
    fn path_dive_recentres_focus_on_neighbor() {
        let (mut st, _id) = state_with_node();
        let nb = nid("nb");
        st.core.model.nodes.insert(nb.clone(), process_node());
        let idx = st.spatial.intern(&nb);
        st.spatial.set_position(idx, Vec3::ZERO);
        let mut radial = RadialMenu(Some(RadialState::open(nid("n"))));

        dive_to_neighbor(&mut st, &mut radial, nb.clone());

        assert_eq!(
            st.ui.focus_mode.as_ref(),
            Some(&nb),
            "path dive re-centres Focus Mode on the neighbour"
        );
        assert_eq!(st.ui.focus.as_ref(), Some(&nb));
        assert_eq!(st.ui.jump_to.as_ref(), Some(&nb), "camera eases onward");
        assert_eq!(
            radial.0.as_ref().map(|s| &s.focused),
            Some(&nb),
            "the radial re-opens on the neighbour"
        );
    }

    #[test]
    fn radial_render_does_not_panic() {
        // Exercise the painter with a real standalone egui context (no bevy_egui),
        // covering both the Commands wedge ring and the Paths tick ring.
        let (_st, id) = state_with_node();
        let mut state = RadialState::open(id);
        let neighbors = vec![nid("a"), nid("b"), nid("c")];
        for ring in [Ring::Commands, Ring::Paths] {
            state.active_ring = ring;
            let ctx = egui::Context::default();
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                render_radial(ctx, bevy::math::Vec2::new(120.0, 120.0), &state, &neighbors);
            });
        }
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
