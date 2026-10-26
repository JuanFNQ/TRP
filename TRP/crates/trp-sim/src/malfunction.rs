//! A weapon's wear, its malfunctions and its heat (`docs/spec/malfunctions.md`):
//! the game's `Weapon.MalfunctionState`, `Weapon.OnShot`, `GetCurrentOverheat`
//! and the firearm controller's roll (`GetTotalMalfunctionChance`,
//! `GetMalfunctionSources`, `GetSpecificMalfunctionVariants`), as pure rules.
//! The weapon's logic (`crate::Firearm`) asks them at each shot and keeps
//! what they leave; the host keeps the weapon's wear and its state with the
//! weapon's item between draws.

use anyhow::{Context, Result};
use serde_json::Value as Json;

use crate::math::lerp;
use crate::unity_random::UnityRandom;

/// A malfunction (`Weapon.EMalfunctionState`; the animator's whole numbers).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Malf {
    #[default]
    None,
    Misfire,
    Jam,
    HardSlide,
    SoftSlide,
    Feed,
}

impl Malf {
    /// The game's number for it, which the animator's `Malfunction` and
    /// `MalfType` take.
    pub fn number(self) -> i32 {
        match self {
            Malf::None => 0,
            Malf::Misfire => 1,
            Malf::Jam => 2,
            Malf::HardSlide => 3,
            Malf::SoftSlide => 4,
            Malf::Feed => 5,
        }
    }

    /// The key of its name in the game's texts (`EMalfunctionState/...`:
    /// "Misfire", "Failure to eject", "Bolt jammed" for either slide,
    /// "Failure to feed").
    pub fn text_key(self) -> &'static str {
        match self {
            Malf::None => "EMalfunctionState/None",
            Malf::Misfire => "EMalfunctionState/Misfire",
            Malf::Jam => "EMalfunctionState/Jam",
            Malf::HardSlide => "EMalfunctionState/HardSlide",
            Malf::SoftSlide => "EMalfunctionState/SoftSlide",
            Malf::Feed => "EMalfunctionState/Feed",
        }
    }
}

/// Where a malfunction came from (`Weapon.EMalfunctionSource`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Source {
    #[default]
    Durability,
    Ammo,
    Magazine,
    Overheat,
}

/// The game's settings (globals' `Malfunction` and `Overheat`, and the two
/// skills' figures that touch them).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub ammo_feed_wt: f32,
    pub ammo_jam_wt: f32,
    pub ammo_misfire_wt: f32,
    pub ammo_malf_chance_mult: f32,
    pub magazine_malf_chance_mult: f32,
    pub dur_feed_wt: f32,
    pub dur_misfire_wt: f32,
    pub dur_jam_wt: f32,
    pub dur_soft_slide_wt: f32,
    pub dur_hard_slide: [f32; 2],
    /// The durability's span in which only the heat may malfunction
    /// (`DurRangeToIgnoreMalfs`, 93 to 100).
    pub dur_ignore: [f32; 2],
    pub overheat_feed_wt: f32,
    pub overheat_jam_wt: f32,
    pub overheat_soft_slide_wt: f32,
    pub overheat_hard_slide: [f32; 2],
    pub repair_one_hand_broken: f32,
    pub repair_two_hands_broken: f32,
    pub repair_hard_slide: f32,
    /// How many reminders within 2 s before the glow shows (`ShowGlowAttemptsCount`).
    pub glow_attempts: u32,
    // `Overheat`.
    pub min_overheat: f32,
    pub max_overheat: f32,
    pub problems_start: f32,
    pub mod_heat_factor: f32,
    pub mod_cool_factor: f32,
    pub cool_coef: f32,
    pub min_malf_chance: f32,
    pub max_malf_chance: f32,
    pub dur_reduce: [f32; 2],
    pub wear_on_overheat: [f32; 2],
    pub wear_on_max_overheat: [f32; 2],
    pub wear_limit: f32,
    pub slide_on_max: bool,
    pub start_slide: f32,
    pub fix_slide: f32,
    pub firerate_border: f32,
    pub firerate_reduce: [f32; 2],
    pub max_coi_increase: f32,
    pub autoshot_chance: f32,
    pub autoshot_min: f32,
    pub autoshot_duration: f32,
    // The skills (`SkillsSettings`).
    pub repair_speed_per_level: f32,
    pub dur_loss_reduce_per_level: f32,
    pub elite_chance_reduce: [f32; 3],
}

