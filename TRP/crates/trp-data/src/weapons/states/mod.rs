//! The weapon animator's states: which clip each one plays and how fast,
//! and the timed events the game's weapon logic waits for.
//!
//! The states come from the animator controller. The events are the
//! game's own: each state carries a script naming a list in a table that
//! ships beside the controller.

use std::collections::HashMap;

use anyhow::{anyhow, Result};

use crate::unity::{Assets, ObjRef, Value};
use crate::weapons::weapon::Weapon;

mod anim_state;
mod table_read;

pub use anim_state::*;
pub use table_read::*;

/// A condition on an animator parameter that an event needs to fire.
#[derive(Debug, Clone)]
pub struct EventCondition {
    pub parameter: String,
    /// 0 a whole number, 1 a float, 2 a flag, as the game numbers them
    /// (`EEventConditionParamTypes`).
    pub kind: i32,
    /// 0 equal, 1 not equal, 2 greater, 3 less, 4 at least, 5 at most
    /// (`EEventConditionModes`).
    pub mode: i32,
    pub float_value: f32,
    pub int_value: i32,
    pub bool_value: bool,
}

#[derive(Debug, Clone)]
pub struct StateEvent {
    pub name: String,
    /// When in the state it fires, as a fraction of the state's length.
    pub time: f32,
    pub int_param: i32,
    pub string_param: String,
    pub enabled: bool,
    pub conditions: Vec<EventCondition>,
}

/// One test of an animator parameter on a transition.
#[derive(Debug, Clone)]
pub struct Condition {
    pub parameter: String,
    /// Unity's numbering: 1 if true, 2 if false, 3 greater, 4 less,
    /// 6 equals, 7 not equal.
    pub mode: i32,
    pub threshold: f32,
}

/// What kind of value an animator parameter holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterKind {
    Float,
    Int,
    Bool,
    /// A boolean that the transition using it clears.
    Trigger,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub kind: ParameterKind,
    /// The value it starts with; 0 or 1 for a boolean or a trigger.
    pub default: f32,
}

/// One of the choices at the entry or exit of a sub-state machine.
#[derive(Debug, Clone)]
pub struct SelectorChoice {
    /// The state it leads to, if it leads to a plain state.
    pub to: Option<String>,
    /// Or the further entry or exit point it leads to, by its place in
    /// the machine's list.
    pub to_selector: Option<usize>,
    pub conditions: Vec<Condition>,
}

/// The entry or exit point of a sub-state machine: choices tried in order.
#[derive(Debug, Clone)]
pub struct Selector {
    pub entry: bool,
    pub choices: Vec<SelectorChoice>,
}

/// The game's script on a body state that names the movement state the
/// player is in while the animator is in that state.
#[derive(Debug, Clone)]
pub struct MovementBehaviour {
    /// The game's own number for the movement state.
    pub state: i32,
    pub default_state: bool,
    pub disable_root_motion: bool,
    pub rotation_speed_clamp: f32,
    pub sensitivity: f32,
    /// Whether the player may look at a thing to work it while in this
    /// state (`CanInteract`; interactions spec, "What is looked at").
    pub can_interact: bool,
    pub animation_authority: f32,
    pub authority_speed: f32,
    /// Which part of a movement the state is, for movements that have a
    /// way in, a hold and a way out (the game's `EStateType`: 0 none or
    /// the hold, 1 in, 2 out).
    pub kind: i32,
    /// The side or direction the state is for (the game's
    /// `EMovementDirection`: 0 none, 1 right, 5 left).
    pub direction: i32,
    /// The length the game has stored for the state, in seconds.
    pub length: f32,
    /// The state gets a movement-state object of its own, and does not
    /// share the one its movement state's other animator states use.
    pub unique: bool,
    /// What the state does to the first-person weapon and camera while the
    /// animator is in it, if anything.
    pub first_person_pose: Option<FirstPersonPose>,
    /// The curve the state's script reads by how far through the state the
    /// animator is (`SmoothLerpAnimCurve`), where it has keys: blind fire's
    /// float follows it (blind-fire spec, "The body").
    pub curve: Option<crate::formats::curve::Curve>,
}

/// An offset the weapon's procedural motion blends in while a body state
/// is the one entered last: added to the rest of the weapon's position and
/// rotation springs and of the camera's.
#[derive(Debug, Clone)]
pub struct FirstPersonPose {
    pub name: String,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub camera_rotation: [f32; 3],
    pub camera_position: [f32; 3],
    /// How much of it applies, against the time it has been coming in.
    pub blend: crate::formats::curve::Curve,
}
