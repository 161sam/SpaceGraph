//! Node interactivity & focused-node effects (v0.4.1) — cheap, **focused-only**
//! polish layered on top of the existing pick/focus glue. Nothing here scales
//! with the visible set:
//!
//! - **Focus ripple:** a decaying ring spawned when the focused node changes
//!   (spawn → expand+fade → despawn), capped to a handful of concurrent entities.
//! - **Preview expand:** a double-click on a node toggles the larger preview
//!   (read by `ui::node_preview`), collapsing when focus clears.
//!
//! Standard theme only (visual; determinism-exempt). The preview open/close glue
//! itself lives in `ui::node_preview` (focus/hover/pin drive the preview set).

use bevy::color::Alpha;
use bevy::prelude::*;
use spacegraph_core::NodeId;

use crate::app::events::Picked;
use crate::graph::GraphState;
use crate::render::theme;
use crate::util::config::VisualTheme;

const RIPPLE_TTL: f32 = 0.6;
const RIPPLE_START: f32 = 0.35;
const RIPPLE_END: f32 = 2.4;
const RIPPLE_GLOW: f32 = 6.0;
/// Max concurrent focus ripples — focused-only, never per-visible-node.
const MAX_RIPPLES: usize = 4;
/// Double-click window for the preview-expand toggle.
const DOUBLE_CLICK_SECS: f64 = 0.35;

/// A short-lived, expanding+fading focus ring.
#[derive(Component)]
pub struct FocusRipple {
    pub age: f32,
    pub ttl: f32,
    tint: LinearRgba,
}

/// Tracks the last focused node so a ripple spawns only on *change* (no churn).
#[derive(Resource, Default)]
pub struct RippleTracker {
    pub last_focus: Option<NodeId>,
}

/// Shared ripple ring mesh (one handle, reused by every ripple).
#[derive(Resource)]
pub struct RippleResources {
    pub mesh: Handle<Mesh>,
}

/// Whether the focused preview is expanded (toggled by a node double-click).
#[derive(Resource, Default)]
pub struct PreviewExpand {
    pub expanded: bool,
    last_click: Option<(NodeId, f64)>,
}

pub fn setup_ripple_resources(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let mesh = meshes.add(Mesh::from(bevy::math::primitives::Torus::new(0.03, 0.5)));
    commands.insert_resource(RippleResources { mesh });
}