impl Default for Settings {
    /// The install's figures (`globals.json`, read 2026-10-25).
    fn default() -> Self {
        Self {
            ammo_feed_wt: 0.2,
            ammo_jam_wt: 0.1,
            ammo_misfire_wt: 0.7,
            ammo_malf_chance_mult: 1.0,
            magazine_malf_chance_mult: 1.0,
            dur_feed_wt: 0.1,
            dur_misfire_wt: 0.1,
            dur_jam_wt: 0.6,
            dur_soft_slide_wt: 0.2,
            dur_hard_slide: [0.3, 0.8],
            dur_ignore: [93.0, 100.0],
            overheat_feed_wt: 0.35,
            overheat_jam_wt: 0.25,
            overheat_soft_slide_wt: 0.7,
            overheat_hard_slide: [0.3, 1.2],
            repair_one_hand_broken: 0.9,
            repair_two_hands_broken: 0.8,
            repair_hard_slide: 1.0,
            glow_attempts: 3,
            min_overheat: 0.0,
            max_overheat: 200.0,
            problems_start: 100.0,
            mod_heat_factor: 1.0,
            mod_cool_factor: 1.0,
            cool_coef: 6.0,
            min_malf_chance: 0.5,
            max_malf_chance: 9.0,
            dur_reduce: [1.0, 3.0],
            wear_on_overheat: [0.1, 0.2],
            wear_on_max_overheat: [0.2, 0.7],
            wear_limit: 0.55,
            slide_on_max: true,
            start_slide: 200.0,
            fix_slide: 170.0,
            firerate_border: 140.0,
            firerate_reduce: [0.85, 1.15],
            max_coi_increase: 1.5,
            autoshot_chance: 0.2,
            autoshot_min: 170.0,
            autoshot_duration: 3.0,
            repair_speed_per_level: 0.005,
            dur_loss_reduce_per_level: 0.005,
            elite_chance_reduce: [0.3, 0.3, 0.3],
        }
    }
}

impl Settings {
    /// From globals.json (`db.config`).
    pub fn from_config(config: &Json) -> Result<Self> {
        let malf = config.get("Malfunction").context("globals have no Malfunction")?;
        let heat = config.get("Overheat").context("globals have no Overheat")?;
        let skills = config.get("SkillsSettings").context("globals have no SkillsSettings")?;
        let trouble = skills.get("TroubleShooting").context("no SkillsSettings.TroubleShooting")?;
        let treatment = skills.get("WeaponTreatment").context("no SkillsSettings.WeaponTreatment")?;
        let n = |v: &Json, name: &str| -> Result<f32> { v.get(name).and_then(Json::as_f64).map(|n| n as f32).with_context(|| format!("no number {name}")) };
        let b = |v: &Json, name: &str| -> Result<bool> { v.get(name).and_then(Json::as_bool).with_context(|| format!("no flag {name}")) };
        let ignore = malf.get("DurRangeToIgnoreMalfs").context("no DurRangeToIgnoreMalfs")?;
        Ok(Self {
            ammo_feed_wt: n(malf, "AmmoFeedWt")?,
            ammo_jam_wt: n(malf, "AmmoJamWt")?,
            ammo_misfire_wt: n(malf, "AmmoMisfireWt")?,
            ammo_malf_chance_mult: n(malf, "AmmoMalfChanceMult")?,
            magazine_malf_chance_mult: n(malf, "MagazineMalfChanceMult")?,
            dur_feed_wt: n(malf, "DurFeedWt")?,
            dur_misfire_wt: n(malf, "DurMisfireWt")?,
            dur_jam_wt: n(malf, "DurJamWt")?,
            dur_soft_slide_wt: n(malf, "DurSoftSlideWt")?,
            dur_hard_slide: [n(malf, "DurHardSlideMinWt")?, n(malf, "DurHardSlideMaxWt")?],
            dur_ignore: [n(ignore, "x")?, n(ignore, "y")?],
            overheat_feed_wt: n(malf, "OverheatFeedWt")?,
            overheat_jam_wt: n(malf, "OverheatJamWt")?,
            overheat_soft_slide_wt: n(malf, "OverheatSoftSlideWt")?,
            overheat_hard_slide: [n(malf, "OverheatHardSlideMinWt")?, n(malf, "OverheatHardSlideMaxWt")?],
            repair_one_hand_broken: n(malf, "MalfRepairOneHandBrokenMult")?,
            repair_two_hands_broken: n(malf, "MalfRepairTwoHandsBrokenMult")?,
            repair_hard_slide: n(malf, "MalfRepairHardSlideMult")?,
            glow_attempts: n(malf, "ShowGlowAttemptsCount")?.max(0.0) as u32,
            min_overheat: n(heat, "MinOverheat")?,
            max_overheat: n(heat, "MaxOverheat")?,
            problems_start: n(heat, "OverheatProblemsStart")?,
            mod_heat_factor: n(heat, "ModHeatFactor")?,
            mod_cool_factor: n(heat, "ModCoolFactor")?,
            cool_coef: n(heat, "MaxOverheatCoolCoef")?,
            min_malf_chance: n(heat, "MinMalfChance")?,
            max_malf_chance: n(heat, "MaxMalfChance")?,
            dur_reduce: [n(heat, "DurReduceMinMult")?, n(heat, "DurReduceMaxMult")?],
            wear_on_overheat: [n(heat, "MinWearOnOverheat")?, n(heat, "MaxWearOnOverheat")?],
            wear_on_max_overheat: [n(heat, "MinWearOnMaxOverheat")?, n(heat, "MaxWearOnMaxOverheat")?],
            wear_limit: n(heat, "OverheatWearLimit")?,
            slide_on_max: b(heat, "EnableSlideOnMaxOverheat")?,
            start_slide: n(heat, "StartSlideOverheat")?,
            fix_slide: n(heat, "FixSlideOverheat")?,
            firerate_border: n(heat, "FirerateOverheatBorder")?,
            firerate_reduce: [n(heat, "FirerateReduceMinMult")?, n(heat, "FirerateReduceMaxMult")?],
            max_coi_increase: n(heat, "MaxCOIIncreaseMult")?,
            autoshot_chance: n(heat, "AutoshotChance")?,
            autoshot_min: n(heat, "AutoshotMinOverheat")?,
            autoshot_duration: n(heat, "AutoshotPossibilityDuration")?,
            repair_speed_per_level: n(trouble, "MalfRepairSpeedBonusPerLevel")?,
            dur_loss_reduce_per_level: n(treatment, "DurLossReducePerLevel")?,
            elite_chance_reduce: [n(trouble, "EliteDurabilityChanceReduceMult")?, n(trouble, "EliteAmmoChanceReduceMult")?, n(trouble, "EliteMagChanceReduceMult")?],
        })
    }
}

