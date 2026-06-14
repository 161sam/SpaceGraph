//! Cyberspace post-process — a fullscreen pass (scanlines + vignette + chromatic
//! aberration + grain) inserted after Tonemapping and before the end of main-pass
//! post-processing. Standard-theme only and toggleable; the WGSL lives in
//! `assets/shaders/cyberspace_post.wgsl` (embedded via `load_internal_asset!`).
//!
//! Structure follows Bevy 0.14's `post_process` example (the pinned version's API
//! wins where this and the sketch disagree). The GPU output can only be verified
//! by local capture (headless has no display); the headless gates are: the WGSL
//! validates via `naga`, the plugin builds without a render app, the config round
//! trips, and `postfx_active` forces Minimal off.

use bevy::asset::load_internal_asset;
use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::core_pipeline::fullscreen_vertex_shader::fullscreen_shader_vertex_state;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::extract_component::{
    ComponentUniforms, DynamicUniformIndex, ExtractComponentPlugin, UniformComponentPlugin,
};
use bevy::render::render_graph::{
    NodeRunError, RenderGraphApp, RenderGraphContext, RenderLabel, ViewNode, ViewNodeRunner,
};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy::render::render_resource::{
    BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries, CachedRenderPipelineId,
    ColorTargetState, ColorWrites, FragmentState, MultisampleState, Operations, PipelineCache,
    PrimitiveState, RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor,
    Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, TextureFormat, TextureSampleType,
};
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::view::ViewTarget;
use bevy::render::RenderApp;

use crate::graph::GraphState;
use crate::util::config::VisualTheme;

const CYBERSPACE_SHADER: Handle<Shader> = Handle::weak_from_u128(0x5A17_C0DE_9B3E_41A2);

/// Whether the post-process pass should run: Standard theme + enabled. Pure +
/// tested — this is how Minimal is forced off without clobbering the saved config.
pub fn postfx_active(theme: VisualTheme, enabled: bool) -> bool {
    theme == VisualTheme::Standard && enabled
}

// In its own module so the `dead_code` allow scopes to the `ShaderType` derive's
// uncalled per-field `check` fns (a derive quirk) without hiding anything else.
/// Maximum alert-focus points fed to the post-process shader. Uniform-bounded
/// (a fixed-size array keeps a single uniform binding — no storage buffer).
pub const MAX_ALERT_FOCUS: usize = 16;

mod settings {
    #![allow(dead_code)]
    use super::MAX_ALERT_FOCUS;
    use bevy::math::Vec4;
    use bevy::prelude::Component;
    use bevy::render::extract_component::ExtractComponent;
    use bevy::render::render_resource::ShaderType;

    /// Per-view post-process uniform. The intensities mirror `cfg.postfx`; `time`
    /// drives the grain. The `anomaly_*` + `alerts` fields localize the post-fx
    /// around alerted nodes (D0/ADR-0012). Field order + the `_pad` scalar keep
    /// the `vec4` array 16-byte aligned so `encase`/WGSL layouts agree.
    #[derive(Component, Default, Clone, Copy, ExtractComponent, ShaderType)]
    pub struct PostFxSettings {
        pub scanline: f32,
        pub vignette: f32,
        pub aberration: f32,
        pub grain: f32,
        pub time: f32,
        /// Global anomaly-focus strength (0 = off, including under Minimal).
        pub anomaly_intensity: f32,
        /// Number of valid entries in `alerts`.
        pub alert_count: u32,
        pub _pad: f32,
        /// Each: `xy` = screen UV of the alert, `z` = per-alert intensity, `w` = 0.
        pub alerts: [Vec4; MAX_ALERT_FOCUS],
    }
}
use settings::PostFxSettings;

