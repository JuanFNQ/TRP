//! What a starting character carries, as SPT's database has it
//! (`templates/profiles.json`), and what the database says of an item as
//! something to carry: its class, weight, cells, slots and grids
//! (`docs/spec/inventory.md`, "What the install holds").

use anyhow::{anyhow, Context, Result};
use serde_json::Value as Json;

use crate::database::{Database, ItemTemplate};
use crate::install::Install;

/// One entry of a character's inventory: an item, and where it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Carried {
    pub id: String,
    pub template: String,
    /// The entry it is in.
    pub parent: Option<String>,
    /// The slot or the grid it is in there, by name (`cartridges` for the
    /// rounds in a magazine).
    pub slot: Option<String>,
    /// In a grid: the column, the row, and whether it lies turned.
    pub place: Option<(u32, u32, bool)>,
    /// How many are in the stack.
    pub count: u32,
}

/// The equipment of a character a new account starts with.
#[derive(Debug, Clone)]
pub struct Loadout {
    /// The id of the equipment item itself, which everything worn is in.
    pub equipment: String,
    pub items: Vec<Carried>,
}

impl Loadout {
    /// `edition` as the database names it (`Standard`, `Edge Of Darkness`
    /// and so on) and `side` `usec` or `bear`. Only what is in the
    /// equipment is kept, not the stash.
    pub fn load(install: &Install, edition: &str, side: &str) -> Result<Self> {
        let path = install.database_dir().join("templates/profiles.json");
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let profiles: Json = serde_json::from_str(&text).context("profiles.json")?;
        let inventory = profiles
            .get(edition)
            .and_then(|e| e.get(side))
            .and_then(|s| s.pointer("/character/Inventory"))
            .ok_or_else(|| anyhow!("the database has no starting character {edition}/{side}"))?;
        let equipment = inventory.get("equipment").and_then(Json::as_str).context("the inventory names no equipment")?.to_owned();
        let text = |v: &Json, name: &str| v.get(name).and_then(Json::as_str).map(str::to_owned);
        let all: Vec<Carried> = inventory
            .get("items")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let place = entry.get("location").filter(|l| l.is_object()).map(|l| {
                    let cell = |name: &str| l.get(name).and_then(Json::as_u64).unwrap_or(0) as u32;
                    // The turn is written as a number or as a word.
                    let turned = match l.get("r") {
                        Some(Json::String(word)) => word == "Vertical",
                        Some(other) => other.as_u64() == Some(1),
                        None => false,
                    };
                    (cell("x"), cell("y"), turned)
                });
                Some(Carried {
                    id: text(entry, "_id")?,
                    template: text(entry, "_tpl")?,
                    parent: text(entry, "parentId"),
                    slot: text(entry, "slotId"),
                    place,
                    count: entry.pointer("/upd/StackObjectsCount").and_then(Json::as_u64).unwrap_or(1) as u32,
                })
            })
            .collect();
        // The equipment and everything under it, parents before children.
        let mut items: Vec<Carried> = all.iter().filter(|item| item.id == equipment).cloned().collect();
        let mut at = 0;
        while at < items.len() {
            let parent = items[at].id.clone();
            items.extend(all.iter().filter(|item| item.parent.as_deref() == Some(parent.as_str())).cloned());
            at += 1;
        }
        if items.is_empty() {
            return Err(anyhow!("the starting character {edition}/{side} has no equipment item {equipment}"));
        }
        Ok(Self { equipment, items })
    }
}

/// What a recording says the game made of one carried item: its size in
/// cells with its parts on, and its weight with all that is in it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recorded {
    pub cells: [u32; 2],
    pub weight: f32,
}

