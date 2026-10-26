//! The character's health, as specified in `docs/spec/health.md`: seven
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

use crate::recoil::Rng;

mod stimulator;
pub use stimulator::{BuffDef, BuffKind, BuffRun, StimWork, StimulatorSettings};

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

    fn limb(self) -> bool {
        matches!(self, Part::LeftArm | Part::RightArm | Part::LeftLeg | Part::RightLeg)
    }

    /// Head and chest: destroyed, they kill, and nothing mends them.
    fn vital(self) -> bool {
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
        }
    }

    /// `DamageHelper.SelfInflictedDamage`.
    pub(crate) fn self_inflicted(self) -> bool {
        matches!(self, DamageKind::LightBleeding | DamageKind::HeavyBleeding | DamageKind::Dehydration | DamageKind::Exhaustion | DamageKind::Existence | DamageKind::Stimulator)
    }

    /// `DamageHelper.WeaponDamage`.
    pub fn weapon(self) -> bool {
        matches!(self, DamageKind::Bullet | DamageKind::Melee | DamageKind::Blunt)
    }

    /// `DamageHelper.EnemiesDamage`.
    fn enemy(self) -> bool {
        self.weapon() || matches!(self, DamageKind::Explosion | DamageKind::GrenadeFragment)
    }

    fn bleeding(self) -> bool {
        matches!(self, DamageKind::LightBleeding | DamageKind::HeavyBleeding)
    }

    /// `DamageHelper.EnvironmentalDamage` (barbed wire and fire).
    fn environmental(self) -> bool {
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
    fn read(v: &Json) -> Result<Self> {
        let power = match v.get("FunctionType").and_then(Json::as_str) {
            Some("Quadratic") => 2.0,
            Some("SquareRoot") => 0.5,
            _ => 1.0,
        };
        Ok(Self { power, k: num(v, "K")?, b: num(v, "B")?, threshold: num(v, "Threshold")? })
    }

    /// Whether a draw between 0 and `top` comes under the chance at `x`.
    fn try_at(&self, x: f32, top: f32, bias: f32, rng: &mut Rng) -> bool {
        if x < self.threshold {
            return false;
        }
        rng.range(0.0, top) < self.k * x.powf(self.power) + self.b + bias
    }
}

/// A bleeding's figures (`EffectsSettings.BleedingSettings`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BleedingSettings {
    pub delay: f32,
    pub residue: f32,
    pub damage: f32,
    pub loop_time: f32,
    pub damage_dehydrated: f32,
    pub loop_dehydrated: f32,
    pub life_dehydrated: f32,
    pub energy_damage: f32,
    pub energy_loop: f32,
    pub elite_duration: f32,
    pub odds: Odds,
}

/// An effect's delay and residue from the install.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Timing {
    pub delay: f32,
    pub residue: f32,
}

/// The install's figures for health (`config.Health` of `globals.json`, and
/// the over-damage factors and the interruption time beside it).
#[derive(Debug, Clone, PartialEq)]
pub struct HealthSettings {
    /// Each part's maximum, in `Part`'s order.
    pub maximum: [f32; 7],
    /// What environmental damage is multiplied by on each part.
    pub environment: [f32; 7],
    /// The share of over-damage each part takes.
    pub over_damage_received: [f32; 7],
    /// What over-damage from each part is multiplied by.
    pub over_damage_factor: [f32; 7],
    /// Energy's and hydration's least, most and start.
    pub energy: [f32; 3],
    pub hydration: [f32; 3],
    /// The body's temperature and its poisoning, which nothing here
    /// changes: their least, most and start, which the inventory's figures
    /// show.
    pub temperature: [f32; 3],
    pub poisoning: [f32; 3],
    pub light: BleedingSettings,
    pub heavy: BleedingSettings,
    pub fracture: Timing,
    pub fracture_falling: Odds,
    pub fracture_hit: Odds,
    pub dehydration: Timing,
    /// Dehydration's damage to each part, and how often.
    pub dehydration_damage: [f32; 2],
    pub exhaustion: Timing,
    pub exhaustion_damage: [f32; 2],
    /// Existence: energy taken and its loop, hydration taken and its loop,
    /// and how many times faster each goes with the stomach destroyed.
    pub existence: [f32; 6],
    pub low_health: Timing,
    pub low_health_common: f32,
    /// Medicine's delay before it works, by kind: a drink, drugs, food, a
    /// medkit, medical, a stimulator, anything else.
    pub med_start: [f32; 7],
    pub med_loop: f32,
    pub pain_tremor_delay: f32,
    pub tremor: Timing,
    /// A wound's threshold, least and most, and its working time.
    pub wound: [f32; 3],
    /// Chronic fatigue: energy a second, more a second each stack, a tick,
    /// its working time.
    pub fatigue: [f32; 4],
    pub fall_safe_height: f32,
    pub fall_damage_per_meter: f32,
    /// What a fall's damage is multiplied by fully overweight, from 1
    /// unburdened (`Stamina.FallDamageMultiplier`).
    pub fall_overweight: f32,
    /// A use cut short before this many seconds of work costs nothing.
    pub interruption_time: f32,
    /// The stimulators' sets of buffs (health spec, "Buffs").
    pub stimulator: StimulatorSettings,
}

fn num(v: &Json, name: &str) -> Result<f32> {
    v.get(name).and_then(Json::as_f64).map(|f| f as f32).with_context(|| format!("no number {name}"))
}

fn at<'a>(v: &'a Json, name: &str) -> Result<&'a Json> {
    v.get(name).ok_or_else(|| anyhow!("no {name}"))
}

impl HealthSettings {
    /// `config` is the `config` section of `globals.json`.
    pub fn new(config: &Json) -> Result<Self> {
        let health = at(config, "Health")?;
        let effects = at(health, "Effects")?;
        let profile = at(health, "ProfileHealthSettings")?;
        let parts = at(profile, "BodyPartsSettings")?;
        let per_part = |name: &str| -> Result<[f32; 7]> {
            let mut out = [0.0; 7];
            for part in Part::ALL {
                out[part.index()] = num(at(parts, part.name())?, name).with_context(|| part.name())?;
            }
            Ok(out)
        };
        let factors = at(profile, "HealthFactorsSettings")?;
        let factor = |name: &str| -> Result<[f32; 3]> {
            let f = at(factors, name)?;
            Ok([num(f, "Minimum")?, num(f, "Maximum")?, num(f, "Default")?])
        };
        let effect = |name: &str| at(effects, name).with_context(|| format!("Health.Effects.{name}"));
        let timing = |name: &str| -> Result<Timing> {
            let e = effect(name)?;
            Ok(Timing { delay: num(e, "DefaultDelay")?, residue: num(e, "DefaultResidueTime")? })
        };
        let bleeding = |name: &str| -> Result<BleedingSettings> {
            let e = effect(name)?;
            Ok(BleedingSettings {
                delay: num(e, "DefaultDelay")?,
                residue: num(e, "DefaultResidueTime")?,
                damage: num(e, "DamageHealth")?,
                loop_time: num(e, "HealthLoopTime")?,
                damage_dehydrated: num(e, "DamageHealthDehydrated")?,
                loop_dehydrated: num(e, "HealthLoopTimeDehydrated")?,
                life_dehydrated: num(e, "LifeTimeDehydrated")?,
                energy_damage: num(e, "DamageEnergy")?,
                energy_loop: num(e, "EnergyLoopTime")?,
                elite_duration: num(e, "EliteVitalityDuration")?,
                odds: Odds::read(at(e, "Probability")?)?,
            })
        };
        let fracture = effect("Fracture")?;
        let dehydration = effect("Dehydration")?;
        let exhaustion = effect("Exhaustion")?;
        let existence = effect("Existence")?;
        let low = effect("LowEdgeHealth")?;
        let med = effect("MedEffect")?;
        let wound = effect("Wound")?;
        let fatigue = effect("ChronicStaminaFatigue")?;
        let falling = at(health, "Falling")?;
        let over = |name: &str| num(config, name);
        Ok(Self {
            maximum: per_part("Maximum")?,
            environment: per_part("EnvironmentDamageMultiplier")?,
            over_damage_received: per_part("OverDamageReceivedMultiplier")?,
            // The game's own table (`GlobalConfiguration.OverDamageFactor`):
            // head and chest 1, the rest from the settings.
            over_damage_factor: {
                let (stomach, hands, legs) = (over("StomachOverdamage")?, over("HandsOverdamage")?, over("LegsOverdamage")?);
                [1.0, 1.0, stomach, hands, hands, legs, legs]
            },
            energy: factor("Energy")?,
            hydration: factor("Hydration")?,
            temperature: factor("Temperature")?,
            poisoning: factor("Poisoning")?,
            light: bleeding("LightBleeding")?,
            heavy: bleeding("HeavyBleeding")?,
            fracture: Timing { delay: num(fracture, "DefaultDelay")?, residue: num(fracture, "DefaultResidueTime")? },
            fracture_falling: Odds::read(at(fracture, "FallingProbability")?)?,
            fracture_hit: Odds::read(at(fracture, "BulletHitProbability")?)?,
            dehydration: Timing { delay: num(dehydration, "DefaultDelay")?, residue: num(dehydration, "DefaultResidueTime")? },
            dehydration_damage: [num(dehydration, "DamageOnStrongDehydration")?, num(dehydration, "StrongDehydrationLoopTime")?],
            exhaustion: Timing { delay: num(exhaustion, "DefaultDelay")?, residue: num(exhaustion, "DefaultResidueTime")? },
            exhaustion_damage: [num(exhaustion, "Damage")?, num(exhaustion, "DamageLoopTime")?],
            existence: [
                num(existence, "EnergyDamage")?,
                num(existence, "EnergyLoopTime")?,
                num(existence, "HydrationDamage")?,
                num(existence, "HydrationLoopTime")?,
                num(existence, "DestroyedStomachEnergyTimeFactor")?,
                num(existence, "DestroyedStomachHydrationTimeFactor")?,
            ],
            low_health: Timing { delay: num(low, "DefaultDelay")?, residue: num(low, "DefaultResidueTime")? },
            low_health_common: num(low, "StartCommonHealth")?,
            med_start: [
                num(med, "DrinkStartDelay")?,
                num(med, "DrugsStartDelay")?,
                num(med, "FoodStartDelay")?,
                num(med, "MedKitStartDelay")?,
                num(med, "MedicalStartDelay")?,
                num(med, "StimulatorStartDelay")?,
                num(med, "StartDelay")?,
            ],
            med_loop: num(med, "LoopTime")?,
            pain_tremor_delay: num(effect("Pain")?, "TremorDelay")?,
            tremor: timing("Tremor")?,
            wound: [num(wound, "ThresholdMin")?, num(wound, "ThresholdMax")?, num(wound, "WorkingTime")?],
            fatigue: [num(fatigue, "EnergyRate")?, num(fatigue, "EnergyRatePerStack")?, num(fatigue, "TicksEvery")?, num(fatigue, "WorkingTime")?],
            fall_safe_height: num(falling, "SafeHeight")?,
            fall_damage_per_meter: num(falling, "DamagePerMeter")?,
            fall_overweight: num(at(config, "Stamina")?, "FallDamageMultiplier")?,
            interruption_time: config.pointer("/ItemsCommonSettings/ItemRemoveAfterInterruptionTime").and_then(Json::as_f64).map(|f| f as f32).context("no ItemsCommonSettings.ItemRemoveAfterInterruptionTime")?,
            stimulator: StimulatorSettings::read(effects)?,
        })
    }
}

/// What a cure of an item does (`EDamageEffectType`), in the game's order,
/// which is also the order of how bad each is when parts are ranked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Cure {
    HeavyBleeding,
    LightBleeding,
    Fracture,
    Contusion,
    Intoxication,
    LethalIntoxication,
    RadExposure,
    Pain,
    DestroyedPart,
}

impl Cure {
    const ALL: [Cure; 9] = [
        Cure::HeavyBleeding,
        Cure::LightBleeding,
        Cure::Fracture,
        Cure::Contusion,
        Cure::Intoxication,
        Cure::LethalIntoxication,
        Cure::RadExposure,
        Cure::Pain,
        Cure::DestroyedPart,
    ];

    fn of_name(name: &str) -> Option<Cure> {
        Some(match name {
            "HeavyBleeding" => Cure::HeavyBleeding,
            "LightBleeding" => Cure::LightBleeding,
            "Fracture" => Cure::Fracture,
            "Contusion" => Cure::Contusion,
            "Intoxication" => Cure::Intoxication,
            "LethalIntoxication" => Cure::LethalIntoxication,
            "RadExposure" => Cure::RadExposure,
            "Pain" => Cure::Pain,
            "DestroyedPart" => Cure::DestroyedPart,
            _ => return None,
        })
    }

    /// The effect it cures, of those built.
    fn effect(self) -> Option<Kind> {
        Some(match self {
            Cure::HeavyBleeding => Kind::HeavyBleeding,
            Cure::LightBleeding => Kind::LightBleeding,
            Cure::Fracture => Kind::Fracture,
            Cure::Pain => Kind::Pain,
            Cure::Contusion => Kind::Contusion,
            _ => return None,
        })
    }
}

