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
/// Host / Container — violet (reserved for Phase 7 network nodes).
pub const HOST: Color = Color::srgb(0.65, 0.45, 0.98);
/// Alert / threat — red (reserved for Phase 8).
pub const ALERT: Color = Color::srgb(0.98, 0.22, 0.25);
/// Recent-activity flash colour; decays back to the node's type colour.
pub const RECENT_GLOW: Color = Color::srgb(1.0, 1.0, 1.0);

// ---- Edge class colours ----
pub const EDGE_OPENS: Color = Color::srgb(0.25, 0.95, 0.45); // green
pub const EDGE_EXECS: Color = Color::srgb(0.20, 0.85, 0.95); // cyan
pub const EDGE_RUNS_AS: Color = Color::srgb(0.98, 0.75, 0.25); // amber

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
}

impl NodeKind {
    /// All kinds, in stable index order (used to build per-kind material ramps).
    pub const ALL: [NodeKind; 3] = [NodeKind::Process, NodeKind::File, NodeKind::User];

    pub fn of(node: &Node) -> Self {
        match node {
            Node::Process { .. } => NodeKind::Process,
            Node::File { .. } => NodeKind::File,
            Node::User { .. } => NodeKind::User,
        }
    }

    pub fn index(self) -> usize {
        match self {
            NodeKind::Process => 0,
            NodeKind::File => 1,
            NodeKind::User => 2,
        }
    }

    pub fn base_color(self) -> Color {
        match self {
            NodeKind::Process => PROCESS,
            NodeKind::File => FILE,
            NodeKind::User => USER,
        }
    }
}

/// Colour for an aggregated edge class.
pub fn edge_color(class: EdgeKindClass) -> Color {
    match class {
        EdgeKindClass::Opens => EDGE_OPENS,
        EdgeKindClass::Execs => EDGE_EXECS,
        EdgeKindClass::RunsAs => EDGE_RUNS_AS,
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
        assert_eq!(idx, vec![0, 1, 2]);
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
