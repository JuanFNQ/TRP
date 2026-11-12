//! The player's body: its animator, run for what the movement and the
//! first-person view take from it. See `docs/spec/body-animator.md`.

use std::sync::Arc;

use anyhow::Result;
use trp_data::formats::curve::Curve;
use trp_data::weapons::states::{AnimState, FirstPersonPose};
use trp_data::unity::Assets;

use crate::base::animator::{Animator, BoneRest, Controller, Event, Mix};
use crate::weapons::hands::{Carried, Held};
use crate::base::math::{quat_mul, quat_rotate};
use crate::PointOfView;

mod arms;
mod body_data;
mod inputs;
mod update;
mod drawn;

pub use arms::*;
pub use body_data::*;
use arms::{ARMS, BEND_GOALS, BEND_GOAL_NAMES, BONES, CAMERA, CAMERA_CONTAINER, CAMERA_CONTAINER_NAME, DRAWN_FROM, LEFT_FINGERS, RIBCAGE, SPINE, TO_RIBCAGE, TO_RIGHT_PALM, WEAPON, finger_paths, limbs_reach, wanted};
use body_data::{BACKPACK_DROP, BIPOD_STATES, HEAD_TURN_LEAST, Ids, UTILITY_FADE, UTILITY_LAYER, UTILITY_RANGE, WEAPON_LAYER};

/// The game's numbers for the movement states the body's states name.
pub mod movement_state {
    pub const IDLE: i32 = 1;
    pub const PRONE_IDLE: i32 = 2;
    pub const PRONE_MOVE: i32 = 3;
    pub const RUN: i32 = 4;
    pub const SPRINT: i32 = 5;
    pub const JUMP: i32 = 6;
    pub const TRANSITION: i32 = 8;
    pub const BREACH_DOOR: i32 = 9;
    pub const SIDESTEP: i32 = 15;
    pub const APPROACH: i32 = 17;
    pub const PRONE_TO_STAND: i32 = 18;
    pub const TO_PRONE: i32 = 19;
    pub const ROLL: i32 = 22;
    pub const JUMP_LANDING: i32 = 23;
    pub const CLIMB_OVER: i32 = 24;
    pub const CLIMB_UP: i32 = 25;
    pub const VAULTING_FALL_DOWN: i32 = 26;
    pub const VAULTING_LANDING: i32 = 27;
    pub const BLIND_FIRE: i32 = 28;
}

/// The kind of thing in the hands, as the body's animator is told it: the
/// game's own numbers (`PlayerAnimator.EWeaponAnimationType`; body-animator
/// spec, "The kind of thing in hand").
pub mod hands_kind {
    pub const RIFLE: f32 = 0.0;
    pub const PISTOL: f32 = 1.0;
    pub const THROWN: f32 = 2.0;
    pub const KNIFE: f32 = 3.0;
    pub const EMPTY_HANDS: f32 = 4.0;
    pub const LAUNCHER: f32 = 5.0;
    /// The share of the way to the new kind that the animator's figure
    /// goes in a second of frames (`PlayerAnimatorSetWeaponIdSmooth`), and
    /// how near it has to be to be put there.
    pub const RATE: f32 = 7.0;
    pub const NEAR: f32 = 0.01;
}

/// Which part of a movement an animator state is, for movements with a way
/// in, a hold and a way out: the game's own numbers.
pub mod part {
    pub const HOLD: i32 = 0;
    pub const IN: i32 = 1;
    pub const OUT: i32 = 2;
}

/// A state of the base layer that names a movement state, as the animator
/// enters it: the movement state by the game's number, which part of the
/// movement the state is (`part`), the side it is for (1 right, -1
/// left, 0 neither), what it does to the view's turning (a factor on
/// each frame's turn and the most degrees a frame may turn), and whether
/// a thing may be looked at to be worked in it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entered {
    pub state: i32,
    pub part: i32,
    pub side: f32,
    pub turn_factor: f32,
    pub turn_limit: f32,
    pub can_interact: bool,
}

