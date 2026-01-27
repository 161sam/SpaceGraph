use spacegraph_core::{EdgeKind, NodeId};
use std::time::{Duration, Instant};

use crate::graph::model::node_kind_lane;
use crate::graph::state::{GraphState, TimelineState};
use crate::util::ids::stable_u32;

#[derive(Debug, Clone)]
pub struct NodeLife {
    pub first_seen: Instant,
    pub last_seen: Instant,
    pub removed_at: Option<Instant>,
}

#[derive(Debug, Clone)]
pub struct BatchSpan {
    pub id: u64,
    pub start: Instant,
    pub end: Option<Instant>,
}

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

impl TimelineState {
    pub fn effective_now(&self) -> Instant {
        self.effective_now_from(Instant::now())
    }

    pub fn effective_now_from(&self, base_now: Instant) -> Instant {
        let base = self.frozen_now.unwrap_or(base_now);
        base - Duration::from_secs_f32(self.scrub_seconds.max(0.0))
    }

    pub fn window_start(&self, now: Instant) -> Instant {
        now - self.window
    }

    pub fn record_node_upsert(&mut self, id: &NodeId, ts: Instant) {
        let entry = self.node_life.entry(id.clone()).or_insert(NodeLife {
            first_seen: ts,
            last_seen: ts,
            removed_at: None,
        });
        if ts < entry.first_seen {
            entry.first_seen = ts;
        }
        if ts > entry.last_seen {
            entry.last_seen = ts;
        }
        if entry
            .removed_at
            .map(|removed| ts >= removed)
            .unwrap_or(false)
        {
            entry.removed_at = None;
        }
    }

    pub fn record_node_remove(&mut self, id: &NodeId, ts: Instant) {
        let entry = self.node_life.entry(id.clone()).or_insert(NodeLife {
            first_seen: ts,
            last_seen: ts,
            removed_at: Some(ts),
        });
        if ts < entry.first_seen {
            entry.first_seen = ts;
        }
        if ts > entry.last_seen {
            entry.last_seen = ts;
        }
        entry.removed_at = Some(ts);
    }

    pub fn record_batch_begin(&mut self, id: u64, ts: Instant) {
        self.batch_spans.push_back(BatchSpan {
            id,
            start: ts,
            end: None,
        });
    }

    pub fn record_batch_end(&mut self, id: u64, ts: Instant) {
        if let Some(span) = self
            .batch_spans
            .iter_mut()
            .rev()
            .find(|span| span.id == id && span.end.is_none())
        {
            span.end = Some(ts);
        }
    }

    pub fn trim(&mut self, now: Instant) {
        while self.events.len() > self.max_events {
            self.events.pop_front();
        }

        let window_start = self.window_start(now);
        while let Some(front) = self.events.front() {
            if front.ts < window_start {
                self.events.pop_front();
            } else {
                break;
            }
        }

        while let Some(front) = self.batch_spans.front() {
            match front.end {
                Some(end) if end < window_start => {
                    self.batch_spans.pop_front();
                }
                _ => break,
            }
        }
    }

    pub fn node_life_interval(&self, id: &NodeId, now: Instant) -> Option<(Instant, Instant)> {
        let life = self.node_life.get(id)?;
        let window_start = self.window_start(now);
        let start = if life.first_seen < window_start {
            window_start
        } else {
            life.first_seen
        };
        let mut end = life.removed_at.unwrap_or(now);
        if end > now {
            end = now;
        }
        if end <= start {
            None
        } else {
            Some((start, end))
        }
    }

    pub fn active_batch_span(&self, id: u64) -> Option<&BatchSpan> {
        self.batch_spans.iter().rev().find(|span| span.id == id)
    }
}

impl GraphState {
    // ----- Timeline ticks -----
    pub fn timeline_now(&self) -> Instant {
        self.timeline.effective_now()
    }

    pub fn tick_timeline(&mut self) {
        // cap + window trimming
        let now = self.timeline_now();
        self.timeline.trim(now);
    }

