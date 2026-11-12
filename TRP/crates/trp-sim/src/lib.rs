//! The character controller as a plain simulation.
//!
//! Nothing here knows about rendering or about any particular engine. A
//! host steps it with input and reads back where the character, the weapon
//! and the camera are.

// What the crate prints, to the console and, in the app, to its run's log too
// (`trp_data::console`). Before the modules, so that they print with these. (Only the
// tests print to the standard output today.)
#[allow(unused_macros)]
macro_rules! println {
    () => {
        ::trp_data::console::line(false, ::std::format_args!(""))
    };
    ($($arg:tt)*) => {
        ::trp_data::console::line(false, ::std::format_args!($($arg)*))
    };
}
macro_rules! eprintln {
    () => {
        ::trp_data::console::line(true, ::std::format_args!(""))
    };
    ($($arg:tt)*) => {
        ::trp_data::console::line(true, ::std::format_args!($($arg)*))
    };
}

pub mod lighting;
pub mod base;
pub mod condition;
pub mod weapons;
pub mod combat;
pub mod effects;
pub mod characters;
pub mod locomotion;
pub mod held;
pub mod items;
pub mod recordings;
pub mod sound;

pub use locomotion::body::{ArmPose, Body};
pub use weapons::breath::Wind;
pub use held::empty_hands::EmptyHands;
pub use weapons::firearm::{ChamberByHand, ChamberKey, Event as FirearmEvent, FireMode, Firearm, FirearmSettings};
pub use held::grenade_hands::{GrenadeEvent, GrenadeHands};
pub use held::knife_hands::{BladeSweep, KnifeEvent, KnifeHands, KnifeSettings, Stroke};
pub use held::rangefinder::{RangefinderEvent, RangefinderHands};
pub use items::magazine_process::MagazineProcess;
pub use weapons::hands::{AimPoint, CameraPose, Carried, Hands, HandsInput, HandsSettings, Held, SightLine};
pub use held::interactions::Interaction;
pub use held::meds_hands::{MedsEvent, MedsHands};
pub use locomotion::movement::{Acted, Flat, Input, Movement, State, Stopwatch, World, WorldInteraction};
pub use base::pacing::{Pacer, REFERENCE_FRAME_RATE};
pub use base::settings::MovementSettings;
pub use locomotion::stamina::{Stamina, StaminaSettings};
pub use base::systems::Systems;

use anyhow::{anyhow, Context, Result};
use trp_data::formats::curve::{Curve, Key};
use trp_data::setup::engine::EngineSettings;
use trp_data::setup::hard_settings::HardSettings;
use trp_data::character::player::HandsData;
use trp_data::character::rootmotion::RootMotionTable;
use trp_data::weapons::states::StateTable;
use trp_data::unity::Assets;
use trp_data::weapons::weapon::Weapon;
use trp_data::Database;
use serde_json::Value as Json;

use weapons::recoil::{CurveRecoil, RecoilSettings, RotationSettings, ShotGroup};

mod install_settings;
mod weapon_geometry;
mod weapon_settings;

pub use install_settings::*;
pub use weapon_geometry::*;
pub use weapon_settings::*;
use install_settings::{number, pair, triple};
use weapon_geometry::{aim_plane_depth, weapon_root};
use weapon_settings::{REST_STATE, recoil_settings};

/// Whose eyes a character is seen with: its own, or anyone else's (another
/// player's, a camera's). The game has one class for both and a switch on
/// it (`Player.PointOfView`): a bot, and whoever is not the one playing,
/// is that class seen from outside. What the switch changes is in
/// `docs/spec/third-person.md`; `Body::set_point_of_view` and
/// `Hands::set_point_of_view` are told it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointOfView {
    #[default]
    FirstPerson,
    ThirdPerson,
}

/// The player's own settings that change how the weapon is shown, with the
/// game's defaults.
#[derive(Debug, Clone, Copy)]
pub struct ViewSettings {
    /// Vertical field of view in degrees, 50 to 75.
    pub field_of_view: f32,
    /// 0.2 to 1.
    pub head_bobbing: f32,
    /// Seed of the recoil's random numbers.
    pub seed: u64,
    /// What the character's skills add to the aiming speed, as a fraction.
    /// Zero for an untrained character.
    pub aim_speed_bonus: f32,
}

impl Default for ViewSettings {
    fn default() -> Self {
        Self { field_of_view: 50.0, head_bobbing: 0.2, seed: 1, aim_speed_bonus: 0.0 }
    }
}
