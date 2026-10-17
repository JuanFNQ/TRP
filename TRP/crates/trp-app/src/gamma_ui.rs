//! The screens composed in gamma space, as the game's are (with the
//! options' `TARKOV UI`; `docs/spec/inventory.md`, "The screens' colours").
//!
//! The game's screens lay their pictures and colours over one another and
//! over the world by the stored, gamma-encoded numbers: a white at 0.149
//! over a dark ground of 18 comes to 51 in the user's capture (53 by the
//! numbers, 110 in linear light), and a black at 0.498 halves the world
//! behind a panel (0.5 of it there; 0.73 in linear light). Bevy blends in
//! linear light, which makes everything half clear lighter. So here the
//! frame is turned into its gamma-encoded numbers just before the screens
//! are drawn, the screens' two shaders (`ui.wgsl`, `ui_texture_slice.wgsl`)
//! work in those numbers, their colours and pictures encoded as they are
//! read, and the frame is decoded again after them. Where nothing is half
//! clear the picture is the same either way. The world is untouched.

use bevy::asset::{load_internal_asset, uuid_handle, AssetEvent};
use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::core_pipeline::FullscreenShader;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner};
use bevy::render::render_resource::binding_types::{sampler, texture_2d};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::{Shader, Source};
use bevy::ui_render::graph::NodeUi;

const GAMMA_UI_SHADER: Handle<Shader> = uuid_handle!("e3a9c6f1-2b47-4d8a-9f05-6c1d8e2b7a49");

/// The screens' shaders as Bevy has them, and the line in each where its
/// colour and picture are put together: there both are encoded.
const PATCHES: [(&str, &str, &str); 2] = [
    (
        "embedded://bevy_ui_render/ui.wgsl",
        "let color = select(in.color, in.color * texture_color, enabled(in.flags, TEXTURED));",
        "let color = select(gamma_numbers(in.color), gamma_numbers(in.color) * gamma_numbers(texture_color), enabled(in.flags, TEXTURED));",
    ),
    (
        "embedded://bevy_ui_render/ui_texture_slice.wgsl",
        "return in.color * textureSample(sprite_texture, sprite_sampler, atlas_uv);",
        "return gamma_numbers(in.color) * gamma_numbers(textureSample(sprite_texture, sprite_sampler, atlas_uv));",
    ),
];

/// What a patched shader gains, before its fragment shader: the sRGB
/// curve, a colour's numbers as the game's screens have them.
const GAMMA_NUMBERS: &str = "
// (trp: the screens composed in gamma space, `gamma_ui.rs`.)
fn gamma_numbers(c: vec4<f32>) -> vec4<f32> {
    let v = max(c.rgb, vec3(0.0));
    let encoded = select(1.055 * pow(v, vec3(1.0 / 2.4)) - 0.055, v * 12.92, v <= vec3(0.0031308));
    return vec4(encoded, c.a);
}
";

/// On the camera whose screens are composed so.
#[derive(Component, Clone, Copy, Default, ExtractComponent)]
pub struct GammaUi;

pub struct GammaUiPlugin;

impl Plugin for GammaUiPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, GAMMA_UI_SHADER, "gamma_ui.wgsl", Shader::from_wgsl);
        app.add_plugins(ExtractComponentPlugin::<GammaUi>::default()).add_systems(Startup, hold_shaders).add_systems(Update, patch_shaders);
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<GammaPipeline>>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_render_graph_node::<ViewNodeRunner<GammaNode<true>>>(Core3d, GammaLabel::Encode)
            .add_render_graph_node::<ViewNodeRunner<GammaNode<false>>>(Core3d, GammaLabel::Decode)
            .add_render_graph_edges(Core3d, (Node3d::EndMainPassPostProcessing, GammaLabel::Encode, NodeUi::UiPass, GammaLabel::Decode, Node3d::Upscaling));
    }
}

/// The screens' shaders, held so that each is patched as it arrives.
#[derive(Resource)]
struct HeldShaders(Vec<Handle<Shader>>);

fn hold_shaders(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(HeldShaders(PATCHES.iter().map(|(path, ..)| assets.load(*path)).collect()));
}

