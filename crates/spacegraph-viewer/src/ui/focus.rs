//! Focus Mode (v0.5.1) — the headline. Composes the pieces already built into one
//! cinematic mode: the camera eases the node to screen-centre + close
//! (`request_jump`), the rest of the graph **dims on all tiers**, the force layout
//! **freezes** (FM-2, `GraphState::layout_frozen`), and the node becomes the
//! foreground centerpiece — the radial command HUD rings (`ui::context_menu`), the
//! v0.4.1 preview rendered as the centre (`ui::node_preview`, re-anchored), and the
//! identity arcs drawn here. Enter with `F` / double-click, exit with `Esc`; a path
//! dive re-centres focus on a neighbour. Minimal theme degrades to a plain
//! dim+centre (no rings/arcs/DoF). The High-tier DoF blur is deferred (dim-only).
//!
//! Tier-independent + determinism-exempt; **O(1)** (one focused node + one dim
//! rect), never per-visible-node — no entities spawned here.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use spacegraph_core::NodeId;
use std::sync::atomic::Ordering;

use crate::app::events::Picked;
use crate::graph::GraphState;
use crate::render::theme;
use crate::ui::context_menu::{RadialMenu, RadialState};
use crate::ui::tokens::color;
use crate::util::config::VisualTheme;

/// Double-click window (seconds) for entering Focus Mode with the mouse.
const FOCUS_DOUBLE_CLICK_SECS: f64 = 0.35;

/// Enter Focus Mode on `id`: set the subject (drives the dim + layout freeze +
/// edge cull), select it (fires the dive ripple on the focus change), and ease the
/// camera to centre it. The keyboard radial command HUD is the in-focus interaction
/// in the Standard theme; Minimal focus is a plain dim+centre spotlight (no rings).
pub fn enter_focus(st: &mut GraphState, radial: &mut RadialMenu, id: NodeId) {
    // Mutual exclusion (P1): entering focus closes the transient overlays that
    // would otherwise stack on the focused node (right-click menu, palette,
    // search). The radial HUD is the in-focus interaction.
    st.ui.context_menu = None;
    st.ui.palette_open = false;
    st.ui.search_open = false;
    st.ui.focus_mode = Some(id.clone());
    st.ui.selected = Some(id.clone());
    st.request_jump(id.clone());
    radial.0 = if st.cfg.visual_theme == VisualTheme::Standard {
        Some(RadialState::open(id))
    } else {
        None
    };
    st.needs_redraw.store(true, Ordering::Relaxed);
}

/// Exit Focus Mode (the eased camera return is handled by `render::focus_mode_camera`).
pub fn exit_focus(st: &mut GraphState, radial: &mut RadialMenu) {
    st.ui.focus_mode = None;
    radial.0 = None;
    st.needs_redraw.store(true, Ordering::Relaxed);
}

/// Mouse entry: a double-click on a node enters Focus Mode (`F` is the keyboard
/// entry, handled in `ui::shortcuts`).
pub fn focus_double_click(
    time: Res<Time>,
    mut ev: EventReader<Picked>,
    mut st: ResMut<GraphState>,
    mut radial: ResMut<RadialMenu>,
    mut last: Local<Option<(NodeId, f64)>>,
) {
    let now = time.elapsed_seconds_f64();
    for Picked(id) in ev.read() {
        let is_double = last
            .as_ref()
            .is_some_and(|(lid, lt)| lid == id && now - lt < FOCUS_DOUBLE_CLICK_SECS);
        if is_double {
            enter_focus(&mut st, &mut radial, id.clone());
        }
        *last = Some((id.clone(), now));
    }
}

