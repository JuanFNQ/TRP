//! A recording's health set beside the simulation's, for the health replay and its test.
//! After `docs/spec/health.md`, "Compared with what", for `bin/health_replay` and
//! the test that keeps it: the character as the recording's first frame has it, the step's own
//! hurt done as the recorder's step did it (`hosts/spt-oracle/Courses.cs`,
//! recorder 0.8.0's course 1), the recorded character's skills, an item put
//! to use where the recording's comes in hand (food and drink by the share
//! the game used), jumps and sprints as the recording's movement made them,
//! and the health stepped by the player's whole-millisecond clock, as the
//! game's is. With the install's assets, the item's own hands too
//! (`MedsHands` on the item's rig), their animator's states set beside the
//! recording's frame by frame.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde_json::Value;
use trp_data::unity::anim::path_hash;
use trp_data::unity::Assets;
use trp_data::Database;

use crate::condition::health::{Chance, DamageKind, Health, HealthSettings, HealthSkills, Kind, Medicine, Part, Stage};
use crate::held::meds_hands::{MedsEvent, MedsHands};
use crate::weapons::weapon_animator::{WeaponAnimator, WeaponAnimatorData};

mod log_reading;
mod in_hand;
mod replay;

pub use log_reading::*;
pub use replay::*;
use log_reading::{effects_of, prepare};
use in_hand::{loop_of, take_in_hand};

/// A replay's outcome.
#[derive(Default)]
pub struct Replay {
    pub scenario: String,
    /// What was done, line by line (items used, jumps, effects taken up).
    pub notes: Vec<String>,
    /// The recording's and the simulation's effects over time.
    pub recorded: Log,
    pub simulated: Log,
    /// Effects at work in the first frame that the step did not make, as
    /// the recording had them: taken up as started, their timers unknown.
    pub taken_up: Vec<(String, String)>,
    /// The largest gap of each part (current, and maximum where `true`
    /// marks a maximum the game changed, a mended part's random share), of
    /// energy and of hydration, with when.
    pub worst: [(f32, f32); 7],
    pub worst_maximum: [(f32, f32, bool); 7],
    pub worst_energy: (f32, f32),
    pub worst_hydration: (f32, f32),
    /// Every so often (when asked), the game's line and ours.
    pub samples: Vec<String>,
    /// The parts at the end: the game's, ours.
    pub end: [[[f32; 2]; 7]; 2],
    /// Each use's hands, where the assets were given.
    pub hands: Vec<HandsUse>,
    /// Barbed wire's cuts as the game made them (the barbed wire's step):
    /// when, on which part, and the tally done (the part's drop over its
    /// `EnvironmentDamageMultiplier`).
    pub cuts: Vec<(f32, Part, f32)>,
}
