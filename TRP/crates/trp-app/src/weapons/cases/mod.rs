//! Spent cases.
//! After the firing spec, "Spent cases"; the options' `SPENT CASES`. At a shot of a weapon of one
//! chamber its round's own model goes to the weapon's shell port as a case (its `shell` objects on,
//! its `full` off) and rides there; at the weapon's `ShellEject` mark it is thrown from where the
//! port has carried it, with the weapon's extraction's force and turn and the player's velocity,
//! flies as the game's `BouncingObject` does (`trp_sim::shells`) against the range (`Range::cast`),
//! turning, and is gone a second after it was thrown. At a bounce its sound plays
//! (`PlayShellSound`): the bank of its calibre's class and the surface met, from a quarter metre
//! over it, heard at the camera as the steps are (`step_sound`), through the mixer's `CommonSounds`
//! (0 dB with the groups above it, as read: nothing added). A cylinder's or barrels' cases stay at
//! their ports, one a chamber, until a reload throws them (more-weapons spec, "The rounds and cases
//! seen in chambers of their own"). An underbarrel launcher's case (the M203's) stays at its own
//! port from the shot until a `ShellEject` of its reload or a look throws it, with its own throw
//! and a third of the player's velocity, or a round loaded from the inventory takes it away
//! (underbarrel-launcher spec, "Reloading and unloading").

use std::sync::atomic::AtomicU32;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

use bevy::math::Affine3A;
use bevy::prelude::*;
use bevy::transform::helper::TransformHelper;
use trp_data::audio::sound::ShellSounds;
use trp_sim::weapons::recoil::Rng;
use trp_sim::combat::shells::{Flight, ShellSettings};
use trp_sim::sound::steps::{choose, Shuffle};

use crate::player::controller::Controller;
use crate::platform::convert;
use crate::screens::inventory_view::{Carried, HandsHeld};
use crate::world::loose::Loose;
use crate::world::range::Range;
use crate::audio::ui_sound::{Levels, Mixer, Samples, Voice};
use crate::weapons::weapon::WeaponRig;
use crate::MainCamera;

mod case_sounds;
mod flying;

pub use case_sounds::*;
pub use flying::*;
use case_sounds::{draw, launch, mul, play_bounce, take_sounds, unsqueezed};
use flying::{spawn_case};

/// What a spent case is to do, as the weapon's events say.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CaseCall {
    /// A shot: the round to the shell port (`MoveAmmoFromChamberToShellPort`).
    Shot,
    /// The `ShellEject` mark: the case thrown (`StartSpawnShell`).
    Eject,
    /// The case at a chamber of its own's port thrown (`StartSpawnShell`,
    /// `StartSpawnAllShells`), with that share of the player's velocity,
    /// or with none, at a metre a second straight down.
    Thrown { chamber: usize, share: Option<f32> },
    /// The case at an underbarrel launcher's own port thrown, with that
    /// share of the player's velocity (`UnderbarrelContainer.StartSpawnShell`).
    LauncherThrown(f32),
}

/// How long a thrown case lasts (`StartAutoDestroyCountDown`'s one second).
const LASTS: f32 = 1.0;
/// The share of the player's velocity a case takes (`SpawnShell`'s call).
const PLAYER_SHARE: f32 = 0.66;
/// How far over the case its sound plays (`Vector3.up / 4`).
const SOUND_OVER: f32 = 0.25;
/// Where the source's falloff begins, as the steps' (`step_sound`).
const FULL_WITHIN: f32 = 1.0;

/// The cases: the figures they fly by (none with `SPENT CASES` off), the
/// one at the shell port, those in flight, the draws, and their sounds
/// once read.
#[derive(Resource)]
pub struct Cases {
    settings: Option<ShellSettings>,
    at_port: Option<Entity>,
    /// A cylinder's or barrels' cases at their ports, by the chambers'
    /// numbers, each with its round's template; and the rig they are on.
    in_ports: Vec<Option<(Entity, String)>>,
    ports_rig: Option<Entity>,
    /// An underbarrel launcher's case at its own port, with its round's
    /// template, and the kind of the round last seen in its chamber, which
    /// its case takes at the shot.
    launcher_case: Option<(Entity, String)>,
    launcher_kind: Option<String>,
    flying: Vec<Flying>,
    seed: u32,
    sounds: Option<CaseSounds>,
    coming: Option<Mutex<Receiver<ShellSounds>>>,
}

/// The cases' banks with their clips ready to mix, each bank's shuffle,
/// and the draws for the clips, volume and pitch.
struct CaseSounds {
    read: ShellSounds,
    clips: Vec<Option<Samples>>,
    shuffles: Vec<Shuffle>,
    rng: Rng,
}

/// A case in flight: its drawn model, its flight, its turn (the game's
/// axes), how long since it was thrown, and its calibre's class.
struct Flying {
    entity: Entity,
    flight: Flight,
    turn: [f32; 4],
    since: f32,
    caliber: Option<u8>,
}

impl Cases {
    /// The hard settings' figures (`Shells`), where the
    /// options' switch is on.
    pub fn new(hard: &trp_data::setup::hard_settings::HardSettings, on: bool) -> Self {
        let settings = on
            .then(|| -> anyhow::Result<ShellSettings> {
                Ok(ShellSettings {
                    radius: hard.float("Shells.radius")?,
                    play_mult: hard.float("Shells.playMult")?,
                    max_casts: hard.float("Shells.maxCastCount")? as i32,
                    step: hard.float("Shells.deltaTimeStep")?,
                    spread: hard.float("Shells.randomReboundSpread")?,
                    velocity_mult: hard.float("Shells.velocityMult")?,
                    velocity_rotation: hard.float("Shells.velocityRotation")?,
                    bounce: hard.float("Shells.bounceSpeedMult")?,
                    rebound_x: hard.vec2("Shells.ReboundRotationX")?,
                    rebound_y: hard.vec2("Shells.ReboundRotationY")?,
                })
            })
            .and_then(|read| {
                read.map_err(|e| eprintln!("the hard settings' figures for spent cases could not be read; none is thrown: {e:#}")).ok()
            });
        Self { settings, at_port: None, in_ports: Vec::new(), ports_rig: None, launcher_case: None, launcher_kind: None, flying: Vec::new(), seed: 0x5bd1_e995, sounds: None, coming: None }
    }
}
