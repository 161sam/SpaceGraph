use bevy::prelude::*;

use crate::app::events::Picked;
use crate::app::resources::{NetRx, NetTx};
use crate::graph::state::{NetCommand, NetStreamStatus, OutboundMsg};
use crate::graph::GraphState;
use crate::net;
use crate::ui::UiLayout;
use crate::util::config;
use crate::util::config::AgentEndpointKind;
use spacegraph_core::Msg;

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
            .insert_resource(crate::render::NodeGlyphs::default())
            .insert_resource(crate::ui::PreviewState::default())
            .insert_resource(crate::render::RippleTracker::default())
            .insert_resource(crate::render::PreviewExpand::default())
            .insert_resource(crate::ui::RadialMenu::default())
            .insert_resource(crate::ui::RailState::default())
            .insert_resource(crate::render::FocusCam::default())
            // Default detail capability + quality tier; `finish` refines both from
            // the real GPU adapter (or config) once the renderer is initialized.
            .insert_resource(crate::render::DetailCapability::Mid)
            .insert_resource(crate::render::QualityState::default());

        match self.demo_load {
            Some(n) => {
                app.insert_resource(DemoLoad(n))
                    .add_systems(Startup, seed_demo_load);
            }
            None => {
                app.add_systems(Startup, auto_connect_agents);
            }
        }

        // Gated visual smoke-test hook (no effect unless the env var is set):
        // deterministically enters Focus Mode on a hub node for screenshot capture.
        if std::env::var("SPACEGRAPH_DEMO_FOCUS").is_ok() {
            app.add_systems(Update, demo_autofocus);
        }

        app.add_systems(
            Startup,
            (
                crate::render::setup_scene,
                crate::render::setup_node_render_resources,
                crate::render::setup_node_icon_resources,
                crate::render::setup_node_glyph_resources,
                crate::render::setup_ripple_resources,
                crate::render::setup_edge_mesh,
            ),
        )
        .add_systems(
            Update,
            (
                process_net_commands,
                pump_network,
                pump_outbound,
                crate::graph::tick_housekeeping,
                crate::ui::apply_egui_theme,
                crate::ui::handle_shortcuts,
                // P2: the slim command rail + corner HUD panels replace the old
                // permanent left sidebar. `update_ui_layout` runs first so panels
                // read a fresh content_rect; `dispatch_windows` hosts the modal
                // windows the old `ui_panel` used to dispatch.
                (
                    crate::ui::update_ui_layout,
                    crate::ui::command_rail,
                    crate::ui::hud_panels,
                    crate::ui::dispatch_windows,
                )
                    .chain(),
                crate::ui::help_overlay,
                crate::ui::hud_overlay,
                crate::ui::hud_frame_overlay,
                crate::ui::inspector_overlay,
                crate::ui::legend_overlay,
                crate::ui::reticle_overlay,
                crate::ui::context_menu_overlay,
                crate::ui::radial_hud,
                (crate::ui::focus_overlay, crate::ui::entity_card_overlay),
                crate::ui::command_palette_overlay,
                crate::ui::node_preview_overlay,
                crate::ui::minimap,
            )
                // Deterministic within-layer paint order for the egui overlays
                // (P1: was an ambiguous tuple — the structural root of the
                // overlap bug; z-order is now owned by `ui::overlay::layer`).
                .chain(),
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
                crate::render::trigger_alert_ripple,
                crate::render::update_focus_ripples,
                crate::render::detect_preview_expand,
                crate::render::apply_quality,
                crate::render::adaptive_quality,
            ),
        )
        // Focus Mode (v0.5.1): mouse entry + eased camera return (its own group to
        // stay within the per-`add_systems` tuple limit).
        .add_systems(
            Update,
            (
                crate::ui::focus_double_click,
                crate::render::focus_mode_camera,
            ),
        )
        // Render pipeline runs in order: layout publishes the visible set, the
        // entity sync diffs against it, then overlays draw, then camera jumps.
        .add_systems(
            Update,
            (
                crate::graph::update_layout_or_timeline,
                // D1: budgeted graph-native detection after layout (ADR-0005);
                // emits spacegraph-rule alerts on its interval cadence.
                crate::graph::rules::run_detection_rules,
                crate::render::sync_node_entities,
                crate::render::sync_node_rings,
                crate::render::sync_node_icons,
                crate::render::sync_node_glyphs,
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

        // D1 detection engine state (registry + active-set for de-dup/re-arm).
        app.init_resource::<crate::graph::rules::DetectionState>();

        // Opt-in UI sound effects (DefaultPlugins includes AudioPlugin when the
        // `audio` feature enables bevy_audio).
        #[cfg(feature = "audio")]
        {
            app.add_systems(Startup, crate::render::setup_audio);
            app.add_systems(Update, crate::render::audio_triggers);
        }
    }

    /// Resolve the quality tier (and derived node-detail capability) once the
    /// renderer has initialized. `[quality] tier` overrides auto-detection from
    /// `RenderAdapterInfo` (in the `RenderApp`); `[node_detail] level` still
    /// overrides the derived `DetailCapability`. (Spec §2.4.)
    fn finish(&self, app: &mut App) {
        let (cfg_tier, detail_override, qcfg) = {
            let st = app.world().resource::<GraphState>();
            (
                crate::render::quality::parse_tier(&st.cfg.quality.tier),
                crate::render::capability::parse_override(&st.cfg.node_detail.level),
                st.cfg.quality.clone(),
            )
        };
        let detected =
            app.get_sub_app(bevy::render::RenderApp)
                .and_then(|render_app| {
                    render_app
                        .world()
                        .get_resource::<bevy::render::renderer::RenderAdapterInfo>()
                        .map(|info| {
                            let kind = crate::render::capability::adapter_kind_from_debug(
                                &format!("{:?}", info.device_type),
                            );
                            let backend = format!("{:?}", info.backend);
                            crate::render::quality::detect_tier(&info.name, kind, &backend)
                        })
                });
        let base = cfg_tier
            .or(detected)
            .unwrap_or(crate::render::QualityTier::Medium);
        // node_detail.level explicitly overrides the tier-derived capability.
        let cap = detail_override.unwrap_or_else(|| base.detail_capability());

        info!(
            "quality tier: {:?} ({}); node detail: {:?}",
            base,
            if cfg_tier.is_some() { "config" } else { "auto" },
            cap
        );
        app.insert_resource(crate::render::quality::QualityState::new(base, &qcfg));
        app.insert_resource(cap);
    }
}

