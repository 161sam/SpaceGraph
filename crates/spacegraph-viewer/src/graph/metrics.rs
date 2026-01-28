use bevy::prelude::{Res, ResMut, Time};
use std::time::{Duration, Instant};

use crate::graph::state::GraphState;

pub fn tick_housekeeping(time: Res<Time>, mut st: ResMut<GraphState>) {
    let dt = time.delta_seconds().max(0.0001);
    st.perf.fps = 1.0 / dt;

    st.tick_glow();
    st.tick_metrics(Instant::now());
    st.tick_gc();

    st.tick_timeline();
}

impl GraphState {
    // ----- HUD metrics -----
    pub fn tick_metrics(&mut self, now: Instant) {
        let window = Duration::from_secs(2);
        while let Some(front) = self.perf.ev_window.front() {
            if now.duration_since(*front) > window {
                self.perf.ev_window.pop_front();
            } else {
                break;
            }
        }
        self.perf.event_rate = (self.perf.ev_window.len() as f32) / window.as_secs_f32();

        for stream in self.net.streams.values_mut() {
            while let Some(front) = stream.msg_window.front() {
                if now.duration_since(*front) > self.net.msg_window {
                    stream.msg_window.pop_front();
                } else {
                    break;
                }
            }
            stream.msg_rate = stream.msg_window.len() as f32 / self.net.msg_window.as_secs_f32();
        }
    }

    pub(crate) fn on_message(&mut self) {
        self.perf.event_total += 1;
        self.perf.ev_window.push_back(Instant::now());
    }
}