/// One cure an item has (`DamageEffectSpecification`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CureSpec {
    pub delay: f32,
    pub duration: f32,
    pub fade_out: f32,
    pub cost: f32,
    /// The share of a destroyed part's maximum it comes back with, in
    /// hundredths, least and most (`healthPenaltyMin`, `Max`).
    pub kept: [i32; 2],
}

/// What sort of thing is used (the item's class), which sets the delay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MedClass {
    Drink,
    Drugs,
    Food,
    MedKit,
    Medical,
    Stimulator,
}

/// What the database says of an item that is used on the body: medicine,
/// food or drink (`MedsTemplate`, `FoodDrinkTemplate`).
#[derive(Debug, Clone, PartialEq)]
pub struct Medicine {
    pub template: String,
    pub class: MedClass,
    /// Seconds of use (`medUseTime`, `foodUseTime`).
    pub use_time: f32,
    /// What the use time is multiplied by on some parts.
    pub part_time: Vec<(Part, f32)>,
    /// Hit points it holds when new, and gives over a use (medicine only).
    pub max_hp_resource: f32,
    pub hp_resource_rate: f32,
    /// The parts it goes to first (the surgical kits).
    pub part_priority: Vec<Part>,
    /// Energy and hydration the whole item gives (`effects_health`).
    pub energy: f32,
    pub hydration: f32,
    pub cures: Vec<(Cure, CureSpec)>,
    /// Food and drink: how much there is of it when new.
    pub max_resource: f32,
    /// The stimulator buff it gives, if any (not built).
    pub buffs: Option<String>,
}

impl Medicine {
    /// The item of the database, if it is medicine, food or drink.
    pub fn of(db: &Database, template: &str) -> Option<Self> {
        let item = db.items.get(template)?;
        let classes = db.class_names(template);
        let is = |name: &str| classes.iter().any(|class| class == name);
        let class = if is("Drink") {
            MedClass::Drink
        } else if is("Food") {
            MedClass::Food
        } else if is("Drugs") {
            MedClass::Drugs
        } else if is("MedKit") {
            MedClass::MedKit
        } else if is("Stimulator") {
            MedClass::Stimulator
        } else if is("Medical") || is("Meds") {
            MedClass::Medical
        } else {
            return None;
        };
        // The game's reader takes a property in any case
        // (`BodyPartPriority` in the install for `bodyPartPriority`).
        let props = &item.props;
        let get = |name: &str| props.get(name).or_else(|| props.as_object()?.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, v)| v));
        let f = |name: &str| get(name).and_then(Json::as_f64).unwrap_or(0.0) as f32;
        let food = matches!(class, MedClass::Food | MedClass::Drink);
        let parts = |name: &str| -> Vec<Part> { get(name).and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_str).filter_map(Part::of_name).collect() };
        let part_time = get("bodyPartTimeMults").and_then(Json::as_array).into_iter().flatten().filter_map(|pair| {
            let (part, mult) = (pair.get("Key").or_else(|| pair.get(0))?, pair.get("Value").or_else(|| pair.get(1))?);
            Some((Part::of_name(part.as_str()?)?, mult.as_f64()? as f32))
        });
        let factor = |name: &str| get("effects_health").and_then(|e| e.get(name)).and_then(|e| e.get("value")).and_then(Json::as_f64).unwrap_or(0.0) as f32;
        let cures = get("effects_damage").and_then(Json::as_object).into_iter().flatten().filter_map(|(name, spec)| {
            let g = |k: &str| spec.get(k).and_then(Json::as_f64).unwrap_or(0.0);
            Some((Cure::of_name(name)?, CureSpec {
                delay: g("delay") as f32,
                duration: g("duration") as f32,
                fade_out: g("fadeOut") as f32,
                cost: g("cost") as f32,
                kept: [g("healthPenaltyMin") as i32, g("healthPenaltyMax") as i32],
            }))
        });
        let mut cures: Vec<(Cure, CureSpec)> = cures.collect();
        cures.sort_by_key(|(cure, _)| *cure);
        Some(Self {
            template: template.to_owned(),
            class,
            use_time: if food { f("foodUseTime") } else { f("medUseTime") },
            part_time: part_time.collect(),
            max_hp_resource: if food { 0.0 } else { f("MaxHpResource") },
            hp_resource_rate: if food { 0.0 } else { f("hpResourceRate") },
            part_priority: parts("bodyPartPriority"),
            energy: factor("Energy"),
            hydration: factor("Hydration"),
            cures,
            max_resource: if food { f("MaxResource") } else { 0.0 },
            buffs: get("StimulatorBuffs").and_then(Json::as_str).filter(|s| !s.is_empty()).map(str::to_owned),
        })
    }

    pub fn food(&self) -> bool {
        matches!(self.class, MedClass::Food | MedClass::Drink)
    }

    /// What a new one holds: a medkit's hit points, food's portion.
    pub fn full(&self) -> f32 {
        if self.food() { self.max_resource } else { self.max_hp_resource }
    }

    fn cure(&self, cure: Cure) -> Option<&CureSpec> {
        self.cures.iter().find(|(c, _)| *c == cure).map(|(_, spec)| spec)
    }

    fn use_time_on(&self, part: Part) -> f32 {
        self.part_time.iter().find(|(p, _)| *p == part).map_or(self.use_time, |(_, mult)| self.use_time * mult)
    }

    fn start_delay(&self, s: &HealthSettings) -> f32 {
        s.med_start[match self.class {
            MedClass::Drink => 0,
            MedClass::Drugs => 1,
            MedClass::Food => 2,
            MedClass::MedKit => 3,
            MedClass::Medical => 4,
            MedClass::Stimulator => 5,
        }]
    }
}

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
}

impl Kind {
    pub const ALL: [Kind; 22] = [
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
        }
    }

    /// `IStackable`: a second is a second, not the first lengthened.
    fn stacks(self) -> bool {
        matches!(self, Kind::Tremor | Kind::MedEffect | Kind::Contusion)
    }

    /// `IStackableFromDifferentTemplates`: one for each item.
    fn stacks_by_item(self) -> bool {
        matches!(self, Kind::PainKiller | Kind::Stimulator)
    }

    fn bleeding(self) -> bool {
        matches!(self, Kind::LightBleeding | Kind::HeavyBleeding)
    }

    /// Neither wanted (`IDesirable`) nor for good (`IPermanent`): what
    /// `RemoveNegativeEffects` takes away.
    pub fn negative(self) -> bool {
        !matches!(self, Kind::PainKiller | Kind::Existence | Kind::MedEffect | Kind::Stimulator)
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
    health_points: f32,
    energy_points: f32,
    hydration_points: f32,
    energy_gained: f32,
    hydration_gained: f32,
    loop_time: f32,
    interrupted: bool,
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
    state_time: f32,
    pub state_current: f32,
    /// Seconds since it was made.
    pub whole: f32,
    forced: bool,
    side: Option<u32>,
    /// Timers of its own loops.
    loops: [f32; 2],
    /// Its own figures (a bleeding's damage and loop, a wound's threshold
    /// and tally, a painkiller's item and full duration).
    figures: [f32; 4],
    /// A painkiller's item.
    item: Option<String>,
    /// Pain's count to its tremor.
    tremor_after: Option<f32>,
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

    fn calculate_strength(&mut self, dt: f32) {
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

/// A part's state (`BodyPartState`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PartState {
    pub current: f32,
    pub maximum: f32,
    pub destroyed: bool,
}

/// What the character can and cannot do because of its health
/// (`EPhysicalCondition`, as `Player` sets them on the movement context).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Conditions {
    pub on_painkillers: bool,
    pub left_leg: bool,
    pub right_leg: bool,
    pub left_arm: bool,
    pub right_arm: bool,
    pub tremor: bool,
    pub using_meds: bool,
    pub healing_legs: bool,
}

impl Conditions {
    pub fn leg_damaged(&self) -> bool {
        self.left_leg || self.right_leg
    }

    /// `MovementContext.CanSprint`.
    pub fn can_sprint(&self) -> bool {
        if self.using_meds || self.healing_legs {
            return false;
        }
        self.on_painkillers || !self.leg_damaged()
    }

    /// Whether the stamina's sprint drain may start (`Physical`'s sprint
    /// availability): no medicine at work, and no damaged leg unless a
    /// painkiller is at work.
    pub fn can_sprint_drain(&self) -> bool {
        !self.using_meds && (!self.leg_damaged() || self.on_painkillers)
    }

    /// `MovementContext.CanJump`.
    pub fn can_jump(&self) -> bool {
        !self.using_meds && !self.healing_legs
    }

    /// `MovementContext.CanWalk`, health's part.
    pub fn can_walk(&self) -> bool {
        !self.healing_legs
    }

    /// `MovementContext.CanProne`, health's part.
    pub fn can_prone(&self) -> bool {
        !self.using_meds && !self.healing_legs
    }

    /// `MovementContext.CanMoveInProne`, health's part.
    pub fn can_crawl(&self) -> bool {
        !self.using_meds && !self.healing_legs
    }

    /// The speed limit health puts on the character, if any
    /// (`ESpeedLimit.HealthCondition`).
    pub fn speed_limit(&self) -> Option<f32> {
        if !self.leg_damaged() || self.on_painkillers {
            None
        } else if self.left_leg && self.right_leg {
            Some(0.2)
        } else {
            Some(0.3)
        }
    }

    /// What aiming's stamina cost is multiplied by (`Physical`'s aim drain).
    pub fn aim_cost(&self) -> f32 {
        if self.on_painkillers {
            1.0
        } else if self.left_arm && self.right_arm {
            2.0
        } else if self.left_arm || self.right_arm {
            1.5
        } else {
            1.0
        }
    }

    /// How far the sway spring is toward the damaged one's: 0, 0.5 or 1.
    pub fn sway_damage(&self) -> f32 {
        match (self.left_arm, self.right_arm) {
            (true, true) => 1.0,
            (false, false) => 0.0,
            _ => 0.5,
        }
    }

    /// The breathing's fracture (`BreathEffector.Fracture`).
    pub fn arms_fractured(&self) -> bool {
        self.left_arm || self.right_arm
    }

    /// The breathing's tremor (`BreathEffector.TremorOn`).
    pub fn tremor_on(&self) -> bool {
        self.tremor && !self.on_painkillers
    }

    /// The walk's bob is the lame one.
    pub fn lame(&self) -> bool {
        self.leg_damaged() && !self.on_painkillers
    }

    /// A vault's or a climb's slow end (`MovementContext.SetVaultingSpeed`):
    /// the figure its speed range is read at, before the weight's. `Some`
    /// only where the obstacle is high enough for a limb to matter.
    pub fn vault_slowing(&self, climb: bool, height: f32, without_hand: f32, one_hand: f32) -> f32 {
        let slow = if self.on_painkillers { 0.75 } else { 1.0 };
        let mut b = 0.0;
        if height > without_hand {
            if self.leg_damaged() || self.left_arm {
                b = slow;
            }
            if climb && self.right_arm && height > one_hand {
                b = slow;
            }
        }
        b
    }
}

/// Something that happened, for the host.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// An effect started (`EffectStartedEvent`).
    Started(Kind, Part),
    /// An effect went residual (`EffectResidualEvent`).
    Residual(Kind, Part),
    /// An effect was taken off (`EffectRemovedEvent`): for medicine, the
    /// hands go on to the next part or finish.
    Removed(Kind, Part, u32),
    Damaged(Part, f32, DamageKind),
    Destroyed(Part, DamageKind),
    Restored(Part),
    Died(DamageKind),
    /// What an item at work holds now.
    Resource(u64, f32),
    /// An item at work used up: it is gone.
    UsedUp(u64),
}

/// How a use of an item was refused (`FailedResult`'s word).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// `Health/PlayerIsDead`.
    Dead,
    /// `Inventory/IncompatibleItem`: it can do nothing, or something is at
    /// work already.
    Incompatible,
    /// `Health/ItemResourceDepleted`.
    Depleted,
}

/// What the character's skills do to its health (`SkillManager`'s buffs,
/// each by the skill's level: its progress over 100, at most 51; elite
/// above 50). All 0 untrained, as the app's character is; the health
/// replay gives the recorded character's (`bin/health_replay`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HealthSkills {
    /// Stress Resistance's `StressPain`, 0.01 a level: pain's residue,
    /// working time and strength cut by it (`Pain.Init`).
    pub stress_pain: f32,
    /// Health's `HealthEnergy` and `HealthHydration`, 0.006 a level: what
    /// existence drains, cut by them (`Existence.GetEnergyDamage`).
    pub health_energy: f32,
    pub health_hydration: f32,
    /// Health's `HealthBreakChanceRed`, 0.012 a level: the top of a
    /// fracture's odds raised by it.
    pub break_chance: f32,
    /// Vitality's `VitalityBuffBleedChanceRed`, 0.012 a level: the top of a
    /// bleeding's odds raised by it.
    pub bleed_chance: f32,
    /// Metabolism's `MetabolismRatioPlus`, 0.01 a level: what food and drink
    /// give, raised by it; Immunity's `ImmunityMiscEffects`, 0.01 a level:
    /// what they take, cut by it.
    pub metabolism_ratio_plus: f32,
    pub immunity_misc: f32,
    /// Immunity's `ImmunityPainKiller`, 0.006 a level: a painkiller's time
    /// raised by it.
    pub immunity_painkiller: f32,
    /// Surgery's `SurgerySpeed` (0.2 at level 50, 0.4 elite): a destroyed
    /// part's working time divided by one more than it; its
    /// `SurgeryReducePenalty` (0.01 a level, 1 elite): a mended part's kept
    /// share drawn that far toward all.
    pub surgery_speed: f32,
    pub surgery_penalty: f32,
    /// Vitality's elite (`VitalityBuffBleedStop`): a bleeding works for its
    /// `EliteVitalityDuration` (light 22 s, heavy 33 s here) and stops.
    pub bleed_stop: bool,
    /// Metabolism's elite (`MetabolismEliteBuffNoDyhydration`): strong
    /// dehydration and exhaustion do no damage.
    pub no_starvation_damage: bool,
    /// Metabolism's `MetabolismMiscDebuffTime`, 0.01 a level: a stimulator's
    /// side effects' time cut by it; Immunity's elite
    /// `ImmunityAvoidMiscEffectsChance` (0.9): the share of side effects
    /// dodged (health spec, "Buffs").
    pub metabolism_debuff: f32,
    pub immunity_avoid: f32,
}