impl Loadout {
    /// The equipment a recording's header lists (`equipment`, recorder
    /// 0.5.0 on): a list of the same kind, the equipment itself first,
    /// and beside each entry what the game made of the item.
    pub fn of_recording(list: &Json) -> Result<(Self, Vec<Recorded>)> {
        let entries = list.as_array().context("the recording's equipment is not a list")?;
        let text = |entry: &Json, name: &str| entry.get(name).and_then(Json::as_str).map(str::to_owned);
        let whole = |entry: &Json, name: &str| entry.get(name).and_then(Json::as_u64).map(|v| v as u32);
        let (mut items, mut recorded) = (Vec::new(), Vec::new());
        for entry in entries {
            items.push(Carried {
                id: text(entry, "id").context("an entry without an id")?,
                template: text(entry, "tpl").context("an entry without a template")?,
                parent: text(entry, "in"),
                slot: text(entry, "at"),
                place: whole(entry, "x").zip(whole(entry, "y")).map(|(x, y)| (x, y, whole(entry, "r") == Some(1))),
                count: whole(entry, "n").unwrap_or(1).max(1),
            });
            let weight = entry.get("kg").and_then(Json::as_f64).unwrap_or(0.0) as f32;
            recorded.push(Recorded { cells: [whole(entry, "w").unwrap_or(0), whole(entry, "h").unwrap_or(0)], weight });
        }
        let equipment = items.first().context("the recording's equipment is empty")?.id.clone();
        Ok((Self { equipment, items }, recorded))
    }

    /// A weapon as its stock preset in the database has it, as a list of
    /// the same kind: the weapon itself first, then its parts.
    pub fn of_preset(db: &Database, weapon_template: &str) -> Result<Self> {
        let preset = db.default_preset(weapon_template)?;
        let entry = |item: &crate::database::PresetItem| Carried {
            id: item.id.clone(),
            template: item.template.clone(),
            parent: item.parent.clone().filter(|_| item.id != preset.root),
            slot: item.slot.clone(),
            place: None,
            count: 1,
        };
        let mut items: Vec<Carried> = preset.items.iter().filter(|item| item.id == preset.root).map(entry).collect();
        let mut at = 0;
        while at < items.len() {
            let parent = items[at].id.clone();
            items.extend(preset.items.iter().filter(|item| item.id != preset.root && item.parent.as_deref() == Some(parent.as_str())).map(entry));
            at += 1;
        }
        Ok(Self { equipment: preset.root.clone(), items })
    }
}

impl Loadout {
    /// The same list made to go with a weapon: every magazine the weapon's
    /// magazine slot does not take becomes the magazine of the weapon's
    /// stock preset, and every round its chamber does not take becomes
    /// its default round (`defAmmo`), with no more to a magazine or to a
    /// stack than each holds. For a weapon whose magazine stays in it
    /// (`ReloadMode` `InternalMagazine`: a shotgun's tube) a spare
    /// magazine is of no use, and each becomes a full stack of the
    /// weapon's rounds, what was in it left out. Returns how many
    /// magazines and how many stacks of rounds were changed. What the
    /// database says of each (weight, cells, capacity) is then that of the
    /// new thing.
    pub fn fit_to(&mut self, db: &Database, weapon_template: &str) -> Result<[usize; 2]> {
        let weapon = db.items.get(weapon_template).ok_or_else(|| anyhow!("no item {weapon_template}"))?;
        let takes_magazine: Vec<String> = weapon.slots().into_iter().filter(|slot| slot.name == "mod_magazine").flat_map(|slot| slot.takes).collect();
        // A weapon with barrels and nothing else has no magazine at all.
        let own_magazine = db
            .default_preset(weapon_template)?
            .items
            .iter()
            .find(|part| part.slot.as_deref() == Some("mod_magazine"))
            .map(|part| part.template.clone());
        // A revolver's rounds go into its cylinder's chambers: the weapon
        // has none of its own.
        let mut takes_round: Vec<String> = weapon.holders("Chambers").into_iter().flat_map(|chamber| chamber.takes).collect();
        if let Some(cylinder) = own_magazine.as_ref().and_then(|magazine| db.items.get(magazine)).filter(|_| takes_round.is_empty()) {
            takes_round = cylinder.holders("Slots").into_iter().filter(|slot| slot.name.starts_with("camora_")).flat_map(|camora| camora.takes).collect();
        }
        let own_round = weapon
            .props
            .get("defAmmo")
            .and_then(Json::as_str)
            .map(str::to_owned)
            .ok_or_else(|| anyhow!("{weapon_template} names no round of its own"))?;
        let is = |template: &str, class: &str| db.class_names(template).iter().any(|name| name == class);
        let capacity = own_magazine
            .as_ref()
            .and_then(|magazine| db.items.get(magazine))
            .and_then(|magazine| magazine.props.get("Cartridges")?.get(0)?.get("_max_count")?.as_u64())
            .unwrap_or(1) as u32;
        let stack = db.items.get(&own_round).map_or(1, ItemTemplate::stack_max);
        // Filled with loose rounds: a magazine that stays in the weapon
        // (a tube, a cylinder), or barrels loaded one by one.
        let internal = own_magazine.is_none() || matches!(weapon.props.get("ReloadMode").and_then(Json::as_str), Some("InternalMagazine" | "OnlyBarrel"));
        let own_magazine = own_magazine.unwrap_or_default();
        let mut changed = [0; 2];
        if internal {
            // The spare magazines become rounds, and what they held goes.
            let spare: Vec<String> = self.items.iter().filter(|item| is(&item.template, "Magazine") && item.slot.as_deref() != Some("mod_magazine")).map(|item| item.id.clone()).collect();
            self.items.retain(|item| !item.parent.as_ref().is_some_and(|parent| spare.contains(parent)));
            for item in self.items.iter_mut().filter(|item| spare.contains(&item.id)) {
                (item.template, item.count) = (own_round.clone(), stack);
                changed[0] += 1;
            }
        }
        for item in &mut self.items {
            // (A weapon with no magazine leaves the one in another weapon be.)
            if !own_magazine.is_empty() && is(&item.template, "Magazine") && !takes_magazine.contains(&item.template) {
                item.template = own_magazine.clone();
                changed[0] += 1;
            } else if is(&item.template, "Ammo") && !takes_round.contains(&item.template) {
                item.template = own_round.clone();
                changed[1] += 1;
            }
        }
        // A round keeps its number as far as what it is in, or its own
        // stack, holds it.
        let magazines: Vec<String> = self.items.iter().filter(|item| item.template == own_magazine).map(|item| item.id.clone()).collect();
        for item in self.items.iter_mut().filter(|item| item.template == own_round) {
            let in_magazine = item.parent.as_ref().is_some_and(|parent| magazines.contains(parent));
            item.count = item.count.min(if in_magazine { capacity } else { stack });
        }
        Ok(changed)
    }
}

