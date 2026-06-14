//! Threat-motion vocabulary (D2-core, ADR-0009) + purple-team origin.
//!
//! Each attack class gets a distinct motion keyed off its ATT&CK **tactic** (the
//! D1/ADR-0006 enum), so an operator reads *what kind* of activity is moving from
//! across the scene. Motion is a pure classifier (`motion_style`) over the tactic;
//! the per-frame animation that consumes it is render-only and degrades to static
//! under Minimal. **Purple-team origin** disambiguates authorized red-team activity
//! (observed via a Nebula stream) from real threats — a viewer-side field derived
//! from the emitting stream, **no wire change** (O-8).

use crate::graph::rules::Tactic;
use crate::util::config::VisualTheme;

/// The motion an attack class exhibits, keyed off its ATT&CK tactic (ADR-0009).
/// A pure classifier mirroring `render::spatial::aperture_style`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionStyle {
    /// No motion (default / Minimal theme).
    Static,
    /// Command-and-control: a periodic beacon pulse.
    BeaconPulse,
    /// Lateral movement: a traversal sweep along edges.
    TraversalSweep,
    /// Exfiltration: an outbound-weighted flow.
    OutboundFlow,
    /// Credential access / brute force: rapid edge flashes.
    RapidFlash,
    /// Execution / impact: a worm-spread along edges.
    WormSpread,
}

impl MotionStyle {
    /// `(speed, amplitude)` motion constants — no per-call magic numbers in render.
    pub fn params(self) -> (f32, f32) {
        match self {
            MotionStyle::Static => (0.0, 0.0),
            MotionStyle::BeaconPulse => (2.0, 0.25),
            MotionStyle::TraversalSweep => (1.2, 0.40),
            MotionStyle::OutboundFlow => (1.6, 0.30),
            MotionStyle::RapidFlash => (6.0, 0.20),
            MotionStyle::WormSpread => (0.9, 0.50),
        }
    }
}

/// Select the motion for a tactic. Total over [`Tactic`]; the renderer forces
/// [`MotionStyle::Static`] under Minimal via [`motion_style_themed`].
pub fn motion_style(tactic: Tactic) -> MotionStyle {
    match tactic {
        Tactic::CommandAndControl => MotionStyle::BeaconPulse,
        Tactic::LateralMovement => MotionStyle::TraversalSweep,
        Tactic::Exfiltration => MotionStyle::OutboundFlow,
        Tactic::CredentialAccess => MotionStyle::RapidFlash,
        Tactic::Execution | Tactic::Impact => MotionStyle::WormSpread,
        // Remaining tactics have no distinct motion yet → calm beacon as a default
        // "something is here" cue; extended as the rule corpus grows.
        _ => MotionStyle::BeaconPulse,
    }
}

/// Theme-aware motion: Minimal is always static (motion is a Standard-only cue).
pub fn motion_style_themed(tactic: Tactic, theme: VisualTheme) -> MotionStyle {
    match theme {
        VisualTheme::Minimal => MotionStyle::Static,
        VisualTheme::Standard => motion_style(tactic),
    }
}

/// Whether a node/edge represents authorized red-team activity or a real
/// observation (ADR-0009). Derived viewer-side from the emitting stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Observed live telemetry / real-world activity.
    Observed,
    /// Authorized purple-/red-team activity (observed via a Nebula stream).
    RedTeam,
}

impl Origin {
    pub fn label(self) -> &'static str {
        match self {
            Origin::Observed => "observed",
            Origin::RedTeam => "red-team",
        }
    }
}

/// Classify a node's origin from its emitting **stream** key (the per-connection
/// namespace). A stream that marks itself as a red-team / Nebula feed is
/// `RedTeam`; everything else is `Observed`. This is the no-wire mechanism:
/// deploy the Nebula log source as its own agent stream named `nebula-*` /
/// `red-team-*` (ADR-0009), and its emitted entities style as red-team.
pub fn origin_of(stream: &str) -> Origin {
    let s = stream.to_ascii_lowercase();
    if s.contains("nebula") || s.contains("red-team") || s.contains("redteam") {
        Origin::RedTeam
    } else {
        Origin::Observed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tactic_maps_to_a_motion() {
        for t in Tactic::ALL {
            // Standard: a real motion; Minimal: always static.
            let _ = motion_style(t);
            assert_eq!(
                motion_style_themed(t, VisualTheme::Minimal),
                MotionStyle::Static
            );
        }
    }

    #[test]
    fn key_tactics_get_their_signature_motion() {
        assert_eq!(
            motion_style(Tactic::CommandAndControl),
            MotionStyle::BeaconPulse
        );
        assert_eq!(
            motion_style(Tactic::LateralMovement),
            MotionStyle::TraversalSweep
        );
        assert_eq!(
            motion_style(Tactic::Exfiltration),
            MotionStyle::OutboundFlow
        );
        // Standard keeps the motion; Minimal flattens it.
        assert_eq!(
            motion_style_themed(Tactic::Exfiltration, VisualTheme::Standard),
            MotionStyle::OutboundFlow
        );
    }

    #[test]
    fn static_motion_has_zero_params() {
        assert_eq!(MotionStyle::Static.params(), (0.0, 0.0));
        assert!(MotionStyle::BeaconPulse.params().0 > 0.0);
    }

    #[test]
    fn origin_red_team_only_for_marked_streams() {
        assert_eq!(origin_of("nebula-lab"), Origin::RedTeam);
        assert_eq!(origin_of("red-team-01"), Origin::RedTeam);
        assert_eq!(origin_of("RedTeam"), Origin::RedTeam);
        assert_eq!(origin_of("host-prod"), Origin::Observed);
        assert_eq!(origin_of(""), Origin::Observed);
    }
}
