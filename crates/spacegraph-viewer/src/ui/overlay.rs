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

use bevy_egui::egui::{Align, Align2, Pos2, Rect, Vec2};

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

/// Middle-ellipsis truncation for long paths/labels: keep the head and the tail,
/// elide the middle with `…`, never exceeding `max_chars` (the `…` counts). The
/// tail is favoured (`≥` the head) so the load-bearing basename survives. Operates
/// on `char`s, never byte slices — paths carry multibyte / `⚠` prefixes, so this is
/// char-boundary safe. Pure — unit-tested.
pub fn middle_truncate(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    if n <= max_chars {
        return s.to_string();
    }
    if max_chars <= 1 {
        return "…".to_string();
    }
    let keep = max_chars - 1; // room for the ellipsis
    let tail = keep.div_ceil(2); // tail (basename) gets the extra char
    let head = keep - tail;
    let head_s: String = chars[..head].iter().collect();
    let tail_s: String = chars[n - tail..].iter().collect();
    format!("{head_s}…{tail_s}")
}

/// Greedily de-collide screen labels: each label keeps its anchor unless it would
/// overlap an already-placed one, in which case it is nudged straight down until
/// clear (bounded), then **clamped fully inside `vp`** so a long nudge stack can't
/// push a label off-screen. Returns the resolved top-left per input, in order.
/// Pure — the node-label anti-overlap pass.
pub fn decollide_labels(anchors: &[(Pos2, Vec2)], vp: Rect) -> Vec<Pos2> {
    let mut placed: Vec<Rect> = Vec::with_capacity(anchors.len());
    let mut out = Vec::with_capacity(anchors.len());
    for &(anchor, size) in anchors {
        let mut p = anchor;
        for _ in 0..24 {
            let r = Rect::from_min_size(p, size);
            if placed.iter().any(|q| q.intersects(r)) {
                p.y += size.y + 2.0;
            } else {
                break;
            }
        }
        // On-screen clamp (mirrors `place_card`). `.max(vp.min)` keeps the range
        // valid when the viewport is smaller than the label, so `clamp` never panics.
        p.x = p.x.clamp(vp.min.x, (vp.max.x - size.x).max(vp.min.x));
        p.y = p.y.clamp(vp.min.y, (vp.max.y - size.y).max(vp.min.y));
        placed.push(Rect::from_min_size(p, size));
        out.push(p);
    }
    out
}

