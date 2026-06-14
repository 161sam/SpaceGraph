use bevy::core_pipeline::bloom::BloomSettings;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;
use std::sync::atomic::Ordering;

use crate::graph::{GraphState, ViewMode};
use crate::render::theme;
use crate::util::config::VisualTheme;

pub fn setup_scene(mut commands: Commands) {
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 5000.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(10.0, 20.0, 10.0),
        ..default()
    });

    // HDR camera with bloom so emissive (neon) elements glow. Bloom intensity
    // is themed at runtime by `sync_visual_theme` (0 in the Minimal theme).
    //
    // Navigation: LMB = select (picking), RMB-drag = orbit, MMB-drag = pan,
    // scroll = zoom. PanOrbitCamera initialises orbit from this transform and
    // skips input while egui wants the pointer (bevy_egui feature).
    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                hdr: true,
                ..default()
            },
            tonemapping: Tonemapping::TonyMcMapface,
            transform: Transform::from_xyz(0.0, 18.0, 28.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        BloomSettings::NATURAL,
        PanOrbitCamera {
            button_orbit: MouseButton::Right,
            button_pan: MouseButton::Middle,
            ..default()
        },
    ));
}

/// Apply the active visual theme to scene-wide settings: background clear colour
/// (bloom intensity is owned by `render::quality::apply_quality`, which folds the
/// theme into the tier gates).
pub fn sync_visual_theme(
    st: Res<GraphState>,
    mut clear: ResMut<ClearColor>,
    mut rebuild: ResMut<crate::render::RebuildNodeEntities>,
    mut last_theme: Local<Option<VisualTheme>>,
) {
    let bg = match st.cfg.visual_theme {
        VisualTheme::Standard => theme::CLEAR_STANDARD,
        VisualTheme::Minimal => theme::CLEAR_MINIMAL,
    };
    if clear.0 != bg {
        clear.0 = bg;
    }
    // On an actual theme change (not first run), trigger one node-entity rebuild
    // so cores/shells switch between per-kind geometry and the flat sphere.
    if *last_theme != Some(st.cfg.visual_theme) {
        if last_theme.is_some() {
            rebuild.0 = true;
        }
        *last_theme = Some(st.cfg.visual_theme);
    }
}

pub fn apply_jump_to(mut st: ResMut<GraphState>, mut cam_q: Query<&mut PanOrbitCamera>) {
    // Fit-to-view (tree mode): frame the whole visible subgraph.
    if st.ui.fit_to_view {
        st.ui.fit_to_view = false;
        if st.ui.view_mode == ViewMode::Tree {
            let vis = st.visible_set_capped();
            let mut min = Vec3::splat(f32::INFINITY);
            let mut max = Vec3::splat(f32::NEG_INFINITY);
            for id in &vis {
                if let Some(pos) = st.spatial.position_of(id) {
                    min = min.min(pos);
                    max = max.max(pos);
                }
            }
            if min.x.is_finite() {
                if let Ok(mut pan) = cam_q.get_single_mut() {
                    let center = (min + max) * 0.5;
                    let extent = (max.x - min.x).max(max.y - min.y).max(max.z - min.z);
                    pan.target_focus = center;
                    pan.target_radius = (extent * 1.1).max(6.0);
                }
            }
        }
    }

    let Some(id) = st.ui.jump_to.take() else {
        return;
    };

    // Timeline jump only sets focus/selection (no spatial camera move).
    if st.ui.view_mode == ViewMode::Timeline {
        st.ui.focus = Some(id.clone());
        st.ui.selected = Some(id);
        return;
    }

    let Some(target) = st.spatial.position_of(&id) else {
        return;
    };
    st.ui.focus = Some(id.clone());
    st.ui.selected = Some(id);
    st.needs_redraw.store(true, Ordering::Relaxed);

    // Lock-on: ease the orbit pivot to the target and pull to a close radius.
    // PanOrbitCamera interpolates focus/radius → smooth "fly-to" transition.
    if let Ok(mut pan) = cam_q.get_single_mut() {
        pan.target_focus = target;
        let r = pan.radius.unwrap_or(pan.target_radius);
        pan.target_radius = r.clamp(6.0, 18.0);
    }
}

