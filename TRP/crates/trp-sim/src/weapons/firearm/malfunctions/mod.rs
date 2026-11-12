//! The weapon's malfunctions, wear and heat in the hands.
//! After `docs/spec/malfunctions.md`: the roll at each shot, the malfunction's
//! own state (`StartMalfunction`), the look at it
//! (`ExamineMalfunctionTypeOperation`), its repair (`RepairMalfunction`),
//! the trigger and the keys while it lasts, the heat's stray shot and the
//! automatic fire's rate by the heat. The rules themselves are
//! `crate::malfunction`'s; the host gives the weapon its wear and its state
//! as it is drawn (`set_wear`) and takes them back (`wear`).

use super::{Event, Firearm, Operation};
use crate::weapons::malfunction::{self, Malf, MalfRandom, RoundFigures, Settings, Skills, State, WeaponFigures};
use crate::items::repair::{BuffKind, RepairBuff};
use crate::base::unity_random::UnityRandom;

mod wear_inputs;
mod rolling;
mod clearing;


/// Which of the three systems are on (the options' `DURABILITY`,
/// `MALFUNCTIONS`, `OVERHEATING`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WearSwitches {
    pub durability: bool,
    pub malfunctions: bool,
    pub overheating: bool,
}

impl Default for WearSwitches {
    fn default() -> Self {
        Self { durability: true, malfunctions: true, overheating: true }
    }
}

/// What the weapon's logic keeps of its wear while in hand.
#[derive(Clone, Debug)]
pub struct Wear {
    pub figures: WeaponFigures,
    pub settings: Settings,
    /// The weapon's durability and its greatest durability now
    /// (`Repairable`), and its malfunction state.
    pub durability: f32,
    pub max_durability: f32,
    pub state: State,
    /// The roll's own generator (`MalfunctionRandom`) and Unity's global one
    /// for the other draws (the heat's wear, the fire rate's factor, the
    /// stray shot).
    pub random: MalfRandom,
    pub engine: UnityRandom,
    /// The round to fire next and the magazine's chance (none with no
    /// magazine in), as the host's inventory has them.
    pub round: RoundFigures,
    pub magazine_chance: Option<f32>,
    pub skills: Skills,
    /// How many arms are broken (`LeftArmDamaged`, `RightArmDamaged`).
    pub arms_broken: u32,
    /// The raid's clock (`GameHelper.PastTime`), as the host last said.
    pub now: f64,
    pub switches: WearSwitches,
    /// The next shot's malfunction, given outright (the game's console's,
    /// `EMalfunctionSource.ConsoleCommand`): for a test, or a recording's
    /// own malfunction set on its shot.
    pub force_next: Option<Malf>,
    /// The weapon's repair buff, as its item keeps it (`Weapon.Buff`): a
    /// malfunction protection lowers the roll's chances; any is gone on
    /// the shot whose wear takes the durability under its threshold.
    pub buff: Option<RepairBuff>,
}

impl Wear {
    pub fn new(figures: WeaponFigures, settings: Settings, durability: f32, max_durability: f32, state: State, seed: i32) -> Self {
        Self {
            figures,
            settings,
            durability,
            max_durability,
            state,
            random: MalfRandom::new(seed),
            engine: UnityRandom::new(seed.wrapping_mul(31).wrapping_add(7)),
            round: RoundFigures::default(),
            magazine_chance: None,
            skills: Skills::default(),
            arms_broken: 0,
            now: 0.0,
            switches: WearSwitches::default(),
            force_next: None,
            buff: None,
        }
    }

    /// The figures with the switches applied: heat only where overheating
    /// is on.
    fn figures(&self) -> WeaponFigures {
        WeaponFigures { allow_overheat: self.figures.allow_overheat && self.switches.overheating, ..self.figures }
    }

    /// The heat now.
    pub fn heat(&self) -> f32 {
        malfunction::current_heat(&self.state, self.now, &self.figures(), &self.settings)
    }
}

/// The hands' state of each malfunction, played outright by an automatic
/// weapon's (`SwitchToMalfunction`).
fn state_name(malf: Malf) -> Option<&'static str> {
    match malf {
        Malf::None => None,
        Malf::Misfire => Some("Hands.MISFIRE"),
        Malf::Jam => Some("Hands.JAM"),
        Malf::HardSlide => Some("Hands.HARD_SLIDE"),
        Malf::SoftSlide => Some("Hands.SOFT_SLIDE"),
        Malf::Feed => Some("Hands.FEED"),
    }
}

impl Firearm {
}
