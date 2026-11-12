//! Body parts, damage kinds and the odds of a chance.

use super::*;

/// A body part (`EBodyPart`), in the game's order, which ranks them where
/// nothing else does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Part {
    Head,
    Chest,
    Stomach,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
}

impl Part {
    pub const ALL: [Part; 7] = [Part::Head, Part::Chest, Part::Stomach, Part::LeftArm, Part::RightArm, Part::LeftLeg, Part::RightLeg];

    pub fn index(self) -> usize {
        self as usize
    }

    /// The game's own name for it (`BodyPartsSettings`' keys).
    pub fn name(self) -> &'static str {
        match self {
            Part::Head => "Head",
            Part::Chest => "Chest",
            Part::Stomach => "Stomach",
            Part::LeftArm => "LeftArm",
            Part::RightArm => "RightArm",
            Part::LeftLeg => "LeftLeg",
            Part::RightLeg => "RightLeg",
        }
    }

    pub fn of_name(name: &str) -> Option<Part> {
        Part::ALL.into_iter().find(|part| part.name() == name)
    }

    pub(super) fn limb(self) -> bool {
        matches!(self, Part::LeftArm | Part::RightArm | Part::LeftLeg | Part::RightLeg)
    }

    /// Head and chest: destroyed, they kill, and nothing mends them.
    pub(super) fn vital(self) -> bool {
        matches!(self, Part::Head | Part::Chest)
    }
}

/// What did the damage (`EDamageType`): those the range can bring about,
/// and a bullet's, for a host that shoots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageKind {
    Undefined,
    Fall,
    Barbed,
    Existence,
    Medicine,
    Bullet,
    Melee,
    Blunt,
    GrenadeFragment,
    Explosion,
    LightBleeding,
    HeavyBleeding,
    Dehydration,
    Exhaustion,
    /// A stimulator's buff taking health away (`EDamageType.Stimulator`).
    Stimulator,
    /// An intoxication's loop (`DamageHelper.PoisonDamage`).
    Poison,
}

impl DamageKind {
    /// The game's own word for it, which its texts name the cause of a
    /// death by.
    pub fn name(self) -> &'static str {
        match self {
            DamageKind::Undefined => "Undefined",
            DamageKind::Fall => "Fall",
            DamageKind::Barbed => "Barbed",
            DamageKind::Existence => "Existence",
            DamageKind::Medicine => "Medicine",
            DamageKind::Bullet => "Bullet",
            DamageKind::Melee => "Melee",
            DamageKind::Blunt => "Blunt",
            DamageKind::GrenadeFragment => "GrenadeFragment",
            DamageKind::Explosion => "Explosion",
            DamageKind::LightBleeding => "LightBleeding",
            DamageKind::HeavyBleeding => "HeavyBleeding",
            DamageKind::Dehydration => "Dehydration",
            DamageKind::Exhaustion => "Exhaustion",
            DamageKind::Stimulator => "Stimulator",
            DamageKind::Poison => "Poison",
        }
    }

    /// `DamageHelper.SelfInflictedDamage`.
    pub fn self_inflicted(self) -> bool {
        matches!(self, DamageKind::LightBleeding | DamageKind::HeavyBleeding | DamageKind::Dehydration | DamageKind::Exhaustion | DamageKind::Existence | DamageKind::Stimulator | DamageKind::Poison)
    }

    /// `DamageHelper.WeaponDamage`.
    pub fn weapon(self) -> bool {
        matches!(self, DamageKind::Bullet | DamageKind::Melee | DamageKind::Blunt)
    }

    /// `DamageHelper.EnemiesDamage`.
    pub fn enemy(self) -> bool {
        self.weapon() || matches!(self, DamageKind::Explosion | DamageKind::GrenadeFragment)
    }

    pub(super) fn bleeding(self) -> bool {
        matches!(self, DamageKind::LightBleeding | DamageKind::HeavyBleeding)
    }

    /// `DamageHelper.EnvironmentalDamage` (barbed wire and fire).
    pub(super) fn environmental(self) -> bool {
        self == DamageKind::Barbed
    }
}

/// The chances damage draws on a part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chance {
    HeavyBleeding,
    LightBleeding,
    Fracture,
}

/// The odds of a bleeding or a fracture (`ProbabilitySetting`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Odds {
    /// 1 for `x`, 2 for `x²`, 0.5 for `√x`.
    pub power: f32,
    pub k: f32,
    pub b: f32,
    pub threshold: f32,
}

impl Odds {
    pub(super) fn read(v: &Json) -> Result<Self> {
        let power = match v.get("FunctionType").and_then(Json::as_str) {
            Some("Quadratic") => 2.0,
            Some("SquareRoot") => 0.5,
            _ => 1.0,
        };
        Ok(Self { power, k: num(v, "K")?, b: num(v, "B")?, threshold: num(v, "Threshold")? })
    }

    /// Whether a draw between 0 and `top` comes under the chance at `x`.
    pub(super) fn try_at(&self, x: f32, top: f32, bias: f32, rng: &mut Rng) -> bool {
        if x < self.threshold {
            return false;
        }
        rng.range(0.0, top) < self.k * x.powf(self.power) + self.b + bias
    }
}
