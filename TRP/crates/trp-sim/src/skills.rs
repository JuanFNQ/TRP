//! Tarkov's character skills (`docs/spec/skills.md`): each skill's progress
//! (a level a hundred points, 0 to 51, the last elite), and what the levels
//! make of the buffs the rest of the simulation reads, by the game's own
//! rules (`SkillManager`, `Skill.UpdateRules`). The levels are set by a
//! host (the app's options tab, or a host game at run time); nothing here
//! earns points (the user, 2026-10-22 and 2026-10-23: "never earning").
//!
//! Every figure is 0, or what leaves its system as it was, at level 0: a
//! character with no levels is the untrained one the simulation always had.

/// The progress a level takes (`SkillExpPerLevel`), the most a skill holds
/// (`BaseSkill.ClampCurrent`), and the elite level.
pub const POINTS_PER_LEVEL: f32 = 100.0;
pub const MOST_PROGRESS: f32 = 5100.0;
pub const ELITE: u32 = 51;

/// The game's four kinds of skill, as its skill page groups them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Class {
    Physical,
    Mental,
    Combat,
    Practical,
}

impl Class {
    pub const ALL: [Self; 4] = [Self::Physical, Self::Mental, Self::Combat, Self::Practical];

    /// The game's name for it (`ESkillClass`).
    pub fn name(self) -> &'static str {
        match self {
            Self::Physical => "Physical",
            Self::Mental => "Mental",
            Self::Combat => "Combat",
            Self::Practical => "Practical",
        }
    }
}

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
fn base(level: u32) -> f32 {
    level.min(50) as f32
}

/// `FloatBuff.PerLevel(f)`: the level times `f`.
fn per_level(level: u32, f: f32) -> f32 {
    base(level) * f
}

/// `FloatBuff.Max(top)`: the level times `top / 50`, worked as the game
/// works it (the factor divided first).
fn to_top(level: u32, top: f32) -> f32 {
    let factor = top / 50.0;
    base(level) * factor
}

/// Above 50, where the elite rules run.
fn elite(level: u32) -> bool {
    level > 50
}

/// What a skill's level makes of a buff that elite puts a figure on:
/// `rule` below elite, `top` at it.
fn or_elite(level: u32, rule: f32, top: f32) -> f32 {
    if elite(level) { top } else { rule }
}

/// The character's skills: each one's progress, 0 to 5,100.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillLevels {
    progress: [f32; 43],
}

impl Default for SkillLevels {
    fn default() -> Self {
        Self { progress: [0.0; 43] }
    }
}

impl SkillLevels {
    pub fn progress(&self, skill: Skill) -> f32 {
        self.progress[skill.index()]
    }

    /// A skill's progress, held between 0 and 5,100 (`ClampCurrent`). A
    /// fraction of a level is kept: it is what was earned toward the next.
    pub fn set_progress(&mut self, skill: Skill, progress: f32) {
        self.progress[skill.index()] = if progress.is_finite() { progress.clamp(0.0, MOST_PROGRESS) } else { 0.0 };
    }

    /// The level, whole (`BaseSkill.Level`), 0 to 51.
    pub fn level(&self, skill: Skill) -> u32 {
        ((self.progress(skill) / POINTS_PER_LEVEL) as u32).min(ELITE)
    }

    /// Sets a skill to a whole level, 0 to 51 (`Skill.SetLevel`).
    pub fn set_level(&mut self, skill: Skill, level: u32) {
        self.set_progress(skill, level.min(ELITE) as f32 * POINTS_PER_LEVEL);
    }

    /// A level by the game's name of the skill, 0 for a name it has not.
    pub fn level_of(&self, name: &str) -> u32 {
        Skill::of_name(name).map_or(0, |skill| self.level(skill))
    }

    /// Every skill at level 0.
    pub fn untrained(&self) -> bool {
        self.progress.iter().all(|p| *p == 0.0)
    }

    /// What the levels make of every buff.
    pub fn buffs(&self) -> Buffs {
        Buffs::of(self)
    }