    pub fn set_timeline_pause(&mut self, pause: bool) {
        if pause == self.timeline.pause {
            return;
        }
        self.timeline.pause = pause;
        if pause {
            self.timeline.frozen_now = Some(Instant::now());
        } else {
            self.timeline.frozen_now = None;
            self.timeline.scrub_seconds = 0.0;
        }
        self.needs_redraw
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub(crate) fn push_timeline_at(
        &mut self,
        ts: Instant,
        kind: TimelineEvtKind,
        a: Option<NodeId>,
        b: Option<NodeId>,
        ek: Option<EdgeKind>,
    ) {
        let evt = TimelineEvt {
            ts,
            kind,
            a,
            b,
            edge_kind: ek,
        };
        match evt.kind {
            TimelineEvtKind::NodeUpsert => {
                if let Some(id) = evt.a.as_ref() {
                    self.timeline.record_node_upsert(id, ts);
                }
            }
            TimelineEvtKind::NodeRemove => {
                if let Some(id) = evt.a.as_ref() {
                    self.timeline.record_node_remove(id, ts);
                }
            }
            TimelineEvtKind::BatchBegin(id) => {
                self.timeline.record_batch_begin(id, ts);
            }
            TimelineEvtKind::BatchEnd(id) => {
                self.timeline.record_batch_end(id, ts);
            }
            _ => {}
        }
        self.timeline.events.push_back(evt);
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
        st.timeline.pause = true;
        let now = Instant::now();
        st.timeline.frozen_now = Some(now);
        st.timeline.window = Duration::from_secs(10);

        st.timeline.events.push_back(TimelineEvt {
            ts: now - Duration::from_secs(20),
            kind: TimelineEvtKind::NodeUpsert,
            a: None,
            b: None,
            edge_kind: None,
        });
        st.timeline.events.push_back(TimelineEvt {
            ts: now - Duration::from_secs(5),
            kind: TimelineEvtKind::NodeUpsert,
            a: None,
            b: None,
            edge_kind: None,
        });

        st.tick_timeline();
        assert_eq!(st.timeline.events.len(), 1);
    }

    #[test]
    fn timeline_caps_max_events() {
        let mut st = GraphState::default();
        st.timeline.pause = true;
        let now = Instant::now();
        st.timeline.frozen_now = Some(now);
        st.timeline.max_events = 3;
        st.timeline.window = Duration::from_secs(60);

        for i in 0..5 {
            st.timeline.events.push_back(TimelineEvt {
                ts: now - Duration::from_secs(i),
                kind: TimelineEvtKind::NodeUpsert,
                a: None,
                b: None,
                edge_kind: None,
            });
        }

        st.tick_timeline();
        assert_eq!(st.timeline.events.len(), 3);
    }

    #[test]
    fn pause_freezes_now_and_scrub_moves_back() {
        let mut timeline = TimelineState::default();
        let base = Instant::now();
        timeline.pause = true;
        timeline.frozen_now = Some(base);
        timeline.scrub_seconds = 2.5;
        let effective = timeline.effective_now_from(base + Duration::from_secs(5));
        assert_eq!(effective, base - Duration::from_secs_f32(2.5));
    }

    #[test]
    fn worldline_lifespan_respects_first_seen_and_removed_at() {
        let mut timeline = TimelineState::default();
        let base = Instant::now();
        let id = NodeId("node-1".to_string());
        timeline.window = Duration::from_secs(30);
        timeline.record_node_upsert(&id, base);
        timeline.record_node_remove(&id, base + Duration::from_secs(12));

        let now = base + Duration::from_secs(20);
        let (start, end) = timeline.node_life_interval(&id, now).expect("interval");
        assert_eq!(start, base);
        assert_eq!(end, base + Duration::from_secs(12));
    }

    #[test]
    fn batch_spans_open_close_and_trim() {
        let mut timeline = TimelineState::default();
        let base = Instant::now();
        timeline.window = Duration::from_secs(10);

        timeline.record_batch_begin(7, base);
        timeline.record_batch_end(7, base + Duration::from_secs(4));

        let span = timeline.active_batch_span(7).expect("span");
        assert_eq!(span.start, base);
        assert_eq!(span.end, Some(base + Duration::from_secs(4)));

        let now = base + Duration::from_secs(25);
        timeline.trim(now);
        assert!(timeline.active_batch_span(7).is_none());
    }
}
