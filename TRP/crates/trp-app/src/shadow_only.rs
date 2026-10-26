//! A mesh drawn as its shadow alone, as Unity draws a renderer whose
//! shadow casting is "shadows only" (the game's first-person body and head:
//! `skin_Body` and `skin_Head` recorded `ShadowsOnly`).
//!
//! Until 0.13.0 this was a render layer the camera did not draw and the sun
//! did. Bevy 0.17 on queues a mesh into a camera's shadow maps only if it is
//! on that camera's layers (`queue_shadows`), so such a mesh cast nothing:
//! in first person the shadow had the legs and no body above them
//! (`docs/findings.md`). Now the mesh stays on the camera's layer and is
//! drawn with a twin of its standard material whose fragment shader keeps
//! no pixel (`shadow_only.wgsl`); the shadow pass, which runs no fragment
//! shader of the material for an opaque one and the standard one for a
//! masked one, writes its depth as before. With the camera's normal prepass
//! (the ambient occlusion's, `hbao_view`) its prepass keeps no pixel either
//! (`shadow_only_prepass.wgsl`), where a depth prepass of it would hide
//! what is behind it. The drawing alone: nothing of the simulation changes.

use std::collections::HashMap;

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

const SHADOW_ONLY_SHADER: Handle<Shader> = uuid_handle!("2b7d3e51-8c4a-4f0e-9a61-3d5e7c2b9f14");
const SHADOW_ONLY_PREPASS_SHADER: Handle<Shader> = uuid_handle!("6f1c8a93-2d54-4b7e-a0c3-91e5d7b42f68");

/// The standard material, its pixels never drawn: its shadow alone.
pub type ShadowOnlyMaterial = ExtendedMaterial<StandardMaterial, NoPixels>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct NoPixels {}

impl MaterialExtension for NoPixels {
    fn fragment_shader() -> ShaderRef {
        SHADOW_ONLY_SHADER.into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        SHADOW_ONLY_PREPASS_SHADER.into()
    }
}

/// Whether a mesh is to be drawn as its shadow alone; its material is
/// changed for its twin and back to match (`apply`).
#[derive(Component, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct ShadowOnly(pub bool);

/// The standard material a mesh drawn as its shadow alone is drawn with
/// otherwise.
#[derive(Component)]
pub struct Drawn(Handle<StandardMaterial>);

/// Each mesh whose wish differs from how it is drawn is changed over: to
/// the twin of its material (made once for each material and shared), or
/// back to the material itself.
pub fn apply(
    mut commands: Commands,
    meshes: Query<(Entity, &ShadowOnly, Option<&MeshMaterial3d<StandardMaterial>>, Option<&Drawn>), crate::park::EvenParked>,
    standard: Res<Assets<StandardMaterial>>,
    mut shadows: ResMut<Assets<ShadowOnlyMaterial>>,
    mut twins: Local<HashMap<AssetId<StandardMaterial>, Handle<ShadowOnlyMaterial>>>,
) {
    for (entity, wish, drawn_with, kept) in &meshes {
        match (wish.0, drawn_with, kept) {
            (true, Some(material), None) => {
                let Some(base) = standard.get(&material.0) else { continue };
                let twin = twins.entry(material.0.id()).or_insert_with(|| shadows.add(ShadowOnlyMaterial { base: base.clone(), extension: NoPixels {} })).clone();
                commands.entity(entity).remove::<MeshMaterial3d<StandardMaterial>>().insert((MeshMaterial3d(twin), Drawn(material.0.clone())));
            }
            (false, _, Some(Drawn(material))) => {
                commands.entity(entity).remove::<(MeshMaterial3d<ShadowOnlyMaterial>, Drawn)>().insert(MeshMaterial3d(material.clone()));
            }
            _ => {}
        }
    }
}

pub struct ShadowOnlyPlugin;

impl Plugin for ShadowOnlyPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, SHADOW_ONLY_SHADER, "shadow_only.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, SHADOW_ONLY_PREPASS_SHADER, "shadow_only_prepass.wgsl", Shader::from_wgsl);
        app.add_plugins(MaterialPlugin::<ShadowOnlyMaterial>::default()).add_systems(PostUpdate, apply);
    }
}
