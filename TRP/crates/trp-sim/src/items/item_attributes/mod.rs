//! A thing's figures as the inspect window lists them.
//! After the inspect-window spec: its attributes in the game's order, each with its name, its value as
//! written and how it is shown (a bar, a compact row, a row with a
//! dropdown), for weapons, armour and rounds; the window's own durability
//! row of a weapon. Things are as made: the app keeps no wear, so a
//! durability is its template's.

use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::Value as Json;
use trp_data::Database;

use crate::items::inventory::{folding_part, is, Item};

mod meds;
mod firearms;
mod protection;

pub use meds::*;
use meds::{medicine};
use firearms::{ammo, percent, weapon};
use protection::{armour, penetration_lines};

/// Whether a raid's discard limits are listed (the options' `RAID LIMITS`,
/// set once as the run begins: with it off, "Can be discarded in raid" is
/// not listed).
/// On, the install's globals decide (`DiscardLimitsEnabled`); off, no
/// thing lists one.
static DISCARD_LIMITS: AtomicBool = AtomicBool::new(true);

/// Sets whether the discard limit is listed (`RAID LIMITS`).
pub fn set_discard_limits(on: bool) {
    DISCARD_LIMITS.store(on, Ordering::Relaxed);
}

/// How a row is shown (`EItemAttributeDisplayType`): a bar over its range
/// (which may run downwards) with its base and its base's change as a
/// share of it, a compact row, or a compact row with a dropdown.
#[derive(Debug, Clone, PartialEq)]
pub enum Show {
    Bar { range: [f32; 2], base: f32, delta: f32 },
    Compact,
    Dropdown,
}

/// One attribute as listed: its own name (`EItemAttributeId`, by which its
/// icon goes), its name in the install's words in upper case, its value as
/// written (none where it shows none), how it is shown, its base where its
/// value is coloured by its sign (`Colored`), and whether less is better.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub id: &'static str,
    /// The table its icon is in, by the enum its id is of
    /// (`StaticIcons.GetAttributeIcon`).
    pub table: Table,
    pub name: String,
    pub value: Option<String>,
    pub show: Show,
    pub colored: Option<f32>,
    pub less_is_good: bool,
    /// What a dropdown row says when the mouse rests on its arrow
    /// (`CompactCharacteristicDropdownPanel.ShowTooltip`): its lines; an
    /// enhanced row's, when it rests on its value (`HoverTooltipArea`).
    pub tip: Vec<TipLine>,
    /// Enhanced by a repair kit (`ItemAttribute.Enhancement`): the buff's
    /// own row, and ACCURACY with a spread buff; its value drawn in the
    /// enhancement's blue (`CompactCharacteristicPanel.Show`), its tooltip
    /// its `tip`.
    pub enhanced: bool,
}

/// An enhanced value's colour (`CompactCharacteristicPanel.Show`: 84, 193,
/// 255), the tooltip's own (`#54c1ff`).
pub const ENHANCED: [u8; 3] = [84, 193, 255];

/// The enum an attribute's id is of, which picks the table of its icon
/// (`StaticIcons`: `ItemAttributeSprites`, `HealEffectSprites`,
/// `DamageEffectSprites`, `StimulatorBuffSprites`): the same name may be
/// in more than one (`Pain`, `Fracture`, the bleedings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Table {
    /// `EItemAttributeId`.
    #[default]
    Attribute,
    /// `EHealthFactorType`.
    Health,
    /// `EDamageEffectType`.
    Damage,
    /// `EStimulatorBuffType`.
    Buff,
}

/// A line of a row's tooltip (`MultiLineInfo`'s `SingleLineInfo`, or a
/// line of the simple tooltip's words): an armour class's mark before it
/// (`<sprite name="armor_classes_N">`), or as much room as one
/// (`<space=1.5em>`), its words (with the install's colour tags); or a
/// break between groups (a line that is none).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TipLine {
    pub class: Option<u32>,
    pub indent: bool,
    pub text: String,
    pub divider: bool,
}

impl TipLine {
    fn words(text: String) -> Self {
        Self { text, ..Self::default() }
    }
}

/// A weapon's durability row (`DurabilityPanel`): its present durability,
/// its greatest, and the template's greatest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Durability {
    pub now: f32,
    pub most: f32,
    pub peak: f32,
}


/// What the window lists of a thing: a weapon's durability row, and the
/// rows in their order (`RecreateAttributeBars` takes the bars, then the
/// compact rows, then the dropdown rows, keeping it), ARMOR POINTS first
/// among the compact rows where it is had.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Figures {
    pub durability: Option<Durability>,
    pub rows: Vec<Row>,
    /// The durability row's name (`ItemAttributeNames`: "Durability", in
    /// the install's words, upper case).
    pub durability_name: String,
}