    /// A skill's summary level with stimulators' buffs at work on it
    /// (`BaseSkill.SummaryLevel`, `Skill.Buff`; health spec, "Buffs"): its
    /// whole level and each buff's value, each held to what takes the level
    /// to between nought and 60 (the level rounded up) and rounded, at most
    /// 60 where they add up above nought and 51 otherwise. A buff can so make
    /// a skill elite while it lasts.
    pub fn summary(&self, skill: Skill, boosts: &[(Skill, f32)]) -> u32 {
        let level = self.level(skill) as i32;
        let up = (self.progress(skill) / POINTS_PER_LEVEL).ceil() as i32;
        let buff: i32 = boosts.iter().filter(|(s, _)| *s == skill).map(|(_, v)| v.clamp(-up as f32, (60 - up) as f32).round() as i32).sum();
        let cap = if buff > 0 { 60 } else { ELITE as i32 };
        (level + buff).clamp(0, cap) as u32
    }

    /// What the summary levels make of every buff.
    pub fn buffs_with(&self, boosts: &[(Skill, f32)]) -> Buffs {
        Buffs::of_fn(|skill| self.summary(skill, boosts))
    }
}

/// Covert Movement's buffs (`MovementContext.CovertMovementVolume`,
/// `CovertMovementVolumeBySpeed`, `CovertEquipmentNoise`,
/// `UpdateCovertEfficiency`; footsteps spec).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Covert {
    /// A step on a soft surface quieter by it, times how quietly one walks
    /// (`CovertMovementSoundVolume`, 0.012 a level).
    pub soft: f32,
    /// The same on any other (`CovertMovementLoud`, 0.006 a level, 0.6 at
    /// elite).
    pub loud: f32,
    /// The kit's noise less it (`CovertMovementEquipment`, 0.012 a level).
    pub equipment: f32,
    /// How slowly one must walk to walk quietly (`CovertMovementSpeed`,
    /// 1 + 0.01 a level).
    pub speed: f32,
}

impl Default for Covert {
    fn default() -> Self {
        Self { soft: 0.0, loud: 0.0, equipment: 0.0, speed: 1.0 }
    }
}

impl Covert {
    /// How quietly one walks, 0 to 1 (`UpdateCovertEfficiency`): in full
    /// below `0.4 × speed − 0.2` of the top speed, not at all above
    /// `0.4 × speed`; none with a leg damaged, less by the walk's
    /// overweight.
    pub fn efficiency(&self, speed_share: f32, leg_damaged: bool, walk_overweight: f32) -> f32 {
        if leg_damaged {
            return 0.0;
        }
        let top = 0.4 * self.speed;
        crate::math::inverse_lerp(top, top - 0.2, speed_share) * (1.0 - walk_overweight)
    }

    /// A step's factor (`CovertMovementVolumeBySpeed`): 1 untrained.
    pub fn step_volume(&self, soft_surface: bool, efficiency: f32) -> f32 {
        1.0 - if soft_surface { self.soft } else { self.loud } * efficiency
    }

    /// The other sounds' factor (`CovertMovementVolume`): 1 untrained.
    pub fn volume(&self, soft_surface: bool) -> f32 {
        1.0 - if soft_surface { self.soft } else { self.loud }
    }
}

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
}

impl Default for WeaponBuffs {
    fn default() -> Self {
        Self { reload_speed: 1.0, swap_speed: 1.0, recoil: 0.0, ergonomics: 0.0, stiff_draw: false, mount_ergonomics: [0.0; 2] }
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
    levels: [u32; 43],
}

impl Buffs {
    fn of(levels: &SkillLevels) -> Self {
        Self::of_fn(|skill| levels.level(skill))
    }

