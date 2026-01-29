use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::atomic::Ordering;
use std::time::Instant;

use crate::graph::model::{edge_explain, edge_kind_name};
use crate::graph::timeline::timeline_lane_key;
use crate::graph::{GraphState, TimelineEvtKind};
use crate::ui::tooltips::render_tooltip;
use crate::ui::UiLayout;
use crate::util::ids::{node_label_long, node_label_short};

#[derive(Clone)]
enum TimelinePick {
    Node(spacegraph_core::NodeId),
    Edge(spacegraph_core::NodeId, spacegraph_core::NodeId),
}

struct HoverPick {
    dist: f32,
    text: String,
    pick: Option<TimelinePick>,
}

struct EventEntry {
    lane_key: String,
    x: f32,
    kind: TimelineEvtKind,
    age: f32,
    pick: Option<TimelinePick>,
    a: Option<spacegraph_core::NodeId>,
    b: Option<spacegraph_core::NodeId>,
    edge_kind: Option<spacegraph_core::EdgeKind>,
}

fn is_valid_segment(a: Vec3, b: Vec3) -> bool {
    a.is_finite() && b.is_finite() && (a - b).length_squared() > f32::EPSILON
}

fn draw_segment(gizmos: &mut Gizmos, a: Vec3, b: Vec3, color: Color) {
    if is_valid_segment(a, b) {
        gizmos.line(a, b, color);
    }
}

fn event_color(kind: &TimelineEvtKind) -> Color {
    match kind {
        TimelineEvtKind::NodeUpsert => Color::srgb(0.2, 0.85, 0.3),
        TimelineEvtKind::NodeRemove => Color::srgb(0.9, 0.2, 0.2),
        TimelineEvtKind::EdgeUpsert => Color::srgb(0.2, 0.55, 0.9),
        TimelineEvtKind::EdgeRemove => Color::srgb(0.9, 0.55, 0.2),
        TimelineEvtKind::BatchBegin(_) | TimelineEvtKind::BatchEnd(_) => {
            Color::srgb(0.75, 0.75, 0.75)
        }
    }
}

fn event_color_with_alpha(kind: &TimelineEvtKind, alpha: f32) -> Color {
    match kind {
        TimelineEvtKind::NodeUpsert => Color::srgba(0.2, 0.85, 0.3, alpha),
        TimelineEvtKind::NodeRemove => Color::srgba(0.9, 0.2, 0.2, alpha),
        TimelineEvtKind::EdgeUpsert => Color::srgba(0.2, 0.55, 0.9, alpha),
        TimelineEvtKind::EdgeRemove => Color::srgba(0.9, 0.55, 0.2, alpha),
        TimelineEvtKind::BatchBegin(_) | TimelineEvtKind::BatchEnd(_) => {
            Color::srgba(0.75, 0.75, 0.75, alpha)
        }
    }
}

fn draw_event_marker(gizmos: &mut Gizmos, pos: Vec3, kind: &TimelineEvtKind) {
    if !pos.is_finite() {
        return;
    }
    let s = 0.25;
    let color = event_color(kind);
    match kind {
        TimelineEvtKind::NodeUpsert => {
            draw_segment(
                gizmos,
                pos + Vec3::new(-s, 0.0, 0.0),
                pos + Vec3::new(s, 0.0, 0.0),
                color,
            );
            draw_segment(
                gizmos,
                pos + Vec3::new(0.0, -s, 0.0),
                pos + Vec3::new(0.0, s, 0.0),
                color,
            );
        }
        TimelineEvtKind::NodeRemove => {
            draw_segment(
                gizmos,
                pos + Vec3::new(-s, -s, 0.0),
                pos + Vec3::new(s, s, 0.0),
                color,
            );
            draw_segment(
                gizmos,
                pos + Vec3::new(-s, s, 0.0),
                pos + Vec3::new(s, -s, 0.0),
                color,
            );
        }
        TimelineEvtKind::EdgeUpsert => {
            draw_segment(
                gizmos,
                pos + Vec3::new(0.0, -s, 0.0),
                pos + Vec3::new(0.0, s, 0.0),
                color,
            );
        }
        TimelineEvtKind::EdgeRemove => {
            let offset = 0.12;
            for dy in [-offset, offset] {
                draw_segment(
                    gizmos,
                    pos + Vec3::new(0.0, -s + dy, 0.0),
                    pos + Vec3::new(0.0, s + dy, 0.0),
                    color,
                );
            }
        }
        TimelineEvtKind::BatchBegin(_) | TimelineEvtKind::BatchEnd(_) => {}
    }
}

