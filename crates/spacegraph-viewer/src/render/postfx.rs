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
mod settings {
    #![allow(dead_code)]
    use bevy::prelude::Component;
    use bevy::render::extract_component::ExtractComponent;
    use bevy::render::render_resource::ShaderType;

    /// Per-view post-process uniform. The intensities mirror `cfg.postfx`; `time`
    /// drives the grain. (5 × f32; `encase`/WGSL agree on the padded layout.)
    #[derive(Component, Default, Clone, Copy, ExtractComponent, ShaderType)]
    pub struct PostFxSettings {
        pub scanline: f32,
        pub vignette: f32,
        pub aberration: f32,
        pub grain: f32,
        pub time: f32,
    }
}
use settings::PostFxSettings;

/// Attach/update/remove the per-camera `PostFxSettings` so the pass runs only
/// when active (Standard + enabled); removing it disables the pass entirely.
pub fn sync_postfx(
    mut commands: Commands,
    st: Res<GraphState>,
    time: Res<Time>,
    cam_q: Query<Entity, With<Camera>>,
) {
    let cfg = st.cfg.postfx;
    let active = postfx_active(st.cfg.visual_theme, cfg.enabled);
    for cam in cam_q.iter() {
        if active {
            commands.entity(cam).insert(PostFxSettings {
                scanline: cfg.scanline,
                vignette: cfg.vignette,
                aberration: cfg.aberration,
                grain: cfg.grain,
                time: time.elapsed_seconds(),
            });
        } else {
            commands.entity(cam).remove::<PostFxSettings>();
        }
    }
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
    fn postfx_plugin_builds_without_render_app() {
        // No RenderApp (headless) → build/finish early-return without panic.
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Shader>();
        app.add_plugins(PostFxPlugin);
        app.finish();
    }
}
