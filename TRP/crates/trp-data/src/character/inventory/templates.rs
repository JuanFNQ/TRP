//! Item templates' holders, slots, grids and cells.

use super::*;

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
    /// An armour's slot, for a plate or a soft insert (`ArmorSlot`: its
    /// prototype is `ARMOR_SLOT_PROTO`), which a container's row does not
    /// show among its special slots (`SearchableSlotView.CreateSlots`).
    pub armour: bool,
    /// Its first filter says it is locked (`locked`: an armour's built-in
    /// inserts' slots, which nothing is taken out of).
    pub locked: bool,
    /// The plate its filters name (`Plate`, the first that names one):
    /// what the game puts into a locked slot of a thing it makes whole
    /// (`ItemFactory.CreateAndFillItem`; `Item::fill_locked`).
    pub plate: Option<String>,
}

/// The prototype of an armour's slots (`SlotSerializer.Deserialize` makes
/// an `ArmorSlot` of a slot with it).
pub const ARMOR_SLOT_PROTO: &str = "64479fdf9731c8fadc0642c1";

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
    pub(super) fn holders(&self, list: &str) -> Vec<Holder> {
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
                    armour: holder.get("_proto").and_then(Json::as_str).is_some_and(|proto| proto.eq_ignore_ascii_case(ARMOR_SLOT_PROTO)),
                    locked: filters.first().and_then(|filter| filter.get("locked")).and_then(Json::as_bool).unwrap_or(false),
                    plate: filters.iter().find_map(|filter| filter.get("Plate")?.as_str()).map(str::to_owned),
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

    /// What a magazine's rounds go into (`Cartridges`), with what it takes
    /// and refuses: none for anything else.
    pub fn cartridges(&self) -> Vec<Holder> {
        self.holders("Cartridges")
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

    /// The name of its own layout of its grids in the inventory, where it
    /// has one (`RigLayoutName` for a vest, `GridLayoutName` for a
    /// backpack: `UI/Rig Layouts/<name>`); empty where it has none.
    pub fn grid_layout_name(&self) -> &str {
        ["RigLayoutName", "GridLayoutName"].iter().find_map(|key| self.props.get(*key).and_then(Json::as_str).filter(|name| !name.is_empty())).unwrap_or("")
    }

    /// The colour of its view's ground in the inventory (`BackgroundColor`:
    /// `blue`, `violet`, `default` and the like, `TaxonomyColor`).
    pub fn background_color(&self) -> &str {
        self.props.get("BackgroundColor").and_then(Json::as_str).unwrap_or("default")
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

    /// The equipment slots it blocks while worn, by name, in the order of
    /// the game's blocker (`SlotBlockerComponent`): the template's
    /// `BlocksEarpiece`, `BlocksEyewear`, `BlocksFaceCover` and
    /// `BlocksHeadwear`, then `BlocksArmorVest`, by which an armoured rig
    /// keeps body armour off. The game reads the first four of armour and
    /// head things and the last of what is searched; the install sets
    /// each only there (inventory spec, "What a slot accepts", 4).
    pub fn blocked_slots(&self) -> Vec<String> {
        let blocks = |slot: &str| self.props.get(format!("Blocks{slot}")).and_then(Json::as_bool) == Some(true);
        ["Earpiece", "Eyewear", "FaceCover", "Headwear", "ArmorVest"].into_iter().filter(|slot| blocks(slot)).map(str::to_owned).collect()
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
    Ok(words_of(&Locale::read(install, language)?.words(), names))
}
