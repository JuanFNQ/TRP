//! What the character carries and where each thing sits, by the game's
//! rules (`docs/spec/inventory.md`): items with slots and grids, what a
//! slot and a grid accept, where a grid finds room, an item's size in
//! cells and its weight, and the carried weight the controller goes by.
//! No engine types; a host fills it and reads it.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use trp_data::inventory::{Holder, Loadout};
use trp_data::Database;

/// The character's equipment slots, in the game's order (`EquipmentSlot`).
pub const SLOTS: [&str; 15] = [
    "FirstPrimaryWeapon",
    "SecondPrimaryWeapon",
    "Holster",
    "Scabbard",
    "Backpack",
    "SecuredContainer",
    "TacticalVest",
    "ArmorVest",
    "Pockets",
    "Eyewear",
    "FaceCover",
    "Headwear",
    "Earpiece",
    "Dogtag",
    "ArmBand",
];

/// The slots whose item is drawn on the body (`AllVisualSlotNames`).
pub const WORN: [&str; 8] = ["Backpack", "TacticalVest", "ArmorVest", "Eyewear", "FaceCover", "Headwear", "Earpiece", "ArmBand"];

/// The slots that still weigh something with Strength at its elite level.
const ELITE_WEIGHT: [&str; 9] = ["TacticalVest", "Pockets", "Backpack", "SecuredContainer", "Earpiece", "Headwear", "Eyewear", "FaceCover", "ArmorVest"];

/// The template of the equipment item itself ("Default Inventory").
pub const EQUIPMENT_TEMPLATE: &str = "55d7217a4bdc2d86028b456d";

/// A kind of item: what the database says of it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Kind {
    pub template: String,
    /// The database's own name for it (not the game's localised one).
    pub name: String,
    /// Its template and the classes it belongs to, nearest first.
    pub classes: Vec<String>,
    /// The database's names for the same, nearest first.
    pub class_names: Vec<String>,
    /// In kilograms, for one.
    pub weight: f32,
    /// Width and height in cells.
    pub cells: [u32; 2],
    /// The cells it adds to what it is a part of: left, right, up, down.
    pub extra: [u32; 4],
    /// Whether its parts' extra cells count toward its size.
    pub merges: bool,
    pub slots: Vec<Holder>,
    pub grids: Vec<Holder>,
    /// The equipment slots it blocks while worn.
    pub blocks: Vec<String>,
    /// How many of it one stack holds; 0 or 1 for what does not stack.
    pub stack_max: u32,
    /// A part that may not be put on or taken off in a raid.
    pub not_raid_moddable: bool,
    /// The equipment slots it is not taken out of in a raid.
    pub kept_in: Vec<String>,
}

impl Kind {
    pub fn of(db: &Database, template: &str) -> Result<Self> {
        let item = db.items.get(template).ok_or_else(|| anyhow!("the database has no item {template}"))?;
        Ok(Self {
            template: item.id.clone(),
            name: item.name.clone(),
            classes: db.classes(template),
            class_names: db.class_names(template),
            weight: item.prop_f32("Weight").unwrap_or(0.0),
            cells: item.cells(),
            extra: item.extra_cells(),
            merges: item.merges_with_parts(),
            slots: item.slots(),
            grids: item.grids(),
            blocks: item.blocked_slots(),
            stack_max: item.stack_max(),
            not_raid_moddable: !item.raid_moddable(),
            kept_in: item.kept_in_slots(),
        })
    }

    /// Whether a filter's list names this kind or a class of it.
    fn among(&self, list: &[String]) -> bool {
        list.iter().any(|id| self.classes.contains(id))
    }
}

/// Where an item lies in a grid: the column and row of its top left
/// cell, and whether it lies turned a quarter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    pub x: u32,
    pub y: u32,
    pub turned: bool,
}

/// Why a slot or a grid does not take an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// There is no such slot or grid.
    NoSuchPlace,
    Taken,
    /// The filter does not take this kind of item.
    Filter,
    /// Something worn blocks the slot: the slot that item is in.
    Blocked(String),
    /// The item would block a slot that has something in it.
    WouldBlock(String),
    /// A slot takes a single item, not a stack.
    Stack,
    OutsideGrid,
    NoRoom,
    /// It is there already.
    AlreadyThere,
    /// It would go into itself.
    Cycle,
    /// Backpacks would lie in one another deeper than the game allows.
    TooDeep,
    /// The slot holds a part the item cannot do without, which stays
    /// where it is in a raid: the slot.
    Vital(String),
    /// A part that may not be changed in a raid.
    NotInRaid,
    /// A weapon that lacks a vital part does not go onto the character.
    MissingParts,
    /// The dogtag's slot takes nothing by hand.
    Dogtag,
    /// It is not taken out of that equipment slot in a raid: the slot.
    KeptOn(String),
    /// Nothing can be done with the one on the other.
    NoAction,
}

/// One item, with everything in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// A number of its own, by which it is found wherever it lies.
    pub id: Id,
    pub kind: Arc<Kind>,
    /// How many are in the stack.
    pub count: u32,
    /// What is in each slot, in the kind's order.
    pub slots: Vec<Option<Item>>,
    /// What lies in each grid, in the kind's order.
    pub grids: Vec<Vec<(Place, Item)>>,
    /// What it holds outside slots and grids: a magazine's rounds.
    pub loose: Vec<Item>,
}

impl Item {
    pub fn new(kind: Arc<Kind>) -> Self {
        let (slots, grids) = (vec![None; kind.slots.len()], vec![Vec::new(); kind.grids.len()]);
        Self { id: next_id(), kind, count: 1, slots, grids, loose: Vec::new() }
    }

    /// Everything directly in it.
    pub fn contents(&self) -> impl Iterator<Item = &Item> {
        let in_grids = self.grids.iter().flatten().map(|(_, item)| item);
        self.slots.iter().flatten().chain(in_grids).chain(&self.loose)
    }

    /// Its weight with everything in it, in kilograms: its own and its
    /// contents', for each of the stack.
    pub fn weight(&self) -> f32 {
        (self.kind.weight + self.contents().map(Item::weight).sum::<f32>()) * self.count.max(1) as f32
    }

    /// The extra cells its parts give it, largest on each side.
    fn extra(&self) -> [u32; 4] {
        let mut most = [0; 4];
        for (part, slot) in self.slots.iter().zip(&self.kind.slots) {
            // A slot counts if it says so itself, or else if its item
            // does; the round in the chamber never does.
            let counts = !is_chamber(&slot.name) && (slot.merges == Some(true) || self.kind.merges);
            let Some(part) = part.as_ref().filter(|_| counts) else { continue };
            let (own, deeper) = (part.kind.extra, part.extra());
            most = std::array::from_fn(|side| most[side].max(own[side]).max(deeper[side]));
        }
        most
    }

    /// Width and height in cells as it lies, with its parts on.
    pub fn cells(&self) -> [u32; 2] {
        let [left, right, up, down] = self.extra();
        [self.kind.cells[0] + left + right, self.kind.cells[1] + up + down]
    }

    fn cells_at(&self, place: Place) -> [u32; 2] {
        let [width, height] = self.cells();
        if place.turned { [height, width] } else { [width, height] }
    }

    pub fn slot_index(&self, name: &str) -> Option<usize> {
        self.kind.slots.iter().position(|slot| slot.name == name)
    }

    pub fn grid_index(&self, name: &str) -> Option<usize> {
        self.kind.grids.iter().position(|grid| grid.name == name)
    }

    /// What is in the slot of that name.
    pub fn in_slot(&self, name: &str) -> Option<&Item> {
        self.slots.get(self.slot_index(name)?)?.as_ref()
    }

    /// Whether the slot takes the item, by the slot's own rules: it is
    /// empty, its filter passes, and the item is a single one.
    pub fn slot_takes(&self, slot: usize, item: &Item) -> Result<(), Refusal> {
        let holder = self.kind.slots.get(slot).ok_or(Refusal::NoSuchPlace)?;
        if self.slots[slot].is_some() {
            return Err(Refusal::Taken);
        }
        if !item.kind.among(&holder.takes) {
            return Err(Refusal::Filter);
        }
        if item.count != 1 {
            return Err(Refusal::Stack);
        }
        Ok(())
    }

    /// Whether the grid takes the item at that place: inside the grid,
    /// on no taken cell, and through the grid's filter.
    pub fn grid_takes(&self, grid: usize, item: &Item, place: Place) -> Result<(), Refusal> {
        self.grid_room(grid, item, place, true)
    }

    /// The same, saying whether the cells the item lies on now are free to
    /// it. They are to a move, which takes it out first; they are not to
    /// the game's search for a free place (`Grid.FindFreeSpace`), which
    /// goes by the grid as it stands.
    fn grid_room(&self, grid: usize, item: &Item, place: Place, own_free: bool) -> Result<(), Refusal> {
        let holder = self.kind.grids.get(grid).ok_or(Refusal::NoSuchPlace)?;
        if (!holder.takes.is_empty() && !item.kind.among(&holder.takes)) || item.kind.among(&holder.refuses) {
            return Err(Refusal::Filter);
        }
        let [width, height] = item.cells_at(place);
        if place.x + width > holder.cells[0] || place.y + height > holder.cells[1] {
            return Err(Refusal::OutsideGrid);
        }
        let overlaps = |(at, other): &(Place, Item)| {
            let [w, h] = other.cells_at(*at);
            // Its own cells are no hindrance to it: a move takes it out first.
            (other.id != item.id || !own_free) && place.x < at.x + w && at.x < place.x + width && place.y < at.y + h && at.y < place.y + height
        };
        if self.grids[grid].iter().any(overlaps) {
            return Err(Refusal::Taken);
        }
        Ok(())
    }

    /// Where the grid has room for the item: rows from the top and cells
    /// from the left, as it lies first and, if it fits nowhere so and is
    /// not square, turned.
    pub fn find_room(&self, grid: usize, item: &Item) -> Option<Place> {
        self.room(grid, item, true)
    }

    /// The same as the game's own search has it for a thing that may lie
    /// in this grid already: the cells it lies on are taken, so the place
    /// found is never the one it has.
    pub fn find_other_room(&self, grid: usize, item: &Item) -> Option<Place> {
        self.room(grid, item, false)
    }

    fn room(&self, grid: usize, item: &Item, own_free: bool) -> Option<Place> {
        let holder = self.kind.grids.get(grid)?;
        let [width, height] = item.cells();
        let turns: &[bool] = if width == height { &[false] } else { &[false, true] };
        turns.iter().find_map(|turned| {
            let places = (0..holder.cells[1]).flat_map(|y| (0..holder.cells[0]).map(move |x| Place { x, y, turned: *turned }));
            places.into_iter().find(|place| self.grid_room(grid, item, *place, own_free).is_ok())
        })
    }

    /// Puts the item into the grid at that place.
    pub fn put(&mut self, grid: usize, item: Item, place: Place) -> Result<(), Refusal> {
        self.grid_takes(grid, &item, place)?;
        self.grids[grid].push((place, item));
        Ok(())
    }

