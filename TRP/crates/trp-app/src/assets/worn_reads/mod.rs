//! The worn things read when they are first worn.
//!
//! The body is made with what is worn as the run begins, by the character
//! and by the dummies (`body_view::BodyParts::load`). A thing put on later,
//! on the character or on a dummy (searching), that the run has not read
//! is read then, away from the frame: its meshes for every case of what it
//! may be worn over (`body_view::ALL_UNDER`), or its model on the head with
//! the parts fitted to it, its pictures made ready (`PreparedLooks`), on a
//! thread with a reader of its own and the run's database (`GameData::db`),
//! one thing at a time, as a suit chosen is read (`clothing`). Into the
//! engine's stores on the frame it comes; waited for on the graphics card
//! (`UploadWatch`), and then hung on the body and on every copy kept (the
//! gear page's figure, the corpse, the dummies), shown by the same rules as
//! those read with the body (`body_view::show_worn`, `show_head_parts`,
//! `show_torso`; `dummy::show_dummies`). Until then it is not drawn: the
//! game, too, draws a thing put on once its bundle has loaded.
//!
//! A thing that cannot be read is not asked for again; one that is read is
//! kept for the rest of the run.

use std::collections::HashSet;
use std::sync::Arc;

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use trp_data::character::dress::{Under, Worn};
use trp_data::unity::Assets as UnityAssets;

use crate::player::body_view::{BandMeshes, Banded, BodyParts, BodyRig, Fitted, HeadModel, HeadModels, UpperBody, Wearer, WornPiece, SEEN};
use crate::assets::render_data::PreparedLooks;

mod prepare_pieces;
mod reading;

pub use reading::*;
use prepare_pieces::{prepare};

/// What the uploads are waited for under, before the template.
const WAITED: &str = "a worn thing, ";

pub struct WornReadsPlugin;

impl Plugin for WornReadsPlugin {
    fn build(&self, app: &mut App) {
        // After the inventory has put things on (`settle`), and before what
        // is worn is shown (`show_worn`): a thing hung on a frame is shown
        // on it.
        app.add_systems(Update, read_worn.after(crate::screens::inventory_view::Settled).before(crate::player::body_view::show_worn).before(crate::player::body_view::fit_head_parts));
    }
}

/// The things read and being read, and the thread's reader.
#[derive(Resource)]
pub struct WornReads {
    install: trp_data::Install,
    db: Arc<trp_data::Database>,
    /// Every thing read with the body, read since, being read, or that
    /// could not be read: none of them is asked for again.
    known: HashSet<String>,
    /// Whether `known` has been given what the body was made with.
    begun: bool,
    /// The options' `HEAD FITTING` as the run began (`BodyParts::fitting`):
    /// on, a thing's meshes' second forms are read with it. And `ARMBAND`
    /// (`BodyParts::armband`): off, an armband is not read.
    fitting: bool,
    armband: bool,
    worker: Option<UnityAssets>,
    task: Option<Task<(Option<UnityAssets>, String, anyhow::Result<Prepared>)>>,
    /// Read and in the stores, waited for on the graphics card.
    loaded: Vec<(String, Loaded)>,
}

impl WornReads {
    pub fn new(install: trp_data::Install, db: Arc<trp_data::Database>) -> Self {
        Self { install, db, known: HashSet::new(), begun: false, fitting: false, armband: false, worker: None, task: None, loaded: Vec::new() }
    }
}

/// A skinned piece made ready: its bones, its submeshes and their
/// materials (places among the looks'), its bind poses, and the cases of
/// what it is worn over it is the mesh for.
struct PreparedPiece {
    bone_paths: Vec<String>,
    meshes: Vec<usize>,
    materials: Vec<usize>,
    poses: Vec<Mat4>,
    over: Vec<Under>,
    /// Its second form (`body_view::Fitted`): its kind, a mesh for each of
    /// `meshes` and its bind poses.
    second: Option<(u8, Vec<usize>, Vec<Mat4>)>,
    /// An armband skin's mesh for every upper body (`body_view::
    /// BandMeshes`): each set's id with the place of its mesh, and each
    /// mesh's submeshes and bind poses (the first is `meshes`'s).
    band: Option<(Vec<(String, usize)>, Vec<(Vec<usize>, Vec<Mat4>)>)>,
    /// Its levels of detail, where they are read (`lod_view`): its group,
    /// placed under the player's mesh transform, and its levels.
    lod: Option<(Arc<crate::assets::lod_view::LodSpec>, u8)>,
}

enum PreparedShape {
    Skinned(Vec<PreparedPiece>),
    /// A model on the head, and each of its objects' submeshes and
    /// materials (places among the looks'), by the object's index; and the
    /// meshes of its second form where it has one.
    Rigid(HeadModel, Vec<Vec<(usize, usize)>>, Vec<Vec<usize>>),
}

struct Prepared {
    shape: PreparedShape,
    /// Armour worn like a vest (`VestlikeArmor`).
    vestlike: bool,
    looks: PreparedLooks,
}

struct LoadedPiece {
    bone_paths: Vec<String>,
    drawn: Vec<(Handle<Mesh>, Handle<StandardMaterial>)>,
    poses: Handle<SkinnedMeshInverseBindposes>,
    over: Vec<Under>,
    second: Option<(u8, Vec<Handle<Mesh>>, Handle<SkinnedMeshInverseBindposes>)>,
    band: Option<Arc<BandMeshes>>,
    lod: Option<crate::assets::lod_view::LodPiece>,
}

enum Loaded {
    Skinned(Vec<LoadedPiece>, bool),
    Rigid(HeadModel, Vec<Vec<(Handle<Mesh>, Handle<StandardMaterial>)>>, Vec<Vec<Handle<Mesh>>>),
}
