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
//!
//! The game's whole project is in gamma space (its `PlayerSettings`'
//! `m_ActiveColorSpace` 0, Gamma): its camera's effects too work in the
//! gamma-encoded numbers and read their pictures as stored. So while any of
//! them is on (a hit's or the inventory's blur, blood, a contusion's and
//! health's others, a shot's flash and smoke, the fumes, sparks and haze,
//! the explosions' and hits' systems: `screen_blur`, `blood_view`,
//! `contusion_view`, `muzzle_view`, `particles_view`, `explosion_view`; and
//! the world's passes, on whenever their switches are: the ambient
//! occlusion, the sky's scattering, the clouds) the
//! frame is encoded before the first and decoded after the last, by the
//! same two passes; with the screens composed so too, the effects' decode
//! and the screens' encode would undo each other, and neither runs. The
//! optic camera's picture goes both ways by the same power with a level's
//! light (`KeepFigures`).

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

/// On a camera lit by a level's light whose picture stays in the scene's
/// figures after its effects, with no PRISM of its own to take it to the
/// screen's (the optic camera: the main camera's lens reads its picture as
/// the scene's own): its frame goes to the game's numbers by the power, as
/// the main camera's, and back by the same power, so that the scene comes
/// back as it went.
#[derive(Component, Clone, Copy, Default, ExtractComponent)]
pub struct KeepFigures;

/// The camera's effects in gamma space, always; the screens too where `ui`
/// (`TARKOV UI`). Added after the effects' own plugins, whose nodes it goes
/// round.
pub struct GammaUiPlugin {
    pub ui: bool,
}