    /// Puts the item into the first of its grids that has room.
    pub fn stow(&mut self, item: Item) -> Result<(usize, Place), Refusal> {
        let room = (0..self.grids.len()).find_map(|grid| self.find_room(grid, &item).map(|place| (grid, place)));
        let (grid, place) = room.ok_or(Refusal::NoRoom)?;
        self.put(grid, item, place).map(|()| (grid, place))
    }
}

/// The character's equipment: one item with the fifteen slots.
#[derive(Debug, Clone, PartialEq)]
pub struct Equipment {
    pub item: Item,
}

impl Equipment {
    /// Nothing worn.
    pub fn empty(db: &Database) -> Result<Self> {
        Ok(Self { item: Item::new(Arc::new(Kind::of(db, EQUIPMENT_TEMPLATE)?)) })
    }

    /// What is in the equipment slot of that name.
    pub fn worn(&self, slot: &str) -> Option<&Item> {
        self.item.in_slot(slot)
    }

    /// Whether the slot takes the item: the slot's own rules, nothing
    /// worn blocking that slot, and the item blocking no slot in use.
    pub fn can_wear(&self, slot: &str, item: &Item) -> Result<(), Refusal> {
        let index = self.item.slot_index(slot).ok_or(Refusal::NoSuchPlace)?;
        self.item.slot_takes(index, item)?;
        self.item.blocking(index, item)
    }

    pub fn wear(&mut self, slot: &str, item: Item) -> Result<(), Refusal> {
        self.can_wear(slot, &item)?;
        let index = self.item.slot_index(slot).ok_or(Refusal::NoSuchPlace)?;
        self.item.slots[index] = Some(item);
        Ok(())
    }

    /// Takes what is in the slot off.
    pub fn take_off(&mut self, slot: &str) -> Option<Item> {
        let index = self.item.slot_index(slot)?;
        self.item.slots[index].take()
    }

    /// The carried weight the controller goes by, in kilograms: what is
    /// in every slot, or, with Strength at its elite level, in nine of
    /// them (the weapons, the scabbard, the dogtag and the armband then
    /// weigh nothing).
    pub fn carried_weight(&self, elite_strength: bool) -> f32 {
        let counted = SLOTS.into_iter().filter(|slot| !elite_strength || ELITE_WEIGHT.contains(slot));
        // (An empty sum is minus zero.)
        counted.filter_map(|slot| self.worn(slot)).map(Item::weight).sum::<f32>() + 0.0
    }

    /// The templates of what is worn where it is drawn on the body.
    pub fn drawn(&self) -> Vec<&str> {
        WORN.iter().filter_map(|slot| self.worn(slot)).map(|item| item.kind.template.as_str()).collect()
    }

    /// A starting character's equipment as the database has it, put
    /// together as that list says, with no rule asked: it is the game's
    /// own arrangement. An entry whose slot or grid its parent does not
    /// have is left out and named in the second value.
    pub fn of_loadout(db: &Database, loadout: &Loadout) -> Result<(Self, Vec<String>)> {
        Item::of_list(db, loadout).map(|(item, left_out)| (Self { item }, left_out))
    }
}

impl Item {
    /// An item with everything in it, put together as a list of the
    /// database says (a starting character's equipment, a weapon's
    /// preset), with no rule asked: it is the game's own arrangement. An
    /// entry whose slot or grid its parent does not have is left out and
    /// named in the second value.
    pub fn of_list(db: &Database, loadout: &Loadout) -> Result<(Self, Vec<String>)> {
        let mut kinds: std::collections::HashMap<String, Arc<Kind>> = std::collections::HashMap::new();
        let mut kind = |template: &str| -> Result<Arc<Kind>> {
            if let Some(kind) = kinds.get(template) {
                return Ok(kind.clone());
            }
            let kind = Arc::new(Kind::of(db, template)?);
            kinds.insert(template.to_owned(), kind.clone());
            Ok(kind)
        };
        // Built from the leaves up: the list has parents before children.
        let mut built: std::collections::HashMap<&str, Item> = std::collections::HashMap::new();
        let mut left_out = Vec::new();
        for entry in loadout.items.iter().rev() {
            let mut item = built.remove(entry.id.as_str()).unwrap_or(Item::new(kind(&entry.template)?));
            item.count = entry.count.max(1);
            let Some(parent) = entry.parent.as_deref().filter(|_| entry.id != loadout.equipment) else {
                built.insert(entry.id.as_str(), item);
                continue;
            };
            let parent_entry = loadout.items.iter().find(|other| other.id == parent).ok_or_else(|| anyhow!("{} is in {parent}, which is not listed", entry.id))?;
            let holder = match built.remove(parent_entry.id.as_str()) {
                Some(holder) => holder,
                None => Item::new(kind(&parent_entry.template)?),
            };
            let mut holder = holder;
            let name = entry.slot.as_deref().unwrap_or("");
            match (holder.slot_index(name), holder.grid_index(name), entry.place) {
                (Some(slot), ..) => holder.slots[slot] = Some(item),
                (None, Some(grid), Some((x, y, turned))) => holder.grids[grid].push((Place { x, y, turned }, item)),
                (None, None, None) if name == "cartridges" => holder.loose.push(item),
                _ => left_out.push(format!("{} ({}) in {name} of {}", item.kind.name, entry.id, holder.kind.name)),
            }
            built.insert(parent_entry.id.as_str(), holder);
        }
        let item = built.remove(loadout.equipment.as_str()).ok_or_else(|| anyhow!("the list has no item {}", loadout.equipment))?;
        Ok((item, left_out))
    }
}

/// One carried item as a recording has it beside what the model makes of
/// the same item: its cells and its weight, the game's first.
#[derive(Debug, Clone, PartialEq)]
pub struct Compared {
    pub name: String,
    pub cells: [[u32; 2]; 2],
    pub weight: [f32; 2],
}

impl Compared {
    /// Whether the two agree: the cells exactly, the weight to a gram.
    pub fn agrees(&self) -> bool {
        self.cells[0] == self.cells[1] && (self.weight[0] - self.weight[1]).abs() < 0.001
    }
}

impl Equipment {
    /// The equipment a recording's header lists (recorder 0.5.0 on), put
    /// together as the list says, and each of its items as the game had
    /// it beside the model's own figure for it. The third value names the
    /// entries that found no place.
    pub fn of_recording(db: &Database, list: &serde_json::Value) -> Result<(Self, Vec<Compared>, Vec<String>)> {
        let (loadout, recorded) = Loadout::of_recording(list)?;
        let (item, left_out) = Item::of_list(db, &loadout)?;
        let mut compared = Vec::new();
        for (index, said) in recorded.iter().enumerate().skip(1) {
            let Some(found) = item.at_entry(&loadout, index) else { continue };
            compared.push(Compared { name: found.kind.name.clone(), cells: [said.cells, found.cells()], weight: [said.weight, found.weight()] });
        }
        Ok((Self { item }, compared, left_out))
    }
}

impl Item {
    /// A thing with everything in it as a recording lists it: what the
    /// character carries, or what a container holds (recorder 0.6.2 writes
    /// both whenever either changes). With it, each listed entry's own id
    /// beside the number the model gives the same thing, and the entries
    /// that found no place.
    pub fn of_recording(db: &Database, list: &serde_json::Value) -> Result<(Self, Vec<(String, Id)>, Vec<String>)> {
        let (mut loadout, _) = Loadout::of_recording(list)?;
        // The thing itself is the entry that lies in nothing.
        if let Some(root) = loadout.items.iter().find(|entry| entry.parent.is_none() && entry.slot.is_none()) {
            loadout.equipment = root.id.clone();
        }
        let (item, left_out) = Item::of_list(db, &loadout)?;
        let ids = (0..loadout.items.len()).filter_map(|index| item.at_entry(&loadout, index).map(|found| (loadout.items[index].id.clone(), found.id))).collect();
        Ok((item, ids, left_out))
    }

    /// The item an entry of the list this one was put together from
    /// stands for: found from the top down by the slot, the cell or, for
    /// what lies loose, the kind and the count each entry names.
    fn at_entry(&self, list: &Loadout, index: usize) -> Option<&Item> {
        let entry = list.items.get(index)?;
        if entry.id == list.equipment {
            return Some(self);
        }
        let above = list.items.iter().position(|other| Some(&other.id) == entry.parent.as_ref())?;
        let holder = self.at_entry(list, above)?;
        let name = entry.slot.as_deref().unwrap_or("");
        match (holder.slot_index(name), holder.grid_index(name), entry.place) {
            (Some(slot), ..) => holder.slots[slot].as_ref(),
            (None, Some(grid), Some((x, y, turned))) => holder.grids[grid].iter().find(|(place, _)| *place == Place { x, y, turned }).map(|(_, item)| item),
            _ => holder.loose.iter().find(|item| item.kind.template == entry.template && item.count == entry.count.max(1)),
        }
    }
}

/// What stands for an item wherever it lies: a number of its own.
pub type Id = u64;

fn next_id() -> Id {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// The name of a weapon's slot for a chambered round; a weapon with more
/// barrels than one numbers them after it (`patron_in_weapon_000`).
const CHAMBER: &str = "patron_in_weapon";
/// What the name of each chamber of a cylinder begins with: slots of the
/// cylinder, which is the weapon's magazine (`CylinderMagazine.Camoras`).
const CAMORA: &str = "camora_";

fn is_chamber(slot: &str) -> bool {
    slot.starts_with(CHAMBER)
}

/// What a weapon's loose rounds are tested against when its reload key
/// looks for them (`FirearmHandsInputTranslator`): its chamber
/// (`ReloadWithAmmo`), the first of its barrels (`ReloadBarrels`), or for
/// a revolver, which has no chamber of the weapon's own, its cylinder's
/// (`ReloadRevolverDrum`, by `CylinderMagazine.CheckCompatibility`).
fn takes_rounds(weapon: &Item) -> Option<Vec<String>> {
    if let Some(chamber) = weapon.kind.slots.iter().find(|slot| is_chamber(&slot.name)) {
        return Some(chamber.takes.clone());
    }
    let cylinder = weapon.in_slot("mod_magazine")?;
    cylinder.kind.slots.iter().find(|slot| slot.name.starts_with(CAMORA)).map(|camora| camora.takes.clone())
}

/// The class of backpacks, which go into one another only so deep.
const BACKPACK: &str = "5448e53e4bdc2d60728b4567";

/// A place in an item for another: one of its slots, a cell of one of its
/// grids (and which way the item lies there), or loose in it, as a
/// magazine's rounds are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spot {
    Slot(usize),
    Cell(usize, Place),
    Loose,
}

/// What the moving of things goes by besides the places' own rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rules {
    /// In a raid a weapon's vital parts stay where they are, and so do
    /// the parts the database says are not to be changed there.
    pub in_raid: bool,
    /// How many backpacks deep one may lie in another
    /// (`MaxBackpackInserting` in the game's settings); 0 for no limit.
    pub backpacks_deep: u32,
}

