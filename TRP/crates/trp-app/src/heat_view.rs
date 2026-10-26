//! The weapon's parts glowing with its heat (`docs/spec/malfunctions.md`,
//! "The barrel's glow"; the `OVERHEATING` switch): each part the game's
//! prefab gives a `HotObject` whose material draws the glow
//! (`trp_data::heat`), drawn again over itself with the glow added
//! (`heat_glow.wgsl`), its heat's term `((T - 30) * 0.1)^2` from the weapon's
//! heat as its logic has it (`T = 10 * heat / 220 + 30`,
//! `HotObject.ConvertHeat2Celsio`: so `(heat / 220)^2`), on the rig in hand.
//! A part with `VisibleHeatAlpha` nought (a handguard, a gas block) only
//! warms in thermal vision: no glow is drawn for it.

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::light::NotShadowCaster;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError};
use bevy::shader::ShaderRef;

const HEAT_SHADER: Handle<Shader> = uuid_handle!("6d2f8e31-4b7a-4c95-9e08-1a3c5b7d9f24");

/// The globals' `HeatVisibleFactor`, which the install's `Overheat` leaves
/// out: the class's own 1 (`GlobalConfiguration`).
const HEAT_VISIBLE_FACTOR: f32 = 1.0;

/// `Heat` in `heat_glow.wgsl`.
#[derive(ShaderType, Debug, Clone, Copy, Default)]
pub struct HeatUniform {
    pub centre_term: Vec4,
    pub size_visible: Vec4,
    pub color_1: Vec4,
    pub color_2: Vec4,
    pub spec: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct HeatMaterial {
    #[uniform(0)]
    pub heat: HeatUniform,
}

impl Material for HeatMaterial {
    fn vertex_shader() -> ShaderRef {
        HEAT_SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        HEAT_SHADER.into()
    }

    /// Added over the part (Bevy's additive mode: the colour added, the
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

/// A part's glow, on a twin of the part's mesh: the rig it is of (its key),
/// and its figures (in Bevy's axes).
#[derive(Component, Clone)]
pub struct HotGlow {
    pub rig: String,
    pub centre: Vec3,
    pub size: Vec3,
    pub visible: f32,
    pub colors: [Vec4; 2],
    pub spec: Vec4,
}

pub struct HeatViewPlugin;

impl Plugin for HeatViewPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, HEAT_SHADER, "heat_glow.wgsl", Shader::from_wgsl);
        app.add_plugins(MaterialPlugin::<HeatMaterial>::default()).add_systems(Update, (dress_glows, follow_layers, glow_heat).chain());
    }
}

/// Twins for a rig's parts that glow, each a child of the entity that draws
/// its slot's mesh (`drawn`: the node, the slot and the entity), out of
/// sight until the weapon is hot. A part drawn on bones is left without
/// (none of the install's weapons sampled has one: `PROVISIONAL`).
pub fn spawn_glows(commands: &mut Commands, rig: &str, hot: &[trp_data::heat::HotPart], drawn: &[(usize, usize, Entity)], meshes: &[Vec<Handle<Mesh>>], skinned: &dyn Fn(usize) -> bool, layer: usize) {
    let vec4 = |c: [f32; 4]| Vec4::from_array(c);
    for part in hot.iter().filter(|part| part.visible > 0.0 && !skinned(part.node)) {
        for slot in &part.slots {
            let Some(&(_, _, entity)) = drawn.iter().find(|(node, at, _)| *node == part.node && *at == slot.slot) else { continue };
            let Some(mesh) = meshes.get(part.node).and_then(|meshes| meshes.get(slot.slot)) else { continue };
            let glow = HotGlow {
                rig: rig.to_owned(),
                centre: crate::convert::position(part.center),
                size: Vec3::from_array(part.size),
                visible: part.visible,
                colors: [vec4(slot.colors[0]), vec4(slot.colors[1])],
                spec: vec4(slot.spec),
            };
            let twin = commands.spawn((Mesh3d(mesh.clone()), Transform::default(), Visibility::Hidden, NotShadowCaster, NoFrustumCulling, RenderLayers::layer(layer), glow)).id();
            commands.entity(entity).add_child(twin);
        }
    }
}

/// Each new twin its own material.
fn dress_glows(mut commands: Commands, glows: Query<(Entity, &HotGlow), Without<MeshMaterial3d<HeatMaterial>>>, mut materials: ResMut<Assets<HeatMaterial>>) {
    for (entity, glow) in &glows {
        let heat = HeatUniform {
            centre_term: glow.centre.extend(0.0),
            size_visible: glow.size.max(Vec3::splat(1e-6)).extend(glow.visible),
            color_1: glow.colors[0],
            color_2: glow.colors[1],
            spec: Vec4::new(glow.spec.x, glow.spec.y, HEAT_VISIBLE_FACTOR, 0.0),
        };
        commands.entity(entity).insert(MeshMaterial3d(materials.add(HeatMaterial { heat })));
    }
}

/// A twin drawn where its part is: its layers kept as the part's.
fn follow_layers(mut twins: Query<(&ChildOf, &mut RenderLayers), With<HotGlow>>, layers: Query<&RenderLayers, Without<HotGlow>>) {
    for (parent, mut own) in &mut twins {
        if let Ok(theirs) = layers.get(parent.parent()) {
            if *own != *theirs {
                *own = theirs.clone();
            }
        }
    }
}

/// The rig in hand's twins given its heat's term, shown while it is above
/// nought; every other rig's out of sight.
fn glow_heat(
    hands: Option<Res<crate::weapon::HandsRig>>,
    stowed: Option<Res<crate::weapon::Stowed>>,
    options: Res<crate::Options>,
    mut glows: Query<(&HotGlow, &MeshMaterial3d<HeatMaterial>, &mut Visibility)>,
    mut materials: ResMut<Assets<HeatMaterial>>,
) {
    let held = match stowed.as_deref() {
        Some(stowed) => stowed.held().map(str::to_owned),
        None => Some(options.weapon.clone()),
    };
    let heat = hands.as_deref().and_then(|hands| hands.firearm.as_ref()).map_or(0.0, trp_sim::Firearm::heat);
    let term = (heat / 220.0).powi(2);

    for (glow, material, mut shown) in &mut glows {
        let on = held.as_deref() == Some(glow.rig.as_str()) && term > 0.0;
        let wanted = if on { Visibility::Inherited } else { Visibility::Hidden };
        if *shown != wanted {
            *shown = wanted;
        }
        // (Changed only when it moved: a material touched is sent again.)
        let stale = on && materials.get(&material.0).is_some_and(|m| (m.heat.centre_term.w - term).abs() > 1e-6);
        if stale {
            if let Some(material) = materials.get_mut(&material.0) {
                material.heat.centre_term.w = term;
            }
        }
    }
}