/// Background dim (FM-1, **all tiers**) + the Standard-theme node centerpiece
/// (prominent ring + identity arcs). Painted with `try_ctx_mut` so it is panic-free
/// headless. O(1): one screen rect + a handful of arc labels for the focused node.
pub fn focus_overlay(
    mut contexts: EguiContexts,
    st: Res<GraphState>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
) {
    let Some(focus_id) = st.ui.focus_mode.clone() else {
        return;
    };
    let centre_world = st.spatial.position_of(&focus_id);
    let label = st.node_label_with_id(&focus_id);
    let degree = st.core.model.degree(&focus_id);
    let kind = st.core.model.nodes.get(&focus_id).map(theme::NodeKind::of);
    let standard = st.cfg.visual_theme == VisualTheme::Standard;
    let dim = (st.cfg.focus.dim.clamp(0.0, 0.95) * 255.0) as u8;

    // Project the focused node to screen (≈ centre once the camera has centred it).
    let projected = cam_q
        .get_single()
        .ok()
        .zip(centre_world)
        .and_then(|((cam, tf), p)| cam.world_to_viewport(tf, p));

    let Some(ctx) = contexts.try_ctx_mut() else {
        return; // headless / no egui context yet
    };
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("focus_overlay"),
    ));
    // FM-1: dim the whole graph on every tier (DoF is High-only, deferred to a
    // later pass — see RUNLOG). The radial HUD + preview draw on higher layers.
    painter.rect_filled(
        screen,
        0.0,
        egui::Color32::from_rgba_unmultiplied(2, 6, 12, dim),
    );

    if !standard {
        return; // Minimal → plain dim + centre (no rings/arcs/DoF)
    }
    let c = projected
        .map(|v| egui::pos2(v.x, v.y))
        .unwrap_or_else(|| screen.center());
    draw_centerpiece(&painter, c, &label, degree, kind);
}

/// The focused-node **labels** (Standard): the radial kind / connections /
/// identity readout + a FOCUS tag around the node. The *geometry* of the layered
/// core (rings + wireframe shell + pulsing pip) is now a real depth-3D mesh rig
/// (`render::focus_core`, P5); this 2D pass only carries the legible labels so it
/// never double-draws the 3D rings. Static (no per-frame egui animation).
fn draw_centerpiece(
    painter: &egui::Painter,
    c: egui::Pos2,
    label: &str,
    degree: usize,
    kind: Option<theme::NodeKind>,
) {
    let accent = color::ACCENT;
    // Label ring radius: sits just outside the 3D core's screen footprint.
    let r = 150.0_f32;

    let kind_name = kind.map(kind_label).unwrap_or("node");
    // Focus tag above the kind.
    painter.text(
        egui::pos2(c.x, c.y - r - 30.0),
        egui::Align2::CENTER_BOTTOM,
        "◤ FOCUS ◥",
        egui::FontId::monospace(11.0),
        accent,
    );
    // Top arc: kind / status.
    painter.text(
        egui::pos2(c.x, c.y - r - 14.0),
        egui::Align2::CENTER_BOTTOM,
        format!("◢ {kind_name} ◣"),
        egui::FontId::monospace(12.0),
        accent,
    );
    // Right arc: connection count.
    painter.text(
        egui::pos2(c.x + r + 12.0, c.y),
        egui::Align2::LEFT_CENTER,
        format!("links {degree}"),
        egui::FontId::monospace(11.0),
        color::TEXT_DIM,
    );
    // Bottom arc: identity (uid/pid/path via the long label).
    painter.text(
        egui::pos2(c.x, c.y + r + 14.0),
        egui::Align2::CENTER_TOP,
        label.chars().take(40).collect::<String>(),
        egui::FontId::monospace(11.0),
        color::TEXT,
    );
}

