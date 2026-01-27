use bevy::prelude::*;
use std::sync::atomic::Ordering;

use crate::graph::{GraphState, ViewMode};

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

    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 18.0, 28.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
}

pub fn apply_jump_to(mut st: ResMut<GraphState>, mut cam_q: Query<&mut Transform, With<Camera>>) {
    let Some(id) = st.ui.jump_to.take() else {
        return;
    };

    // Jump affects spatial; timeline currently just sets focus/selected
    if st.ui.view_mode != ViewMode::Spatial {
        st.ui.focus = Some(id);
        st.ui.selected = Some(id);
        return;
    }

    let Some(target) = st.spatial.positions.get(&id).cloned() else {
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
