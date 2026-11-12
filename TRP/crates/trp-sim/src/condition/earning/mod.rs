//! How the character's skills and weapon mastering are earned.
//! After `docs/spec/skills.md`, "How a skill is earned" (`SkillManager`,
//! `Skill.OnTrigger`, `UseEffectiveness`, `CalculateExpOnFirstLevels`,
//! `Mastering`): what each of the player's actions gives, by the install's
//! figures (`SkillsSettings`), the share the fed skills take, the session's
//! fatigue, the cheaper first nine levels, and the level each award leaves.
//! The app raises the actions where its systems see them happen (`trp-app`'s
//! `experience`), to cover every source of experience the game has.
//!
//! **Simple XP gain** (TRP's own option: the game's diminishing returns
//! off, every award at its whole worth):
//! an award skips the session's fatigue, whole at every step; and
//! Metabolism earns once for each food or drink used, not by the point
//! (`metabolism`), since
//! its eight points a point gained are the game's to be held down by that
//! fatigue alone. Nothing else of the game's rule changes.

use serde_json::Value;

use crate::condition::health::Event;
use crate::condition::skills::{Skill, SkillLevels, ELITE, MOST_PROGRESS, POINTS_PER_LEVEL};

/// Strength's factor for a blow on a character (`FistfightAction.Factor(0.2f)`
/// in `SkillManager`: the client's own; the install's
/// `SkillsSettings.Strength.FistfightAction` is 0.2 too, and read by nothing).
const FISTFIGHT: f32 = 0.2;

/// An action of the player's that a skill listens to (`SkillManager`'s
/// actions), with what the game passes with it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Act {
    /// A walk (the `Run` state) or a sprint ended on the ground
    /// (`MovementContextOnStateChanged`): its metres by the pedometer, how
    /// far over the base overweight limit the character was (0 to 1,
    /// `Physical.Overweight`; 0 at or under it), the chronic fatigue's
    /// strength, and whether the walk was quiet (`CovertNoiseLevel` 0).
    Moved { metres: f32, sprint: bool, overweight: f32, fatigue: f32, quiet: bool },
    /// A grenade thrown (`ThrowGrenade`): Strength's and Throwing's one
    /// shared action, the later factor the one in force (0.25).
    Thrown,
    /// Damage taken that is not self-inflicted, by its amount
    /// (`OnHealthApplyDamage`).
    DamageTaken(f32),
    /// A bleeding begun (Vitality), an intoxication begun (Immunity), a
    /// pain begun off painkillers (Stress Resistance): `HealthNegativeEffect`.
    Bleeding,
    Intoxication,
    Pain,
    /// The seconds at low health, told as it ends (`LowHPDuration`).
    LowHealth(f32),
    /// Energy and hydration gained, by the change (`EnergyRecoveryRate`,
    /// `HydrationRecoveryRate`).
    Energy(f32),
    Hydration(f32),
    /// The seconds a stimulant's ill effect lasted, told as the last ends
    /// (`StimulatorNegativeBuff`).
    StimulantIll(f32),
    /// A thing taken into the inventory for the first time this run, not
    /// ammunition or money (`UniqueLoot`).
    UniqueLoot,
    /// A thing found by a search, not of a stack (Attention: `FindActionTrue`
    /// on a body, `FindActionFalse` elsewhere; on a body also Search's
    /// `FindAction`).
    Found { on_body: bool },
    /// A search run to its end (`SearchAction`).
    Searched,
    /// A weapon of a class reloaded, a round chambered by its hands, and a
    /// hit with it on a living enemy (`WeaponReloadAction`,
    /// `WeaponChamberAction`, `WeaponShotAction`; the last two at the class's
    /// elite: `WeaponDoubleMastering`); the hit gives Aim Drills too where the
    /// sights came up within the fast-aim timer's two seconds.
    Reload(Skill),
    Chamber(Skill),
    Hit { class: Skill, fast_aim: bool },
    /// A malfunction put right (`SkillPointsPerMalfFix`).
    Malfunction,
    /// A body part restored by surgery (`SurgeryAction`).
    Surgery,
    /// A round loaded into a magazine, one taken out, a magazine checked the
    /// first time (`RaidLoadedAmmoAction`, `RaidUnloadedAmmoAction`,
    /// `MagazineCheckAction`).
    RoundLoaded,
    RoundUnloaded,
    MagazineChecked,
    /// A melee weapon's blow on a character's collider (`FistfightAction`,
    /// completed by `BaseKnifeController.ProcessHit` on any body part's or
    /// plate's collider): Strength's, by the client's own factor 0.2.
    Fistfight,
    /// Durability a worn armour lost, by its class (`LightVests`,
    /// `HeavyVests`: their counters' `points / divisor`... the client's own:
    /// 0.05 and 0.025 a point).
    Armor { heavy: bool, amount: f32 },
}

