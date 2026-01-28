use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use std::collections::HashSet;
use std::sync::atomic::Ordering;

use crate::graph::model::{edge_explain, edge_kind_name};
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

    let has_visible_nodes = st.perf.visible_nodes > 0;
    let has_events = !st.timeline.events.is_empty();
    let has_batches = !st.timeline.batch_spans.is_empty();
    let should_draw_guides = has_visible_nodes || has_events || has_batches;
    if !should_draw_guides {
        return;
    }

    // axis line (now)
    gizmos.line(
        Vec3::new(0.0, -10.0, 0.0),
        Vec3::new(0.0, 10.0, 0.0),
        Color::WHITE,
    );

    // lane guides
    gizmos.line(
        Vec3::new(x_min, 8.0, -20.0),
        Vec3::new(x_max, 8.0, 20.0),
        Color::WHITE,
    );
    gizmos.line(
        Vec3::new(x_min, 0.0, -20.0),
        Vec3::new(x_max, 0.0, 20.0),
        Color::WHITE,
    );
    gizmos.line(
        Vec3::new(x_min, -8.0, -20.0),
        Vec3::new(x_max, -8.0, 20.0),
        Color::WHITE,
    );

    // Worldlines for current visible set (capped)
    if has_visible_nodes {
        let vis: HashSet<_> = st.visible_set_capped();
        for id in vis.iter() {
            let Some((start, end)) = st.timeline.node_life_interval(id, now) else {
                continue;
            };
            let base = st.timeline_pos_for_node(id);
            let start_age = now.duration_since(start).as_secs_f32();
            let end_age = now.duration_since(end).as_secs_f32();
            let a = Vec3::new(-start_age * scale, base.y, base.z);
            let b = Vec3::new(-end_age * scale, base.y, base.z);
            gizmos.line(a, b, Color::WHITE);
        }
    }

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
        let start_age = now.duration_since(start).as_secs_f32();
        let end_age = now.duration_since(end).as_secs_f32();
        let x_start = -start_age * scale;
        let x_end = -end_age * scale;
        let y_min = -10.0;
        let y_max = 10.0;

        gizmos.line(
            Vec3::new(x_start, y_min, 0.0),
            Vec3::new(x_start, y_max, 0.0),
            Color::WHITE,
        );
        if span.end.is_some() {
            gizmos.line(
                Vec3::new(x_end, y_min, 0.0),
                Vec3::new(x_end, y_max, 0.0),
                Color::WHITE,
            );
        }
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

    // Draw events
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
                let base = st.timeline_pos_for_node(aid);
                let p = Vec3::new(x, base.y, base.z);

                let s = 0.25;
                match ev.kind {
                    TimelineEvtKind::NodeUpsert => {
                        gizmos.line(
                            p + Vec3::new(-s, 0.0, 0.0),
                            p + Vec3::new(s, 0.0, 0.0),
                            Color::WHITE,
                        );
                        gizmos.line(
                            p + Vec3::new(0.0, -s, 0.0),
                            p + Vec3::new(0.0, s, 0.0),
                            Color::WHITE,
                        );
                    }
                    TimelineEvtKind::NodeRemove => {
                        gizmos.line(
                            p + Vec3::new(-s, -s, 0.0),
                            p + Vec3::new(s, s, 0.0),
                            Color::WHITE,
                        );
                        gizmos.line(
                            p + Vec3::new(-s, s, 0.0),
                            p + Vec3::new(s, -s, 0.0),
                            Color::WHITE,
                        );
                    }
                    _ => {}
                }

                // hover candidate
                if allow_pick {
                    if let (Some(cur), Some(screen)) = (cursor, camera.world_to_viewport(cam_tf, p))
                    {
                        let d = screen.distance(cur);
                        if d < 14.0 {
                            let mut lines = Vec::new();
                            let label = label_for_node(aid);
                            lines.push(format!("{:?}", ev.kind));
                            lines.push(format!("node: {} ({})", label, aid.0));
                            if let Some(node) = st.model.nodes.get(aid) {
                                lines.extend(node_label_long(node));
                            }
                            lines.push(format!("age: {:.2}s", age));
                            let pick = Some(TimelinePick::Node(aid.clone()));
                            if hover_best
                                .as_ref()
                                .map(|best| d < best.dist)
                                .unwrap_or(true)
                            {
                                hover_best = Some(HoverPick {
                                    dist: d,
                                    text: lines.join("\n"),
                                    pick,
                                });
                            }
                        }
                    }
                }
            }
            TimelineEvtKind::EdgeUpsert | TimelineEvtKind::EdgeRemove => {
                let (Some(aid), Some(bid)) = (&ev.a, &ev.b) else {
                    continue;
                };
                let pa = st.timeline_pos_for_node(aid);
                let pb = st.timeline_pos_for_node(bid);

                let a3 = Vec3::new(x, pa.y, pa.z);
                let b3 = Vec3::new(x, pb.y, pb.z);

                // interaction line
                gizmos.line(a3, b3, Color::WHITE);

                // midpoint tick (event vertex marker)
                let mid = (a3 + b3) * 0.5;
                let s = 0.18;
                match ev.kind {
                    TimelineEvtKind::EdgeUpsert => {
                        gizmos.line(
                            mid + Vec3::new(0.0, -s, 0.0),
                            mid + Vec3::new(0.0, s, 0.0),
                            Color::WHITE,
                        );
                    }
                    TimelineEvtKind::EdgeRemove => {
                        let offset = 0.12;
                        for dy in [-offset, offset] {
                            gizmos.line(
                                mid + Vec3::new(0.0, -s + dy, 0.0),
                                mid + Vec3::new(0.0, s + dy, 0.0),
                                Color::WHITE,
                            );
                        }
                    }
                    _ => {}
                }

                // hover candidate at midpoint
                if allow_pick {
                    if let (Some(cur), Some(screen)) =
                        (cursor, camera.world_to_viewport(cam_tf, mid))
                    {
                        let d = screen.distance(cur);
                        if d < 14.0 {
                            let ek = ev.edge_kind.as_ref();
                            let kind_line = ek
                                .map(|k| {
                                    format!(
                                        "edge_kind: {} ({})",
                                        edge_kind_name(k),
                                        edge_explain(k)
                                    )
                                })
                                .unwrap_or_else(|| "edge_kind: (none)".to_string());
                            let label = format!(
                                "{:?}\nfrom: {} ({})\nto: {} ({})\n{}\nage: {:.2}s",
                                ev.kind,
                                label_for_node(aid),
                                aid.0,
                                label_for_node(bid),
                                bid.0,
                                kind_line,
                                age
                            );
                            let pick = Some(TimelinePick::Edge(aid.clone(), bid.clone()));
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
            TimelineEvtKind::BatchBegin(id) | TimelineEvtKind::BatchEnd(id) => {
                if let Some(span) = st.timeline.active_batch_span(*id) {
                    let start = span.start.max(window_start);
                    let end = span.end.unwrap_or(now).min(now);
                    if end <= start {
                        continue;
                    }
                    let mid_ts = start + (end - start) / 2;
                    let mid_age = now.duration_since(mid_ts).as_secs_f32();
                    let mid = Vec3::new(-mid_age * scale, 0.0, 0.0);
                    if allow_pick {
                        if let (Some(cur), Some(screen)) =
                            (cursor, camera.world_to_viewport(cam_tf, mid))
                        {
                            let d = screen.distance(cur);
                            if d < 14.0 {
                                let duration = end.duration_since(span.start).as_secs_f32();
                                let label = format!(
                                    "Batch {}\nspan: {:.2}s\nage: {:.2}s",
                                    id, duration, age
                                );
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