/// The key of each attribute's name (`ItemAttributeNames.GetName`; an
/// attribute not named there goes by its own name).
fn name_key(id: &'static str) -> &'static str {
    match id {
        "ArmorZone" => "ARMOR ZONE(S)",
        "Ricochet" => "RICOCHET CHANCE",
        "ArmorClass" => "ARMOR CLASS",
        "ArmorMaterial" => "MATERIAL",
        "ChangeMovementSpeed" => "CHANGE MOVEMENT SPEED",
        "ChangeTurningSpeed" => "CHANGE TURNING SPEED",
        "Ergonomics" => "ERGONOMICS",
        "ArmorPoints" => "ARMOR POINTS",
        "BulletSpeed" => "BULLET SPEED",
        "Caliber" | "AmmoCaliber" => "CALIBER",
        "CenterOfImpact" => "ACCURACY",
        "Recoil" => "RECOIL",
        "SightingRange" => "SightingRange",
        "Velocity" => "Velocity",
        "RecoilUp" => "Recoil up",
        "RecoilBack" => "Recoil back",
        "WeaponFireType" => "TYPES OF FIRE",
        "FireRate" => "FIRE RATE",
        "EffectiveDist" => "EFFECTIVE DISTANCE",
        "BlindnessProtection" => "EYES PROTECTION",
        "ArmorType" => "ARMOR TYPE",
        "MaxAmmoDamage" => "MAX AMMO DAMAGE",
        "AmmoProjectileCount" => "AMMO PROJECTILE COUNT",
        "AmmoPenetrationPower" => "AMMO PENETRATION POWER",
        "HpResource" => "HP RESOURCE",
        "UseTime" => "USE TIME",
        "LimitedDiscard" => "EItemAttributeId/LimitedDiscard",
        other => other,
    }
}

/// The thing's figures, with the install's words for a key where it has
/// them (`say`; the key itself otherwise, as the game's locale does).
pub fn figures(item: &Item, db: &Database, say: &dyn Fn(&str) -> Option<String>) -> Figures {
    let words = |key: &str| say(key).unwrap_or_else(|| key.to_owned());
    let row = |id: &'static str, value: Option<String>, show: Show| Row { id, table: Table::Attribute, name: words(name_key(id)).to_uppercase(), value, show, colored: None, less_is_good: false, tip: Vec::new(), enhanced: false };
    let num = |item: &Item, name: &str| of(db, &item.kind.template, name);
    let mut figures = Figures { durability_name: words(name_key("Durability")).to_uppercase(), ..Figures::default() };
    // The discard limit, `Item`'s own and so before anything else: where
    // the template has one (0 and over) and the globals have them on
    // (`DiscardLimitsEnabled`): "> " and the limit, nothing at 0. None with
    // `RAID LIMITS` off (`set_discard_limits`).
    let limit = db.items.get(&item.kind.template).and_then(|template| template.props.get("DiscardLimit")).and_then(number);
    let listed = DISCARD_LIMITS.load(Ordering::Relaxed) && db.config.get("DiscardLimitsEnabled").and_then(Json::as_bool).unwrap_or(false);
    if let (Some(limit), true) = (limit.filter(|limit| *limit >= 0.0), listed) {
        let said = if limit == 0.0 { String::new() } else { format!("> {}", plain(limit)) };
        figures.rows.push(row("LimitedDiscard", Some(said), Show::Compact));
    }
    // CONTAINER SIZE, for what has grids (`CompoundItem`'s own, made before
    // the rest): its grids' cells summed, where there are any.
    let cells: u32 = item.kind.grids.iter().map(|grid| grid.cells[0] * grid.cells[1]).sum();
    if cells > 0 {
        figures.rows.push(row("ContainerSize", Some(cells.to_string()), Show::Compact));
    }
    if is(item, "Weapon") {
        weapon(item, db, &num, &words, &row, &mut figures);
    } else if is(item, "Ammo") {
        ammo(item, db, &num, &words, &row, &mut figures);
    } else if is(item, "Meds") || is(item, "FoodDrink") {
        medicine(item, db, &num, &words, &row, &mut figures);
    } else {
        armour(item, db, &num, &words, &row, &mut figures);
    }
    // ARMOR POINTS, first among the compact rows: the present durability of
    // every armour of a class in the thing, summed, to one place.
    let armours: Vec<&Item> = parts(item).into_iter().filter(|part| has_armour(part, &num)).collect();
    if !armours.is_empty() {
        let points: f32 = armours.iter().map(|part| part.durability()[0]).sum();
        let at = figures.rows.iter().position(|row| row.show == Show::Compact).unwrap_or(figures.rows.len());
        figures.rows.insert(at, row("ArmorPoints", Some(trimmed((points * 10.0).round() / 10.0, 1)), Show::Compact));
    }
    // A repair kit's buff, the thing's last attribute (`BuffComponent.
    // UpdateEnhancedAttribute` adds it to the list made before it): its
    // name, its whole percent, compact, enhanced; a spread buff marks
    // ACCURACY enhanced too (`TryGetEnhancedAttributeId`).
    if let Some(buff) = item.buff {
        let tip = vec![TipLine::words(enhancement_tip(item, &buff, &words))];
        if buff.kind == crate::items::repair::BuffKind::WeaponSpread {
            if let Some(accuracy) = figures.rows.iter_mut().find(|row| row.id == "CenterOfImpact") {
                (accuracy.enhanced, accuracy.tip) = (true, tip.clone());
            }
        }
        let value = format!("{} {}", buff.percent(), words("%"));
        figures.rows.push(Row { name: words(buff.name_key()).to_uppercase(), tip, enhanced: true, ..row(buff.attribute(), Some(value), Show::Compact) });
    }
    figures
}

/// An enhanced row's tooltip (`RepairBuffAttribute.GetEnhancementTooltip`):
/// "Enhanced by a repair kit", and where the durability is nought or more
/// " (Durability left: n)", the durability less the threshold to two
/// places (halves to the even, written shortest), all in the
/// enhancement's blue.
fn enhancement_tip(item: &Item, buff: &crate::items::repair::RepairBuff, words: &dyn Fn(&str) -> String) -> String {
    let mut text = words("UI/ModifiedByRepairKitEnhancement");
    let durability = item.durability()[0];
    if durability >= 0.0 {
        let left = ((durability as f64 - buff.threshold) * 100.0).round_ties_even() / 100.0;
        text += &format!(" ({} {left})", words("UI/DurabilityLeft"));
    }
    format!("<color=#54c1ff>{text}</color>")
}

/// The thing and every part in it, at any depth, the thing first and each
/// part before what hangs on it.
fn parts(item: &Item) -> Vec<&Item> {
    let mut out = vec![item];
    for part in item.slots.iter().flatten() {
        out.extend(parts(part));
    }
    out
}

/// Whether a thing is armour of its own (`CompositeArmorComponent`: a
/// greatest durability and a class).
fn has_armour(item: &Item, num: &dyn Fn(&Item, &str) -> f32) -> bool {
    num(item, "MaxDurability") > 0.0 && num(item, "armorClass") > 0.0
}

/// A figure of a template's, nought where it has none.
fn of(db: &Database, template: &str, name: &str) -> f32 {
    db.items.get(template).and_then(|template| template.props.get(name)).and_then(number).unwrap_or(0.0)
}

fn number(value: &Json) -> Option<f32> {
    value.as_f64().map(|v| v as f32).or_else(|| value.as_str()?.parse().ok())
}

/// A figure written with no more places than it needs, at most `places`
/// (.NET's `0.##`).
fn trimmed(value: f32, places: usize) -> String {
    let written = format!("{value:.places$}");
    if written.contains('.') {
        written.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        written
    }
}

/// A figure as .NET writes a float by default (shortest form).
fn plain(value: f32) -> String {
    let written = format!("{value}");
    written.strip_suffix(".0").map(str::to_owned).unwrap_or(written)
}

/// The names a damage effect is listed by (`Item._replacements`); a kind
/// not here (a destroyed part, a lethal intoxication) is not listed.
const DAMAGE_NAMES: [(&str, &str); 7] = [
    ("LightBleeding", "REMOVES LIGHT BLEEDS"),
    ("HeavyBleeding", "REMOVES HEAVY BLEEDS"),
    ("Fracture", "REMOVES FRACTURE"),
    ("Pain", "REMOVES PAIN"),
    ("Contusion", "REMOVES CONTUSION"),
    ("Intoxication", "REMOVES TOXICATION"),
    ("RadExposure", "REMOVES RADIATION EXPOSURE"),
];

/// The health factors a health effect names (`EHealthFactorType`).
const HEALTH_FACTORS: [&str; 7] = ["Health", "Hydration", "Energy", "Radiation", "Temperature", "Poisoning", "Effect"];

/// A figure rounded to `places`, halves to the even (.NET's `Math.Round`).
fn round_even(value: f32, places: i32) -> f32 {
    let scale = 10f64.powi(places);
    ((value as f64 * scale).round_ties_even() / scale) as f32
}
