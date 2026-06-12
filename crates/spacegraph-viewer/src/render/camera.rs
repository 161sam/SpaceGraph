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
/// and bloom intensity (Minimal = no bloom, flat background).
pub fn sync_visual_theme(
    st: Res<GraphState>,
    mut clear: ResMut<ClearColor>,
    mut bloom_q: Query<&mut BloomSettings>,
) {
    let (bg, bloom) = match st.cfg.visual_theme {
        VisualTheme::Standard => (theme::CLEAR_STANDARD, 0.25_f32),
        VisualTheme::Minimal => (theme::CLEAR_MINIMAL, 0.0),
    };
    if clear.0 != bg {
        clear.0 = bg;
    }
    for mut settings in bloom_q.iter_mut() {
        if (settings.intensity - bloom).abs() > f32::EPSILON {
            settings.intensity = bloom;
        }
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