impl HealthSkills {
    /// The buffs from the character's skills.
    pub fn of(levels: &crate::skills::SkillLevels) -> Self {
        Self::of_levels(|name| levels.level_of(name))
    }

    /// The buffs from the skills' levels, by the game's names (`ESkillId`):
    /// each rule worked with the level held at 50 (`Skill.UpdateRules`),
    /// the elites above it.
    pub fn of_levels(level: impl Fn(&str) -> u32) -> Self {
        let at = |name: &str| level(name).min(50) as f32;
        let elite = |name: &str| level(name) > 50;
        Self {
            stress_pain: at("StressResistance") * 0.01,
            health_energy: at("Health") * 0.006,
            health_hydration: at("Health") * 0.006,
            break_chance: at("Health") * 0.012,
            bleed_chance: at("Vitality") * 0.012,
            metabolism_ratio_plus: at("Metabolism") * 0.01,
            immunity_misc: at("Immunity") * 0.01,
            immunity_painkiller: at("Immunity") * 0.006,
            surgery_speed: if elite("Surgery") { 0.4 } else { at("Surgery") * 0.2 / 50.0 },
            surgery_penalty: if elite("Surgery") { 1.0 } else { at("Surgery") * 0.01 },
            bleed_stop: elite("Vitality"),
            no_starvation_damage: elite("Metabolism"),
            metabolism_debuff: at("Metabolism") * 0.01,
            immunity_avoid: if elite("Immunity") { 0.9 } else { 0.0 },
        }
    }
}

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
    /// Full health, as a run begins (the user, 2026-10-14: full every run),
    /// with the existence effect on the head as the game's player has it.
    pub fn new(settings: Arc<HealthSettings>, seed: u64) -> Self {
        let parts = std::array::from_fn(|i| PartState { current: settings.maximum[i], maximum: settings.maximum[i], destroyed: false });
        let (e, h) = (settings.energy, settings.hydration);
        let mut health = Self {
            parts,
            energy: [e[2], e[0], e[1]],
            hydration: [h[2], h[0], h[1]],
            settings,
            alive: true,
            cause: None,
            effects: Vec::new(),
            next_id: 0,
            rng: Rng::new(seed),
            outcomes: Vec::new(),
            events: Vec::new(),
            damage_multiplier: 1.0,
            skills: HealthSkills::default(),
            metabolism_disabled: false,
            clock: 0.0,
            self_damage: None,
            encumbered: None,
            over_encumbered: None,
        };
        health.add(Kind::Existence, Part::Head, None, None, None, None);
        health
    }

    /// An effect taken up as at work already, with its kind's own figures
    /// and no delay: for a run that takes up a character mid-raid (the
    /// health replay), where an effect began before it did.
    pub fn take_up(&mut self, kind: Kind, part: Part) -> u32 {
        self.add(kind, part, Some(0.0), None, None, None)
    }

    /// Sets how far into its loops existence's hunger and thirst are: for a
    /// run that takes up a character mid-raid (the health replay, whose
    /// recordings begin with the raid's clock already going).
    pub fn set_existence_clock(&mut self, into_loop: f32) {
        for e in self.effects.iter_mut().filter(|e| e.kind == Kind::Existence) {
            e.loops = [into_loop, into_loop];
        }
    }

    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// What the effects at work take from or give to the whole's health,
    /// energy and hydration, a second, as the inventory's figures show it
    /// (`HealthRate`, `EnergyRate`, `HydrationRate`, which the screen shows
    /// a minute): each effect's own rate (`SetHealthRatesPerSecond`), set
    /// as it starts and changed as it works, counted while it works; energy
    /// and hydration none while metabolism is stopped. Only the effects
    /// this simulation has: a bleeding's damage, and its energy; strong
    /// dehydration's and exhaustion's damage; existence's energy and
    /// hydration; chronic fatigue's energy; medicine's, food's and drink's
    /// points over their work.
    pub fn rates(&self) -> [f32; 3] {
        let parts = Part::ALL.len() as f32;
        let mut sum = [0.0f32; 3];
        for e in self.effects.iter().filter(|e| e.active()) {
            let rate = match e.kind {
                Kind::LightBleeding | Kind::HeavyBleeding => {
                    let s = if e.kind == Kind::HeavyBleeding { self.settings.heavy } else { self.settings.light };
                    [e.figures[2], -s.energy_damage / s.energy_loop, 0.0]
                }
                Kind::Dehydration => {
                    let [damage, every] = self.starving(self.settings.dehydration_damage);
                    [-damage * parts / every, 0.0, 0.0]
                }
                Kind::Exhaustion => {
                    let [damage, every] = self.starving(self.settings.exhaustion_damage);
                    [-damage * parts / every, 0.0, 0.0]
                }
                Kind::Existence => {
                    let s = self.settings.existence;
                    let stomach_gone = self.part(Part::Stomach).destroyed;
                    let (energy_loop, hydration_loop) = if stomach_gone { (s[1] / s[4], s[3] / s[5]) } else { (s[1], s[3]) };
                    [0.0, -s[0] * (1.0 - self.skills.health_energy) / energy_loop, -s[2] * (1.0 - self.skills.health_hydration) / hydration_loop]
                }
                Kind::ChronicFatigue => [0.0, -self.fatigue_rate(e.id), 0.0],
                Kind::MedEffect => match e.med.as_deref() {
                    Some(m) if e.work > 0.0 => [m.health_points / e.work, m.energy_points / e.work, m.hydration_points / e.work],
                    _ => [0.0; 3],
                },
                _ => [0.0; 3],
            };
            for (total, own) in sum.iter_mut().zip(rate) {
                *total += own;
            }
        }
        if self.metabolism_disabled {
            sum[1] = 0.0;
            sum[2] = 0.0;
        }
        sum
    }

    /// The seven parts' sum (`GetBodyPartHealth(Common)`): current and
    /// maximum.
    pub fn common(&self) -> [f32; 2] {
        self.parts.iter().fold([0.0, 0.0], |[c, m], p| [c + p.current, m + p.maximum])
    }

    pub fn part(&self, part: Part) -> &PartState {
        &self.parts[part.index()]
    }

    /// Energy and hydration as shares of their most.
    pub fn energy_share(&self) -> f32 {
        share(self.energy)
    }

    pub fn hydration_share(&self) -> f32 {
        share(self.hydration)
    }

    fn index_of(&self, id: u32) -> Option<usize> {
        self.effects.iter().position(|e| e.id == id)
    }

    /// The first effect of a kind on a part (`None` for any part) that is
    /// added or started.
    pub fn existing(&self, kind: Kind, part: Option<Part>) -> Option<&Effect> {
        self.effects.iter().find(|e| e.kind == kind && part.is_none_or(|p| e.part == p) && e.existing())
    }

    /// The first effect of a kind on a part that has started.
    pub fn active(&self, kind: Kind, part: Option<Part>) -> Option<&Effect> {
        self.effects.iter().find(|e| e.kind == kind && part.is_none_or(|p| e.part == p) && e.active())
    }

    fn active_id(&self, kind: Kind, part: Option<Part>) -> Option<u32> {
        self.active(kind, part).map(|e| e.id)
    }

    fn existing_id(&self, kind: Kind, part: Option<Part>) -> Option<u32> {
        self.existing(kind, part).map(|e| e.id)
    }

    /// Whether a limb is broken (`IsBodyPartBroken`: a fracture started).
    pub fn broken(&self, part: Part) -> bool {
        self.active(Kind::Fracture, Some(part)).is_some()
    }

    /// The conditions as the game's player sets them.
    pub fn conditions(&self) -> Conditions {
        let on_painkillers = self.active(Kind::PainKiller, None).is_some();
        let damaged = |part: Part| self.broken(part) || self.part(part).destroyed;
        let meds: Vec<&Effect> = self.effects.iter().filter(|e| e.kind == Kind::MedEffect && e.active()).collect();
        Conditions {
            on_painkillers,
            left_leg: damaged(Part::LeftLeg),
            right_leg: damaged(Part::RightLeg),
            left_arm: damaged(Part::LeftArm) && !on_painkillers,
            right_arm: damaged(Part::RightArm) && !on_painkillers,
            tremor: self.active(Kind::Tremor, None).is_some(),
            using_meds: !meds.is_empty(),
            healing_legs: meds.iter().any(|e| e.med.as_ref().is_some_and(|m| m.no_move)),
        }
    }

    /// Adds an effect as `AddEffect` does, or lengthens the one there.
    fn add(&mut self, kind: Kind, part: Part, delay: Option<f32>, work: Option<f32>, residue: Option<f32>, strength: Option<f32>) -> u32 {
        self.add_with(kind, part, delay, work, residue, strength, |_| {})
    }

    #[allow(clippy::too_many_arguments)]
    fn add_with(&mut self, kind: Kind, part: Part, delay: Option<f32>, work: Option<f32>, residue: Option<f32>, strength: Option<f32>, init: impl FnOnce(&mut Effect)) -> u32 {
        if !kind.stacks() && !kind.stacks_by_item() {
            if let Some(id) = self.existing_id(kind, Some(part)) {
                self.add_work_time(id, work, false);
                if let Some(s) = strength.filter(|s| *s > self.effects[self.index_of(id).unwrap()].strength) {
                    self.set_strength(id, s);
                }
                return id;
            }
        }
        let residual: Vec<u32> = self.effects.iter().filter(|e| e.kind == kind && e.part == part && e.stage == Stage::Residual).map(|e| e.id).collect();
        for id in residual {
            self.force_remove(id);
        }
        let [default_delay, default_work, default_residue, default_build] = self.defaults(kind);
        let (mut work, mut residue, mut strength) = (work, residue, strength.unwrap_or(1.0));
        if kind == Kind::Pain {
            // `Pain.Init`: cut by the Stress Resistance skill's pain buff.
            let cut = (1.0 - self.skills.stress_pain).max(0.01);
            work = work.map(|w| if w.is_finite() { w * cut } else { w });
            residue = residue.map(|r| r * cut);
            strength *= cut;
        }
        self.next_id += 1;
        let delay = delay.unwrap_or(default_delay);
        let work = work.unwrap_or(default_work);
        let mut effect = Effect {
            id: self.next_id,
            kind,
            part,
            stage: Stage::None,
            strength,
            current_strength: 0.0,
            delay,
            build_up: default_build.min(work),
            work,
            residue: residue.unwrap_or(default_residue),
            state_time: delay,
            state_current: 0.0,
            whole: 0.0,
            forced: false,
            side: None,
            loops: [0.0; 2],
            figures: [0.0; 4],
            item: None,
            tremor_after: None,
            med: None,
            stim: None,
        };
        init(&mut effect);
        let id = effect.id;
        self.effects.push(effect);
        self.next_state(id);
        id
    }

    /// A painkiller or the like from an item: a second from the same item
    /// lengthens the first (`AddStackableEffect`, `UpdateWithSameOne`).
    fn add_by_item(&mut self, kind: Kind, part: Part, item: &str, strength: f32, delay: f32, work: f32, residue: f32, full_duration: f32) -> u32 {
        if let Some(i) = self.effects.iter().position(|e| e.kind == kind && e.part == part && e.existing() && e.item.as_deref() == Some(item)) {
            let id = self.effects[i].id;
            self.effects[i].figures[0] = full_duration;
            let left = self.effects[i].time_left();
            let longer = (left + full_duration * strength).clamp(0.0, full_duration);
            self.add_work_time(id, Some(longer), true);
            return id;
        }
        let item = item.to_owned();
        self.add_with(kind, part, Some(delay), Some(work), Some(residue), Some(strength), move |e| {
            e.item = Some(item);
            e.figures[0] = full_duration;
        })
    }

    /// `AddSideEffect`: made for another, and ended with it, unless the
    /// part had one already.
    fn add_side(&mut self, of: u32, kind: Kind, part: Part, delay: Option<f32>) {
        let before = self.existing_id(kind, Some(part));
        let side = self.add(kind, part, delay, None, None, None);
        if let Some(i) = self.index_of(of) {
            self.effects[i].side = (Some(side) != before).then_some(side);
        }
    }

    fn remove_side(&mut self, side: Option<u32>) {
        let Some(id) = side else { return };
        let Some(i) = self.index_of(id) else { return };
        match self.effects[i].stage {
            Stage::Started => self.add_work_time(id, Some(0.0), true),
            Stage::Added => self.force_remove(id),
            _ => {}
        }
    }

    fn add_work_time(&mut self, id: u32, time: Option<f32>, reset: bool) {
        let Some(i) = self.index_of(id) else { return };
        let default_work = self.defaults(self.effects[i].kind)[1];
        let default_build = self.defaults(self.effects[i].kind)[3];
        let e = &mut self.effects[i];
        if e.stage > Stage::Started {
            return;
        }
        let time = time.unwrap_or(default_work);
        if e.stage >= Stage::Started && time.is_finite() {
            e.work = if reset { e.state_current } else { e.work } + time;
            e.build_up = (e.state_current + default_build).min(e.work);
        } else {
            e.work = time.max(e.work);
        }
        e.calculate_strength(0.0);
        if e.stage == Stage::Started && (e.state_time - e.work).abs() > f32::EPSILON {
            e.state_time = e.work;
        }
    }

    fn set_residue_time(&mut self, id: u32, time: f32) {
        let Some(i) = self.index_of(id) else { return };
        let e = &mut self.effects[i];
        if e.stage < Stage::Residual {
            e.residue = time.min(e.residue);
        } else if e.state_current + time < e.residue {
            e.residue = e.state_current + time;
            e.state_time = e.residue;
            e.calculate_strength(0.0);
        }
    }

    fn set_strength(&mut self, id: u32, strength: f32) {
        let Some(i) = self.index_of(id) else { return };
        let e = &mut self.effects[i];
        if (strength - e.strength).abs() > f32::EPSILON {
            e.strength = strength;
            e.calculate_strength(0.0);
        }
    }

    /// Moves an effect on to residual now (`ForceResidue`).
    pub fn force_residue(&mut self, id: u32) {
        let Some(i) = self.index_of(id) else { return };
        match self.effects[i].kind {
            // A forced residue lasts two seconds (`DEFAULT_FORCE_RESIDUE_TIME`).
            Kind::Pain | Kind::Contusion => self.set_residue_time(id, 2.0),
            Kind::MedEffect => {
                if let Some(med) = self.effects[i].med.as_mut() {
                    med.interrupted = true;
                }
            }
            _ => {}
        }
        let Some(i) = self.index_of(id) else { return };
        self.effects[i].forced = true;
        if self.effects[i].stage == Stage::Added {
            self.next_state(id);
        }
        if self.index_of(id).is_some_and(|i| self.effects[i].stage == Stage::Started) {
            self.next_state(id);
        }
    }

    /// Takes an effect off now (`ForceRemove`).
    pub fn force_remove(&mut self, id: u32) {
        self.force_residue(id);
        if self.index_of(id).is_some_and(|i| self.effects[i].stage == Stage::Residual) {
            self.next_state(id);
        }
    }


    fn next_state(&mut self, id: u32) {
        let Some(i) = self.index_of(id) else { return };
        let e = &mut self.effects[i];
        e.state_current = 0.0;
        match e.stage {
            Stage::None => {
                e.stage = Stage::Added;
                let (kind, part, delay) = (e.kind, e.part, e.delay);
                self.on_added(id, kind, part);
                if delay <= 0.0 {
                    self.next_state(id);
                }
            }
            Stage::Added => {
                e.stage = Stage::Started;
                e.state_time = e.work;
                let (kind, part, work) = (e.kind, e.part, e.work);
                self.on_started(id, kind, part);
                self.events.push(Event::Started(kind, part));
                if work.abs() < f32::EPSILON && self.index_of(id).is_some_and(|i| self.effects[i].stage == Stage::Started) {
                    self.next_state(id);
                }
            }
            Stage::Started => {
                e.stage = Stage::Residual;
                e.state_time = e.residue;
                let (kind, part, residue) = (e.kind, e.part, e.residue);
                self.on_residue(id, kind, part);
                self.events.push(Event::Residual(kind, part));
                if residue.abs() < f32::EPSILON && self.index_of(id).is_some_and(|i| self.effects[i].stage == Stage::Residual) {
                    self.next_state(id);
                }
            }
            Stage::Residual => {
                e.stage = Stage::Removed;
                e.state_time = 0.0;
                let removed = self.effects.remove(i);
                self.on_removed(&removed);
                self.events.push(Event::Removed(removed.kind, removed.part, removed.id));
                self.remove_side(removed.side);
            }
            Stage::Removed => {}
        }
    }

    fn on_added(&mut self, id: u32, kind: Kind, part: Part) {
        match kind {
            Kind::Wound => {
                let [least, most, _] = self.settings.wound;
                let threshold = self.rng.range(least, most);
                if let Some(i) = self.index_of(id) {
                    self.effects[i].figures = [threshold, 0.0, 0.0, 0.0];
                }
            }
            Kind::Dehydration => self.add_side(id, Kind::Pain, Part::Head, Some(10.0)),
            _ => {}
        }
        let _ = part;
    }

    fn on_started(&mut self, id: u32, kind: Kind, part: Part) {
        match kind {
            Kind::LightBleeding | Kind::HeavyBleeding => {
                let dehydrated = self.active(Kind::Dehydration, None).is_some();
                self.reset_bleeding(id, dehydrated);
            }
            Kind::Dehydration => {
                let bleedings: Vec<u32> = self.effects.iter().filter(|e| e.kind.bleeding() && e.active()).map(|e| e.id).collect();
                for b in bleedings {
                    self.reset_bleeding(b, true);
                }
                let damage = self.starving(self.settings.dehydration_damage)[0];
                if let Some(i) = self.index_of(id) {
                    self.effects[i].figures[0] = damage;
                }
            }
            Kind::Fracture => self.add_side(id, Kind::Pain, part, Some(1.0)),
            Kind::Pain => {
                if let Some(i) = self.index_of(id) {
                    self.effects[i].tremor_after = Some(0.0);
                }
            }
            Kind::ChronicFatigue => {
                // `ChronicStaminaFatigue.Started` sets its strength to 1
                // and its first tick a tick away.
                let tick = self.settings.fatigue[2];
                let clock = self.clock;
                if let Some(i) = self.index_of(id) {
                    self.effects[i].strength = 1.0;
                    self.effects[i].calculate_strength(0.0);
                    self.effects[i].figures[1] = clock + tick;
                }
            }
            Kind::MedEffect => self.med_started(id),
            Kind::Stimulator => self.stim_started(id),
            // The damage taken changed by its strength while it works,
            // kept to be undone (`DamageModifier.Started`).
            Kind::DamageModifier => {
                if let Some(i) = self.index_of(id) {
                    let s = self.effects[i].strength;
                    self.effects[i].figures[0] = s;
                    self.damage_multiplier *= 1.0 + s;
                }
            }
            _ => {}
        }
    }

    fn on_residue(&mut self, id: u32, kind: Kind, part: Part) {
        match kind {
            Kind::Dehydration => {
                let bleedings: Vec<u32> = self.effects.iter().filter(|e| e.kind.bleeding() && e.active()).map(|e| e.id).collect();
                for b in bleedings {
                    self.reset_bleeding(b, false);
                }
            }
            Kind::MedEffect => self.med_residue(id),
            Kind::Stimulator => self.stim_residue(id),
            Kind::DamageModifier => {
                if let Some(i) = self.index_of(id) {
                    self.damage_multiplier /= 1.0 + self.effects[i].figures[0];
                }
            }
            _ => {}
        }
        let _ = part;
    }

    fn on_removed(&mut self, effect: &Effect) {
        if effect.kind == Kind::HeavyBleeding {
            self.add(Kind::Wound, effect.part, None, None, None, None);
        }
    }

    /// A kind's default figures (`defaults`), a bleeding's working time
    /// the elite Vitality buff's where it has one (`Bleeding.
    /// DefaultWorkTime`: `EliteVitalityDuration`, if above 0).
    fn defaults(&self, kind: Kind) -> [f32; 4] {
        let mut figures = defaults(kind, &self.settings);
        if kind.bleeding() && self.skills.bleed_stop {
            let s = if kind == Kind::HeavyBleeding { self.settings.heavy } else { self.settings.light };
            if s.elite_duration > 0.0 {
                figures[1] = s.elite_duration;
            }
        }
        figures
    }

    /// Strong dehydration's or exhaustion's damage and how often: no
    /// damage with the elite Metabolism buff (`Dehydration.Damage`,
    /// `Exhaustion.Damage`).
    fn starving(&self, [damage, every]: [f32; 2]) -> [f32; 2] {
        [if self.skills.no_starvation_damage { 0.0 } else { damage }, every]
    }

    /// `Bleeding.ResetParameters`: the figures with or without
    /// dehydration.
    fn reset_bleeding(&mut self, id: u32, dehydrated: bool) {
        let Some(i) = self.index_of(id) else { return };
        let s = if self.effects[i].kind == Kind::HeavyBleeding { self.settings.heavy } else { self.settings.light };
        let default_work = self.defaults(self.effects[i].kind)[1];
        let life = if dehydrated && s.life_dehydrated > 0.0 { s.life_dehydrated.min(default_work) } else { default_work };
        self.add_work_time(id, Some(life), true);
        let Some(i) = self.index_of(id) else { return };
        let e = &mut self.effects[i];
        e.figures[0] = if dehydrated { s.damage_dehydrated } else { s.damage };
        e.figures[1] = if dehydrated { s.loop_dehydrated } else { s.loop_time };
        // Its rate of health, a second (`SetHealthRatesPerSecond`): its
        // damage to each of the seven parts each loop.
        e.figures[2] = -e.figures[0] * Part::ALL.len() as f32 / e.figures[1];
        if e.loops[0] > e.figures[1] {
            e.loops[0] = e.figures[1];
        }
        if e.loops[1] > s.energy_loop {
            e.loops[1] = s.energy_loop;
        }
    }

    fn fatigue_rate(&self, id: u32) -> f32 {
        let strength = self.index_of(id).map_or(1.0, |i| self.effects[i].strength);
        self.settings.fatigue[0] + strength * self.settings.fatigue[1]
    }

    /// Steps every effect by the frame's time (`ManualUpdate`).
    pub fn step(&mut self, dt: f32) {
        self.clock += dt;
        let ids: Vec<u32> = self.effects.iter().map(|e| e.id).collect();
        for id in ids {
            let Some(i) = self.index_of(id) else { continue };
            if matches!(self.effects[i].stage, Stage::None | Stage::Removed) {
                continue;
            }
            self.update_effect(id, dt);
        }
    }

    fn update_effect(&mut self, id: u32, dt: f32) {
        let Some(i) = self.index_of(id) else { return };
        let e = &mut self.effects[i];
        let mut dt = dt;
        if e.state_current + dt >= e.state_time {
            dt = e.state_time - e.state_current;
            e.state_current = e.state_time;
        } else {
            e.state_current += dt;
        }
        e.whole += dt;
        match e.stage {
            Stage::Added => {
                if e.state_current >= e.state_time {
                    self.next_state(id);
                }
            }
            Stage::Started => {
                if dt > 0.0 {
                    self.regular_update(id, dt);
                }
                if self.index_of(id).is_some_and(|i| self.effects[i].stage == Stage::Started && self.effects[i].state_current >= self.effects[i].state_time) {
                    self.next_state(id);
                }
            }
            Stage::Residual => {
                if self.index_of(id).is_some_and(|i| self.effects[i].stage == Stage::Residual && self.effects[i].state_current >= self.effects[i].state_time) {
                    self.next_state(id);
                }
            }
            Stage::None | Stage::Removed => {}
        }
        if let Some(i) = self.index_of(id) {
            self.effects[i].calculate_strength(dt);
        }
    }

    fn regular_update(&mut self, id: u32, dt: f32) {
        let Some(i) = self.index_of(id) else { return };
        let (kind, part) = (self.effects[i].kind, self.effects[i].part);
        match kind {
            Kind::LightBleeding | Kind::HeavyBleeding => {
                let s = if kind == Kind::HeavyBleeding { self.settings.heavy } else { self.settings.light };
                let e = &mut self.effects[i];
                e.loops[0] += dt;
                let (damage, every) = (e.figures[0], e.figures[1]);
                let bleed = e.loops[0] >= every;
                if bleed {
                    e.loops[0] -= every;
                }
                e.loops[1] += dt;
                let drain = e.loops[1] >= s.energy_loop;
                if drain {
                    e.loops[1] -= s.energy_loop;
                }
                if bleed {
                    let as_kind = if kind == Kind::HeavyBleeding { DamageKind::HeavyBleeding } else { DamageKind::LightBleeding };
                    let mut taken = 0.0;
                    for p in Part::ALL {
                        taken += self.apply_damage(p, damage, as_kind);
                    }
                    // `BleedHealth`: its rate becomes what the loop took
                    // from the whole, where that differs.
                    let rate = -taken / every;
                    let multiplier = self.damage_multiplier;
                    if let Some(i) = self.index_of(id) {
                        if (self.effects[i].figures[2] - rate).abs() >= f32::EPSILON {
                            self.effects[i].figures[2] = rate / multiplier;
                        }
                    }
                }
                if drain {
                    self.change_energy(-s.energy_damage);
                    if self.energy[0] <= self.energy[1] {
                        self.add(Kind::Exhaustion, Part::Head, None, None, None, None);
                    }
                }
            }
            Kind::Exhaustion => {
                let [damage, every] = self.starving(self.settings.exhaustion_damage);
                let e = &mut self.effects[i];
                e.loops[0] += dt;
                if e.loops[0] >= every {
                    e.loops[0] -= every;
                    for p in Part::ALL {
                        self.apply_damage(p, damage, DamageKind::Exhaustion);
                    }
                }
                if self.energy[0] > self.energy[1] {
                    self.force_residue(id);
                }
            }
            Kind::Dehydration => {
                let [damage, every] = self.starving(self.settings.dehydration_damage);
                let e = &mut self.effects[i];
                e.loops[0] += dt;
                if e.loops[0] < every {
                    return;
                }
                if self.hydration[0] > 0.0 {
                    self.force_residue(id);
                    return;
                }
                if let Some(i) = self.index_of(id) {
                    self.effects[i].loops[0] -= every;
                }
                for p in Part::ALL {
                    self.apply_damage(p, damage, DamageKind::Dehydration);
                }
            }
            Kind::Existence => {
                let s = self.settings.existence;
                let stomach_gone = self.part(Part::Stomach).destroyed;
                let (energy_loop, hydration_loop) = if stomach_gone { (s[1] / s[4], s[3] / s[5]) } else { (s[1], s[3]) };
                let e = &mut self.effects[i];
                e.loops[0] += dt;
                let eat = e.loops[0] >= energy_loop && !self.metabolism_disabled;
                if eat {
                    e.loops[0] -= energy_loop;
                }
                e.loops[1] += dt;
                let drink = e.loops[1] >= hydration_loop && !self.metabolism_disabled;
                if drink {
                    e.loops[1] -= hydration_loop;
                }
                // Cut by the skills' buffs (`HealthEnergy`, `HealthHydration`).
                let (energy_cut, hydration_cut) = (1.0 - self.skills.health_energy, 1.0 - self.skills.health_hydration);
                if eat {
                    self.change_energy(-s[0] * energy_cut);
                }
                if drink {
                    self.change_hydration(-s[2] * hydration_cut);
                }
            }
            Kind::Pain => {
                let delay = self.settings.pain_tremor_delay;
                let e = &mut self.effects[i];
                if let Some(t) = e.tremor_after.as_mut() {
                    *t += dt;
                    if *t >= delay {
                        e.tremor_after = None;
                        self.add_side(id, Kind::Tremor, Part::Head, None);
                    }
                }
            }
            Kind::LowHealth => {
                if self.common()[0] >= self.settings.low_health_common {
                    self.force_residue(id);
                }
            }
            Kind::ChronicFatigue => {
                let tick = self.settings.fatigue[2];
                if self.clock > self.effects[i].figures[1] {
                    let rate = self.fatigue_rate(id);
                    self.change_energy(-rate * tick);
                    if let Some(i) = self.index_of(id) {
                        self.effects[i].figures[1] = self.clock + tick;
                    }
                }
            }
            Kind::MedEffect => self.med_update(id, dt),
            Kind::Stimulator => self.stim_update(id, dt),
            Kind::Wound | Kind::Fracture | Kind::Tremor | Kind::PainKiller | Kind::Encumbered | Kind::OverEncumbered | Kind::Contusion | Kind::Stun | Kind::Flash | Kind::Endurance | Kind::DamageModifier | Kind::TunnelVision => {}
        }
        let _ = part;
    }

    fn change_health(&mut self, part: Part, value: f32) -> f32 {
        if !self.alive {
            return 0.0;
        }
        let p = &mut self.parts[part.index()];
        if p.destroyed {
            return 0.0;
        }
        let before = p.current;
        p.current = (p.current + value).clamp(0.0, p.maximum);
        p.current - before
    }

    /// `ChangeEnergy`.
    pub fn change_energy(&mut self, value: f32) {
        if !self.alive {
            return;
        }
        let before = self.energy[0];
        self.energy[0] = (self.energy[0] + value).clamp(self.energy[1], self.energy[2]);
        if self.energy[0] != before && self.energy[0] <= self.energy[1] && !self.metabolism_disabled {
            self.add(Kind::Exhaustion, Part::Head, None, None, None, None);
        }
    }

    /// `ChangeHydration`.
    pub fn change_hydration(&mut self, value: f32) {
        if !self.alive {
            return;
        }
        let before = self.hydration[0];
        self.hydration[0] = (self.hydration[0] + value).clamp(self.hydration[1], self.hydration[2]);
        if self.hydration[0] != before && self.hydration[0] <= self.hydration[1] && !self.metabolism_disabled {
            self.add(Kind::Dehydration, Part::Stomach, None, None, None, None);
        }
    }

    fn kill(&mut self, kind: DamageKind) {
        if self.alive {
            self.alive = false;
            self.cause = Some(kind);
            self.self_damage = None;
            self.events.push(Event::Died(kind));
        }
    }

    /// `DestroyBodyPart`: the part marked destroyed, its points as they are.
    pub fn destroy(&mut self, part: Part, kind: DamageKind) {
        if !self.parts[part.index()].destroyed {
            self.parts[part.index()].destroyed = true;
            self.events.push(Event::Destroyed(part, kind));
        }
        self.kill_after_destroyed(part, kind);
    }

    fn kill_after_destroyed(&mut self, part: Part, kind: DamageKind) {
        if !kind.bleeding() && part.vital() {
            self.kill(kind);
        }
    }

    /// Damage to a part (`ApplyDamage`); returns how much common health
    /// was lost. `bias` is what a round adds to the odds of a bleeding
    /// (`HeavyBleedingDelta`, `LightBleedingDelta`); 0 for anything else.
    pub fn apply_damage(&mut self, part: Part, damage: f32, kind: DamageKind) -> f32 {
        self.apply_damage_biased(part, damage, kind, [0.0, 0.0])
    }

    pub fn apply_damage_biased(&mut self, part: Part, damage: f32, kind: DamageKind, bias: [f32; 2]) -> f32 {
        self.apply_damage_info(part, damage, kind, bias, false)
    }

    /// A character taken up as it stands (a replay's victim): each part's
    /// health now and at most, a part at nought destroyed.
    pub fn take_up_parts(&mut self, current: [f32; 7], maximum: [f32; 7]) {
        for k in 0..7 {
            self.parts[k] = PartState { current: current[k], maximum: maximum[k], destroyed: current[k] <= 0.0 };
        }
    }

    /// `ApplyDamage` with the hit's figures (`DamageInfo`): the round's
    /// bleedings' deltas, and whether light armour blocks a bleeding
    /// (`BleedBlock`, body hits spec).
    pub fn apply_damage_info(&mut self, part: Part, damage: f32, kind: DamageKind, bias: [f32; 2], bleed_block: bool) -> f32 {
        if !self.alive {
            return 0.0;
        }
        let mut damage = damage;
        if self.damage_multiplier > 1.0 || part != Part::Head || !kind.enemy() {
            damage *= self.damage_multiplier;
        }
        if kind.environmental() {
            damage *= self.settings.environment[part.index()];
        }
        let before = self.parts[part.index()].current;
        let common_before = self.common()[0];
        self.change_health(part, -damage);
        self.events.push(Event::Damaged(part, damage, kind));
        let state = self.parts[part.index()];
        if !state.destroyed && state.current <= 0.0 {
            self.destroy(part, kind);
        }
        if self.parts[part.index()].destroyed {
            self.kill_after_destroyed(part, kind);
        }
        if !kind.self_inflicted() {
            let over = (damage - before).max(0.0);
            if over > 0.0 {
                let others: f32 = Part::ALL.iter().filter(|p| **p != part && !self.part(**p).destroyed).map(|p| self.part(*p).maximum).sum();
                let spread = over * self.settings.over_damage_factor[part.index()];
                for other in Part::ALL {
                    let o = self.parts[other.index()];
                    if other != part && !o.destroyed && others > 0.0 {
                        let share = (-spread * o.maximum / others * self.settings.over_damage_received[other.index()]).min(0.0);
                        self.change_health(other, share);
                        if self.parts[other.index()].current <= 0.0 {
                            self.destroy(other, kind);
                        }
                    }
                }
            }
            let maximum = self.parts[part.index()].maximum;
            if damage >= 1.0 && kind != DamageKind::Barbed {
                self.do_pain(part, 0.0, 0.0, (15.0 * damage / maximum).clamp(1.0, 10.0), (damage / maximum * 2.0).clamp(0.33, 2.0));
            }
            let x = damage / maximum;
            // A stimulator at work may stop blood loss; light armour may
            // block it. The odds' top raised by the Vitality skill's buff.
            let top = 1.0 + self.skills.bleed_chance;
            if bleed_block || self.blood_loss_blocked() {
            } else if (kind.weapon() || kind == DamageKind::GrenadeFragment) && self.chance(Chance::HeavyBleeding, part, self.settings.heavy.odds, x, top, bias[0]) {
                self.do_bleed(true, part);
            } else if self.existing(Kind::HeavyBleeding, Some(part)).is_none() && self.chance(Chance::LightBleeding, part, self.settings.light.odds, x, top, bias[1]) {
                self.do_bleed(false, part);
            }
            let odds = if kind == DamageKind::Fall { self.settings.fracture_falling } else { self.settings.fracture_hit };
            if self.chance(Chance::Fracture, part, odds, x, 1.0 + self.skills.break_chance, 0.0) {
                self.do_fracture(part);
            }
        }
        let common = self.common()[0];
        if common <= 0.0 {
            self.kill(kind);
        } else if common <= self.settings.low_health_common {
            self.add(Kind::LowHealth, Part::Head, None, None, None, None);
        }
        common_before - self.common()[0]
    }

    fn do_pain(&mut self, part: Part, delay: f32, work: f32, residue: f32, strength: f32) {
        if self.alive {
            self.add(Kind::Pain, part, Some(delay), Some(work), Some(residue), Some(strength));
        }
    }

    /// `DoBleed`: one bleeding a part, the heavier replacing the lighter.
    pub fn do_bleed(&mut self, heavy: bool, part: Part) {
        let kind = if heavy { Kind::HeavyBleeding } else { Kind::LightBleeding };
        if !self.alive || self.existing(kind, Some(part)).is_some() {
            return;
        }
        for other in [Kind::LightBleeding, Kind::HeavyBleeding, Kind::Wound] {
            if let Some(id) = self.existing_id(other, Some(part)) {
                self.force_remove(id);
            }
        }
        self.add(kind, part, None, None, None, None);
    }

    /// `DoContusion`: a contusion on the head for so many seconds, from
    /// 0.1 s on, fading for half of them (health spec, "A contusion").
    pub fn do_contusion(&mut self, time: f32, strength: f32) {
        if self.alive {
            self.add(Kind::Contusion, Part::Head, Some(0.1), Some(time), Some(time / 2.0), Some(strength));
        }
    }

    /// `BluntContusion` (after a hit's damage, `Player.ApplyDamageInfo`):
    /// a hit to the head whose armour took 10 or more off it concusses,
    /// alive: a contusion on the head after 0.1 s, at half strength,
    /// working a quarter of what the armour took and fading over as long
    /// (health spec, "A blunt hit's concussion").
    pub fn blunt_contusion(&mut self, part: Part, absorbed: f32) {
        if self.alive && part == Part::Head && absorbed >= 10.0 {
            let quarter = absorbed / 4.0;
            self.add(Kind::Contusion, Part::Head, Some(0.1), Some(quarter), Some(quarter), Some(0.5));
        }
    }

    /// `DoStun`: a stun on the head at once (grenade spec, "A flash"),
    /// working its build-up and the flash's time by how hard it struck and
    /// the eyes' cover (`BlindnessDuration`), at that strength.
    pub fn do_stun(&mut self, max_time: f32, strength: f32, blindness: f32) {
        if self.alive {
            self.add(Kind::Stun, Part::Head, Some(0.0), Some(0.2 + max_time * strength * blindness), None, Some(strength));
        }
    }

    /// `DoBurnEyes`: the eyes burnt, a `Flash` on the head at once working
    /// its build-up and the flash's time by the distance's share and the
    /// eyes' cover twice, at the strength the flash struck with. Whether it
    /// was added (the camera then told, `BurnEyesEvent`), and the distance's
    /// share as the camera is told it.
    pub fn do_burn_eyes(&mut self, distance_strength: f32, strength: f32, max_time: f32, blindness: f32) -> Option<f32> {
        if !self.alive {
            return None;
        }
        let distance_strength = distance_strength * blindness;
        self.add(Kind::Flash, Part::Head, Some(0.0), Some(0.2 + max_time * distance_strength * blindness), None, Some(strength));
        Some(distance_strength)
    }

    /// `DoFracture`: an arm or a leg, not destroyed, breaks at once.
    pub fn do_fracture(&mut self, part: Part) {
        if self.alive && part.limb() && !self.part(part).destroyed {
            self.add(Kind::Fracture, part, Some(0.0), None, None, None);
        }
    }

    /// A chance drawn (`ProbabilitySetting.Try`): nothing under its
    /// threshold; else an outcome laid down for it (`outcomes`) if there is
    /// one, else the draw.
    fn chance(&mut self, chance: Chance, part: Part, odds: Odds, x: f32, top: f32, bias: f32) -> bool {
        if x < odds.threshold {
            return false;
        }
        if let Some(at) = self.outcomes.iter().position(|(c, p, _)| *c == chance && *p == part) {
            return self.outcomes.remove(at).2;
        }
        odds.try_at(x, top, bias, &mut self.rng)
    }

    /// A fall (`HandleFall`): the height fallen since last on the ground,
    /// and the weight's multiplier (`Physical.FallDamageMultiplier`).
    /// Returns the damage done to each leg.
    pub fn handle_fall(&mut self, height: f32, weight_multiplier: f32) -> f32 {
        if !self.alive {
            return 0.0;
        }
        let over = height - self.settings.fall_safe_height;
        if over <= 0.0 {
            return 0.0;
        }
        let damage = over * over.sqrt() * self.settings.fall_damage_per_meter * weight_multiplier;
        self.apply_damage(Part::LeftLeg, damage, DamageKind::Fall);
        self.apply_damage(Part::RightLeg, damage, DamageKind::Fall);
        damage
    }

    /// A wound's tally pushed (`DoWoundRelapse`): on one part's wounds, or
    /// on every wound.
    pub fn wound_relapse(&mut self, value: f32, part: Option<Part>) {
        if !self.alive {
            return;
        }
        let wounds: Vec<u32> = self.effects.iter().filter(|e| e.kind == Kind::Wound && e.active() && part.is_none_or(|p| e.part == p)).map(|e| e.id).collect();
        for id in wounds {
            let Some(i) = self.index_of(id) else { continue };
            let wound_part = self.effects[i].part;
            self.effects[i].figures[1] += value;
            let [threshold, tally, ..] = self.effects[i].figures;
            if tally > threshold && !self.blood_loss_blocked() && self.existing(Kind::LightBleeding, Some(wound_part)).is_none() && self.existing(Kind::HeavyBleeding, Some(wound_part)).is_none() {
                self.do_bleed(false, wound_part);
            }
        }
    }

    /// The stamina's fatigue (`AddFatigue`): one more stack.
    pub fn add_fatigue(&mut self) {
        let strength = self.active(Kind::ChronicFatigue, Some(Part::Chest)).map_or(1.0, |e| e.strength + 1.0);
        let work = self.settings.fatigue[3];
        self.add(Kind::ChronicFatigue, Part::Chest, None, Some(work), None, Some(strength));
    }

    /// The weight's two notices (`SetEncumbered`, `SetOverEncumbered`).
    pub fn set_encumbered(&mut self, encumbered: bool, over: bool) {
        for (on, slot, kind) in [(encumbered, 0, Kind::Encumbered), (over, 1, Kind::OverEncumbered)] {
            let held = if slot == 0 { self.encumbered } else { self.over_encumbered };
            let now = match (on, held) {
                (true, None) => Some(self.add(kind, Part::Chest, None, None, None, None)),
                (false, Some(id)) => {
                    self.force_remove(id);
                    None
                }
                (_, held) => held,
            };
            if slot == 0 { self.encumbered = now } else { self.over_encumbered = now }
        }
    }

    /// The sprint's self-damage on a damaged leg (`InflictSelfDamage`),
    /// once it has been started: each frame its count down runs, and in a
    /// sprint at 0 it hurts each damaged leg by 2 and counts again from 1 to
    /// 1.5 s. `sprinting` is whether the movement's state is the sprint.
    pub fn sprint_frame(&mut self, dt: f32, sprinting: bool, conditions: Conditions) {
        let Some(countdown) = self.self_damage.as_mut() else { return };
        *countdown -= dt;
        if *countdown <= 0.0 && sprinting {
            *countdown = self.rng.range(1.0, 1.5);
            if conditions.left_leg {
                self.apply_damage(Part::LeftLeg, 2.0, DamageKind::Fall);
            }
            if conditions.right_leg {
                self.apply_damage(Part::RightLeg, 2.0, DamageKind::Fall);
            }
        }
    }

    /// Starts the sprint's self-damage if it is not running
    /// (`StartInflictSelfDamageCoroutine`).
    pub fn start_self_damage(&mut self) {
        if self.alive && self.self_damage.is_none() {
            self.self_damage = Some(0.0);
        }
    }

    /// A jump landed (`MovementContextOnStateChanged`, leaving the jump):
    /// each damaged leg hurt by 3.
    pub fn jump_landed(&mut self, conditions: Conditions) {
        if conditions.left_leg {
            self.apply_damage(Part::LeftLeg, 3.0, DamageKind::Fall);
        }
        if conditions.right_leg {
            self.apply_damage(Part::RightLeg, 3.0, DamageKind::Fall);
        }
    }

    /// A vault begun (`ApplyDamageByVaulting`): the left leg, damaged, is
    /// hurt by 3 over a height above `without_hand`.
    pub fn vaulted(&mut self, height: f32, without_hand: f32, conditions: Conditions) {
        if conditions.left_leg && height >= without_hand {
            self.apply_damage(Part::LeftLeg, 3.0, DamageKind::Fall);
        }
    }

    /// The frame after the movement's (`Player`'s handlers of both): what
    /// the movement did comes in (falls, jumps, sprints, vaults, the
    /// stamina's fatigue), the effects are stepped, and what health allows
    /// goes out to the movement and its stamina.
    pub fn follow(&mut self, dt: f32, movement: &mut crate::Movement) {
        use crate::movement::Moved;
        let before = self.conditions();
        let overweight = movement.stamina.settings.overweight.clamp(0.0, 1.0);
        let vaulting = movement.settings.vaulting;
        for moved in movement.take_moved() {
            match moved {
                Moved::Fell(height) => {
                    let weight = 1.0 + (self.settings.fall_overweight - 1.0) * overweight;
                    self.handle_fall(height, weight);
                }
                Moved::JumpStarted => self.wound_relapse(1.0, None),
                Moved::JumpEnded => self.jump_landed(before),
                Moved::SprintStarted => {
                    if before.leg_damaged() {
                        self.start_self_damage();
                    }
                }
                Moved::SprintEnded(distance) => self.wound_relapse(distance / 10.0, None),
                Moved::Vaulted(height) => self.vaulted(height, vaulting.vault.max_without_hand, before),
            }
        }
        for _ in 0..movement.stamina.take_fatigues() {
            self.add_fatigue();
        }
        self.sprint_frame(dt, movement.state == crate::State::Sprint, before);
        self.step(dt);
        let now = self.conditions();
        movement.set_conditions(now);
        if now.leg_damaged() && !now.on_painkillers && movement.state == crate::State::Sprint {
            self.start_self_damage();
        }
        let [common, most] = self.common();
        movement.stamina.set_health(self.energy_share(), self.hydration_share(), if most > 0.0 { common / most } else { 0.0 });
        // Stimulators' capacity and refill (health spec, "Buffs").
        let (capacity, refill) = self.stamina_buffs();
        movement.stamina.set_buffs(capacity, refill);
    }

    // ---- Medicine, food and drink -------------------------------------

    /// Whether the part can be helped by the item, with `resource` left in
    /// it (`BodyPartHealHelper.CanApply`).
    fn helps(&self, m: &Medicine, resource: f32, part: Part) -> bool {
        let state = self.part(part);
        let affordable = |cure: Cure| m.cure(cure).is_some_and(|spec| m.food() || spec.cost <= resource);
        if !state.destroyed {
            if state.current < state.maximum && !m.food() && m.hp_resource_rate > 0.0 && resource > 0.0 {
                return true;
            }
        } else if !part.vital() && affordable(Cure::DestroyedPart) {
            return true;
        }
        self.effects.iter().filter(|e| e.part == part && e.active()).filter_map(|e| cure_of(e.kind)).any(affordable)
    }

    /// The cures the item would do on a part, as far as `resource` pays,
    /// and whether it would heal hit points (`BodyPartHealHelper.UpdateCache`).
    fn cures_on(&self, m: &Medicine, resource: f32, part: Part) -> (bool, Vec<Cure>) {
        let state = self.part(part);
        let mut cures: Vec<Cure> = self.effects.iter().filter(|e| e.part == part && e.active()).filter_map(|e| cure_of(e.kind)).filter(|c| m.cure(*c).is_some()).collect();
        if state.destroyed && !part.vital() && m.cure(Cure::DestroyedPart).is_some() {
            cures.push(Cure::DestroyedPart);
        }
        cures.sort();
        if m.food() {
            return (false, cures);
        }
        let mut left = resource;
        let mut heals = false;
        if !state.destroyed && state.current < state.maximum && m.hp_resource_rate > 0.0 && left > 0.0 {
            heals = true;
            left -= (state.maximum - state.current).min(m.hp_resource_rate);
        }
        let mut kept = Vec::new();
        for cure in cures {
            let cost = m.cure(cure).map_or(0.0, |s| s.cost);
            if left >= cost {
                left -= cost;
                kept.push(cure);
            }
        }
        (heals, kept)
    }

    /// The parts the item can help, best first (`BodyPartsPriority`,
    /// `BodyPartHealHelper.CompareTo`, `PrioritizeBodyParts`).
    pub fn parts_to_help(&self, m: &Medicine, resource: f32, only: Option<Part>) -> Vec<Part> {
        struct Ranked {
            part: Part,
            can: bool,
            cures: Vec<Cure>,
            destroyed: bool,
            share: f32,
        }
        let mut ranked: Vec<Ranked> = Part::ALL
            .into_iter()
            .map(|part| {
                if only.is_some_and(|o| o != part) {
                    return Ranked { part, can: false, cures: Vec::new(), destroyed: false, share: 1.0 };
                }
                let (heals, cures) = self.cures_on(m, resource, part);
                let s = self.part(part);
                Ranked { part, can: heals || !cures.is_empty(), cures, destroyed: s.destroyed, share: if s.maximum > 0.0 { s.current / s.maximum } else { 0.0 } }
            })
            .collect();
        ranked.sort_by(|a, b| {
            use std::cmp::Ordering;
            if a.can != b.can {
                return b.can.cmp(&a.can);
            }
            let (ha, hb) = (!a.cures.is_empty(), !b.cures.is_empty());
            if ha != hb {
                return hb.cmp(&ha);
            }
            if ha {
                for (x, y) in a.cures.iter().zip(&b.cures) {
                    if x != y {
                        return x.cmp(y);
                    }
                }
                if a.cures.len() != b.cures.len() {
                    return b.cures.len().cmp(&a.cures.len());
                }
                if a.destroyed != b.destroyed {
                    return a.destroyed.cmp(&b.destroyed);
                }
            }
            if (a.share - b.share).abs() >= 0.05 {
                return a.share.partial_cmp(&b.share).unwrap_or(Ordering::Equal);
            }
            a.part.cmp(&b.part)
        });
        if !m.part_priority.is_empty() {
            let rank = |p: Part| m.part_priority.iter().position(|q| *q == p).unwrap_or(m.part_priority.len());
            ranked.sort_by_key(|r| rank(r.part));
        }
        ranked.into_iter().filter(|r| r.can).map(|r| r.part).collect()
    }

    /// Whether the item may be used now at all (`ReadyToApplyItem`, health's
    /// part; the hands being busy is the host's).
    pub fn ready(&self) -> Result<(), Refused> {
        if !self.alive {
            return Err(Refused::Dead);
        }
        if self.existing(Kind::MedEffect, None).is_some() {
            return Err(Refused::Incompatible);
        }
        Ok(())
    }

    /// Whether using the item would do anything (`HasPartsToApply`).
    pub fn has_parts_to_apply(&self, m: &Medicine, resource: f32) -> Result<(), Refused> {
        self.ready()?;
        if m.class == MedClass::Drugs || m.buffs.is_some() {
            return Ok(());
        }
        if m.food() {
            return if resource > 0.0 { Ok(()) } else { Err(Refused::Depleted) };
        }
        if Part::ALL.into_iter().any(|p| self.helps(m, resource, p)) { Ok(()) } else { Err(Refused::Incompatible) }
    }

    /// The part the inventory's "Use" names (`ItemUiContext.HealPriority`:
    /// `BodyPartsPriority` with the game's continuous healing off, as the
    /// install has it): the common body (`None`), unless the item has a
    /// priority of parts of its own (on its medkit's part, so with points
    /// to give) and is not of the medkits' class: then the first part it
    /// can help, if that is one of its own, and the common body if not.
    pub fn use_part(&self, m: &Medicine, resource: f32) -> Option<Part> {
        if m.part_priority.is_empty() || m.max_hp_resource <= 0.0 || m.class == MedClass::MedKit {
            return None;
        }
        let first = self.parts_to_help(m, resource, None).first().copied()?;
        m.part_priority.contains(&first).then_some(first)
    }

    /// The part the item goes to (`TryGetBodyPartToApply`): `asked` is
    /// the part it was dropped on, if any.
    pub fn part_to_apply(&self, m: &Medicine, resource: f32, asked: Option<Part>) -> Option<Part> {
        self.ready().ok()?;
        if m.food() && resource > 0.0 && asked.is_none_or(|p| p == Part::Head) {
            return Some(Part::Head);
        }
        if m.class == MedClass::Drugs && asked.is_some() {
            return asked;
        }
        let found = self.parts_to_help(m, resource, asked).first().copied();
        if m.class == MedClass::Drugs {
            return Some(found.unwrap_or(Part::Head));
        }
        if m.buffs.is_some() {
            return Some(Part::Head);
        }
        found
    }

    /// Starts the item at work on a part (`DoMedEffect`): returns the
    /// effect's number, or nothing if it can do nothing there.
    pub fn use_item(&mut self, m: Arc<Medicine>, item: u64, resource: f32, asked: Option<Part>, amount: Option<f32>) -> Option<u32> {
        let part = self.part_to_apply(&m, resource, asked)?;
        let delay = m.start_delay(&self.settings);
        if m.food() {
            let amount = amount.unwrap_or(if m.max_resource > 0.0 { resource / m.max_resource } else { 1.0 });
            let work = m.use_time * amount;
            return Some(self.add_med(m, item, resource, part, delay, work, amount));
        }
        let state = *self.part(part);
        let mut work = m.use_time_on(part);
        if state.maximum - state.current < 10.0 {
            work /= 1.0 + 0.2;
        }
        // The Surgery skill's speed, for a destroyed part the item mends.
        if state.destroyed && m.cure(Cure::DestroyedPart).is_some() {
            work /= 1.0 + self.skills.surgery_speed;
        }
        Some(self.add_med(m, item, resource, part, delay, work, 1.0))
    }

    #[allow(clippy::too_many_arguments)]
    fn add_med(&mut self, m: Arc<Medicine>, item: u64, resource: f32, part: Part, delay: f32, work: f32, amount: f32) -> u32 {
        let no_move = {
            let legs = matches!(part, Part::LeftLeg | Part::RightLeg);
            (legs && m.cure(Cure::Fracture).is_some() && self.broken(part)) || m.cure(Cure::DestroyedPart).is_some()
        };
        let med = MedWork {
            medicine: m,
            item,
            amount,
            resource,
            health_points: 0.0,
            energy_points: 0.0,
            hydration_points: 0.0,
            energy_gained: 0.0,
            hydration_gained: 0.0,
            loop_time: 0.0,
            interrupted: false,
            no_move,
        };
        self.add_with(Kind::MedEffect, part, Some(delay), Some(work), None, None, move |e| e.med = Some(Box::new(med)))
    }

    /// Stops what is at work (`RemoveMedEffect`, `CancelApplyingItem`).
    pub fn cancel_items(&mut self) {
        let ids: Vec<u32> = self.effects.iter().filter(|e| e.kind == Kind::MedEffect && e.existing()).map(|e| e.id).collect();
        for id in ids {
            self.force_residue(id);
        }
    }

    fn med_started(&mut self, id: u32) {
        let Some(i) = self.index_of(id) else { return };
        let Some(med) = self.effects[i].med.as_mut() else { return };
        let m = med.medicine.clone();
        if !m.food() {
            med.health_points = m.hp_resource_rate * med.amount;
        }
        // `CutPiece`: the item's value times the share used; food's and
        // drink's gains raised by the Metabolism skill, their losses cut by
        // the Immunity skill's.
        med.energy_points = m.energy * med.amount;
        med.hydration_points = m.hydration * med.amount;
        if m.food() {
            let (gain, loss) = (1.0 + self.skills.metabolism_ratio_plus, 1.0 - self.skills.immunity_misc);
            for points in [&mut med.energy_points, &mut med.hydration_points] {
                *points *= if *points > 0.0 { gain } else if *points < 0.0 { loss } else { 1.0 };
            }
        }
    }

    fn med_update(&mut self, id: u32, dt: f32) {
        let loop_time = self.settings.med_loop;
        let Some(i) = self.index_of(id) else { return };
        let (part, work, left) = (self.effects[i].part, self.effects[i].work, self.effects[i].state_left());
        let Some(med) = self.effects[i].med.as_mut() else { return };
        med.loop_time += dt;
        if med.loop_time < loop_time && left > 0.0 {
            return;
        }
        let step = if med.loop_time >= loop_time { loop_time } else { med.loop_time };
        med.loop_time -= step;
        let m = med.medicine.clone();
        let mut events = Vec::new();
        if m.food() {
            let piece = m.max_resource * med.amount * step / work;
            if med.resource > 0.0 {
                med.resource = (med.resource - piece).max(0.0);
                events.push(Event::Resource(med.item, med.resource));
            }
        }
        let (health_points, energy_points, hydration_points) = (med.health_points, med.energy_points, med.hydration_points);
        let mut resource = med.resource;
        let state = *self.part(part);
        if health_points != 0.0 {
            let mut give = (health_points * step / work).min(state.maximum - state.current);
            if !m.food() {
                give = give.min(resource);
            }
            if give != 0.0 && !state.destroyed {
                self.change_health(part, give);
                if !m.food() {
                    resource -= give;
                    events.push(Event::Resource(0, resource));
                }
            }
        }
        if energy_points != 0.0 {
            let gain = (energy_points * step / work).min(self.energy[2] - self.energy[0]);
            if gain != 0.0 {
                self.change_energy(gain);
                if let Some(med) = self.index_of(id).and_then(|i| self.effects[i].med.as_mut()) {
                    med.energy_gained += gain;
                }
            }
        }
        if hydration_points != 0.0 {
            let gain = (hydration_points * step / work).min(self.hydration[2] - self.hydration[0]);
            if gain != 0.0 {
                self.change_hydration(gain);
                if let Some(med) = self.index_of(id).and_then(|i| self.effects[i].med.as_mut()) {
                    med.hydration_gained += gain;
                }
            }
        }
        if let Some(med) = self.index_of(id).and_then(|i| self.effects[i].med.as_mut()) {
            if !m.food() {
                med.resource = resource;
            }
            let item = med.item;
            for event in events {
                self.events.push(match event {
                    Event::Resource(_, value) => Event::Resource(item, value),
                    other => other,
                });
            }
        }
    }

    fn med_residue(&mut self, id: u32) {
        let interruption = self.settings.interruption_time;
        let Some(i) = self.index_of(id) else { return };
        let (part, whole, delay) = (self.effects[i].part, self.effects[i].whole, self.effects[i].delay);
        let Some(med) = self.effects[i].med.clone() else { return };
        let m = med.medicine.clone();
        let kit = !m.food();
        let mut resource = med.resource;
        if !med.interrupted {
            for cure in Cure::ALL {
                let Some(spec) = m.cure(cure).copied() else { continue };
                if kit && resource < spec.cost {
                    continue;
                }
                let mut done = false;
                match cure {
                    Cure::Pain => {
                        // A painkiller on the head: its delay, its duration
                        // times the share used and raised by the Immunity
                        // skill's buff, its fade.
                        let template = m.template.clone();
                        let duration = spec.duration * med.amount * (1.0 + self.skills.immunity_painkiller);
                        self.add_by_item(Kind::PainKiller, Part::Head, &template, med.amount, spec.delay, duration, spec.fade_out, spec.duration);
                        done = true;
                    }
                    Cure::DestroyedPart => {
                        let kept = whole_range(&mut self.rng, spec.kept[0], spec.kept[1]) as f32 / 100.0;
                        done = self.restore(part, kept);
                    }
                    other => {
                        let on = match other {
                            Cure::Contusion => Some(Part::Head),
                            Cure::Intoxication | Cure::LethalIntoxication | Cure::RadExposure => Some(Part::Stomach),
                            _ => Some(part),
                        };
                        if let (Some(kind), Some(on)) = (other.effect(), on) {
                            if let Some(found) = self.active_id(kind, Some(on)) {
                                self.force_residue(found);
                                done = true;
                            }
                        }
                    }
                }
                if done && kit {
                    resource -= spec.cost;
                }
            }
            // The item's set of buffs, at the share used (`MedEffect.Residue`:
            // only a use not cut short; health spec, "Buffs").
            if let Some(set) = m.buffs.as_deref() {
                self.add_stimulator(&m.template, set, med.amount);
            }
            if kit && m.hp_resource_rate == 0.0 {
                resource = (resource - 1.0).round();
            }
        } else {
            let worked = whole - delay;
            if kit && worked > interruption && m.hp_resource_rate == 0.0 {
                resource = (resource - 1.0).round();
            }
            if worked < interruption {
                self.change_energy(-med.energy_gained);
                self.change_hydration(-med.hydration_gained);
            }
        }
        resource = resource.floor();
        self.events.push(Event::Resource(med.item, resource));
        if let Some(i) = self.index_of(id) {
            if let Some(w) = self.effects[i].med.as_mut() {
                w.resource = resource;
            }
        }
        if resource <= 0.0 {
            self.events.push(Event::UsedUp(med.item));
        }
    }

    /// Mends a destroyed part (`RestoreBodyPart`): back to 1 point, its
    /// maximum that share of the old, drawn toward all by the Surgery
    /// skill's buff.
    fn restore(&mut self, part: Part, kept: f32) -> bool {
        if part.vital() || !self.parts[part.index()].destroyed {
            return false;
        }
        let kept = kept + (1.0 - kept) * self.skills.surgery_penalty;
        let p = &mut self.parts[part.index()];
        p.destroyed = false;
        p.maximum = (p.maximum * kept).ceil().max(1.0);
        p.current = 1.0f32.min(p.maximum);
        self.events.push(Event::Restored(part));
        true
    }

    /// What an item at work holds now, if it is at work.
    pub fn item_at_work(&self) -> Option<&MedWork> {
        self.effects.iter().filter(|e| e.kind == Kind::MedEffect && e.existing()).find_map(|e| e.med.as_deref())
    }
}

