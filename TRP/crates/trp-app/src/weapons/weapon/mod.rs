//! Spawning an assembled weapon as an entity tree.

use std::collections::{HashMap, VecDeque};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::camera::visibility::NoFrustumCulling;
use trp_data::unity::anim::path_hash;

use crate::player::animation::{ClipLibrary, ClipPlayer};
use trp_data::unity::extract::Hierarchy;
use crate::assets::render_data::{unskin_all, Converted, PreparedLooks};
use crate::{platform::convert, GameData, Options};

mod rig_frame;
mod hands_rig;
mod rig_builder;
mod preparing;
mod nodes;
mod weapon_spawn;
mod hands_found;

pub use rig_frame::*;
pub use hands_rig::*;
pub use rig_builder::*;
pub use preparing::*;
pub use nodes::*;
pub use weapon_spawn::*;
use rig_frame::{launcher_of, launcher_ports_of, muzzle_of, shell_port_of, shell_ports_of};
use rig_builder::{FREE_AFTER, MOST_UNCARRIED, OwnOfRig, Seed, Worker};
use preparing::{Measured, RigKind, kind_of, rig_of};
use nodes::{drawn_renderer, glows_of, skin_bones};
use hands_found::{clips_of, empty_hands_of, hands_of, item_hands_of, plain_hands_of};

/// The player. Its origin is at the feet and it turns with the look yaw.
#[derive(Component)]
pub struct Player;

/// The frame the first-person rig hangs in: at the body's ribcage, turned
/// to where the player looks. It carries the look pitch; the weapon rig
/// hangs from it unmoved and the camera sits in it beside the weapon.
#[derive(Component)]
pub struct ViewPivot;

/// A mesh of the hands as the player sees them: the game draws it in first
/// person only (`body_view::show_upper_body`).
#[derive(Component)]
pub struct FirstPersonHands;

/// Where the character's own eyes are: the first-person camera's place in
/// the view's frame, kept there whatever camera the picture is drawn from.
/// What the character looks at goes by it.
#[derive(Component)]
pub struct Eye;

/// The render layer of the hands' rigs' meshes (the weapon in hand, the
/// empty hands, a dummy's weapon): seen by the cameras and the sun as
/// layer 0's things are, and left out of a tactical device's lights'
/// shadows, as the game keeps one's own weapon out of its devices' shadow
/// maps (`GunShadowDisabler`; tactical-devices spec, "The look").
///
/// A layer no other camera draws: the character model's views draw layer
/// 22 (`figure_view::LAYER`; with `OVERALL FIGURE` the OVERALL tab's own
/// figure 18, `figure_view::OVERALL_LAYER`), and the range's sun is on this one, so on 22
/// (as it was from 0.120.0 to 0.123.0) the sun lit the OVERALL tab's model
/// at that camera's exposure and it came out white. The others: 0 the
/// world, 2 nothing, 20 and 21 the optic's, 23 the icons', 24 the edit
/// build screen's, 25 to 28 the inspect window's, 29 and 30 the models'
/// lights.
pub const HANDS_LAYER: usize = 19;

/// The layers that see the hands' rigs' meshes besides layer 0's.
pub fn seeing_hands() -> bevy::camera::visibility::RenderLayers {
    bevy::camera::visibility::RenderLayers::from_layers(&[0, HANDS_LAYER])
}

/// Entities of the spawned weapon.
#[derive(Resource)]
pub struct WeaponRig {
    /// Nodes by the hash of their path from the animated root, which is
    /// how animation tracks name them.
    pub by_hash: HashMap<u32, Entity>,
    /// The animated root, which clips do not move.
    pub root: Entity,
    /// The view pivot the root hangs from. The camera hangs from it too:
    /// it sits in the root's frame but is not squeezed with it.
    pub pivot: Entity,
    /// The bone the weapon hangs from, which the weapon's clips place.
    pub weapon_root: Option<Entity>,
    /// Its child, which the procedural motion moves.
    pub weapon_root_anim: Option<Entity>,
    /// The two arms, each reaching for its hold on the weapon.
    pub arms: Vec<crate::player::arms::ArmRig>,
    /// How the game's solver for each arm is set up, right then left, if
    /// the player prefab's could be read and is one this follows.
    pub limbs: Option<[trp_data::character::player::LimbData; 2]>,
    /// How many inventory cells wide the body's animator is told the
    /// thing in hand is.
    pub width: f32,
    /// And what kind of thing it is (`trp_sim::body::hands_kind`).
    pub kind: f32,
    /// Every bone of the rig in the order of its prefab's hierarchy, which
    /// is the order its animator poses them in.
    pub bones: Vec<Entity>,
    /// Its shell port and how it throws a spent case, where it has them
    /// (`cases`): of a weapon of several chambers, its first port's.
    pub shell_port: Option<ShellPort>,
    /// Its shell ports by its chambers' numbers (`Firearms.
    /// _shellPortsTransforms`): a weapon of several chambers' every one,
    /// another weapon's one.
    pub shell_ports: Vec<Entity>,
    /// Its muzzle flash, where it has one (`muzzle_view`).
    pub muzzle: Option<MuzzleParts>,
    /// Its fireport, which a shot leaves from (`bullets`).
    pub fireport: Option<Entity>,
    /// Its underbarrel launcher's own fireport, which its grenade leaves
    /// from (`bullets`), and its own muzzle flash (`muzzle_view`), where it
    /// has them.
    pub launcher_fireport: Option<Entity>,
    pub launcher_muzzle: Option<MuzzleParts>,
    /// Its underbarrel launcher's own chamber point, where its round's model
    /// is drawn (`magazine_rounds`), and its own shell port, where its spent
    /// case is kept and thrown from (`cases`), where it has them.
    pub launcher_chamber: Option<Entity>,
    pub launcher_shell_port: Option<ShellPort>,
    /// Its tactical devices, in the order the game walks the build's items
    /// (`trp_data::tactical`), their objects by the rig's `bones`
    /// (`tactical_view`).
    pub devices: std::sync::Arc<Vec<trp_data::weapons::tactical::Device>>,
}
