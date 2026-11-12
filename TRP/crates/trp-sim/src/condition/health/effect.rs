//! The kinds of effect, their stages, medical work and the effect itself.

use super::*;

/// An effect's kind, of those built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    LightBleeding,
    HeavyBleeding,
    Wound,
    Fracture,
    Pain,
    Tremor,
    PainKiller,
    LowHealth,
    Existence,
    Exhaustion,
    Dehydration,
    ChronicFatigue,
    Encumbered,
    OverEncumbered,
    MedEffect,
    Contusion,
    Stun,
    Flash,
    /// A stimulator or a buffed food's set at work (`Stimulator`), and what
    /// its buffs make: the weight's limits raised (`Endurance`), the damage
    /// taken changed (`DamageModifier`), the tunnel vision.
    Stimulator,
    Endurance,
    DamageModifier,
    TunnelVision,
    /// Stress Resistance's elite (`Berserk`): low health's side effect.
    Berserk,
    /// A poisoning (`Intoxication`): on the stomach, health from every
    /// part each loop until cured.
    Intoxication,
    /// Immunity's notice that it rolled to keep an ill effect off
    /// (`ImmunityPreventedNegativeEffect`), on the chest.
    ImmunityPrevented,
}

impl Kind {
    pub const ALL: [Kind; 25] = [
        Kind::LightBleeding,
        Kind::HeavyBleeding,
        Kind::Wound,
        Kind::Fracture,
        Kind::Pain,
        Kind::Tremor,
        Kind::PainKiller,
        Kind::LowHealth,
        Kind::Existence,
        Kind::Exhaustion,
        Kind::Dehydration,
        Kind::ChronicFatigue,
        Kind::Encumbered,
        Kind::OverEncumbered,
        Kind::MedEffect,
        Kind::Contusion,
        Kind::Stun,
        Kind::Flash,
        Kind::Stimulator,
        Kind::Endurance,
        Kind::DamageModifier,
        Kind::TunnelVision,
        Kind::Berserk,
        Kind::Intoxication,
        Kind::ImmunityPrevented,
    ];

    /// The kind by the game's class name for it.
    pub fn of_name(name: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|kind| kind.name() == name)
    }

    /// The game's class name for it, which its texts are keyed by.
    pub fn name(self) -> &'static str {
        match self {
            Kind::LightBleeding => "LightBleeding",
            Kind::HeavyBleeding => "HeavyBleeding",
            Kind::Wound => "Wound",
            Kind::Fracture => "Fracture",
            Kind::Pain => "Pain",
            Kind::Tremor => "Tremor",
            Kind::PainKiller => "PainKiller",
            Kind::LowHealth => "LowEdgeHealth",
            Kind::Existence => "Existence",
            Kind::Exhaustion => "Exhaustion",
            Kind::Dehydration => "Dehydration",
            Kind::ChronicFatigue => "ChronicStaminaFatigue",
            Kind::Encumbered => "Encumbered",
            Kind::OverEncumbered => "OverEncumbered",
            Kind::MedEffect => "MedEffect",
            Kind::Contusion => "Contusion",
            Kind::Stun => "Stun",
            Kind::Flash => "Flash",
            Kind::Stimulator => "Stimulator",
            Kind::Endurance => "Endurance",
            Kind::DamageModifier => "DamageModifier",
            Kind::TunnelVision => "TunnelVision",
            Kind::Berserk => "Berserk",
            Kind::Intoxication => "Intoxication",
            Kind::ImmunityPrevented => "ImmunityPreventedNegativeEffect",
        }
    }

    /// `IStackable`: a second is a second, not the first lengthened.
    pub(super) fn stacks(self) -> bool {
        matches!(self, Kind::Tremor | Kind::MedEffect | Kind::Contusion)
    }

    /// `IStackableFromDifferentTemplates`: one for each item.
    pub(super) fn stacks_by_item(self) -> bool {
        matches!(self, Kind::PainKiller | Kind::Stimulator)
    }

    pub(super) fn bleeding(self) -> bool {
        matches!(self, Kind::LightBleeding | Kind::HeavyBleeding)
    }

    /// Neither wanted (`IDesirable`) nor for good (`IPermanent`): what
    /// `RemoveNegativeEffects` takes away. An intoxication is for good.
    pub fn negative(self) -> bool {
        !matches!(self, Kind::PainKiller | Kind::Existence | Kind::MedEffect | Kind::Stimulator | Kind::Berserk | Kind::Intoxication)
    }
}

