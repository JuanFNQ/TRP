//! The buffs worked out from the skill levels and their definitions.

use super::*;

/// Mag Drills' buffs (`Player.LoadMagazine`, `UnloadMagazine`,
/// `CheckMagazine`): each a percentage taken off a round's or a check's
/// time.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MagDrills {
    pub load: f32,
    pub unload: f32,
    pub check: f32,
    /// Elite: each round after the first quicker (`MagDrillsLoadProgression`).
    pub load_progression: bool,
    /// Elite: a magazine known once put into one's own things
    /// (`MagDrillsInstantCheck`).
    pub instant_check: bool,
}

/// A weapon class's buffs (`SkillManager.GetWeaponBuffs`, `GetWeaponInfo`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponBuffs {
    /// The animator's reload and draw speeds, 1 untrained.
    pub reload_speed: f32,
    pub swap_speed: f32,
    /// The recoil's suppression, its first part (`WeaponRecoilBuff`).
    pub recoil: f32,
    /// Added to the share the weapon's ergonomics is multiplied by
    /// (`WeaponErgonomicsBuff`; `TotalErgonomicsFunc`).
    pub ergonomics: f32,
    /// Elite: the breath held stiff for 3 s when the sights come up
    /// (`DrawElite`, the class's or Aim Drills').
    pub stiff_draw: bool,
    /// Added to the ergonomics' share while mounted, and mounted forward on
    /// the bipod (`MountingErgonomicsGainPerLevel` 0.2 and
    /// `BipodErgonomicsGainPerLevel` 0.5 a level, hidden buffs; mounting
    /// spec, "Ergonomics": PROVISIONAL, read at face value).
    pub mount_ergonomics: [f32; 2],
    /// The share of a double action's accuracy penalty taken off
    /// (`WeaponDoubleActionRecoilReduceBuff`, 0.01 a level, hidden; read
    /// only by the shot's spread, whatever its name says).
    pub double_action_reduce: f32,
}

impl Default for WeaponBuffs {
    fn default() -> Self {
        Self { reload_speed: 1.0, swap_speed: 1.0, recoil: 0.0, ergonomics: 0.0, stiff_draw: false, mount_ergonomics: [0.0; 2], double_action_reduce: 0.0 }
    }
}

/// What the levels make of the buffs the simulation and the app read, each
/// by the game's rule and figure (`docs/spec/skills.md`, "What each skill
/// changes"). Health's skills are `crate::health::HealthSkills`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Buffs {
    /// Endurance: the legs' capacity (0.5 at 50, 0.7 elite), the arms'
    /// (0.5 at elite), a jump's cost (0.3), the breath's capacity (1), the
    /// legs' and the breath's recovery (0.5, 0.75 elite), and at elite the
    /// breath free of the energy.
    pub endurance_capacity: f32,
    pub endurance_hands: f32,
    pub endurance_jump: f32,
    pub endurance_breath: f32,
    pub endurance_restoration: f32,
    pub endurance_breath_elite: bool,
    /// Strength: its level for the top speeds and the inertia's limits
    /// (`SummaryLevel`); a jump's push (0.2 at 50), the overweight limits
    /// (0.3), a melee blow (0.3), a high throw (0.2), aiming's drain
    /// (0.2); at elite the weapons weighing nothing.
    pub strength_level: u32,
    pub strength_jump: f32,
    pub strength_lift: f32,
    pub strength_melee: f32,
    pub strength_throw: f32,
    pub strength_aim: f32,
    pub strength_elite: bool,
    /// Aim Drills: the sights brought up faster by it (`DrawSpeed`, 0.5 at
    /// 50); at elite the stiff breath (`DrawElite`).
    pub aim_speed: f32,
    pub aim_drills_elite: bool,
    /// Throwing: a throw's cost less by it (0.01 a level); at elite no
    /// cost and the throw's strength not cut by the arms.
    pub throwing_cost: f32,
    pub throwing_elite: bool,
    /// Troubleshooting: a malfunction cleared faster (0.005 a level).
    pub fix_speed: f32,
    /// Attention: searching faster (0.02 a level), examining faster (0.02),
    /// at elite a container's first search over at once half the time.
    pub attention_search: f32,
    pub attention_examine: f32,
    pub attention_lucky: f32,
    /// Search: searching a body faster (0.01 a level); at elite two at once.
    pub search_speed: f32,
    pub search_double: bool,
    /// Perception: the look's finding sphere wider (0.02 a level).
    pub perception_radius: f32,
    pub covert: Covert,
    pub mag_drills: MagDrills,
    pub(super) levels: [u32; 43],
}