/// A weapon's figures as built (`WeaponTemplate`, and its parts' at every
/// depth, `Weapon.Mods`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponFigures {
    pub allow_jam: bool,
    pub allow_feed: bool,
    pub allow_misfire: bool,
    pub allow_slide: bool,
    pub allow_overheat: bool,
    /// `BaseMalfunctionChance`, and the template's fire rate (`bFirerate`),
    /// which the durability's chance goes by.
    pub base_chance: f32,
    pub fire_rate: f32,
    /// What one shot wears: `OperatingResource`, `DurabilityBurnRatio`, and
    /// the parts' `DurabilityBurnModificator` multiplied together.
    pub operating_resource: f32,
    pub burn_ratio: f32,
    pub parts_burn: f32,
    /// What one shot heats (`HeatFactorByShot`, `HeatFactorGun`, the parts'
    /// `HeatFactor` multiplied together), and how fast it cools
    /// (`CoolFactorGun` times `CoolFactorGunMods` times the parts'
    /// `CoolFactor`).
    pub heat_by_shot: f32,
    pub heat_gun: f32,
    pub parts_heat: f32,
    pub cool: f32,
    /// `isBoltCatch`: a jam needs rounds left or no catch.
    pub bolt_catch: bool,
}

impl Default for WeaponFigures {
    fn default() -> Self {
        Self {
            allow_jam: false,
            allow_feed: false,
            allow_misfire: false,
            allow_slide: false,
            allow_overheat: false,
            base_chance: 0.0,
            fire_rate: 600.0,
            operating_resource: 6000.0,
            burn_ratio: 1.0,
            parts_burn: 1.0,
            heat_by_shot: 1.0,
            heat_gun: 1.0,
            parts_heat: 1.0,
            cool: 1.0,
            bolt_catch: false,
        }
    }
}

impl WeaponFigures {
    /// Any malfunction at all (`Weapon.AllowMalfunction`).
    pub fn malfunctions(&self) -> bool {
        self.allow_jam || self.allow_feed || self.allow_misfire || self.allow_slide
    }

