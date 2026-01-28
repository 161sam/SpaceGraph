use bevy_egui::egui;

use crate::graph::GraphState;
use crate::ui::UiLayout;
use crate::util::config::{self, ViewerConfig};

pub fn path_editor_window(ctx: &egui::Context, st: &mut GraphState, layout: &UiLayout) {
    if !st.ui.show_path_editor {
        return;
    }

    let mut content_rect = layout.content_rect;
    if content_rect == egui::Rect::NOTHING {
        content_rect = ctx.screen_rect();
    }

    let default_size = egui::vec2(
        content_rect.width().clamp(360.0, 640.0),
        content_rect.height().clamp(280.0, 420.0),
    );
    let mut default_pos = content_rect.center() - default_size / 2.0;
    default_pos.x = default_pos
        .x
        .clamp(content_rect.min.x, content_rect.max.x - default_size.x);
    default_pos.y = default_pos
        .y
        .clamp(content_rect.min.y, content_rect.max.y - default_size.y);

    let mut open = st.ui.show_path_editor;
    let mut close_requested = false;
    egui::Window::new("Paths (Include/Exclude)")
        .collapsible(false)
        .resizable(true)
        .default_size(default_size)
        .default_pos(default_pos)
        .constrain_to(content_rect)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label("Manage whitelist (includes) and blacklist (excludes) paths.");
            ui.add_space(6.0);

            ui.columns(2, |cols| {
                render_path_list(
                    &mut cols[0],
                    "Includes",
                    &mut st.ui.path_editor.includes,
                    &mut st.ui.path_editor.include_input,
                    &mut st.ui.path_editor.include_notice,
                );
                render_path_list(
                    &mut cols[1],
                    "Excludes",
                    &mut st.ui.path_editor.excludes,
                    &mut st.ui.path_editor.exclude_input,
                    &mut st.ui.path_editor.exclude_notice,
                );
            });

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Reset to defaults").clicked() {
                    let defaults = ViewerConfig::default();
                    st.ui.path_editor.includes = defaults.path_includes;
                    st.ui.path_editor.excludes = defaults.path_excludes;
                    st.ui.path_editor.include_input.clear();
                    st.ui.path_editor.exclude_input.clear();
                    st.ui.path_editor.include_notice = None;
                    st.ui.path_editor.exclude_notice = None;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        close_requested = true;
                    }
                    if ui.button("Save").clicked() {
                        st.cfg.path_includes = sanitize_paths(&st.ui.path_editor.includes);
                        st.cfg.path_excludes = sanitize_paths(&st.ui.path_editor.excludes);
                        let cfg = st.viewer_config();
                        if let Err(err) = config::save(&cfg) {
                            eprintln!("failed to save settings: {err}");
                        }
                        close_requested = true;
                    }
                });
            });
        });
    if close_requested {
        open = false;
    }
    st.ui.show_path_editor = open;
}

fn render_path_list(
    ui: &mut egui::Ui,
    title: &str,
    entries: &mut Vec<String>,
    input: &mut String,
    notice: &mut Option<String>,
) {
    ui.label(egui::RichText::new(title).strong());
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(input).desired_width(140.0));
        if ui.button("Add").clicked() {
            *notice = None;
            if let Some(value) = sanitize_path_entry(input) {
                if entries.contains(&value) {
                    *notice = Some("already added".to_string());
                } else {
                    entries.push(value);
                }
                input.clear();
            } else {
                *notice = Some("enter a non-empty path".to_string());
            }
        }
    });
    if let Some(msg) = notice.as_ref() {
        ui.label(egui::RichText::new(msg).weak());
    }
    ui.add_space(4.0);

    let mut remove_index: Option<usize> = None;
    egui::ScrollArea::vertical()
        .max_height(240.0)
        .show(ui, |ui| {
            for (idx, path) in entries.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.button("🗑").clicked() {
                        remove_index = Some(idx);
                    }
                    ui.label(path);
                });
            }
        });
    if let Some(idx) = remove_index {
        entries.remove(idx);
    }
}

fn sanitize_paths(paths: &[String]) -> Vec<String> {
    let mut cleaned = Vec::new();
    for path in paths {
        if let Some(value) = sanitize_path_entry(path) {
            if !cleaned.contains(&value) {
                cleaned.push(value);
            }
        }
    }
    cleaned
}

fn sanitize_path_entry(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = trimmed.replace('\\', "/");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_paths_trims_normalizes_and_dedups() {
        let input = vec![
            " /etc ".to_string(),
            "/home".to_string(),
            "/home".to_string(),
            "\\var\\log".to_string(),
            "  ".to_string(),
        ];

        let cleaned = sanitize_paths(&input);

        assert_eq!(cleaned, vec!["/etc", "/home", "/var/log"]);
    }

    #[test]
    fn sanitize_path_entry_rejects_empty() {
        assert!(sanitize_path_entry("   ").is_none());
        assert!(sanitize_path_entry("").is_none());
    }
}
