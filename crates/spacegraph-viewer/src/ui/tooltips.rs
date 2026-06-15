use bevy_egui::egui;

use crate::ui::overlay::middle_truncate;

/// Max chars per tooltip line before a middle-ellipsis kicks in — keeps a long
/// path/cmdline from stretching the box across the viewport.
const TOOLTIP_MAX_CHARS: usize = 52;

pub fn render_tooltip(
    ctx: &egui::Context,
    id: &str,
    pos: egui::Pos2,
    order: egui::Order,
    lines: impl IntoIterator<Item = String>,
) {
    egui::Area::new(egui::Id::new(id))
        .order(order)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            ui.group(|ui| {
                ui.set_max_width(TOOLTIP_MAX_CHARS as f32 * 7.5);
                for line in lines {
                    ui.add(
                        egui::Label::new(middle_truncate(&line, TOOLTIP_MAX_CHARS))
                            .wrap_mode(egui::TextWrapMode::Truncate),
                    )
                    .on_hover_text(line);
                }
            });
        });
}