/// The game's short name for every item, by template, in one of the
/// languages the database has texts for (`locales/global/en.json`).
pub fn short_names(install: &Install, language: &str) -> Result<std::collections::HashMap<String, String>> {
    let path = install.database_dir().join(format!("locales/global/{language}.json"));
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let texts: std::collections::HashMap<String, Json> = serde_json::from_str(&text).with_context(|| path.display().to_string())?;
    let short = |(key, value): (String, Json)| Some((key.strip_suffix(" ShortName")?.to_owned(), value.as_str()?.to_owned()));
    Ok(texts.into_iter().filter_map(short).collect())
}

/// A slot of an item, or a grid: its name and what it takes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Holder {
    pub name: String,
    /// For a grid, how many cells wide and high.
    pub cells: [u32; 2],
    /// Templates and classes it takes; nothing listed takes anything.
    pub takes: Vec<String>,
    pub refuses: Vec<String>,
    /// Whether what is in it counts toward its owner's size, if the
    /// holder says so itself.
    pub merges: Option<bool>,
    /// A slot whose part the item cannot do without (`_required`): a
    /// vital part.
    pub required: bool,
}

impl Database {
    /// An item's classes: its own template and its parents', nearest
    /// first.
    pub fn classes(&self, template: &str) -> Vec<String> {
        let mut chain = Vec::new();
        let mut at = template;
        while let Some(item) = self.items.get(at) {
            chain.push(item.id.clone());
            at = item.parent.as_str();
        }
        chain
    }

    /// The database's own names of an item and the classes it belongs
    /// to, nearest first (`Magazine`, `Weapon`, `Backpack` and so on).
    pub fn class_names(&self, template: &str) -> Vec<String> {
        self.classes(template).iter().filter_map(|id| self.items.get(id)).map(|item| item.name.clone()).collect()
    }
}

