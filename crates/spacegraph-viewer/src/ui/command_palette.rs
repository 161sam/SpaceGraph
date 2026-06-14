//! Command palette (v0.5.0, spec §3.7) — `Ctrl/Cmd+P` fuzzy palette over actions,
//! navigation and nodes. In-house subsequence fuzzy matcher (no new crate),
//! extending the node-only `ui/search.rs`.

use std::sync::atomic::Ordering;

use bevy::prelude::ResMut;
use bevy_egui::{egui, EguiContexts};
use spacegraph_core::NodeId;

use crate::graph::GraphState;
use crate::render::quality::{parse_tier, QualityState};
use crate::util::config::VisualTheme;

/// Cap on nodes scanned per keystroke (bounds the palette's per-frame cost).
const NODE_SCAN_CAP: usize = 4000;
const MAX_RESULTS: usize = 20;

/// Subsequence fuzzy match: `Some(score)` if every char of `needle` appears in
/// order in `haystack` (case-insensitive). Higher is better — contiguous runs,
/// an early/prefix match, and shorter haystacks score higher. Empty needle
/// matches everything (score 0). Pure + unit-tested.
pub fn fuzzy_match(needle: &str, haystack: &str) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    let n: Vec<char> = needle.to_lowercase().chars().collect();
    let mut ni = 0usize;
    let mut score = 0i32;
    let mut last: Option<usize> = None;
    let mut len = 0i32;
    for (hi, hc) in haystack.to_lowercase().chars().enumerate() {
        len = hi as i32 + 1;
        if ni < n.len() && hc == n[ni] {
            score += 10;
            if last.is_some_and(|l| hi == l + 1) {
                score += 15; // contiguous run
            }
            if hi == 0 {
                score += 12; // prefix bonus
            }
            last = Some(hi);
            ni += 1;
        }
    }
    (ni == n.len()).then_some(score - len / 4)
}