/// What the movement tells the body's animator each step.
#[derive(Debug, Clone, Copy, Default)]
pub struct BodyInput {
    /// The smoothed speed setting, 0 to 1.
    pub speed: f32,
    pub direction: [f32; 2],
    /// The direction as one number: left 0, front 1, right 2, back 3.
    pub discrete_direction: f32,
    /// The smoothed stance, 0 crouched to 1 standing.
    pub level: f32,
    pub tilt: f32,
    /// The player is asking to move.
    pub inert: bool,
    pub sprint: bool,
    pub sprint_speed: f32,
    /// The load's figure the sprint's stop blends its two clips by
    /// (`SprintInertia`; movement spec, "The sprint's stop").
    pub sprint_inertia: f32,
    pub jumping: bool,
    pub landing: bool,
    pub falling: bool,
    pub grounded: bool,
    /// The character is lying down or going to.
    pub prone: bool,
    /// How fast the clips that go down and get up play; 1 for a rested,
    /// unburdened character.
    pub transition_speed: f32,
    /// The view's pitch, degrees, positive downward.
    pub aim_angle: f32,
    /// The angle from the body to the view.
    pub delta_rotation: f32,
    /// The sights are up: an additive layer leans the body into them.
    pub aiming: bool,
    /// A reload is in hand. Lying down, the animator holds the body still
    /// for it; upright no state answers.
    pub reloading: bool,
    /// The hands are busy with the weapon, which in a sprint brings it up
    /// into view.
    pub hands_busy: bool,
    /// The step aside asked for: 1 right, -1 left, 0 none.
    pub step: i32,
    /// How far the body is stepped aside, -1 left to 1 right, which picks
    /// the clips that bend it with the view.
    pub sidestep: f32,
    /// How fast the step aside and the step back play.
    pub sideback_speed: f32,
    /// A vault or a climb has been chosen, and one is under way.
    pub do_vault: bool,
    pub do_climb: bool,
    pub vaulting: bool,
    /// The obstacle as last given: its height and length, the drop behind
    /// it, and the forward speed it was measured at.
    pub vault_figures: [f32; 4],
    /// The pace of a vault's and a climb's clips.
    pub vaulting_speed: f32,
    /// The gesture the interactions' layer is to play, by its number, or
    /// none (`IsInteraction`, `InteractionIndex`).
    pub interaction: Option<u8>,
    /// The inventory is open and the hands have gone down for it, and
    /// whether they are meanwhile doing something else (`Inventory`,
    /// `InventoryOperation`): the layer's pose for an open inventory.
    pub inventory: bool,
    pub inventory_operation: bool,
    /// A door is being kicked, whether the kick will open it, and whether
    /// the character stands where it is to (`Kick`, `KickSucceed`,
    /// `Approached`).
    pub kick: bool,
    pub kick_succeeds: bool,
    pub approached: bool,
    /// Blind fire as the animator is told it: the side asked, 1 over the
    /// top, -1 to the right, 0 none (`BlindFire`), and the float its state
    /// works out each frame (`BlindFireFloat`; blind-fire spec, "The body").
    pub blind_fire: f32,
    pub blind_fire_float: f32,
    /// Mounted lying down (`ProneBipodsOn`, `PlayerAnimator.
    /// SetProneBipodMount`; mounting spec, "The bipod"): the aiming layer's
    /// prone state gives way to `Prone_Bipods`.
    pub prone_bipods: bool,
    /// Mounted at all, and lying down: a mount hands the first-person weapon
    /// to the body's weapon bone not at all, and upright holds the shoulders
    /// on the weapon (`Player`'s IK pass: `ThirdPersonWeaponRootAuthority`
    /// nought and `SetShoulders(1, 1)` in `IsInMountedState`; mounting spec,
    /// "Mounted").
    pub mounted: bool,
    pub lying: bool,
    /// The left shoulder as its controller tells the animator (`LeftStance`,
    /// `ForceLeftStanceInOneFrame`; left-shoulder spec): the `LS` layer
    /// plays the stance in, from a sprint at once, and out.
    pub left_stance: bool,
    pub force_left_stance: bool,
}

/// The layer of the body's animator that plays the hands' gestures on the
/// world, and its resting state.
const INTERACTIONS_LAYER: &str = "InteractionsLayer";
const INTERACTIONS_REST: &str = "InteractionsLayer.Idle";

#[derive(Clone)]
pub struct Body {
    /// The interactions' layer and its machine, where the controller has
    /// them, and the gestures whose states the last update left, by
    /// number.
    interactions: Option<(usize, usize)>,
    finished: Vec<u8>,
    /// The mark of a backpack's drop: when in its state it comes, whether
    /// the state playing has passed it, and whether that is still to be
    /// told.
    backpack_drop: Option<f32>,
    backpack_passed: bool,
    backpack_dropped: bool,
    /// The bipod's marks, the same way.
    bipod_marks: [Option<f32>; 4],
    bipod_passed: bool,
    bipod_toggled: bool,
    /// The marks of the moves over an obstacle: when in its state each
    /// comes, which the state playing has passed, and which have passed
    /// since they were last asked for (each the move and the mark).
    vault_marks: [[Option<f32>; 4]; 3],
    vault_passed: [[bool; 4]; 3],
    vault_due: Vec<(usize, usize)>,
    /// The crawl's sound marks the clips have passed and not yet told
    /// (`PlaySoundBank("Prone")`, `docs/spec/footsteps.md`).
    crawl_marks: usize,
    animator: Animator,
    ids: Ids,
    faded_layer: usize,
    sidestep_length: f32,
    pose_changes: Vec<Option<FirstPersonPose>>,
    base_machine: usize,
    /// The movement states named by the base layer's states that the last
    /// update left.
    left: Vec<i32>,
    /// How many further bones are posed for a host that draws the body,
    /// and which of them is the root.
    drawn: usize,
    drawn_root: Option<usize>,
    /// The kind of thing in the hands (`hands_kind`), and the figure the
    /// animator's trees have for it, which follows it over some frames.
    kind: f32,
    kind_now: f32,
    /// Whose eyes the character is seen with, and for one seen from
    /// outside the weight of the layer that reaches for the vest.
    point_of_view: PointOfView,
    utility_weight: f32,
    /// Mounted, and whether lying down, as the last update was told.
    mounted: [bool; 2],
    /// The neck and the head among the bones drawn, the share of the free
    /// look's turn each takes, and that turn as last told.
    head_bones: [Option<usize>; 2],
    head_shares: [f32; 2],
    head_turn: [f32; 3],
    blind_curves: [[Option<Curve>; 2]; 2],
    reach: [f32; 3],
}

impl Body {
}
