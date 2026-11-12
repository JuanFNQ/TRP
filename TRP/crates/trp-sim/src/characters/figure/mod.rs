//! The character as the game's own screens show it, as the inventory's OVERALL figure.
//! It holds what is in its hands, breathes and looks about, after
//! `docs/spec/inventory.md`, "The model's view", and `docs/spec/body-animator.md`,
//! "The stance of the game's screens". No engine types: a host draws the
//! pose it leaves.
//!
//! The menus' copy of the player (`menuplayer.bundle`) has the body's own
//! controller, stood by its `GUI` layer at a weight of 1 and the kind of
//! thing in hand (`PlayerModelLoader.CreateWeapon`), and its animator runs
//! on. A weapon in hand is made with its own animator, hung from the
//! ribcage at nought and played in `PATROL`; every frame after the
//! animators (`MenuPlayerPoser.LateUpdate`) its moved bone is put on the
//! body's `Weapon_root_3rd_anim` by the body's `Weapon_3rd` curve (or hung
//! by its right marker from the right palm where the body's animation keeps
//! that hand), the body's elbow goals are moved to the weapon's, each arm
//! is solved onto its marker (pulled onto the part's grip as the marker
//! nears its reference), the fingers take the weapon's hand pose and the
//! grip's, and the forearms' twist bones are relaxed. Near the end of each
//! 7.33 s loop of the idle an event of its clip (`OnRandomizeAnimation`)
//! picks what comes next: nothing, a look out, or a move of the weapon.
//! A look is the body's alone. A weapon move takes the weapon off patrol
//! and the body into its `Hands_Operation_*` states, whose clip's event
//! (`OnWeaponOperationStart`) raises the move's trigger on the weapon's
//! animator; the weapon's own marks, handed to the poser in its late
//! update, end it (`IdleStart`) and count a reload's rounds
//! (`AddAmmoInMag`, `AddAmmoInChamber`), and the body's way out
//! (`OnWeaponOperationEnd`) puts the weapon back on patrol. The weapon's
//! other marks (its sounds among them) reach no one in the game's menu:
//! nothing is heard (inventory spec, "The hold and the random looks").

use std::collections::VecDeque;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use trp_data::character::player::LimbData;
use trp_data::unity::Assets;
use trp_data::weapons::weapon::Weapon;
use trp_data::Database;

use crate::base::animator::{Animator, BoneRest, Controller};
use crate::weapons::arms::{self, Arm, Placed};
use crate::base::math::{inverse_lerp, quat_lerp, quat_mul, quat_rotate};
use crate::base::rig::WeaponRig;
use crate::base::unity_random::UnityRandom;
use crate::weapons::weapon_animator::{WeaponAnimator, WeaponAnimatorData};

mod skeleton;
mod figure_data;
mod stepping;
mod operations;
mod grip;

pub use figure_data::*;
use skeleton::{Blender, Named, Skeleton, distance, inverse, lerp3, then, within};
use figure_data::{GripChoice};
use grip::{compose};

/// The kind of thing in hand as the screens tell the body
/// (`PlayerModelLoader.CreateWeapon`: `WeaponType`, `WeaponTypeFloat`,
/// `UI_WeaponType`).
pub mod kind {
    /// Nothing, or the empty hands.
    pub const NOTHING: i32 = -1;
    /// Any weapon that is none of the three below: a rifle, a shotgun, a
    /// revolver (the game's `Revolver` is no `Pistol`).
    pub const TWO_HANDED: i32 = 0;
    pub const PISTOL: i32 = 1;
    pub const THROWN: i32 = 2;
    pub const KNIFE: i32 = 3;
}

/// The kind of an item in hand by its template: a `Pistol`, a `ThrowWeap`,
/// a `Knife` (the classes whose items carry a `KnifeComponent`), any other
/// two-handed; nothing for a template the database lacks.
pub fn kind_of(db: &Database, template: &str) -> i32 {
    let Ok(item) = db.item(template) else { return kind::NOTHING };
    if db.is_a(item, "Pistol") {
        kind::PISTOL
    } else if db.is_a(item, "ThrowWeap") {
        kind::THROWN
    } else if db.is_a(item, "Knife") {
        kind::KNIFE
    } else {
        kind::TWO_HANDED
    }
}

/// The last spine bone, which the ribcage, the neck, the weapon's bone and
/// the elbows' goals hang from.
const SPINE3: &str = "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3";
/// The layer the screens stand the character by, and its parameters.
const LAYER: &str = "GUI";
const KIND: [&str; 3] = ["WeaponType", "WeaponTypeFloat", "UI_WeaponType"];
const RANDOM_TRANSITION: &str = "UI_RandomTransition";
const RANDOM_COUNT: &str = "UI_RandomAnimationsCount";
/// The idle's clip event that picks what comes next.
pub const RANDOMIZE: &str = "OnRandomizeAnimation";
/// The body's curves the poser goes by: the weapon's hold, each hand's and
/// each elbow's share that is the body's, right then left.
const WEAPON_3RD: &str = "Weapon_3rd";
const HAND: [&str; 2] = ["Hand_Right", "Hand_Left"];
const ELBOW: [&str; 2] = ["Elbow_Right", "Elbow_Left"];
/// The hold's least share for the weapon to be put on the body's bone
/// (`num2 > 0.01f`).
const HOLD_LEAST: f32 = 0.01;
/// How far a marker may be from its reference before the hand is off the
/// part's grip (`Mathf.InverseLerp(0.1f, 0f, distance)`).
const GRIP_RELEASE: f32 = 0.1;
/// The weapon's animator's one update as it is made (`Update(3f)`).
const MAKING_UPDATE: f32 = 3.0;
/// How fast the left grip's blend and the crutch's go, a second
/// (`new GripBlender(…, 2f, 0f)`, `_crutchBlender.Speed = 2f`).
const BLEND_SPEED: f32 = 2.0;
/// A hand poser at no more than this weight is left alone (`ManualUpdate`).
const POSER_LEAST: f32 = 0.001;
/// The weapon's state its animator is asked for (`Play("PATROL", 1, 0f)`).
const PATROL: &str = "PATROL";
/// The body's flag of a weapon move under way, and the clips' events that
/// begin the move on the weapon and end it (`inv_idle_weaponcheck_in` at
/// 1.073 s, `inv_idle_weaponcheck_out` at 0.257 s).
const WEAPON_OPERATION: &str = "UI_WeaponOperation";
pub const OPERATION_START: &str = "OnWeaponOperationStart";
pub const OPERATION_END: &str = "OnWeaponOperationEnd";
/// The weapon's marks the poser acts on (`IEventsConsumer.OnIdleStart`,
/// `OnAddAmmoInMag`, `OnAddAmmoInChamber`); it does nothing with the rest.
const IDLE_START: &str = "IdleStart";
const ADD_AMMO: [&str; 2] = ["AddAmmoInMag", "AddAmmoInChamber"];
/// The rounds a reload counts down before the weapon is told it can reload
/// no more (`MenuPlayerPoser.RandomAmmoCount`: `Random.Range(2, 11)`).
const AMMO_DRAW: (i32, i32) = (2, 11);

