//! What a starting character carries, and what SPT's database says of an item as something to carry.
//! The character's from `templates/profiles.json`; an item's class, weight, cells, slots and grids
//! (`docs/spec/inventory.md`, "What the install holds").

use anyhow::{anyhow, Context, Result};
use serde_json::Value as Json;

use crate::setup::database::{Database, ItemTemplate};
use crate::setup::install::Install;

mod locale;
mod templates;

pub use locale::*;
pub use templates::*;

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
    /// Its fold folded, where it has one (`upd.Foldable.Folded`; a
    /// recording's `fold`, which recorder 0.10.2 writes).
    pub folded: bool,
}

/// The kinds of thing a new account's character of that edition and side
/// knows as it starts (`profiles.json`, its `character.Encyclopedia`: the
/// templates it holds, whatever each one's value; `Profile.Examined` asks
/// only whether a template is there).
pub fn starting_encyclopedia(install: &Install, edition: &str, side: &str) -> Result<Vec<String>> {
    let path = install.database_dir().join("templates/profiles.json");
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let profiles: Json = serde_json::from_str(text.trim_start_matches('\u{feff}')).context("profiles.json")?;
    let list = profiles
        .get(edition)
        .and_then(|e| e.get(side))
        .and_then(|s| s.pointer("/character/Encyclopedia"))
        .and_then(Json::as_object)
        .ok_or_else(|| anyhow!("the database's starting character {edition}/{side} has no Encyclopedia"))?;
    Ok(list.keys().cloned().collect())
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
                    folded: entry.pointer("/upd/Foldable/Folded").and_then(Json::as_bool).unwrap_or(false),
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
                folded: whole(entry, "fold") == Some(1),
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
        Ok(Self::of_build(db.default_preset(weapon_template)?))
    }

    /// The same list of any of the database's builds (`ItemPresets`, the
    /// stock one or another).
    pub fn of_build(preset: &crate::setup::database::Preset) -> Self {
        let entry = |item: &crate::setup::database::PresetItem| Carried {
            id: item.id.clone(),
            template: item.template.clone(),
            parent: item.parent.clone().filter(|_| item.id != preset.root),
            slot: item.slot.clone(),
            place: None,
            count: 1,
            folded: false,
        };
        let mut items: Vec<Carried> = preset.items.iter().filter(|item| item.id == preset.root).map(entry).collect();
        let mut at = 0;
        while at < items.len() {
            let parent = items[at].id.clone();
            items.extend(preset.items.iter().filter(|item| item.id != preset.root && item.parent.as_deref() == Some(parent.as_str())).map(entry));
            at += 1;
        }
        Self { equipment: preset.root.clone(), items }
    }
}

impl Loadout {
    /// The same list made to go with a weapon: every magazine the weapon's
    /// magazine slot does not take becomes the magazine of the weapon's
    /// stock preset, and every round its chamber does not take becomes
    /// its default round (`defAmmo`), with no more to a magazine or to a
    /// stack than each holds. For a weapon whose magazine stays in it
    /// (`ReloadMode` `InternalMagazine`: a shotgun's tube; not the Model
    /// 700, whose stock magazine comes out, `changes_magazines`) a spare
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
        // A weapon with no chamber at all (the M60s, the RPD: `Chambers`
        // empty) takes what its own magazine's cartridges take.
        if let Some(magazine) = own_magazine.as_ref().and_then(|magazine| db.items.get(magazine)).filter(|_| takes_round.is_empty()) {
            takes_round = magazine.holders("Cartridges").into_iter().flat_map(|cartridges| cartridges.takes).collect();
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
        let internal = own_magazine.is_none() || !db.changes_magazines(weapon_template);
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
