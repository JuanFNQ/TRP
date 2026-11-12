//! The things of the world the hands work: a door, a container, a car's trunk.
//! After the interactions spec, "A thing's own part": `WorldInteractiveObject`
//! and its kinds `Door`, `LootableContainer` and `Trunk`.
//!
//! A thing is shut or open, and "being worked" from the key until it has
//! done what it was told. Told to open or to shut, it waits (a door half a
//! second of its own, a container or a trunk the install's
//! `DelayToOpenContainer`, 0.55 s), then turns from the angle it stands at
//! to its new one along the settings object's `DoorCurve`: at the curve's
//! own pace for a door, so 2.5 s, and half as fast again for a container or
//! a trunk, 1.67 s. A drawer is moved out and back in by the same curve.
//! Each frame the thing stands where the curve has it at the time run so
//! far, and the time is then moved on by the frame; the frame that finds
//! the curve's end passed puts it at its new angle and in its new state.
//!
//! Which gesture the hand makes for it is the thing's to say: a door's by
//! where the character stands to it and which way it will swing, a
//! container's or a trunk's the two it names (or the default two).
//!
//! Set beside fourteen recordings made by hand: four doors'
//! swings within 0.07 degrees on every frame, six lids' and a trunk's
//! within 0.04, and every new state on the recorded frame.

use trp_data::formats::curve::Curve;
use trp_data::setup::hard_settings::HardSettings;

use crate::held::interactions::Interaction;

mod actions;
mod reading;
mod breaching;
mod swinging;

pub use actions::*;
pub use swinging::*;
use actions::{CONTAINER_PACE};
use swinging::{cross, dot, flat_unit, project, sub};

/// How far off a door's leaf, and how far along it from the hinge, a kick
/// is made from, in metres (`Door.GetBreakInPosition`).
const BREAK_IN_OFF: f32 = 1.3;
const BREAK_IN_ALONG: f32 = 0.5;

/// A kick as it begins: where it is made from, what it is aimed at,
/// whether the door puts the character at that place first, and whether
/// it will throw the door open.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Breach {
    pub position: [f32; 3],
    pub view_target: [f32; 3],
    pub snaps: bool,
    pub opens: bool,
}

/// How long a kick takes to throw a door open, in seconds, and the slope of
/// its curve at both ends (`Door.KickCurve`, as the class has it: from
/// nothing to all of the turn).
const KICK_LASTS: f32 = 0.5;
const KICK_SLOPE: f32 = 1.0;
/// How long after a kick that fails the door is what it was again, in
/// seconds (`Door.FailBreachRoutine`).
pub const FAILED_KICK_SHOWS: f32 = 0.3;

/// The two figures every thing goes by, from the install.
#[derive(Debug, Clone)]
pub struct ThingSettings {
    /// The way through a swing, its share against seconds (`DoorCurve`).
    pub curve: Curve,
    /// How long a container or a trunk waits after it is told, in seconds
    /// (`DelayToOpenContainer`, 0.55 in the install).
    pub answers_after: f32,
}

impl ThingSettings {
    pub fn new(hard: &HardSettings) -> anyhow::Result<Self> {
        Ok(Self { curve: hard.curve("DoorCurve")?.clone(), answers_after: hard.float("DelayToOpenContainer")? })
    }

    /// How long the curve runs, in seconds (`GetDuration`: its last key's time).
    fn duration(&self) -> f32 {
        self.curve.keys.last().map_or(0.0, |key| key.time)
    }
}

/// How a door stands and turns, in the simulation's axes (the game's: x
/// right, y up, z ahead).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hinge {
    /// A point on the line it turns about (the door's own place).
    pub at: [f32; 3],
    /// The line it turns about, a unit vector. An angle turns the door as
    /// the game's `Quaternion.AngleAxis` does about it.
    pub axis: [f32; 3],
    /// Which way the door points from its hinge at an angle of nothing
    /// (its `DoorForward` axis), a unit vector square to `axis`.
    pub forward: [f32; 3],
}

