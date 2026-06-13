//! Gamification mechanics: a scan pulse (active exploration) and an
//! incident-hunt mission loop built on the Phase 8 alerts.

use bevy::prelude::*;
use bevy_egui::EguiContexts;
use spacegraph_core::NodeId;
use std::time::Instant;

use crate::graph::model::EdgeKindClass;
use crate::graph::GraphState;
use crate::render::theme;

// ---- Scan pulse ----

/// An expanding "sonar" wave from the camera that glows nodes it sweeps over.
#[derive(Resource, Default)]
pub struct ScanPulse {
    pub active: bool,
    pub radius: f32,
    pub origin: Vec3,
}

/// Width of the scan-pulse "active band" that glows nodes as the wave passes.
/// (Speed, max range and reveal radius are configurable — see `cfg`.)
const SCAN_BAND: f32 = 10.0;

/// Reveal nodes the camera comes close to (fog-of-war exploration). Placement
/// runs on the full projection, so unrevealed nodes already have positions to
/// test against here. No-op when fog is off.
pub fn reveal_tick(cam_q: Query<&Transform, With<Camera>>, mut st: ResMut<GraphState>) {
    if !st.cfg.fog_of_war {
        return;
    }
    let Ok(tf) = cam_q.get_single() else {
        return;
    };
    let cam = tf.translation;
    let r2 = st.cfg.reveal_radius * st.cfg.reveal_radius;
    let mut newly: Vec<NodeId> = st
        .spatial
        .placed_positions()
        .filter(|(id, p)| (*p - cam).length_squared() <= r2 && !st.revealed.contains(*id))
        .map(|(id, _)| id.clone())
        .collect();
    // The active focus/selection is always revealed (e.g. mission jump targets).
    newly.extend(st.ui.focus.iter().chain(st.ui.selected.iter()).cloned());
    for id in newly {
        st.reveal(&id);
    }
}

pub fn scan_pulse(
    mut scan: ResMut<ScanPulse>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut contexts: EguiContexts,
    cam_q: Query<&Transform, With<Camera>>,
    mut st: ResMut<GraphState>,
    mut gizmos: Gizmos,
) {
    if !contexts.ctx_mut().wants_keyboard_input() && keys.just_pressed(KeyCode::KeyG) {
        if let Ok(tf) = cam_q.get_single() {
            scan.active = true;
            scan.radius = 0.0;
            scan.origin = tf.translation;
        }
    }
    if !scan.active {
        return;
    }
    scan.radius += st.cfg.scan_speed * time.delta_seconds();
    gizmos.sphere(scan.origin, Quat::IDENTITY, scan.radius, theme::PROCESS);

    let lo = scan.radius - SCAN_BAND;
    let hi = scan.radius;
    let origin = scan.origin;
    let hits: Vec<NodeId> = st
        .spatial
        .placed_positions()
        .filter(|(_, p)| {
            let d = (*p - origin).length();
            d >= lo && d <= hi
        })
        .map(|(id, _)| id.clone())
        .collect();
    let until = Instant::now() + st.cfg.glow_duration;
    for id in hits {
        st.spatial.set_node_glow(&id, until);
        st.revealed.insert(id); // scanning reveals (fog-of-war)
    }
    st.needs_redraw
        .store(true, std::sync::atomic::Ordering::Relaxed);

    if scan.radius > st.cfg.scan_max {
        scan.active = false;
    }
}

// ---- Incident-hunt mission ----

/// A simple threat-hunting loop: "investigate the alerted host". Pick the
/// newest alert, the player selects its target → score (faster = more points).
#[derive(Resource, Default)]
pub struct Mission {
    pub active: bool,
    pub target: Option<NodeId>,
    pub signature: String,
    pub score: u32,
    pub started: Option<Instant>,
    pub last_message: String,
}

/// Points awarded for resolving an incident in `secs` seconds.
pub fn mission_bonus(secs: f32) -> u32 {
    let base = 100.0;
    (base - secs.clamp(0.0, 90.0)).max(10.0) as u32
}

/// The investigation target of an alert: the node its `alerts_on` edge points to.
fn alert_target(st: &GraphState, alert: &NodeId) -> Option<NodeId> {
    st.model
        .edges_for_node(alert)
        .find(|e| &e.from == alert && EdgeKindClass::from_kind(&e.kind) == EdgeKindClass::AlertsOn)
        .map(|e| e.to.clone())
}

pub fn mission_tick(
    mut mission: ResMut<Mission>,
    keys: Res<ButtonInput<KeyCode>>,
    mut contexts: EguiContexts,
    mut st: ResMut<GraphState>,
) {
    // Start / take next incident with M.
    if !contexts.ctx_mut().wants_keyboard_input() && keys.just_pressed(KeyCode::KeyM) {
        let next = st
            .alerts_newest_first()
            .find(|id| alert_target(&st, id).is_some())
            .cloned();
        if let Some(alert) = next {
            let target = alert_target(&st, &alert);
            let signature = match st.model.nodes.get(&alert) {
                Some(spacegraph_core::Node::Alert { signature, .. }) => signature.clone(),
                _ => "alert".to_string(),
            };
            mission.active = true;
            mission.target = target.clone();
            mission.signature = signature;
            mission.started = Some(Instant::now());
            mission.last_message = "Investigate: select the alerted host".to_string();
            if let Some(t) = target {
                st.request_jump(t); // fly toward the lead
            }
        } else {
            mission.active = false;
            mission.last_message = "No alerts to investigate".to_string();
        }
    }

    // Completion: the player selected the target (single or box-select).
    if mission.active {
        if let Some(target) = mission.target.clone() {
            let hit =
                st.ui.selected.as_ref() == Some(&target) || st.ui.multi_selected.contains(&target);
            if hit {
                let secs = mission
                    .started
                    .map(|t| Instant::now().duration_since(t).as_secs_f32())
                    .unwrap_or(0.0);
                let bonus = mission_bonus(secs);
                mission.score += bonus;
                mission.active = false;
                mission.target = None;
                mission.last_message = format!("Resolved! +{bonus} (press M for next)");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faster_resolution_scores_higher() {
        assert!(mission_bonus(1.0) > mission_bonus(60.0));
        assert_eq!(mission_bonus(1000.0), 10); // clamped floor
        assert!(mission_bonus(0.0) <= 100);
    }
}