    /// From the database: the weapon's template and the templates of every
    /// part in its build (its magazine among them, which counts 1 to each).
    pub fn of(db: &trp_data::Database, weapon: &str, parts: &[&str]) -> Self {
        let props = |template: &str| db.items.get(template).map(|item| &item.props);
        let num = |props: Option<&Json>, name: &str, default: f32| -> f32 { props.and_then(|p| p.get(name)).and_then(Json::as_f64).map_or(default, |n| n as f32) };
        let flag = |props: Option<&Json>, name: &str| -> bool { props.and_then(|p| p.get(name)).and_then(Json::as_bool).unwrap_or(false) };
        let w = props(weapon);
        let (mut burn, mut heat, mut cool) = (1.0f32, 1.0f32, 1.0f32);
        for part in parts {
            let p = props(part);
            burn *= num(p, "DurabilityBurnModificator", 1.0);
            heat *= num(p, "HeatFactor", 1.0);
            cool *= num(p, "CoolFactor", 1.0);
        }
        Self {
            allow_jam: flag(w, "AllowJam"),
            allow_feed: flag(w, "AllowFeed"),
            allow_misfire: flag(w, "AllowMisfire"),
            allow_slide: flag(w, "AllowSlide"),
            allow_overheat: flag(w, "AllowOverheat"),
            base_chance: num(w, "BaseMalfunctionChance", 0.0),
            fire_rate: num(w, "bFirerate", 600.0),
            operating_resource: num(w, "OperatingResource", 6000.0).max(1.0),
            burn_ratio: num(w, "DurabilityBurnRatio", 1.0),
            parts_burn: burn,
            heat_by_shot: num(w, "HeatFactorByShot", 1.0),
            heat_gun: num(w, "HeatFactorGun", 1.0),
            parts_heat: heat,
            cool: num(w, "CoolFactorGun", 1.0) * num(w, "CoolFactorGunMods", 1.0) * cool,
            bolt_catch: flag(w, "isBoltCatch"),
        }
    }
}

/// A round's figures (`AmmoTemplate`): its chances to misfire and to fail
/// to feed (`MalfMisfireChance`, `MalfFeedChance`), what it heats
/// (`HeatFactor`) and wears (`DurabilityBurnModificator`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundFigures {
    pub misfire: f32,
    pub feed: f32,
    pub heat: f32,
    pub burn: f32,
}

impl Default for RoundFigures {
    fn default() -> Self {
        Self { misfire: 0.0, feed: 0.0, heat: 1.0, burn: 1.0 }
    }
}

impl RoundFigures {
    pub fn of(db: &trp_data::Database, template: &str) -> Self {
        let props = db.items.get(template).map(|item| &item.props);
        let num = |name: &str, default: f32| props.and_then(|p| p.get(name)).and_then(Json::as_f64).map_or(default, |n| n as f32);
        Self { misfire: num("MalfMisfireChance", 0.0), feed: num("MalfFeedChance", 0.0), heat: num("HeatFactor", 1.0), burn: num("DurabilityBurnModificator", 1.0) }
    }
}

/// A magazine's chance to fail to feed (`MalfunctionChance`).
pub fn magazine_chance(db: &trp_data::Database, template: &str) -> f32 {
    db.items.get(template).and_then(|item| item.props.get("MalfunctionChance")).and_then(Json::as_f64).map_or(0.0, |n| n as f32)
}

/// The skills that touch it, as the character has them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Skills {
    /// Weapon Maintenance's level (`WeaponTreatment`): each lowers a shot's wear.
    pub maintenance: f32,
    /// Troubleshooting's level: each quickens a repair; elite lowers the
    /// chances once a malfunction of a source has been had with the weapon.
    pub troubleshooting: f32,
    pub troubleshooting_elite: bool,
}

/// What the weapon keeps of it, with its item (`Weapon.MalfunctionState`'s
/// saved part, `MalfunctionDescriptor`): the malfunction, its source,
/// whether the shooter knows of it and of its kind, the last shot's heat and
/// the raid's clock then, and the slide the heat holds; and what is drawn
/// once and not saved (the fire rate's factor, the stray shot's time).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    pub malf: Malf,
    pub source: Source,
    pub known: bool,
    pub type_known: bool,
    pub last_shot_heat: f32,
    pub last_shot_time: f64,
    pub slide_reached: bool,
    /// The sources this shooter has had a malfunction from while elite at
    /// Troubleshooting (`AddMalfReduceChance`): any one lowers the
    /// durability's chance too (the game's flag 0 is every flag).
    pub reduce_flags: u8,
    pub firerate_mult: f32,
    pub autoshot_drawn: bool,
    pub autoshot_at: Option<f64>,
}

/// The four chances of a shot (`GetTotalMalfunctionChance`): the total, the
/// durability's (a probability), the round's, the magazine's and the heat's
/// (per mille), and the durability's floor (`weaponDurability`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Chances {
    pub total: f32,
    pub durability: f32,
    pub ammo: f32,
    pub magazine: f32,
    pub heat: f32,
    pub floor: f32,
}

/// The heat now, from the last shot's and the time since it
/// (`GetCurrentOverheat`): falling at a rate the last shot's heat sets.
pub fn current_heat(state: &State, now: f64, w: &WeaponFigures, s: &Settings) -> f32 {
    let h0 = state.last_shot_heat;
    if h0 <= 0.0 || !w.allow_overheat {
        return 0.0;
    }
    let dt = (now - state.last_shot_time).max(0.0) as f32;
    let k = w.cool * s.mod_cool_factor;
    let c = s.cool_coef;
    let d = h0 - (h0 / s.max_overheat + 0.0338 * c + 0.2925).powf(9.667 + 0.0668 * c);
    (h0 - dt * h0 * k / d).clamp(s.min_overheat, s.max_overheat)
}

