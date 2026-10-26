//! A thing's figures as the inspect window lists them (inspect-window spec):
//! its attributes in the game's order, each with its name, its value as
//! written and how it is shown (a bar, a compact row, a row with a
//! dropdown), for weapons, armour and rounds; the window's own durability
//! row of a weapon. Things are as made: the app keeps no wear, so a
//! durability is its template's.

use serde_json::Value as Json;
use trp_data::Database;

use crate::inventory::{folding_part, is, Item};

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
    /// (`CompactCharacteristicDropdownPanel.ShowTooltip`): its lines.
    pub tip: Vec<TipLine>,
}

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
    let row = |id: &'static str, value: Option<String>, show: Show| Row { id, table: Table::Attribute, name: words(name_key(id)).to_uppercase(), value, show, colored: None, less_is_good: false, tip: Vec::new() };
    let num = |item: &Item, name: &str| of(db, &item.kind.template, name);
    let mut figures = Figures { durability_name: words(name_key("Durability")).to_uppercase(), ..Figures::default() };
    // The discard limit, `Item`'s own and so before anything else: where
    // the template has one (0 and over) and the globals have them on
    // (`DiscardLimitsEnabled`): "> " and the limit, nothing at 0.
    let limit = db.items.get(&item.kind.template).and_then(|template| template.props.get("DiscardLimit")).and_then(number);
    if let (Some(limit), true) = (limit.filter(|limit| *limit >= 0.0), db.config.get("DiscardLimitsEnabled").and_then(Json::as_bool).unwrap_or(false)) {
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
    figures
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

/// Medicine's, food's and drink's rows (`Meds`, `FoodDrink`; each compact):
/// the medkit's resource where it has one (`MedKitComponent`), then
/// (`HealthEffectsComponent`) the use time, each health effect (coloured
/// by its value's sign), each damage effect it takes away (`_replacements`'
/// kinds), and a stimulant's buffs (`SetupStimulatorBuffsAttributes`), in
/// the template's and the set's order (inspect-window spec, "Medicine").
fn medicine(item: &Item, db: &Database, num: &dyn Fn(&Item, &str) -> f32, words: &dyn Fn(&str) -> String, row: &dyn Fn(&'static str, Option<String>, Show) -> Row, figures: &mut Figures) {
    let props = db.items.get(&item.kind.template).map(|template| &template.props);
    let sec = words("sec");
    let (del, dur) = (words("Del."), words("Dur."));
    // A delay's, a duration's and a figure's parts, " / " between.
    let timed = |delay: f32, duration: f32| -> Vec<String> {
        let mut parts = Vec::new();
        if delay > 1.0 {
            parts.push(format!("{del} {}{sec}", plain(delay)));
        }
        if duration > 0.0 {
            parts.push(format!("{dur} {}{sec}", plain(duration)));
        }
        parts
    };
    let most = num(item, "MaxHpResource");
    if is(item, "Meds") && most > 0.0 {
        let now = item.resource.unwrap_or(most);
        figures.rows.push(row("HpResource", Some(format!("{}/{} {}", now as i64, most as i64, words("HP"))), Show::Compact));
    }
    let use_time = if is(item, "Meds") { num(item, "medUseTime") } else { num(item, "foodUseTime") };
    figures.rows.push(row("UseTime", Some(format!("{}{sec}", trimmed(use_time, 1))), Show::Compact));
    let field = |name: &str| props.and_then(|props| props.get(name)).and_then(Json::as_object);
    let f = |spec: &Json, key: &str| spec.get(key).and_then(number).unwrap_or(0.0);
    // (The install's keys are in the game's order; serde's map keeps them
    // sorted, which is that order while they are.)
    for (factor, spec) in field("effects_health").into_iter().flatten() {
        let Some(id) = HEALTH_FACTORS.iter().find(|name| name.eq_ignore_ascii_case(factor)).copied() else { continue };
        let value = f(spec, "value");
        let mut parts = timed(f(spec, "delay"), f(spec, "duration"));
        if value != 0.0 {
            parts.push(plain(value.abs()));
        }
        figures.rows.push(Row { table: Table::Health, name: words(id).to_uppercase(), colored: Some(value), ..row(id, Some(parts.join(" / ")), Show::Compact) });
    }
    for (kind, spec) in field("effects_damage").into_iter().flatten() {
        let Some((id, key)) = DAMAGE_NAMES.iter().find(|(name, _)| name == kind).copied() else { continue };
        let mut parts = timed(f(spec, "delay"), f(spec, "duration"));
        let cost = f(spec, "cost");
        if cost > 0.0 {
            parts.push(format!("{} HP", plain(cost)));
        }
        figures.rows.push(Row { table: Table::Damage, name: words(key).trim_end().to_uppercase(), ..row(id, Some(parts.join(" / ")), Show::Compact) });
    }
    let set = props.and_then(|props| props.get("StimulatorBuffs")).and_then(Json::as_str).filter(|set| !set.is_empty());
    let settings = set.and_then(|_| crate::health::StimulatorSettings::read(&db.config["Health"]["Effects"]).ok());
    for buff in set.zip(settings.as_ref()).and_then(|(set, settings)| settings.sets.get(set)).into_iter().flatten() {
        use crate::health::BuffKind;
        let name = match (&buff.kind, &buff.skill) {
            (BuffKind::SkillRate, Some(skill)) => format!("{} \"{}\"", words("Skill"), words(skill)),
            _ => words(buff.kind.name()),
        };
        figures.rows.push(Row { table: Table::Buff, name: name.to_uppercase(), ..row(buff.kind.name(), Some(buff_value(buff, words)), Show::Compact) });
    }
}

/// A buff's value as its row writes it (`StimulatorBuffSettings.
/// GetStringValue`): its chance below a whole, its delay over a second, its
/// duration under two hours with its figure in brackets, else its figure
/// alone; the figure in the buff's colour or the side effect's
/// (`StimulatorHelper.BuffColoredStringValue`: by `IsBuff`, not by its sign).
fn buff_value(buff: &crate::health::BuffDef, words: &dyn Fn(&str) -> String) -> String {
    use crate::health::BuffKind;
    let sec = words("sec");
    let mut parts = Vec::new();
    if buff.chance < 1.0 {
        parts.push(format!("{} {}%", words("UI/ItemAttribute/Chance"), (buff.chance * 100.0).round() as i64));
    }
    if buff.delay > 1.0 {
        parts.push(format!("{} {}{sec}", words("Del."), plain(buff.delay)));
    }
    // The figure as shown (`GetVisualBuffValue`): absolute to a place,
    // else a percentage to a whole, halves to the even.
    let shown = if buff.absolute { round_even(buff.value, 1) } else { round_even(buff.value * 100.0, 0) };
    let units = match (buff.kind, buff.absolute) {
        (_, false) => "%".to_owned(),
        (BuffKind::BodyTemperature, true) => " C°".to_owned(),
        (BuffKind::WeightLimit, true) => format!(" {}", words(" kg")),
        _ => String::new(),
    };
    let colored = (buff.value != 0.0).then(|| {
        let sign = if shown > 0.0 { "+" } else { "" };
        let color = if buff.kind.is_buff(buff.value) { "#54C1FFFF" } else { "#C40000FF" };
        format!("<color={color}>{sign}{}{units}</color>", plain(shown))
    });
    let mut said = parts.join(" / ");
    if buff.duration > 0.0 && buff.duration < 7200.0 {
        if !said.is_empty() {
            said.push_str(" / ");
        }
        said.push_str(&format!("{} {}{sec}", words("Dur."), plain(buff.duration)));
        if let Some(colored) = colored {
            said.push_str(&format!(" ({colored})"));
        }
    } else if let Some(colored) = colored {
        if !said.is_empty() {
            said.push_str(" / ");
        }
        said.push_str(&format!(" {colored}"));
    }
    said
}

/// A share as a whole percentage (`P0`, the invariant culture's: "15 %").
fn percent(share: f32) -> String {
    format!("{} %", (share * 100.0).round() as i64)
}

/// A weapon's rows (`Weapon`): its durability row, its six bars over its
/// parts, and its compact rows.
fn weapon(item: &Item, db: &Database, num: &dyn Fn(&Item, &str) -> f32, words: &dyn Fn(&str) -> String, row: &dyn Fn(&'static str, Option<String>, Show) -> Row, figures: &mut Figures) {
    let mods: Vec<&Item> = parts(item).into_iter().skip(1).filter(|part| is(part, "Mod")).collect();
    let sum = |name: &str| mods.iter().map(|part| num(part, name)).sum::<f32>();
    let [now, most] = item.durability();
    let peak = num(item, "MaxDurability").max(most);
    figures.durability = Some(Durability { now, most, peak });
    // ERGONOMICS.
    let ergonomics = num(item, "Ergonomics");
    let delta = sum("Ergonomics") / ergonomics.max(1.0);
    let total = (ergonomics * (1.0 + delta)).clamp(0.0, 100.0);
    figures.rows.push(row("Ergonomics", Some(trimmed(total, 2)), Show::Bar { range: [0.0, 100.0], base: ergonomics.round(), delta }));
    // ACCURACY: the barrel's (else the weapon's) centre of impact and
    // deviation, the parts' accuracy, the round's.
    let barrel = parts(item).into_iter().skip(1).find(|part| is(part, "Barrel"));
    let from = barrel.unwrap_or(item);
    let base = num(from, "CenterOfImpact");
    let change = -sum("Accuracy") / 100.0;
    let curve = num(from, "DeviationCurve");
    let greatest = Some(num(from, "DeviationMax")).filter(|max| *max != 0.0).unwrap_or(100.0);
    let deviation = |durability: f32| {
        let twice = 2.0 * curve;
        let s = if 100.0 - twice == 0.0 { durability / twice } else { (-curve + ((100.0 - twice) * durability + curve * curve).sqrt()) / (100.0 - twice) };
        let rest = 1.0 - s;
        rest * rest * greatest + 2.0 * s * rest * curve + s * s
    };
    let at_peak = deviation(peak);
    let delta = change + (deviation(now) - at_peak) / (greatest - at_peak);
    let round = current_round(item, db);
    let factor = round.as_deref().map_or(1.0, |round| {
        let accuracy = of(db, round, "ammoAccr");
        if accuracy <= 0.0 { (100.0 + accuracy.abs()) / 100.0 } else { 100.0 / (100.0 + accuracy) }
    });
    let moa = base * (1.0 + change) * factor * deviation(now) * 100.0 / 2.9089;
    let widest = db.items.values().filter(|template| db.is_a(template, "Barrel")).filter_map(|template| template.props.get("CenterOfImpact").and_then(number)).fold(0.0f32, f32::max);
    let mut accuracy = row("CenterOfImpact", Some(format!("{} {}", one_or_two(moa), words("moa"))), Show::Bar { range: [widest + 0.1, 0.001], base, delta });
    accuracy.less_is_good = true;
    figures.rows.push(accuracy);
    // SIGHTING RANGE.
    let sighting = mods.iter().map(|part| num(part, "SightingRange")).fold(num(item, "SightingRange"), f32::max);
    figures.rows.push(row("SightingRange", Some(if sighting > 0.0 { plain(sighting) } else { "N/A".to_owned() }), Show::Bar { range: [0.0, 5000.0], base: sighting, delta: 0.0 }));
    // Recoil up and back: a folded stock's recoil left out.
    let folded = folding_part(item).is_some_and(|part| part.folded);
    let recoil = mods.iter().filter(|part| !(folded && is(part, "Stock"))).map(|part| num(part, "Recoil")).sum::<f32>() / 100.0;
    for (id, force) in [("RecoilUp", "RecoilForceUp"), ("RecoilBack", "RecoilForceBack")] {
        let base = num(item, force);
        let mut recoil_row = row(id, Some(format!("{}", (base + base * recoil).round() as i64)), Show::Bar { range: [0.0, 1000.0], base, delta: recoil });
        recoil_row.less_is_good = true;
        figures.rows.push(recoil_row);
    }
    // Velocity: the round it would fire.
    let speed = round.as_deref().map_or(0.0, |round| of(db, round, "InitialSpeed"));
    let delta = (sum("Velocity") + num(item, "Velocity")) / 100.0;
    figures.rows.push(row("Velocity", Some(format!("{} {}", (speed * (1.0 + delta)).round() as i64, words("m/s"))), Show::Bar { range: [0.0, 1500.0], base: speed, delta }));
    // The compact rows, each left out where its base is nought.
    let template = db.items.get(&item.kind.template).map(|template| &template.props);
    let modes: Vec<String> = template.and_then(|props| props.get("weapFireType")).and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_str).map(words).collect();
    if !modes.is_empty() {
        figures.rows.push(row("WeaponFireType", Some(modes.join(", ")), Show::Compact));
    }
    let caliber = template.and_then(|props| props.get("ammoCaliber")).and_then(Json::as_str).unwrap_or("").replace("Caliber", "");
    if !caliber.is_empty() {
        figures.rows.push(row("AmmoCaliber", Some(words(&caliber)), Show::Compact));
    }
    let rate = num(item, "bFirerate");
    if rate != 0.0 {
        figures.rows.push(row("FireRate", Some(format!("{} {}", plain(rate), words("rpm"))), Show::Compact));
    }
    let distance = num(item, "bEffDist");
    if distance != 0.0 {
        figures.rows.push(row("EffectiveDist", Some(format!("{} {}", plain(distance), words("meters"))), Show::Compact));
    }
}

/// A figure to one place and a second where it has one (.NET's `0.0#`).
fn one_or_two(value: f32) -> String {
    let two = format!("{value:.2}");
    match two.strip_suffix('0') {
        Some(one) => one.to_owned(),
        None => two,
    }
}

/// The template of the round a weapon would fire (`CurrentAmmoTemplate`):
/// the one in its chamber, else its magazine's first (`FirstRealAmmo`:
/// its cartridges' last, the top one, as a magazine here keeps them),
/// else its template's own (`defAmmo`).
fn current_round(item: &Item, db: &Database) -> Option<String> {
    let chamber = item.slots.iter().zip(&item.kind.slots).find(|(_, holder)| holder.name.starts_with("patron_in_weapon")).and_then(|(round, _)| round.as_ref());
    let fed = item.slots.iter().zip(&item.kind.slots).find(|(_, holder)| holder.name == crate::inventory::MAGAZINE_SLOT).and_then(|(magazine, _)| magazine.as_ref()).and_then(|magazine| magazine.loose.last());
    chamber.or(fed).map(|round| round.kind.template.clone()).or_else(|| db.items.get(&item.kind.template)?.props.get("defAmmo")?.as_str().map(str::to_owned))
}

/// A round's rows (`AmmoTemplate.GetCachedReadonlyQualities`), each where
/// its base is not nought but its calibre.
fn ammo(item: &Item, db: &Database, num: &dyn Fn(&Item, &str) -> f32, words: &dyn Fn(&str) -> String, row: &dyn Fn(&'static str, Option<String>, Show) -> Row, figures: &mut Figures) {
    let damage = num(item, "Damage");
    if damage != 0.0 {
        figures.rows.push(row("MaxAmmoDamage", Some(plain(damage)), Show::Compact));
    }
    let penetration = num(item, "PenetrationPower");
    if penetration != 0.0 {
        let mut penetration_row = row("AmmoPenetrationPower", Some(plain(penetration)), Show::Dropdown);
        penetration_row.tip = penetration_lines(db, penetration, words);
        figures.rows.push(penetration_row);
    }
    // (The round's own `Caliber`, in the install's words.)
    let caliber = db.items.get(&item.kind.template).and_then(|template| template.props.get("Caliber")).and_then(Json::as_str).unwrap_or("");
    figures.rows.push(row("Caliber", Some(words(caliber)), Show::Compact));
    let projectiles = num(item, "ProjectileCount");
    if projectiles > 1.0 {
        figures.rows.push(row("AmmoProjectileCount", Some(plain(projectiles)), Show::Compact));
    }
    let speed = num(item, "InitialSpeed");
    if speed != 0.0 {
        figures.rows.push(row("BulletSpeed", Some(format!("{} {}", plain(speed), words("m/s"))), Show::Compact));
    }
    let accuracy = num(item, "ammoAccr");
    if accuracy != 0.0 {
        let mut accuracy_row = row("CenterOfImpact", Some(format!("{accuracy:.1}%")), Show::Compact);
        accuracy_row.colored = Some(accuracy);
        figures.rows.push(accuracy_row);
    }
    let recoil = num(item, "ammoRec");
    if recoil != 0.0 {
        let mut recoil_row = row("Recoil", Some(plain(recoil)), Show::Compact);
        (recoil_row.colored, recoil_row.less_is_good) = (Some(recoil), true);
        figures.rows.push(recoil_row);
    }
    for id in ["HeavyBleedingDelta", "LightBleedingDelta"] {
        let chance = num(item, id);
        if chance != 0.0 {
            figures.rows.push(row(id, Some(percent(chance)), Show::Compact));
        }
    }
}

/// Armour's rows, in the order its class makes them (`ArmoredEquipment`:
/// its own armour's, its penalties', its eyes' protection, then what holds
/// armour moves its zones and class to the end with the ricochet between;
/// a `Vest` its penalties before its own armour's).
fn armour(item: &Item, db: &Database, num: &dyn Fn(&Item, &str) -> f32, words: &dyn Fn(&str) -> String, row: &dyn Fn(&'static str, Option<String>, Show) -> Row, figures: &mut Figures) {
    let template = db.items.get(&item.kind.template).map(|template| &template.props);
    let text = |name: &str| template.and_then(|props| props.get(name)).and_then(Json::as_str).unwrap_or("").to_owned();
    let list = |item: &Item, name: &str| -> Vec<String> {
        db.items.get(&item.kind.template).and_then(|template| template.props.get(name)).and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_str).map(str::to_owned).collect()
    };
    let vest = is(item, "Vest");
    let equipment = is(item, "ArmoredEquipment");
    if !vest && !equipment {
        return;
    }
    // Slots for armour (an insert's or a plate's: any but an attachment's).
    let armour_slots = item.kind.slots.iter().any(|holder| !holder.name.starts_with("mod_"));
    let own = has_armour(item, num);
    let others: Vec<&Item> = parts(item).into_iter().skip(1).filter(|part| has_armour(part, num)).collect();
    // Its zones' names in the install's words, once each (`UniqueLocalizedZones`).
    let zone_names = |of: &[&Item]| -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for part in of {
            for zone in list(part, "armorColliders").into_iter().chain(list(part, "armorPlateColliders")) {
                let said = words(&format!("Armor Zone {zone}"));
                if !names.contains(&said) {
                    names.push(said);
                }
            }
        }
        names
    };
    let armours: Vec<&Item> = parts(item).into_iter().filter(|part| has_armour(part, num)).collect();
    let own_rows = |figures: &mut Figures| {
        if !own {
            return;
        }
        let zones = zone_names(&[item]);
        if !zones.is_empty() {
            figures.rows.push(row("ArmorZone", Some(zones.join("\n")), if zones.len() > 1 { Show::Dropdown } else { Show::Compact }));
        }
        // With more than one armour in it, each one's short name and class.
        let class = if armours.len() > 1 {
            armours.iter().map(|part| format!("{}: {}", words(&format!("{} ShortName", part.kind.template)), plain(num(part, "armorClass")))).collect::<Vec<_>>().join("\n")
        } else {
            plain(num(item, "armorClass"))
        };
        figures.rows.push(row("ArmorClass", Some(class), if armours.len() > 1 { Show::Dropdown } else { Show::Compact }));
        figures.rows.push(row("ArmorMaterial", Some(words(&format!("Mat{}", text("ArmorMaterial")))), Show::Compact));
        let kind = text("ArmorType");
        if kind == "Light" || kind == "Heavy" {
            figures.rows.push(row("ArmorType", Some(words(&kind)), Show::Compact));
        }
    };
    let penalty_rows = |figures: &mut Figures| {
        for (id, name, shown) in [("ChangeMovementSpeed", "speedPenaltyPercent", true), ("ChangeTurningSpeed", "mousePenalty", true), ("Ergonomics", "weaponErgonomicPenalty", true)] {
            let value = num(item, name) + others.iter().map(|part| num(part, name)).sum::<f32>();
            if shown && (armour_slots || num(item, name) != 0.0) {
                let mut penalty = row(id, Some(format!("{} %", plain(value))), Show::Compact);
                penalty.colored = Some(value);
                figures.rows.push(penalty);
            }
        }
    };
    let any_penalty = ["speedPenaltyPercent", "mousePenalty", "weaponErgonomicPenalty"].iter().any(|name| num(item, name) != 0.0) || armour_slots;
    if vest {
        if any_penalty {
            penalty_rows(figures);
        }
        own_rows(figures);
    } else {
        own_rows(figures);
        if any_penalty {
            penalty_rows(figures);
        }
        let blindness = num(item, "BlindnessProtection");
        if blindness != 0.0 {
            figures.rows.push(row("BlindnessProtection", Some(percent(blindness)), Show::Compact));
        }
    }
    // What holds armour (`ArmorHolderComponent`): body armour; a rig,
    // headwear or a face cover with a class or slots for armour.
    let holder = is(item, "Armor") || ((vest || is(item, "Headwear") || is(item, "FaceCover")) && (num(item, "armorClass") > 0.0 || armour_slots));
    if !holder {
        return;
    }
    let zones = zone_names(&armours);
    figures.rows.retain(|row| row.id != "ArmorZone" && row.id != "ArmorClass");
    let mut zone_row = row("ArmorZone", None, if zones.len() > 1 { Show::Dropdown } else { Show::Compact });
    zone_row.tip = zones.iter().cloned().map(TipLine::words).collect();
    figures.rows.push(zone_row);
    if is(item, "Headwear") || is(item, "FaceCover") {
        let head = parts(item).into_iter().find(|part| has_armour(part, num));
        let top = head.and_then(|part| db.items.get(&part.kind.template)).and_then(|template| template.props.get("RicochetParams")).and_then(|params| params.get("y")).and_then(number).unwrap_or(0.0);
        let chance = if top > 0.3 { "RHigh" } else if top > 0.15 { "RMed" } else { "RLow" };
        figures.rows.push(row("Ricochet", Some(words(chance)), Show::Compact));
    }
    let class = armours.first().map_or(0.0, |part| num(part, "armorClass"));
    let mut class_row = row("ArmorClass", Some(plain(class)), if armours.len() > 1 { Show::Dropdown } else { Show::Compact });
    class_row.tip = plate_lines(item, db, num, words);
    figures.rows.push(class_row);
}

/// A round's chance through each armour class as its dropdown says it
/// (`MultiLineInfo(penetrationPower)`): for classes 1 to 6, its mark and
/// the install's words for the chance (`VisualizeArmorPenetrationChance`:
/// the chance through new armour of the class, `RealResistance` at 100 of
/// 100: under 20 very low, under 40 low, under 60 medium, under 80 high,
/// else very high).
fn penetration_lines(db: &Database, penetration: f32, words: &dyn Fn(&str) -> String) -> Vec<TipLine> {
    let Ok(rules) = crate::armor::ArmorRules::new(&db.config) else { return Vec::new() };
    (1..=6)
        .map(|class| {
            let chance = crate::armor::Resistance::of(100.0, 100.0, rules.resistance_of(class), penetration).chance(penetration);
            let key = match chance {
                c if c < 20.0 => "UI/ArmorPenetration/VeryLow",
                c if c < 40.0 => "UI/ArmorPenetration/Low",
                c if c < 60.0 => "UI/ArmorPenetration/Medium",
                c if c < 80.0 => "UI/ArmorPenetration/High",
                _ => "UI/ArmorPenetration/VeryHigh",
            };
            TipLine { class: Some(class as u32), text: words(key), ..TipLine::default() }
        })
        .collect()
}

/// What holds armour's class dropdown (`MultiLineInfo(item, false)`): the
/// install's words for ARMOR POINTS, a break, then each plate in its own
/// slots (an `ArmorSlot` with an armour plate in it), the locked ones first
/// and each group by its class downwards, a break between the locked and
/// the rest; each its class's mark where the class changes (else as much
/// room), its slot's name in the install's words, " - ", its durability and
/// its greatest to a place.
fn plate_lines(item: &Item, db: &Database, num: &dyn Fn(&Item, &str) -> f32, words: &dyn Fn(&str) -> String) -> Vec<TipLine> {
    let template = db.items.get(&item.kind.template);
    let mut plates: Vec<(bool, u32, String, [f32; 2])> = item
        .kind
        .slots
        .iter()
        .zip(&item.slots)
        .filter(|(holder, part)| holder.armour && part.as_ref().is_some_and(|part| is(part, "ArmorPlate")))
        .filter_map(|(holder, part)| {
            let part = part.as_ref()?;
            let locked = template.and_then(|template| crate::armor::ArmorSlot::of(template, &holder.name)).is_some_and(|slot| slot.locked);
            Some((locked, num(part, "armorClass") as u32, words(&holder.name), part.durability()))
        })
        .collect();
    plates.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    let mut lines = vec![TipLine::words(words("ARMOR POINTS")), TipLine { divider: true, ..TipLine::default() }];
    let (mut seen_locked, mut broke, mut last) = (false, false, None);
    for (locked, class, name, [now, most]) in plates {
        seen_locked |= locked;
        if !locked && !broke && seen_locked {
            lines.push(TipLine { divider: true, ..TipLine::default() });
            broke = true;
            last = None;
        }
        let mark = (last != Some(class)).then_some(class);
        last = Some(class);
        let round = |value: f32| trimmed((value * 10.0).round() / 10.0, 1);
        lines.push(TipLine { class: mark, indent: mark.is_none(), text: format!("{name} - {} / {}", round(now), round(most)), divider: false });
    }
    lines
}
