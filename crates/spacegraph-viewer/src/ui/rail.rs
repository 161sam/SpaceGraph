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

const SECTIONS: [(RailSection, &str, &str); 5] = [
    (RailSection::View, "◳", "VIEW"),
    (RailSection::Filter, "⌕", "FILT"),
    (RailSection::Alerts, "⚠", "ALRT"),
    (RailSection::Agents, "⬡", "AGNT"),
    (RailSection::Settings, "⚙", "CFG"),
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
                    for (sec, glyph, label) in SECTIONS {
                        let active = rail.open == Some(sec);
                        let badge = match sec {
                            RailSection::Alerts if alerts > 0 => Some(alerts),
                            RailSection::Agents if agents > 0 => Some(agents),
                            _ => None,
                        };
                        if rail_button(ui, glyph, label, active, badge, standard).clicked() {
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
    glyph: &str,
    label: &str,
    active: bool,
    badge: Option<usize>,
    standard: bool,
) -> egui::Response {
    let top = match badge {
        Some(n) => format!("{glyph}{n}"),
        None => glyph.to_string(),
    };
    let mut text = egui::RichText::new(format!("{top}\n{label}"))
        .monospace()
        .size(11.0);
    if active && standard {
        text = text.color(color::ACCENT);
    } else if badge.is_some() && standard {
        text = text.color(color::SEV_HIGH);
    }
    ui.add(
        egui::Button::new(text)
            .min_size(egui::vec2(RAIL_WIDTH - 16.0, 34.0))
            .selected(active),
    )
    .on_hover_text(label)
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