impl Plugin for GammaUiPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, GAMMA_UI_SHADER, "gamma_ui.wgsl", Shader::from_wgsl);
        app.add_plugins(ExtractComponentPlugin::<KeepFigures>::default());
        if self.ui {
            app.add_plugins(ExtractComponentPlugin::<GammaUi>::default()).add_systems(Startup, hold_shaders).add_systems(Update, patch_shaders);
        }
        let ui = self.ui;
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<GammaPipeline>>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_render_graph_node::<ViewNodeRunner<GammaNode<true, false>>>(Core3d, GammaLabel::EffectsEncode)
            .add_render_graph_node::<ViewNodeRunner<GammaNode<false, false>>>(Core3d, GammaLabel::EffectsDecode)
            .add_render_graph_edges(
                Core3d,
                (Node3d::Tonemapping, GammaLabel::EffectsEncode, crate::muzzle_view::DepthCopyLabel, crate::explosion_view::ExplosionLabel, crate::muzzle_view::JetLabel, crate::screen_blur::HitBlurLabel),
            )
            .add_render_graph_edges(Core3d, (crate::contusion_view::NearDeathLabel, GammaLabel::EffectsDecode, Node3d::EndMainPassPostProcessing));
        if ui {
            render_app
                .add_render_graph_node::<ViewNodeRunner<GammaNode<true, true>>>(Core3d, GammaLabel::Encode)
                .add_render_graph_node::<ViewNodeRunner<GammaNode<false, true>>>(Core3d, GammaLabel::Decode)
                .add_render_graph_edges(Core3d, (Node3d::EndMainPassPostProcessing, GammaLabel::Encode, NodeUi::UiPass, GammaLabel::Decode, Node3d::Upscaling));
        }
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
pub(crate) enum GammaLabel {
    Encode,
    Decode,
    EffectsEncode,
    EffectsDecode,
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
    /// Encoded by a plain 2.2 power, not the sRGB curve: the scene lit by a
    /// level's light (`lighting`), whose figures are made linear by it.
    power: bool,
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
                entry_point: Some(match (key.encode, key.power) {
                    (true, true) => "encode_power",
                    (true, false) => "encode",
                    (false, true) => "decode_power",
                    (false, false) => "decode",
                }
                .into()),
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

/// The encoding, the decoding, and the encoding and decoding by a power.
#[derive(Component)]
struct GammaPipelines([CachedRenderPipelineId; 4]);

fn prepare(mut commands: Commands, views: Query<(Entity, &ViewTarget)>, cache: Res<PipelineCache>, pipeline: Res<GammaPipeline>, mut pipelines: ResMut<SpecializedRenderPipelines<GammaPipeline>>) {
    for (entity, target) in &views {
        let format = target.main_texture_format();
        let ids = [(true, false), (false, false), (true, true), (false, true)].map(|(encode, power)| pipelines.specialize(&cache, &pipeline, GammaKey { format, encode, power }));
        commands.entity(entity).insert(GammaPipelines(ids));
    }
}

/// The frame encoded (`ENCODE`) before the screens (`UI`) or the camera's
/// effects, decoded after them.
#[derive(Default)]
struct GammaNode<const ENCODE: bool, const UI: bool>;

impl<const ENCODE: bool, const UI: bool> ViewNode for GammaNode<ENCODE, UI> {
    type ViewQuery = (
        &'static ViewTarget,
        &'static GammaPipelines,
        Has<GammaUi>,
        Has<crate::screen_blur::ScreenBlur>,
        Has<crate::blood_view::BloodScreen>,
        Has<crate::contusion_view::ContusionScreen>,
        Has<crate::muzzle_view::JetRig>,
        Has<crate::muzzle_view::SmokeScreen>,
        Has<crate::prism_view::PrismScreen>,
        // The particles' effects, drawn on the frame in the game's numbers
        // as the flash is.
        (Has<crate::particles_view::FumeScreen>, Has<crate::particles_view::SparkScreen>, Has<crate::particles_view::HazeScreen>, Has<crate::explosion_view::ExplosionScreen>),
        // The world's passes after the depth's copy, the game's image
        // effects on its gamma-space frame: the ambient occlusion, the
        // sky's scattering, the clouds. Each is extracted on every frame it
        // is on, so the frame is in the game's numbers for it always, not
        // only while another effect shows (the occlusion was darker while a
        // shot or the inventory's blur was on, until 0.109.1).
        (Has<crate::hbao_view::HbaoScreen>, Has<crate::scattering_view::ScatteringScreen>, Has<crate::clouds_view::CloudsScreen>),
        Has<KeepFigures>,
        // The face shield's mask, the camera's effect after pain's blur.
        Has<crate::visor_view::VisorScreen>,
    );

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (target, ids, screens, blur, blood, effects, jets, smoke, prism, particles, passes, keep, visor): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<GammaPipeline>();
        let cache = world.resource::<PipelineCache>();
        // Both of a pair, or neither: the frame is never left encoded.
        let [Some(encode), Some(decode), Some(power), Some(power_back)] = ids.0.map(|id| cache.get_render_pipeline(id)) else { return Ok(()) };
        // The camera's effects, each extracted only while it is on (PRISM
        // always, with `CUSTOMS LIGHT`).
        let (fumes, sparks, haze, explosions) = particles;
        let (occlusion, scattering, clouds) = passes;
        let effects = blur || blood || effects || jets || smoke || prism || fumes || sparks || haze || explosions || occlusion || scattering || clouds || visor;
        let run = match (UI, ENCODE) {
            (true, true) => screens && !effects,
            (true, false) => screens,
            (false, true) => effects,
            (false, false) => effects && !screens,
        };
        if !run {
            return Ok(());
        }
        // The scene lit by a level's light goes to the game's numbers by
        // the power its figures were made linear by (`lighting`); the
        // screens' and the way back by the sRGB curve the screen itself
        // takes them by.
        // A picture kept in the scene's figures (`KeepFigures`) comes back
        // by the power it went by.
        let drawn = match (ENCODE, UI) {
            (true, false) if prism || keep => power,
            (true, _) => encode,
            (false, false) if keep && !prism => power_back,
            _ => decode,
        };
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
