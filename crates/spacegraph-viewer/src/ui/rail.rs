//! Slim command rail (MP-UI-GitS, P2) — replaces the permanent left dev sidebar.
//!
//! A narrow vertical icon strip floating over the full-width graph; each button
//! toggles a corner-anchored GitS HUD panel (see [`crate::ui::hud_panels`]). The
//! rail owns no controls itself — it is the launcher; the panels carry every
//! control that used to live in the sidebar. Graph renders full-width when no
//! panel is open.

use bevy::prelude::{Res, ResMut, Resource};
use bevy_egui::{egui, EguiContexts};

use crate::graph::GraphState;
use crate::ui::overlay::layer;
use crate::ui::tokens::color;
use crate::ui::{gits, UiLayout};
use crate::util::config::VisualTheme;

/// Width (px) reserved at the screen's left edge for the rail.
pub const RAIL_WIDTH: f32 = 60.0;
/// Y offset where the rail / HUD panels begin (clears the top status strip).
pub const TOP_OFFSET: f32 = 28.0;

/// The rail's icon-grouped sections; each toggles one HUD panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RailSection {
    View,
    Filter,
    Alerts,
    Agents,
    Settings,
}

/// Which HUD panel is currently expanded (UI state — not graph truth).
#[derive(Resource, Default)]
pub struct RailState {
    pub open: Option<RailSection>,
}

const SECTIONS: [(RailSection, &str); 5] = [
    (RailSection::View, "VIEW"),
    (RailSection::Filter, "FILT"),
    (RailSection::Alerts, "ALRT"),
    (RailSection::Agents, "AGNT"),
    (RailSection::Settings, "CFG"),
];

/// True when the docked right inspector will render this frame, so `content_rect`
/// must reserve its column (mirrors `inspector_overlay`'s gate). Pure — tested.
pub fn inspector_reserves(st: &GraphState) -> bool {
    st.ui.inspector_open
        && st.ui.focus_mode.is_none()
        && st.cfg.shell.right_open
        && (st.ui.selected.is_some() || st.ui.focus.is_some())
}

/// Publish the layout authority (P2): `content_rect` is the screen minus the slim
/// rail (left), the top status strip, and the docked inspector column (right, when
/// it will render) — so every floating panel that constrains to `content_rect`
/// clears them instead of stacking on a shared screen corner. Runs before the
/// panels so they read a fresh rect.
pub fn update_ui_layout(
    mut contexts: EguiContexts,
    mut layout: ResMut<UiLayout>,
    st: Res<GraphState>,
) {
    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };
    let screen = ctx.screen_rect();
    let rail_right = (screen.min.x + RAIL_WIDTH).min(screen.max.x);
    let top = (screen.min.y + TOP_OFFSET).min(screen.max.y);
    let inspector_w = if inspector_reserves(&st) {
        st.cfg.shell.right_width
    } else {
        0.0
    };
    let right = (screen.max.x - inspector_w).max(rail_right);
    layout.panel_rect = egui::Rect::from_min_max(screen.min, egui::pos2(rail_right, screen.max.y));
    layout.content_rect =
        egui::Rect::from_min_max(egui::pos2(rail_right, top), egui::pos2(right, screen.max.y));
}

/// Draw the rail and toggle the active HUD panel section.
pub fn command_rail(mut contexts: EguiContexts, mut rail: ResMut<RailState>, st: Res<GraphState>) {
    let standard = st.cfg.visual_theme == VisualTheme::Standard;
    let (low, med, high) = st.alert_severity_counts();
    let alerts = low + med + high;
    let agents = st.net.active_connection_count();
    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };
    egui::Area::new("command_rail".into())
        .order(layer::PANEL)
        .anchor(egui::Align2::LEFT_TOP, [6.0, TOP_OFFSET])
        .show(ctx, |ui| {
            gits::panel_frame(standard).show(ui, |ui| {
                ui.set_width(RAIL_WIDTH - 14.0);
                ui.vertical_centered_justified(|ui| {
                    for (sec, label) in SECTIONS {
                        let active = rail.open == Some(sec);
                        let badge = match sec {
                            RailSection::Alerts if alerts > 0 => Some(alerts),
                            RailSection::Agents if agents > 0 => Some(agents),
                            _ => None,
                        };
                        if rail_button(ui, sec, label, active, badge, standard).clicked() {
                            rail.open = if active { None } else { Some(sec) };
                        }
                        ui.add_space(3.0);
                    }
                });
            });
        });
}

