use bevy::prelude::*;

use crate::app::events::Picked;
use crate::app::resources::NetRx;
use crate::graph::GraphState;

pub mod events;
pub mod resources;

pub struct SpaceGraphViewerPlugin;

impl Plugin for SpaceGraphViewerPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<Picked>()
            .insert_resource(GraphState::default())
            .add_systems(Startup, crate::render::setup_scene)
            .add_systems(
                Update,
                (
                    pump_network,
                    crate::graph::tick_housekeeping,
                    crate::ui::ui_panel,
                    crate::ui::hud_overlay,
                    crate::render::hover_detection_spatial,
                    crate::render::picking_focus,
                    crate::render::apply_picked_focus,
                    crate::graph::update_layout_or_timeline,
                    crate::render::draw_scene,
                    crate::render::apply_jump_to,
                ),
            );
    }
}

fn pump_network(mut st: ResMut<GraphState>, rx: Res<NetRx>) {
    for msg in rx.0.try_iter().take(100_000) {
        st.apply(msg);
    }
}