/// One run of a thing's routine.
#[derive(Debug, Clone, Copy)]
struct Swing {
    /// A door's own wait before it turns: how long it has waited, how long
    /// it waits, and whether it began this frame (that frame does not count).
    wait: Option<(f32, f32, bool)>,
    /// The angle it began from, the turn to make, and whether it opens (a
    /// drawer's way out goes by the latter).
    from: f32,
    turn: f32,
    opening: bool,
    /// Seconds of the curve run so far, and how fast they run.
    progress: f32,
    pace: f32,
    /// What the thing is when the swing is over.
    to: ThingState,
    /// Thrown open by a kick: along the kick's own curve, not the
    /// settings'.
    kicked: bool,
    /// Whether its first frame has run, which sounds its opening, and
    /// whether a shutting has sounded its shut clip (`_shutPlayed`).
    begun: bool,
    shut_played: bool,
}

/// A container's or a trunk's wait before it does what it was told.
#[derive(Debug, Clone, Copy)]
struct Timer {
    waited: f32,
    fresh: bool,
    to: ThingState,
}

/// A door, a container or a trunk.
#[derive(Debug, Clone)]
pub struct Thing {
    pub kind: Kind,
    state: ThingState,
    /// What it was before it was being worked (`FallbackState`).
    fallback: ThingState,
    angle: f32,
    /// Its angles shut and open (`CloseAngle`, `OpenAngle`), in degrees.
    pub shut_angle: f32,
    pub open_angle: f32,
    /// A drawer's share of the way out: 0 in, 1 out.
    out: f32,
    /// The gestures a container or a trunk names for its opening and its
    /// shutting (`_openInteraction`, `_closeInteraction`); none for the
    /// game's default two.
    open_gesture: Option<Interaction>,
    close_gesture: Option<Interaction>,
    /// A door's hinge.
    pub hinge: Option<Hinge>,
    /// Whether a kick can throw it open, and whether the menu offers one
    /// (`CanBeBreached`, `CanInteractWithBreach`); whether it can be worked
    /// at all (`Operatable`); and whether a kick has broken it (`IsBroken`).
    pub can_be_breached: bool,
    pub offers_breach: bool,
    pub operatable: bool,
    broken: bool,
    /// The key that unlocks it, by its item's key id (`KeyId`).
    pub key: Option<String>,
    /// The point of it a kick is aimed at, from the hinge along the leaf
    /// and up, in metres (`viewTarget1`, in the door's own turning frame).
    pub view_target: [f32; 2],
    /// Whether it puts a character that kicks it at a place of its own
    /// first, shut and locked (`Snap`, a set of the door's states: each
    /// door's own).
    pub snaps_shut: bool,
    pub snaps_locked: bool,
    /// The last order taken was a kick (`_interaction.ResultState` is
    /// `Breaching`).
    breaching: bool,
    /// A kick that failed is waiting to put it back as it was.
    failed_kick: Option<(f32, bool)>,
    /// What the last order taken was for (`_interaction.ResultState`),
    /// and how far the game's shared record of a swing has run against
    /// how long it is (`ProgressTime`, `Duration`): a container's routine
    /// keeps its own count and never moves it on.
    result: Option<ThingState>,
    record: [f32; 2],
    /// The last swing of the game's own routine that ran out (a door's,
    /// a trunk's): what it was for and when it ended, on this thing's clock.
    previous: Option<(ThingState, f32)>,
    clock: f32,
    timers: Vec<Timer>,
    swings: Vec<Swing>,
    /// Seconds of the curve before its end at which a shutting sounds its
    /// shut clip (`ShutShift`, the prefab's: 0 unless it says).
    pub shut_shift: f32,
    /// What it has sounded and the host has not yet taken.
    sounds: Vec<ThingSound>,
}

impl Thing {
}

#[cfg(test)]
mod tests;
