//! Free-fly "pilot" camera mode — fly through the graph like a space sim.
//!
//! Toggled with `V`. While active the orbit camera (`PanOrbitCamera`) is
//! disabled and the cursor is grabbed: mouse moves look, WASD + Q/E move, Shift
//! boosts. `Esc` (or `V` again) exits and hands control back to the orbit
//! camera, re-synced to the current pose so there's no jump.

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use bevy_egui::EguiContexts;
use bevy_panorbit_camera::PanOrbitCamera;

use crate::graph::GraphState;

/// Free-fly state (inactive by default → orbit camera is in control).
#[derive(Resource, Default)]
pub struct FlyCam {
    pub active: bool,
    pub yaw: f32,
    pub pitch: f32,
}

#[allow(clippy::too_many_arguments)]
pub fn fly_camera(
    mut fly: ResMut<FlyCam>,
    keys: Res<ButtonInput<KeyCode>>,
    mut motion: EventReader<MouseMotion>,
    time: Res<Time>,
    mut contexts: EguiContexts,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut cam_q: Query<(&mut Transform, &mut PanOrbitCamera), With<Camera>>,
    st: Res<GraphState>,
) {
    let egui_keyboard = contexts.ctx_mut().wants_keyboard_input();
    let Ok((mut tf, mut pan)) = cam_q.get_single_mut() else {
        return;
    };

    // Toggle on V (ignore while typing in egui). Esc also exits.
    let toggle = !egui_keyboard && keys.just_pressed(KeyCode::KeyV);
    let exit = fly.active && keys.just_pressed(KeyCode::Escape);
    if toggle || exit {
        let activate = toggle && !fly.active;
        if activate {
            fly.active = true;
            pan.enabled = false;
            // Seed yaw/pitch from the current camera orientation.
            let (y, p, _) = tf.rotation.to_euler(EulerRot::YXZ);
            fly.yaw = y;
            fly.pitch = p;
            grab_cursor(&mut windows, true);
        } else {
            // Deactivate: hand back to orbit, re-synced so it doesn't snap.
            fly.active = false;
            let radius = pan.radius.unwrap_or(pan.target_radius.max(6.0));
            let focus = tf.translation + tf.forward() * radius;
            pan.focus = focus;
            pan.target_focus = focus;
            pan.radius = Some(radius);
            pan.target_radius = radius;
            pan.yaw = Some(fly.yaw);
            pan.target_yaw = fly.yaw;
            pan.pitch = Some(fly.pitch);
            pan.target_pitch = fly.pitch;
            pan.enabled = true;
            grab_cursor(&mut windows, false);
        }
    }

    if !fly.active {
        return;
    }

    // Mouse look (skip while egui owns the pointer, e.g. a panel is hovered).
    if !contexts.ctx_mut().wants_pointer_input() {
        let mut delta = Vec2::ZERO;
        for ev in motion.read() {
            delta += ev.delta;
        }
        fly.yaw -= delta.x * st.cfg.fly_sensitivity;
        fly.pitch = (fly.pitch - delta.y * st.cfg.fly_sensitivity).clamp(
            -std::f32::consts::FRAC_PI_2 + 0.01,
            std::f32::consts::FRAC_PI_2 - 0.01,
        );
    }
    tf.rotation = Quat::from_euler(EulerRot::YXZ, fly.yaw, fly.pitch, 0.0);

    // Movement along the camera basis.
    let dir = fly_move_dir(&keys, &tf);
    if dir != Vec3::ZERO {
        let boost = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            st.cfg.fly_boost
        } else {
            1.0
        };
        tf.translation += dir.normalize() * st.cfg.fly_speed * boost * time.delta_seconds();
    }
}

/// Movement direction from WASD/QE in the camera's local frame (pure, tested).
fn fly_move_dir(keys: &ButtonInput<KeyCode>, tf: &Transform) -> Vec3 {
    let mut dir = Vec3::ZERO;
    let fwd = *tf.forward();
    let right = *tf.right();
    if keys.pressed(KeyCode::KeyW) {
        dir += fwd;
    }
    if keys.pressed(KeyCode::KeyS) {
        dir -= fwd;
    }
    if keys.pressed(KeyCode::KeyD) {
        dir += right;
    }
    if keys.pressed(KeyCode::KeyA) {
        dir -= right;
    }
    if keys.pressed(KeyCode::KeyE) {
        dir += Vec3::Y;
    }
    if keys.pressed(KeyCode::KeyQ) {
        dir -= Vec3::Y;
    }
    dir
}

fn grab_cursor(windows: &mut Query<&mut Window, With<PrimaryWindow>>, grab: bool) {
    if let Ok(mut window) = windows.get_single_mut() {
        if grab {
            window.cursor.grab_mode = CursorGrabMode::Locked;
            window.cursor.visible = false;
        } else {
            window.cursor.grab_mode = CursorGrabMode::None;
            window.cursor.visible = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_dir_follows_keys_in_camera_frame() {
        // Identity rotation: forward = -Z, right = +X.
        let tf = Transform::from_xyz(0.0, 0.0, 0.0);
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyW);
        let d = fly_move_dir(&keys, &tf);
        assert!(d.z < 0.0 && d.x.abs() < 1e-6, "W moves forward (-Z): {d:?}");

        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyE);
        assert!(fly_move_dir(&keys, &tf).y > 0.0, "E moves up");

        let keys = ButtonInput::<KeyCode>::default();
        assert_eq!(fly_move_dir(&keys, &tf), Vec3::ZERO, "no keys → no move");
    }
}
