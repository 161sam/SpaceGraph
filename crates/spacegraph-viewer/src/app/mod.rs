use bevy::prelude::*;

use crate::app::events::Picked;
use crate::app::resources::{NetRx, NetTx};
use crate::graph::state::{NetCommand, NetStreamStatus};
use crate::graph::GraphState;
use crate::net;
use crate::ui::UiLayout;
use crate::util::config;
use crate::util::config::AgentEndpointKind;

pub mod events;
pub mod resources;

#[derive(Default)]
pub struct SpaceGraphViewerPlugin {
    /// When set (via `--demo-load <n>`), seed a deterministic synthetic graph
    /// of `n` nodes instead of auto-connecting to agents.
    pub demo_load: Option<usize>,
}

#[derive(Resource)]
struct DemoLoad(usize);

impl Plugin for SpaceGraphViewerPlugin {
    fn build(&self, app: &mut App) {
        let cfg = config::load_or_default();
        let mut st = GraphState::default();
        st.apply_viewer_config(&cfg);
        app.add_plugins(bevy_panorbit_camera::PanOrbitCameraPlugin)
            .add_plugins(crate::render::PostFxPlugin)
            .add_event::<Picked>()
            // Reactive rendering: idle low-power, full speed only while animating
            // (see `render::pacing`). Continuous to start so the initial layout
            // converges at full speed; pacing takes over from the first frame.
            .insert_resource(bevy::winit::WinitSettings {
                focused_mode: bevy::winit::UpdateMode::Continuous,
                unfocused_mode: bevy::winit::UpdateMode::reactive_low_power(
                    std::time::Duration::from_millis(500),
                ),
            })
            .insert_resource(st)
            .insert_resource(UiLayout::default())
            .insert_resource(crate::render::NodeEntities::default())
            .insert_resource(crate::render::NodeRings::default())
            .insert_resource(crate::render::RebuildNodeEntities::default())
            .insert_resource(crate::render::FlyCam::default())
            .insert_resource(crate::render::DragSelect::default())
            .insert_resource(crate::render::ScanPulse::default())
            .insert_resource(crate::render::Mission::default())
            .insert_resource(crate::render::FramePacing::default())
            .insert_resource(crate::render::NodeIcons::default())
            .insert_resource(crate::ui::PreviewState::default())
            .insert_resource(crate::render::RippleTracker::default())
            .insert_resource(crate::render::PreviewExpand::default())
            // Default detail capability; `finish` refines it from the real GPU
            // adapter once the renderer is initialized.
            .insert_resource(crate::render::DetailCapability::Mid);

        match self.demo_load {
            Some(n) => {
                app.insert_resource(DemoLoad(n))
                    .add_systems(Startup, seed_demo_load);
            }
            None => {
                app.add_systems(Startup, auto_connect_agents);
            }
        }

        app.add_systems(
            Startup,
            (
                crate::render::setup_scene,
                crate::render::setup_node_render_resources,
                crate::render::setup_node_icon_resources,
                crate::render::setup_ripple_resources,
                crate::render::setup_edge_mesh,
            ),
        )
        .add_systems(
            Update,
            (
                process_net_commands,
                pump_network,
                crate::graph::tick_housekeeping,
                crate::ui::handle_shortcuts,
                crate::ui::ui_panel,
                crate::ui::help_overlay,
                crate::ui::hud_overlay,
                crate::ui::inspector_overlay,
                crate::ui::legend_overlay,
                crate::ui::reticle_overlay,
                crate::ui::context_menu_overlay,
                crate::ui::node_preview_overlay,
                crate::ui::minimap,
            ),
        )
        .add_systems(
            Update,
            (
                crate::render::hover_detection_spatial,
                crate::render::picking_focus,
                crate::render::apply_picked_focus,
                crate::render::update_tree_zoom,
                crate::render::sync_visual_theme,
                crate::render::fly_camera,
                crate::render::scan_pulse,
                crate::render::mission_tick,
                crate::render::reveal_tick,
                crate::render::rotate_node_rings,
                crate::render::sync_postfx,
                crate::ui::update_preview_requests,
                crate::ui::poll_preview_decodes,
                crate::render::trigger_focus_ripple,
                crate::render::update_focus_ripples,
                crate::render::detect_preview_expand,
            ),
        )
        // Render pipeline runs in order: layout publishes the visible set, the
        // entity sync diffs against it, then overlays draw, then camera jumps.
        .add_systems(
            Update,
            (
                crate::graph::update_layout_or_timeline,
                crate::render::sync_node_entities,
                crate::render::sync_node_rings,
                crate::render::sync_node_icons,
                crate::render::update_edge_mesh,
                crate::render::draw_scene,
                crate::render::draw_node_labels,
                crate::render::apply_jump_to,
            )
                .chain(),
        )
        // Runs after every Update system so it sees all redraw requests made
        // this frame before deciding whether the next frame can be skipped.
        .add_systems(Last, crate::render::update_frame_pacing);

        // Opt-in UI sound effects (DefaultPlugins includes AudioPlugin when the
        // `audio` feature enables bevy_audio).
        #[cfg(feature = "audio")]
        {
            app.add_systems(Startup, crate::render::setup_audio);
            app.add_systems(Update, crate::render::audio_triggers);
        }
    }

