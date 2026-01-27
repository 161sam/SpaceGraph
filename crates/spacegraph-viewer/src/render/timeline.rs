use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use std::collections::HashSet;

use crate::graph::model::{edge_explain, edge_kind_name};
use crate::graph::{GraphState, TimelineEvtKind};
use crate::ui::tooltips::render_tooltip;

// v0.1.7 Timeline:
// - Worldlines for visible nodes: constant (y,z), x in [-window*scale .. 0]
// - Event vertices & edge interactions at time slice x
// - Hover tooltip for nearest event point/interaction mid-point
pub fn draw_timeline(
    mut st: ResMut<GraphState>,
    mut gizmos: Gizmos,
    mut contexts: EguiContexts,
    windows: Query<&Window>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };
    let Ok((camera, cam_tf)) = cam_q.get_single() else {
        return;
    };

    egui::Area::new("timeline_legend")
        .fixed_pos(egui::pos2(10.0, 120.0))
        .show(contexts.ctx_mut(), |ui| {
            ui.group(|ui| {
                ui.label("Timeline/Feynman:");
                ui.label("X = time (past → left), now at x=0");
                ui.label("Y lanes: user=0, proc=+8, file=-8");
                ui.label("Z = stable hash spread");
                ui.label("Worldlines + event vertices + interactions");
            });
        });

    let now = st.timeline_now();
    let window_dur = st.timeline.timeline_window;
    let scale = st.timeline.timeline_scale.max(0.001);
    let x_min = -window_dur.as_secs_f32() * scale;
    let x_max = 0.0;

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
    let vis: HashSet<_> = st.visible_set_capped();
    for id in vis.iter() {
        let base = st.timeline_pos_for_node(id);
        let a = Vec3::new(x_min, base.y, base.z);
        let b = Vec3::new(x_max, base.y, base.z);
        gizmos.line(a, b, Color::WHITE);
    }

    // --- Hover detection on events (timeline) ---
    // We define a "representative point" per event:
    // - node events: point at (x, y, z)
    // - edge events: midpoint at (x, avg(y,z))
    let cursor = window.cursor_position();
    let mut hover_best: Option<(f32, String)> = None;

    // Draw events
    for ev in st.timeline.timeline_events.iter() {
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

                // vertex cross
                let s = 0.25;
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

                // hover candidate
                if let (Some(cur), Some(screen)) = (cursor, camera.world_to_viewport(cam_tf, p)) {
                    let d = screen.distance(cur);
                    if d < 14.0 {
                        let label = format!("{:?}\nnode: {}\nage: {:.2}s", ev.kind, aid.0, age);
                        if hover_best.as_ref().map(|(bd, _)| d < *bd).unwrap_or(true) {
                            hover_best = Some((d, label));
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
                gizmos.line(
                    mid + Vec3::new(0.0, -s, 0.0),
                    mid + Vec3::new(0.0, s, 0.0),
                    Color::WHITE,
                );

                // hover candidate at midpoint
                if let (Some(cur), Some(screen)) = (cursor, camera.world_to_viewport(cam_tf, mid)) {
                    let d = screen.distance(cur);
                    if d < 14.0 {
                        let ek = ev.edge_kind.as_ref();
                        let kind_line = ek
                            .map(|k| {
                                format!("edge_kind: {} ({})", edge_kind_name(k), edge_explain(k))
                            })
                            .unwrap_or_else(|| "edge_kind: (none)".to_string());
                        let label = format!(
                            "{:?}\nfrom: {}\nto: {}\n{}\nage: {:.2}s",
                            ev.kind, aid.0, bid.0, kind_line, age
                        );
                        if hover_best.as_ref().map(|(bd, _)| d < *bd).unwrap_or(true) {
                            hover_best = Some((d, label));
                        }
                    }
                }
            }
            TimelineEvtKind::BatchBegin(_) | TimelineEvtKind::BatchEnd(_) => {
                // MVP: don't draw
            }
        }
    }

    // Tooltip rendering (timeline)
    if let Some((_, text)) = hover_best {
        let pos = contexts
            .ctx_mut()
            .input(|i| i.pointer.hover_pos().unwrap_or(egui::pos2(0.0, 0.0)))
            + egui::vec2(14.0, 14.0);

        render_tooltip(
            contexts.ctx_mut(),
            "tooltip_timeline",
            pos,
            text.lines().map(|line| line.to_string()),
        );
    }
}