/// The chance of the shot about to fire (`GetTotalMalfunctionChance`): by
/// the weapon's durability (relative to 100), its round, its magazine (if
/// one is in) and the heat at the trigger.
pub fn chances(w: &WeaponFigures, s: &Settings, durability: f32, round: Option<&RoundFigures>, magazine: Option<f32>, heat: f32, skills: &Skills, reduce_flags: u8) -> Chances {
    if !w.malfunctions() {
        return Chances::default();
    }
    let mut mag = magazine.map_or(0.0, |chance| chance * s.magazine_malf_chance_mult);
    let mut ammo = round.map_or(0.0, |r| (r.misfire + r.feed) * s.ammo_malf_chance_mult);
    let num = durability;
    let floor = num.floor();
    let heat_chance = if heat >= s.problems_start { lerp(s.min_malf_chance, s.max_malf_chance, (heat - s.problems_start) / (s.max_overheat - s.problems_start)) } else { 0.0 };
    let (b, fr, wd) = (w.base_chance as f64, w.fire_rate as f64, floor as f64);
    let mut d = if floor > 59.0 {
        ((b + 1.0).powf(3.0 + (100.0 - wd) / (20.0 - 10.0 / (fr / 10.0).powf(0.322))) - 1.0) / 1000.0
    } else {
        ((b + 1.0).powf(((101.0 - wd).powf((50.0 - wd.powf(1.286) / 4.8) / (fr.powf(0.17) / 2.9815 + 2.1))).log10()) - 1.0) / 1000.0
    } as f32;
    if skills.troubleshooting_elite && reduce_flags != 0 {
        d *= s.elite_chance_reduce[0];
    }
    if skills.troubleshooting_elite && reduce_flags & 1 != 0 {
        ammo *= s.elite_chance_reduce[1];
    }
    if skills.troubleshooting_elite && reduce_flags & 2 != 0 {
        mag *= s.elite_chance_reduce[2];
    }
    if s.dur_ignore[0] <= num && num <= s.dur_ignore[1] {
        (d, ammo, mag) = (0.0, 0.0, 0.0);
    }
    let d = d.clamp(0.0, 1.0);
    let total = ((d as f64 + ((ammo + mag + heat_chance) / 1000.0) as f64) * 100_000.0).round() / 100_000.0;
    Chances { total: (total as f32).clamp(0.0, 1.0), durability: d, ammo, magazine: mag, heat: heat_chance, floor }
}

/// The roll's own generator (`MalfunctionRandom`): each roll puts Unity's
/// generator to the seed plus the rolls so far and draws from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MalfRandom {
    pub seed: i32,
    pub count: i32,
}

impl MalfRandom {
    pub fn new(seed: i32) -> Self {
        Self { seed, count: 0 }
    }

    /// `GetRandomFloat`: the draw, and the generator left after it.
    fn float(&mut self) -> (f32, UnityRandom) {
        let mut random = UnityRandom::new(self.seed.wrapping_add(self.count));
        self.count = self.count.wrapping_add(1);
        let u = random.value();
        (u, random)
    }
}