fn share(value: [f32; 3]) -> f32 {
    let [current, least, most] = value;
    if most > least { (current - least) / (most - least) } else { 1.0 }
}

/// The cure an effect is cured by (`HealthHelper.TryGetDamageEffectType`).
fn cure_of(kind: Kind) -> Option<Cure> {
    Some(match kind {
        Kind::HeavyBleeding => Cure::HeavyBleeding,
        Kind::LightBleeding => Cure::LightBleeding,
        Kind::Fracture => Cure::Fracture,
        Kind::Pain => Cure::Pain,
        Kind::Contusion => Cure::Contusion,
        _ => return None,
    })
}

/// A body-part collider's tally in barbed wire (`BodyPartCollider.
/// ProceedBarb`): 2 a metre the collider moves, done in whole points.
#[derive(Debug, Clone, Copy, Default)]
pub struct BarbTally {
    last: Option<[f32; 3]>,
    tally: f32,
}

impl BarbTally {
    /// The collider is in the wire at `at` this step: the damage due, if a
    /// whole point is.
    pub fn cut(&mut self, at: [f32; 3]) -> Option<f32> {
        let moved = self.last.map_or(f32::INFINITY, |l| ((at[0] - l[0]).powi(2) + (at[1] - l[1]).powi(2) + (at[2] - l[2]).powi(2)).sqrt());
        self.last = Some(at);
        if moved < 0.5 {
            self.tally += moved * 2.0;
            if self.tally >= 1.0 {
                let damage = self.tally;
                self.tally %= 1.0;
                return Some(damage);
            }
        } else {
            self.tally = 0.0;
        }
        None
    }

