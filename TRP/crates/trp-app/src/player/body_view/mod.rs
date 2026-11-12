//! The body as drawn: its clothing skinned to a skeleton that the
//! simulation's body animator poses.
//!
//! A piece of clothing is a skinned mesh that names its bones by their
//! path on the player's skeleton (`trp_data::arms::load` reads one; the
//! arms are the same kind of thing). The bones those meshes hang on are
//! asked of the body's animator when the simulation is made
//! (`BodyData::load_with`), and each drawn frame they are put where the
//! animation has them, between the simulation's last two frames like
//! everything else drawn.
//!
//! In first person the game does three things to the body, done here by
//! which entity a mesh's bone is tied to. The arms are solved onto the
//! weapon: a bone of an arm below the collarbone is the weapon rig's own,
//! which the arms' solver places. The ribcage is turned to the view and
//! squeezed along it with the weapon (`FirstPersonStrategy`): the ribcage
//! is the weapon rig's root, which is exactly that. And when the game
//! turns the ribcage it keeps the two collarbones where the body's
//! animation had them (they are on the player prefab's list of bones to
//! keep, `PlayerBones.FovSpecialTransforms`), though the squeeze, which
//! comes after, still draws them in: each collarbone is an entity of its
//! own under the rig's root, put each frame where the animated one is.
//! Whatever else hangs from the ribcage would turn with it, and so hangs
//! from the rig's root; nothing of the default clothing does (the neck
//! and head hang from the last spine bone, beside the ribcage, and are
//! the body's animation's alone).
//!
//! And the game does not draw all of it to its own player
//! (`PlayerBody.UpdatePlayerRenders`): in first person the legs are drawn
//! and the hands are, and every other part (the upper body, the head) is
//! left to cast its shadow and nothing more. The upper body's meshes are
//! on a render layer of their own that the light sees and, in first
//! person, the camera does not.
//!
//! Seen by anyone else (`docs/spec/third-person.md`) the game does none of
//! the first two: the ribcage is where the body's animation has it, the
//! collarbones on it, and the arms are solved onto the weapon from the
//! body's own shoulders. So the ribcage drawn is an entity of its own
//! under the rig's root, like the kept collarbones: left on that root for
//! the character's own eyes, and put where the animated ribcage is for
//! another's.
//!
//! Not in this: the feet set on the ground (the game runs a solver for
//! them, which moves nothing on level ground) and equipment.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use trp_data::character::dress::{Under, Worn};
use trp_data::unity::anim::path_hash;
use trp_data::unity::extract::Hierarchy;
use trp_data::unity::Assets as UnityAssets;

use crate::player::controller::Controller;
use crate::screens::inventory_view::Carried;
use crate::platform::park::{EvenParked, Park};
use crate::assets::render_data::Converted;
use crate::weapons::weapon::{FirstPersonHands, WeaponRig};
use crate::{platform::convert, GameData, View};

mod pieces;
mod loading;
mod showing;
mod head_fitting;
mod head_models;
mod spawning;
mod posing;

pub use pieces::*;
pub use showing::*;
pub use head_fitting::*;
pub use head_models::*;
pub use spawning::*;
pub use posing::*;
use pieces::{COLLARBONES, HEAD, LevelSpecs, RIBCAGE, VEST, a_collarbone, of_an_arm};
use showing::{worn_everywhere, worn_of};
use head_models::{Copies, Heads, converted_looks, hang_head_model};

/// The drawn body's skeleton.
#[derive(Resource)]
pub struct BodyRig {
    /// What the skeleton hangs from: at the feet, turned as the body is.
    root: Entity,
    /// One entity for each bone of `BodyParts::bones`, in that order.
    bones: Vec<Entity>,
    /// The collarbones: each as the animation has it, and the entity
    /// under the weapon rig's root that is kept where that one is.
    collarbones: Vec<(Entity, Entity)>,
    /// The weapon rig's root, which stands for the ribcage in first
    /// person.
    ribcage: Entity,
    /// The ribcage as the animation has it, and the entity under the
    /// weapon rig's root that the meshes' ribcage is: on that root for
    /// the character's own eyes, where the animated one is for another's.
    ribcage_animated: Option<Entity>,
    ribcage_drawn: Entity,
}

impl BodyRig {
    /// The entity of the bone whose path ends so, if the body has it.
    pub fn bone(&self, parts: &BodyParts, end: &str) -> Option<Entity> {
        parts.bones.iter().position(|path| path.ends_with(end)).map(|at| self.bones[at])
    }

    /// The entity a bone of the skeleton (by its path) is drawn on, as
    /// `spawn_body` ties the meshes to it: the ribcage as drawn, a kept
    /// collarbone, the weapon rig's own bone for an arm's (the nearest
    /// above it that the rig in hand has), or the body's own.
    pub fn drawn(&self, parts: &BodyParts, weapon: &WeaponRig, path: &str) -> Option<Entity> {
        if path.ends_with(RIBCAGE) {
            return Some(self.ribcage_drawn);
        }
        if a_collarbone(path) {
            let index = parts.bones.iter().filter(|known| a_collarbone(known)).position(|known| known == path)?;
            return self.collarbones.get(index).map(|(_, kept)| *kept);
        }
        if of_an_arm(path) {
            let mut on_rig = &path[path.find(RIBCAGE).map_or(0, |at| at + RIBCAGE.len() + 1)..];
            loop {
                if let Some(entity) = weapon.by_hash.get(&path_hash(on_rig)) {
                    return Some(*entity);
                }
                on_rig = &on_rig[..on_rig.rfind('/')?];
            }
        }
        parts.bones.iter().position(|known| known == path).map(|at| self.bones[at])
    }

    /// What the drawn body hangs from.
    pub fn root(&self) -> Entity {
        self.root
    }

    /// Each collarbone the meshes are drawn on (kept where the animated
    /// one is), in the order the body lists them.
    pub fn kept_collarbones(&self) -> impl Iterator<Item = Entity> + '_ {
        self.collarbones.iter().map(|(_, kept)| *kept)
    }

    /// The ribcage as the body's animation has it, and as it is drawn.
    pub fn ribcages(&self) -> (Option<Entity>, Entity) {
        (self.ribcage_animated, self.ribcage_drawn)
    }
}

/// Where the kept collarbones go this frame, in the rig root's frame, and
/// the drawn ribcage with them.
#[derive(Resource, Default)]
pub struct KeptCollarbones {
    collarbones: Vec<Transform>,
    ribcage: Transform,
}
