use bevy::prelude::Resource;
use bevy_egui::egui;

#[derive(Resource, Clone, Copy)]
pub struct UiLayout {
    pub panel_rect: egui::Rect,
    pub content_rect: egui::Rect,
}

impl Default for UiLayout {
    fn default() -> Self {
        Self {
            panel_rect: egui::Rect::NOTHING,
            content_rect: egui::Rect::NOTHING,
        }
    }
}
