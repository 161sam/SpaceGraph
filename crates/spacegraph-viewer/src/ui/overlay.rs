//! Panel-layer & anchoring authority (MP-UI-GitS, P1).
//!
//! A single owner of on-screen overlay z-order and node-anchored placement, so
//! the radial HUD / preview / hover readout / reticle readout stop stacking
//! concentrically on the focused node. The crux is one pure, unit-tested
//! anchoring rule — [`place_card`] — reused by the hover tooltip, the reticle
//! selection readout, and (P3) the focus entity card.
//!
//! UI-only: no graph truth here (module boundary). egui paints
//! `Background < Middle < Foreground`; assigning each overlay a class in
//! [`layer`] removes the same-tier ambiguity that let those surfaces stack.

use bevy_egui::egui::{Pos2, Rect, Vec2};

/// Canonical egui draw-order per overlay class — the single z-order authority.
pub mod layer {
    use bevy_egui::egui::Order;

    /// Full-screen backing: the focus dim + centerpiece and the HUD frame.
    pub const BACKDROP: Order = Order::Background;
    /// Corner/docked panels & windows: entity card, preview, inspector, HUD
    /// panels, legend, minimap, command palette.
    pub const PANEL: Order = Order::Middle;
    /// Node-anchored transient readouts: hover tooltip + reticle selection
    /// readout (suppressed while a modal owns the node region — see
    /// [`super::hover_readout_suppressed`]).
    pub const READOUT: Order = Order::Middle;
    /// Modal, always on top: the radial HUD and the right-click context menu.
    pub const MODAL: Order = Order::Foreground;
}

/// Default clearance (px) between a node's visual envelope and a readout beside it.
pub const CARD_GAP: f32 = 14.0;

/// Approx. on-screen half-extent (px) of a node glyph plus its reticle brackets,
/// so a readout placed beside it clears the lock-on frame.
pub const NODE_HALF_PX: f32 = 44.0;

/// Place a card/readout beside a node so it never overlaps the node's on-screen
/// footprint and stays fully inside the viewport.
///
/// `node` = node centre (screen px); `node_half` = node footprint half-extent to
/// clear (use `0.0` to anchor beside a bare point such as the pointer); `size` =
/// card size (px); `vp` = viewport rect; `gap` = clearance.
///
/// Strategy: prefer the node's right; if the card would overflow the right edge,
/// flip to the left; vertically centre on the node; finally clamp fully
/// on-screen. For a node well inside a viewport at least
/// `2*(node_half+gap)+size.x` wide the result provably clears the node footprint.
/// Pure — unit-tested.
pub fn place_card(node: Pos2, node_half: f32, size: Vec2, vp: Rect, gap: f32) -> Pos2 {
    let clear = node_half + gap;
    let right_x = node.x + clear;
    let left_x = node.x - clear - size.x;

    let x = if right_x + size.x <= vp.max.x {
        right_x
    } else if left_x >= vp.min.x {
        left_x
    } else {
        // Neither side fully fits — keep the side with more room, then clamp.
        let room_right = vp.max.x - right_x;
        let room_left = (node.x - clear) - vp.min.x;
        if room_right >= room_left {
            right_x
        } else {
            left_x
        }
    };

    let y = node.y - size.y * 0.5;

    // Final on-screen clamp. `.max(vp.min.*)` keeps the clamp range valid (lo<=hi)
    // when the viewport is smaller than the card, so `clamp` never panics.
    let x = x.clamp(vp.min.x, (vp.max.x - size.x).max(vp.min.x));
    let y = y.clamp(vp.min.y, (vp.max.y - size.y).max(vp.min.y));
    Pos2::new(x, y)
}

/// Rough on-screen size (px) of a monospace readout box for `lines`, used to
/// anchor it with [`place_card`] before egui lays it out. Approximate by design
/// (a slightly-off estimate only shifts placement, never breaks the clamp).
pub fn estimate_text_size(lines: &[String]) -> Vec2 {
    let cols = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f32;
    let w = (cols * 7.2 + 16.0).clamp(80.0, 380.0);
    let h = lines.len().max(1) as f32 * 16.0 + 12.0;
    Vec2::new(w, h)
}

