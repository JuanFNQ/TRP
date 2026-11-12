//! The character's health: body parts, effects, damage, medicine and death.
//! As specified in `docs/spec/health.md`: seven
//! body parts with their hit points, energy and hydration, the effects
//! that hurt, heal or change what the character can do, damage and its
//! spread, falls, barbed wire, medicine and food at work, death, and the
//! physical conditions the rest of the controller reads from all of it.
//!
//! The game's `ActiveHealthController` as the one at the keys has it
//! (`PlayerHealthController`). Nothing here knows of an engine: a host
//! steps it, tells it of falls, cuts and what is used, and reads back the
//! conditions (`Health::conditions`) and what happened (`take_events`).

use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use serde_json::Value as Json;
use trp_data::Database;

use crate::weapons::recoil::Rng;

mod stimulator;
pub use stimulator::{BuffDef, BuffKind, BuffRun, SideEffect, StimWork, StimulatorSettings};

mod parts;
mod settings;
mod medicine;
mod effect;
mod conditions;
mod readings;
mod lifecycle;
mod update;
mod damage;
mod treatment;
mod med_work;

pub use parts::*;
pub use settings::*;
pub use medicine::*;
pub use effect::*;
pub use conditions::*;
pub use damage::*;
use settings::num;
use med_work::{cure_of, share};

/// The character's health (`ActiveHealthController`).
#[derive(Debug, Clone)]
pub struct Health {
    pub settings: Arc<HealthSettings>,
    pub parts: [PartState; 7],
    /// Current, least and most.
    pub energy: [f32; 3],
    pub hydration: [f32; 3],
    pub alive: bool,
    /// What killed it.
    pub cause: Option<DamageKind>,
    pub effects: Vec<Effect>,
    next_id: u32,
    rng: Rng,
    /// Chances' outcomes laid down beforehand, each used once by the next
    /// draw of its chance on its part in place of the draw: a replay takes
    /// them from a recording, where the game drew them (none otherwise).
    pub outcomes: Vec<(Chance, Part, bool)>,
    events: Vec<Event>,
    /// The damage multiplier (stimulators'); 1.
    pub damage_multiplier: f32,
    /// The skills' buffs (none untrained).
    pub skills: HealthSkills,
    /// Turned off, hunger and thirst stop (`DisableMetabolism`).
    pub metabolism_disabled: bool,
    /// The health's own clock, for what the game times by the engine's.
    clock: f32,
    /// The sprint's self-damage on a damaged leg (`InflictSelfDamage`):
    /// whether it runs, and its count down.
    self_damage: Option<f32>,
    encumbered: Option<u32>,
    over_encumbered: Option<u32>,
    /// A berserk's starts (true) and residues (false) since the stamina was
    /// last told, in order (`Player.OnHealthEffectAdded`, `Removed`: one
    /// forced off while it waited goes through both at once).
    berserk_edges: Vec<bool>,
}

/// The default figures each kind takes when not told: delay, working time,
/// residue, build-up.
fn defaults(kind: Kind, s: &HealthSettings) -> [f32; 4] {
    let inf = f32::INFINITY;
    match kind {
        Kind::LightBleeding => [s.light.delay, inf, s.light.residue, 1.0],
        Kind::HeavyBleeding => [s.heavy.delay, inf, s.heavy.residue, 1.0],
        Kind::Wound => [0.0, s.wound[2], 0.0, 1.0],
        Kind::Fracture => [s.fracture.delay, inf, s.fracture.residue, 1.0],
        // `Pain.DefaultResidue`, `DefaultBuildUpTime`: the game's code.
        Kind::Pain => [0.0, inf, 15.0, 0.1],
        Kind::Tremor => [s.tremor.delay, inf, s.tremor.residue, 1.0],
        Kind::LowHealth => [s.low_health.delay, inf, s.low_health.residue, 1.0],
        Kind::Exhaustion => [s.exhaustion.delay, inf, s.exhaustion.residue, 1.0],
        Kind::Dehydration => [s.dehydration.delay, inf, s.dehydration.residue, 1.0],
        Kind::PainKiller | Kind::Existence | Kind::ChronicFatigue | Kind::Encumbered | Kind::OverEncumbered | Kind::MedEffect | Kind::Contusion | Kind::Flash => [0.0, inf, 0.0, 1.0],
        Kind::Stimulator | Kind::Endurance | Kind::DamageModifier => [0.0, inf, 0.0, 1.0],
        // `TunnelVision.DefaultBuildUpTime`, `DefaultResidueTime`.
        Kind::TunnelVision => [0.0, inf, 2.0, 2.0],
        // The settings' times; the base build-up (`Effect.DefaultBuildUpTime`).
        Kind::Berserk => [s.berserk[0], s.berserk[1], s.berserk[2], 1.0],
        // The settings' delay and residue, working till cured.
        Kind::Intoxication => [s.intoxication[0], inf, s.intoxication[1], 1.0],
        // `ImmunityPreventedNegativeEffect`'s own: 2 s of work, 8 of residue.
        Kind::ImmunityPrevented => [0.0, 2.0, 8.0, 1.0],
        // `Stun.StunBuildUpTime`, `DefaultResidueTime`.
        Kind::Stun => [0.0, inf, 2.0, 0.2],
    }
}

/// `Random.Range` for whole numbers: the larger never drawn.
impl Health {
    /// A whole number from `least` up to but not `most`, by health's own
    /// chance (`UnityEngine.Random.Range` of whole numbers): what the
    /// item's hands start their variants' count at.
    pub fn random_whole(&mut self, least: i32, most: i32) -> i32 {
        if most <= least { least } else { whole_range(&mut self.rng, least, most) }
    }
}

fn whole_range(rng: &mut Rng, least: i32, most: i32) -> i32 {
    if most <= least {
        return least;
    }
    least + ((rng.unit() * (most - least) as f32) as i32).min(most - least - 1)
}

impl Health {
}

#[cfg(test)]
pub(crate) mod tests;
