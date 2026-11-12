//! The range's dummies' kit, armed builds, crates of a kind and the tables' loads.

use super::*;

/// Kalashnikov AKM 7.62x39.
pub(super) const DEFAULT_WEAPON: &str = "59d6088586f774275f37482f";

/// The equipment slots whose things are the character's own, which every run
/// begins with (the pockets and the dogtag).
pub(super) const OWN_SLOTS: [&str; 2] = ["Pockets", "Dogtag"];

/// A BEAR's dogtag (`dogtags_bear`).
pub(super) const BEAR_DOGTAG: &str = "59f32bb586f774757e1e8442";

/// The crate on the range: the install's airdrop crate, a loot container
/// with one grid of ten by twelve cells, which is the smallest of the
/// install's that holds a rifle, a vest and a backpack together (its
/// weapon boxes are five by five at most).
pub(super) const CRATE: &str = "6223349b3136504a544d1608";

/// How many cells the crate has here: the install's ten across, and
/// thirty down where the install's has twelve. **The app's own**: larger
/// inside, with a scroll bar, to hold more.
pub(super) const CRATE_CELLS: [u32; 2] = [10, 30];

/// How many full stacks of its own rounds lie in the crate for each
/// weapon the kit is not made for, and how many full magazines for each
/// such weapon fed from magazines. The app's choice.
const SPARE_STACKS: usize = 2;

const SPARE_MAGAZINES: usize = 3;

/// For a weapon whose magazine comes out to be changed
/// (`ReloadMode` `ExternalMagazine`, or the Model 700's stock magazine:
/// `Database::changes_magazines`): the kind of magazine its stock
/// preset has, how many rounds that holds, and the round the weapon
/// names as its own (`defAmmo`).
fn fed_from_magazines(db: &trp_data::Database, weapon: &str) -> Option<(std::sync::Arc<trp_sim::items::inventory::Kind>, u32, std::sync::Arc<trp_sim::items::inventory::Kind>)> {
    let item = db.items.get(weapon)?;
    if !db.changes_magazines(weapon) {
        return None;
    }
    let preset = db.default_preset(weapon).ok()?;
    let magazine = &preset.items.iter().find(|part| part.slot.as_deref() == Some(trp_sim::items::inventory::MAGAZINE_SLOT))?.template;
    let capacity = db.items.get(magazine)?.props.get("Cartridges")?.get(0)?.get("_max_count")?.as_u64()? as u32;
    let round = item.props.get("defAmmo")?.as_str()?;
    let kind = |template: &str| trp_sim::items::inventory::Kind::of(db, template).ok().map(std::sync::Arc::new);
    Some((kind(magazine)?, capacity, kind(round)?))
}

/// A weapon as its stock preset has it, with what loads it: fed from
/// magazines, its own full (as the hands have it when it is drawn) and,
/// with `spares`, `SPARE_MAGAZINES` more of the kind, full of its own
/// round, which its reloads take (firing spec, "The quick reload");
/// otherwise, with `spares`, `SPARE_STACKS` full stacks of that round,
/// which a tube, a cylinder or barrels are filled from. The weapon first.
/// `--preset`'s build of the run's weapon: one of the database's by its id,
/// or one in the database's own form (`_id`, `_parent`, `_items` with
/// `_tpl`, `parentId`, `slotId`) in a file of that name.
pub(super) fn preset_build(db: &trp_data::Database, wanted: &str, weapon: &str) -> anyhow::Result<trp_data::setup::database::Preset> {
    use anyhow::Context;
    let build = match db.presets.iter().find(|preset| preset.id == wanted) {
        Some(preset) => preset.clone(),
        None => serde_json::from_str(&std::fs::read_to_string(wanted).with_context(|| format!("no build {wanted} in the database, nor a file of that name"))?)?,
    };
    let root = build.items.iter().find(|item| item.id == build.root).context("the build has no item at its root")?;
    anyhow::ensure!(root.template == weapon, "the build is of {}, not of the run's weapon {weapon} (`--weapon`)", root.template);
    Ok(build)
}

