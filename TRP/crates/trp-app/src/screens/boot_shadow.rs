//! The shadow at the OVERALL figure's feet.
//!
//! The game's menu figure has it as an object of its prefab,
//! `BootShadow`, which `MenuPlayerPoser.BottomShadow` names and
//! `PlayerModelLoader.CreatePlayerBody` switches on for the screens'
//! figure: five of Unity's own planes on the prefab's root, under the
//! soles, so they turn with the figure as it is dragged and do not follow
//! the animated feet (`trp_data::body::boot_shadow`, read with the figure).
//! Each is drawn by `Unlit/Transparent Colored`: its picture (black, the
//! shape in its alpha) times the vertex colour (white: the plane has
//! none), unlit, blended by its alpha for colour **and alpha alike**, depth
//! tested and not written, both faces, offset -1, -1, after what is
//! opaque. Over the view's cleared picture a plane so leaves black at an
//! alpha of a² (a the texel's), which the page lays over the panel: the
//! panel times 1 - a².
//!
//! Here: read on a worker with a reader of its own as the run begins (the
//! shaders' bundle its check opens costs 150 ms; `effects_reads` is the
//! same way), the planes made on the OVERALL tab's own figure's root
//! (`figure_view::Figure::overall_root`) once both are there, on its
//! layer; drawn by the standard material unlit and blended, its blend for
//! alpha made the colour's and its depth offset the shader's
//! (`specialize`). With `OVERALL FIGURE` alone, as its figure.
//! `PROVISIONAL`: the planes are sorted as Bevy sorts what is blended (by
//! their middles' distance; the engine's order not read), which changes
//! where they overlap a little.

use bevy::camera::visibility::RenderLayers;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::math::Affine2;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, DepthBiasState, RenderPipelineDescriptor, SpecializedMeshPipelineError};
use bevy::tasks::{AsyncComputeTaskPool, Task};

use crate::platform::convert;
use crate::assets::render_data::Role;

/// The standard material, its alpha blended as its colour is.
pub type BootShadowMaterial = ExtendedMaterial<StandardMaterial, AlphaAsColour>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct AlphaAsColour {}

impl MaterialExtension for AlphaAsColour {
    /// `Blend SrcAlpha OneMinusSrcAlpha` for colour and alpha, and `Offset
    /// -1, -1`: towards the eye, which with Bevy's reversed depth is up.
    fn specialize(_: &MaterialExtensionPipeline, descriptor: &mut RenderPipelineDescriptor, _: &MeshVertexBufferLayoutRef, _: MaterialExtensionKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        let by_alpha = BlendComponent { src_factor: BlendFactor::SrcAlpha, dst_factor: BlendFactor::OneMinusSrcAlpha, operation: BlendOperation::Add };
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten().filter(|target| target.blend.is_some()) {
                target.blend = Some(BlendState { color: by_alpha, alpha: by_alpha });
            }
        }
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.bias = DepthBiasState { constant: 1, slope_scale: 1.0, clamp: 0.0 };
        }
        Ok(())
    }
}

/// The shadow being read, then read, until it is made on the figure.
#[derive(Resource)]
enum Read {
    Coming(Task<anyhow::Result<trp_data::character::body::BootShadow>>),
    Come(trp_data::character::body::BootShadow),
}

/// With `OVERALL FIGURE` and its figure read, the shadow read from that
/// install.
pub struct BootShadowPlugin {
    pub install: trp_data::Install,
}

impl Plugin for BootShadowPlugin {
    fn build(&self, app: &mut App) {
        let install = self.install.clone();
        let task = AsyncComputeTaskPool::get().spawn(async move {
            let began = std::time::Instant::now();
            let mut assets = trp_data::unity::Assets::new(install)?;
            let shadow = trp_data::character::body::boot_shadow(&mut assets)?;
            info!("the shadow at the OVERALL figure's feet: read on a worker in {:.0} ms", began.elapsed().as_secs_f32() * 1000.0);
            Ok(shadow)
        });
        app.add_plugins(MaterialPlugin::<BootShadowMaterial>::default()).insert_resource(Read::Coming(task)).add_systems(Update, hang);
    }
}

/// The shadow made on the OVERALL tab's own figure's root once that is
/// built: each of its objects where the prefab has it, the shadow's own
/// switched on (the game's `enableBottomShadow`, true for the screens'
/// figure), the others as their own flags have them; each plane its mesh
/// and its picture, each made once.
/// (An unattended run waits for the read once the figure is there, so
/// that its pictures are the same at any pace.)
fn hang(
    mut commands: Commands,
    read: Option<ResMut<Read>>,
    figure: Option<Res<crate::screens::figure_view::Figure>>,
    options: Res<crate::Options>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<BootShadowMaterial>>,
) {
    let Some(mut read) = read else { return };
    if let Read::Coming(task) = &mut *read {
        match crate::assets::background::take(task, options.unattended() && figure.is_some(), "the shadow at the OVERALL figure's feet") {
            None => return,
            Some(Ok(shadow)) => *read = Read::Come(shadow),
            Some(Err(e)) => {
                warn!("the shadow at the OVERALL figure's feet could not be read; it is not drawn: {e:#}");
                commands.remove_resource::<Read>();
                return;
            }
        }
    }
    let (Read::Come(shadow), Some(figure)) = (&*read, figure) else { return };
    commands.remove_resource::<Read>();
    let Some(root) = figure.overall_root() else { return };
    let made: Vec<Vec<Handle<Mesh>>> = shadow.meshes.iter().map(|data| crate::assets::render_data::meshes_of(data).into_iter().map(|mesh| meshes.add(mesh)).collect()).collect();
    let pictures: Vec<Option<Handle<Image>>> = shadow
        .pictures
        .iter()
        .map(|picture| match crate::assets::render_data::build_image(picture, Role::Color) {
            Ok(image) => Some(images.add(image)),
            Err(e) => {
                warn!("the shadow at the OVERALL figure's feet: its picture {} could not be made: {e:#}", picture.name);
                None
            }
        })
        .collect();
    let layer = RenderLayers::layer(crate::screens::figure_view::OVERALL_LAYER);
    let mut entities: Vec<Entity> = Vec::with_capacity(shadow.nodes.len());
    let mut planes = 0;
    for (index, node) in shadow.nodes.iter().enumerate() {
        let on = index == shadow.switched || node.active;
        let visibility = if on { Visibility::Inherited } else { Visibility::Hidden };
        let entity = commands.spawn((Name::new(format!("{}, the OVERALL figure's shadow", node.name)), convert::transform(node.translation, node.rotation, node.scale), visibility)).id();
        commands.entity(node.parent.map_or(root, |parent| entities[parent])).add_child(entity);
        entities.push(entity);
        let Some(plane) = node.plane else { continue };
        let Some(picture) = pictures.get(plane.picture).cloned().flatten() else { continue };
        let material = materials.add(BootShadowMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(picture),
                uv_transform: Affine2::from_scale_angle_translation(Vec2::from(plane.scale), 0.0, Vec2::from(plane.offset)),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                double_sided: true,
                fog_enabled: false,
                ..default()
            },
            extension: AlphaAsColour {},
        });
        for mesh in made.get(plane.mesh).into_iter().flatten() {
            let piece = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default(), Visibility::Inherited, layer.clone(), NotShadowCaster, NotShadowReceiver)).id();
            commands.entity(entity).add_child(piece);
        }
        planes += 1;
    }
    info!("the shadow at the OVERALL figure's feet: {planes} planes on its root");
}