// v0.1.7 Timeline:
// - Worldlines for visible nodes: constant (y,z), x in [-window*scale .. 0]
// - Event vertices & edge interactions at time slice x
// - Hover tooltip for nearest event point/interaction mid-point
pub fn draw_timeline(
    mut st: ResMut<GraphState>,
    mut gizmos: Gizmos,
    mut contexts: EguiContexts,
    layout: Res<UiLayout>,
    windows: Query<&Window>,
    buttons: Res<ButtonInput<MouseButton>>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };
    let Ok((camera, cam_tf)) = cam_q.get_single() else {
        return;
    };

    let ctx = contexts.ctx_mut();
    let content_rect = if layout.content_rect.width() > 0.0 && layout.content_rect.height() > 0.0 {
        layout.content_rect
    } else {
        ctx.screen_rect()
    };

    let now = st.timeline_now();
    let window_dur = st.timeline.window;
    let scale = st.timeline.scale.max(0.001);
    let x_min = -window_dur.as_secs_f32() * scale;
    let x_max = 0.0;
    let window_start = st.timeline.window_start(now);
    let cursor = window.cursor_position();
    let allow_pick = cursor
        .map(|pos| content_rect.contains(egui::pos2(pos.x, pos.y)))
        .unwrap_or(false)
        && !ctx.wants_pointer_input();

    let has_events = !st.timeline.events.is_empty();
    let has_batches = !st.timeline.batch_spans.is_empty();
    let should_draw_guides = has_events || has_batches;
    if !should_draw_guides {
        return;
    }

    let lane_key_for_node = |id: &spacegraph_core::NodeId| {
        st.model
            .nodes
            .get(id)
            .map(timeline_lane_key)
            .unwrap_or_else(|| format!("id:{}", id.0))
    };
    let mut lane_keys: BTreeSet<String> = BTreeSet::new();
    let mut event_entries: Vec<EventEntry> = Vec::new();

    let mut batch_spans: Vec<(u64, Instant, Instant, bool)> = Vec::new();
    // Batch spans (begin/end bands)
    for span in st.timeline.batch_spans.iter() {
        let start = if span.start < window_start {
            window_start
        } else {
            span.start
        };
        let end = span.end.unwrap_or(now).min(now);
        if end <= start {
            continue;
        }
        batch_spans.push((span.id, start, end, span.end.is_some()));
    }

    // --- Hover detection on events (timeline) ---
    // We define a "representative point" per event:
    // - node events: point at (x, y, z)
    // - edge events: midpoint at (x, avg(y,z))
    let mut hover_best: Option<HoverPick> = None;
    let label_for_node = |id: &spacegraph_core::NodeId| {
        st.model
            .nodes
            .get(id)
            .map(node_label_short)
            .unwrap_or_else(|| id.0.clone())
    };

    // Collect events
    for ev in st.timeline.events.iter() {
        if ev.ts > now {
            continue;
        }
        let age = now.duration_since(ev.ts).as_secs_f32();
        if age > window_dur.as_secs_f32() {
            continue;
        }
        let x = -age * scale;

        match &ev.kind {
            TimelineEvtKind::NodeUpsert | TimelineEvtKind::NodeRemove => {
                let Some(aid) = &ev.a else {
                    continue;
                };
                let lane_key = lane_key_for_node(aid);
                lane_keys.insert(lane_key.clone());
                event_entries.push(EventEntry {
                    lane_key,
                    x,
                    kind: ev.kind.clone(),
                    age,
                    pick: Some(TimelinePick::Node(aid.clone())),
                    a: Some(aid.clone()),
                    b: None,
                    edge_kind: None,
                });
            }
            TimelineEvtKind::EdgeUpsert | TimelineEvtKind::EdgeRemove => {
                let (Some(aid), Some(bid)) = (&ev.a, &ev.b) else {
                    continue;
                };
                let lane_key_a = lane_key_for_node(aid);
                let lane_key_b = lane_key_for_node(bid);
                lane_keys.insert(lane_key_a.clone());
                lane_keys.insert(lane_key_b.clone());
                let pick = Some(TimelinePick::Edge(aid.clone(), bid.clone()));
                event_entries.push(EventEntry {
                    lane_key: lane_key_a,
                    x,
                    kind: ev.kind.clone(),
                    age,
                    pick: pick.clone(),
                    a: Some(aid.clone()),
                    b: Some(bid.clone()),
                    edge_kind: ev.edge_kind.clone(),
                });
                event_entries.push(EventEntry {
                    lane_key: lane_key_b,
                    x,
                    kind: ev.kind.clone(),
                    age,
                    pick,
                    a: Some(aid.clone()),
                    b: Some(bid.clone()),
                    edge_kind: ev.edge_kind.clone(),
                });
            }
            TimelineEvtKind::BatchBegin(_) | TimelineEvtKind::BatchEnd(_) => {}
        }
    }

    let lane_spacing = 2.2;
    let lane_count = lane_keys.len();
    let lane_offset = if lane_count > 0 {
        (lane_count as f32 - 1.0) * 0.5 * lane_spacing
    } else {
        0.0
    };
    let mut lane_positions: BTreeMap<String, f32> = BTreeMap::new();
    for (idx, key) in lane_keys.iter().enumerate() {
        let y = idx as f32 * lane_spacing - lane_offset;
        lane_positions.insert(key.clone(), y);
    }
    let (lane_y_min, lane_y_max) = if lane_count > 0 {
        (
            -lane_offset - lane_spacing * 0.5,
            lane_offset + lane_spacing * 0.5,
        )
    } else {
        (-2.0, 2.0)
    };

    // axis line (now)
    draw_segment(
        &mut gizmos,
        Vec3::new(0.0, lane_y_min, 0.0),
        Vec3::new(0.0, lane_y_max, 0.0),
        Color::WHITE,
    );

    // lane guides
    for y in lane_positions.values() {
        draw_segment(
            &mut gizmos,
            Vec3::new(x_min, *y, 0.0),
            Vec3::new(x_max, *y, 0.0),
            Color::srgb(0.7, 0.7, 0.7),
        );
    }

    // Batch spans (begin/end bands)
    for (id, start, end, has_end) in batch_spans.iter() {
        let start_age = now.duration_since(*start).as_secs_f32();
        let end_age = now.duration_since(*end).as_secs_f32();
        let x_start = -start_age * scale;
        let x_end = -end_age * scale;
        let color = event_color(&TimelineEvtKind::BatchBegin(*id));

        draw_segment(
            &mut gizmos,
            Vec3::new(x_start, lane_y_min, 0.0),
            Vec3::new(x_start, lane_y_max, 0.0),
            color,
        );
        if *has_end {
            draw_segment(
                &mut gizmos,
                Vec3::new(x_end, lane_y_min, 0.0),
                Vec3::new(x_end, lane_y_max, 0.0),
                color,
            );
        }
    }

    let mut last_in_lane: HashMap<String, Vec3> = HashMap::new();

    // Draw events + hover detection
    for entry in event_entries.iter() {
        let Some(y) = lane_positions.get(&entry.lane_key).copied() else {
            continue;
        };
        let pos = Vec3::new(entry.x, y, 0.0);
        if st.timeline.show_connectors {
            if let Some(prev) = last_in_lane.get(&entry.lane_key) {
                draw_segment(
                    &mut gizmos,
                    *prev,
                    pos,
                    event_color_with_alpha(&entry.kind, 0.35),
                );
            }
        }
        last_in_lane.insert(entry.lane_key.clone(), pos);
        draw_event_marker(&mut gizmos, pos, &entry.kind);

        if allow_pick {
            if let (Some(cur), Some(screen)) = (cursor, camera.world_to_viewport(cam_tf, pos)) {
                let d = screen.distance(cur);
                if d < 14.0 {
                    let pick = entry.pick.clone();
                    let label = match &entry.kind {
                        TimelineEvtKind::NodeUpsert | TimelineEvtKind::NodeRemove => {
                            entry.a.as_ref().map(|aid| {
                                let mut lines = Vec::new();
                                let label = label_for_node(aid);
                                lines.push(format!("{:?}", entry.kind));
                                lines.push(format!("node: {} ({})", label, aid.0));
                                if let Some(node) = st.model.nodes.get(aid) {
                                    lines.extend(node_label_long(node));
                                }
                                lines.push(format!("age: {:.2}s", entry.age));
                                lines.join("\n")
                            })
                        }
                        TimelineEvtKind::EdgeUpsert | TimelineEvtKind::EdgeRemove => {
                            if let (Some(aid), Some(bid)) = (entry.a.as_ref(), entry.b.as_ref()) {
                                let ek = entry.edge_kind.as_ref();
                                let kind_line = ek
                                    .map(|k| {
                                        format!(
                                            "edge_kind: {} ({})",
                                            edge_kind_name(k),
                                            edge_explain(k)
                                        )
                                    })
                                    .unwrap_or_else(|| "edge_kind: (none)".to_string());
                                Some(format!(
                                    "{:?}\nfrom: {} ({})\nto: {} ({})\n{}\nage: {:.2}s",
                                    entry.kind,
                                    label_for_node(aid),
                                    aid.0,
                                    label_for_node(bid),
                                    bid.0,
                                    kind_line,
                                    entry.age
                                ))
                            } else {
                                None
                            }
                        }
                        TimelineEvtKind::BatchBegin(_) | TimelineEvtKind::BatchEnd(_) => None,
                    };
                    if let Some(label) = label {
                        if hover_best
                            .as_ref()
                            .map(|best| d < best.dist)
                            .unwrap_or(true)
                        {
                            hover_best = Some(HoverPick {
                                dist: d,
                                text: label,
                                pick,
                            });
                        }
                    }
                }
            }
        }
    }

    // Hover detection for batch spans
    if allow_pick {
        for (id, start, end, _has_end) in batch_spans.iter() {
            let mid_ts = *start + (*end - *start) / 2;
            let mid_age = now.duration_since(mid_ts).as_secs_f32();
            let mid = Vec3::new(-mid_age * scale, 0.0, 0.0);
            if let (Some(cur), Some(screen)) = (cursor, camera.world_to_viewport(cam_tf, mid)) {
                let d = screen.distance(cur);
                if d < 14.0 {
                    let duration = end.duration_since(*start).as_secs_f32();
                    let label =
                        format!("Batch {}\nspan: {:.2}s\nage: {:.2}s", id, duration, mid_age);
                    if hover_best
                        .as_ref()
                        .map(|best| d < best.dist)
                        .unwrap_or(true)
                    {
                        hover_best = Some(HoverPick {
                            dist: d,
                            text: label,
                            pick: None,
                        });
                    }
                }
            }
        }
    }

    if allow_pick && buttons.just_pressed(MouseButton::Left) {
        if let Some(pick) = hover_best.as_ref().and_then(|best| best.pick.clone()) {
            match pick {
                TimelinePick::Node(id) => {
                    st.ui.selected_a = Some(id.clone());
                    st.ui.selected_b = None;
                    st.ui.selected = Some(id);
                }
                TimelinePick::Edge(from, to) => {
                    st.ui.selected_a = Some(from.clone());
                    st.ui.selected_b = Some(to);
                    st.ui.selected = Some(from);
                }
            }
            st.needs_redraw.store(true, Ordering::Relaxed);
        }
    }

    // Tooltip rendering (timeline)
    if let Some(best) = hover_best {
        let mut pos = ctx.input(|i| i.pointer.hover_pos().unwrap_or(egui::pos2(0.0, 0.0)))
            + egui::vec2(14.0, 14.0);
        if content_rect.width() > 0.0 && content_rect.height() > 0.0 {
            pos.x = pos.x.clamp(content_rect.min.x, content_rect.max.x);
            pos.y = pos.y.clamp(content_rect.min.y, content_rect.max.y);
        }

        render_tooltip(
            ctx,
            "tooltip_timeline",
            pos,
            best.text.lines().map(|line| line.to_string()),
        );
    }
}