/// A dummy's own kit (`dummy::SIDES`), a PMC's starting kit: the equipment a
/// new character of that side starts with
/// (`profiles.json`, `dummy::KIT_EDITION`), with the AKM in its hands in
/// its rifle's slot (as its stock preset has it, loaded) and its magazines
/// and rounds made the AKM's (`Loadout::fit_to`, as the range's crate kit),
/// as a Scav's body has it, neither dog tag nor armband (`search::
/// scav_body`). As it dies the AKM stays in that slot, the corpse's thing
/// in hand, as the game keeps a weapon in hand in its slot (`Player.
/// CreateCorpse`, `Corpse.ItemInHands`), its model hung on the strap until
/// the thing leaves the slot (`dummy::clear_dummy_weapons`).
pub(super) fn dummy_kit(install: &Install, db: &trp_data::Database, side: &str) -> anyhow::Result<trp_sim::items::inventory::Item> {
    use anyhow::Context;
    let mut list = trp_data::character::inventory::Loadout::load(install, world::dummy::KIT_EDITION, side)?;
    list.fit_to(db, world::dummy::WEAPON)?;
    let (mut equipment, _) = trp_sim::items::inventory::Equipment::of_loadout(db, &list)?;
    let weapon = armed(db, world::dummy::WEAPON, false, Some(&world::dummy::build(db)?))?.into_iter().next().context("no AKM was made")?;
    let slot = equipment.item.slot_index("FirstPrimaryWeapon").context("the equipment has no rifle's slot")?;
    equipment.item.slots[slot] = Some(weapon);
    Ok(trp_sim::items::search::scav_body(&equipment.item))
}

/// A dummy's own look, as SPT's server dresses a PMC bot of its side
/// (`BotGenerator.SetBotAppearance`: its head, its legs and its body, in
/// that order, each a weighted pick of the side's list, `bots/types/
/// <side>.json`; `trp_sim::bots::weighted`), as the bundles of its upper
/// body, legs and head (`body_view::LOOK_UPPER` and on; none where a part
/// names none). The draws are the app's own (`Dice`), from the run's start.
pub(super) fn dummy_look(install: &Install, customizations: &trp_data::character::suits::Customizations, side: &str, dice: &mut trp_sim::items::search::Dice) -> anyhow::Result<[String; 3]> {
    let looks = trp_data::character::suits::bot_looks(install, side)?;
    let mut pick = |list: &[(String, f64)]| trp_sim::characters::bots::weighted(list, dice.unit()).and_then(|id| customizations.bundle(id)).unwrap_or_default();
    let head = pick(&looks.head);
    let feet = pick(&looks.feet);
    let body = pick(&looks.body);
    Ok([body, feet, head])
}

/// A dummy's voice, as SPT's server draws a PMC bot's (`BotGenerator.
/// GenerateBot`: a weighted pick of its side's `appearance.voice`,
/// `trp_sim::bots::weighted`), by the name its phrases are found by
/// (`Customizations::voice_name`). The draws are the app's own (`Dice`).
pub(super) fn dummy_voice(install: &Install, customizations: &trp_data::character::suits::Customizations, side: &str, dice: &mut trp_sim::items::search::Dice) -> anyhow::Result<Option<String>> {
    let looks = trp_data::character::suits::bot_looks(install, side)?;
    Ok(trp_sim::characters::bots::weighted(&looks.voice, dice.unit()).and_then(|id| customizations.voice_name(id)))
}

pub(super) fn armed(db: &trp_data::Database, weapon: &str, spares: bool, build: Option<&trp_data::setup::database::Preset>) -> anyhow::Result<Vec<trp_sim::items::inventory::Item>> {
    use trp_sim::items::inventory::{fill_magazine, Item, Kind, MAGAZINE_SLOT};
    // Its stock build, or the one asked for (`--preset`).
    let list = match build {
        Some(build) => trp_data::character::inventory::Loadout::of_build(build),
        None => trp_data::character::inventory::Loadout::of_preset(db, weapon)?,
    };
    let (mut item, _) = Item::of_list(db, &list)?;
    let mut made = Vec::new();
    match fed_from_magazines(db, weapon) {
        Some((magazine, capacity, round)) => {
            if let Some(fitted) = item.slot_index(MAGAZINE_SLOT).and_then(|slot| item.slots[slot].as_mut()) {
                fill_magazine(fitted, round.clone(), capacity);
            }
            for _ in (0..SPARE_MAGAZINES).filter(|_| spares) {
                let mut spare = Item::new(magazine.clone());
                fill_magazine(&mut spare, round.clone(), capacity);
                made.push(spare);
            }
            // A magazine of its own that comes out (the Model 700's) is
            // also filled a round at a time where no magazine with rounds
            // is within reach: its rounds too.
            let own_magazine = db.items.get(weapon).and_then(|item| item.props.get("ReloadMode")?.as_str()) == Some("InternalMagazine");
            for _ in (0..SPARE_STACKS).filter(|_| spares && own_magazine) {
                let mut stack = Item::new(round.clone());
                stack.count = db.items.get(&round.template).map_or(1, |template| template.stack_max());
                made.push(stack);
            }
        }
        // A rocket launcher's controller has no state to reload in: it
        // has its rocket and no more (more-weapons spec).
        None if !db.items.get(weapon).is_some_and(|item| db.is_a(item, "RocketLauncher")) => {
            let round = db.items.get(weapon).and_then(|item| item.props.get("defAmmo")?.as_str()).and_then(|round| db.items.get(round));
            if let Some(round) = round {
                let kind = std::sync::Arc::new(Kind::of(db, &round.id)?);
                // A tube full, as the hands have it when it is drawn.
                if let Some(tube) = item.slot_index(MAGAZINE_SLOT).and_then(|slot| item.slots[slot].as_mut()).filter(|tube| tube.kind.capacity > 0) {
                    let capacity = tube.kind.capacity;
                    fill_magazine(tube, kind.clone(), capacity);
                }
            }
            if let Some(round) = round.filter(|_| spares) {
                let kind = std::sync::Arc::new(Kind::of(db, &round.id)?);
                for _ in 0..SPARE_STACKS {
                    let mut stack = Item::new(kind.clone());
                    stack.count = round.stack_max();
                    made.push(stack);
                }
            }
        }
        None => {}
    }
    made.insert(0, item);
    Ok(made)
}