/// The install's figures an award goes by (`SkillsSettings`, the config's
/// `SkillFresh…`, `SkillFatigue…`).
#[derive(Debug, Clone, PartialEq)]
pub struct EarnRules {
    /// `SkillsSettings.SkillProgressRate` (0.4) and `WeaponSkillProgressRate`
    /// (1): every factor times the one or the other.
    pub progress_rate: f32,
    pub weapon_rate: f32,
    fresh_effectiveness: f32,
    fresh_points: i32,
    before_fatigue: i32,
    fatigue_per_point: f32,
    min_effectiveness: f32,
    fatigue_reset: f64,
    /// `SkillsSettings`, from which each action's figure is read.
    settings: Value,
}

impl EarnRules {
    pub fn read(config: &Value) -> Self {
        let settings = config["SkillsSettings"].clone();
        let f = |v: &Value, otherwise: f64| v.as_f64().unwrap_or(otherwise) as f32;
        Self {
            progress_rate: f(&settings["SkillProgressRate"], 0.4),
            weapon_rate: f(&settings["WeaponSkillProgressRate"], 1.0),
            fresh_effectiveness: f(&config["SkillFreshEffectiveness"], 1.3),
            fresh_points: config["SkillFreshPoints"].as_i64().unwrap_or(1) as i32,
            before_fatigue: config["SkillPointsBeforeFatigue"].as_i64().unwrap_or(1) as i32,
            fatigue_per_point: f(&config["SkillFatiguePerPoint"], 0.6),
            min_effectiveness: f(&config["SkillMinEffectiveness"], 0.0001),
            fatigue_reset: config["SkillFatigueReset"].as_f64().unwrap_or(200.0),
            settings,
        }
    }

    /// A figure of a skill's settings (`SkillsSettings.<skill>.<name>`),
    /// nought where the install has none.
    pub fn figure(&self, skill: &str, name: &str) -> f32 {
        self.settings[skill][name].as_f64().unwrap_or(0.0) as f32
    }

    /// The effectiveness after so many whole points earned in the session
    /// (`SkillManager.GetEffectiveness`): fresh below the fresh points, 1
    /// for the points before fatigue, then the fatigue's power, held over
    /// the least.
    pub fn effectiveness(&self, earned: i32) -> f32 {
        let tired_from = self.fresh_points + self.before_fatigue;
        if earned < self.fresh_points {
            self.fresh_effectiveness
        } else if earned < tired_from {
            1.0
        } else {
            self.min_effectiveness.max(self.fatigue_per_point.powi(1 + earned - tired_from))
        }
    }

    /// The figures a weapon class earns by, as the client takes them
    /// (`WeaponsSettingExtension`, `CreateCombat`): the SMGs and the LMGs the
    /// assault rifles', the launchers and the melee weapons the pistols';
    /// reload, shot (a hit), chamber.
    fn weapon_figures(&self, class: Skill) -> [f32; 3] {
        let name = match class {
            Skill::Smg | Skill::Lmg => "Assault",
            Skill::Launcher | Skill::Melee => "Pistol",
            other => other.name(),
        };
        ["WeaponReloadAction", "WeaponShotAction", "WeaponChamberAction"].map(|action| self.figure(name, action))
    }
}

/// A skill's part of the session (`Skill`'s `_pointsEarned`,
/// `_effectiveness`, `_fatigueTimer`; `LastCall`): what this run has
/// earned it, after fatigue; how much an award is worth now; until when
/// the fatigue lasts (none until an award leaves it at 1 or less); when it
/// last earned (the SKILLS tab's "Most recent").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkillSession {
    pub earned: f32,
    pub effectiveness: f32,
    fatigue_until: f64,
    pub last: Option<f64>,
}

