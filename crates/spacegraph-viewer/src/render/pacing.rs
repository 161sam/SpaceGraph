//! Reactive frame pacing.
//!
//! By default Bevy renders continuously at full speed even when nothing on
//! screen changes. This module drives [`WinitSettings::focused_mode`] between
//! [`UpdateMode::Continuous`] (something is animating) and a low-rate
//! [`UpdateMode::Reactive`] heartbeat (idle), cutting CPU/GPU use to a fraction
//! when the graph is at rest. Reactive mode still redraws immediately on any
//! input/window event, so interaction stays instant; the heartbeat only exists
//! so incoming network data and late layout get processed while idle.
//!
//! The decision runs in the `Last` schedule so it observes every `needs_redraw`
//! request made during the frame before consuming it.

use bevy::prelude::*;
use bevy::winit::{UpdateMode, WinitSettings};
use bevy_panorbit_camera::PanOrbitCamera;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::graph::{GraphState, ViewMode};
use crate::render::freefly::FlyCam;
use crate::render::gameplay::{Mission, ScanPulse};

/// Frames to stay continuous after the last detected change, so eased camera
/// motion and egui transitions finish cleanly before dropping to reactive.
const COOLDOWN_FRAMES: u32 = 6;

/// Idle heartbeat: how often to wake when nothing animates. Its only job is to
/// poll the network channel (data arrives off a thread, not via a winit event);
/// input/window events still redraw instantly in reactive mode regardless. ~4 Hz
/// keeps worst-case "new event appears" latency imperceptible while idle CPU/GPU
/// drops to a small fraction of the 60 Hz continuous cost.
const IDLE_WAIT: Duration = Duration::from_millis(250);

#[derive(Resource, Default)]
pub struct FramePacing {
    cooldown: u32,
}

#[allow(clippy::too_many_arguments)]
pub fn update_frame_pacing(
    st: Res<GraphState>,
    fly: Res<FlyCam>,
    scan: Res<ScanPulse>,
    mission: Res<Mission>,
    cam_q: Query<&PanOrbitCamera>,
    mut pacing: ResMut<FramePacing>,
    mut winit: ResMut<WinitSettings>,
) {
    // Consume one-shot redraw requests raised anywhere this frame.
    let redraw = st.needs_redraw.swap(false, Ordering::Relaxed);

    let now = Instant::now();
    let layout_active =
        st.ui.view_mode == ViewMode::Spatial && st.cfg.layout_force && !st.spatial.layout_settled;
    let timeline_active = st.ui.view_mode == ViewMode::Timeline && !st.timeline.pause;
    // The orbit camera eases toward its targets over several self-driven frames.
    let camera_easing = cam_q.iter().any(|c| {
        (c.focus - c.target_focus).length_squared() > 1.0e-4
            || c.radius
                .is_some_and(|r| (r - c.target_radius).abs() > 1.0e-3)
            || c.yaw.is_some_and(|y| (y - c.target_yaw).abs() > 1.0e-4)
            || c.pitch.is_some_and(|p| (p - c.target_pitch).abs() > 1.0e-4)
    });

    let active = redraw
        || fly.active
        || scan.active
        || mission.active
        || layout_active
        || timeline_active
        || camera_easing
        || st.spatial.has_active_glow(now);

    if active {
        pacing.cooldown = COOLDOWN_FRAMES;
    } else {
        pacing.cooldown = pacing.cooldown.saturating_sub(1);
    }

    winit.focused_mode = if pacing.cooldown > 0 {
        UpdateMode::Continuous
    } else {
        UpdateMode::reactive(IDLE_WAIT)
    };
}
