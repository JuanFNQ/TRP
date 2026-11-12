//! What the character pays for moving and for holding a weapon up.
//! The game's three pools (legs, arms, oxygen), the drains on them and the
//! refill with its pause. Specified in `docs/spec/stamina.md`.

use anyhow::{anyhow, Context, Result};
use serde_json::Value as Json;

use crate::base::math::{inverse_lerp, lerp};

mod settings;
mod pool;
mod feeds;
mod moves;
mod arms;

pub use settings::*;
pub use pool::*;

/// Below this a pool is exhausted, and at or below it nothing that needs
/// a start threshold starts (`Stamina.EXHAUSTED_THRESHOLD`; the sprint's
/// and the jump's `StartThreshold` are the same figure).
pub const EXHAUSTED: f32 = 15.0;
/// The melee consumption's start threshold and its pause on the refill
/// (`Physical`: `Consumptions[Melee]`, `StartThreshold` 10, `Downtime` 0.5).
const MELEE_START: f32 = 10.0;
const MELEE_DOWNTIME: f32 = 0.5;

/// The character's stamina (the game's `Physical`).
#[derive(Clone, Debug)]
pub struct Stamina {
    pub settings: StaminaSettings,
    pub legs: Pool,
    pub arms: Pool,
    pub oxygen: Pool,
    /// The weapon's ergonomic weight, which aiming costs by: a host sets
    /// it for the weapon in hand (weapon-handling spec, "When the
    /// weapon's weight is taken").
    pub aim_mass: f32,
    /// The stance class the costs go by: 0 lying, 1 fully crouched, 2 any
    /// higher.
    pose: usize,
    /// How much the stance has risen since the last charge for it.
    pub pose_rise: f32,
    /// The cost of starting a sprint again too soon, and until when
    /// (engine's clock) a new start counts as too soon.
    spam: f32,
    spam_until: f32,
    /// What the sights up cost a second as last worked out, with the
    /// sights' state and the weight it was worked out for. The game keeps
    /// this figure until the sights, the stance class, the weapon's
    /// weight or the skills change; how well watered the character is is
    /// in it and is not among those (stamina spec, "Figures the game
    /// keeps").
    aim_cost: Option<f32>,
    aim_taken: (bool, f32),
    /// The aiming arms' drain multiplier on a mount (`Physical`: 0.85 at a
    /// side, 0.6 forward, 0.2 forward on the bipod; 1 off a mount), set by
    /// the movement (`set_aim_drain_by`).
    aim_drain_by: f32,
    /// The handheld rangefinder is in hand: the aim at its own rate
    /// (`Physical`: `HandsController is PortableRangeFinderController`).
    rangefinder: bool,
    /// The two systems of the features' list this is (`crate::Systems`):
    /// with `drains` off nothing is paid and the pools stay full; with
    /// `holds_back` off (exhaustion) a spent pool holds nothing back.
    pub drains: bool,
    pub holds_back: bool,
    /// Whether the character's health lets a sprint be paid for (`Physical`'s
    /// sprint drain: no medicine at work, no damaged leg without a
    /// painkiller). Health's; true unless a host's health system says.
    pub health_lets_sprint: bool,
    /// The tally of spent legs, and how many fatigues it has given that
    /// health has not taken (`PhysicalBase`'s `Fatigue`).
    fatigue: f32,
    fatigues: u32,
    /// What the arms lacked at the last fast switch that was paid for
    /// (`_fastWeaponSwitchStaminaLack`, worked out anew at each reading of
    /// the cost: kept through a switch while nothing is paid).
    switch_lack: f32,
}

impl Stamina {

    /// One frame of `dt` seconds by the player's clock, with the engine's
    /// clock at `now`: the legs, the arms and the oxygen in that order.
    pub fn update(&mut self, dt: f32, now: f32) {
        // Switched off, nothing is paid for: the pools are full again,
        // whatever the frame's charges took.
        if !self.drains {
            for pool in [&mut self.legs, &mut self.arms, &mut self.oxygen] {
                pool.current = pool.capacity;
            }
            return;
        }
        self.aim_cost = Some(self.cost(Drain::Aim));
        let costs = [Drain::Sprint, Drain::Walk, Drain::Aim, Drain::OverweightIdle, Drain::Crawl, Drain::HoldBreath]
            .map(|d| (d, self.cost(d)));
        let s = &self.settings;
        let refills = [
            s.restoration * s.refill_factor(self.pose),
            s.hands_restoration * s.refill_factor(self.pose),
            s.oxygen_restoration * (1.0 + s.endurance_restoration),
        ];
        let buff = s.buff_restoration;
        if self.legs.work(&costs, refills[0], buff, dt, now) {
            self.legs_ran_dry();
        }
        if self.arms.work(&costs, refills[1], buff / 2.0, dt, now) {
            self.arms_ran_dry();
        }
        if self.oxygen.work(&costs, refills[2], buff, dt, now) && self.holds_back {
            // Out of oxygen, the breath is let go.
            self.oxygen.stop(Drain::HoldBreath);
        }
        // The tally of spent legs: under the threshold it runs up, and past
        // its figure health is given a fatigue and it starts again;
        // otherwise it falls.
        let [enough, falls] = self.settings.fatigue;
        if self.legs.exhausted() {
            self.fatigue += dt;
            if self.fatigue > enough {
                self.fatigues += 1;
                self.fatigue = 0.0;
            }
        } else if self.fatigue > 0.0 {
            self.fatigue -= falls * dt;
        }
    }
}

#[cfg(test)]
mod tests;