/// Whether the simulation has the logic a weapon runs on
/// (`docs/spec/more-weapons.md`): every kind of the install's but a weapon
/// fired once and thrown away (`IsOneoff`, whose firing is not read; no
/// weapon with a stock preset in the install read is one).
pub(super) fn logic_built(db: &trp_data::Database, weapon: &str) -> bool {
    let Some(item) = db.items.get(weapon) else { return false };
    !item.props.get("IsOneoff").and_then(serde_json::Value::as_bool).unwrap_or(false)
}

/// Which crate of weapons a weapon goes into (`range::WEAPON_CRATES`): a
/// weapon of one's own is custom; any other goes by the class its template
/// gives it (`weapClass`, which the weapon skills go by too; the item's
/// place among the database's classes would have the M32A1 a revolver).
pub(super) fn crate_of(db: &trp_data::Database, weapon: &str) -> Option<usize> {
    if db.custom.contains_key(weapon) {
        return Some(7);
    }
    match db.items.get(weapon)?.props.get("weapClass")?.as_str()? {
        "assaultRifle" | "assaultCarbine" => Some(0),
        "smg" => Some(1),
        "machinegun" => Some(2),
        "sniperRifle" | "marksmanRifle" => Some(3),
        "shotgun" => Some(4),
        "pistol" => Some(5),
        "grenadeLauncher" | "specialWeapon" => Some(6),
        _ => None,
    }
}

/// The game's own weapon box and lootable ammunition box (`WeaponCrate`, a
/// grid of 5 by 2; `AmmoBox`, 3 by 3), which the tables of weapons' gun
/// cases and magazine boxes are.
pub(super) const GUN_CASE: &str = "578f87ad245977356274f2cc";

pub(super) const MAGAZINE_BOX: &str = "5909d45286f77465a8136dc6";

/// The templates a slot's or a magazine's filter names, a class among them
/// standing for every item under it: those with a model, not a quest's, not
/// hidden as unfinished (`unfinished_items`).
fn filtered(db: &trp_data::Database, takes: &[String], hidden: &std::collections::HashSet<String>) -> Vec<String> {
    let quest = |item: &trp_data::setup::database::ItemTemplate| item.props.get("QuestItem").and_then(serde_json::Value::as_bool).unwrap_or(false);
    let under = |item: &trp_data::setup::database::ItemTemplate, class: &str| {
        let mut at = Some(item);
        while let Some(template) = at {
            if template.id == class {
                return true;
            }
            at = db.items.get(&template.parent);
        }
        false
    };
    let mut found = Vec::new();
    for id in takes {
        let Some(item) = db.items.get(id) else { continue };
        if item.prefab_key().is_some() {
            if !quest(item) {
                found.push(id.clone());
            }
            continue;
        }
        found.extend(db.items.values().filter(|other| other.prefab_key().is_some() && !quest(other) && under(other, id)).map(|other| other.id.clone()));
    }
    found.retain(|id| !hidden.contains(id));
    found.sort();
    found.dedup();
    found
}

