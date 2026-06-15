//! Visual design language — the single source of truth for colours.
//!
//! See `docs/DESIGN_LANGUAGE.md` for the rationale. Every colour the renderer
//! uses lives here so the palette can be tuned in one place. Node and edge
//! colours are deliberately high-chroma neon on near-black space for the
//! "Ghost in the Shell" look; with the HDR + bloom camera the emissive
//! channels (which can exceed 1.0) are what glow.

use bevy::prelude::Color;
use spacegraph_core::Node;

use crate::graph::model::EdgeKindClass;

// ---- Node type base colours ----
/// Process — cyan.
pub const PROCESS: Color = Color::srgb(0.20, 0.85, 0.95);
/// File — green.
pub const FILE: Color = Color::srgb(0.25, 0.95, 0.45);
/// User — amber.
pub const USER: Color = Color::srgb(0.98, 0.75, 0.25);
/// Socket — blue (network layer).
pub const SOCKET: Color = Color::srgb(0.30, 0.60, 0.98);
/// Host / Container / RemoteHost — violet (network layer).
pub const HOST: Color = Color::srgb(0.70, 0.55, 0.99);
/// Alert / threat — red.
pub const ALERT: Color = Color::srgb(0.98, 0.22, 0.25);
/// Alert severity ramp: low = amber, medium = orange, high/critical = red.
pub const ALERT_LOW: Color = Color::srgb(0.98, 0.75, 0.25);
pub const ALERT_MEDIUM: Color = Color::srgb(0.99, 0.50, 0.15);
pub const ALERT_HIGH: Color = Color::srgb(1.0, 0.18, 0.20);

/// Colour for an alert severity string ("low" | "medium" | "high" | …).
pub fn alert_severity_color(severity: &str) -> Color {
    match severity {
        "low" => ALERT_LOW,
        "medium" => ALERT_MEDIUM,
        _ => ALERT_HIGH, // high / critical / unknown → red
    }
}
/// Recent-activity flash colour; decays back to the node's type colour.
pub const RECENT_GLOW: Color = Color::srgb(1.0, 1.0, 1.0);

// ---- Lock-on reticle / selection feedback ----
/// Hovered node reticle / bubble (light cyan-white).
pub const RETICLE_HOVER: Color = Color::srgb(0.90, 0.95, 1.0);
/// Selected node reticle / bubble (cyan).
pub const RETICLE_SELECT: Color = Color::srgb(0.25, 0.95, 1.0);
/// Focused node reticle / bubble (teal).
pub const RETICLE_FOCUS: Color = Color::srgb(0.20, 1.0, 0.85);
/// Marked node tint (magenta).
pub const MARKED: Color = Color::srgb(0.95, 0.35, 0.85);
/// Pinned node marker (dimmed amber).
pub const PINNED: Color = Color::srgb(0.75, 0.6, 0.25);
/// Hovered edge highlight (bright white-cyan).
pub const EDGE_HOVER: Color = Color::srgb(0.8, 1.0, 1.0);

// ---- Edge class colours ----
pub const EDGE_OPENS: Color = Color::srgb(0.25, 0.95, 0.45); // green
pub const EDGE_EXECS: Color = Color::srgb(0.20, 0.85, 0.95); // cyan
pub const EDGE_RUNS_AS: Color = Color::srgb(0.98, 0.75, 0.25); // amber
pub const EDGE_OWNS_SOCKET: Color = Color::srgb(0.30, 0.60, 0.98); // blue
pub const EDGE_CONNECTS_TO: Color = Color::srgb(0.40, 0.70, 1.0); // bright blue
pub const EDGE_LISTENS_ON: Color = Color::srgb(0.30, 0.85, 0.85); // teal

// ---- Perimeter & exposure (D0, ADR-0012) ----
/// Aperture tint by port state. Open (LISTEN) glows outward; active
/// (ESTABLISHED) keeps the socket blue; shuttered (gated/filtered) dims behind a
/// barrier ring; closing dims toward neutral.
pub const APERTURE_OPEN: Color = Color::srgb(0.45, 0.85, 1.0);
pub const APERTURE_ACTIVE: Color = SOCKET;
pub const APERTURE_SHUTTERED: Color = Color::srgb(0.30, 0.34, 0.45);
pub const APERTURE_CLOSING: Color = Color::srgb(0.26, 0.40, 0.55);
/// Barrier ring around a shuttered/gated aperture (dormant until the D2 firewall
/// source emits a gated state).
pub const BARRIER_RING: Color = Color::srgb(0.95, 0.55, 0.20);
/// Gateway (default-route `RemoteHost`) accent.
pub const GATEWAY_ACCENT: Color = Color::srgb(0.55, 0.80, 1.0);
/// Exposure tints (informational; radial depth is the primary cue).
pub const EXPOSURE_LOOPBACK: Color = Color::srgb(0.30, 0.85, 0.55);
pub const EXPOSURE_LAN: Color = Color::srgb(0.40, 0.70, 1.0);
pub const EXPOSURE_PUBLIC: Color = Color::srgb(1.0, 0.55, 0.25);