/// What the shot is to come to (`GetMalfunctionState`), the weapon's figures
/// and its state as they stand before it, the heat at the trigger: the
/// heat's slide first, with no draw; else one draw decides whether and from
/// which source, a second which malfunction, each by its weights where the
/// weapon allows it and the magazine has what it needs.
#[allow(clippy::too_many_arguments)]
pub fn roll(random: &mut MalfRandom, w: &WeaponFigures, s: &Settings, chances: &Chances, state: &State, heat: f32, rounds_in_magazine: bool, magazine_in: bool) -> (Malf, Source) {
    if !w.malfunctions() {
        return (Malf::None, Source::Durability);
    }
    if state.slide_reached && heat > s.fix_slide && w.allow_slide && rounds_in_magazine {
        return (Malf::SoftSlide, Source::Overheat);
    }
    let (u, mut generator) = random.float();
    if u > chances.total {
        return (Malf::None, Source::Durability);
    }
    let mut sources = vec![(Source::Durability, chances.durability)];
    if chances.ammo > 0.0 {
        sources.push((Source::Ammo, chances.ammo / 1000.0));
    }
    if chances.magazine > 0.0 && rounds_in_magazine && magazine_in {
        sources.push((Source::Magazine, chances.magazine / 1000.0));
    }
    if chances.heat > 0.0 {
        sources.push((Source::Overheat, chances.heat / 1000.0));
    }
    let source = pick(&sources, u);
    let check_jam = rounds_in_magazine || !w.bolt_catch || !magazine_in;
    let feeds = rounds_in_magazine && magazine_in && w.allow_feed;
    let slides = rounds_in_magazine && w.allow_slide;
    let hard = chances.floor <= 5.0;
    let share = 1.0 - chances.floor / 5.0;
    let mut variants: Vec<(Malf, f32)> = Vec::new();
    let mut add = |malf: Malf, weight: f32, allowed: bool| {
        if allowed {
            variants.push((malf, weight));
        }
    };
    match source {
        Source::Durability => {
            add(Malf::Feed, s.dur_feed_wt, feeds);
            add(Malf::Misfire, s.dur_misfire_wt, w.allow_misfire);
            add(Malf::Jam, s.dur_jam_wt, check_jam && w.allow_jam);
            add(Malf::SoftSlide, s.dur_soft_slide_wt, slides);
            add(Malf::HardSlide, lerp(s.dur_hard_slide[0], s.dur_hard_slide[1], share), slides && hard);
        }
        Source::Ammo => {
            let both = (chances.ammo / s.ammo_malf_chance_mult.max(1e-6)).max(1e-6);
            add(Malf::Misfire, s.ammo_misfire_wt / both, w.allow_misfire);
            add(Malf::Feed, s.ammo_feed_wt, feeds);
            add(Malf::Jam, s.ammo_jam_wt, check_jam && w.allow_jam);
        }
        Source::Magazine => add(Malf::Feed, 1.0, feeds),
        Source::Overheat => {
            add(Malf::Feed, s.overheat_feed_wt, feeds);
            add(Malf::Jam, s.overheat_jam_wt, check_jam && w.allow_jam);
            add(Malf::SoftSlide, s.overheat_soft_slide_wt, slides);
            add(Malf::HardSlide, lerp(s.overheat_hard_slide[0], s.overheat_hard_slide[1], share), slides && hard);
        }
    }
    if variants.is_empty() {
        return (Malf::None, source);
    }
    let sum: f32 = variants.iter().map(|(_, weight)| weight).sum();
    let v = generator.range(0.0, sum);
    (pick(&variants, v), source)
}

/// The first whose running sum passes the value, else the last
/// (`DropManager.GenerateDrop`).
fn pick<T: Copy>(items: &[(T, f32)], value: f32) -> T {
    let mut sum = 0.0;
    for (item, weight) in items {
        sum += weight;
        if value < sum {
            return *item;
        }
    }
    items.last().expect("a list to pick from").0
}

/// What one shot that fires leaves (`Weapon.OnShot`): the heat added and
/// its effects drawn, the wear taken.
#[allow(clippy::too_many_arguments)]
pub fn on_shot(state: &mut State, now: f64, w: &WeaponFigures, s: &Settings, round: &RoundFigures, skills: &Skills, durability: &mut f32, max_durability: &mut f32, random: &mut UnityRandom) {
    // The heat.
    if w.allow_overheat {
        let current = current_heat(state, now, w, s);
        let shot = w.heat_by_shot * (round.heat * w.parts_heat) * w.heat_gun * s.mod_heat_factor;
        state.last_shot_heat = (current + shot).clamp(s.min_overheat, s.max_overheat);
        state.last_shot_time = now;
    } else {
        state.last_shot_heat = 0.0;
    }
    let heat = state.last_shot_heat;
    let t = heat.clamp(s.problems_start, s.max_overheat) / 100.0 - 1.0;
    // The wear.
    if *durability > 0.0 {
        let loss = 100.0 / w.operating_resource * w.burn_ratio * (w.parts_burn * round.burn) * lerp(s.dur_reduce[0], s.dur_reduce[1], t) * (1.0 - skills.maintenance * s.dur_loss_reduce_per_level);
        *durability = (*durability - loss).max(0.0);
    }
    // The greatest durability, worn by heat.
    if heat >= s.problems_start && *max_durability / 100.0 >= s.wear_limit {
        let wear = if (heat - s.max_overheat).abs() < f32::EPSILON { random.range(s.wear_on_max_overheat[0], s.wear_on_max_overheat[1]) } else { lerp(s.wear_on_overheat[0], s.wear_on_overheat[1], t) };
        if *durability < *max_durability - wear {
            *max_durability = (*max_durability - wear).max(s.wear_limit * 100.0);
        }
    }
    // The slide the heat holds.
    if s.slide_on_max {
        if heat >= s.start_slide {
            state.slide_reached = true;
        } else if heat <= s.fix_slide {
            state.slide_reached = false;
        }
    }
    // The fire rate's factor.
    if heat > s.firerate_border {
        if state.firerate_mult <= 0.0 {
            state.firerate_mult = random.range(s.firerate_reduce[0], s.firerate_reduce[1]);
        }
    } else {
        state.firerate_mult = 0.0;
    }
    // The stray shot.
    if heat >= s.autoshot_min {
        if !state.autoshot_drawn {
            state.autoshot_drawn = true;
            let delay = random.range(0.05, s.autoshot_duration);
            state.autoshot_at = (random.range(0.0, 1.0) <= s.autoshot_chance).then_some(now + delay as f64);
        }
    } else {
        state.autoshot_drawn = false;
        state.autoshot_at = None;
    }
}