/// Severity ordering weight (higher = more urgent).
pub fn severity_weight(severity: &str) -> u8 {
    match severity {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

/// Pick up to `cap` alert indices to focus, ordered by severity then recency
/// (most severe + most recent first). Input is `(severity_weight, recency_rank)`
/// where a higher `recency_rank` is more recent. Pure + unit-tested; the GPU look
/// is documented in RUNLOG (ADR-0012 §3).
pub fn select_focus_alerts(alerts: &[(u8, u64)], cap: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..alerts.len()).collect();
    idx.sort_by(|&a, &b| {
        alerts[b]
            .0
            .cmp(&alerts[a].0) // severity desc
            .then(alerts[b].1.cmp(&alerts[a].1)) // recency desc
            .then(a.cmp(&b)) // stable on ties
    });
    idx.truncate(cap.min(MAX_ALERT_FOCUS));
    idx
}

/// Attach/update/remove the per-camera `PostFxSettings` so the pass runs only
/// when active (Standard + enabled); removing it disables the pass entirely.
pub fn sync_postfx(
    mut commands: Commands,
    st: Res<GraphState>,
    quality: Res<crate::render::quality::QualityState>,
    time: Res<Time>,
    cam_q: Query<(Entity, &Camera, &GlobalTransform)>,
) {
    let cfg = st.cfg.postfx;
    // Standard + user-enabled + the active quality tier permits post-FX.
    let active = postfx_active(st.cfg.visual_theme, cfg.enabled)
        && quality.gates(st.cfg.visual_theme).postfx.is_on();
    let sd = st.cfg.socket_display;
    // Alert world positions + per-alert intensity (shared across cameras;
    // projection is per-camera). Empty when anomaly focus is off / pass inactive.
    let focus_world: Vec<(Vec3, f32)> = if active && sd.anomaly_focus {
        collect_focus_alerts(&st)
    } else {
        Vec::new()
    };

    for (cam, camera, cam_tf) in cam_q.iter() {
        if !active {
            commands.entity(cam).remove::<PostFxSettings>();
            continue;
        }
        let mut s = PostFxSettings {
            scanline: cfg.scanline,
            vignette: cfg.vignette,
            aberration: cfg.aberration,
            grain: cfg.grain,
            time: time.elapsed_seconds(),
            ..Default::default()
        };
        if sd.anomaly_focus && !focus_world.is_empty() {
            s.anomaly_intensity = sd.anomaly_intensity.clamp(0.0, 1.0);
            let size = camera
                .logical_viewport_size()
                .unwrap_or(Vec2::new(1.0, 1.0));
            let mut count = 0usize;
            for (wpos, intensity) in &focus_world {
                if count >= MAX_ALERT_FOCUS {
                    break;
                }
                if let Some(vp) = camera.world_to_viewport(cam_tf, *wpos) {
                    let uv = vp / size;
                    s.alerts[count] = Vec4::new(uv.x, uv.y, *intensity, 0.0);
                    count += 1;
                }
            }
            s.alert_count = count as u32;
        }
        commands.entity(cam).insert(s);
    }
}

/// Collect the focus alerts' world positions + intensities, ordered by severity
/// then recency and capped at `MAX_ALERT_FOCUS` (via [`select_focus_alerts`]).
/// Only placed alert nodes are eligible. HashMap iteration order seeds recency —
/// adequate for the visual cue (the tested selection uses real recency).
fn collect_focus_alerts(st: &GraphState) -> Vec<(Vec3, f32)> {
    let mut sev_rec: Vec<(u8, u64)> = Vec::new();
    let mut pos: Vec<Vec3> = Vec::new();
    let mut recency: u64 = 0;
    for (id, node) in st.model.nodes.iter() {
        if let spacegraph_core::Node::Alert { severity, .. } = node {
            if let Some(idx) = st.spatial.index_of(id) {
                if st.spatial.placed[idx.slot()] {
                    sev_rec.push((severity_weight(severity), recency));
                    pos.push(st.spatial.positions[idx.slot()]);
                    recency += 1;
                }
            }
        }
    }
    select_focus_alerts(&sev_rec, MAX_ALERT_FOCUS)
        .into_iter()
        .map(|i| {
            let intensity = (sev_rec[i].0 as f32 / 4.0).clamp(0.25, 1.0);
            (pos[i], intensity)
        })
        .collect()
}

pub struct PostFxPlugin;

impl Plugin for PostFxPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            CYBERSPACE_SHADER,
            "../../assets/shaders/cyberspace_post.wgsl",
            Shader::from_wgsl
        );
        app.add_plugins((
            ExtractComponentPlugin::<PostFxSettings>::default(),
            UniformComponentPlugin::<PostFxSettings>::default(),
        ));

        // No render app (headless / tests) → nothing more to wire.
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .add_render_graph_node::<ViewNodeRunner<PostFxNode>>(Core3d, PostFxLabel)
            .add_render_graph_edges(
                Core3d,
                (
                    Node3d::Tonemapping,
                    PostFxLabel,
                    Node3d::EndMainPassPostProcessing,
                ),
            );
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.init_resource::<PostFxPipeline>();
    }
}

