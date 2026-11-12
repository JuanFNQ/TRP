//! Cures, medical classes and the medicines read from the install.

use super::*;

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
    pub(super) const ALL: [Cure; 9] = [
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

    pub(super) fn of_name(name: &str) -> Option<Cure> {
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
    pub(super) fn effect(self) -> Option<Kind> {
        Some(match self {
            Cure::HeavyBleeding => Kind::HeavyBleeding,
            Cure::LightBleeding => Kind::LightBleeding,
            Cure::Fracture => Kind::Fracture,
            Cure::Pain => Kind::Pain,
            Cure::Contusion => Kind::Contusion,
            Cure::Intoxication => Kind::Intoxication,
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

    pub(super) fn cure(&self, cure: Cure) -> Option<&CureSpec> {
        self.cures.iter().find(|(c, _)| *c == cure).map(|(_, spec)| spec)
    }

    pub(super) fn use_time_on(&self, part: Part) -> f32 {
        self.part_time.iter().find(|(p, _)| *p == part).map_or(self.use_time, |(_, mult)| self.use_time * mult)
    }

    pub(super) fn start_delay(&self, s: &HealthSettings) -> f32 {
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