/// Top-left position for a panel of `size` anchored to a `content` rect corner with
/// `margin` clearance. The single content_rect-aware corner rule — so floating
/// panels land in their own zone (rail/inspector-clear) instead of stacking on a
/// shared screen corner. Pure — the placement math the panel-layer asserts.
pub fn corner_anchor(content: Rect, align: Align2, size: Vec2, margin: Vec2) -> Pos2 {
    let x = match align.x() {
        Align::Min => content.min.x + margin.x,
        Align::Center => content.center().x - size.x * 0.5,
        Align::Max => content.max.x - size.x - margin.x,
    };
    let y = match align.y() {
        Align::Min => content.min.y + margin.y,
        Align::Center => content.center().y - size.y * 0.5,
        Align::Max => content.max.y - size.y - margin.y,
    };
    Pos2::new(x, y)
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

    #[test]
    fn middle_truncate_keeps_head_tail_and_respects_max() {
        // Short strings pass through untouched.
        assert_eq!(middle_truncate("short.log", 20), "short.log");
        // Long path: middle elided, never longer than max, basename tail survives.
        let p = "/synthetic/dir005/file000322.dat";
        let t = middle_truncate(p, 24);
        assert!(t.chars().count() <= 24, "respects max_chars: {t:?}");
        assert!(t.contains('…'), "has an ellipsis: {t:?}");
        assert!(t.starts_with("/synthetic"), "keeps the head: {t:?}");
        assert!(t.ends_with(".dat"), "keeps the basename tail: {t:?}");
        // Degenerate budgets never panic / never slice mid-char.
        assert_eq!(middle_truncate("⚠ alert/✓/path", 1), "…");
        let multibyte = middle_truncate("⚠/réseau/socket/établi:8080", 10);
        assert!(multibyte.chars().count() <= 10);
    }

    #[test]
    fn corner_panels_do_not_overlap() {
        // content_rect with the rail (left) + top strip reserved, inspector closed.
        let content = Rect::from_min_max(pos2(66.0, 28.0), pos2(1600.0, 1000.0));
        let ar = |a, size, m| rect(corner_anchor(content, a, size, m), size);
        let minimap = ar(Align2::RIGHT_TOP, vec2(160.0, 180.0), vec2(12.0, 12.0));
        let card = ar(Align2::RIGHT_BOTTOM, vec2(290.0, 320.0), vec2(14.0, 14.0));
        let telemetry = ar(Align2::LEFT_BOTTOM, vec2(230.0, 96.0), vec2(10.0, 10.0));
        assert!(
            !overlaps(minimap, card),
            "minimap (top) clears the card (bottom)"
        );
        assert!(
            !overlaps(card, telemetry),
            "card (right) clears telemetry (left)"
        );
        assert!(!overlaps(minimap, telemetry), "minimap clears telemetry");
        for r in [minimap, card, telemetry] {
            assert!(
                on_screen(r, content),
                "panel stays inside content_rect: {r:?}"
            );
        }
    }

    #[test]
    fn decollide_labels_separates_overlapping_keeps_far_apart() {
        let a = pos2(100.0, 100.0);
        let size = vec2(80.0, 16.0);
        let out = decollide_labels(&[(a, size), (a, size), (a, size)], vp());
        assert_eq!(out[0], a, "first keeps its anchor");
        assert!(
            out[1].y > out[0].y && out[2].y > out[1].y,
            "stacked downward"
        );
        let rects: Vec<Rect> = out.iter().map(|&p| rect(p, size)).collect();
        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                assert!(!overlaps(rects[i], rects[j]), "labels {i},{j} disjoint");
            }
        }
        // Far-apart labels are left untouched.
        let far = decollide_labels(&[(pos2(0.0, 0.0), size), (pos2(500.0, 500.0), size)], vp());
        assert_eq!(far[1], pos2(500.0, 500.0), "no needless nudging");
    }

    #[test]
    fn decollide_labels_stay_on_screen_under_pressure() {
        // Many labels anchored at the very bottom would stack off-screen without a
        // clamp; every resolved label must stay inside the viewport.
        let size = vec2(80.0, 16.0);
        let anchors: Vec<(Pos2, Vec2)> = (0..12).map(|_| (pos2(60.0, 990.0), size)).collect();
        let out = decollide_labels(&anchors, vp());
        for p in &out {
            assert!(
                on_screen(rect(*p, size), vp()),
                "label clamped on-screen: {p:?}"
            );
        }
    }

    #[test]
    fn corner_anchor_clears_a_reserved_inspector_column() {
        // When content_rect's right edge is pulled in by the inspector, the
        // right-anchored card moves left with it (never under the inspector).
        let full = Rect::from_min_max(pos2(66.0, 28.0), pos2(1600.0, 1000.0));
        let narrowed = Rect::from_min_max(pos2(66.0, 28.0), pos2(1280.0, 1000.0));
        let size = vec2(290.0, 320.0);
        let m = vec2(14.0, 14.0);
        let wide = corner_anchor(full, Align2::RIGHT_BOTTOM, size, m);
        let tight = corner_anchor(narrowed, Align2::RIGHT_BOTTOM, size, m);
        assert!(
            tight.x < wide.x,
            "card shifts left to clear the inspector column"
        );
        assert!(
            tight.x + size.x <= narrowed.max.x,
            "card stays within content_rect"
        );
    }
}
