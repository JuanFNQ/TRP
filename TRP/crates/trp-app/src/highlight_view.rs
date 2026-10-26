//! The game's ambient highlight with the options' `AMBIENT HIGHLIGHT` on,
//! under `CUSTOMS LIGHT` (`docs/spec/lighting.md`, "The ambient highlight";
//! `highlight.wgsl`): after the lighting the game draws the ambient again
//! over every surface whose material marks it with its stencil type
//! (Customs': the hands', 2, which the first-person arms' and the weapons'
//! materials carry), times the surface's colour and a multiplier by the
//! sun's height, its harmonics the time of day's without the sky's samples
//! and the light from above; none within an indoor volume (one less its
//! alpha).
//!
//! Here each mesh of the first-person rig (under the view's pivot) drawn
//! with such a material gets a twin drawn after it with the same mesh and
//! skin, adding that light (`AlphaMode::Add`). `PROVISIONAL`: things with
//! those materials elsewhere (a weapon on a table or on the ground) are not
//! highlighted, a twin for each of them costing the frame its draws; the
//! sum made in linear light as the scene's others are. The drawing alone:
//! nothing of the simulation changes.

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::light::NotShadowCaster;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError};
use bevy::shader::ShaderRef;

const HIGHLIGHT_SHADER: Handle<Shader> = uuid_handle!("3f8a1c52-7d94-4e06-b1a3-6c2e9f0d4b78");

/// The materials made with the game's stencil type for hands
/// (`_StencilType` 2), as they are made (`render_data`).
pub static HANDS_MATERIALS: LazyLock<Mutex<HashSet<AssetId<StandardMaterial>>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// A material of the hands' stencil type, noted as made.
pub fn note_hands(material: &Handle<StandardMaterial>) {
    if let Ok(mut set) = HANDS_MATERIALS.lock() {
        set.insert(material.id());
    }
}

fn is_hands(material: &Handle<StandardMaterial>) -> bool {
    HANDS_MATERIALS.lock().is_ok_and(|set| set.contains(&material.id()))
}

/// The highlight's figures for the run: the harmonics packed (the three
/// `a`, the three `b`, `c`, in the game's axes), the multiplier, and the
/// indoor volume (the world into its unit box) with its alpha.
#[derive(Resource, Clone, Copy)]
pub struct HighlightFigures {
    pub sh: [Vec4; 7],
    pub multiplier: f32,
    pub blur: f32,
    pub indoors: Option<(Mat4, f32)>,
}

/// `Highlight` in `highlight.wgsl`.
#[derive(ShaderType, Debug, Clone, Copy, Default)]
pub struct HighlightUniform {
    pub base_color: Vec4,
    pub sh: [Vec4; 7],
    /// The multiplier, whether a normal map is given, the indoor volume's
    /// alpha (nought for none), the blur's power (`_AmbientBlur`, one over
    /// the script's).
    pub figures: Vec4,
    pub indoors: Mat4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct HighlightMaterial {
    #[uniform(0)]
    pub figures: HighlightUniform,
    #[texture(1)]
    #[sampler(2)]
    pub base: Option<Handle<Image>>,
    #[texture(3)]
    #[sampler(4)]
    pub normal: Option<Handle<Image>>,
}

impl Material for HighlightMaterial {
    fn fragment_shader() -> ShaderRef {
        HIGHLIGHT_SHADER.into()
    }

    /// Added over what is drawn (Bevy's additive mode: the colour added, the
    /// alpha nought), testing the depth and writing none.
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(_: &MaterialPipeline, descriptor: &mut RenderPipelineDescriptor, _: &MeshVertexBufferLayoutRef, _: MaterialPipelineKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = false;
        }
        Ok(())
    }
}

/// A twin drawing the highlight over its parent's mesh.
#[derive(Component)]
pub struct HighlightTwin;

/// The highlight's material for each source material, made as needed.
#[derive(Resource, Default)]
struct Made(HashMap<AssetId<StandardMaterial>, Handle<HighlightMaterial>>);

/// Each new mesh of the first-person rig drawn with a hands' material gets
/// its twin.
#[allow(clippy::type_complexity)]
fn attach(
    mut commands: Commands,
    figures: Res<HighlightFigures>,
    mut made: ResMut<Made>,
    standard: Res<Assets<StandardMaterial>>,
    mut materials: ResMut<Assets<HighlightMaterial>>,
    meshes: Query<(Entity, &Mesh3d, &MeshMaterial3d<StandardMaterial>, Option<&SkinnedMesh>, Option<&RenderLayers>), (Added<MeshMaterial3d<StandardMaterial>>, Without<HighlightTwin>)>,
    parents: Query<&ChildOf>,
    pivots: Query<(), With<crate::weapon::ViewPivot>>,
) {
    for (entity, mesh, material, skin, layers) in &meshes {
        if !is_hands(&material.0) || !parents.iter_ancestors(entity).any(|a| pivots.contains(a)) {
            continue;
        }
        let Some(source) = standard.get(&material.0) else { continue };
        let handle = made
            .0
            .entry(material.0.id())
            .or_insert_with(|| {
                let c = source.base_color.to_linear();
                let indoors = figures.indoors.unwrap_or((Mat4::ZERO, 0.0));
                materials.add(HighlightMaterial {
                    figures: HighlightUniform {
                        base_color: Vec4::new(c.red, c.green, c.blue, 1.0),
                        sh: figures.sh,
                        figures: Vec4::new(figures.multiplier, if source.normal_map_texture.is_some() { 1.0 } else { 0.0 }, indoors.1, 1.0 / figures.blur.max(1e-3)),
                        indoors: indoors.0,
                    },
                    base: source.base_color_texture.clone(),
                    normal: source.normal_map_texture.clone(),
                })
            })
            .clone();
        let mut twin = commands.spawn((Mesh3d(mesh.0.clone()), MeshMaterial3d(handle), Transform::default(), NotShadowCaster, NoFrustumCulling, HighlightTwin));
        if let Some(skin) = skin {
            twin.insert(skin.clone());
        }
        if let Some(layers) = layers {
            twin.insert(layers.clone());
        }
        let twin = twin.id();
        commands.entity(entity).add_child(twin);
    }
}

/// A twin drawn where its parent is: its layers kept as the parent's.
fn follow_layers(mut twins: Query<(&ChildOf, Option<&mut RenderLayers>, Entity), With<HighlightTwin>>, layers: Query<&RenderLayers, Without<HighlightTwin>>, mut commands: Commands) {
    for (parent, own, entity) in &mut twins {
        let theirs = layers.get(parent.parent()).ok();
        match (own, theirs) {
            (Some(mut own), Some(theirs)) if *own != *theirs => *own = theirs.clone(),
            (None, Some(theirs)) => {
                commands.entity(entity).insert(theirs.clone());
            }
            (Some(_), None) => {
                commands.entity(entity).remove::<RenderLayers>();
            }
            _ => {}
        }
    }
}

pub struct HighlightPlugin;

impl Plugin for HighlightPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, HIGHLIGHT_SHADER, "highlight.wgsl", Shader::from_wgsl);
        app.add_plugins(MaterialPlugin::<HighlightMaterial>::default())
            .init_resource::<Made>()
            .add_systems(PostUpdate, (attach, follow_layers).chain().run_if(resource_exists::<HighlightFigures>).before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate));
    }
}