#[derive(Clone)]
enum PaletteAction {
    SetTheme(VisualTheme),
    SetTier(&'static str),
    ToggleLeft,
    ToggleRight,
    ToggleTechnician,
    ToggleHelp,
    ToggleFog,
    OpenSearch,
    Jump(NodeId),
}

struct Entry {
    label: String,
    action: PaletteAction,
}

fn static_entries() -> Vec<Entry> {
    use PaletteAction::*;
    [
        ("Theme: Standard", SetTheme(VisualTheme::Standard)),
        ("Theme: Minimal", SetTheme(VisualTheme::Minimal)),
        ("Quality: Auto (detect)", SetTier("auto")),
        ("Quality: Potato", SetTier("potato")),
        ("Quality: Low", SetTier("low")),
        ("Quality: Medium", SetTier("medium")),
        ("Quality: High", SetTier("high")),
        ("Toggle left rail", ToggleLeft),
        ("Toggle inspector (right)", ToggleRight),
        ("Toggle Technician section", ToggleTechnician),
        ("Toggle help", ToggleHelp),
        ("Toggle fog of war", ToggleFog),
        ("Open node search", OpenSearch),
    ]
    .into_iter()
    .map(|(label, action)| Entry {
        label: label.to_string(),
        action,
    })
    .collect()
}

fn apply_palette_action(st: &mut GraphState, quality: &mut QualityState, action: PaletteAction) {
    match action {
        PaletteAction::SetTheme(t) => st.cfg.visual_theme = t,
        PaletteAction::SetTier(s) => {
            st.cfg.quality.tier = s.to_string();
            if let Some(t) = parse_tier(s) {
                quality.base = t;
                quality.set_effective(t);
            }
        }
        PaletteAction::ToggleLeft => st.cfg.shell.left_open = !st.cfg.shell.left_open,
        PaletteAction::ToggleRight => st.cfg.shell.right_open = !st.cfg.shell.right_open,
        PaletteAction::ToggleTechnician => {
            st.cfg.shell.technician_open = !st.cfg.shell.technician_open
        }
        PaletteAction::ToggleHelp => st.ui.help_open = !st.ui.help_open,
        PaletteAction::ToggleFog => st.cfg.fog_of_war = !st.cfg.fog_of_war,
        PaletteAction::OpenSearch => st.ui.search_open = true,
        PaletteAction::Jump(id) => {
            st.reveal(&id);
            st.ui.focus = Some(id.clone());
            st.ui.selected = Some(id.clone());
            st.request_jump(id);
        }
    }
    st.needs_redraw.store(true, Ordering::Relaxed);
}

/// Rank palette entries (actions + nodes) against the query. Pure ranking — the
/// per-frame work the overlay does before drawing.
fn ranked_entries(st: &GraphState, query: &str) -> Vec<Entry> {
    let mut scored: Vec<(i32, Entry)> = Vec::new();
    for e in static_entries() {
        if let Some(s) = fuzzy_match(query, &e.label) {
            scored.push((s, e));
        }
    }
    if !query.is_empty() {
        for (id, _) in st.model.nodes.iter().take(NODE_SCAN_CAP) {
            let label = st.node_label_with_id(id);
            if let Some(s) = fuzzy_match(query, &label) {
                scored.push((
                    s - 4, // slight bias toward command entries
                    Entry {
                        label: format!("→ {label}"),
                        action: PaletteAction::Jump(id.clone()),
                    },
                ));
            }
        }
    }
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    scored.truncate(MAX_RESULTS);
    scored.into_iter().map(|(_, e)| e).collect()
}

/// `Ctrl/Cmd+P` command palette overlay. Panic-free headless (`try_ctx_mut`).
pub fn command_palette_overlay(
    mut contexts: EguiContexts,
    mut st: ResMut<GraphState>,
    mut quality: ResMut<QualityState>,
) {
    if !st.ui.palette_open {
        return;
    }
    let query = st.ui.palette_query.clone();
    let entries = ranked_entries(&st, &query);

    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };
    let mut chosen: Option<PaletteAction> = None;
    let mut close = false;
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }
    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));

    egui::Window::new("⌘ Command Palette")
        .anchor(egui::Align2::CENTER_TOP, [0.0, 60.0])
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.set_min_width(420.0);
            let resp = ui.add(
                egui::TextEdit::singleline(&mut st.ui.palette_query)
                    .hint_text("type a command or node…")
                    .desired_width(f32::INFINITY),
            );
            resp.request_focus();
            ui.separator();
            for (i, e) in entries.iter().enumerate() {
                let hot = i == 0;
                let text = if hot {
                    egui::RichText::new(&e.label).strong()
                } else {
                    egui::RichText::new(&e.label)
                };
                if ui.selectable_label(hot, text).clicked() {
                    chosen = Some(e.action.clone());
                }
            }
            if entries.is_empty() {
                ui.weak("no matches");
            }
        });

    // Enter executes the top result.
    if enter {
        if let Some(e) = entries.into_iter().next() {
            chosen = Some(e.action);
        }
    }

    if let Some(action) = chosen {
        apply_palette_action(&mut st, &mut quality, action);
        close = true;
    }
    if close {
        st.ui.palette_open = false;
        st.ui.palette_query.clear();
        st.needs_redraw.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_matches_subsequence_only() {
        assert!(
            fuzzy_match("fb", "foobar").is_some(),
            "f..b is a subsequence"
        );
        assert!(fuzzy_match("foo", "foobar").is_some());
        assert!(fuzzy_match("xyz", "foobar").is_none(), "not a subsequence");
        assert!(fuzzy_match("", "anything").is_some(), "empty matches all");
        assert!(fuzzy_match("oof", "foobar").is_none(), "order matters");
    }

    #[test]
    fn fuzzy_prefers_contiguous_and_prefix() {
        // "foo" contiguous-at-start beats scattered "f-o-o" in a longer string.
        let exact = fuzzy_match("foo", "foobar").unwrap();
        let scattered = fuzzy_match("foo", "xfxoxo").unwrap();
        assert!(exact > scattered, "contiguous/prefix scores higher");
        // A prefix beats a mid-string match for the same needle.
        let prefix = fuzzy_match("bar", "bartender").unwrap();
        let mid = fuzzy_match("bar", "xxbar").unwrap();
        assert!(prefix > mid);
    }

    #[test]
    fn ranked_entries_surface_theme_command() {
        let st = GraphState::default();
        let entries = ranked_entries(&st, "minimal");
        assert!(
            entries.iter().any(|e| e.label.contains("Minimal")),
            "fuzzy 'minimal' surfaces the theme command"
        );
    }

    #[test]
    fn palette_overlay_runs_without_panic_headless() {
        use bevy::prelude::*;
        let mut st = GraphState::default();
        st.ui.palette_open = true;
        st.ui.palette_query = "the".to_string();
        let mut app = App::new();
        app.init_resource::<bevy_egui::EguiUserTextures>()
            .insert_resource(st)
            .insert_resource(QualityState::default())
            .add_systems(Update, command_palette_overlay);
        app.update();
    }
}