/// What a table's weapons load: one of every magazine any of them
/// takes, as its magazine slot's filter says (weapons fed from magazines;
/// its stock preset's own too), full of the first weapon's own round where
/// the magazine takes it, else of the first round it takes; and a full
/// stack of every round any of them or their magazines take (the chambers'
/// filters, and the magazines'), by name; none hidden as unfinished. The
/// app's.
pub(super) fn table_loads(db: &trp_data::Database, weapons: &[&String], hidden: &std::collections::HashSet<String>) -> (Vec<trp_sim::items::inventory::Item>, Vec<trp_sim::items::inventory::Item>) {
    use trp_sim::items::inventory::{fill_magazine, Item, Kind, MAGAZINE_SLOT};
    let kind = |template: &str| Kind::of(db, template).ok().map(std::sync::Arc::new);
    // Each magazine model with the round of the first weapon that takes it.
    let mut magazines: std::collections::BTreeMap<String, Option<String>> = std::collections::BTreeMap::new();
    let mut rounds: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for weapon in weapons {
        let Some(item) = db.items.get(weapon.as_str()) else { continue };
        let own_round = item.props.get("defAmmo").and_then(serde_json::Value::as_str).map(str::to_owned);
        for chamber in item.slots().into_iter().filter(|slot| slot.name.starts_with("patron_in_weapon")) {
            rounds.extend(filtered(db, &chamber.takes, hidden));
        }
        if !db.changes_magazines(weapon) {
            // A tube, a cylinder or barrels: what is fitted takes rounds,
            // which the box of rounds has.
            if let Some(round) = &own_round {
                rounds.insert(round.clone());
            }
            continue;
        }
        let mut takes: Vec<String> = item.slots().into_iter().filter(|slot| slot.name == MAGAZINE_SLOT).flat_map(|slot| filtered(db, &slot.takes, hidden)).collect();
        if let Ok(preset) = db.default_preset(weapon) {
            takes.extend(preset.items.iter().filter(|part| part.slot.as_deref() == Some(MAGAZINE_SLOT)).map(|part| part.template.clone()));
        }
        for magazine in takes {
            if db.items.get(&magazine).is_some_and(|it| db.is_a(it, "Magazine")) && !hidden.contains(&magazine) {
                magazines.entry(magazine).or_insert_with(|| own_round.clone());
            }
        }
    }
    let mut loaded = Vec::new();
    for (magazine, own_round) in &magazines {
        let Some(template) = db.items.get(magazine) else { continue };
        let takes: Vec<String> = template.cartridges().into_iter().flat_map(|holder| filtered(db, &holder.takes, hidden)).collect();
        rounds.extend(takes.iter().cloned());
        let capacity = template.props.get("Cartridges").and_then(|c| c.get(0)?.get("_max_count")?.as_u64()).unwrap_or(0) as u32;
        let round = own_round.as_ref().filter(|round| takes.contains(round)).or_else(|| takes.first());
        let Some(kind_of) = kind(magazine) else {
            eprintln!("no magazine {magazine} for the tables");
            continue;
        };
        let mut item = Item::new(kind_of);
        if let Some(round) = round.and_then(|round| kind(round)) {
            if capacity > 0 {
                fill_magazine(&mut item, round, capacity);
            }
        }
        loaded.push(item);
    }
    // Rounds only (a filter may name a grenade launcher's grenades: those
    // are rounds too), each a full stack, by calibre and then by name.
    let caliber = |item: &trp_data::setup::database::ItemTemplate| item.props.get("Caliber").and_then(serde_json::Value::as_str).unwrap_or("").to_owned();
    let mut named: Vec<(String, &String, &String)> = rounds.iter().filter(|id| !hidden.contains(*id)).filter_map(|id| db.items.get(id).filter(|it| db.is_a(it, "Ammo")).map(|it| (caliber(it), &it.name, &it.id))).collect();
    named.sort();
    let mut stacks = Vec::new();
    for (_, _, id) in named {
        let Some(round) = db.items.get(id) else { continue };
        let Some(kind_of) = kind(id) else { continue };
        let mut stack = Item::new(kind_of);
        stack.count = round.stack_max();
        stacks.push(stack);
    }
    (loaded, stacks)
}

/// What each table of weapons holds, in `range::WEAPON_CRATES`' order: in
/// its gun case every weapon of the install of its class that has a stock
/// preset and runs on a logic built, and every weapon of one's own, by
/// name, its own magazine full; in its magazine box one of every magazine
/// they take, full, and in its box of rounds (the tables of weapons' alone,
/// `range::AMMO_TABLES`) a stack of every round (`table_loads`). Each is its
/// game's container, ten cells across as the range's crates are, and as
/// deep as what it holds (the app's: the game's own hold one weapon, and
/// nine things). None of them is a thing hidden as unfinished
/// (`unfinished_items`).
pub(super) type Crates = (Vec<trp_sim::items::inventory::Item>, Vec<trp_sim::items::inventory::Item>, Vec<trp_sim::items::inventory::Item>);