impl Buffs {
    pub(super) fn of(levels: &SkillLevels) -> Self {
        Self::of_fn(|skill| levels.level(skill))
    }

    /// The buffs from each skill's level as `at` gives it: the plain level,
    /// or the summary level with stimulators' buffs (`SkillLevels::summary`).
    pub(super) fn of_fn(at: impl Fn(Skill) -> u32) -> Self {
        let endurance = at(Skill::Endurance);
        let strength = at(Skill::Strength);
        let covert = at(Skill::CovertMovement);
        let drills = at(Skill::MagDrills);
        let attention = at(Skill::Attention);
        Self {
            endurance_capacity: or_elite(endurance, to_top(endurance, 0.5), 0.7),
            endurance_hands: or_elite(endurance, 0.0, 0.5),
            endurance_jump: to_top(endurance, 0.3),
            endurance_breath: to_top(endurance, 1.0),
            endurance_restoration: or_elite(endurance, to_top(endurance, 0.5), 0.75),
            endurance_breath_elite: elite(endurance),
            strength_level: strength,
            strength_jump: to_top(strength, 0.2),
            strength_lift: to_top(strength, 0.3),
            strength_melee: to_top(strength, 0.3),
            strength_throw: to_top(strength, 0.2),
            strength_aim: to_top(strength, 0.2),
            strength_elite: elite(strength),
            aim_speed: to_top(at(Skill::AimDrills), 0.5),
            aim_drills_elite: elite(at(Skill::AimDrills)),
            throwing_cost: per_level(at(Skill::Throwing), 0.01),
            throwing_elite: elite(at(Skill::Throwing)),
            fix_speed: per_level(at(Skill::TroubleShooting), 0.005),
            attention_search: per_level(attention, 0.02),
            attention_examine: per_level(attention, 0.02),
            attention_lucky: or_elite(attention, 0.0, 0.5),
            search_speed: per_level(at(Skill::Search), 0.01),
            search_double: elite(at(Skill::Search)),
            perception_radius: per_level(at(Skill::Perception), 0.02),
            covert: Covert {
                soft: per_level(covert, 0.012),
                loud: or_elite(covert, per_level(covert, 0.006), 0.6),
                equipment: per_level(covert, 0.012),
                speed: 1.0 + base(covert) * 0.01,
            },
            mag_drills: MagDrills {
                load: per_level(drills, 0.6),
                unload: per_level(drills, 0.6),
                check: per_level(drills, 0.8),
                load_progression: elite(drills),
                instant_check: elite(drills),
            },
            levels: std::array::from_fn(|i| at(Skill::ALL[i])),
        }
    }

    /// A weapon class's buffs: the reload's speed (0.004 a level), the
    /// draw's (0.008), the recoil's suppression (`WeaponSkillRecoilBonus
    /// PerLevel`, 0.003), the ergonomics (0.002), a double action's penalty
    /// (0.01); at elite the stiff breath, as Aim Drills' elite has it for
    /// every class. `None`, a weapon of no class: only Aim Drills' elite.
    pub fn weapon(&self, class: Option<Skill>) -> WeaponBuffs {
        let Some(class) = class else { return WeaponBuffs { stiff_draw: self.aim_drills_elite, ..WeaponBuffs::default() } };
        let level = self.levels[class.index()];
        WeaponBuffs {
            reload_speed: 1.0 + per_level(level, 0.004),
            swap_speed: 1.0 + per_level(level, 0.008),
            recoil: per_level(level, 0.003),
            ergonomics: per_level(level, 0.002),
            stiff_draw: elite(level) || self.aim_drills_elite,
            mount_ergonomics: [per_level(level, 0.2), per_level(level, 0.5)],
            double_action_reduce: per_level(level, 0.01),
        }
    }

