//! A character no one is at the keys of, seen from outside with a firearm in its hands.
//! After `docs/spec/body-hits.md`, "The dummy on the range", and
//! `docs/spec/third-person.md`. It holds the movement with the body's animator, the
//! weapon's motion in its third-person strategy, the firearm with its own
//! animator, the weapon's rig posed by that animator, and both arms solved
//! onto it; every frame as the player's own is run (the app's controller,
//! and the replay that sets a character seen from outside beside the
//! game's), on a pacer of its own. A host draws what it leaves
//! ([`PuppetPose`]): the body's skeleton, the rig's bones and the arms.
//! Its health, where the host gives one, is followed after each of its
//! frames as the player's is, its skills and its load put on it as the
//! player's are (body hits spec, "The dummy on the range").
//! It may hold nothing instead ([`Puppet::empty_handed`]): the game's empty
//! hands, a rig with an animator and a controller of their own
//! (`EmptyHands`), the arms solved onto that rig as onto a weapon's, as the
//! game's character has them once the thing in its hands is gone
//! (`Player.SetControllerInsteadRemovedOne`, with no weapon on the body to
//! take). It keeps its last few frames' poses, and a host
//! draws it between two of them at the moment drawn ([`Puppet::drawn`]), as
//! the app draws the player's body: rule 8's one liberty, the frames
//! themselves untouched.
//!
//! **The puppet's economy** (TRP's own,
//! an exception to rule 8 for posing; third-person
//! spec, "Posing by distance and off screen"; body-animator spec, "Layers",
//! "TRP's own"). Each frame is two halves. The first, the simulation,
//! always runs: the movement with the body's animator, health, the hands'
//! springs, the thing in hand with its animator, and what the frame leaves
//! for its pose, its [`Recipe`]: the animators' mixes, the moved bone's
//! place, the elbows' shift, the body's arms as its animation has them, the
//! head's turn, where the body and the view stand, and the [`Bound`]. The
//! second, the pose, is made from a recipe alone: at once where the host
//! has the character posed in full ([`Posing::Full`]), or only when a pose is asked for ([`Posing::Asked`]: `pose`,
//! `pose_of_frame`, `drawn`), each kept with its frame once made. Nothing
//! of the simulation goes by the second half, so its results are the same
//! either way, and a pose made late is the one made at once, to the bit.
//! The hit colliders' arms are solved on the animator's straight pose
//! (`PuppetPose::straight_arms`), the drawn arms on the pose for drawing
//! (by decision: `SMOOTH ANIMATIONS` moves no hit). A character's
//! step touches nothing but itself and reads the world through `&dyn
//! World`, so a host may step many on threads of its own; this crate starts
//! none.

use std::collections::VecDeque;
use std::sync::OnceLock;

use anyhow::{Context, Result};
use trp_data::character::player::{effector, LimbData};
use trp_data::unity::extract::Hierarchy;

use crate::base::animator::Mix;
use crate::weapons::arms::{self, Placed};
use crate::locomotion::body::ArmPose;
use crate::condition::health::Health;
use crate::base::math::{lerp_angle, quat_axis, quat_lerp, quat_mul, quat_rotate};
use crate::base::rig::{ArmNodes, SightNodes, WeaponRig};
use crate::condition::skills::{Buffs, Skill};
use crate::weapons::weapon_animator::{WeaponAnimator, WeaponMix};
use crate::{EmptyHands, Firearm, FirearmEvent, Hands, HandsInput, Input, Movement, Pacer, PointOfView, State, World};

mod pose;
mod drawing;
mod access;
mod frame;

pub use pose::*;
pub use drawing::*;
pub use frame::*;
use pose::RigHand;
use drawing::{Drawing, Held, Kept};
use frame::{nlerp};