impl ItemTemplate {
    fn holders(&self, list: &str) -> Vec<Holder> {
        let ids = |filter: &Json, name: &str| -> Vec<String> {
            filter.get(name).and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_str).map(str::to_owned).collect()
        };
        self.props
            .get(list)
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .map(|holder| {
                let filters = holder.pointer("/_props/filters").and_then(Json::as_array).cloned().unwrap_or_default();
                let cells = |name: &str| holder.pointer(&format!("/_props/{name}")).and_then(Json::as_u64).unwrap_or(0) as u32;
                Holder {
                    name: holder.get("_name").and_then(Json::as_str).unwrap_or("").to_owned(),
                    cells: [cells("cellsH"), cells("cellsV")],
                    takes: filters.iter().flat_map(|f| ids(f, "Filter")).collect(),
                    refuses: filters.iter().flat_map(|f| ids(f, "ExcludedFilter")).collect(),
                    merges: holder.get("_mergeSlotWithChildren").and_then(Json::as_bool),
                    required: holder.get("_required").and_then(Json::as_bool).unwrap_or(false),
                }
            })
            .collect()
    }

    /// The item's slots, each with what it takes; a weapon's chambers
    /// among them (the game keeps those in a list of their own, each
    /// named `patron_in_weapon`, and a chambered round is an item in one
    /// as a part is an item in a slot).
    pub fn slots(&self) -> Vec<Holder> {
        let mut slots = self.holders("Slots");
        slots.extend(self.holders("Chambers"));
        slots
    }

    /// The item's grids, each with its cells and what it takes and
    /// refuses.
    pub fn grids(&self) -> Vec<Holder> {
        self.holders("Grids")
    }

    /// Width and height in cells.
    pub fn cells(&self) -> [u32; 2] {
        let cell = |name: &str| self.props.get(name).and_then(Json::as_u64).unwrap_or(1) as u32;
        [cell("Width"), cell("Height")]
    }

    /// The cells it adds to what it is a part of: left, right, up, down.
    pub fn extra_cells(&self) -> [u32; 4] {
        ["ExtraSizeLeft", "ExtraSizeRight", "ExtraSizeUp", "ExtraSizeDown"].map(|name| self.props.get(name).and_then(Json::as_u64).unwrap_or(0) as u32)
    }

    /// Whether its parts count toward its size (`MergesWithChildren`).
    pub fn merges_with_parts(&self) -> bool {
        self.props.get("MergesWithChildren").and_then(Json::as_bool).unwrap_or(false)
    }

    /// How many of it one stack holds (`StackMaxSize`); 1 for what does
    /// not stack.
    pub fn stack_max(&self) -> u32 {
        self.props.get("StackMaxSize").and_then(Json::as_u64).unwrap_or(1).max(1) as u32
    }

    /// Whether, as a part, it may be put on or taken off in a raid
    /// (`RaidModdable`); true where the template does not say.
    pub fn raid_moddable(&self) -> bool {
        self.props.get("RaidModdable").and_then(Json::as_bool).unwrap_or(true)
    }

    /// The equipment slots it blocks while worn, by name: the template's
    /// `BlocksEarpiece`, `BlocksEyewear`, `BlocksFaceCover` and
    /// `BlocksHeadwear`.
    pub fn blocked_slots(&self) -> Vec<String> {
        let blocks = |slot: &str| self.props.get(format!("Blocks{slot}")).and_then(Json::as_bool) == Some(true);
        ["Earpiece", "Eyewear", "FaceCover", "Headwear"].into_iter().filter(|slot| blocks(slot)).map(str::to_owned).collect()
    }

    /// The equipment slots it is not taken out of in a raid, by name
    /// (`CantRemoveFromSlotsDuringRaid`): the pockets and the secured
    /// containers name their own.
    pub fn kept_in_slots(&self) -> Vec<String> {
        let listed = self.props.get("CantRemoveFromSlotsDuringRaid").and_then(Json::as_array);
        listed.into_iter().flatten().filter_map(Json::as_str).map(str::to_owned).collect()
    }
}

/// The game's own words for some lines of its interface, each by the name
/// the game's code asks for it by, in one of the languages the database
/// has texts for. A name is found whatever its case, as the game finds it
/// (the install has `OPENDOOR` for the code's `OpenDoor`); what comes back
/// is keyed by the name in small letters. A name the install has no words
/// for is left out.
pub fn texts(install: &Install, language: &str, names: &[&str]) -> Result<std::collections::HashMap<String, String>> {
    let path = install.database_dir().join(format!("locales/global/{language}.json"));
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let all: std::collections::HashMap<String, Json> = serde_json::from_str(&text).with_context(|| path.display().to_string())?;
    let wanted: Vec<String> = names.iter().map(|name| name.to_lowercase()).collect();
    Ok(all
        .into_iter()
        .filter_map(|(key, value)| {
            let key = key.to_lowercase();
            wanted.contains(&key).then(|| value.as_str().map(|words| (key, words.to_owned()))).flatten()
        })
        .collect())
}