/// An effect's state (`EEffectState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    None,
    Added,
    Started,
    Residual,
    Removed,
}

/// What a use of medicine keeps while at work (`MedEffect`).
#[derive(Debug, Clone)]
pub struct MedWork {
    pub medicine: Arc<Medicine>,
    /// The item's own number, which the host knows it by.
    pub item: u64,
    /// The share of it used (food); 1 for medicine.
    pub amount: f32,
    /// What it holds: a medkit's hit points, food's portion.
    pub resource: f32,
    pub(super) health_points: f32,
    pub(super) energy_points: f32,
    pub(super) hydration_points: f32,
    pub(super) energy_gained: f32,
    pub(super) hydration_gained: f32,
    pub(super) loop_time: f32,
    pub(super) interrupted: bool,
    /// It cannot be used walking (`MedEffect.IsNoMove`), as at its start.
    pub no_move: bool,
}

/// One effect on the character.
#[derive(Debug, Clone)]
pub struct Effect {
    pub id: u32,
    pub kind: Kind,
    pub part: Part,
    pub stage: Stage,
    pub strength: f32,
    pub current_strength: f32,
    pub delay: f32,
    pub build_up: f32,
    pub work: f32,
    pub residue: f32,
    pub(super) state_time: f32,
    pub state_current: f32,
    /// Seconds since it was made.
    pub whole: f32,
    pub(super) forced: bool,
    pub(super) side: Option<u32>,
    /// Timers of its own loops.
    pub(super) loops: [f32; 2],
    /// Its own figures (a bleeding's damage and loop, a wound's threshold
    /// and tally, a painkiller's item and full duration).
    pub(super) figures: [f32; 4],
    /// A painkiller's item.
    pub(super) item: Option<String>,
    /// Pain's count to its tremor.
    pub(super) tremor_after: Option<f32>,
    pub med: Option<Box<MedWork>>,
    /// A stimulator's own state (its set, its buffs).
    pub stim: Option<Box<StimWork>>,
}

impl Effect {
    /// Added and not yet started, or started (`Existing`).
    pub fn existing(&self) -> bool {
        matches!(self.stage, Stage::Added | Stage::Started)
    }

    pub fn active(&self) -> bool {
        self.stage == Stage::Started
    }

    /// Seconds left of the state it is in (`CurStateTimeLeft`).
    pub fn state_left(&self) -> f32 {
        if self.state_time.is_finite() { self.state_time - self.state_current } else { f32::INFINITY }
    }

    /// Seconds left of it all (`TimeLeft`).
    pub fn time_left(&self) -> f32 {
        if self.stage == Stage::Residual {
            self.state_time - self.state_current
        } else {
            self.delay + self.work + 0.0 - self.whole
        }
    }

    pub(super) fn calculate_strength(&mut self, dt: f32) {
        match self.stage {
            // A stun holds to a fifth of its time past its build-up, then
            // falls straight to nothing at its end
            // (`Stun.CalculateCurrentStrength`).
            Stage::Started if self.kind == Kind::Stun && self.state_current >= self.build_up + 0.2 * self.work => {
                let held = self.build_up + 0.2 * self.work;
                self.current_strength = self.strength * (self.state_left() / (self.work - held)).clamp(0.0, 1.0);
            }
            Stage::Started => {
                let (gap, left) = (self.strength - self.current_strength, self.build_up - self.state_current);
                if self.build_up != 0.0 && left > 0.0 {
                    self.current_strength += dt * gap / left;
                } else {
                    self.current_strength = self.strength;
                }
            }
            Stage::Residual => {
                let left = self.state_left();
                if left.abs() < f32::EPSILON {
                    self.current_strength = 0.0;
                } else {
                    self.current_strength *= 1.0 - dt / left;
                }
            }
            Stage::None | Stage::Added | Stage::Removed => self.current_strength = 0.0,
        }
        self.current_strength = if self.strength > 0.0 { self.current_strength.clamp(0.0, self.strength) } else { self.current_strength.clamp(self.strength, 0.0) };
    }
}
