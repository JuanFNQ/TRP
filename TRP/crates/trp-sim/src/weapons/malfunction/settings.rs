//! The malfunction settings and the weapon's and the round's figures.

use super::*;

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
    /// The install's figures (`globals.json`).
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

    /// Whether the weapon can have that malfunction at all: its template's
    /// `AllowMisfire`, `AllowJam`, `AllowFeed`, and `AllowSlide` for either
    /// slide (`GetSpecificMalfunctionVariants`). Of the install's weapons,
    /// the UZI and the AA-12 allow none, the PPSh and the M60s only
    /// overheating; the Mosins, the T-5000 and the TOZ-106 no jam; the MP-18
    /// and the MP-43s only a misfire.
    pub fn allows(&self, malf: Malf) -> bool {
        match malf {
            Malf::None => true,
            Malf::Misfire => self.allow_misfire,
            Malf::Jam => self.allow_jam,
            Malf::Feed => self.allow_feed,
            Malf::SoftSlide | Malf::HardSlide => self.allow_slide,
        }
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