/// Every skill's session, begun with the run (`GetEffectiveness(0)`).
/// `PROVISIONAL`: the game keeps the session's points in the profile
/// between raids (`PointsEarnedDuringSession`); the app begins each run
/// fresh, as a raid's start whose server cleared them.
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    skills: Vec<SkillSession>,
}

impl Session {
    pub fn new(rules: &EarnRules) -> Self {
        let fresh = SkillSession { earned: 0.0, effectiveness: rules.effectiveness(0), fatigue_until: f64::MAX, last: None };
        Self { skills: vec![fresh; Skill::ALL.len()] }
    }

    pub fn of(&self, skill: Skill) -> &SkillSession {
        &self.skills[skill.index()]
    }

    /// An award through the skill's fatigue (`Skill.UseEffectiveness`): the
    /// fatigue first over where its time has passed (the effectiveness 1,
    /// the session's points at most the fresh ones); then the award a point
    /// at a time, each step at the effectiveness of the moment, which is
    /// worked out again whenever the session's whole points go up; the
    /// fatigue's time set again where it leaves the effectiveness at 1 or
    /// less. What the award is worth.
    fn use_effectiveness(&mut self, skill: Skill, input: f32, now: f64, rules: &EarnRules) -> f32 {
        let s = &mut self.skills[skill.index()];
        if now > s.fatigue_until {
            s.effectiveness = 1.0;
            s.earned = s.earned.min(rules.fresh_points as f32);
        }
        let mut left = input;
        let mut worth = 0.0;
        for _ in 0..input.ceil().max(0.0) as i64 {
            let before = s.earned;
            let step = left.min(1.0);
            left -= step;
            let gain = step * s.effectiveness;
            worth += gain;
            s.earned += gain;
            if s.earned as i32 > before as i32 {
                s.effectiveness = rules.effectiveness(s.earned as i32);
            }
        }
        if s.effectiveness <= 1.0 {
            s.fatigue_until = now + rules.fatigue_reset;
        }
        worth
    }
}

/// A level a skill's award took it to (`SkillLevelChanged`): one for an
/// award that passes several, at the last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelUp {
    pub skill: Skill,
    pub level: u32,
}

/// The progress so many points make below level 9
/// (`Skill.CalculateExpOnFirstLevels`): the points already into the level
/// worked back from its progress; each level below 9 taking (level + 1) ×
/// 10 points, each worth 10 / (level + 1) of progress; what is left past
/// level 9 at one a point.
pub fn first_levels(current: f32, points: f32) -> f32 {
    let mut left = points;
    let mut worth = 0.0;
    let mut level = (current / POINTS_PER_LEVEL) as i32;
    let mut into = current % POINTS_PER_LEVEL * (level + 1) as f32 / 10.0;
    while left > 0.0 && level < 9 {
        let need = ((level + 1) * 10) as f32 - into;
        worth += 10.0 / (level + 1) as f32 * left.min(need);
        left -= need;
        into = 0.0;
        level += 1;
    }
    if left > 0.0 {
        worth += left;
    }
    worth
}

/// The skills a skill's awards feed, and their share (`SkillProgress` of
/// the taker's settings, times the progress rate): Health from Strength,
/// Endurance and Vitality; Charisma from Intellect, Attention and
/// Perception (the client's own figures, 0.5; the install's
/// `DependentSkillRatios` are read by nothing).
fn fed_by(giver: Skill, rules: &EarnRules) -> Option<(Skill, f32)> {
    match giver {
        Skill::Strength | Skill::Endurance | Skill::Vitality => Some((Skill::Health, rules.figure("Health", "SkillProgress"))),
        Skill::Intellect => Some((Skill::Charisma, rules.figure("Charisma", "SkillProgressInt"))),
        Skill::Attention => Some((Skill::Charisma, rules.figure("Charisma", "SkillProgressAtn"))),
        Skill::Perception => Some((Skill::Charisma, rules.figure("Charisma", "SkillProgressPer"))),
        _ => None,
    }
}