// ---- Timeline event-marker colours ----
pub const TL_NODE_UPSERT: Color = FILE; // green
pub const TL_NODE_REMOVE: Color = ALERT; // red
pub const TL_EDGE_UPSERT: Color = PROCESS; // cyan
pub const TL_EDGE_REMOVE: Color = USER; // amber
pub const TL_BATCH: Color = Color::srgb(0.75, 0.78, 0.85); // neutral

// ---- Scene dressing ----
/// Near-black space background for the Standard theme.
pub const CLEAR_STANDARD: Color = Color::srgb(0.004, 0.012, 0.025);
/// Distance fog colour (matches the clear colour so geometry fades to space).
pub const FOG_STANDARD: Color = Color::srgb(0.004, 0.012, 0.025);
/// Faint floor-grid line colour.
pub const GRID_LINE: Color = Color::srgb(0.07, 0.13, 0.20);
/// Flat dark background for the Minimal theme.
pub const CLEAR_MINIMAL: Color = Color::srgb(0.05, 0.05, 0.06);

/// Coarse node category used to index per-type material ramps.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum NodeKind {
    Process,
    File,
    User,
    Socket,
    RemoteHost,
    Alert,
}

impl NodeKind {
    /// All kinds, in stable index order (used to build per-kind material ramps).
    pub const ALL: [NodeKind; 6] = [
        NodeKind::Process,
        NodeKind::File,
        NodeKind::User,
        NodeKind::Socket,
        NodeKind::RemoteHost,
        NodeKind::Alert,
    ];

    pub fn of(node: &Node) -> Self {
        match node {
            Node::Process { .. } => NodeKind::Process,
            Node::File { .. } => NodeKind::File,
            Node::User { .. } => NodeKind::User,
            Node::Socket { .. } => NodeKind::Socket,
            Node::RemoteHost { .. } => NodeKind::RemoteHost,
            Node::Alert { .. } => NodeKind::Alert,
        }
    }

    pub fn index(self) -> usize {
        match self {
            NodeKind::Process => 0,
            NodeKind::File => 1,
            NodeKind::User => 2,
            NodeKind::Socket => 3,
            NodeKind::RemoteHost => 4,
            NodeKind::Alert => 5,
        }
    }

    pub fn base_color(self) -> Color {
        match self {
            NodeKind::Process => PROCESS,
            NodeKind::File => FILE,
            NodeKind::User => USER,
            NodeKind::Socket => SOCKET,
            NodeKind::RemoteHost => HOST,
            NodeKind::Alert => ALERT,
        }
    }

    /// True for network-layer nodes that should sit on an outer shell.
    pub fn is_network(self) -> bool {
        matches!(self, NodeKind::Socket | NodeKind::RemoteHost)
    }
}

/// Colour for an aggregated edge class.
pub fn edge_color(class: EdgeKindClass) -> Color {
    match class {
        EdgeKindClass::Opens => EDGE_OPENS,
        EdgeKindClass::Execs => EDGE_EXECS,
        EdgeKindClass::RunsAs => EDGE_RUNS_AS,
        EdgeKindClass::OwnsSocket => EDGE_OWNS_SOCKET,
        EdgeKindClass::ConnectsTo => EDGE_CONNECTS_TO,
        EdgeKindClass::ListensOn => EDGE_LISTENS_ON,
        EdgeKindClass::AlertsOn => ALERT,
    }
}

/// Linearly interpolate two colours in linear-RGB space at `t` ∈ [0,1].
pub fn lerp(a: Color, b: Color, t: f32) -> Color {
    let a = a.to_linear();
    let b = b.to_linear();
    let t = t.clamp(0.0, 1.0);
    Color::linear_rgb(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use spacegraph_core::FileKind;

    #[test]
    fn node_kind_indices_are_stable_and_distinct() {
        let idx: Vec<usize> = NodeKind::ALL.iter().map(|k| k.index()).collect();
        assert_eq!(idx, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn node_kind_of_maps_each_variant() {
        let p = Node::Process {
            pid: 1,
            ppid: 0,
            exe: String::new(),
            cmdline: String::new(),
            uid: 0,
        };
        let f = Node::File {
            path: String::new(),
            inode: 0,
            kind: FileKind::Regular,
        };
        let u = Node::User {
            uid: 0,
            name: String::new(),
        };
        assert_eq!(NodeKind::of(&p), NodeKind::Process);
        assert_eq!(NodeKind::of(&f), NodeKind::File);
        assert_eq!(NodeKind::of(&u), NodeKind::User);
    }

    #[test]
    fn lerp_endpoints() {
        let a = Color::linear_rgb(0.0, 0.0, 0.0);
        let b = Color::linear_rgb(1.0, 1.0, 1.0);
        let mid = lerp(a, b, 0.5).to_linear();
        assert!((mid.red - 0.5).abs() < 1e-5);
    }
}