/// What the idle's event picked (`MenuPlayerPoser.OnRandomizeAnimation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// Nothing: the idle plays on (`UI_RandomTransition` 0).
    Idle,
    /// `UI_RandomTransition` 1 to the controller's count (4): a look out
    /// (1 `LookOut`, 2 `LookOut_Subtle`, 3 `LookOut_Alert`), or 4, which
    /// only a thrown thing or a knife plays (`WeaponCheck_Melee`).
    Random(i32),
    /// A move of the weapon, by its place among the figure's moves
    /// (`Figure::weapon_moves`).
    WeaponMove(usize),
}

/// The weapon in the figure's hands, running.
#[derive(Clone)]
struct Held {
    data: FigureWeaponData,
    animator: WeaponAnimator,
    /// The rig as the animator and the hold leave it.
    rig: WeaponRig,
    /// Each hand poser's share of the grip's pose, right then left
    /// (`HandPoser.GripWeight`).
    grip_weight: [f32; 2],
    crutch: Blender,
    blender: Option<Blender>,
    /// The left grip's blend as last read (`GripBlender._blendValue`).
    blend: f32,
    /// Where the hold put the weapon's moved bone, and each hand's marker,
    /// in the figure's root axes.
    moved: Placed,
    markers: [Option<Placed>; 2],
    /// Where each hand was solved to on the last frame: its marker, or
    /// toward its grip; none where its arm was not.
    targets: [Option<Placed>; 2],
}

/// What reached the figure's poser on a frame, in the order it came: the
/// body's clip events (`OnRandomizeAnimation`, `OnWeaponOperationStart`,
/// `OnWeaponOperationEnd`, this last also when the start falls back to it),
/// then the weapon's marks that passed their conditions, by their names and
/// texts (`IdleStart`, `StartUtilityOperation`, `Sound` with its sound's
/// name, ...), as the recorder writes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FigureEvent {
    /// The weapon's mark, or the body's event.
    pub weapon: bool,
    pub name: String,
    pub text: String,
}

/// The bones the recorder writes of the game's figure, each in the
/// figure's own axes (the root's: `InverseTransformPoint`), right then
/// left in each pair: for comparing and for tests.
#[derive(Debug, Clone, Copy)]
pub struct FigureBones {
    pub head: Placed,
    pub neck: Placed,
    pub ribcage: Placed,
    pub palms: [Placed; 2],
    pub feet: [Placed; 2],
    /// The body's elbow goals, after the poser moved them.
    pub goals: [Placed; 2],
    pub weapon_3rd: Placed,
    pub markers: [Option<Placed>; 2],
    /// Where the poser put each hand's target on the last frame: its
    /// marker, or toward the part's grip; none where the arm was the
    /// body's alone (not written by the recorder).
    pub targets: [Option<Placed>; 2],
    /// The weapon's moved bone (`Weapon_root_anim`), and its prefab's root
    /// (on the ribcage at nought).
    pub weapon_root_anim: Option<Placed>,
    pub weapon: Option<Placed>,
}

/// The figure, running.
pub struct Figure {
    data: FigureData,
    body: Animator,
    kind: i32,
    held: Option<Held>,
    moves: Vec<&'static str>,
    random_count: i32,
    /// The draws for the picks, as the engine makes them (`Random.Range`
    /// on Unity's generator); the game's own seed is not known.
    dice: UnityRandom,
    /// Picks given by the host for the next events, before any draw.
    steered: VecDeque<Pick>,
    /// Every pick made, with the figure's time.
    picks: Vec<(f32, Pick)>,
    /// The weapon move last picked, by its place in `moves`
    /// (`_weaponAnimationType`: never put back after a move).
    move_picked: Option<usize>,
    /// The first `IdleStart` after a move is picked only clears this (the
    /// weapon reaching its idle off patrol), the next ends the move.
    patrol_delayed: bool,
    /// The rounds a reload still counts before the weapon is told it can
    /// reload no more (`_ammoToLoad`; drawn at the first, -1 till then).
    ammo_to_load: i32,
    /// What the item lets the weapon do (`_canReload`), told it again as
    /// each move ends.
    can_reload: bool,
    /// What reached the poser on the last frame.
    heard: Vec<FigureEvent>,
    age: f32,
    /// Every bone in its parent, as the last frame left it.
    pose: Vec<Placed>,
}

impl Figure {
}