/// The bone the procedural motion moves, and the one it hangs from.
const MOVED: &str = "Weapon_root/Weapon_root_anim";
const PARENT: &str = "Weapon_root";
const RIGHT_MARKER: &str = "Weapon_root/Weapon_root_anim/weapon/weapon_R_IK_marker";
const LEFT_MARKER: &str = "Weapon_root/Weapon_root_anim/weapon/weapon_L_IK_marker";
const VEST_MARKER: &str = "Weapon_root/Weapon_root_anim/weapon/weapon_vest_IK_marker";

/// How far a hit collider reaches past the bones its limb's reach counts
/// (`Body::reach`), and what a clip moves a bone off its length at rest:
/// the bound's margin over the skeleton (`Bound`). `ESTIMATE`, this work's:
/// the head's collider stands above the head's bone, and a calf's ends past
/// the foot's; the test that sweeps the poses (`trp-sim`'s `puppet` tests)
/// finds every placed collider of the player prefab 0.2 m inside at least.
pub const BOUND_MARGIN: f32 = 0.15;

/// The character.
pub struct Puppet {
    pub movement: Movement,
    pub hands: Hands,
    held: Held,
    /// The gear's share of the weapon's ergonomics (the armour's and the
    /// backpack's `WeaponErgonomicPenalty` over 100: `armor::
    /// ergonomics_share`), told to the weapon's motion with the load on
    /// every frame, as the player's (`carry_gear`).
    pub gear_ergonomics: f32,
    pacer: Pacer,
    /// The rig as the simulation reads it (the moved bone at rest).
    read: WeaponRig,
    parent: usize,
    marker: Option<usize>,
    vest: Option<[usize; 2]>,
    sights: Option<SightNodes>,
    drawing: Drawing,
    /// How its frames are posed, as the host last said (`posing`).
    posing: Posing,
    /// Its frames made so far: the next one's number.
    made: u64,
    /// What its last few frames left, the newest last: enough to reach
    /// back past the moment drawn (`drawn`), as many as the app keeps of
    /// the player's (`Controller::stepped`).
    frames: VecDeque<Kept>,
    /// The oldest frame the host may still ask for (`keep_from`): none
    /// older is let go while it is said.
    hold: Option<u64>,
}

/// The sight a character's weapon is aimed by, as a puppet picks it: its
/// motion's (`Hands::sight_nodes`), else the weapon's first. A host splits
/// the weapon's animator by it (`followed_rig_bones`).
pub fn sights_for(hands: &Hands, hierarchy: &Hierarchy) -> Option<SightNodes> {
    hands.sight_nodes().or_else(|| SightNodes::find(hierarchy))
}

impl Puppet {
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bound_meets_a_segment_within_its_radius() {
        let bound = Bound { ends: [[0.0, 1.0, 0.0], [0.0, 1.5, 0.0]], radius: 1.0 };
        assert!(bound.holds([0.5, 1.2, 0.0], 0.4) && !bound.holds([0.5, 1.2, 0.0], 0.6));
        assert!((bound.distance_to_axis([0.0, 3.0, 0.0]) - 1.5).abs() < 1e-6);
        // A path across in front, 0.9 m off the axis, and one 1.1 m off.
        assert!(bound.meets([-5.0, 1.2, 0.9], [5.0, 1.2, 0.9]));
        assert!(!bound.meets([-5.0, 1.2, 1.1], [5.0, 1.2, 1.1]));
        // Stopping short of it, and a path along the axis' line above it.
        assert!(!bound.meets([-5.0, 1.2, 0.5], [-1.2, 1.2, 0.5]));
        assert!(bound.meets([0.0, 3.0, 0.0], [0.0, 2.4, 0.0]) && !bound.meets([0.0, 3.0, 0.0], [0.0, 2.6, 0.0]));
        // Parallel to the axis, beside it.
        assert!(bound.meets([0.95, 0.0, 0.0], [0.95, 3.0, 0.0]) && !bound.meets([1.05, 0.0, 0.0], [1.05, 3.0, 0.0]));
        // A segment that is a point.
        assert!(bound.meets([0.3, 1.2, 0.0], [0.3, 1.2, 0.0]));
    }
}
