//! The game's deferred decals as the app lays them.
//! After the grenades spec, "The scorch"; `trp_sim::decals`: a grenade's scorch, where its effect
//! lays one (`Effect.Emit`, `DeferredDecalRenderer.DrawDecal`), as one of Bevy's clustered decals
//! over the decal's box, at most the renderer's number at once (the oldest taken away). The game
//! draws the box into its G-buffer before its light; Bevy's clustered decals change a surface's own
//! values before its light, and their step is patched as Bevy's shaders load (as `shading` patches
//! its light) to the game's shader's rule: the surface's colour blended toward the picture's by its
//! alpha in the game's gamma numbers, only on a surface facing the decal's normal within 0.8 (its
//! dot), its emission toward nothing by the same; the picture's level by its size on the screen
//! (`sample` in the game's). The game's stencil test (the decal's stencil type, 0, against the
//! surface's: characters 1, hands 2) is kept by each mesh's tag: a mesh of a material whose
//! `_StencilType` is not nought is tagged with it (`note_stencil`), in the tag's top two bits (the
//! rest is the levels of detail's band, `lod_view`), and takes no decal.
//!
//! `PROVISIONAL`: Bevy's decals share one sampler, the first image's (the
//! clouds' cookie's with `CUSTOMS CLOUDS`); overlapping decals are blended
//! in Bevy's order, the game's in the order laid. Not built: a decal's
//! specular and normal parts (the grenade's material has neither: its
//! weights nought), its colour other than white, the occlusion it writes
//! (the game's ambient takes none on its diffuse part: lighting spec).
//!
//! A bullet's hole (ballistics spec, "What a hit shows and sounds"): the
//! renderer's decal for the struck material (`_decals` by
//! `DecalMaterialType`, else `_genericDecal`), where its effect lays one,
//! by the same box (its own sizes and the renderer's projector height); its
//! tile of the sheet drawn as the game draws it (`GenerateTangents`). Bevy's
//! decal step takes at most eight pictures at once (the clouds' cookie and
//! the scorch among them), so the bullets' sheets are packed into one atlas
//! and each hole's place in it, its tile and its sheet's cells ride in the
//! decal's tag, which the game's step reads. One ring of 200 places runs
//! through every sheet's static holes (`_currentDecalIndex`); a hole on a
//! thing that moves (a door's leaf) follows it, from a ring of its own
//! (`_maxDynamicDecals`). `PROVISIONAL`: the atlas's own levels, made from
//! it (the pictures' own are not used), so a hole far off may take a
//! little of its neighbours; the moving holes drawn with the static
//! material's picture (`DynamicDecalMaterial` not read); a cutout sheet
//! (soil's) blended as the rest. Not built: their normals and specular.
//!
//! Blood (body hits spec, "What a body's hit shows"): behind a living
//! dummy's hit that went through, on what the bullet's line meets within
//! 5 m (`EmitBloodOnEnvironment`: the renderer's `_environmentBlood`), laid
//! at once, before the frame's holes; and under a character bleeding
//! heavily, every 1 to 3 s where a line 10 m down from its feet meets
//! something (`EmitBleeding`: `_bleedingDecal`). Each laid as a still
//! thing's hole is, in the same ring; each its own picture (with the
//! picture's own levels), its colour (`_Color`, red) put on every texel as
//! the shader puts it on the sample (exact for a colour of noughts and
//! ones). Not built: their specular and normals, as the holes'.

use std::collections::{HashMap, VecDeque};
use std::sync::{LazyLock, Mutex};

use bevy::asset::AssetEvent;
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::ClusteredDecal;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::mesh::MeshTag;
use bevy::prelude::*;
use bevy::shader::{Shader, Source};
use trp_sim::weapons::recoil::Rng;

mod holes;
mod scorches_patch;

pub use holes::*;
pub(crate) use scorches_patch::*;
use holes::{forget_bleedings, read_holes, receive_holes};
use scorches_patch::{hold, lay, patch, tag_by_stencil};

/// The game's stencil type of each material made with one not nought
/// (`_StencilType`: 1 characters, 2 hands), as they are made
/// (`render_data`).
static STENCIL_MATERIALS: LazyLock<Mutex<HashMap<AssetId<StandardMaterial>, u8>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// A material noted with its stencil type as made.
pub fn note_stencil(material: &Handle<StandardMaterial>, stencil: u8) {
    if stencil == 0 {
        return;
    }
    if let Ok(mut map) = STENCIL_MATERIALS.lock() {
        map.insert(material.id(), stencil);
    }
}

fn stencil_of(material: &Handle<StandardMaterial>) -> u8 {
    STENCIL_MATERIALS.lock().ok().and_then(|map| map.get(&material.id()).copied()).unwrap_or(0)
}

/// The grenade decal read, its picture, the decals laid (oldest first).
#[derive(Resource)]
pub(crate) struct Scorches {
    decal: trp_data::drawing::particles::DeferredDecal,
    picture: Handle<Image>,
    /// The effects that lay a deferred decal, by name.
    laying: Vec<String>,
    laid: VecDeque<Entity>,
    rng: Rng,
}

pub struct DecalPlugin {
    /// The explosions drawn (`PARTICLES`) and the hard settings'
    /// `DEFERRED_DECALS_ENABLED`.
    pub on: bool,
}

impl Plugin for DecalPlugin {
    fn build(&self, app: &mut App) {
        if !self.on {
            app.add_systems(Update, forget_bleedings);
            return;
        }
        app.add_systems(Startup, (read_holes, hold))
            .add_systems(Update, (receive_holes.after(crate::weapons::bullets::fly_bullets).before(lay_holes).before(lay_bleeding), patch, tag_by_stencil, lay.after(crate::weapons::bullets::fly_bullets).run_if(resource_exists::<Scorches>), lay_bleeding.after(lay_holes)));
    }
}