/// The spread's factor from the heat (`FirearmController.Shot`): 1 to
/// `MaxCOIIncreaseMult` from the problems' start to the most.
pub fn spread_factor(heat: f32, s: &Settings) -> f32 {
    if heat < s.problems_start {
        return 1.0;
    }
    lerp(1.0, s.max_coi_increase, (heat - s.problems_start) / (s.max_overheat - s.problems_start))
}

/// A repair's speed (`SpeedFix`): Troubleshooting's, the arms', a hard
/// slide's.
pub fn repair_speed(s: &Settings, skills: &Skills, arms_broken: u32, malf: Malf) -> f32 {
    let hands = match arms_broken {
        0 => 1.0,
        1 => s.repair_one_hand_broken,
        _ => s.repair_two_hands_broken,
    };
    (1.0 + skills.troubleshooting * s.repair_speed_per_level) * hands * if malf == Malf::HardSlide { s.repair_hard_slide } else { 1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The install's AKM as built by default (`59d6088586f774275f37482f`,
    /// its preset's parts: the gas tube 0.995 and 1.004, the handguard 1.054
    /// and 0.95, the dust cover's burn 0.97).
    fn akm() -> WeaponFigures {
        WeaponFigures {
            allow_jam: true,
            allow_feed: true,
            allow_misfire: true,
            allow_slide: true,
            allow_overheat: true,
            base_chance: 0.1771,
            fire_rate: 600.0,
            operating_resource: 6000.0,
            burn_ratio: 1.15,
            parts_burn: 0.97,
            heat_by_shot: 1.34,
            heat_gun: 0.9306,
            parts_heat: 0.995 * 1.054,
            cool: 3.168 * 0.9503 * 1.004 * 0.95,
            bolt_catch: false,
        }
    }

    const PS: RoundFigures = RoundFigures { misfire: 0.154, feed: 0.02, heat: 1.125, burn: 1.2 };

    /// The durability's chance by its floor, as the game's two formulas give
    /// it for the AKM: 0.0758% at 92, 0.410% at 50, 19.5% at 0; nothing at 93
    /// to 100 but the heat's.
    #[test]
    fn the_durability_s_chance_climbs_as_the_weapon_wears() {
        let (w, s) = (akm(), Settings::default());
        let at = |durability: f32| chances(&w, &s, durability, Some(&PS), Some(0.09), 0.0, &Skills::default(), 0);
        assert!((at(92.4).durability - 0.000758).abs() < 2e-6, "{:?}", at(92.4));
        assert!((at(50.0).durability - 0.00410).abs() < 2e-5, "{:?}", at(50.0));
        assert!((at(0.0).durability - 0.195).abs() < 1e-3, "{:?}", at(0.0));
        assert_eq!(at(95.0).total, 0.0);
        let ninety = at(92.0);
        assert!((ninety.total - (ninety.durability + (0.174 + 0.09) / 1000.0)).abs() < 1e-5);
        let hot = chances(&w, &s, 97.0, Some(&PS), Some(0.09), 150.0, &Skills::default(), 0);
        assert!((hot.heat - 4.75).abs() < 1e-4 && (hot.total - 0.00475).abs() < 1e-5, "{hot:?}");
    }

    /// Recorded (`courses2\5-malfunction`, recorder 0.10.0, the user's AKM
    /// at durability 35 with 7.62x39 FMJ, `64b7af5a8532cf95ee0a0dbd`: misfire
    /// 0.154, feed 0.02, heat 1, burn 1): the game's chances at the shot
    /// [0.01674, 0.0164727252, 0.09, 0.174, 0, 35], a shot's wear 0.0185928
    /// (35 to 34.9814072) and its heat 1.30777049.
    #[test]
    fn the_recorded_akm_s_chance_wear_and_heat() {
        let (w, s) = (akm(), Settings::default());
        let fmj = RoundFigures { misfire: 0.154, feed: 0.02, heat: 1.0, burn: 1.0 };
        let c = chances(&w, &s, 35.0, Some(&fmj), Some(0.09), 0.0, &Skills::default(), 0);
        assert!((c.durability - 0.016_472_725).abs() < 1e-6, "{c:?}");
        assert!((c.total - 0.01674).abs() < 1e-6 && (c.ammo - 0.174).abs() < 1e-6 && (c.magazine - 0.09).abs() < 1e-6 && c.floor == 35.0, "{c:?}");
        let mut state = State::default();
        let (mut durability, mut most) = (35.0, 100.0);
        on_shot(&mut state, 1.0, &w, &s, &fmj, &Skills::default(), &mut durability, &mut most, &mut UnityRandom::new(1));
        assert!((35.0 - durability - 0.018_592_8).abs() < 2e-6, "{durability}");
        assert!((state.last_shot_heat - 1.307_770_5).abs() < 1e-5, "{state:?}");
    }

    /// A shot of PS wears the AKM 0.02231 below 100 heat and heats it 1.4712;
    /// the heat falls 2.87 a second at low heat and 4.03 from 200.
    #[test]
    fn a_shot_wears_and_heats_and_the_heat_falls_back() {
        let (w, s) = (akm(), Settings::default());
        let mut state = State::default();
        let (mut durability, mut most) = (100.0, 100.0);
        let mut random = UnityRandom::new(1);
        on_shot(&mut state, 10.0, &w, &s, &PS, &Skills::default(), &mut durability, &mut most, &mut random);
        assert!((100.0 - durability - 0.02231).abs() < 2e-5, "{durability}");
        assert!((state.last_shot_heat - 1.4712).abs() < 1e-3, "{state:?}");
        let rate = |h0: f32| {
            let state = State { last_shot_heat: h0, last_shot_time: 0.0, ..State::default() };
            h0 - current_heat(&state, 0.1, &w, &s)
        } / 0.1;
        assert!((rate(10.0) - 2.87).abs() < 0.02, "{}", rate(10.0));
        assert!((rate(200.0) - 4.03).abs() < 0.03, "{}", rate(200.0));
    }

    /// Kept firing, the heat reaches 100 about the 85th round and the
    /// maximum about the 171st, where the slide is held; the fire rate's
    /// factor is drawn past 140.
    #[test]
    fn sustained_fire_overheats_the_weapon() {
        let (w, s) = (akm(), Settings::default());
        let mut state = State::default();
        let (mut durability, mut most) = (100.0, 100.0);
        let mut random = UnityRandom::new(7);
        let mut hundred = None;
        let mut most_at = None;
        for shot in 1..=200 {
            on_shot(&mut state, shot as f64 * 0.1, &w, &s, &PS, &Skills::default(), &mut durability, &mut most, &mut random);
            if hundred.is_none() && state.last_shot_heat >= 100.0 {
                hundred = Some(shot);
            }
            if most_at.is_none() && state.last_shot_heat >= 200.0 {
                most_at = Some(shot);
            }
            if state.last_shot_heat > 140.0 {
                assert!((0.85..=1.15).contains(&state.firerate_mult));
            }
        }
        assert!(hundred.is_some_and(|n| (83..=87).contains(&n)), "{hundred:?}");
        assert!(most_at.is_some_and(|n| (168..=174).contains(&n)), "{most_at:?}");
        assert!(state.slide_reached);
        assert!(most < 100.0, "the heat wore the greatest durability once below it: {most}");
    }

    /// The roll: nothing above the chance; with a chance of 1, a durability's
    /// malfunction of one of its allowed kinds; the heat's slide with no
    /// draw.
    #[test]
    fn the_roll_picks_a_source_and_a_kind() {
        let (w, s) = (akm(), Settings::default());
        let mut random = MalfRandom::new(12345);
        let none = Chances { total: 0.0, durability: 0.0, ..Chances::default() };
        assert_eq!(roll(&mut random, &w, &s, &none, &State::default(), 0.0, true, true).0, Malf::None);
        let certain = Chances { total: 1.0, durability: 1.0, floor: 20.0, ..Chances::default() };
        let mut kinds = std::collections::HashMap::new();
        for _ in 0..2000 {
            *kinds.entry(roll(&mut random, &w, &s, &certain, &State::default(), 0.0, true, true).0).or_insert(0) += 1;
        }
        let jam = kinds[&Malf::Jam] as f32 / 2000.0;
        assert!((jam - 0.6).abs() < 0.05, "{kinds:?}");
        assert!(!kinds.contains_key(&Malf::HardSlide), "no hard slide above 5");
        let hot = State { slide_reached: true, ..State::default() };
        let count = random.count;
        assert_eq!(roll(&mut random, &w, &s, &none, &hot, 180.0, true, true), (Malf::SoftSlide, Source::Overheat));
        assert_eq!(random.count, count, "no draw spent");
    }
}