/// Spawn a decaying ripple when the focused node changes (Standard theme only).
pub fn trigger_focus_ripple(
    mut commands: Commands,
    st: Res<GraphState>,
    res: Res<RippleResources>,
    mut tracker: ResMut<RippleTracker>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    existing: Query<(), With<FocusRipple>>,
) {
    if st.cfg.visual_theme != VisualTheme::Standard {
        tracker.last_focus = None;
        return;
    }
    let focus = st.ui.selected.clone().or_else(|| st.ui.focus.clone());
    if focus == tracker.last_focus {
        return; // no change → no spawn
    }
    tracker.last_focus = focus.clone();

    let Some(id) = focus else {
        return;
    };
    let Some(idx) = st.spatial.index_of(&id) else {
        return;
    };
    if !st.spatial.placed[idx.slot()] || existing.iter().count() >= MAX_RIPPLES {
        return;
    }
    let pos = st.spatial.positions[idx.slot()];
    let color = st
        .model
        .nodes
        .get(&id)
        .map(theme::NodeKind::of)
        .unwrap_or(theme::NodeKind::File)
        .base_color();
    let tint = color.to_linear();
    let material = mats.add(StandardMaterial {
        base_color: color.with_alpha(0.9),
        emissive: LinearRgba::rgb(
            tint.red * RIPPLE_GLOW,
            tint.green * RIPPLE_GLOW,
            tint.blue * RIPPLE_GLOW,
        ),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    commands.spawn((
        PbrBundle {
            mesh: res.mesh.clone(),
            material,
            transform: Transform::from_translation(pos).with_scale(Vec3::splat(RIPPLE_START)),
            ..default()
        },
        FocusRipple {
            age: 0.0,
            ttl: RIPPLE_TTL,
            tint,
        },
    ));
}

/// Advance ripples: expand + fade, despawn at end of life (frees the material).
/// Keeps the reactive renderer awake while a ripple is alive.
pub fn update_focus_ripples(
    mut commands: Commands,
    time: Res<Time>,
    st: Res<GraphState>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut q: Query<(
        Entity,
        &mut FocusRipple,
        &mut Transform,
        &Handle<StandardMaterial>,
    )>,
) {
    if !q.is_empty() {
        st.needs_redraw
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let dt = time.delta_seconds();
    for (e, mut r, mut tf, mat) in &mut q {
        r.age += dt;
        let f = (r.age / r.ttl).clamp(0.0, 1.0);
        tf.scale = Vec3::splat(RIPPLE_START + f * (RIPPLE_END - RIPPLE_START));
        let a = 1.0 - f;
        if let Some(m) = mats.get_mut(mat) {
            m.base_color = m.base_color.with_alpha(0.9 * a);
            m.emissive = LinearRgba::rgb(
                r.tint.red * RIPPLE_GLOW * a,
                r.tint.green * RIPPLE_GLOW * a,
                r.tint.blue * RIPPLE_GLOW * a,
            );
        }
        if r.age >= r.ttl {
            commands.entity(e).despawn();
        }
    }
}

/// Toggle the expanded preview on a node double-click; collapse when focus clears.
pub fn detect_preview_expand(
    time: Res<Time>,
    mut ev: EventReader<Picked>,
    mut expand: ResMut<PreviewExpand>,
    st: Res<GraphState>,
) {
    let now = time.elapsed_seconds_f64();
    for Picked(id) in ev.read() {
        let is_double = expand
            .last_click
            .as_ref()
            .is_some_and(|(lid, lt)| lid == id && now - lt < DOUBLE_CLICK_SECS);
        if is_double {
            expand.expanded = !expand.expanded;
        }
        expand.last_click = Some((id.clone(), now));
    }
    if st.ui.selected.is_none() && st.ui.focus.is_none() {
        expand.expanded = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::GraphState;

    fn focused_state() -> (GraphState, NodeId) {
        let mut gs = GraphState::default();
        gs.cfg.max_visible_nodes = 64;
        gs.cfg.progressive_nodes_per_frame = 64;
        gs.load_synthetic_graph(40);
        let vis = gs.visible_set_capped();
        gs.progressive_prepare(&vis);
        gs.spatial.vis_cache = vis;
        let id = gs
            .spatial
            .vis_cache
            .iter()
            .find(|id| {
                gs.spatial
                    .index_of(id)
                    .map(|i| gs.spatial.placed[i.slot()])
                    .unwrap_or(false)
            })
            .cloned()
            .expect("a placed node");
        gs.ui.selected = Some(id.clone());
        (gs, id)
    }

    fn ripple_app(gs: GraphState) -> App {
        let mut app = App::new();
        app.add_plugins((bevy::MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<StandardMaterial>()
            .insert_resource(gs)
            .insert_resource(RippleResources {
                mesh: Handle::weak_from_u128(1),
            })
            .insert_resource(RippleTracker::default());
        app
    }

    fn ripple_count(app: &mut App) -> usize {
        let mut q = app.world_mut().query::<&FocusRipple>();
        q.iter(app.world()).count()
    }

    #[test]
    fn ripple_spawns_once_per_focus_change() {
        let (gs, _) = focused_state();
        let mut app = ripple_app(gs);
        app.add_systems(Update, trigger_focus_ripple);
        app.update();
        assert_eq!(ripple_count(&mut app), 1, "focus change spawns one ripple");
        // Focus unchanged → no further spawns (churn-free).
        app.update();
        assert_eq!(ripple_count(&mut app), 1, "stable focus must not respawn");
    }

    #[test]
    fn ripple_decays_and_despawns() {
        let (gs, _) = focused_state();
        let mut app = ripple_app(gs);
        app.add_systems(Update, trigger_focus_ripple);
        app.update();
        assert_eq!(ripple_count(&mut app), 1);

        // Force the ripple past its lifetime, then advance the decay system.
        {
            let mut q = app.world_mut().query::<&mut FocusRipple>();
            for mut r in q.iter_mut(app.world_mut()) {
                r.age = r.ttl + 1.0;
            }
        }
        app.add_systems(Update, update_focus_ripples);
        app.update();
        assert_eq!(ripple_count(&mut app), 0, "ripple despawns at end of life");
    }

    #[test]
    fn no_ripple_in_minimal_theme() {
        let (mut gs, _) = focused_state();
        gs.cfg.visual_theme = VisualTheme::Minimal;
        let mut app = ripple_app(gs);
        app.add_systems(Update, trigger_focus_ripple);
        app.update();
        assert_eq!(ripple_count(&mut app), 0, "Minimal draws no ripple");
    }

    #[test]
    fn double_click_toggles_expand_and_focus_clear_collapses() {
        let (gs, id) = focused_state();
        let mut app = App::new();
        app.add_plugins(bevy::MinimalPlugins)
            .add_event::<Picked>()
            .insert_resource(gs)
            .insert_resource(PreviewExpand::default())
            .add_systems(Update, detect_preview_expand);

        // First click: arm. Second click on the same node within the window: toggle.
        app.world_mut().send_event(Picked(id.clone()));
        app.update();
        assert!(!app.world().resource::<PreviewExpand>().expanded);
        app.world_mut().send_event(Picked(id.clone()));
        app.update();
        assert!(
            app.world().resource::<PreviewExpand>().expanded,
            "double-click expands"
        );

        // Clearing focus collapses the expanded preview.
        {
            let mut st = app.world_mut().resource_mut::<GraphState>();
            st.ui.selected = None;
            st.ui.focus = None;
        }
        app.update();
        assert!(!app.world().resource::<PreviewExpand>().expanded);
    }
}
