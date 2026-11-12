//! The health settings read from the install.

use super::*;

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
    /// The berserk's delay, working time and residue (`Berserk`).
    pub berserk: [f32; 3],
    /// An intoxication's delay, residue, damage to each part a loop and
    /// its loop (`Intoxication`: `DefaultDelay`, `DefaultResidueTime`,
    /// `DamageHealth`, `HealthLoopTime`).
    pub intoxication: [f32; 4],
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

pub(super) fn num(v: &Json, name: &str) -> Result<f32> {
    v.get(name).and_then(Json::as_f64).map(|f| f as f32).with_context(|| format!("no number {name}"))
}

pub(super) fn at<'a>(v: &'a Json, name: &str) -> Result<&'a Json> {
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
            berserk: {
                let b = effect("Berserk")?;
                [num(b, "DefaultDelay")?, num(b, "WorkingTime")?, num(b, "DefaultResidueTime")?]
            },
            intoxication: {
                let i = effect("Intoxication")?;
                [num(i, "DefaultDelay")?, num(i, "DefaultResidueTime")?, num(i, "DamageHealth")?, num(i, "HealthLoopTime")?]
            },
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