    /// The collider out of the wire: it keeps where it was, as the game's
    /// does, and its tally.
    pub fn moved_to(&mut self, at: [f32; 3]) {
        self.last = Some(at);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn settings() -> Arc<HealthSettings> {
        let odds = |k: f32, b: f32, threshold: f32, power: f32| Odds { power, k, b, threshold };
        let bleeding = |damage: f32, loop_time: f32, energy_loop: f32, odds: Odds| BleedingSettings {
            delay: 0.4,
            residue: 0.4,
            damage,
            loop_time,
            damage_dehydrated: 0.4,
            loop_dehydrated: 6.0,
            life_dehydrated: 30.0,
            energy_damage: 0.5,
            energy_loop,
            elite_duration: 22.0,
            odds,
        };
        // This install's figures (`globals.json`, 2026-10-14).
        Arc::new(HealthSettings {
            maximum: [35.0, 85.0, 70.0, 60.0, 60.0, 65.0, 65.0],
            environment: [0.125, 0.25, 0.17, 0.5, 0.5, 0.5, 0.5],
            over_damage_received: [0.7; 7],
            over_damage_factor: [1.0, 1.0, 1.5, 0.7, 0.7, 1.0, 1.0],
            energy: [0.0, 100.0, 100.0],
            hydration: [0.0, 100.0, 100.0],
            temperature: [28.0, 40.0, 36.6],
            poisoning: [0.0, 100.0, 0.0],
            light: bleeding(0.8, 6.0, 6.0, odds(0.5, -0.125, 0.35, 1.0)),
            heavy: bleeding(0.9, 4.0, 5.0, odds(0.45, -0.17, 0.5, 1.0)),
            fracture: Timing { delay: 5.0, residue: 5.0 },
            fracture_falling: odds(1.0, 0.38, 0.2, 1.0),
            fracture_hit: odds(0.3, 0.05, 0.3, 0.5),
            dehydration: Timing { delay: 50.0, residue: 5.0 },
            dehydration_damage: [1.0, 15.0],
            exhaustion: Timing { delay: 30.0, residue: 5.0 },
            exhaustion_damage: [1.0, 5.0],
            existence: [3.2, 60.0, 2.6, 60.0, 5.0, 5.0],
            low_health: Timing { delay: 0.4, residue: 5.0 },
            low_health_common: 130.0,
            med_start: [2.0, 0.0, 1.0, 2.0, 0.0, 0.0, 0.0],
            med_loop: 0.4,
            pain_tremor_delay: 30.0,
            tremor: Timing { delay: 15.0, residue: 0.2 },
            wound: [25.0, 60.0, 480.0],
            fatigue: [0.0001, 0.0063, 10.0, 180.0],
            fall_safe_height: 3.0,
            fall_damage_per_meter: 9.0,
            fall_overweight: 3.9,
            interruption_time: 2.0,
            stimulator: StimulatorSettings::default(),
        })
    }

    fn run(h: &mut Health, seconds: f32) {
        let steps = (seconds * 60.0).round() as usize;
        for _ in 0..steps {
            h.step(1.0 / 60.0);
        }
    }

    #[test]
    fn a_run_starts_full_with_existence() {
        let h = Health::new(settings(), 1);
        assert_eq!(h.common(), [440.0, 440.0]);
        assert_eq!(h.energy[0], 100.0);
        assert!(h.active(Kind::Existence, Some(Part::Head)).is_some());
        assert_eq!(h.conditions(), Conditions::default());
    }

    #[test]
    fn hunger_and_thirst_take_their_steps_a_minute() {
        let mut h = Health::new(settings(), 1);
        run(&mut h, 59.9);
        assert_eq!(h.energy[0], 100.0);
        run(&mut h, 0.2);
        assert!((h.energy[0] - 96.8).abs() < 1e-4, "{}", h.energy[0]);
        assert!((h.hydration[0] - 97.4).abs() < 1e-4, "{}", h.hydration[0]);
    }

    #[test]
    fn falls_hurt_both_legs_above_three_metres() {
        let mut h = Health::new(settings(), 1);
        assert_eq!(h.handle_fall(3.0, 1.0), 0.0);
        let damage = h.handle_fall(5.0, 1.0);
        assert!((damage - 2.0f32.powf(1.5) * 9.0).abs() < 1e-4);
        assert!((h.part(Part::LeftLeg).current - (65.0 - damage)).abs() < 1e-3);
        assert!((h.part(Part::RightLeg).current - (65.0 - damage)).abs() < 1e-3);
        // A seven-metre fall destroys both legs: 72 each, the rest spread.
        let mut h = Health::new(settings(), 1);
        h.handle_fall(7.0, 1.0);
        assert!(h.part(Part::LeftLeg).destroyed && h.part(Part::RightLeg).destroyed);
        assert!(h.alive);
        assert!(h.common()[0] < 440.0 - 130.0);
    }

    /// A contusion on the head: started after 0.1 s, working its time,
    /// fading for half of it; a second is a second (`IStackable`).
    #[test]
    fn a_contusion_works_its_time_and_stacks() {
        let mut h = Health::new(settings(), 1);
        h.do_contusion(10.0, 1.0);
        h.do_contusion(4.0, 1.0);
        let contusions = |h: &Health| h.effects.iter().filter(|e| e.kind == Kind::Contusion).map(|e| (e.part, e.stage)).collect::<Vec<_>>();
        assert_eq!(contusions(&h), [(Part::Head, Stage::Added), (Part::Head, Stage::Added)]);
        for _ in 0..12 {
            h.step(1.0 / 100.0);
        }
        assert_eq!(contusions(&h), [(Part::Head, Stage::Started), (Part::Head, Stage::Started)]);
        // The shorter one's 4 s, then its 2 s of fading.
        for _ in 0..500 {
            h.step(1.0 / 100.0);
        }
        assert_eq!(contusions(&h), [(Part::Head, Stage::Started), (Part::Head, Stage::Residual)]);
        for _ in 0..200 {
            h.step(1.0 / 100.0);
        }
        assert_eq!(contusions(&h), [(Part::Head, Stage::Started)]);
    }

    #[test]
    fn a_stun_builds_holds_and_falls_and_a_flash_works_its_time() {
        let mut h = Health::new(settings(), 1);
        // The Zarya's 40 s at full strength, no cover: 40.2 s working.
        h.do_stun(40.0, 1.0, 1.0);
        let stun = |h: &Health| h.effects.iter().find(|e| e.kind == Kind::Stun).map(|e| (e.stage, e.current_strength));
        let at = |h: &mut Health, seconds: f32| {
            for _ in 0..(seconds * 100.0).round() as usize {
                h.step(1.0 / 100.0);
            }
        };
        at(&mut h, 0.1);
        let (stage, strength) = stun(&h).expect("the stun");
        assert_eq!(stage, Stage::Started);
        assert!(strength > 0.2 && strength < 0.8, "half its build-up: {strength}");
        // Held whole to 0.2 + a fifth of 40.2 s.
        at(&mut h, 7.0);
        assert!((stun(&h).expect("the stun").1 - 1.0).abs() < 1e-4);
        // Then down straight: half way through the fall, half its strength.
        let held = 0.2 + 0.2 * 40.2;
        at(&mut h, (held + 40.2) / 2.0 - 7.1);
        assert!((stun(&h).expect("the stun").1 - 0.5).abs() < 0.02, "{:?}", stun(&h));
        // Its 2 s of residue, then gone.
        at(&mut h, 25.0);
        assert!(stun(&h).is_none_or(|(stage, _)| stage == Stage::Removed));
        // The eyes burnt by a flash 15 m off (its share of 20 m, a quarter):
        // 0.2 + 40 × 0.25 s.
        let share = h.do_burn_eyes(0.25, 0.8, 40.0, 1.0);
        assert_eq!(share, Some(0.25));
        let flash = h.effects.iter().find(|e| e.kind == Kind::Flash).expect("the flash");
        assert!((flash.work - 10.2).abs() < 1e-5 && (flash.strength - 0.8).abs() < 1e-6);
    }

    #[test]
    fn over_damage_is_shared_by_the_maxima() {
        let mut h = Health::new(settings(), 7);
        // 100 to a leg of 65: 35 over, times the leg's factor 1, shared by
        // the other six (375 of maxima) at 0.7 each.
        h.apply_damage(Part::LeftLeg, 100.0, DamageKind::Bullet);
        assert!(h.part(Part::LeftLeg).destroyed);
        let chest = 85.0 - 35.0 * 85.0 / 375.0 * 0.7;
        assert!((h.part(Part::Chest).current - chest).abs() < 1e-3, "{}", h.part(Part::Chest).current);
    }

    #[test]
    fn a_destroyed_head_kills_but_bleeding_does_not_by_that() {
        let mut h = Health::new(settings(), 1);
        h.apply_damage(Part::Head, 40.0, DamageKind::Bullet);
        assert!(!h.alive);
        assert_eq!(h.cause, Some(DamageKind::Bullet));
        let mut h = Health::new(settings(), 1);
        h.apply_damage(Part::Head, 40.0, DamageKind::LightBleeding);
        assert!(h.part(Part::Head).destroyed);
        assert!(h.alive);
    }

    #[test]
    fn a_light_bleeding_drains_every_part_every_six_seconds() {
        let mut h = Health::new(settings(), 1);
        h.do_bleed(false, Part::LeftArm);
        // 0.4 s of delay, then a step every 6 s.
        run(&mut h, 6.3);
        assert_eq!(h.common()[0], 440.0);
        run(&mut h, 0.2);
        assert!((h.common()[0] - (440.0 - 7.0 * 0.8)).abs() < 1e-3, "{}", h.common()[0]);
        assert!(h.active(Kind::LightBleeding, Some(Part::LeftArm)).is_some());
    }

    #[test]
    fn the_screen_s_rates_follow_the_effects_at_work() {
        let mut h = Health::new(settings(), 1);
        // Existence alone: 3.2 energy and 2.6 hydration a minute.
        let r = h.rates();
        assert!(r[0] == 0.0 && (r[1] * 60.0 + 3.2).abs() < 1e-4 && (r[2] * 60.0 + 2.6).abs() < 1e-4, "{r:?}");
        // A light bleeding at work: 0.8 from each of seven parts every
        // 6 s, and its own energy.
        h.do_bleed(false, Part::LeftArm);
        run(&mut h, 0.5);
        let light = settings().light;
        let r = h.rates();
        assert!((r[0] + 0.8 * 7.0 / 6.0).abs() < 1e-5, "{r:?}");
        assert!((r[1] + 3.2 / 60.0 + light.energy_damage / light.energy_loop).abs() < 1e-5, "{r:?}");
        // A part with less than a loop's damage left: the rate becomes
        // what the loop took from the whole.
        h.parts[Part::RightArm.index()].current = 0.3;
        run(&mut h, 6.0);
        let r = h.rates();
        assert!((r[0] + (0.8 * 6.0 + 0.3) / 6.0).abs() < 1e-4, "{r:?}");
        // Stopped metabolism leaves energy and hydration still.
        h.metabolism_disabled = true;
        let r = h.rates();
        assert!(r[1] == 0.0 && r[2] == 0.0, "{r:?}");
    }

    #[test]
    fn a_broken_leg_slows_and_a_painkiller_lets_it_run() {
        let mut h = Health::new(settings(), 1);
        h.do_fracture(Part::LeftLeg);
        let c = h.conditions();
        assert!(c.left_leg && !c.right_leg);
        assert_eq!(c.speed_limit(), Some(0.3));
        assert!(!c.can_sprint());
        // Its pain a second later; tremor 30 s after the pain starts, and
        // then its own 15 s.
        run(&mut h, 1.1);
        assert!(h.active(Kind::Pain, Some(Part::LeftLeg)).is_some());
        run(&mut h, 44.0);
        assert!(!h.conditions().tremor);
        run(&mut h, 1.2);
        assert!(h.conditions().tremor);
        h.add_by_item(Kind::PainKiller, Part::Head, "analgin", 1.0, 0.0, 80.0, 15.0, 80.0);
        let c = h.conditions();
        assert!(c.on_painkillers && c.can_sprint() && c.speed_limit().is_none() && !c.tremor_on());
    }

    #[test]
    fn a_medkit_heals_a_part_and_pays_for_a_cure() {
        let mut h = Health::new(settings(), 1);
        h.apply_damage(Part::Chest, 30.0, DamageKind::Fall);
        h.do_bleed(false, Part::Chest);
        run(&mut h, 0.5);
        let ifak = Arc::new(Medicine {
            template: "ifak".into(),
            class: MedClass::MedKit,
            use_time: 3.0,
            part_time: Vec::new(),
            max_hp_resource: 300.0,
            hp_resource_rate: 50.0,
            part_priority: Vec::new(),
            energy: 0.0,
            hydration: 0.0,
            cures: vec![(Cure::LightBleeding, CureSpec { delay: 0.0, duration: 0.0, fade_out: 0.0, cost: 30.0, kept: [0, 0] })],
            max_resource: 0.0,
            buffs: None,
        });
        assert_eq!(h.part_to_apply(&ifak, 300.0, None), Some(Part::Chest));
        let id = h.use_item(ifak, 9, 300.0, None, None).expect("at work");
        assert!(h.index_of(id).is_some());
        run(&mut h, 2.0 + 3.0 + 0.2);
        assert!(h.existing(Kind::LightBleeding, Some(Part::Chest)).is_none());
        // No more than the part lacked: 30 of the kit's 50, and the cure's
        // 30 besides.
        assert_eq!(h.part(Part::Chest).current, 85.0);
        let events = h.take_events();
        let last = events.iter().rev().find_map(|e| if let Event::Resource(9, r) = e { Some(*r) } else { None }).expect("its resource");
        assert_eq!(last, 240.0);
    }

    #[test]
    fn the_inventory_s_use_names_a_surgical_kit_s_own_part() {
        let mut h = Health::new(settings(), 1);
        h.apply_damage(Part::RightArm, 200.0, DamageKind::Fall);
        assert!(h.part(Part::RightArm).destroyed);
        let kit = |class: MedClass, priority: Vec<Part>| Medicine {
            template: "kit".into(),
            class,
            use_time: 16.0,
            part_time: Vec::new(),
            max_hp_resource: 5.0,
            hp_resource_rate: 0.0,
            part_priority: priority,
            energy: 0.0,
            hydration: 0.0,
            cures: vec![(Cure::DestroyedPart, CureSpec { delay: 0.0, duration: 0.0, fade_out: 0.0, cost: 1.0, kept: [25, 45] })],
            max_resource: 0.0,
            buffs: None,
        };
        let own = vec![Part::Stomach, Part::LeftLeg, Part::RightLeg, Part::LeftArm, Part::RightArm];
        // The first part it can help is one of its own: that part.
        assert_eq!(h.use_part(&kit(MedClass::Medical, own.clone()), 5.0), Some(Part::RightArm));
        // Not one of its own: the common body.
        assert_eq!(h.use_part(&kit(MedClass::Medical, vec![Part::Stomach]), 5.0), None);
        // A medkit's class, or no priority: the common body.
        assert_eq!(h.use_part(&kit(MedClass::MedKit, own), 5.0), None);
        assert_eq!(h.use_part(&kit(MedClass::Medical, Vec::new()), 5.0), None);
    }

    #[test]
    fn barbed_wire_cuts_two_a_metre_in_whole_points() {
        let mut t = BarbTally::default();
        assert_eq!(t.cut([0.0, 0.0, 0.0]), None);
        assert_eq!(t.cut([0.3, 0.0, 0.0]), None);
        let d = t.cut([0.6, 0.0, 0.0]).expect("a point");
        assert!((d - 1.2).abs() < 1e-5);
        // A jump through starts over.
        assert_eq!(t.cut([2.0, 0.0, 0.0]), None);
    }
}