    /// The level of a skill as the buffs were made from it.
    pub fn level(&self, skill: Skill) -> u32 {
        self.levels[skill.index()]
    }
}

impl Default for Buffs {
    fn default() -> Self {
        SkillLevels::default().buffs()
    }
}

/// A buff's kind (`SkillManager.EBuffType`), which decides when the skill
/// page shows it and whether its ground is the elite one: always, only at
/// elite, both (one that elite changes), only below elite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BuffKind {
    Simple,
    Elite,
    Switching,
    Plebian,
}

/// How a buff's figure comes from the level (`FloatBuff`'s rules; a yes or
/// no for a `BooleanBuff`, on at elite).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rule {
    /// `PerLevel(f)`, and the figure elite puts on it, if any.
    Level(f32, Option<f32>),
    /// `Max(top)`, and elite's figure.
    Top(f32, Option<f32>),
    /// A `BooleanBuff`: on at elite.
    Flag,
    /// Covert Movement's speed: `1 + 0.01 a level`.
    CovertSpeed,
    /// Mag Drills' check accuracy: the level over 10, at most 2.
    CheckAccuracy,
    /// A weapon class's doubled mastering: 1, 2 at elite.
    Mastering,
}

/// One of a skill's buffs as the game's skill page shows it: its name
/// (`EBuffId`, which keys its icon and its words), its kind, whether it is
/// hidden from the player, and its rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BuffDef {
    pub id: &'static str,
    pub kind: BuffKind,
    pub hidden: bool,
    pub rule: Rule,
}

/// A buff's value at a level: a figure, or a yes or no.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuffValue {
    Figure(f32),
    Flag(bool),
}

impl BuffDef {
    pub(super) const fn of(id: &'static str, kind: BuffKind, rule: Rule) -> Self {
        Self { id, kind, hidden: false, rule }
    }

    pub(super) const fn hidden(id: &'static str, rule: Rule) -> Self {
        Self { id, kind: BuffKind::Simple, hidden: true, rule }
    }

    /// Its value at a level (`Skill.UpdateRules`: the rule held at 50, the
    /// elite's after it above 50).
    pub fn value(&self, level: u32) -> BuffValue {
        let at_elite = elite(level);
        match self.rule {
            Rule::Level(f, top) => BuffValue::Figure(top.filter(|_| at_elite).unwrap_or(per_level(level, f))),
            Rule::Top(f, top) => BuffValue::Figure(top.filter(|_| at_elite).unwrap_or(to_top(level, f))),
            Rule::Flag => BuffValue::Flag(at_elite),
            Rule::CovertSpeed => BuffValue::Figure(1.0 + base(level) * 0.01),
            Rule::CheckAccuracy => BuffValue::Figure((base(level) / 10.0).clamp(0.0, 2.0)),
            Rule::Mastering => BuffValue::Figure(if at_elite { 2.0 } else { 1.0 }),
        }
    }

    /// Whether the skill page shows it at a level (`BuffThumb.
    /// UpdateVisibility`): not an elite buff below elite, not a plebian one
    /// at it, none at level 0, never a hidden one.
    pub fn shown(&self, level: u32) -> bool {
        let at_elite = elite(level);
        !((self.kind == BuffKind::Elite && !at_elite) || (self.kind == BuffKind::Plebian && at_elite) || level == 0 || self.hidden)
    }

    /// Whether its ground is the elite one and its line says ELITE
    /// (`BuffIcon.UpdateBuff`: not a simple one, at elite).
    pub fn elite_look(&self, level: u32) -> bool {
        self.kind != BuffKind::Simple && elite(level)
    }
}