/// What a skill takes of an award (`Skill.OnTrigger`), the award its
/// action's amount times its factor: nothing at elite (unsubscribed);
/// else the skills it feeds take their share first, of the award before
/// its fatigue; then the fatigue (none with `simple`), the cheaper first
/// levels, and the progress held at 5,100. A level reached is pushed on
/// `ups`.
pub fn award(levels: &mut SkillLevels, session: &mut Session, rules: &EarnRules, skill: Skill, amount: f32, now: f64, simple: bool, ups: &mut Vec<LevelUp>) {
    if amount <= 0.0 || levels.level(skill) >= ELITE || skill.locked() {
        return;
    }
    if let Some((taker, share)) = fed_by(skill, rules) {
        award(levels, session, rules, taker, amount * share * rules.progress_rate, now, simple, ups);
    }
    let worth = if simple { amount } else { session.use_effectiveness(skill, amount, now, rules) };
    let before = levels.level(skill);
    let current = levels.progress(skill);
    let gain = if before < 9 { first_levels(current, worth) } else { worth };
    levels.set_progress(skill, (current + gain).min(MOST_PROGRESS));
    session.skills[skill.index()].last = Some(now);
    let after = levels.level(skill);
    if after != before {
        ups.push(LevelUp { skill, level: after });
    }
}

/// The skills an action gives to and how much, each its amount times its
/// factor (`SkillAction.Factor`), before any skill takes it.
pub fn awards(act: Act, rules: &EarnRules, class_elite: impl Fn(Skill) -> bool) -> Vec<(Skill, f32)> {
    let p = rules.progress_rate;
    let fig = |skill: &str, name: &str| rules.figure(skill, name);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t.clamp(0.0, 1.0);
    match act {
        Act::Moved { metres, sprint, overweight, fatigue, quiet } => {
            let mut out = Vec::new();
            if overweight <= 0.0 {
                let per = if sprint { fig("Endurance", "SprintAction") } else { fig("Endurance", "MovementAction") };
                out.push((Skill::Endurance, metres * per * (1.0 + fig("Endurance", "GainPerFatigueStack") * fatigue) * p));
            } else {
                let per = if sprint { lerp(fig("Strength", "SprintActionMin"), fig("Strength", "SprintActionMax"), overweight) } else { lerp(fig("Strength", "MovementActionMin"), fig("Strength", "MovementActionMax"), overweight) };
                out.push((Skill::Strength, metres * per * p));
            }
            if !sprint && quiet {
                out.push((Skill::CovertMovement, metres * fig("CovertMovement", "MovementAction") * p));
            }
            out
        }
        Act::Thrown => {
            let each = fig("Throwing", "ThrowAction") * p;
            vec![(Skill::Strength, each), (Skill::Throwing, each)]
        }
        // `FistfightAction.Factor(0.2f)`: the client's figure, not the
        // install's `Strength.FistfightAction`, which nothing reads.
        Act::Fistfight => vec![(Skill::Strength, FISTFIGHT * p)],
        Act::DamageTaken(amount) => vec![(Skill::Vitality, amount * fig("Vitality", "DamageTakenAction") * p)],
        Act::Bleeding => vec![(Skill::Vitality, fig("Vitality", "HealthNegativeEffect") * p)],
        Act::Intoxication => vec![(Skill::Immunity, fig("Immunity", "HealthNegativeEffect") * p)],
        Act::Pain => vec![(Skill::StressResistance, fig("StressResistance", "HealthNegativeEffect") * p)],
        Act::LowHealth(seconds) => vec![(Skill::StressResistance, seconds * fig("StressResistance", "LowHPDuration") * p)],
        Act::Energy(amount) => vec![(Skill::Metabolism, amount * fig("Metabolism", "EnergyRecoveryRate") * p)],
        Act::Hydration(amount) => vec![(Skill::Metabolism, amount * fig("Metabolism", "HydrationRecoveryRate") * p)],
        Act::StimulantIll(seconds) => vec![(Skill::Immunity, seconds * fig("Immunity", "StimulatorNegativeBuff") * p)],
        Act::UniqueLoot => vec![(Skill::Perception, fig("Perception", "UniqueLoot") * p)],
        Act::Found { on_body: true } => vec![(Skill::Attention, fig("Attention", "FindActionTrue") * p), (Skill::Search, fig("Search", "FindAction") * p)],
        Act::Found { on_body: false } => vec![(Skill::Attention, fig("Attention", "FindActionFalse") * p)],
        Act::Searched => vec![(Skill::Search, fig("Search", "SearchAction") * p)],
        Act::Reload(class) => vec![(class, rules.weapon_figures(class)[0] * rules.weapon_rate)],
        Act::Chamber(class) => vec![(class, rules.weapon_figures(class)[2] * rules.weapon_rate)],
        Act::Hit { class, fast_aim } => {
            let times = if class_elite(class) { 2.0 } else { 1.0 };
            let mut out = vec![(class, times * rules.weapon_figures(class)[1] * rules.weapon_rate)];
            if fast_aim {
                out.push((Skill::AimDrills, times * fig("AimDrills", "WeaponShotAction") * p));
            }
            out
        }
        Act::Malfunction => vec![(Skill::TroubleShooting, fig("TroubleShooting", "SkillPointsPerMalfFix") * p)],
        Act::Surgery => vec![(Skill::Surgery, fig("Surgery", "SurgeryAction") * p)],
        Act::RoundLoaded => vec![(Skill::MagDrills, fig("MagDrills", "RaidLoadedAmmoAction") * p)],
        Act::RoundUnloaded => vec![(Skill::MagDrills, fig("MagDrills", "RaidUnloadedAmmoAction") * p)],
        Act::MagazineChecked => vec![(Skill::MagDrills, fig("MagDrills", "MagazineCheckAction") * p)],
        Act::Armor { heavy: false, amount } => vec![(Skill::LightVests, amount * 0.05 * p)],
        Act::Armor { heavy: true, amount } => vec![(Skill::HeavyVests, amount * 0.025 * p)],
    }
}