fn kind_label(kind: theme::NodeKind) -> &'static str {
    match kind {
        theme::NodeKind::Process => "process",
        theme::NodeKind::File => "file",
        theme::NodeKind::User => "user",
        theme::NodeKind::Socket => "socket",
        theme::NodeKind::RemoteHost => "host",
        theme::NodeKind::Alert => "alert",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spacegraph_core::Node;

    fn state_with_node(theme: VisualTheme) -> (GraphState, NodeId) {
        let mut st = GraphState::default();
        st.cfg.visual_theme = theme;
        let id = NodeId("n".to_string());
        st.core.model.nodes.insert(
            id.clone(),
            Node::Process {
                pid: 1,
                ppid: 0,
                exe: "x".into(),
                cmdline: "x".into(),
                uid: 0,
            },
        );
        let idx = st.spatial.intern(&id);
        st.spatial.set_position(idx, Vec3::ZERO);
        (st, id)
    }

    #[test]
    fn enter_focus_sets_subject_and_opens_radial_in_standard() {
        let (mut st, id) = state_with_node(VisualTheme::Standard);
        let mut radial = RadialMenu::default();
        enter_focus(&mut st, &mut radial, id.clone());
        assert_eq!(st.ui.focus_mode.as_ref(), Some(&id));
        assert_eq!(st.ui.selected.as_ref(), Some(&id));
        assert_eq!(st.ui.jump_to.as_ref(), Some(&id), "camera jump requested");
        assert!(radial.0.is_some(), "Standard focus opens the radial HUD");
    }

    #[test]
    fn enter_focus_minimal_dims_without_radial() {
        let (mut st, id) = state_with_node(VisualTheme::Minimal);
        let mut radial = RadialMenu::default();
        enter_focus(&mut st, &mut radial, id.clone());
        assert_eq!(st.ui.focus_mode.as_ref(), Some(&id));
        assert!(
            radial.0.is_none(),
            "Minimal focus is plain dim+centre (no rings)"
        );
    }

    #[test]
    fn exit_focus_clears_subject_and_radial() {
        let (mut st, id) = state_with_node(VisualTheme::Standard);
        let mut radial = RadialMenu::default();
        enter_focus(&mut st, &mut radial, id);
        exit_focus(&mut st, &mut radial);
        assert!(st.ui.focus_mode.is_none());
        assert!(radial.0.is_none());
    }

    #[test]
    fn double_click_enters_focus() {
        let (st, id) = state_with_node(VisualTheme::Standard);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_event::<Picked>()
            .insert_resource(st)
            .insert_resource(RadialMenu::default())
            .add_systems(Update, focus_double_click);
        // First click arms; the second within the window enters focus.
        app.world_mut().send_event(Picked(id.clone()));
        app.update();
        assert!(app.world().resource::<GraphState>().ui.focus_mode.is_none());
        app.world_mut().send_event(Picked(id.clone()));
        app.update();
        assert_eq!(
            app.world().resource::<GraphState>().ui.focus_mode.as_ref(),
            Some(&id)
        );
    }

    #[test]
    fn focus_mode_spawns_no_per_node_entities() {
        // Structural: Focus Mode is O(1). `focus_overlay` paints egui (it has no
        // `Commands`) and cannot spawn entities — so entering focus adds no
        // per-visible-node cost regardless of graph size.
        let (mut st, id) = state_with_node(VisualTheme::Standard);
        st.ui.focus_mode = Some(id);
        let mut app = App::new();
        app.init_resource::<bevy_egui::EguiUserTextures>()
            .insert_resource(st)
            .add_systems(Update, focus_overlay);
        app.world_mut()
            .spawn((Camera::default(), GlobalTransform::default()));
        let before = app.world_mut().query::<Entity>().iter(app.world()).count();
        app.update();
        let after = app.world_mut().query::<Entity>().iter(app.world()).count();
        assert_eq!(before, after, "focus overlay must not spawn entities");
    }

    #[test]
    fn focus_overlay_runs_without_panic_headless() {
        let (mut st, id) = state_with_node(VisualTheme::Standard);
        st.ui.focus_mode = Some(id);
        let mut app = App::new();
        app.init_resource::<bevy_egui::EguiUserTextures>()
            .insert_resource(st)
            .add_systems(Update, focus_overlay);
        app.world_mut()
            .spawn((Camera::default(), GlobalTransform::default()));
        app.update(); // try_ctx_mut None headless → no panic
    }
}