fn pump_network(mut st: ResMut<GraphState>, rx: Res<NetRx>) {
    for msg in rx.0.try_iter().take(100_000) {
        st.apply(msg);
    }
}

/// Drain the viewer → agent outbox (FS `SearchRequest` / `MaterialiseRequest`)
/// onto each stream's outbound channel. Non-blocking (`try_send`); a full or
/// missing channel drops the message (the user can retype).
fn pump_outbound(mut st: ResMut<GraphState>) {
    if st.net.outbox.is_empty() {
        return;
    }
    let outbox = std::mem::take(&mut st.net.outbox);
    for OutboundMsg { stream, msg } in outbox {
        if let Some(tx) = st.net.outbound.get(&stream) {
            let _ = tx.try_send(msg);
        }
    }
}

fn seed_demo_load(mut st: ResMut<GraphState>, demo: Res<DemoLoad>) {
    st.load_synthetic_graph(demo.0);
}

/// Visual smoke-test hook (gated on `SPACEGRAPH_DEMO_FOCUS`, registered only when
/// that env var is set — see `build`). Once the demo layout exists, select and
/// enter Focus Mode on the highest-degree (hub) node so screenshot automation can
/// capture the focus visuals deterministically, without fragile click-picking
/// against a still-settling layout. No effect on a normal run.
fn demo_autofocus(
    time: Res<Time>,
    mut st: ResMut<GraphState>,
    mut radial: ResMut<crate::ui::RadialMenu>,
    mut done: Local<bool>,
) {
    // Time-gated (not frame-gated) so it fires reliably regardless of the
    // reactive frame pacing: ~1.5s of app time lets the layout spread first.
    if *done || time.elapsed_seconds() < 1.5 {
        return;
    }
    // Highest-degree node *among placed nodes* (so it has a spatial position the
    // camera can dive to and the focus core can frame). Deterministic enough.
    let hub = st
        .spatial
        .placed_positions()
        .map(|(id, _)| id.clone())
        .max_by_key(|id| st.core.model.degree(id));
    if let Some(id) = hub {
        crate::ui::focus::enter_focus(&mut st, &mut radial, id);
    }
    *done = true;
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
                // Outbound channel: viewer → agent FS requests (v4). Held in
                // `net.outbound` for the connection's lifetime.
                let (out_tx, out_rx) = tokio::sync::mpsc::channel::<Msg>(256);
                let handle =
                    net::spawn_reader(endpoint.name.clone(), path, net_tx.0.clone(), out_rx);
                st.net.connections.insert(endpoint.name.clone(), handle);
                st.net.outbound.insert(endpoint.name.clone(), out_tx);
            }
            NetCommand::Disconnect(name) => {
                if let Some(handle) = st.net.connections.remove(&name) {
                    handle.shutdown();
                }
                st.net.outbound.remove(&name);
                if let Some(stream) = st.net.streams.get_mut(&name) {
                    stream.status = NetStreamStatus::Disconnected;
                }
            }
            NetCommand::Reconnect(name) => {
                if let Some(handle) = st.net.connections.remove(&name) {
                    handle.shutdown();
                }
                st.net.outbound.remove(&name);
                if let Some(stream) = st.net.streams.get_mut(&name) {
                    stream.status = NetStreamStatus::Connecting;
                    stream.last_error = None;
                }
                st.net.commands.push(NetCommand::Connect(name));
            }
        }
    }
}
