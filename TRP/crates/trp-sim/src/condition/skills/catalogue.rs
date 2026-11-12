//! The skills, their names, classes, weapons and level curves.

use super::*;

/// A skill of the game's list (`SkillManager.DisplayList`, its order). The
/// skills made but left out of it (Recoil Control, Sniping and the rest)
/// are not here: no profile's level ever reaches them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Skill {
    Endurance,
    Strength,
    Vitality,
    Health,
    StressResistance,
    Metabolism,
    Immunity,
    Perception,
    Intellect,
    Attention,
    Charisma,
    Pistol,
    Revolver,
    Smg,
    Assault,
    Shotgun,
    Sniper,
    Lmg,
    Hmg,
    Launcher,
    AttachedLauncher,
    Throwing,
    Melee,
    Dmr,
    AimDrills,
    TroubleShooting,
    Surgery,
    CovertMovement,
    Search,
    MagDrills,
    FieldMedicine,
    FirstAid,
    LightVests,
    HeavyVests,
    NightOps,
    SilentOps,
    WeaponTreatment,
    Auctions,
    Cleanoperations,
    Shadowconnections,
    Taskperformance,
    Crafting,
    HideoutManagement,
}

impl Skill {
    /// Every skill, in the game's order.
    pub const ALL: [Self; 43] = [
        Self::Endurance,
        Self::Strength,
        Self::Vitality,
        Self::Health,
        Self::StressResistance,
        Self::Metabolism,
        Self::Immunity,
        Self::Perception,
        Self::Intellect,
        Self::Attention,
        Self::Charisma,
        Self::Pistol,
        Self::Revolver,
        Self::Smg,
        Self::Assault,
        Self::Shotgun,
        Self::Sniper,
        Self::Lmg,
        Self::Hmg,
        Self::Launcher,
        Self::AttachedLauncher,
        Self::Throwing,
        Self::Melee,
        Self::Dmr,
        Self::AimDrills,
        Self::TroubleShooting,
        Self::Surgery,
        Self::CovertMovement,
        Self::Search,
        Self::MagDrills,
        Self::FieldMedicine,
        Self::FirstAid,
        Self::LightVests,
        Self::HeavyVests,
        Self::NightOps,
        Self::SilentOps,
        Self::WeaponTreatment,
        Self::Auctions,
        Self::Cleanoperations,
        Self::Shadowconnections,
        Self::Taskperformance,
        Self::Crafting,
        Self::HideoutManagement,
    ];

