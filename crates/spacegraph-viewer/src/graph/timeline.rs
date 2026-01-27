use spacegraph_core::{EdgeKind, NodeId};
use std::time::Instant;

use crate::graph::model::node_kind_lane;
use crate::graph::state::GraphState;
use crate::util::ids::stable_u32;

#[derive(Debug, Clone)]
pub struct TimelineEvt {
    pub ts: Instant,
    pub kind: TimelineEvtKind,
    pub a: Option<NodeId>,
    pub b: Option<NodeId>,
    pub edge_kind: Option<EdgeKind>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum TimelineEvtKind {
    NodeUpsert,
    NodeRemove,
    EdgeUpsert,
    EdgeRemove,
    BatchBegin(u64),
    BatchEnd(u64),
}

impl GraphState {
    // ----- Timeline ticks -----
    pub fn timeline_now(&self) -> Instant {
        if self.timeline.timeline_pause {
            self.timeline
                .timeline_frozen_now
                .unwrap_or_else(Instant::now)
        } else {
            Instant::now()
        }
    }

    pub fn tick_timeline(&mut self) {
        // cap + window trimming
        let now = self.timeline_now();
        while self.timeline.timeline_events.len() > self.timeline.timeline_max_events {
            self.timeline.timeline_events.pop_front();
        }
        while let Some(front) = self.timeline.timeline_events.front() {
            if now.duration_since(front.ts) > self.timeline.timeline_window {
                self.timeline.timeline_events.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn set_timeline_pause(&mut self, pause: bool) {
        if pause == self.timeline.timeline_pause {
            return;
        }
        self.timeline.timeline_pause = pause;
        if pause {
            self.timeline.timeline_frozen_now = Some(Instant::now());
        } else {
            self.timeline.timeline_frozen_now = None;
        }
        self.needs_redraw
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub(crate) fn push_timeline(
        &mut self,
        kind: TimelineEvtKind,
        a: Option<NodeId>,
        b: Option<NodeId>,
        ek: Option<EdgeKind>,
    ) {
        let evt = TimelineEvt {
            ts: Instant::now(),
            kind,
            a,
            b,
            edge_kind: ek,
        };
        self.timeline.timeline_events.push_back(evt);
    }

    // ---- Timeline mapping helpers ----
    pub fn timeline_pos_for_node(&self, id: &NodeId) -> bevy::prelude::Vec3 {
        // Based on node kind -> Y lane; Z = stable hash; X set by event time elsewhere.
        let y = self.model.nodes.get(id).map(node_kind_lane).unwrap_or(0.0);
        let hz = stable_u32(&id.0) as f32 / 65535.0; // 0..1
        let z = (hz - 0.5) * 18.0;
        bevy::prelude::Vec3::new(0.0, y, z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::state::GraphState;
    use std::time::Duration;

    #[test]
    fn timeline_trims_old_events() {
        let mut st = GraphState::default();
        st.timeline.timeline_pause = true;
        let now = Instant::now();
        st.timeline.timeline_frozen_now = Some(now);
        st.timeline.timeline_window = Duration::from_secs(10);

        st.timeline.timeline_events.push_back(TimelineEvt {
            ts: now - Duration::from_secs(20),
            kind: TimelineEvtKind::NodeUpsert,
            a: None,
            b: None,
            edge_kind: None,
        });
        st.timeline.timeline_events.push_back(TimelineEvt {
            ts: now - Duration::from_secs(5),
            kind: TimelineEvtKind::NodeUpsert,
            a: None,
            b: None,
            edge_kind: None,
        });

        st.tick_timeline();
        assert_eq!(st.timeline.timeline_events.len(), 1);
    }

    #[test]
    fn timeline_caps_max_events() {
        let mut st = GraphState::default();
        st.timeline.timeline_pause = true;
        let now = Instant::now();
        st.timeline.timeline_frozen_now = Some(now);
        st.timeline.timeline_max_events = 3;
        st.timeline.timeline_window = Duration::from_secs(60);

        for i in 0..5 {
            st.timeline.timeline_events.push_back(TimelineEvt {
                ts: now - Duration::from_secs(i),
                kind: TimelineEvtKind::NodeUpsert,
                a: None,
                b: None,
                edge_kind: None,
            });
        }

        st.tick_timeline();
        assert_eq!(st.timeline.timeline_events.len(), 3);
    }
}