/// The transient hover readout yields to a modal/own-readout overlay: focus mode
/// (the radial + centerpiece own the node region) or an open right-click context
/// menu. Pure.
pub fn hover_readout_suppressed(focus_mode: bool, context_menu_open: bool) -> bool {
    focus_mode || context_menu_open
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_egui::egui::{pos2, vec2};

    // Local geometry helpers (avoid depending on egui Rect convenience methods
    // whose availability varies across versions).
    fn vp() -> Rect {
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1600.0, 1000.0))
    }
    fn rect(min: Pos2, size: Vec2) -> Rect {
        Rect::from_min_max(min, pos2(min.x + size.x, min.y + size.y))
    }
    fn node_rect(c: Pos2, half: f32) -> Rect {
        Rect::from_min_max(pos2(c.x - half, c.y - half), pos2(c.x + half, c.y + half))
    }
    fn on_screen(card: Rect, vp: Rect) -> bool {
        card.min.x >= vp.min.x
            && card.min.y >= vp.min.y
            && card.max.x <= vp.max.x
            && card.max.y <= vp.max.y
    }
    fn overlaps(a: Rect, b: Rect) -> bool {
        a.min.x < b.max.x && a.max.x > b.min.x && a.min.y < b.max.y && a.max.y > b.min.y
    }

    #[test]
    fn places_right_and_clears_node_when_room() {
        let c = pos2(800.0, 500.0);
        let size = vec2(240.0, 80.0);
        let p = place_card(c, NODE_HALF_PX, size, vp(), CARD_GAP);
        let card = rect(p, size);
        assert!(on_screen(card, vp()), "card on-screen");
        assert!(
            !overlaps(card, node_rect(c, NODE_HALF_PX)),
            "no overlap with node"
        );
        assert!(p.x >= c.x + NODE_HALF_PX, "placed to the right of the node");
    }

    #[test]
    fn flips_left_near_right_edge() {
        let c = pos2(1560.0, 500.0);
        let size = vec2(240.0, 80.0);
        let p = place_card(c, NODE_HALF_PX, size, vp(), CARD_GAP);
        let card = rect(p, size);
        assert!(
            on_screen(card, vp()),
            "card stays on-screen near the right edge"
        );
        assert!(
            !overlaps(card, node_rect(c, NODE_HALF_PX)),
            "no overlap with node"
        );
        assert!(p.x + size.x <= c.x - NODE_HALF_PX, "flipped to the left");
    }

    #[test]
    fn clamps_vertically_near_top_edge() {
        let c = pos2(800.0, 10.0);
        let size = vec2(240.0, 120.0);
        let p = place_card(c, NODE_HALF_PX, size, vp(), CARD_GAP);
        let card = rect(p, size);
        assert!(
            card.min.y >= vp().min.y && card.max.y <= vp().max.y,
            "on-screen vertically"
        );
    }

    #[test]
    fn clamps_into_tiny_viewport_without_panic() {
        let small = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0));
        let size = vec2(240.0, 80.0);
        let p = place_card(pos2(50.0, 50.0), 20.0, size, small, CARD_GAP);
        assert!(p.x >= small.min.x && p.y >= small.min.y);
        assert!(p.x <= small.max.x && p.y <= small.max.y);
    }

    #[test]
    fn point_anchor_offsets_from_pointer() {
        let c = pos2(400.0, 400.0);
        let size = vec2(180.0, 60.0);
        let p = place_card(c, 0.0, size, vp(), CARD_GAP);
        assert!(p.x >= c.x, "to the right of the pointer");
        assert!(on_screen(rect(p, size), vp()));
    }

    #[test]
    fn estimate_text_size_grows_with_lines_and_clamps_width() {
        let one = estimate_text_size(&["short".to_string()]);
        let many = estimate_text_size(&[
            "short".to_string(),
            "another".to_string(),
            "third".to_string(),
        ]);
        assert!(many.y > one.y, "height grows with line count");
        let wide = estimate_text_size(&["x".repeat(500)]);
        assert!((80.0..=380.0).contains(&wide.x), "width is clamped");
    }

    #[test]
    fn suppression_predicate() {
        assert!(
            hover_readout_suppressed(true, false),
            "focus mode suppresses"
        );
        assert!(
            hover_readout_suppressed(false, true),
            "open context menu suppresses"
        );
        assert!(!hover_readout_suppressed(false, false), "shown when idle");
    }
}