    /// The weapon classes' twelve.
    pub const WEAPON_CLASSES: [Self; 12] =
        [Self::Pistol, Self::Revolver, Self::Smg, Self::Assault, Self::Shotgun, Self::Sniper, Self::Lmg, Self::Hmg, Self::Launcher, Self::AttachedLauncher, Self::Melee, Self::Dmr];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    /// The game's name for it (`ESkillId`), which the profile, the texts
    /// and the app's options file key it by.
    pub fn name(self) -> &'static str {
        match self {
            Self::Endurance => "Endurance",
            Self::Strength => "Strength",
            Self::Vitality => "Vitality",
            Self::Health => "Health",
            Self::StressResistance => "StressResistance",
            Self::Metabolism => "Metabolism",
            Self::Immunity => "Immunity",
            Self::Perception => "Perception",
            Self::Intellect => "Intellect",
            Self::Attention => "Attention",
            Self::Charisma => "Charisma",
            Self::Pistol => "Pistol",
            Self::Revolver => "Revolver",
            Self::Smg => "SMG",
            Self::Assault => "Assault",
            Self::Shotgun => "Shotgun",
            Self::Sniper => "Sniper",
            Self::Lmg => "LMG",
            Self::Hmg => "HMG",
            Self::Launcher => "Launcher",
            Self::AttachedLauncher => "AttachedLauncher",
            Self::Throwing => "Throwing",
            Self::Melee => "Melee",
            Self::Dmr => "DMR",
            Self::AimDrills => "AimDrills",
            Self::TroubleShooting => "TroubleShooting",
            Self::Surgery => "Surgery",
            Self::CovertMovement => "CovertMovement",
            Self::Search => "Search",
            Self::MagDrills => "MagDrills",
            Self::FieldMedicine => "FieldMedicine",
            Self::FirstAid => "FirstAid",
            Self::LightVests => "LightVests",
            Self::HeavyVests => "HeavyVests",
            Self::NightOps => "NightOps",
            Self::SilentOps => "SilentOps",
            Self::WeaponTreatment => "WeaponTreatment",
            Self::Auctions => "Auctions",
            Self::Cleanoperations => "Cleanoperations",
            Self::Shadowconnections => "Shadowconnections",
            Self::Taskperformance => "Taskperformance",
            Self::Crafting => "Crafting",
            Self::HideoutManagement => "HideoutManagement",
        }
    }

    pub fn of_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|skill| skill.name().eq_ignore_ascii_case(name))
    }

    pub fn class(self) -> Class {
        match self {
            Self::Endurance | Self::Strength | Self::Vitality | Self::Health | Self::StressResistance | Self::Metabolism | Self::Immunity => Class::Physical,
            Self::Perception | Self::Intellect | Self::Attention | Self::Charisma => Class::Mental,
            Self::Pistol
            | Self::Revolver
            | Self::Smg
            | Self::Assault
            | Self::Shotgun
            | Self::Sniper
            | Self::Lmg
            | Self::Hmg
            | Self::Launcher
            | Self::AttachedLauncher
            | Self::Throwing
            | Self::Melee
            | Self::Dmr
            | Self::AimDrills
            | Self::TroubleShooting => Class::Combat,
            _ => Class::Practical,
        }
    }

    /// A skill with no buffs, which the game shows locked (`Skill.Locked`).
    pub fn locked(self) -> bool {
        matches!(self, Self::FieldMedicine | Self::FirstAid | Self::NightOps | Self::SilentOps | Self::Auctions | Self::Cleanoperations | Self::Shadowconnections | Self::Taskperformance)
    }

    /// The weapon class whose buffs a weapon takes (`SkillManager.
    /// GetWeaponInfo`): by its own class, the database's name of its
    /// template's parent, the two kinds of assault weapon being one. A
    /// grenade launcher, a rocket launcher or a special weapon is of none
    /// and takes no class's buffs; the class the game calls `Launcher` is
    /// the underbarrel launcher's, a part (`Launcher : GearMod`).
    /// `classes` is the item's class names, nearest first.
    pub fn of_weapon(classes: &[String]) -> Option<Self> {
        classes.iter().find_map(|class| match class.as_str() {
            "AssaultRifle" | "AssaultCarbine" => Some(Self::Assault),
            "Pistol" => Some(Self::Pistol),
            "Revolver" => Some(Self::Revolver),
            "Smg" => Some(Self::Smg),
            "Shotgun" => Some(Self::Shotgun),
            "SniperRifle" => Some(Self::Sniper),
            "MarksmanRifle" => Some(Self::Dmr),
            "MachineGun" => Some(Self::Lmg),
            "Launcher" => Some(Self::Launcher),
            _ => None,
        })
    }
}

/// The level a buff's rule works with (`Skill.UpdateRules`): the level held
/// at 50.
pub(super) fn base(level: u32) -> f32 {
    level.min(50) as f32
}

/// `FloatBuff.PerLevel(f)`: the level times `f`.
pub(super) fn per_level(level: u32, f: f32) -> f32 {
    base(level) * f
}

/// `FloatBuff.Max(top)`: the level times `top / 50`, worked as the game
/// works it (the factor divided first).
pub(super) fn to_top(level: u32, top: f32) -> f32 {
    let factor = top / 50.0;
    base(level) * factor
}

/// Above 50, where the elite rules run.
pub(super) fn elite(level: u32) -> bool {
    level > 50
}

/// What a skill's level makes of a buff that elite puts a figure on:
/// `rule` below elite, `top` at it.
pub(super) fn or_elite(level: u32, rule: f32, top: f32) -> f32 {
    if elite(level) { top } else { rule }
}