fn rail_button(
    ui: &mut egui::Ui,
    sec: RailSection,
    label: &str,
    active: bool,
    badge: Option<usize>,
    standard: bool,
) -> egui::Response {
    let w = RAIL_WIDTH - 16.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 40.0), egui::Sense::click());
    let painter = ui.painter();
    let accent = if standard { color::ACCENT } else { color::TEXT };
    let accent_hi = if standard {
        color::ACCENT_HI
    } else {
        color::TEXT
    };

    // Background + a left accent bar marks the active section (hover = faint fill).
    if active {
        painter.rect_filled(rect, 2.0, color::SURFACE_HI);
        painter.rect_filled(
            egui::Rect::from_min_size(rect.left_top(), egui::vec2(2.5, rect.height())),
            0.0,
            accent_hi,
        );
    } else if resp.hovered() {
        painter.rect_filled(
            rect,
            2.0,
            egui::Color32::from_rgba_unmultiplied(
                color::SURFACE_HI.r(),
                color::SURFACE_HI.g(),
                color::SURFACE_HI.b(),
                110,
            ),
        );
    }

    let icon_col = if active { accent_hi } else { accent };
    draw_rail_icon(
        painter,
        egui::pos2(rect.center().x, rect.top() + 13.0),
        8.5,
        sec,
        icon_col,
    );
    painter.text(
        egui::pos2(rect.center().x, rect.bottom() - 4.0),
        egui::Align2::CENTER_BOTTOM,
        label,
        egui::FontId::monospace(9.0),
        if active { accent_hi } else { color::TEXT_DIM },
    );

    // Severity-coloured badge pill (top-right), separate from the icon tint.
    if let Some(n) = badge {
        let bc = egui::pos2(rect.right() - 7.0, rect.top() + 6.0);
        painter.circle_filled(bc, 7.0, color::SEV_HIGH);
        painter.text(
            bc,
            egui::Align2::CENTER_CENTER,
            n.to_string(),
            egui::FontId::monospace(8.0),
            egui::Color32::WHITE,
        );
    }
    resp.on_hover_text(label)
}

/// Draw a simple per-section vector icon centred at `c` with half-extent `r`.
fn draw_rail_icon(p: &egui::Painter, c: egui::Pos2, r: f32, sec: RailSection, col: egui::Color32) {
    use std::f32::consts::{FRAC_PI_2, TAU};
    let s = egui::Stroke::new(1.4, col);
    match sec {
        RailSection::View => {
            // eye — ring + pupil
            p.circle_stroke(c, r, s);
            p.circle_filled(c, r * 0.34, col);
        }
        RailSection::Filter => {
            // funnel
            p.add(egui::Shape::closed_line(
                vec![
                    egui::pos2(c.x - r, c.y - r),
                    egui::pos2(c.x + r, c.y - r),
                    egui::pos2(c.x + r * 0.22, c.y),
                    egui::pos2(c.x + r * 0.22, c.y + r),
                    egui::pos2(c.x - r * 0.22, c.y + r * 0.55),
                    egui::pos2(c.x - r * 0.22, c.y),
                ],
                s,
            ));
        }
        RailSection::Alerts => {
            // warning triangle + exclamation
            p.add(egui::Shape::closed_line(
                vec![
                    egui::pos2(c.x, c.y - r),
                    egui::pos2(c.x + r, c.y + r),
                    egui::pos2(c.x - r, c.y + r),
                ],
                s,
            ));
            p.line_segment(
                [
                    egui::pos2(c.x, c.y - r * 0.25),
                    egui::pos2(c.x, c.y + r * 0.35),
                ],
                s,
            );
            p.circle_filled(egui::pos2(c.x, c.y + r * 0.7), 1.0, col);
        }
        RailSection::Agents => {
            // hexagon node
            let pts: Vec<egui::Pos2> = (0..6)
                .map(|i| {
                    let a = TAU * (i as f32 / 6.0) - FRAC_PI_2;
                    egui::pos2(c.x + a.cos() * r, c.y + a.sin() * r)
                })
                .collect();
            p.add(egui::Shape::closed_line(pts, s));
        }
        RailSection::Settings => {
            // gear — ring + spokes
            p.circle_stroke(c, r * 0.5, s);
            for i in 0..6 {
                let a = TAU * (i as f32 / 6.0);
                let d = egui::vec2(a.cos(), a.sin());
                p.line_segment(
                    [
                        egui::pos2(c.x + d.x * r * 0.62, c.y + d.y * r * 0.62),
                        egui::pos2(c.x + d.x * r, c.y + d.y * r),
                    ],
                    s,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spacegraph_core::NodeId;

    #[test]
    fn inspector_reserves_only_when_visible() {
        let mut st = GraphState::default();
        st.ui.inspector_open = true;
        st.cfg.shell.right_open = true;
        assert!(
            !inspector_reserves(&st),
            "no selection → nothing to inspect"
        );
        st.ui.selected = Some(NodeId("n".into()));
        assert!(
            inspector_reserves(&st),
            "selection + open → reserves a column"
        );
        st.ui.focus_mode = Some(NodeId("n".into()));
        assert!(
            !inspector_reserves(&st),
            "focus mode suppresses the inspector (the card is the surface)"
        );
        st.ui.focus_mode = None;
        st.cfg.shell.right_open = false;
        assert!(!inspector_reserves(&st), "right_open=false → no reserve");
    }
}