impl Rules {
    /// The rules of a raid, with the game's own limit from the database.
    pub fn of(db: &Database) -> Self {
        fn named(json: &serde_json::Value, name: &str) -> Option<u64> {
            match json {
                serde_json::Value::Object(map) => map.get(name).and_then(|v| v.as_u64()).or_else(|| map.values().find_map(|v| named(v, name))),
                serde_json::Value::Array(list) => list.iter().find_map(|v| named(v, name)),
                _ => None,
            }
        }
        Self { in_raid: true, backpacks_deep: named(&db.config, "MaxBackpackInserting").unwrap_or(0) as u32 }
    }
}

/// What dropping one item on another came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    /// The whole stack went into the other.
    Merged,
    /// It went into the other: where.
    Put(Spot),
    /// So many of the stack went into the other, which is now full.
    Topped(u32),
}

impl Item {
    /// The item of that number: this one or anything in it.
    pub fn find(&self, id: Id) -> Option<&Item> {
        if self.id == id {
            return Some(self);
        }
        self.contents().find_map(|inside| inside.find(id))
    }

    fn inside_mut(&mut self) -> impl Iterator<Item = &mut Item> {
        let in_grids = self.grids.iter_mut().flatten().map(|(_, item)| item);
        self.slots.iter_mut().flatten().chain(in_grids).chain(self.loose.iter_mut())
    }

    fn find_mut(&mut self, id: Id) -> Option<&mut Item> {
        if self.id == id {
            return Some(self);
        }
        self.inside_mut().find_map(|inside| inside.find_mut(id))
    }

    /// The item that holds the one of that number, and where in it.
    pub fn holder_of(&self, id: Id) -> Option<(Id, Spot)> {
        let is = |item: &Item| item.id == id;
        let slot = || self.slots.iter().position(|held| held.as_ref().is_some_and(is)).map(Spot::Slot);
        let cell = || self.grids.iter().enumerate().find_map(|(grid, held)| held.iter().find(|(_, item)| is(item)).map(|(place, _)| Spot::Cell(grid, *place)));
        let loose = || self.loose.iter().any(is).then_some(Spot::Loose);
        match slot().or_else(cell).or_else(loose) {
            Some(spot) => Some((self.id, spot)),
            None => self.contents().find_map(|inside| inside.holder_of(id)),
        }
    }

    /// The items from this one down to the one of that number, both
    /// included; none if it is not in here.
    fn chain(&self, id: Id) -> Vec<&Item> {
        if self.id == id {
            return vec![self];
        }
        for inside in self.contents() {
            let mut below = inside.chain(id);
            if !below.is_empty() {
                below.insert(0, self);
                return below;
            }
        }
        Vec::new()
    }

    /// Takes the item of that number out, from however deep.
    fn take(&mut self, id: Id) -> Option<Item> {
        if let Some(held) = self.slots.iter_mut().find(|held| held.as_ref().is_some_and(|item| item.id == id)) {
            return held.take();
        }
        for grid in &mut self.grids {
            if let Some(at) = grid.iter().position(|(_, item)| item.id == id) {
                return Some(grid.remove(at).1);
            }
        }
        if let Some(at) = self.loose.iter().position(|item| item.id == id) {
            return Some(self.loose.remove(at));
        }
        self.inside_mut().find_map(|inside| inside.take(id))
    }

    /// Puts an item where the spot says, no rule asked.
    fn set(&mut self, spot: Spot, item: Item) {
        match spot {
            Spot::Slot(slot) => self.slots[slot] = Some(item),
            Spot::Cell(grid, place) => self.grids[grid].push((place, item)),
            Spot::Loose => self.loose.push(item),
        }
    }

    /// For the character's equipment: nothing worn blocks that slot, and
    /// the item would block no slot that has something in it.
    fn blocking(&self, slot: usize, item: &Item) -> Result<(), Refusal> {
        let name = &self.kind.slots[slot].name;
        for (other, worn) in self.kind.slots.iter().zip(&self.slots) {
            if worn.as_ref().is_some_and(|worn| worn.id != item.id && worn.kind.blocks.iter().any(|blocked| blocked == name)) {
                return Err(Refusal::Blocked(other.name.clone()));
            }
        }
        match item.kind.blocks.iter().find(|blocked| self.in_slot(blocked).is_some()) {
            Some(blocked) => Err(Refusal::WouldBlock(blocked.clone())),
            None => Ok(()),
        }
    }

    fn is_backpack(&self) -> bool {
        self.kind.classes.iter().any(|class| class == BACKPACK)
    }

    /// How many backpacks deep the backpacks in this one lie.
    fn backpacks_within(&self) -> u32 {
        self.grids.iter().flatten().filter(|(_, item)| item.is_backpack()).map(|(_, item)| 1 + item.backpacks_within()).max().unwrap_or(0)
    }

    /// Whether a part it cannot do without is missing, on it or on one
    /// of its parts.
    pub fn missing_vital(&self) -> bool {
        self.kind.slots.iter().zip(&self.slots).any(|(slot, part)| match part {
            None => slot.required,
            Some(part) => part.missing_vital(),
        })
    }

    /// How many more of its kind the stack takes.
    fn stack_room(&self) -> u32 {
        self.kind.stack_max.saturating_sub(self.count)
    }
}

fn root_of(roots: &[&mut Item], id: Id) -> Result<usize, Refusal> {
    roots.iter().position(|root| root.find(id).is_some()).ok_or(Refusal::NoSuchPlace)
}

/// Moves an item to a spot of another (`ItemManipulator.Move`): out of
/// where it is and in there, by the rules of the place it goes to. The
/// roots are the things that stand by themselves: the character's
/// equipment, whatever is open beside it. Refused, nothing has changed.
pub fn move_item(roots: &mut [&mut Item], id: Id, into: Id, spot: Spot, rules: &Rules) -> Result<(), Refusal> {
    let (from, to) = (root_of(roots, id)?, root_of(roots, into)?);
    // Something that stands by itself is not moved.
    let (holder, was) = roots[from].holder_of(id).ok_or(Refusal::NoSuchPlace)?;
    if holder == into && was == spot {
        return Err(Refusal::AlreadyThere);
    }
    let (item, target) = (roots[from].find(id).ok_or(Refusal::NoSuchPlace)?, roots[to].find(into).ok_or(Refusal::NoSuchPlace)?);
    let slot_named = |of: &Item, spot: Spot| match spot {
        Spot::Slot(slot) => of.kind.slots.get(slot).map(|slot| (slot.name.clone(), slot.required)),
        _ => None,
    };
    let (leaves, enters) = (roots[from].find(holder).and_then(|holder| slot_named(holder, was)), slot_named(target, spot));
    if matches!(spot, Spot::Slot(_)) && enters.is_none() {
        return Err(Refusal::NoSuchPlace);
    }
    if enters.as_ref().is_some_and(|(name, _)| name == "Dogtag") {
        return Err(Refusal::Dogtag);
    }
    if rules.in_raid {
        if item.kind.not_raid_moddable && (leaves.is_some() || enters.is_some()) {
            return Err(Refusal::NotInRaid);
        }
        if let Some((name, _)) = enters.iter().chain(&leaves).find(|(_, required)| *required) {
            return Err(Refusal::Vital(name.clone()));
        }
    }
    // What names its own equipment slot stays in it through a raid.
    if let Some((name, _)) = leaves.as_ref().filter(|(name, _)| rules.in_raid && roots[from].id == holder && roots[from].kind.template == EQUIPMENT_TEMPLATE && item.kind.kept_in.contains(name)) {
        return Err(Refusal::KeptOn(name.clone()));
    }
    if item.find(into).is_some() {
        return Err(Refusal::Cycle);
    }
    if rules.backpacks_deep > 0 && item.is_backpack() {
        let above = roots[to].chain(into).iter().filter(|holder| holder.is_backpack()).count() as u32;
        if above + 1 + item.backpacks_within() > rules.backpacks_deep {
            return Err(Refusal::TooDeep);
        }
    }
    // Onto the character, a weapon goes whole.
    let worn = roots[to].kind.template == EQUIPMENT_TEMPLATE;
    if worn && enters.is_some() && item.missing_vital() {
        return Err(Refusal::MissingParts);
    }
    // Whether the place takes it, and only then out and in.
    let fits = match spot {
        Spot::Slot(slot) => {
            let blocked = if target.kind.template == EQUIPMENT_TEMPLATE { target.blocking(slot, item) } else { Ok(()) };
            target.slot_takes(slot, item).and(blocked)
        }
        Spot::Cell(grid, place) => target.grid_takes(grid, item, place),
        Spot::Loose => Err(Refusal::NoSuchPlace),
    };
    fits?;
    let item = roots[from].take(id).ok_or(Refusal::NoSuchPlace)?;
    roots[to].find_mut(into).expect("the place was just looked at").set(spot, item);
    Ok(())
}

/// Whether an item may leave where it is, by the rules of a raid: a part
/// that is not changed there and a vital part stay in their slots, and so
/// does what names its own equipment slot.
fn may_leave(roots: &[&mut Item], from: usize, id: Id, rules: &Rules) -> Result<(), Refusal> {
    let (holder, was) = roots[from].holder_of(id).ok_or(Refusal::NoSuchPlace)?;
    let (item, held_by) = (roots[from].find(id).ok_or(Refusal::NoSuchPlace)?, roots[from].find(holder).ok_or(Refusal::NoSuchPlace)?);
    let Spot::Slot(slot) = was else { return Ok(()) };
    let Some(slot) = held_by.kind.slots.get(slot).filter(|_| rules.in_raid) else { return Ok(()) };
    if item.kind.not_raid_moddable {
        return Err(Refusal::NotInRaid);
    }
    if slot.required {
        return Err(Refusal::Vital(slot.name.clone()));
    }
    if roots[from].id == holder && held_by.kind.template == EQUIPMENT_TEMPLATE && item.kind.kept_in.contains(&slot.name) {
        return Err(Refusal::KeptOn(slot.name.clone()));
    }
    Ok(())
}

/// Takes an item out of where it is, to be put down in the world
/// (`ItemManipulator.Discard`, as far as the rules a thing leaves its
/// place by). Refused, nothing has changed.
pub fn take_out(roots: &mut [&mut Item], id: Id, rules: &Rules) -> Result<Item, Refusal> {
    let from = root_of(roots, id)?;
    may_leave(roots, from, id, rules)?;
    roots[from].take(id).ok_or(Refusal::NoSuchPlace)
}

/// Splits so many off a stack into a place of its own
/// (`ItemManipulator.SplitMax`): a new stack of that many there, by the
/// place's own rules, and the old one that many fewer. The new stack's
/// number is returned.
pub fn split_item(roots: &mut [&mut Item], id: Id, count: u32, into: Id, spot: Spot, rules: &Rules) -> Result<Id, Refusal> {
    let (from, to) = (root_of(roots, id)?, root_of(roots, into)?);
    let (item, target) = (roots[from].find(id).ok_or(Refusal::NoSuchPlace)?, roots[to].find(into).ok_or(Refusal::NoSuchPlace)?);
    if item.kind.stack_max <= 1 || count == 0 || count >= item.count {
        return Err(Refusal::NoAction);
    }
    let _ = rules;
    let part = Item { id: next_id(), count, ..item.clone() };
    match spot {
        Spot::Slot(slot) => target.slot_takes(slot, &part)?,
        Spot::Cell(grid, place) => target.grid_takes(grid, &part, place)?,
        Spot::Loose => return Err(Refusal::NoSuchPlace),
    }
    let made = part.id;
    roots[from].find_mut(id).ok_or(Refusal::NoSuchPlace)?.count -= count;
    roots[to].find_mut(into).ok_or(Refusal::NoSuchPlace)?.set(spot, part);
    Ok(made)
}