#[derive(RenderLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PostFxLabel;

#[derive(Default)]
struct PostFxNode;

impl ViewNode for PostFxNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static DynamicUniformIndex<PostFxSettings>,
    );

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (view_target, settings_index): QueryItem<'w, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let pipeline_res = world.resource::<PostFxPipeline>();
        let cache = world.resource::<PipelineCache>();
        let Some(pipeline) = cache.get_render_pipeline(pipeline_res.pipeline_id) else {
            return Ok(());
        };
        let uniforms = world.resource::<ComponentUniforms<PostFxSettings>>();
        let Some(binding) = uniforms.uniforms().binding() else {
            return Ok(());
        };

        let post = view_target.post_process_write();
        let bind_group = render_context.render_device().create_bind_group(
            "cyberspace_post_bind_group",
            &pipeline_res.layout,
            &BindGroupEntries::sequential((post.source, &pipeline_res.sampler, binding.clone())),
        );

        let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("cyberspace_post_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post.destination,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[settings_index.index()]);
        pass.draw(0..3, 0..1);
        Ok(())
    }
}

#[derive(Resource)]
struct PostFxPipeline {
    layout: BindGroupLayout,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

impl FromWorld for PostFxPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>().clone();
        let layout = render_device.create_bind_group_layout(
            "cyberspace_post_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    uniform_buffer::<PostFxSettings>(true),
                ),
            ),
        );
        let sampler = render_device.create_sampler(&SamplerDescriptor::default());
        let pipeline_id =
            world
                .resource_mut::<PipelineCache>()
                .queue_render_pipeline(RenderPipelineDescriptor {
                    label: Some("cyberspace_post_pipeline".into()),
                    layout: vec![layout.clone()],
                    vertex: fullscreen_shader_vertex_state(),
                    fragment: Some(FragmentState {
                        shader: CYBERSPACE_SHADER,
                        shader_defs: vec![],
                        entry_point: "fragment".into(),
                        targets: vec![Some(ColorTargetState {
                            format: TextureFormat::Rgba16Float,
                            blend: None,
                            write_mask: ColorWrites::ALL,
                        })],
                    }),
                    primitive: PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: MultisampleState::default(),
                    push_constant_ranges: vec![],
                });
        Self {
            layout,
            sampler,
            pipeline_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wgsl_postfx_validates() {
        let src = include_str!("../../assets/shaders/cyberspace_post.wgsl");
        let module = naga::front::wgsl::parse_str(src).expect("WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("WGSL validates");
    }

    #[test]
    fn postfx_active_forces_minimal_off() {
        assert!(postfx_active(VisualTheme::Standard, true));
        assert!(!postfx_active(VisualTheme::Standard, false));
        assert!(!postfx_active(VisualTheme::Minimal, true));
    }

    #[test]
    fn severity_weight_orders_by_urgency() {
        assert!(severity_weight("critical") > severity_weight("high"));
        assert!(severity_weight("high") > severity_weight("medium"));
        assert!(severity_weight("medium") > severity_weight("low"));
        assert_eq!(severity_weight("bogus"), 0);
    }

    #[test]
    fn select_focus_alerts_orders_by_severity_then_recency() {
        // (severity_weight, recency_rank)
        let alerts = [
            (2u8, 5u64), // medium, recent
            (4, 1),      // critical, old
            (4, 9),      // critical, newest
            (1, 8),      // low, recent
        ];
        // critical-newest (2), critical-old (1), then medium (0).
        assert_eq!(select_focus_alerts(&alerts, 3), vec![2, 1, 0]);
    }

    #[test]
    fn select_focus_alerts_is_count_bounded() {
        let alerts: Vec<(u8, u64)> = (0..40).map(|i| (1u8, i as u64)).collect();
        assert!(select_focus_alerts(&alerts, 100).len() <= MAX_ALERT_FOCUS);
        assert_eq!(select_focus_alerts(&alerts, 5).len(), 5);
    }

    #[test]
    fn postfx_plugin_builds_without_render_app() {
        // No RenderApp (headless) → build/finish early-return without panic.
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Shader>();
        app.add_plugins(PostFxPlugin);
        app.finish();
    }
}
