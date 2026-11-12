//! Rounds, fuses and the surfaces they hit.


/// A round's figures as a shot takes them (`AmmoTemplate`).
#[derive(Debug, Clone, PartialEq)]
pub struct Round {
    pub initial_speed: f32,
    pub mass_g: f32,
    pub diameter_mm: f32,
    pub ballistic_coefficient: f32,
    pub damage: f32,
    pub penetration_power: f32,
    pub penetration_deviation: f32,
    /// `PenetrationChanceObstacle`.
    pub penetration_chance: f32,
    pub ricochet_chance: f32,
    pub fragmentation_chance: f32,
    pub min_fragments: i32,
    pub max_fragments: i32,
    /// `ArmorDamage` over 100.
    pub armor_damage: f32,
    /// `AmmoLifeTimeSec`.
    pub life: f32,
    /// `ammoAccr`.
    pub accuracy: i32,
    pub projectiles: i32,
    pub penetration_damage_mod: f32,
    /// The explosive it carries, where it has one (`ExplosiveAmmoComponent`).
    pub fuse: Option<Fuse>,
    /// What a hit of it does to a body beyond its damage (`DamageInfo`'s):
    /// the stamina burnt for each point (`StaminaBurnPerDamage`), and what it
    /// adds to the odds of a heavy and a light bleeding (`HeavyBleedingDelta`,
    /// `LightBleedingDelta`).
    pub stamina_burn: f32,
    pub heavy_bleeding: f32,
    pub light_bleeding: f32,
    /// Its colour where it is a tracer (`Tracer`, `TracerColor`; ballistics
    /// spec, "A tracer").
    pub tracer: Option<crate::combat::tracers::TracerColor>,
}

impl Round {
    /// A round's template from the item database.
    pub fn of(template: &trp_data::setup::database::ItemTemplate) -> Option<Self> {
        let f = |name: &str| template.prop_f32(name);
        let whole = |name: &str| template.props.get(name).and_then(|v| v.as_f64()).map(|v| v as i32);
        Some(Self {
            initial_speed: f("InitialSpeed")?,
            mass_g: f("BulletMassGram")?,
            diameter_mm: f("BulletDiameterMilimeters")?,
            ballistic_coefficient: f("BallisticCoeficient")?,
            damage: f("Damage")?,
            penetration_power: f("PenetrationPower")?,
            penetration_deviation: f("PenetrationPowerDiviation").unwrap_or(0.0),
            penetration_chance: f("PenetrationChanceObstacle").unwrap_or(0.0),
            ricochet_chance: f("RicochetChance").unwrap_or(0.0),
            fragmentation_chance: f("FragmentationChance").unwrap_or(0.0),
            min_fragments: whole("MinFragmentsCount").unwrap_or(0),
            max_fragments: whole("MaxFragmentsCount").unwrap_or(0),
            armor_damage: f("ArmorDamage").unwrap_or(50.0) / 100.0,
            life: f("AmmoLifeTimeSec").unwrap_or(13.0),
            accuracy: whole("ammoAccr").unwrap_or(0),
            projectiles: whole("ProjectileCount").unwrap_or(1).max(1),
            penetration_damage_mod: f("PenetrationDamageMod").unwrap_or(0.0),
            fuse: Fuse::of(template),
            stamina_burn: f("StaminaBurnPerDamage").unwrap_or(0.0),
            heavy_bleeding: f("HeavyBleedingDelta").unwrap_or(0.0),
            light_bleeding: f("LightBleedingDelta").unwrap_or(0.0),
            tracer: template
                .props
                .get("Tracer")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                .then(|| crate::combat::tracers::TracerColor::named(template.props.get("TracerColor").and_then(|v| v.as_str()).unwrap_or("default"))),
        })
    }

    /// It carries an explosive (`ExplosiveAmmoComponent`).
    pub fn explosive(&self) -> bool {
        self.fuse.is_some()
    }

    /// `AmmoFactor`: what the round does to the weapon's spread.
    pub fn ammo_factor(&self) -> f32 {
        if self.accuracy <= 0 {
            (100.0 + self.accuracy.unsigned_abs() as f32) / 100.0
        } else {
            100.0 / (100 + self.accuracy) as f32
        }
    }
}

/// An explosive round's fuse (`ExplosiveAmmoComponent`, made for a round with
/// `HasGrenaderComponent`; underbarrel-launcher spec, "The grenade"): what a hit
/// that sets it off plays and whether it blasts. Its blast's figures are its
/// template's (`ExplosionStrength`, the distances, the fragments), read where
/// the blast is made.
#[derive(Debug, Clone, PartialEq)]
pub struct Fuse {
    /// The round's template.
    pub round: String,
    /// The least age a hit sets it off at (`FuzeArmTimeSec`): before it, the hit
    /// is only a bullet's.
    pub arm_time: f32,
    /// The effect played at the shot's first hit that sets it off
    /// (`ExplosionType`; none where it names none).
    pub effect: Option<String>,
    /// The material's own hit played besides (`ShowHitEffectOnExplode`).
    pub hit_effect_too: bool,
    /// It has no blast (`GrenadeComponentIsDummy`: no blindness and no
    /// contusion within reach, and a greatest distance of nought): the big
    /// rounds' extra effect.
    pub dummy: bool,
}

impl Fuse {
    pub fn of(template: &trp_data::setup::database::ItemTemplate) -> Option<Self> {
        if !template.props.get("HasGrenaderComponent").and_then(|v| v.as_bool()).unwrap_or(false) {
            return None;
        }
        let f = |name: &str| template.prop_f32(name).unwrap_or(0.0);
        let y = |name: &str| template.props.get(name).and_then(|v| v.get("y")).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let dummy = y("Blindness") < f32::EPSILON && y("Contusion") < f32::EPSILON && f("MaxExplosionDistance") < f32::EPSILON;
        Some(Self {
            round: template.id.clone(),
            arm_time: f("FuzeArmTimeSec"),
            effect: template.props.get("ExplosionType").and_then(|v| v.as_str()).filter(|name| !name.is_empty()).map(str::to_owned),
            hit_effect_too: template.props.get("ShowHitEffectOnExplode").and_then(|v| v.as_bool()).unwrap_or(false),
            dummy,
        })
    }
}

/// What a hit thing is made of: its material's number (`MaterialType`) and
/// its six figures (penetration level, penetration chance, ricochet chance,
/// fragmentation chance, deviation chance, deviation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Surface {
    pub material: i32,
    pub figures: [f32; 6],
}

impl Surface {
    pub(super) fn level(&self) -> f32 {
        self.figures[0]
    }
    pub(super) fn penetration_chance(&self) -> f32 {
        self.figures[1]
    }
    pub(super) fn ricochet_chance(&self) -> f32 {
        self.figures[2]
    }
    pub(super) fn fragmentation_chance(&self) -> f32 {
        self.figures[3]
    }
    pub(super) fn deviation_chance(&self) -> f32 {
        self.figures[4]
    }
    pub(super) fn deviation(&self) -> f32 {
        self.figures[5]
    }
}

/// The figures of a material by the presets (`ColliderPresets`): the first
/// preset of that material, as an editor finds it (`FindPresetIndex`).
pub fn preset_surface(presets: &[trp_data::setup::hard_settings::BallisticPreset], material: i32) -> Surface {
    presets
        .iter()
        .find(|p| p.material == material)
        .map(|p| Surface { material, figures: std::array::from_fn(|k| p.figures.get(k).copied().unwrap_or(0.0)) })
        .unwrap_or(Surface { material, figures: [0.0; 6] })
}