/// Whether an item is of one of the game's classes, by the database's
/// name for it (`Weapon`, `Magazine`, `Ammo` and so on: the game's own
/// item types go by the same names).
pub fn is(item: &Item, class: &str) -> bool {
    item.kind.class_names.iter().any(|name| name == class)
}

/// The four things worn that a thing is looked for room in, in the game's
/// order for that kind of thing (`GetPrioritizedContainersForLoot`):
/// rounds and magazines go to the vest first, money to the secured
/// container, grenades to the pockets, everything else to the backpack.
pub fn loot_order(item: &Item) -> [&'static str; 4] {
    if is(item, "Ammo") || is(item, "Magazine") {
        ["TacticalVest", "Pockets", "Backpack", "SecuredContainer"]
    } else if is(item, "Money") {
        ["SecuredContainer", "Backpack", "TacticalVest", "Pockets"]
    } else if is(item, "ThrowWeap") {
        ["Pockets", "TacticalVest", "Backpack", "SecuredContainer"]
    } else {
        ["Backpack", "TacticalVest", "Pockets", "SecuredContainer"]
    }
}

/// The equipment slots a thing is put on in, in the game's order for its
/// kind (`InventoryExtension.FindSlotToPickUp`, which asks the classes in
/// this order); none for what is neither worn nor held. Not told apart:
/// a flare gun, which the game sends to the holster, and what goes only
/// into the pockets' special slots.
fn slots_to_wear(item: &Item) -> &'static [&'static str] {
    const BY_CLASS: [(&str, &[&str]); 13] = [
        ("Armor", &["ArmorVest"]),
        ("Visors", &["Eyewear"]),
        ("Headwear", &["Headwear"]),
        ("FaceCover", &["FaceCover"]),
        ("Headphones", &["Earpiece"]),
        ("Backpack", &["Backpack"]),
        ("Vest", &["TacticalVest"]),
        ("Knife", &["Scabbard"]),
        ("ArmBand", &["ArmBand"]),
        ("Revolver", &["Holster", "FirstPrimaryWeapon", "SecondPrimaryWeapon"]),
        ("Pistol", &["Holster"]),
        ("Weapon", &["FirstPrimaryWeapon", "SecondPrimaryWeapon", "Holster"]),
        ("MobContainer", &["SecuredContainer"]),
    ];
    BY_CLASS.iter().find(|(class, _)| is(item, class)).map_or(&[], |(_, slots)| *slots)
}

/// The equipment slot a thing would be put on in: the first of its
/// kind's that takes it (`FindSlotToPickUp`, `Slot.CheckConditions`).
pub fn slot_to_wear(equipment: &Item, item: &Item) -> Option<usize> {
    let takes = |slot: &usize| equipment.slot_takes(*slot, item).and_then(|()| equipment.blocking(*slot, item)).is_ok();
    let slots = slots_to_wear(item).iter().filter_map(|name| equipment.slot_index(name));
    slots.into_iter().find(takes).filter(|_| !item.missing_vital())
}

/// How the game looks for a place for a thing nobody chose one for
/// (`ItemManipulator.EMoveItemOrder`; both orders here merge or top up a
/// stack of the kind first).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Order {
    /// A slot of the equipment it is worn or held in comes first.
    pub wear: bool,
    /// What it lies in now is looked in too, after the rest.
    pub own_holder: bool,
    /// Each thing to look in is tried by itself, in order; else all of
    /// them are taken as one.
    pub one_by_one: bool,
}

impl Order {
    /// A thing sent to the other side with Ctrl and a click
    /// (`MoveToAnotherSide`).
    pub const OTHER_SIDE: Self = Self { wear: false, own_holder: true, one_by_one: true };
    /// A thing taken up off the ground (`PickUp`).
    pub const PICK_UP: Self = Self { wear: true, own_holder: false, one_by_one: false };
}

/// The things Ctrl and a click looks in for a place for an item, in order
/// (`ItemUiContext.QuickFindAppropriatePlace`): the container whose window
/// is the priority one, unless the item lies in it, and nothing else; else
/// what is open beside the equipment, unless the item lies in that, with
/// the equipment after it for a thing the character has; else the
/// equipment. (`quick_find` adds what the thing lies in now, last.)
pub fn other_side(equipment: &Item, open: Option<&Item>, priority: Option<Id>, id: Id) -> Vec<Id> {
    let find = |holder: Id| equipment.find(holder).or_else(|| open.and_then(|open| open.find(holder)));
    if let Some(priority) = priority.filter(|priority| find(*priority).is_some_and(|window| window.find(id).is_none())) {
        return vec![priority];
    }
    match (open.filter(|open| open.find(id).is_none()), equipment.find(id).is_some()) {
        (Some(open), true) => vec![open.id, equipment.id],
        (Some(open), false) => vec![open.id],
        (None, _) => vec![equipment.id],
    }
}

/// What holds an item, counting a weapon and its parts as one thing
/// (`GetNotMergedParent`): from a part, up to what holds the whole.
fn unmerged_holder(root: &Item, id: Id) -> Option<Id> {
    let (holder, spot) = root.holder_of(id)?;
    let held_by = root.find(holder)?;
    let merged = match spot {
        Spot::Slot(slot) => held_by.kind.slots.get(slot).is_some_and(|slot| !is_chamber(&slot.name) && (slot.merges == Some(true) || held_by.kind.merges)),
        _ => false,
    };
    if merged { unmerged_holder(root, holder) } else { Some(holder) }
}

/// The first place a holder has for an item, its slots before its grids
/// (`CompoundItem.Containers`, `TryFindLocationForItem`): a slot that
/// takes it, or the first room in a grid as the grid stands.
fn first_place(holder: &Item, item: &Item) -> Option<Spot> {
    let slot = (0..holder.kind.slots.len()).find(|slot| !is_chamber(&holder.kind.slots[*slot].name) && holder.slot_takes(*slot, item).is_ok());
    let cell = || (0..holder.kind.grids.len()).find_map(|grid| holder.find_other_room(grid, item).map(|place| Spot::Cell(grid, place)));
    slot.map(Spot::Slot).or_else(cell)
}

/// One go at a set of holders (the game's inner step): a stack of the
/// item's kind among what lies directly in them, with room for one at
/// least, takes the stack or as much of it as it has room for; else the
/// item goes to the first place any of them has. A move refused there
/// ends the go.
fn put_among(roots: &mut [&mut Item], id: Id, holders: &[Id], rules: &Rules) -> Result<(Id, Applied), Refusal> {
    let (alike, place, mut refused) = {
        let item = roots.iter().find_map(|root| root.find(id)).ok_or(Refusal::NoSuchPlace)?;
        let found: Vec<&Item> = holders.iter().filter_map(|holder| roots.iter().find_map(|root| root.find(*holder))).collect();
        let stacks = found.iter().flat_map(|holder| holder.slots.iter().flatten().chain(holder.grids.iter().flatten().map(|(_, inside)| inside)));
        let alike = |other: &&Item| other.id != id && other.kind.template == item.kind.template && other.stack_room() >= 1;
        let alike = stacks.into_iter().find(alike).filter(|_| item.kind.stack_max > 1).map(|other| other.id);
        let place = found.iter().find_map(|holder| first_place(holder, item).map(|spot| (holder.id, spot)));
        let nothing = found.iter().all(|holder| holder.kind.slots.is_empty() && holder.kind.grids.is_empty());
        (alike, place, if nothing { Refusal::NoAction } else { Refusal::NoRoom })
    };
    if let Some(onto) = alike {
        match apply_stack(roots, id, onto) {
            Ok(done) => return Ok((onto, done)),
            Err(refusal) => refused = refusal,
        }
    }
    match place {
        Some((holder, spot)) => move_item(roots, id, holder, spot, rules).map(|()| (holder, Applied::Put(spot))),
        None => Err(refused),
    }
}

/// Finds a thing a place and puts it there (`ItemManipulator.
/// QuickFindAppropriatePlace`). `targets` are the things to look in: the
/// character's equipment stands for the four containers worn, in the
/// game's order for that kind of thing; anything else for its own slots
/// and grids. What it went into or onto, and what that came to.
pub fn quick_find(roots: &mut [&mut Item], id: Id, targets: &[Id], order: Order, rules: &Rules) -> Result<(Id, Applied), Refusal> {
    let from = root_of(roots, id)?;
    let worn = roots.iter().position(|root| root.kind.template == EQUIPMENT_TEMPLATE);
    if let (true, Some(worn)) = (order.wear, worn) {
        let slot = roots[from].find(id).and_then(|item| slot_to_wear(roots[worn], item));
        if let Some(slot) = slot {
            let into = roots[worn].id;
            if move_item(roots, id, into, Spot::Slot(slot), rules).is_ok() {
                return Ok((into, Applied::Put(Spot::Slot(slot))));
            }
        }
    }
    let mut looked_in = targets.to_vec();
    if let Some(holder) = unmerged_holder(roots[from], id).filter(|holder| order.own_holder && !looked_in.contains(holder)) {
        looked_in.push(holder);
    }
    // What each of them offers: the equipment, its four containers in the
    // order for this kind of thing.
    let groups: Vec<Vec<Id>> = {
        let item = roots[from].find(id).ok_or(Refusal::NoSuchPlace)?;
        let offers = |target: &Id| match roots.iter().find_map(|root| root.find(*target)) {
            Some(found) if found.kind.template == EQUIPMENT_TEMPLATE => loot_order(item).iter().filter_map(|slot| found.in_slot(slot)).map(|holder| holder.id).collect(),
            Some(_) => vec![*target],
            None => Vec::new(),
        };
        looked_in.iter().map(offers).collect()
    };
    if !order.one_by_one {
        return put_among(roots, id, &groups.concat(), rules);
    }
    let mut refused = None;
    for group in &groups {
        match put_among(roots, id, group, rules) {
            Ok(done) => return Ok(done),
            // A refusal that says more than "nowhere" is the one kept.
            Err(refusal) if refused.is_none() || !matches!(refusal, Refusal::NoRoom | Refusal::NoAction) => refused = Some(refusal),
            Err(_) => {}
        }
    }
    Err(refused.unwrap_or(Refusal::NoAction))
}

/// Picks a thing up off the ground (`QuickFindAppropriatePlace` as a
/// pick-up asks it): onto the equipment slot of its kind if that is free,
/// else onto a stack of its kind or into the first room in the four
/// containers worn, in the game's order for that kind of thing. What it
/// came to, and what is left on the ground: the thing itself if it was
/// refused, the rest of a stack that only part of went over.
pub fn pick_up(equipment: &mut Item, item: Item, rules: &Rules) -> (Result<(Id, Applied), Refusal>, Option<Item>) {
    let (id, worn) = (item.id, equipment.id);
    let mut ground = Item::new(Arc::new(Kind::default()));
    ground.loose.push(item);
    let done = quick_find(&mut [&mut *equipment, &mut ground], id, &[worn], Order::PICK_UP, rules);
    (done, ground.loose.pop())
}

