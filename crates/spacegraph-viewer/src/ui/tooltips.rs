use bevy_egui::egui;

pub fn render_tooltip(
    ctx: &egui::Context,
    id: &str,
    pos: egui::Pos2,
    lines: impl IntoIterator<Item = String>,
) {
    egui::Area::new(id).fixed_pos(pos).show(ctx, |ui| {
        ui.group(|ui| {
            for line in lines {
                ui.label(line);
            }
        });
    });
}