/// Each of the screens' shaders, as it is loaded (and again if it is ever
/// loaded afresh), is put back patched; a shader whose line is not found
/// (another Bevy) is left as it is, with a line saying so.
fn patch_shaders(held: Option<Res<HeldShaders>>, mut events: MessageReader<AssetEvent<Shader>>, mut shaders: ResMut<Assets<Shader>>) {
    let Some(held) = held else { return };
    for event in events.read() {
        let (AssetEvent::Added { id } | AssetEvent::Modified { id } | AssetEvent::LoadedWithDependencies { id }) = *event else { continue };
        let Some(at) = held.0.iter().position(|handle| handle.id() == id) else { continue };
        let Some(shader) = shaders.get(id) else { continue };
        let Source::Wgsl(source) = &shader.source else { continue };
        if source.contains("fn gamma_numbers(") {
            continue;
        }
        let (path, line, patched) = PATCHES[at];
        let Some(fragment) = source.find("@fragment") else { continue };
        if !source.contains(line) {
            warn!("{path}: its colour's line was not found; the screens are composed in linear light");
            continue;
        }
        let source = format!("{}{GAMMA_NUMBERS}{}", &source[..fragment], &source[fragment..]).replace(line, patched);
        let replaced = Shader::from_wgsl_with_defs(source, shader.path.clone(), shader.shader_defs.clone());
        let _ = shaders.insert(id, replaced);
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
enum GammaLabel {
    Encode,
    Decode,
}

#[derive(Resource)]
struct GammaPipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    fullscreen: FullscreenShader,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
struct GammaKey {
    format: TextureFormat,
    encode: bool,
}

impl SpecializedRenderPipeline for GammaPipeline {
    type Key = GammaKey;

    fn specialize(&self, key: GammaKey) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some(if key.encode { "gamma ui encode" } else { "gamma ui decode" }.into()),
            layout: vec![self.layout.clone()],
            vertex: self.fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: GAMMA_UI_SHADER,
                entry_point: Some(if key.encode { "encode" } else { "decode" }.into()),
                targets: vec![Some(ColorTargetState { format: key.format, blend: None, write_mask: ColorWrites::ALL })],
                ..default()
            }),
            ..default()
        }
    }
}

fn init_pipeline(mut commands: Commands, device: Res<RenderDevice>, fullscreen: Res<FullscreenShader>) {
    let layout = BindGroupLayoutDescriptor::new(
        "gamma_ui",
        &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (texture_2d(TextureSampleType::Float { filterable: false }), sampler(SamplerBindingType::NonFiltering))),
    );
    let sampler = device.create_sampler(&SamplerDescriptor { label: Some("gamma_ui"), ..default() });
    commands.insert_resource(GammaPipeline { layout, sampler, fullscreen: fullscreen.clone() });
}

#[derive(Component)]
struct GammaPipelines([CachedRenderPipelineId; 2]);

fn prepare(mut commands: Commands, views: Query<(Entity, &ViewTarget), With<GammaUi>>, cache: Res<PipelineCache>, pipeline: Res<GammaPipeline>, mut pipelines: ResMut<SpecializedRenderPipelines<GammaPipeline>>) {
    for (entity, target) in &views {
        let format = target.main_texture_format();
        let ids = [true, false].map(|encode| pipelines.specialize(&cache, &pipeline, GammaKey { format, encode }));
        commands.entity(entity).insert(GammaPipelines(ids));
    }
}

/// The frame encoded (`ENCODE`) before the screens, decoded after them.
#[derive(Default)]
struct GammaNode<const ENCODE: bool>;

impl<const ENCODE: bool> ViewNode for GammaNode<ENCODE> {
    type ViewQuery = (&'static ViewTarget, &'static GammaPipelines);

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (target, ids): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<GammaPipeline>();
        let cache = world.resource::<PipelineCache>();
        let Some(drawn) = cache.get_render_pipeline(ids.0[usize::from(!ENCODE)]) else { return Ok(()) };
        let write = target.post_process_write();
        let group = context.render_device().create_bind_group("gamma_ui", &cache.get_bind_group_layout(&pipeline.layout), &BindGroupEntries::sequential((write.source, &pipeline.sampler)));
        let mut render = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some(if ENCODE { "gamma_ui_encode" } else { "gamma_ui_decode" }),
            color_attachments: &[Some(RenderPassColorAttachment { view: write.destination, depth_slice: None, resolve_target: None, ops: Operations::default() })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render.set_render_pipeline(drawn);
        render.set_bind_group(0, &group, &[]);
        render.draw(0..3, 0..1);
        Ok(())
    }
}