/// Whether a pick-up would find the thing a place (the game offers to
/// take a thing only then).
pub fn can_pick_up(equipment: &Item, item: &Item, rules: &Rules) -> bool {
    pick_up(&mut equipment.clone(), item.clone(), rules).0.is_ok()
}

/// The equipment slots whose item the game offers to put on: what its
/// class has the "Equip" of the menu for.
const WEARABLE: [&str; 6] = ["Equipment", "Knife", "MobContainer", "SearchableItem", "Vest", "Weapon"];

/// Puts a thing on (the menu's "Equip", which Alt and a click asks for;
/// `ItemUiContext.EquipItem`): into the first equipment slot of its
/// kind's that takes it. The slot it went into.
pub fn equip(roots: &mut [&mut Item], id: Id, rules: &Rules) -> Result<usize, Refusal> {
    let from = root_of(roots, id)?;
    let worn = roots.iter().position(|root| root.kind.template == EQUIPMENT_TEMPLATE).ok_or(Refusal::NoSuchPlace)?;
    let item = roots[from].find(id).ok_or(Refusal::NoSuchPlace)?;
    if !WEARABLE.iter().any(|class| is(item, class)) {
        return Err(Refusal::NoAction);
    }
    if roots[worn].slots.iter().flatten().any(|worn| worn.id == id) {
        return Err(Refusal::AlreadyThere);
    }
    let slot = slot_to_wear(roots[worn], item).ok_or(Refusal::NoRoom)?;
    let into = roots[worn].id;
    move_item(roots, id, into, Spot::Slot(slot), rules).map(|()| slot)
}

/// How many of a stack may be split off into a place (`SplitMax`): all
/// but one, and into a slot one.
pub fn split_most(roots: &[&mut Item], id: Id, spot: Spot) -> u32 {
    let Some(item) = roots.iter().find_map(|root| root.find(id)) else { return 0 };
    let all_but_one = item.count.saturating_sub(1).min(item.kind.stack_max);
    if matches!(spot, Spot::Slot(_)) { all_but_one.min(1) } else { all_but_one }
}

/// How many of a stack go over when it is dropped on another of its kind
/// with Ctrl held (`TransferMax`): as many as the other has room for, and
/// never the last one. None for two things that do not stack together.
pub fn transfer_most(roots: &[&mut Item], id: Id, onto: Id) -> u32 {
    let find = |id: Id| roots.iter().find_map(|root| root.find(id));
    let (Some(item), Some(target)) = (find(id), find(onto)) else { return 0 };
    let alike = id != onto && item.kind.stack_max > 1 && item.kind.template == target.kind.template;
    if alike { item.count.saturating_sub(1).min(item.kind.stack_max).min(target.stack_room()) } else { 0 }
}

/// Moves so many of a stack onto another of its kind (`TransferResult`
/// carried out with a count).
pub fn transfer_item(roots: &mut [&mut Item], id: Id, onto: Id, count: u32) -> Result<(), Refusal> {
    if count == 0 || count > transfer_most(roots, id, onto) {
        return Err(Refusal::NoAction);
    }
    let (from, to) = (root_of(roots, id)?, root_of(roots, onto)?);
    roots[from].find_mut(id).ok_or(Refusal::NoSuchPlace)?.count -= count;
    roots[to].find_mut(onto).ok_or(Refusal::NoSuchPlace)?.count += count;
    Ok(())
}

/// One stack put on another of its kind with no place chosen
/// (`TransferOrMerge`): the whole of it if the other has the room, else
/// as much as the other has room for.
fn apply_stack(roots: &mut [&mut Item], id: Id, onto: Id) -> Result<Applied, Refusal> {
    let (from, to) = (root_of(roots, id)?, root_of(roots, onto)?);
    let (count, room) = (roots[from].find(id).ok_or(Refusal::NoSuchPlace)?.count, roots[to].find(onto).ok_or(Refusal::NoSuchPlace)?.stack_room());
    if room >= count {
        roots[from].take(id);
        roots[to].find_mut(onto).ok_or(Refusal::NoSuchPlace)?.count += count;
        return Ok(Applied::Merged);
    }
    let most = transfer_most(roots, id, onto);
    transfer_item(roots, id, onto, most).map(|()| Applied::Topped(most))
}

/// The kinds of thing a quick-use key may be bound to
/// (`InventoryController.IsAtBindablePlace`).
const BINDABLE: [&str; 8] = ["Weapon", "ThrowWeap", "Knife", "Meds", "FoodDrink", "PortableRangeFinder", "Compass", "RadioTransmitter"];

/// Whether a thing may be bound to a quick-use key where it lies
/// (`IsAtBindablePlace`): it is of a kind that can be, lies directly in
/// the vest or the pockets worn (`Inventory.FastAccessSlots`), and lacks
/// no part it cannot do without.
pub fn bindable(equipment: &Item, id: Id) -> bool {
    let Some((holder, _)) = equipment.holder_of(id) else { return false };
    let in_reach = ["Pockets", "TacticalVest"].iter().any(|slot| equipment.in_slot(slot).is_some_and(|worn| worn.id == holder));
    in_reach && equipment.find(id).is_some_and(|item| BINDABLE.iter().any(|class| is(item, class)) && !item.missing_vital())
}

/// The things bound to the quick-use keys, 4 to 0 in the game's order
/// (`FastAccess.BoundItems`, `EBoundItem.Item4` to `Item10`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bound {
    pub items: [Option<Id>; 7],
}

impl Bound {
    /// Binds a thing to a key (`BindResult.Run`): refused where it may
    /// not be bound; a thing bound to another key leaves that key, and
    /// what the key had is unbound.
    pub fn bind(&mut self, equipment: &Item, id: Id, key: usize) -> bool {
        if key >= self.items.len() || !bindable(equipment, id) {
            return false;
        }
        for bound in self.items.iter_mut().filter(|bound| **bound == Some(id)) {
            *bound = None;
        }
        self.items[key] = Some(id);
        true
    }

    /// The key a thing is bound to.
    pub fn key_of(&self, id: Id) -> Option<usize> {
        self.items.iter().position(|bound| *bound == Some(id))
    }

    pub fn unbind(&mut self, key: usize) {
        if let Some(bound) = self.items.get_mut(key) {
            *bound = None;
        }
    }

    /// After something has moved: what no longer lies where it may be
    /// bound is unbound (`ItemManipulator.TryUnbindOneItem`).
    pub fn settle(&mut self, equipment: &Item) {
        for bound in self.items.iter_mut().filter(|bound| bound.is_some_and(|id| !bindable(equipment, id))) {
            *bound = None;
        }
    }
}

/// The equipment slots whose things are within reach for a reload
/// (`Inventory.FastAccessSlots`), in the game's order: the pockets'
/// stacks are taken before the vest's.
const WITHIN_REACH: [&str; 2] = ["Pockets", "TacticalVest"];

/// The stacks of rounds within reach that a weapon's chamber takes
/// (`InventoryController.GetReachableItemsOfType`, with the test the
/// reload key makes: `Chambers[0].CanAccept`): what lies directly in the
/// pockets and the vest worn, in the order the game goes through them.
fn rounds_within_reach(equipment: &Item, weapon: Id) -> Vec<Id> {
    let Some(takes) = equipment.find(weapon).and_then(takes_rounds) else {
        return Vec::new();
    };
    let worn = WITHIN_REACH.iter().filter_map(|slot| equipment.in_slot(slot));
    worn.flat_map(|holder| holder.grids.iter().flatten()).filter(|(_, item)| item.kind.among(&takes)).map(|(_, item)| item.id).collect()
}

/// How many rounds the character has within reach for a weapon carried:
/// what a tube is filled from (`FirearmHandsInputTranslator.ReloadWithAmmo`).
pub fn rounds_in_reach(equipment: &Item, weapon: Id) -> u32 {
    rounds_within_reach(equipment, weapon).iter().filter_map(|id| equipment.find(*id)).map(|stack| stack.count).sum()
}

/// Takes so many of those rounds for the weapon, the first stack first
/// (`AmmoPack.LoadAmmo`); a stack used up is gone. How many were taken.
pub fn take_rounds(equipment: &mut Item, weapon: Id, count: u32) -> u32 {
    let mut left = count;
    for id in rounds_within_reach(equipment, weapon) {
        let Some(stack) = equipment.find_mut(id) else { continue };
        let taken = stack.count.min(left);
        stack.count -= taken;
        left -= taken;
        if stack.count == 0 {
            equipment.take(id);
        }
        if left == 0 {
            break;
        }
    }
    count - left
}

/// The slots a weapon is taken in hand from, in the order the game looks
/// through them for something to hold (`Player._slotPriority`).
pub const HAND_SLOTS: [&str; 4] = ["FirstPrimaryWeapon", "SecondPrimaryWeapon", "Holster", "Scabbard"];