    /// Resolve the node-detail capability from the real GPU adapter once the
    /// renderer has initialized (the `[node_detail] level` config overrides it).
    /// `RenderAdapterInfo` lives in the `RenderApp`; we read it here and store the
    /// resolved `DetailCapability` in the main world. (v0.5.0 `QualityTier` seam.)
    fn finish(&self, app: &mut App) {
        let override_cap = {
            let st = app.world().resource::<GraphState>();
            crate::render::capability::parse_override(&st.cfg.node_detail.level)
        };
        let cap = override_cap
            .or_else(|| {
                app.get_sub_app(bevy::render::RenderApp)
                    .and_then(|render_app| {
                        render_app
                            .world()
                            .get_resource::<bevy::render::renderer::RenderAdapterInfo>()
                            .map(|info| {
                                let kind = crate::render::capability::adapter_kind_from_debug(
                                    &format!("{:?}", info.device_type),
                                );
                                crate::render::capability::detect_capability(&info.name, kind)
                            })
                    })
            })
            .unwrap_or(crate::render::DetailCapability::Mid);
        info!(
            "node detail capability: {:?}{}",
            cap,
            if override_cap.is_some() {
                " (config override)"
            } else {
                " (auto-detected)"
            }
        );
        app.insert_resource(cap);
    }
}

fn pump_network(mut st: ResMut<GraphState>, rx: Res<NetRx>) {
    for msg in rx.0.try_iter().take(100_000) {
        st.apply(msg);
    }
}

fn seed_demo_load(mut st: ResMut<GraphState>, demo: Res<DemoLoad>) {
    st.load_synthetic_graph(demo.0);
}

fn auto_connect_agents(mut st: ResMut<GraphState>) {
    let auto_connect: Vec<String> = st
        .net
        .endpoints
        .iter()
        .filter(|endpoint| endpoint.auto_connect)
        .map(|endpoint| endpoint.name.clone())
        .collect();
    for name in auto_connect {
        st.net.commands.push(NetCommand::Connect(name));
    }
}

fn process_net_commands(mut st: ResMut<GraphState>, net_tx: Res<NetTx>) {
    let commands = std::mem::take(&mut st.net.commands);
    for cmd in commands {
        match cmd {
            NetCommand::Connect(name) => {
                if st.net.connections.contains_key(&name) {
                    continue;
                }
                let endpoint = st.net.endpoints.iter().find(|e| e.name == name).cloned();
                let Some(endpoint) = endpoint else {
                    if let Some(stream) = st.net.streams.get_mut(&name) {
                        stream.status = NetStreamStatus::Disconnected;
                        stream.last_error = Some("endpoint not configured".to_string());
                    }
                    continue;
                };
                let path = match &endpoint.kind {
                    AgentEndpointKind::UdsPath(path) => path.clone(),
                };
                st.net.ensure_stream(&endpoint.name);
                if let Some(stream) = st.net.streams.get_mut(&endpoint.name) {
                    stream.status = NetStreamStatus::Connecting;
                    stream.last_error = None;
                }
                let handle = net::spawn_reader(endpoint.name.clone(), path, net_tx.0.clone());
                st.net.connections.insert(endpoint.name.clone(), handle);
            }
            NetCommand::Disconnect(name) => {
                if let Some(handle) = st.net.connections.remove(&name) {
                    handle.shutdown();
                }
                if let Some(stream) = st.net.streams.get_mut(&name) {
                    stream.status = NetStreamStatus::Disconnected;
                }
            }
            NetCommand::Reconnect(name) => {
                if let Some(handle) = st.net.connections.remove(&name) {
                    handle.shutdown();
                }
                if let Some(stream) = st.net.streams.get_mut(&name) {
                    stream.status = NetStreamStatus::Connecting;
                    stream.last_error = None;
                }
                st.net.commands.push(NetCommand::Connect(name));
            }
        }
    }
}