/// Metabolism's actions from what the health told. By the game's rule in a
/// raid each gain of energy or hydration is an action by its amount,
/// whatever made it (`Player.OnHealthEnergyChanged`,
/// `OnHealthHydrationChanged`: `Complete(diff, diff)`), a food's every loop
/// and a stimulant's every tick. With simple XP gain an item used is one
/// action of each that it raised, of amount 1 (`Event::Fed`), as the game's
/// controller out of a raid passes it (`OfflineHealthController`'s
/// `MedEffect`: `Complete(diff)`, the amount left at its default, the
/// filter the change), and a gain by itself (a rate's) gives nothing.
pub fn metabolism(event: &Event, simple: bool) -> Vec<Act> {
    match (event, simple) {
        (Event::EnergyGained(amount), false) => vec![Act::Energy(*amount)],
        (Event::HydrationGained(amount), false) => vec![Act::Hydration(*amount)],
        (Event::Fed { energy, hydration }, true) => {
            [(*energy > 0.0).then_some(Act::Energy(1.0)), (*hydration > 0.0).then_some(Act::Hydration(1.0))].into_iter().flatten().collect()
        }
        _ => Vec::new(),
    }
}

/// An action taken by every skill that listens to it. `class_elite` is
/// whether a weapon class is elite as the buffs have it, its summary level
/// with a stimulator's boosts over 50 (`WeaponDoubleMastering`'s elite rule,
/// `Skill.UpdateRules` by `SummaryLevel`): a hit then counts twice.
pub fn earn(levels: &mut SkillLevels, session: &mut Session, rules: &EarnRules, act: Act, now: f64, simple: bool, class_elite: impl Fn(Skill) -> bool) -> Vec<LevelUp> {
    let mut ups = Vec::new();
    let given = awards(act, rules, class_elite);
    for (skill, amount) in given {
        award(levels, session, rules, skill, amount, now, simple, &mut ups);
    }
    ups
}

/// A hit's mastering points (`ExecuteShotSkill`): 1, or 2 at the weapon's
/// class's elite (`WeaponDoubleMastering`), times the weapon rate; no
/// fatigue, nothing cheaper, held at 5,100 (`BaseSkill.OnTrigger`).
pub fn mastering_points(rules: &EarnRules, class_elite: bool) -> f32 {
    (if class_elite { 2.0 } else { 1.0 }) * rules.weapon_rate
}

#[cfg(test)]
mod tests;