/// Drops one item on another (`ItemController.ExecutePossibleAction`,
/// `CompoundItem.Apply`): two stacks of one kind become one if the other
/// has the room; else it goes into the first of the other's slots that
/// takes it, else into the first of its grids with room; else as much of
/// a stack as the other holds goes over. Swapping two things by dropping
/// one on the other is switched off in the game.
pub fn apply_item(roots: &mut [&mut Item], id: Id, onto: Id, rules: &Rules) -> Result<Applied, Refusal> {
    if id == onto {
        return Err(Refusal::NoAction);
    }
    let (from, to) = (root_of(roots, id)?, root_of(roots, onto)?);
    let (item, target) = (roots[from].find(id).ok_or(Refusal::NoSuchPlace)?, roots[to].find(onto).ok_or(Refusal::NoSuchPlace)?);
    let alike = item.kind.stack_max > 1 && item.kind.template == target.kind.template;
    let (count, room) = (item.count, target.stack_room());
    if alike && room >= count {
        roots[from].take(id);
        roots[to].find_mut(onto).ok_or(Refusal::NoSuchPlace)?.count += count;
        return Ok(Applied::Merged);
    }
    // (A chamber is not among the places a thing dropped on a weapon is
    // tried in: what the game does with a round dropped there is not read.)
    let slots: Vec<usize> = (0..target.kind.slots.len())
        .filter(|slot| !is_chamber(&target.kind.slots[*slot].name) && target.slot_takes(*slot, item).is_ok())
        .collect();
    let cells = (0..target.kind.grids.len()).find_map(|grid| target.find_room(grid, item).map(|place| Spot::Cell(grid, place)));
    let mut refused = Refusal::NoAction;
    for spot in slots.into_iter().map(Spot::Slot).chain(cells) {
        match move_item(roots, id, onto, spot, rules) {
            Ok(()) => return Ok(Applied::Put(spot)),
            Err(refusal) => refused = refusal,
        }
    }
    if alike && room > 0 {
        roots[from].find_mut(id).ok_or(Refusal::NoSuchPlace)?.count -= room;
        roots[to].find_mut(onto).ok_or(Refusal::NoSuchPlace)?.count += room;
        return Ok(Applied::Topped(room));
    }
    Err(refused)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn holder(name: &str, cells: [u32; 2], takes: &[&str], refuses: &[&str]) -> Holder {
        Holder { name: name.into(), cells, takes: takes.iter().map(|s| s.to_string()).collect(), refuses: refuses.iter().map(|s| s.to_string()).collect(), ..Holder::default() }
    }

    /// A kind of thing of a class: the class's name as the database would
    /// have it is the word with its first letter large (`weapon` is a
    /// `Weapon`).
    fn kind(name: &str, class: &str, weight: f32, cells: [u32; 2]) -> Kind {
        let named = format!("{}{}", class[..1].to_uppercase(), &class[1..]);
        Kind { template: name.into(), name: name.into(), classes: vec![name.into(), class.into(), "item".into()], class_names: vec![name.into(), named], weight, cells, ..Kind::default() }
    }

    /// Another of the same.
    fn again(item: &Item) -> Item {
        Item { id: next_id(), ..item.clone() }
    }

    fn thing(name: &str, class: &str, weight: f32, cells: [u32; 2]) -> Item {
        Item::new(Arc::new(kind(name, class, weight, cells)))
    }

    /// A grid gives the first room it has, rows from the top and cells
    /// from the left, turns what fits no other way, and takes nothing on
    /// a taken cell, outside itself or against its filter.
    #[test]
    fn a_grid_finds_room_row_by_row_and_turns_what_does_not_fit() {
        let mut pack = Item::new(Arc::new(Kind { grids: vec![holder("main", [4, 3], &["item"], &["case"])], ..kind("pack", "backpack", 1.0, [4, 4]) }));
        let (magazine, rifle, case) = (thing("magazine", "magazine", 0.2, [1, 2]), thing("rifle", "weapon", 3.0, [3, 1]), thing("box", "case", 1.0, [1, 1]));
        assert_eq!(pack.stow(again(&magazine)), Ok((0, Place { x: 0, y: 0, turned: false })));
        // The rifle goes beside the magazine in the top row.
        assert_eq!(pack.stow(again(&rifle)), Ok((0, Place { x: 1, y: 0, turned: false })));
        // The next magazine: the first free cell from which it fits is under the rifle.
        assert_eq!(pack.find_room(0, &magazine), Some(Place { x: 1, y: 1, turned: false }));
        pack.put(0, again(&magazine), Place { x: 1, y: 1, turned: false }).unwrap();
        // A second rifle fits flat nowhere now, and turned not either
        // (three rows, but no free column of three).
        assert_eq!(pack.find_room(0, &rifle), None);
        let turned = Place { x: 3, y: 0, turned: true };
        assert_eq!(pack.grid_takes(0, &rifle, turned), Err(Refusal::Taken));
        assert_eq!(pack.grid_takes(0, &magazine, Place { x: 3, y: 2, turned: false }), Err(Refusal::OutsideGrid));
        assert_eq!(pack.grid_takes(0, &magazine, Place { x: 2, y: 2, turned: true }), Ok(()));
        assert_eq!(pack.grid_takes(0, &case, Place { x: 0, y: 2, turned: false }), Err(Refusal::Filter));
        // Its weight is its own and all that is in it.
        assert!((pack.weight() - (1.0 + 0.2 + 3.0 + 0.2)).abs() < 1e-6);
    }

    /// An item is as large as its parts make it: the largest extra on
    /// each side, not their sum; and a stack weighs its count.
    #[test]
    fn parts_make_an_item_longer_by_the_largest_on_each_side() {
        let part = |name: &str, extra: [u32; 4]| Item::new(Arc::new(Kind { extra, ..kind(name, "mod", 0.1, [1, 1]) }));
        let slots = vec![holder("stock", [0, 0], &["mod"], &[]), holder("barrel", [0, 0], &["mod"], &[]), holder("sight", [0, 0], &["mod"], &[])];
        let mut rifle = Item::new(Arc::new(Kind { slots, merges: true, ..kind("rifle", "weapon", 2.0, [2, 1]) }));
        assert_eq!(rifle.cells(), [2, 1]);
        rifle.slots[0] = Some(part("stock", [0, 1, 0, 0]));
        rifle.slots[1] = Some(part("barrel", [2, 0, 0, 0]));
        rifle.slots[2] = Some(part("sight", [1, 0, 1, 0]));
        assert_eq!(rifle.cells(), [5, 2]);
        assert!((rifle.weight() - 2.3).abs() < 1e-6);
        let mut rounds = thing("round", "ammo", 0.012, [1, 1]);
        rounds.count = 30;
        assert!((rounds.weight() - 0.36).abs() < 1e-6);
    }

    /// An equipment slot takes what its filter names, one at a time; a
    /// helmet that blocks the earpiece keeps a headset out, and is kept
    /// out by one already worn. The carried weight is every slot's, and
    /// without the weapons' at elite Strength.
    #[test]
    fn equipment_slots_filter_block_and_weigh() {
        let slots = vec![holder("FirstPrimaryWeapon", [0, 0], &["weapon"], &[]), holder("Headwear", [0, 0], &["headwear"], &[]), holder("Earpiece", [0, 0], &["headset"], &[])];
        let mut equipment = Equipment { item: Item::new(Arc::new(Kind { slots, ..kind("equipment", "inventory", 0.0, [1, 1]) })) };
        let rifle = thing("rifle", "weapon", 3.5, [5, 2]);
        let headset = thing("comtac", "headset", 0.6, [2, 2]);
        let cap = thing("cap", "headwear", 0.1, [1, 1]);
        let helmet = Item::new(Arc::new(Kind { blocks: vec!["Earpiece".into()], ..kind("helmet", "headwear", 1.5, [2, 2]) }));
        assert_eq!(equipment.can_wear("Headwear", &rifle), Err(Refusal::Filter));
        assert_eq!(equipment.can_wear("Backpack", &rifle), Err(Refusal::NoSuchPlace));
        equipment.wear("FirstPrimaryWeapon", rifle.clone()).unwrap();
        assert_eq!(equipment.can_wear("FirstPrimaryWeapon", &rifle), Err(Refusal::Taken));
        equipment.wear("Earpiece", headset.clone()).unwrap();
        assert_eq!(equipment.can_wear("Headwear", &helmet), Err(Refusal::WouldBlock("Earpiece".into())));
        equipment.wear("Headwear", cap).unwrap();
        assert!((equipment.carried_weight(false) - 4.2).abs() < 1e-6);
        assert!((equipment.carried_weight(true) - 0.7).abs() < 1e-6);
        // The other way round: the helmet on, the headset is kept out.
        equipment.take_off("Headwear");
        assert_eq!(equipment.take_off("Earpiece"), Some(headset.clone()));
        equipment.wear("Headwear", helmet).unwrap();
        assert_eq!(equipment.can_wear("Earpiece", &headset), Err(Refusal::Blocked("Headwear".into())));
        assert_eq!(equipment.drawn(), ["helmet"]);
    }

    const RAID: Rules = Rules { in_raid: true, backpacks_deep: 2 };

    fn pack(name: &str, cells: [u32; 2]) -> Item {
        Item::new(Arc::new(Kind { grids: vec![holder("main", cells, &["item"], &[])], classes: vec![name.into(), BACKPACK.into(), "item".into()], ..kind(name, "backpack", 1.0, [3, 3]) }))
    }

    /// A thing is moved from a crate onto the character and about in its
    /// containers by the rules of the place it goes to; a move that is
    /// refused leaves everything where it was.
    #[test]
    fn things_move_between_a_crate_and_the_character() {
        let slots = vec![holder("FirstPrimaryWeapon", [0, 0], &["weapon"], &[]), holder("Backpack", [0, 0], &[BACKPACK], &[]), holder("Dogtag", [0, 0], &["item"], &[])];
        let mut worn = Item::new(Arc::new(Kind { slots, ..kind(EQUIPMENT_TEMPLATE, "inventory", 0.0, [1, 1]) }));
        let mut crate_ = Item::new(Arc::new(Kind { grids: vec![holder("main", [5, 5], &["item"], &[])], ..kind("crate", "container", 1.0, [1, 1]) }));
        let (rifle, bag, magazine) = (thing("rifle", "weapon", 3.5, [5, 2]), pack("bag", [4, 4]), thing("magazine", "magazine", 0.3, [1, 2]));
        let (rifle_id, bag_id, magazine_id) = (rifle.id, bag.id, magazine.id);
        for item in [rifle, bag, magazine] {
            crate_.stow(item).unwrap();
        }
        let (worn_id, crate_id) = (worn.id, crate_.id);
        let before = (worn.clone(), crate_.clone());
        let mut roots = [&mut worn, &mut crate_];
        // The rifle is no backpack, and the dogtag's slot takes nothing.
        assert_eq!(move_item(&mut roots, rifle_id, worn_id, Spot::Slot(1), &RAID), Err(Refusal::Filter));
        assert_eq!(move_item(&mut roots, magazine_id, worn_id, Spot::Slot(2), &RAID), Err(Refusal::Dogtag));
        // Where it lies already, and onto a taken cell.
        let (_, lies) = roots[1].holder_of(magazine_id).unwrap();
        assert_eq!(move_item(&mut roots, magazine_id, crate_id, lies, &RAID), Err(Refusal::AlreadyThere));
        assert_eq!(move_item(&mut roots, magazine_id, crate_id, Spot::Cell(0, Place { x: 0, y: 0, turned: false }), &RAID), Err(Refusal::Taken));
        assert_eq!(move_item(&mut roots, bag_id, bag_id, Spot::Cell(0, Place { x: 0, y: 0, turned: false }), &RAID), Err(Refusal::Cycle));
        assert!(*roots[0] == before.0 && *roots[1] == before.1, "a refused move changed something");
        // Onto the sling, onto the back, and the magazine into the bag worn.
        assert_eq!(move_item(&mut roots, rifle_id, worn_id, Spot::Slot(0), &RAID), Ok(()));
        assert_eq!(move_item(&mut roots, bag_id, worn_id, Spot::Slot(1), &RAID), Ok(()));
        assert_eq!(apply_item(&mut roots, magazine_id, bag_id, &RAID), Ok(Applied::Put(Spot::Cell(0, Place { x: 0, y: 0, turned: false }))));
        assert_eq!(roots[0].holder_of(magazine_id), Some((bag_id, Spot::Cell(0, Place { x: 0, y: 0, turned: false }))));
        assert!(roots[1].contents().next().is_none());
        // What is carried is what was in the crate.
        assert!((roots[0].weight() - (3.5 + 1.0 + 0.3)).abs() < 1e-6);
        // What names its own slot stays on the character through a raid.
        let pouch = Item::new(Arc::new(Kind { kept_in: vec!["Dogtag".into()], ..kind("pouch", "item", 0.5, [1, 1]) }));
        let pouch_id = pouch.id;
        roots[0].slots[2] = Some(pouch);
        let free_cell = Spot::Cell(0, Place { x: 0, y: 0, turned: false });
        assert_eq!(move_item(&mut roots, pouch_id, crate_id, free_cell, &RAID), Err(Refusal::KeptOn("Dogtag".into())));
        assert_eq!(move_item(&mut roots, pouch_id, crate_id, free_cell, &Rules { in_raid: false, ..RAID }), Ok(()));
        // Turned and moved within the bag.
        let turned = Spot::Cell(0, Place { x: 2, y: 1, turned: true });
        assert_eq!(move_item(&mut roots, magazine_id, bag_id, turned, &RAID), Ok(()));
        assert_eq!(roots[0].holder_of(magazine_id), Some((bag_id, turned)));
    }

    /// A stack is split into a free cell, a thing is put down and picked
    /// up again into the first place that takes it, and what stays in its
    /// slot through a raid is not put down.
    #[test]
    fn stacks_split_and_things_are_dropped_and_picked_up() {
        let slots = vec![holder("FirstPrimaryWeapon", [0, 0], &["weapon"], &[]), holder("Backpack", [0, 0], &[BACKPACK], &[]), holder("Pockets", [0, 0], &["pockets"], &[])];
        let mut worn = Item::new(Arc::new(Kind { slots, ..kind(EQUIPMENT_TEMPLATE, "inventory", 0.0, [1, 1]) }));
        let bag = pack("bag", [3, 2]);
        let mut rounds = Item::new(Arc::new(Kind { stack_max: 60, ..kind("round", "ammo", 0.01, [1, 1]) }));
        rounds.count = 30;
        let pockets = Item::new(Arc::new(Kind { kept_in: vec!["Pockets".into()], ..kind("pockets", "pockets", 0.0, [1, 1]) }));
        let rifle = thing("rifle", "weapon", 3.0, [3, 1]);
        let (bag_id, rounds_id, pockets_id, rifle_id, worn_id) = (bag.id, rounds.id, pockets.id, rifle.id, worn.id);
        worn.slots[1] = Some(bag);
        worn.slots[2] = Some(pockets);
        worn.find_mut(bag_id).unwrap().stow(rounds).unwrap();
        let mut roots = [&mut worn];
        // Ten of thirty into the next cell; not onto a taken one, and not all.
        let cell = |x| Spot::Cell(0, Place { x, y: 0, turned: false });
        assert_eq!(split_item(&mut roots, rounds_id, 10, bag_id, cell(0), &RAID), Err(Refusal::Taken));
        assert_eq!(split_item(&mut roots, rounds_id, 30, bag_id, cell(1), &RAID), Err(Refusal::NoAction));
        let part = split_item(&mut roots, rounds_id, 10, bag_id, cell(1), &RAID).unwrap();
        assert_eq!((roots[0].find(rounds_id).unwrap().count, roots[0].find(part).unwrap().count), (20, 10));
        // Put down, and the weight goes with it.
        assert_eq!(take_out(&mut roots, pockets_id, &RAID), Err(Refusal::KeptOn("Pockets".into())));
        let dropped = take_out(&mut roots, part, &RAID).unwrap();
        assert!((roots[0].weight() - (1.0 + 0.2)).abs() < 1e-6);
        // Picked up: the rounds go onto the stack of their kind in the
        // bag, a rifle onto the sling, and a second rifle, too long for
        // what room the bag has left, nowhere.
        assert_eq!(pick_up(roots[0], dropped, &RAID), (Ok((rounds_id, Applied::Merged)), None));
        assert_eq!(roots[0].find(rounds_id).unwrap().count, 30);
        assert_eq!(pick_up(roots[0], rifle, &RAID), (Ok((worn_id, Applied::Put(Spot::Slot(0)))), None));
        assert_eq!(roots[0].holder_of(rifle_id), Some((worn_id, Spot::Slot(0))));
        let long = thing("long rifle", "weapon", 4.0, [4, 1]);
        assert!(!can_pick_up(roots[0], &long, &RAID));
        let back = pick_up(roots[0], long.clone(), &RAID);
        assert_eq!(back, (Err(Refusal::NoRoom), Some(long)));
    }

    /// A character with a vest, pockets and a backpack on, and a crate
    /// beside it: the equipment, the three worn, the crate.
    fn dressed() -> (Item, [Id; 3], Item) {
        let slot = |name: &str, class: &str| holder(name, [0, 0], &[class], &[]);
        let slots = vec![
            slot("FirstPrimaryWeapon", "weapon"),
            slot("SecondPrimaryWeapon", "weapon"),
            slot("Holster", "pistol"),
            slot("Backpack", BACKPACK),
            slot("TacticalVest", "vest"),
            slot("Pockets", "pockets"),
            slot("Headwear", "headwear"),
        ];
        let mut worn = Item::new(Arc::new(Kind { slots, ..kind(EQUIPMENT_TEMPLATE, "inventory", 0.0, [1, 1]) }));
        let grids = |cells: &[[u32; 2]]| cells.iter().enumerate().map(|(at, cells)| holder(&format!("{at}"), *cells, &["item"], &[])).collect();
        let vest = Item::new(Arc::new(Kind { grids: grids(&[[1, 2], [1, 2], [2, 2]]), ..kind("vest", "vest", 1.0, [3, 3]) }));
        let pockets = Item::new(Arc::new(Kind { grids: grids(&[[1, 1], [1, 1]]), kept_in: vec!["Pockets".into()], ..kind("pockets", "pockets", 0.0, [1, 1]) }));
        let bag = pack("bag", [4, 4]);
        let ids = [vest.id, pockets.id, bag.id];
        (worn.slots[4], worn.slots[5], worn.slots[3]) = (Some(vest), Some(pockets), Some(bag));
        let crate_ = Item::new(Arc::new(Kind { grids: vec![holder("main", [6, 6], &["item"], &[])], ..kind("crate", "container", 1.0, [1, 1]) }));
        (worn, ids, crate_)
    }

    fn stack(name: &str, class: &str, count: u32) -> Item {
        let mut rounds = Item::new(Arc::new(Kind { stack_max: 60, ..kind(name, class, 0.01, [1, 1]) }));
        rounds.count = count;
        rounds
    }

    /// Ctrl and a click sends a thing to the other side: out of the crate
    /// into the container the game looks in first for that kind of thing,
    /// off the character into the crate, and with nothing open to the
    /// next place the character has. A stack of the kind over there takes
    /// it before any free cell does.
    #[test]
    fn a_thing_is_sent_to_the_other_side() {
        let (mut worn, [vest_id, pockets_id, bag_id], mut crate_) = dressed();
        let (worn_id, crate_id) = (worn.id, crate_.id);
        let (magazine, grenade, tin) = (thing("magazine", "magazine", 0.3, [1, 2]), thing("grenade", "throwWeap", 0.6, [1, 1]), thing("tin", "food", 0.3, [1, 1]));
        let (rounds, more) = (stack("round", "ammo", 40), stack("round", "ammo", 30));
        let (magazine_id, grenade_id, tin_id, rounds_id, more_id) = (magazine.id, grenade.id, tin.id, rounds.id, more.id);
        for item in [magazine, grenade, tin, rounds, more] {
            crate_.stow(item).unwrap();
        }
        let cell = |grid, x, y| Applied::Put(Spot::Cell(grid, Place { x, y, turned: false }));
        let mut roots = [&mut worn, &mut crate_];
        // Out of the crate: a magazine to the vest, a grenade to the
        // pockets, anything else to the backpack.
        assert_eq!(quick_find(&mut roots, magazine_id, &[worn_id], Order::OTHER_SIDE, &RAID), Ok((vest_id, cell(0, 0, 0))));
        assert_eq!(quick_find(&mut roots, grenade_id, &[worn_id], Order::OTHER_SIDE, &RAID), Ok((pockets_id, cell(0, 0, 0))));
        assert_eq!(quick_find(&mut roots, tin_id, &[worn_id], Order::OTHER_SIDE, &RAID), Ok((bag_id, cell(0, 0, 0))));
        // Rounds to the vest; the second lot onto the first, as far as it
        // has room, and the rest stays in the crate.
        assert_eq!(quick_find(&mut roots, rounds_id, &[worn_id], Order::OTHER_SIDE, &RAID), Ok((vest_id, cell(1, 0, 0))));
        assert_eq!(quick_find(&mut roots, more_id, &[worn_id], Order::OTHER_SIDE, &RAID), Ok((rounds_id, Applied::Topped(20))));
        assert_eq!((roots[0].find(rounds_id).unwrap().count, roots[1].find(more_id).unwrap().count), (60, 10));
        // And again: the stack over there is full, so into the next room
        // the vest has.
        assert_eq!(quick_find(&mut roots, more_id, &[worn_id], Order::OTHER_SIDE, &RAID), Ok((vest_id, cell(1, 0, 1))));
        // Off the character, with the crate open: into the crate's first
        // free cell (the crate, then the character, are looked in).
        assert_eq!(quick_find(&mut roots, magazine_id, &[crate_id, worn_id], Order::OTHER_SIDE, &RAID), Ok((crate_id, cell(0, 0, 0))));
        // With nothing open, a thing on the character goes to the first
        // other place the character has for it: the tin from the bag's
        // first cell to its second.
        assert_eq!(quick_find(&mut roots, tin_id, &[worn_id], Order::OTHER_SIDE, &RAID), Ok((bag_id, cell(0, 1, 0))));
        // What stays on through a raid is not sent anywhere.
        assert_eq!(quick_find(&mut roots, pockets_id, &[crate_id, worn_id], Order::OTHER_SIDE, &RAID), Err(Refusal::KeptOn("Pockets".into())));
    }

    /// Alt and a click puts a thing on: a weapon onto the sling, the next
    /// onto the back, a third nowhere; a magazine is not put on at all.
    #[test]
    fn a_thing_is_put_on_by_its_kind() {
        let (mut worn, [_, _, bag_id], mut crate_) = dressed();
        let rifles = [thing("rifle", "weapon", 3.0, [4, 1]), thing("carbine", "weapon", 3.0, [3, 1]), thing("shotgun", "weapon", 3.0, [4, 1])];
        let [rifle_id, carbine_id, shotgun_id] = [rifles[0].id, rifles[1].id, rifles[2].id];
        // (In the database headwear is equipment, which is what is put on.)
        let hat = Item::new(Arc::new(Kind { class_names: vec!["hat".into(), "Headwear".into(), "Equipment".into()], ..kind("hat", "headwear", 0.1, [1, 1]) }));
        let magazine = thing("magazine", "magazine", 0.3, [1, 2]);
        let (hat_id, magazine_id) = (hat.id, magazine.id);
        for item in rifles.into_iter().chain([hat, magazine]) {
            crate_.stow(item).unwrap();
        }
        let mut roots = [&mut worn, &mut crate_];
        assert_eq!(equip(&mut roots, rifle_id, &RAID), Ok(0));
        assert_eq!(equip(&mut roots, rifle_id, &RAID), Err(Refusal::AlreadyThere));
        assert_eq!(equip(&mut roots, carbine_id, &RAID), Ok(1));
        // The holster's filter does not take a long weapon.
        assert_eq!(equip(&mut roots, shotgun_id, &RAID), Err(Refusal::NoRoom));
        assert_eq!(equip(&mut roots, hat_id, &RAID), Ok(6));
        assert_eq!(equip(&mut roots, magazine_id, &RAID), Err(Refusal::NoAction));
        // A pick-up puts a weapon on the same way, and with the slots
        // taken puts it in the backpack.
        let (found, left) = pick_up(roots[0], thing("pistol-sized rifle", "weapon", 1.0, [2, 1]), &RAID);
        assert_eq!((found.map(|(into, _)| into), left), (Ok(bag_id), None));
    }

    /// With Ctrl held a drop is only ever part of a stack: all but one at
    /// the most, into a slot one, onto a stack of the kind as many as it
    /// has room for.
    #[test]
    fn part_of_a_stack_is_split_off_or_handed_over() {
        let (mut worn, [vest_id, ..], mut crate_) = dressed();
        let (a, b, lone) = (stack("round", "ammo", 60), stack("round", "ammo", 45), stack("round", "ammo", 1));
        let (a_id, b_id, lone_id) = (a.id, b.id, lone.id);
        for item in [a, b, lone] {
            crate_.stow(item).unwrap();
        }
        let mut roots = [&mut worn, &mut crate_];
        let cell = Spot::Cell(0, Place { x: 0, y: 0, turned: false });
        assert_eq!((split_most(&roots, a_id, cell), split_most(&roots, a_id, Spot::Slot(0)), split_most(&roots, lone_id, cell)), (59, 1, 0));
        // Onto the 45: it has room for 15; onto the 60: none.
        assert_eq!((transfer_most(&roots, a_id, b_id), transfer_most(&roots, b_id, a_id), transfer_most(&roots, a_id, a_id)), (15, 0, 0));
        assert_eq!(transfer_item(&mut roots, a_id, b_id, 16), Err(Refusal::NoAction));
        assert_eq!(transfer_item(&mut roots, a_id, b_id, 7), Ok(()));
        assert_eq!((roots[1].find(a_id).unwrap().count, roots[1].find(b_id).unwrap().count), (53, 52));
        let part = split_item(&mut roots, a_id, 20, vest_id, cell, &RAID).unwrap();
        assert_eq!((roots[1].find(a_id).unwrap().count, roots[0].find(part).unwrap().count), (33, 20));
    }

    /// A quick-use key takes medicine, food, a grenade or a weapon that
    /// lies in the vest or the pockets, one key a thing and one thing a
    /// key, and lets go of what is moved out of them.
    #[test]
    fn things_are_bound_to_the_quick_use_keys() {
        let (mut worn, [vest_id, pockets_id, bag_id], _) = dressed();
        let (kit, tin, magazine) = (thing("kit", "meds", 0.2, [1, 1]), thing("tin", "foodDrink", 0.3, [1, 1]), thing("magazine", "magazine", 0.3, [1, 2]));
        let (kit_id, tin_id, magazine_id) = (kit.id, tin.id, magazine.id);
        worn.find_mut(vest_id).unwrap().stow(magazine).unwrap();
        worn.find_mut(pockets_id).unwrap().stow(kit).unwrap();
        worn.find_mut(bag_id).unwrap().stow(tin).unwrap();
        let mut bound = Bound::default();
        // A magazine is not bound, and nothing in the backpack is.
        assert!(!bound.bind(&worn, magazine_id, 0) && !bound.bind(&worn, tin_id, 0));
        assert!(bound.bind(&worn, kit_id, 0));
        assert!(bound.bind(&worn, kit_id, 3));
        assert_eq!((bound.items[0], bound.key_of(kit_id)), (None, Some(3)));
        // Into the vest's last grid the tin may be bound, and takes the
        // kit's key from it.
        let cell = Spot::Cell(2, Place { x: 0, y: 0, turned: false });
        assert_eq!(move_item(&mut [&mut worn], tin_id, vest_id, cell, &RAID), Ok(()));
        assert!(bound.bind(&worn, tin_id, 3));
        assert_eq!((bound.key_of(kit_id), bound.key_of(tin_id)), (None, Some(3)));
        // Moved back to the backpack it is bound no longer.
        let free = Spot::Cell(0, Place { x: 2, y: 2, turned: false });
        assert_eq!(move_item(&mut [&mut worn], tin_id, bag_id, free, &RAID), Ok(()));
        bound.settle(&worn);
        assert_eq!(bound.items, [None; 7]);
    }

    /// Two stacks of a kind become one where the other has the room, and
    /// else the other is filled up; backpacks lie in one another only so
    /// deep; a vital part stays on its weapon in a raid, and a weapon
    /// without one does not go onto the character.
    #[test]
    fn stacks_merge_and_the_raid_rules_hold() {
        let rounds = |count: u32| {
            let mut stack = Item::new(Arc::new(Kind { stack_max: 60, ..kind("round", "ammo", 0.01, [1, 1]) }));
            stack.count = count;
            stack
        };
        let mut box_ = pack("box", [6, 6]);
        let (a, b, c) = (rounds(20), rounds(30), rounds(50));
        let (a_id, b_id, c_id) = (a.id, b.id, c.id);
        for stack in [a, b, c] {
            box_.stow(stack).unwrap();
        }
        let (inner, innermost) = (pack("inner", [3, 3]), pack("innermost", [2, 2]));
        let (inner_id, innermost_id, box_id) = (inner.id, innermost.id, box_.id);
        box_.stow(inner).unwrap();
        box_.stow(innermost).unwrap();
        let slots = vec![Holder { required: true, ..holder("barrel", [0, 0], &["mod"], &[]) }];
        let mut rifle = Item::new(Arc::new(Kind { slots, ..kind("rifle", "weapon", 3.0, [3, 1]) }));
        let barrel = thing("barrel", "mod", 0.5, [2, 1]);
        let (rifle_id, barrel_id) = (rifle.id, barrel.id);
        rifle.slots[0] = Some(barrel);
        box_.stow(rifle).unwrap();
        let mut worn = Item::new(Arc::new(Kind { slots: vec![holder("FirstPrimaryWeapon", [0, 0], &["weapon"], &[])], ..kind(EQUIPMENT_TEMPLATE, "inventory", 0.0, [1, 1]) }));
        let worn_id = worn.id;
        let mut roots = [&mut box_, &mut worn];
        assert_eq!(apply_item(&mut roots, a_id, b_id, &RAID), Ok(Applied::Merged));
        assert_eq!((roots[0].find(a_id), roots[0].find(b_id).map(|stack| stack.count)), (None, Some(50)));
        assert_eq!(apply_item(&mut roots, c_id, b_id, &RAID), Ok(Applied::Topped(10)));
        assert_eq!((roots[0].find(c_id).map(|stack| stack.count), roots[0].find(b_id).map(|stack| stack.count)), (Some(40), Some(60)));
        // One backpack in the box is two deep; a third is one too many.
        assert_eq!(apply_item(&mut roots, innermost_id, inner_id, &RAID), Err(Refusal::TooDeep));
        let free = Rules { backpacks_deep: 0, ..RAID };
        assert!(matches!(apply_item(&mut roots, innermost_id, inner_id, &free), Ok(Applied::Put(_))));
        // The barrel stays on in a raid; out of one it comes off, and the
        // rifle then does not go onto the character.
        let cell = Spot::Cell(0, roots[0].find_room(0, roots[0].find(barrel_id).unwrap()).unwrap());
        assert_eq!(move_item(&mut roots, barrel_id, box_id, cell, &RAID), Err(Refusal::Vital("barrel".into())));
        assert_eq!(move_item(&mut roots, barrel_id, box_id, cell, &Rules { in_raid: false, ..RAID }), Ok(()));
        assert_eq!(move_item(&mut roots, rifle_id, worn_id, Spot::Slot(0), &RAID), Err(Refusal::MissingParts));
        assert_eq!(apply_item(&mut roots, barrel_id, rifle_id, &Rules { in_raid: false, ..RAID }), Ok(Applied::Put(Spot::Slot(0))));
        assert_eq!(move_item(&mut roots, rifle_id, worn_id, Spot::Slot(0), &RAID), Ok(()));
    }

    /// The rounds a tube is filled from are those within reach that the
    /// weapon's chamber takes: what lies directly in the pockets and the
    /// vest, the pockets' first; the backpack's are out of reach.
    #[test]
    fn a_tube_is_filled_from_the_pockets_and_the_vest() {
        let (mut worn, [vest_id, pockets_id, bag_id], _) = dressed();
        let chamber = holder(CHAMBER, [0, 0], &["shell"], &[]);
        let gun = Item::new(Arc::new(Kind { slots: vec![chamber], ..kind("shotgun", "weapon", 3.0, [5, 1]) }));
        let gun_id = gun.id;
        worn.slots[0] = Some(gun);
        let (in_vest, in_pocket, in_bag, other) = (stack("shell", "ammo", 7), stack("shell", "ammo", 2), stack("shell", "ammo", 20), stack("round", "ammo", 30));
        let (vest_shells, pocket_shells) = (in_vest.id, in_pocket.id);
        for (holder, item) in [(vest_id, in_vest), (pockets_id, in_pocket), (bag_id, in_bag), (vest_id, other)] {
            worn.find_mut(holder).unwrap().stow(item).unwrap();
        }
        // Seven in the vest and two in a pocket; the bag's twenty and the
        // rifle rounds do not count.
        assert_eq!(rounds_in_reach(&worn, gun_id), 9);
        // The pocket's go first, and a stack used up is gone.
        assert_eq!(take_rounds(&mut worn, gun_id, 3), 3);
        assert!(worn.find(pocket_shells).is_none());
        assert_eq!(worn.find(vest_shells).unwrap().count, 6);
        // No more is taken than there is.
        assert_eq!(take_rounds(&mut worn, gun_id, 10), 6);
        assert_eq!(rounds_in_reach(&worn, gun_id), 0);
    }

    /// What Ctrl and a click looks in: the priority window's container and
    /// nothing else, unless the thing lies in it; else what is open, with
    /// the equipment after it for a thing the character has; else the
    /// equipment.
    #[test]
    fn what_a_thing_sent_over_is_looked_for_room_in() {
        let (mut worn, [vest_id, _, bag_id], mut crate_) = dressed();
        let (worn_id, crate_id) = (worn.id, crate_.id);
        let (mine, theirs) = (thing("tin", "food", 0.3, [1, 1]), thing("can", "food", 0.3, [1, 1]));
        let (mine_id, theirs_id) = (mine.id, theirs.id);
        worn.find_mut(vest_id).unwrap().stow(mine).unwrap();
        crate_.stow(theirs).unwrap();
        assert_eq!(other_side(&worn, None, None, mine_id), [worn_id]);
        assert_eq!(other_side(&worn, Some(&crate_), None, mine_id), [crate_id, worn_id]);
        assert_eq!(other_side(&worn, Some(&crate_), None, theirs_id), [worn_id]);
        // The bag's window is the priority one: that and nothing else.
        assert_eq!(other_side(&worn, Some(&crate_), Some(bag_id), mine_id), [bag_id]);
        assert_eq!(other_side(&worn, Some(&crate_), Some(bag_id), theirs_id), [bag_id]);
        // Unless the thing lies in it.
        assert_eq!(other_side(&worn, Some(&crate_), Some(vest_id), mine_id), [crate_id, worn_id]);
    }
}