/// Camera-restore state for Focus Mode: the pan-orbit framing the camera eases
/// back to when focus exits. Captured on enter, applied on exit — one-time edges,
/// no per-frame cost.
#[derive(Resource, Default)]
pub struct FocusCam {
    active: bool,
    /// `(target_focus, target_radius)` to ease back to on exit.
    restore: Option<(Vec3, f32)>,
}

/// Focus Mode camera (v0.5.1): the **enter** dive is performed by `apply_jump_to`
/// via `request_jump`; this system handles the **eased exit** — it captures the
/// pre-focus framing on the enter edge and restores it on the exit edge, letting
/// `PanOrbitCamera` interpolate the orbit pivot + radius back. Edge-detected from
/// `ui.focus_mode`, so it runs O(1) on transitions only.
pub fn focus_mode_camera(
    st: Res<GraphState>,
    mut fc: ResMut<FocusCam>,
    mut cam_q: Query<&mut PanOrbitCamera>,
) {
    let now_active = st.ui.focus_mode.is_some();
    if now_active && !fc.active {
        // Entered: capture the framing to ease back to on exit.
        if let Some(pan) = cam_q.iter().next() {
            fc.restore = Some((pan.target_focus, pan.target_radius));
        }
    } else if !now_active && fc.active {
        // Exited: ease the orbit pivot + radius back (PanOrbitCamera interpolates).
        if let (Ok(mut pan), Some((focus, radius))) = (cam_q.get_single_mut(), fc.restore.take()) {
            pan.target_focus = focus;
            pan.target_radius = radius;
            st.needs_redraw.store(true, Ordering::Relaxed);
        }
    }
    fc.active = now_active;
}

pub fn update_tree_zoom(cam_q: Query<&Transform, With<Camera>>, mut st: ResMut<GraphState>) {
    if st.ui.view_mode != ViewMode::Tree {
        return;
    }
    let Ok(cam_tf) = cam_q.get_single() else {
        return;
    };
    let dist = cam_tf.translation.distance(st.ui.tree_center).max(1.0);
    st.ui.tree_zoom = 1.0 / dist;
}

#[cfg(test)]
mod tests {
    use super::*;
    use spacegraph_core::NodeId;

    #[test]
    fn focus_camera_captures_then_restores_framing() {
        let mut app = App::new();
        app.insert_resource(GraphState::default())
            .insert_resource(FocusCam::default())
            .add_systems(Update, focus_mode_camera);
        let before = Vec3::new(1.0, 2.0, 3.0);
        let cam = app
            .world_mut()
            .spawn(PanOrbitCamera {
                target_focus: before,
                target_radius: 25.0,
                ..default()
            })
            .id();

        // Enter Focus Mode → capture the pre-focus framing.
        app.world_mut().resource_mut::<GraphState>().ui.focus_mode = Some(NodeId("n".into()));
        app.update();
        // Simulate the dive pulling the camera onto the node.
        {
            let mut pan = app.world_mut().get_mut::<PanOrbitCamera>(cam).unwrap();
            pan.target_focus = Vec3::new(9.0, 9.0, 9.0);
            pan.target_radius = 8.0;
        }
        // Exit Focus Mode → eased restore to the captured framing.
        app.world_mut().resource_mut::<GraphState>().ui.focus_mode = None;
        app.update();
        let pan = app.world().get::<PanOrbitCamera>(cam).unwrap();
        assert_eq!(
            pan.target_focus, before,
            "camera eases back to the pre-focus pivot"
        );
        assert_eq!(pan.target_radius, 25.0);
    }
}