    /// The buffs from each skill's level as `at` gives it: the plain level,
    /// or the summary level with stimulators' buffs (`SkillLevels::summary`).
    fn of_fn(at: impl Fn(Skill) -> u32) -> Self {
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
    /// PerLevel`, 0.003), the ergonomics (0.002); at elite the stiff breath,
    /// as Aim Drills' elite has it for every class. `None`, a weapon of no
    /// class: only Aim Drills' elite.
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
    const fn of(id: &'static str, kind: BuffKind, rule: Rule) -> Self {
        Self { id, kind, hidden: false, rule }
    }

    const fn hidden(id: &'static str, rule: Rule) -> Self {
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

/// Each skill's buffs in the order its constructor gives them
/// (`SkillManager.method_2` to `method_4`, `CreateCombat`, `GetWeaponBuffs`),
/// each field's kind and visibility as declared; the figures the code has
/// from the install's settings are the install's (`skills.md`).
pub fn buffs_of(skill: Skill) -> &'static [BuffDef] {
    use BuffKind::*;
    use Rule::*;
    const WEAPON: [BuffDef; 9] = [
        BuffDef::of("WeaponReloadBuff", Simple, Level(0.004, None)),
        BuffDef::of("WeaponSwapBuff", Simple, Level(0.008, None)),
        BuffDef::of("WeaponRecoilBuff", Simple, Level(0.003, None)),
        BuffDef::of("WeaponDoubleMastering", Elite, Mastering),
        BuffDef::hidden("WeaponDoubleActionRecoilReduceBuff", Level(0.01, None)),
        BuffDef::of("WeaponErgonomicsBuff", Simple, Level(0.002, None)),
        BuffDef::of("DrawElite", Elite, Flag),
        BuffDef::hidden("MountingErgonomicsGainPerLevel", Level(0.2, None)),
        BuffDef::hidden("BipodErgonomicsGainPerLevel", Level(0.5, None)),
    ];
    match skill {
        Skill::Endurance => const { &[
            BuffDef::of("EnduranceBuffEnduranceInc", Switching, Top(0.5, Some(0.7))),
            BuffDef::of("EnduranceHands", Elite, Level(0.0, Some(0.5))),
            BuffDef::of("EnduranceBuffJumpCostRed", Simple, Top(0.3, None)),
            BuffDef::of("EnduranceBuffBreathTimeInc", Simple, Top(1.0, None)),
            BuffDef::of("EnduranceBuffRestorationTimeRed", Plebian, Top(0.5, Some(0.75))),
            BuffDef::of("EnduranceBreathElite", Elite, Level(0.0, Some(1.0))),
        ] },
        Skill::Strength => const { &[
            BuffDef::of("StrengthBuffJumpHeightInc", Simple, Top(0.2, None)),
            BuffDef::of("StrengthBuffLiftWeightInc", Simple, Top(0.3, None)),
            BuffDef::of("StrengthBuffMeleePowerInc", Simple, Top(0.3, None)),
            BuffDef::of("StrengthBuffSprintSpeedInc", Simple, Top(0.2, None)),
            BuffDef::of("StrengthBuffThrowDistanceInc", Simple, Top(0.2, None)),
            BuffDef::of("StrengthBuffAim", Simple, Top(0.2, None)),
            BuffDef::of("StrengthBuffElite", Elite, Flag),
            BuffDef::of("StrengthBuffMeleeCrits", Elite, Level(0.0, Some(0.5))),
        ] },
        Skill::Vitality => const { &[
            BuffDef::of("VitalityBuffBleedChanceRed", Simple, Level(0.012, None)),
            BuffDef::of("VitalityBuffSurviobilityInc", Simple, Level(0.004, None)),
            BuffDef::of("VitalityBuffRegeneration", Elite, Flag),
            BuffDef::of("VitalityBuffBleedStop", Elite, Flag),
        ] },
        Skill::Health => const { &[
            BuffDef::of("HealthBreakChanceRed", Simple, Level(0.012, None)),
            BuffDef::of("HealthEnergy", Simple, Level(0.006, None)),
            BuffDef::of("HealthHydration", Simple, Level(0.006, None)),
            BuffDef::of("HealthEliteAbsorbDamage", Elite, Flag),
        ] },
        Skill::StressResistance => const { &[
            BuffDef::of("StressPainChance", Simple, Level(0.01, None)),
            BuffDef::of("StressTremor", Simple, Level(0.012, None)),
            BuffDef::of("StressBerserk", Elite, Flag),
        ] },
        Skill::Metabolism => const { &[
            BuffDef::of("MetabolismEliteBuffNoDyhydration", Elite, Flag),
            BuffDef::of("MetabolismRatioPlus", Simple, Level(0.01, None)),
            BuffDef::of("MetabolismMiscDebuffTime", Simple, Level(0.01, None)),
        ] },
        Skill::Immunity => const { &[
            BuffDef::of("ImmunityMiscEffects", Simple, Level(0.01, None)),
            BuffDef::of("ImmunityPoisonBuff", Simple, Level(0.01, None)),
            BuffDef::of("ImmunityPainKiller", Simple, Level(0.006, None)),
            BuffDef::of("ImmunityPoisonChance", Elite, Level(0.0, Some(0.9))),
            BuffDef::of("ImmunityMiscEffectsChance", Elite, Level(0.0, Some(0.9))),
        ] },
        Skill::Perception => const { &[BuffDef::of("PerceptionLootDot", Simple, Level(0.02, None)), BuffDef::of("PerceptionmEliteNoIdea", Elite, Flag)] },
        Skill::Intellect => const { &[
            BuffDef::of("IntellectEliteAmmoCounter", Elite, Flag),
            BuffDef::of("IntellectEliteContainerScope", Elite, Flag),
            BuffDef::of("IntellectEliteNaturalLearner", Elite, Flag),
            BuffDef::of("IntellectLearningSpeed", Simple, Level(0.02, None)),
            BuffDef::of("IntellectWeaponMaintance", Simple, Level(0.02, None)),
            BuffDef::of("IntellectRepairPointsCostReduction", Simple, Level(0.004, None)),
        ] },
        Skill::Attention => const { &[
            BuffDef::of("AttentionEliteExtraLootExp", Elite, Flag),
            BuffDef::of("AttentionEliteLuckySearch", Elite, Level(0.0, Some(0.5))),
            BuffDef::of("AttentionLootSpeed", Simple, Level(0.02, None)),
            BuffDef::of("AttentionRareLoot", Simple, Level(0.02, None)),
        ] },
        Skill::Charisma => const { &[
            BuffDef::of("CharismaDailyQuestsRerollDiscount", Simple, Level(0.001, None)),
            BuffDef::of("CharismaHealingDiscount", Simple, Level(0.001, None)),
            BuffDef::of("CharismaInsuranceDiscount", Simple, Level(0.001, None)),
            BuffDef::of("CharismaExfiltrationDiscount", Simple, Level(0.001, None)),
            BuffDef::of("CharismaScavCaseDiscount", Elite, Level(0.0, Some(0.1))),
            BuffDef::of("CharismaFenceRepPenaltyReduction", Elite, Level(0.0, Some(0.5))),
            BuffDef::of("CharismaAdditionalDailyQuests", Elite, Level(0.0, Some(1.0))),
        ] },
        Skill::Pistol | Skill::Revolver | Skill::Smg | Skill::Assault | Skill::Shotgun | Skill::Sniper | Skill::Lmg | Skill::Hmg | Skill::Launcher | Skill::AttachedLauncher | Skill::Melee | Skill::Dmr => &WEAPON,
        Skill::Throwing => const { &[
            BuffDef::of("ThrowingStrengthBuff", Simple, Level(0.005, None)),
            BuffDef::of("ThrowingEnergyExpenses", Simple, Level(0.01, None)),
            BuffDef::of("ThrowingWeaponsBuffElite", Elite, Flag),
        ] },
        Skill::AimDrills => const { &[
            BuffDef::of("DrawSpeed", Simple, Top(0.5, None)),
            BuffDef::of("DrawElite", Elite, Flag),
            BuffDef::of("DrawTremor", Elite, Flag),
            BuffDef::of("DrawSound", Simple, Top(0.5, None)),
        ] },
        Skill::TroubleShooting => const { &[
            BuffDef::of("TroubleFixing", Simple, Level(0.005, None)),
            BuffDef::of("TroubleFixingExamineMalfElite", Elite, Flag),
            BuffDef::of("TroubleFixingDurElite", Elite, Flag),
            BuffDef::of("TroubleFixingMagElite", Elite, Flag),
            BuffDef::of("TroubleFixingAmmoElite", Elite, Flag),
        ] },
        Skill::Surgery => const { &[BuffDef::of("SurgerySpeed", Switching, Top(0.2, Some(0.4))), BuffDef::of("SurgeryReducePenalty", Switching, Level(0.01, Some(1.0)))] },
        Skill::CovertMovement => const { &[
            BuffDef::of("CovertMovementSoundVolume", Simple, Level(0.012, None)),
            BuffDef::of("CovertMovementLoud", Plebian, Level(0.006, Some(0.6))),
            BuffDef::of("CovertMovementElite", Elite, Flag),
            BuffDef::of("CovertMovementSpeed", Simple, CovertSpeed),
            BuffDef::of("CovertMovementEquipment", Simple, Level(0.012, None)),
        ] },
        Skill::Search => const { &[BuffDef::of("SearchDouble", Elite, Flag), BuffDef::of("SearchSpeed", Plebian, Level(0.01, None))] },
        Skill::MagDrills => const { &[
            BuffDef::of("MagDrillsLoadSpeed", Simple, Level(0.6, None)),
            BuffDef::of("MagDrillsUnloadSpeed", Simple, Level(0.6, None)),
            BuffDef::of("MagDrillsInventoryCheckSpeed", Simple, Level(0.8, None)),
            BuffDef::of("MagDrillsInventoryCheckAccuracy", Simple, CheckAccuracy),
            BuffDef::of("MagDrillsInstantCheck", Elite, Flag),
            BuffDef::of("MagDrillsLoadProgression", Elite, Flag),
        ] },
        Skill::LightVests => const { &[
            BuffDef::of("LightVestMoveSpeedPenaltyReduction", Simple, Level(0.006, None)),
            BuffDef::of("LightVestMeleeWeaponDamageReduction", Simple, Level(0.006, None)),
            BuffDef::of("LightVestRepairDegradationReduction", Simple, Level(0.008, None)),
            BuffDef::of("LightVestBleedingProtection", Elite, Flag),
            BuffDef::of("LightVestDeteriorationChanceOnRepairReduce", Elite, Flag),
        ] },
        Skill::HeavyVests => const { &[
            BuffDef::of("HeavyVestMoveSpeedPenaltyReduction", Simple, Level(0.005, None)),
            BuffDef::of("HeavyVestBluntThroughputDamageReduction", Simple, Level(0.004, None)),
            BuffDef::of("HeavyVestRepairDegradationReduction", Simple, Level(0.01, None)),
            BuffDef::of("HeavyVestNoBodyDamageDeflectChance", Elite, Flag),
            BuffDef::of("HeavyVestDeteriorationChanceOnRepairReduce", Elite, Flag),
        ] },
        Skill::WeaponTreatment => const { &[
            BuffDef::of("WeaponDurabilityLossOnShotReduce", Simple, Level(0.005, None)),
            BuffDef::of("WearAmountRepairGunsReducePerLevel", Simple, Level(0.01, None)),
            BuffDef::of("WearChanceRepairGunsReduceEliteLevel", Elite, Flag),
        ] },
        Skill::Crafting => const { &[
            BuffDef::of("CraftingSingleTimeReduce", Simple, Level(0.0075, None)),
            BuffDef::of("CraftingContinueTimeReduce", Simple, Level(0.0075, None)),
            BuffDef::of("CraftingElite", Elite, Flag),
        ] },
        Skill::HideoutManagement => const { &[
            BuffDef::of("HideoutExtraSlots", Elite, Flag),
            BuffDef::of("HideoutZoneBonusBoost", Simple, Level(1.0, None)),
            BuffDef::of("HideoutResourceConsumption", Simple, Level(0.005, None)),
        ] },
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every listed skill with buffs has some, the locked none; what the
    /// page shows at 0, at 40 and at elite (Endurance's: the plebian
    /// restoration gone at elite, the two elites come).
    #[test]
    fn the_skill_page_s_buffs_are_the_game_s() {
        assert!(Skill::ALL.iter().all(|skill| skill.locked() == buffs_of(*skill).is_empty()));
        let shown = |skill: Skill, level: u32| buffs_of(skill).iter().filter(|buff| buff.shown(level)).map(|buff| buff.id).collect::<Vec<_>>();
        assert!(shown(Skill::Endurance, 0).is_empty());
        assert_eq!(shown(Skill::Endurance, 40), ["EnduranceBuffEnduranceInc", "EnduranceBuffJumpCostRed", "EnduranceBuffBreathTimeInc", "EnduranceBuffRestorationTimeRed"]);
        assert_eq!(shown(Skill::Endurance, 51), ["EnduranceBuffEnduranceInc", "EnduranceHands", "EnduranceBuffJumpCostRed", "EnduranceBuffBreathTimeInc", "EnduranceBreathElite"]);
        assert_eq!(buffs_of(Skill::Endurance)[0].value(51), BuffValue::Figure(0.7));
        let covert = buffs_of(Skill::CovertMovement);
        assert!(matches!(covert[0].value(40), BuffValue::Figure(v) if (v - 0.48).abs() < 1e-6));
        assert_eq!(shown(Skill::Assault, 10).len(), 4, "the hidden three and the two elites are not shown");
    }

    /// The table's figures are those `Buffs` puts on the systems.
    #[test]
    fn the_table_agrees_with_the_buffs() {
        let mut levels = SkillLevels::default();
        for (skill, level) in [(Skill::Endurance, 51), (Skill::Strength, 37), (Skill::CovertMovement, 51), (Skill::MagDrills, 23)] {
            levels.set_level(skill, level);
        }
        let b = levels.buffs();
        let figure = |skill: Skill, id: &str| match buffs_of(skill).iter().find(|buff| buff.id == id).expect(id).value(levels.level(skill)) {
            BuffValue::Figure(v) => v,
            BuffValue::Flag(on) => on as u8 as f32,
        };
        assert_eq!(figure(Skill::Endurance, "EnduranceBuffEnduranceInc"), b.endurance_capacity);
        assert_eq!(figure(Skill::Endurance, "EnduranceBuffRestorationTimeRed"), b.endurance_restoration);
        assert_eq!(figure(Skill::Strength, "StrengthBuffLiftWeightInc"), b.strength_lift);
        assert_eq!(figure(Skill::CovertMovement, "CovertMovementLoud"), b.covert.loud);
        assert_eq!(figure(Skill::CovertMovement, "CovertMovementSpeed"), b.covert.speed);
        assert_eq!(figure(Skill::MagDrills, "MagDrillsInventoryCheckSpeed"), b.mag_drills.check);
    }

    #[test]
    fn untrained_buffs_leave_everything_as_it_was() {
        let b = Buffs::default();
        assert_eq!((b.endurance_capacity, b.endurance_hands, b.endurance_jump, b.endurance_breath, b.endurance_restoration), (0.0, 0.0, 0.0, 0.0, 0.0));
        assert!(!b.endurance_breath_elite && !b.strength_elite && !b.throwing_elite && !b.search_double && !b.aim_drills_elite);
        assert_eq!((b.strength_level, b.strength_jump, b.strength_lift, b.strength_aim, b.aim_speed), (0, 0.0, 0.0, 0.0, 0.0));
        assert_eq!(b.covert, Covert::default());
        assert_eq!(b.covert.step_volume(true, 1.0), 1.0);
        assert_eq!(b.covert.volume(false), 1.0);
        assert_eq!(b.mag_drills, MagDrills::default());
        assert_eq!(b.weapon(Some(Skill::Assault)), WeaponBuffs::default());
        assert_eq!(b.weapon(None), WeaponBuffs::default());
    }

    #[test]
    fn a_level_is_its_progress_over_a_hundred_and_the_rules_hold_it_at_fifty() {
        let mut levels = SkillLevels::default();
        levels.set_progress(Skill::CovertMovement, 4099.9);
        assert_eq!(levels.level(Skill::CovertMovement), 40);
        levels.set_progress(Skill::Strength, 9000.0);
        assert_eq!((levels.progress(Skill::Strength), levels.level(Skill::Strength)), (5100.0, 51));
        let b = levels.buffs();
        // Covert Movement 40: 48% quieter on a soft surface walking quietly.
        assert!((b.covert.soft - 0.48).abs() < 1e-6, "{}", b.covert.soft);
        // Strength at elite: the level rules at 50, the elite on top.
        assert!((b.strength_lift - 0.3).abs() < 1e-6 && b.strength_elite && b.strength_level == 51);
        levels.set_level(Skill::Endurance, 51);
        let b = levels.buffs();
        assert_eq!((b.endurance_capacity, b.endurance_hands, b.endurance_restoration), (0.7, 0.5, 0.75));
        assert!((b.endurance_jump - 0.3).abs() < 1e-6 && b.endurance_breath_elite);
        levels.set_level(Skill::Assault, 10);
        let w = levels.buffs().weapon(Skill::of_weapon(&["AssaultCarbine".to_owned(), "Weapon".to_owned()]));
        assert!((w.reload_speed - 1.04).abs() < 1e-6 && (w.swap_speed - 1.08).abs() < 1e-6 && !w.stiff_draw);
        assert_eq!(Skill::of_weapon(&["GrenadeLauncher".to_owned(), "Weapon".to_owned()]), None);
    }

    #[test]
    fn names_are_the_games() {
        assert_eq!(Skill::of_name("StressResistance"), Some(Skill::StressResistance));
        assert_eq!(Skill::of_name("smg"), Some(Skill::Smg));
        assert!(Skill::ALL.iter().enumerate().all(|(i, s)| s.index() == i));
        assert_eq!(Skill::ALL.iter().filter(|s| s.locked()).count(), 8);
    }
}
