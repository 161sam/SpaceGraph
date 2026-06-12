use bevy::core_pipeline::bloom::BloomSettings;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
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

pub fn apply_jump_to(mut st: ResMut<GraphState>, mut cam_q: Query<&mut Transform, With<Camera>>) {
    if st.ui.fit_to_view {
        st.ui.fit_to_view = false;
        if st.ui.view_mode == ViewMode::Tree {
            let vis = st.visible_set_capped();
            let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
            let mut max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
            for id in vis {
                let Some(pos) = st.spatial.position_of(&id) else {
                    continue;
                };
                min.x = min.x.min(pos.x);
                min.y = min.y.min(pos.y);
                min.z = min.z.min(pos.z);
                max.x = max.x.max(pos.x);
                max.y = max.y.max(pos.y);
                max.z = max.z.max(pos.z);
            }

            if min.x.is_finite() {
                let Ok(mut cam_tf) = cam_q.get_single_mut() else {
                    return;
                };
                let center = (min + max) * 0.5;
                let extent = (max.x - min.x).max(max.y - min.y).max(1.0);
                let dist = extent.max(6.0);
                let offset = Vec3::new(dist * 0.6, dist * 0.5, dist * 0.9);
                cam_tf.translation = center + offset;
                cam_tf.look_at(center, Vec3::Y);
            }
        }
    }

    let Some(id) = st.ui.jump_to.take() else {
        return;
    };

    // Jump affects spatial; timeline currently just sets focus/selected
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

    let Ok(mut cam_tf) = cam_q.get_single_mut() else {
        return;
    };

    let current = cam_tf.translation;
    let dist = (current - target).length().max(6.0);
    let offset = Vec3::new(dist * 0.6, dist * 0.5, dist * 0.9);

    cam_tf.translation = target + offset;
    cam_tf.look_at(target, Vec3::Y);
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
