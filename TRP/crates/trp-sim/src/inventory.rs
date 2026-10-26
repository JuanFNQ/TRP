//! What the character carries and where each thing sits, by the game's
//! rules (`docs/spec/inventory.md`): items with slots and grids, what a
//! slot and a grid accept, where a grid finds room, an item's size in
//! cells and its weight, and the carried weight the controller goes by.
//! No engine types; a host fills it and reads it.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    /// Whether those cells are added beside its fellow parts' rather than
    /// only where they reach further (`ExtraSizeForceAdd`).
    pub forced_extra: bool,
    /// Whether its parts' extra cells count toward its size.
    pub merges: bool,
    pub slots: Vec<Holder>,
    pub grids: Vec<Holder>,
    /// What a magazine's rounds go into, with what it takes (they lie in
    /// `Item::loose`); none for anything else.
    pub cartridges: Vec<Holder>,
    /// The equipment slots it blocks while worn.
    pub blocks: Vec<String>,
    /// How many of it one stack holds; 0 or 1 for what does not stack.
    pub stack_max: u32,
    /// A part that may not be put on or taken off in a raid.
    pub not_raid_moddable: bool,
    /// The equipment slots it is not taken out of in a raid.
    pub kept_in: Vec<String>,
    /// The colour of its view's ground in the inventory, by the game's name
    /// for it (`BackgroundColor`); empty where not read.
    pub color: String,
    /// Its own layout of its grids in the inventory, by name; empty for
    /// the default row.
    pub layout: String,
    /// What its view in the inventory writes (`GridItemView.UpdateInfo`):
    /// a medkit's whole resource (`MaxHpResource`), food's, drink's or a
    /// barter thing's (`MaxResource`), a key's uses (`MaximumNumberOfUsage`),
    /// a weapon's round (`ammoCaliber`) and how a weapon's or a knife's
    /// durability stands as made and at most (`Durability`, `MaxDurability`).
    pub max_hp_resource: f32,
    pub max_resource: f32,
    pub key_uses: u32,
    pub caliber: String,
    pub durability: [f32; 2],
    /// How many rounds a magazine or a box of rounds holds (its
    /// `Cartridges` or `StackSlots`' `_max_count`); 0 for anything else.
    pub capacity: u32,
    /// A repair kit's: the classes of what it mends (`TargetItemFilter`),
    /// and the resource it is made with (`MaxRepairResource`); empty and
    /// naught for anything else.
    pub repair_targets: Vec<String>,
    pub repair_resource: f32,
    /// A magazine's share, in per cent, added to the time a round takes to
    /// load or unload (`LoadUnloadModifier`; inventory spec, "Loading and
    /// unloading a magazine").
    pub load_unload_modifier: f32,
    /// The same added to the time it takes to check (`CheckTimeModifier`;
    /// inventory spec, "Checking a magazine").
    pub check_modifier: f32,
    /// How well a look at it in the hands tells its count at the least
    /// (`CheckOverride`, held within 0 and 2; inventory spec, "What the
    /// ammunition's panel says"): 1 on 51 magazines of the install.
    pub check_override: u32,
    /// A magazine's `magAnimationIndex`: which of a weapon's reload and look
    /// clips it plays (firing spec, "Which magazine"); 0 for anything else.
    pub magazine_animation: i32,
    /// Not shown in the slot it is in (`NotShownInSlot`: the pockets).
    pub not_shown_in_slot: bool,
    /// Its sounds' group (`ItemSound`: `weap_ar`, `gear_generic`), which
    /// names its clips (`<group>_drop` and the rest).
    pub sound: String,
    /// What the right click's menu goes by (`item_menu`): a weapon's way
    /// of being loaded (`ReloadMode`), whether it is a quest's, whether it
    /// has a switch on a hinge (`HasHinge`) or is a face shield
    /// (`FaceShieldComponent`), whether it folds or slides in (`Foldable`,
    /// `Retractable`) and the slot that must hold something for it to
    /// (`FoldedSlot`).
    pub reload_mode: String,
    pub quest: bool,
    pub hinge: bool,
    pub face_shield: bool,
    /// A face shield's mask over the first person's view
    /// (`FaceShieldMask`; `visor`).
    pub face_shield_mask: trp_data::visor::MaskKind,
    pub foldable: bool,
    pub folded_slot: String,
    /// The cells a fold takes off the right (`SizeReduceRight`), and
    /// whether, fitted to a weapon, it keeps the weapon from folding
    /// (`BlocksFolding`).
    pub fold_reduce: u32,
    pub blocks_folding: bool,
    /// For a magazine whose rounds are drawn in its model
    /// (`SpringMagazineVisual`: neither a belt's nor a stationary
    /// weapon's), its template's visible ranges (`magazine_view`); empty
    /// for anything else. Whether a picture leaves its contents out
    /// (`HideEntrails`).
    pub visible_rounds: Vec<[u32; 2]>,
    pub hide_entrails: bool,
    /// How much of a flash it keeps from the eyes (`BlindnessProtection`:
    /// a face shield, a visor), 0 to 1.
    pub blindness_protection: f32,
    /// Its armour class (`armorClass`, a number or its words), 0 for a
    /// thing of none: a plate's mark on its view goes by it (`ItemViewStats.
    /// GetArmorPlateClassIconName`).
    pub armor_class: u32,
    /// For a weapon of the install with a stock build, that build's key
    /// (`trp_data::weapon::build_key`): a weapon so built is asked for by
    /// its template alone (`rig_key`). None for anything else, and for a
    /// weapon of one's own, whose parts are never drawn.
    pub stock_key: Option<String>,
    /// The templates it cannot be on a weapon with (`ConflictingItems`;
    /// `Slot.CheckConflictingItems`).
    pub conflicting: Vec<String>,
    /// Whether it is an item the game makes, not a class of them (`_type`
    /// `Item`, not `Node`).
    pub is_item: bool,
    /// A tactical device's modes (`LightComponent`, on a `Flashlight`, a
    /// `TacticalCombo` or a `LightLaser`: its template's `ModesCount`);
    /// none for a thing that carries no light (tactical-devices spec, "The
    /// items").
    pub light_modes: Option<u32>,
}

impl Kind {
    pub fn of(db: &Database, template: &str) -> Result<Self> {
        let item = db.items.get(template).ok_or_else(|| anyhow!("the database has no item {template}"))?;
        let flag = |name: &str| item.props.get(name).and_then(|on| on.as_bool()).unwrap_or(false);
        let class_names = db.class_names(template);
        let class = |name: &str| class_names.iter().any(|class| class == name);
        Ok(Self {
            template: item.id.clone(),
            name: item.name.clone(),
            classes: db.classes(template),
            class_names: class_names.clone(),
            weight: item.prop_f32("Weight").unwrap_or(0.0),
            cells: item.cells(),
            extra: item.extra_cells(),
            forced_extra: flag("ExtraSizeForceAdd"),
            merges: item.merges_with_parts(),
            slots: item.slots(),
            grids: item.grids(),
            cartridges: item.cartridges(),
            blocks: item.blocked_slots(),
            stack_max: item.stack_max(),
            not_raid_moddable: !item.raid_moddable(),
            kept_in: item.kept_in_slots(),
            color: item.background_color().to_owned(),
            layout: item.grid_layout_name().to_owned(),
            max_hp_resource: item.prop_f32("MaxHpResource").unwrap_or(0.0),
            max_resource: item.prop_f32("MaxResource").unwrap_or(0.0),
            key_uses: item.prop_f32("MaximumNumberOfUsage").unwrap_or(0.0).max(0.0) as u32,
            caliber: item.props.get("ammoCaliber").and_then(|caliber| caliber.as_str()).unwrap_or_default().to_owned(),
            durability: [item.prop_f32("Durability").unwrap_or(0.0), item.prop_f32("MaxDurability").unwrap_or(0.0)],
            repair_targets: item
                .props
                .get("TargetItemFilter")
                .and_then(|list| list.as_array())
                .map(|list| list.iter().filter_map(|class| class.as_str().map(str::to_owned)).collect())
                .unwrap_or_default(),
            repair_resource: item.prop_f32("MaxRepairResource").unwrap_or(0.0),
            capacity: ["Cartridges", "StackSlots"]
                .iter()
                .find_map(|list| item.props.get(*list)?.get(0)?.get("_max_count")?.as_u64())
                .unwrap_or(0) as u32,
            load_unload_modifier: item.prop_f32("LoadUnloadModifier").unwrap_or(0.0),
            check_modifier: item.prop_f32("CheckTimeModifier").unwrap_or(0.0),
            check_override: item.prop_f32("CheckOverride").unwrap_or(0.0).clamp(0.0, 2.0) as u32,
            magazine_animation: item.props.get("magAnimationIndex").and_then(|index| index.as_i64()).unwrap_or(0) as i32,
            not_shown_in_slot: item.props.get("NotShownInSlot").and_then(|shown| shown.as_bool()).unwrap_or(false),
            sound: item.props.get("ItemSound").and_then(|sound| sound.as_str()).unwrap_or_default().to_owned(),
            reload_mode: item.props.get("ReloadMode").and_then(|mode| mode.as_str()).unwrap_or_default().to_owned(),
            quest: item.props.get("QuestItem").and_then(|quest| quest.as_bool()).unwrap_or(false),
            hinge: item.props.get("HasHinge").and_then(|hinge| hinge.as_bool()).unwrap_or(false),
            face_shield: item.props.get("FaceShieldComponent").and_then(|shield| shield.as_bool()).unwrap_or(false),
            face_shield_mask: item.props.get("FaceShieldMask").and_then(|mask| mask.as_str()).and_then(trp_data::visor::MaskKind::of_word).unwrap_or_default(),
            // A weapon folds or slides in by its own word, a stock only
            // by `Foldable` (`Weapon`, `Stock`: the only two that are made
            // with a `FoldableComponent`).
            foldable: if class("Weapon") { flag("Foldable") || flag("Retractable") } else { class("Stock") && flag("Foldable") },
            folded_slot: item.props.get("FoldedSlot").and_then(|slot| slot.as_str()).unwrap_or_default().to_owned(),
            fold_reduce: item.prop_f32("SizeReduceRight").unwrap_or(0.0).max(0.0) as u32,
            blocks_folding: flag("BlocksFolding"),
            visible_rounds: if class("Magazine") && !flag("MagazineWithBelt") && !flag("IsMagazineForStationaryWeapon") {
                crate::magazine_view::visible_ranges(item.props.get("VisibleAmmoRangesString").and_then(|ranges| ranges.as_str()))
            } else {
                Vec::new()
            },
            hide_entrails: flag("HideEntrails"),
            blindness_protection: item.prop_f32("BlindnessProtection").unwrap_or(0.0),
            armor_class: item.props.get("armorClass").and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok())).unwrap_or(0.0).max(0.0) as u32,
            stock_key: if class("Weapon") && !db.custom.contains_key(template) { db.default_preset(template).ok().map(trp_data::weapon::build_key) } else { None },
            conflicting: item.props.get("ConflictingItems").and_then(|list| list.as_array()).into_iter().flatten().filter_map(|id| id.as_str().map(str::to_owned)).collect(),
            is_item: item.node_type == "Item",
            light_modes: trp_data::tactical::DeviceClass::of(db, template).map(|_| trp_data::tactical::modes_of(db, template)),
        })
    }

    /// Whether a filter's list names this kind or a class of it.
    pub fn among(&self, list: &[String]) -> bool {
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
    /// The weapon in hand cannot have its parts moved now: its hands are
    /// busy (`FirearmController.CanExecute`: only while its logic idles),
    /// or would not take a round into its chamber (`Rules::chamber_closed`).
    HandsBusy,
    /// The weapon has a malfunction that keeps it (`ItemManipulator.MalfunctionError`,
    /// "You must fix malfunctions first": `Weapon.IncompatibleByMalfunction`).
    Malfunction,
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
    /// What a medkit, food or drink has left once it has been used
    /// (health's `Event::Resource`); none while it is as new.
    pub resource: Option<f32>,
    /// A switch's state, on a part that has one (`TogglableComponent.On`:
    /// a face shield down, night or thermal vision on); off as made.
    pub on: bool,
    /// A fold's state, on a weapon or a stock that folds
    /// (`FoldableComponent.Folded`); unfolded as made.
    pub folded: bool,
    /// Its tag, on a thing that takes one (`TagComponent`: a magazine, a
    /// lockable or simple container): no name, none shown.
    pub tag: Tag,
    /// A tactical device's state (`LightComponent`): off, in its first
    /// mode, as made.
    pub light: Light,
    /// Its wear (`RepairableComponent`): its durability and its greatest
    /// durability now (`Durability`, `MaxDurability`), for a thing that
    /// wears (a template's `MaxDurability` above 0: a weapon, armour, a
    /// helmet, a knife); as its template makes it.
    pub repairable: Option<Repairable>,
    /// Its malfunction, on a weapon (`Weapon.MalfState`, saved with it:
    /// `MalfunctionDescriptor`): the kind and what the shooter knows of
    /// it, the last shot's heat and its time on the raid's clock, the
    /// slide the heat holds; none, and cold, as made.
    pub malfunction: crate::malfunction::State,
    /// The round a failure to feed took out of its magazine
    /// (`MalfState.MalfunctionedAmmo`), which the repair throws out onto
    /// the ground.
    pub fed_round: Option<Arc<Kind>>,
}

/// A thing's wear as its item keeps it (`RepairableComponent`): the
/// durability now, at most the greatest durability, which repairs lower.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Repairable {
    pub durability: f32,
    pub max_durability: f32,
}

/// A tactical device's state as its item keeps it (`LightComponent`'s
/// `IsActive` and `SelectedMode`; saved with the item, no battery, no
/// timer).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Light {
    pub on: bool,
    pub mode: u32,
}

/// A thing's tag (`TagComponent`): its name and its colour's number in
/// the game's seven (`EditTagWindow.GetColor`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tag {
    pub name: String,
    pub color: u32,
}

/// Whether a thing takes a tag (`TagComponent`, made with a magazine, a
/// lockable container and a simple container).
pub fn takes_tag(item: &Item) -> bool {
    is(item, "Magazine") || is(item, "LockableContainer") || is(item, "SimpleContainer")
}

/// Sets the tag of the thing of that number wherever it lies in `root`
/// (`TagComponent.Set`; an empty name with colour 0 is no tag). Whether it
/// takes one there.
pub fn set_tag(root: &mut Item, id: Id, name: &str, color: u32) -> bool {
    match root.find_mut(id).filter(|item| takes_tag(item)) {
        Some(item) => {
            item.tag = Tag { name: name.to_owned(), color };
            true
        }
        None => false,
    }
}

impl Item {
    pub fn new(kind: Arc<Kind>) -> Self {
        let (slots, grids) = (vec![None; kind.slots.len()], vec![Vec::new(); kind.grids.len()]);
        let repairable = (kind.durability[1] > 0.0).then(|| Repairable { durability: kind.durability[0].min(kind.durability[1]), max_durability: kind.durability[1] });
        Self {
            id: next_id(),
            kind,
            count: 1,
            slots,
            grids,
            loose: Vec::new(),
            resource: None,
            on: false,
            folded: false,
            tag: Tag::default(),
            light: Light::default(),
            repairable,
            malfunction: crate::malfunction::State::default(),
            fed_round: None,
        }
    }

    /// Its durability and its greatest durability now: its own wear's where
    /// it wears (`repairable`), else its template's.
    pub fn durability(&self) -> [f32; 2] {
        self.repairable.map_or(self.kind.durability, |wear| [wear.durability, wear.max_durability])
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

    /// The extra cells its parts give it on each side, left, right, up and
    /// down, as the game sums them (`CompoundItem.CalculateExtraSize`): of
    /// every part, and every part's parts, the largest on each side, and
    /// beside that the forced ones added up. Only a thing that merges with
    /// its parts looks into its places, and then into every place but the
    /// chamber's, and a folded one not into its folded slot. A fold takes
    /// its reduction off the right: a part's own, or as a forced cell off
    /// the thing itself.
    fn extra(&self) -> [i32; 4] {
        let (mut most, mut forced) = ([0i32; 4], [0i32; 4]);
        let mut waiting = vec![(self, true)];
        while let Some((item, top)) = waiting.pop() {
            let extra = if top { [0; 4] } else { item.kind.extra.map(|cells| cells as i32) };
            let (mut own, mut own_forced) = if item.kind.forced_extra { ([0; 4], extra) } else { (extra, [0; 4]) };
            let folded = item.kind.foldable && item.folded;
            if folded {
                let reduce = item.kind.fold_reduce as i32;
                if top { own_forced[1] -= reduce } else { own[1] -= reduce }
            }
            most = std::array::from_fn(|side| most[side].max(own[side]));
            forced = std::array::from_fn(|side| forced[side] + own_forced[side]);
            if !item.kind.merges {
                continue;
            }
            let folded_slot = folded.then_some(item.kind.folded_slot.as_str()).filter(|slot| !slot.is_empty());
            for (part, slot) in item.slots.iter().zip(&item.kind.slots) {
                if let Some(part) = part.as_ref().filter(|_| merges(item, slot) && Some(slot.name.as_str()) != folded_slot) {
                    waiting.push((part, false));
                }
            }
            waiting.extend(item.grids.iter().flatten().map(|(_, part)| (part, false)));
        }
        std::array::from_fn(|side| most[side] + forced[side])
    }

    /// Width and height in cells as it lies, with its parts on (never
    /// less than a cell: the install has no thing that would be).
    pub fn cells(&self) -> [u32; 2] {
        let [left, right, up, down] = self.extra();
        let [width, height] = self.kind.cells.map(|cells| cells as i32);
        [(width + left + right).max(1) as u32, (height + up + down).max(1) as u32]
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
            item.folded = entry.folded && item.kind.foldable;
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

/// Whether a thing's place counts its contents as part of the thing
/// (`ContainerExtensions.MergesWithChildren`): a place that says so
/// itself, or else the thing's own word; the chamber of a single barrel
/// never (`SlotSerializer`: `patron_in_weapon` alone).
fn merges(holder: &Item, slot: &Holder) -> bool {
    slot.name != CHAMBER && (slot.merges == Some(true) || holder.kind.merges)
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

/// Whether two things go together, as the screen lights them (inventory
/// spec, "The box under the mouse, and what goes with it"): the one has a
/// place for the other, either way round. A weapon goes with its
/// magazines, its rounds and the parts that fit it or what is on it; a
/// magazine with the weapons it fits and its rounds. A thing does not go
/// with itself, and a grid's room is no place for this: a backpack would
/// go with everything.
pub fn go_together(one: &Item, other: &Item) -> bool {
    one.id != other.id && (has_place_for(one, &other.kind) || has_place_for(other, &one.kind))
}

/// Whether a thing, or a part on it, has a place for that kind of thing by
/// the game's filters: a slot (a chamber, a cylinder's chambers, a part's
/// slot) or a magazine's cartridges, which take what they list and do not
/// refuse.
fn has_place_for(holder: &Item, kind: &Kind) -> bool {
    let fits = |place: &Holder| kind.among(&place.takes) && !kind.among(&place.refuses);
    holder.kind.slots.iter().chain(&holder.kind.cartridges).any(fits) || holder.slots.iter().flatten().any(|part| has_place_for(part, kind))
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
    /// A weapon in hand whose hands cannot take a move of its parts now
    /// (firing spec, "A magazine taken out or put in from the inventory"):
    /// nothing is moved out of it or into it.
    pub busy_weapon: Option<Id>,
    /// The weapon in hand whose hands would not take a round into its
    /// chamber now (`Firearm::can_load_chamber`): nothing goes into its
    /// chamber.
    pub chamber_closed: Option<Id>,
    /// The weapon in hand whose hands would not let the round out of its
    /// chamber now (`Firearm::can_discharge`: a magazine in, or the logic
    /// busy): its chambered round stays.
    pub chamber_held: Option<Id>,
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
        Self { in_raid: true, backpacks_deep: named(&db.config, "MaxBackpackInserting").unwrap_or(0) as u32, busy_weapon: None, chamber_closed: None, chamber_held: None }
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

    pub fn find_mut(&mut self, id: Id) -> Option<&mut Item> {
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
    if chamber_held(roots[from], id, rules) {
        return Err(Refusal::HandsBusy);
    }
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
    // A weapon in hand whose hands are busy keeps its parts.
    if let Some(weapon) = rules.busy_weapon {
        let within = |root: &Item, holder: Id| root.find(weapon).is_some_and(|weapon| weapon.find(holder).is_some());
        if within(roots[from], holder) || within(roots[to], into) {
            return Err(Refusal::HandsBusy);
        }
    }
    if chamber_closed(target, spot, rules) {
        return Err(Refusal::HandsBusy);
    }
    // A malfunctioned weapon's slots neither take nor give up its magazine
    // or its rounds where the malfunction keeps them (`Slot.CheckCompatibility`,
    // `Slot.CanRemove`).
    if (enters.is_some() && malfunction_refuses(roots[to], into, &item.kind)) || (leaves.is_some() && malfunction_refuses(roots[from], holder, &item.kind)) {
        return Err(Refusal::Malfunction);
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
    if chamber_held(roots[from], id, rules) {
        return Err(Refusal::HandsBusy);
    }
    if let (Some((holder, Spot::Slot(_))), Some(item)) = (roots[from].holder_of(id), roots[from].find(id)) {
        if malfunction_refuses(roots[from], holder, &item.kind) {
            return Err(Refusal::Malfunction);
        }
    }
    roots[from].take(id).ok_or(Refusal::NoSuchPlace)
}

/// The weapon a holder's slots are part of (`Slot.ParentItem.GetRootItem()`):
/// the holder itself, or the weapon it is fitted to through slots that
/// merge with their item.
fn weapon_of(root: &Item, holder: Id) -> Option<&Item> {
    let mut at = holder;
    loop {
        let item = root.find(at)?;
        if is(item, "Weapon") {
            return Some(item);
        }
        let (up, Spot::Slot(slot)) = root.holder_of(at)? else { return None };
        let held_by = root.find(up)?;
        if !held_by.kind.slots.get(slot).is_some_and(|slot| merges(held_by, slot)) {
            return None;
        }
        at = up;
    }
}

/// The weapon a thing is fitted to, through slots that merge: a magazine's,
/// a chamber's round's (`GetRootItem`); none for a weapon itself.
pub fn weapon_fitted_with(root: &Item, id: Id) -> Option<&Item> {
    let (holder, spot) = root.holder_of(id)?;
    match spot {
        Spot::Slot(_) => weapon_of(root, holder),
        // A round in a magazine: the magazine's weapon.
        Spot::Loose => weapon_fitted_with(root, holder),
        Spot::Cell(..) => None,
    }
}

/// Whether a thing is drawn as unusable by a malfunction the shooter knows
/// of (`GridItemView.UnusableDueToMalfunction`): a weapon with one, its
/// magazine with a failure to feed (`IsFailedToFeedMagazine`), a round in
/// it with any (`IsFailedToFeedAmmo`).
pub fn unusable_by_malfunction(root: &Item, item: &Item) -> bool {
    let known = |weapon: &Item| weapon.malfunction.malf != crate::malfunction::Malf::None && weapon.malfunction.known;
    if is(item, "Weapon") {
        return known(item);
    }
    let Some(weapon) = weapon_fitted_with(root, item.id) else { return false };
    if is(item, "Magazine") {
        return weapon.malfunction.malf == crate::malfunction::Malf::Feed && known(weapon);
    }
    is(item, "Ammo") && known(weapon)
}

/// Whether a weapon's malfunction keeps a thing out of, or in, a slot of
/// `holder` (`Weapon.IncompatibleByMalfunction`): a magazine with a failure
/// to feed (or any malfunction on a weapon that also loads its chamber
/// alone, `ExternalMagazineWithInternalReloadSupport`), a round with any.
fn malfunction_refuses(root: &Item, holder: Id, kind: &Kind) -> bool {
    let Some(weapon) = weapon_of(root, holder) else { return false };
    let malf = weapon.malfunction.malf;
    let class = |name: &str| kind.class_names.iter().any(|class| class == name);
    if class("Magazine") {
        return malf == crate::malfunction::Malf::Feed || (malf != crate::malfunction::Malf::None && weapon.kind.reload_mode == "ExternalMagazineWithInternalReloadSupport");
    }
    class("Ammo") && malf != crate::malfunction::Malf::None
}

/// What a medkit, food or drink in here holds now, as its use left it
/// (health's `Event::Resource`). Whether it was found.
pub fn set_resource(root: &mut Item, id: Id, left: f32) -> bool {
    match root.find_mut(id) {
        Some(item) => {
            item.resource = Some(left);
            true
        }
        None => false,
    }
}

/// An item used up (health's `Event::UsedUp`): it is gone from wherever it
/// lay in here.
pub fn use_up(root: &mut Item, id: Id) -> Option<Item> {
    root.take(id)
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
    /// A weapon's magazine taken out by its menu's Unload (`UnloadWeapon`:
    /// each target by itself, no stack to merge with).
    pub const UNLOAD_WEAPON: Self = Self { wear: false, own_holder: false, one_by_one: true };
    /// A round unloaded from a magazine (`UnloadAmmo`). The game's order
    /// looks in the round's own holder first, the magazine; a round is not
    /// put back into the magazine it came out of here (`PROVISIONAL`: the
    /// game's look into it is not followed through).
    pub const UNLOAD_AMMO: Self = Self { wear: false, own_holder: false, one_by_one: true };
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
        Spot::Slot(slot) => held_by.kind.slots.get(slot).is_some_and(|slot| merges(held_by, slot)),
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
    take_rounds_of_kinds(equipment, weapon, count).iter().map(|(_, taken)| taken).sum()
}

/// The same, saying what was taken: each kind and how many of it, in the
/// order taken.
pub fn take_rounds_of_kinds(equipment: &mut Item, weapon: Id, count: u32) -> Vec<(Arc<Kind>, u32)> {
    let mut left = count;
    let mut taken_all = Vec::new();
    for id in rounds_within_reach(equipment, weapon) {
        let Some(stack) = equipment.find_mut(id) else { continue };
        let taken = stack.count.min(left);
        stack.count -= taken;
        left -= taken;
        if taken > 0 {
            taken_all.push((stack.kind.clone(), taken));
        }
        if stack.count == 0 {
            equipment.take(id);
        }
        if left == 0 {
            break;
        }
    }
    taken_all
}

/// Puts rounds onto the top of the magazine of that number wherever it
/// lies in `root`, in the order given: onto its top stack where they are
/// of its kind, else as a stack of their own. Whether it was there.
pub fn put_rounds_into(root: &mut Item, magazine: Id, rounds: &[(Arc<Kind>, u32)]) -> bool {
    let Some(magazine) = root.find_mut(magazine) else { return false };
    for (kind, count) in rounds {
        match magazine.loose.last_mut().filter(|top| top.kind.template == kind.template) {
            Some(top) => top.count += count,
            None => {
                let mut stack = Item::new(kind.clone());
                stack.count = *count;
                magazine.loose.push(stack);
            }
        }
    }
    true
}

/// A weapon's slot for its magazine.
pub const MAGAZINE_SLOT: &str = "mod_magazine";

/// The rounds a magazine holds.
pub fn magazine_rounds(magazine: &Item) -> u32 {
    magazine.loose.iter().map(|stack| stack.count).sum()
}

/// What the ammunition's panel says of a magazine looked at in the hands
/// (`AmmoCountPanel.GetAmmoCountByLevel`; inventory spec, "What the
/// ammunition's panel says").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmmoWords {
    Full,
    Empty,
    /// The count itself.
    Count(u32),
    /// "Approx." and the count to the nearest five.
    About(u32),
    FewerThanFive,
    AlmostEmpty,
    LessThanHalf,
    AboutHalf,
    AlmostFull,
}

impl AmmoWords {
    /// The game's key for its words (each looked up in the install's), and
    /// the number said after it, where one is.
    pub fn key(self) -> (&'static str, Option<u32>) {
        match self {
            Self::Full => ("Full", None),
            Self::Empty => ("Empty", None),
            Self::Count(n) => ("", Some(n)),
            Self::About(n) => ("Approx.", Some(n)),
            Self::FewerThanFive => ("Less than 5", None),
            Self::AlmostEmpty => ("Almost empty", None),
            Self::LessThanHalf => ("Less than half", None),
            Self::AboutHalf => ("About half", None),
            Self::AlmostFull => ("Almost full", None),
        }
    }
}

/// The words for `count` rounds of `max` at the look's `level` (Mag
/// Drills' level over 10, at least the magazine's `CheckOverride`): one
/// short of full or more is full, none is empty; at level 0 one of four by
/// `(int)(3.33 count / max + 0.5)`, at 1 the count where the magazine holds
/// 10 or fewer, else to the nearest five from 5 on, below that "fewer than
/// 5"; from 2 on the count. A weapon of a cylinder or several chambers
/// (`GetAmmoCountByLevelForFoldingMechanismWeapon`): full only at `max`,
/// empty at none, else the count.
pub fn ammo_words(count: u32, max: u32, level: u32, folding: bool) -> AmmoWords {
    if folding {
        return if count >= max {
            AmmoWords::Full
        } else if count == 0 {
            AmmoWords::Empty
        } else {
            AmmoWords::Count(count)
        };
    }
    if i64::from(count) >= i64::from(max) - 1 {
        return AmmoWords::Full;
    }
    if count == 0 {
        return AmmoWords::Empty;
    }
    match level {
        0 => {
            let at = (3.33f32 * count as f32 / max as f32 + 0.5) as usize;
            [AmmoWords::AlmostEmpty, AmmoWords::LessThanHalf, AmmoWords::AboutHalf, AmmoWords::AlmostFull][at.min(3)]
        }
        1 if max <= 10 => AmmoWords::Count(count),
        1 if count >= 5 => AmmoWords::About((count as f32 / 5.0 + 0.5) as u32 * 5),
        1 => AmmoWords::FewerThanFive,
        _ => AmmoWords::Count(count),
    }
}

/// Leaves a magazine holding `count` rounds, the last stack losing its
/// first (what was fired was what lay on top); more than it holds are
/// more of the last stack's kind. An empty magazine is given none.
pub fn set_magazine_rounds(magazine: &mut Item, count: u32) {
    let held = magazine_rounds(magazine);
    if count > held {
        if let Some(last) = magazine.loose.last_mut() {
            last.count += count - held;
        }
        return;
    }
    let mut over = held - count;
    while over > 0 {
        let Some(last) = magazine.loose.last_mut() else { break };
        let taken = last.count.min(over);
        (last.count, over) = (last.count - taken, over - taken);
        if last.count == 0 {
            magazine.loose.pop();
        }
    }
}

/// Whether a thing has a switch of its own (`TogglableComponent`:
/// armoured equipment on a hinge, night and thermal vision; inventory spec,
/// "A switch on the head").
pub fn has_switch(item: &Item) -> bool {
    item.kind.hinge || is(item, "NightVision") || is(item, "ThermalVision")
}

/// A thing's switch: the first of it and its parts, itself first, each
/// part before the parts fitted to it (`GetItemComponentsInChildren`).
pub fn switched_part(item: &Item) -> Option<&Item> {
    if has_switch(item) {
        return Some(item);
    }
    item.slots.iter().flatten().find_map(switched_part)
}

/// The first face shield of a thing, itself first, each part before the
/// parts fitted to it (`GetItemComponentsInChildren<FaceShieldComponent>`).
pub fn face_shield(item: &Item) -> Option<&Item> {
    with_parts(item).find(|part| part.kind.face_shield)
}

/// How long a flash blinds, as a share (`Player.BlindnessDuration`; grenade
/// spec, "A flash"): nothing with thermal vision on; else one less the
/// protection of each of a face shield on the head that is down (or has no
/// switch), a visor in the eyewear slot and a face cover's face shield
/// that is down, multiplied.
pub fn blindness_duration(equipment: &Equipment) -> f32 {
    let worn = |slot: &str| equipment.worn(slot);
    let thermal_on = WORN.iter().filter_map(|slot| worn(slot)).flat_map(with_parts).any(|part| is(part, "ThermalVision") && part.on);
    if thermal_on {
        return 0.0;
    }
    let shield_down = |shield: &Item| !has_switch(shield) || shield.on;
    let head = worn("Headwear").and_then(face_shield).filter(|shield| shield_down(shield)).map_or(1.0, |shield| 1.0 - shield.kind.blindness_protection);
    let eyes = worn("Eyewear").filter(|item| is(item, "Visors")).map_or(1.0, |visor| 1.0 - visor.kind.blindness_protection);
    let face = worn("FaceCover").and_then(face_shield).filter(|shield| shield_down(shield)).map_or(1.0, |shield| 1.0 - shield.kind.blindness_protection);
    head * eyes * face
}

/// Whether the character wears glasses (`Player.TryFindGlasses`): the
/// eyewear's thing, else the headwear's, a visor.
pub fn has_glasses(equipment: &Equipment) -> bool {
    ["Eyewear", "Headwear"].iter().filter_map(|slot| equipment.worn(slot)).any(|item| is(item, "Visors"))
}

/// What the head's slots hold (the eyewear's, the headwear's and the face
/// cover's things and all fitted to them, by number): a change in it is a
/// thing put on or taken off there (`Player.OnItemAddedOrRemoved` for
/// those slots).
pub fn head_things(equipment: &Equipment) -> Vec<Id> {
    ["Eyewear", "Headwear", "FaceCover"].iter().filter_map(|slot| equipment.worn(slot)).flat_map(with_parts).map(|item| item.id).collect()
}

/// Whether a helmet's hinge is to be down (`HelmetDress.OnFaceShieldChanged`):
/// its face shield's switch on, or a face shield with no switch, or none.
pub fn hinge_down(helmet: &Item) -> bool {
    face_shield(helmet).is_none_or(|shield| !has_switch(shield) || shield.on)
}

/// Flips the switch of the thing of that number, wherever it lies in
/// `root` (`TogglableComponent.Set`): the part switched and how it stands
/// now; none if the thing has no switch.
pub fn toggle(root: &mut Item, id: Id) -> Option<(Id, bool)> {
    let part = switched_part(root.find(id)?)?.id;
    let part = root.find_mut(part)?;
    part.on = !part.on;
    Some((part.id, part.on))
}

/// The first thing in it that folds, itself first, each part before the
/// parts fitted to it, through the places that merge
/// (`GetItemComponentsInChildren<FoldableComponent>`).
pub fn folding_part(item: &Item) -> Option<&Item> {
    if item.kind.foldable {
        return Some(item);
    }
    item.slots.iter().zip(&item.kind.slots).filter(|(_, slot)| merges(item, slot)).find_map(|(part, _)| part.as_ref().and_then(folding_part))
}

/// Whether the thing's fold can be turned at all (`FoldableComponent.
/// CanBeFolded`: its folded slot, where it names one, holds something).
pub fn can_be_folded(item: &Item) -> bool {
    folding_part(item).is_some_and(|part| {
        let slot = &part.kind.folded_slot;
        slot.is_empty() || part.kind.slots.iter().zip(&part.slots).any(|(holder, held)| holder.name == *slot && held.is_some())
    })
}

/// Whether the thing can be folded or unfolded (`ItemManipulator.
/// CanFold`): its fold can be turned, and on a weapon no part blocks it
/// (`BlocksFolding`).
pub fn can_fold(item: &Item) -> bool {
    can_be_folded(item) && (!is(item, "Weapon") || !item.slots.iter().flatten().any(|part| with_parts(part).any(|part| part.kind.blocks_folding)))
}

/// A thing and everything fitted to it.
pub(crate) fn with_parts(item: &Item) -> Box<dyn Iterator<Item = &Item> + '_> {
    Box::new(std::iter::once(item).chain(item.slots.iter().flatten().flat_map(with_parts)))
}

/// Folds or unfolds the thing of that number, wherever it lies in the roots
/// (`ItemManipulator.Fold`, `Resize_Helper`): its folding part turned the
/// other way, and from that part up each weapon or part that lies in a
/// grid made its new size where it lies, which is refused where the grid
/// has no room for it so (`Grid.Resize`: the same corner, the same turn).
/// The part folded and how it stands now; nothing changed if refused.
pub fn fold(roots: &mut [&mut Item], id: Id) -> Result<(Id, bool), Refusal> {
    let root = roots.iter_mut().find(|root| root.find(id).is_some()).ok_or(Refusal::NoSuchPlace)?;
    let thing = root.find(id).ok_or(Refusal::NoSuchPlace)?;
    if !can_fold(thing) {
        return Err(Refusal::NoAction);
    }
    let part = folding_part(thing).ok_or(Refusal::NoAction)?.id;
    let turn = |root: &mut Item| root.find_mut(part).map(|part| { part.folded = !part.folded; part.folded });
    let to = turn(root).ok_or(Refusal::NoSuchPlace)?;
    let chain: Vec<Id> = root.chain(part).iter().map(|item| item.id).collect();
    for pair in chain.windows(2).rev() {
        let (Some(holder), Some(item)) = (root.find(pair[0]), root.find(pair[1])) else { break };
        if !is(item, "Weapon") && !is(item, "Mod") {
            break;
        }
        if let Some((_, Spot::Cell(grid, place))) = holder.holder_of(item.id) {
            if holder.grid_takes(grid, item, place).is_err() {
                turn(root);
                return Err(Refusal::NoRoom);
            }
        }
    }
    Ok((part, to))
}

/// Whether a magazine takes a round of this kind (its cartridges' filter)
/// and has room for one more.
pub fn takes_round(magazine: &Item, round: &Kind) -> bool {
    let fits = magazine.kind.cartridges.first().is_some_and(|holder| round.among(&holder.takes) && !round.among(&holder.refuses));
    fits && magazine_rounds(magazine) < magazine.kind.capacity
}

/// How many rounds of a stack a drop on a magazine loads
/// (`GridView.AcceptItem`): as many as the magazine has room for, up to
/// the stack.
pub fn rounds_to_load(magazine: &Item, stack: &Item) -> u32 {
    magazine.kind.capacity.saturating_sub(magazine_rounds(magazine)).min(stack.count)
}

/// One round from a stack onto the top of a magazine's rounds
/// (`LoadMagazineProcess`, a magazine's `Apply` of one): onto its top stack
/// where that is the same round, else as a stack of its own; the stack it
/// came from gone when empty.
pub fn load_round(roots: &mut [&mut Item], source: Id, magazine: Id) -> Result<(), Refusal> {
    let kind = roots.iter().find_map(|root| root.find(source)).ok_or(Refusal::NoSuchPlace)?.kind.clone();
    // Not into a magazine fitted to a weapon with a malfunction
    // (`Magazine.Apply`'s `IncompatibleByMalfunction`).
    let fitted = roots.iter().find_map(|root| match root.holder_of(magazine) {
        Some((holder, Spot::Slot(_))) => Some(malfunction_refuses(root, holder, &kind)),
        _ => None,
    });
    if fitted == Some(true) {
        return Err(Refusal::Malfunction);
    }
    let target = roots.iter().find_map(|root| root.find(magazine)).ok_or(Refusal::NoSuchPlace)?;
    if !takes_round(target, &kind) {
        return Err(if target.kind.cartridges.is_empty() {
            Refusal::NoAction
        } else if magazine_rounds(target) >= target.kind.capacity {
            Refusal::NoRoom
        } else {
            Refusal::Filter
        });
    }
    let from = root_of(roots, source)?;
    let stack = roots[from].find_mut(source).ok_or(Refusal::NoSuchPlace)?;
    stack.count -= 1;
    if stack.count == 0 {
        roots[from].take(source);
    }
    let to = root_of(roots, magazine)?;
    let target = roots[to].find_mut(magazine).ok_or(Refusal::NoSuchPlace)?;
    match target.loose.last_mut() {
        Some(top) if top.kind.template == kind.template => top.count += 1,
        _ => target.loose.push(Item::new(kind)),
    }
    Ok(())
}

/// The top round of a magazine out (`UnloadMagazineProcess`), found a
/// place among the equipment's containers as the order `UnloadAmmo` has
/// it: onto a stack of the round with room for one, else the first free
/// cell, the vest first. Where it went; refused, it stays in the magazine.
pub fn unload_round(roots: &mut [&mut Item], magazine: Id, equipment: Id, rules: &Rules) -> Result<(Id, Applied), Refusal> {
    let at = root_of(roots, magazine)?;
    let target = roots[at].find_mut(magazine).ok_or(Refusal::NoSuchPlace)?;
    let top = target.loose.last_mut().ok_or(Refusal::NoAction)?;
    let kind = top.kind.clone();
    top.count -= 1;
    if top.count == 0 {
        target.loose.pop();
    }
    // The round in a hand of its own while a place is found for it.
    let round = Item::new(kind.clone());
    let id = round.id;
    let mut hand = Item::new(Arc::new(Kind::default()));
    hand.loose.push(round);
    let done = {
        let mut all: Vec<&mut Item> = roots.iter_mut().map(|root| &mut **root).collect();
        all.push(&mut hand);
        quick_find(&mut all, id, &[equipment], Order::UNLOAD_AMMO, rules)
    };
    if done.is_err() {
        if let Some(target) = roots[at].find_mut(magazine) {
            match target.loose.last_mut() {
                Some(top) if top.kind.template == kind.template => top.count += 1,
                _ => target.loose.push(Item::new(kind)),
            }
        }
    }
    done
}

/// Takes a weapon apart as far as it may come apart where it lies
/// (`ItemUiContext.Disassemble`): each part fitted to the weapon itself, in
/// its slots' order and with whatever is fitted to it, is found a place as
/// Ctrl and a click finds one (`QuickFindAppropriatePlace` in the order
/// asked, among `targets`); one that finds none, or may not come off here
/// (a raid's vital or fixed part), stays. Each part tried, and what came of
/// it.
pub fn disassemble(roots: &mut [&mut Item], weapon: Id, targets: &[Id], order: Order, rules: &Rules) -> Vec<(Id, Result<(Id, Applied), Refusal>)> {
    let parts: Vec<Id> = roots
        .iter()
        .find_map(|root| root.find(weapon))
        .map(|gun| gun.slots.iter().flatten().filter(|part| is(part, "Mod")).map(|part| part.id).collect())
        .unwrap_or_default();
    parts.into_iter().map(|part| (part, quick_find(roots, part, targets, order, rules))).collect()
}

/// Whether a weapon's rig, picture and model on the ground are made from
/// its own parts (the options' `ADVANCED ATTACHMENT SYSTEM`, set once as
/// the run begins); off, every weapon is its stock build.
static BUILDS: AtomicBool = AtomicBool::new(true);

/// Sets whether weapons are made from their own parts (`ADVANCED
/// ATTACHMENT SYSTEM`).
pub fn set_builds(on: bool) {
    BUILDS.store(on, Ordering::Relaxed);
}

/// Whether weapons are made from their own parts.
pub fn builds() -> bool {
    BUILDS.load(Ordering::Relaxed)
}

/// A weapon's build as a list (`trp_data::database::Preset`): itself and
/// every part fitted to it, at any depth, each with the slot of its parent
/// it is in; the rounds in its chambers and magazines are no part of it
/// (`EditBuildScreen.GetItemsInBuild`: what is a `Mod`). Items by their
/// numbers.
pub fn build_of(weapon: &Item) -> trp_data::database::Preset {
    use trp_data::database::{Preset, PresetItem};
    fn parts(item: &Item, items: &mut Vec<PresetItem>) {
        for (part, slot) in item.slots.iter().zip(&item.kind.slots) {
            let Some(part) = part.as_ref().filter(|part| is(part, "Mod")) else { continue };
            items.push(PresetItem { id: part.id.to_string(), template: part.kind.template.clone(), parent: Some(item.id.to_string()), slot: Some(slot.name.clone()) });
            parts(part, items);
        }
    }
    let mut items = vec![PresetItem { id: weapon.id.to_string(), template: weapon.kind.template.clone(), parent: None, slot: None }];
    parts(weapon, &mut items);
    Preset { id: weapon.id.to_string(), name: String::new(), root: weapon.id.to_string(), items, encyclopedia: None }
}

/// What a rig of the hands, a thing's picture or its model on the ground
/// is asked for by: a weapon of the install built otherwise than its stock
/// build, its build's key (`trp_data::weapon::build_key`); anything else,
/// and a weapon as its stock build has it, its template. With weapons
/// made from their own parts switched off (`set_builds`), a template.
pub fn rig_key(item: &Item) -> String {
    let Some(stock) = item.kind.stock_key.as_deref().filter(|_| builds() && is(item, "Weapon")) else {
        return item.kind.template.clone();
    };
    let key = trp_data::weapon::build_key(&build_of(item));
    if key == stock { item.kind.template.clone() } else { key }
}

/// The template a rig's key is of: the key itself for a template, its
/// first part for a build's.
pub fn key_template(key: &str) -> &str {
    key.split('[').next().unwrap_or(key)
}

/// The place of a weapon's chamber of its own, where it has one
/// (`patron_in_weapon`; not a cylinder's nor barrels').
pub fn chamber_slot(weapon: &Item) -> Option<usize> {
    weapon.slot_index(CHAMBER)
}

/// The round in a weapon's chamber of its own.
pub fn chambered(weapon: &Item) -> Option<&Item> {
    weapon.slots.get(chamber_slot(weapon)?)?.as_ref()
}

/// Puts a round of that kind into the chamber of the weapon of that number
/// wherever it lies in `root`, or empties it (none). Whether it has one.
pub fn set_chambered(root: &mut Item, weapon: Id, round: Option<Arc<Kind>>) -> bool {
    let Some(gun) = root.find_mut(weapon) else { return false };
    let Some(slot) = chamber_slot(gun) else { return false };
    gun.slots[slot] = round.map(Item::new);
    true
}

/// The kind of a magazine's round so many down from its top (0 the top:
/// the last stack's).
pub fn magazine_round_at(magazine: &Item, from_top: u32) -> Option<Arc<Kind>> {
    let mut left = from_top;
    for stack in magazine.loose.iter().rev() {
        if left < stack.count {
            return Some(stack.kind.clone());
        }
        left -= stack.count;
    }
    None
}

/// Whether a magazine's count is known once a round has gone in or out
/// (`CheckMagazineAmmoDepend`): left empty or full it is; otherwise it is
/// as it was, unless a shot took the round and the install unknows a
/// magazine on a shot (`UncheckOnShot`: `shot`), when it is not.
pub fn known_after(count: u32, capacity: u32, known: bool, shot: bool) -> bool {
    count == 0 || count >= capacity || (known && !shot)
}

/// Leaves the magazine of that number, wherever it lies in `root`,
/// holding `count` rounds (`set_magazine_rounds`). Whether it was there.
pub fn set_rounds_of(root: &mut Item, id: Id, count: u32) -> bool {
    match root.find_mut(id) {
        Some(magazine) => {
            set_magazine_rounds(magazine, count);
            true
        }
        None => false,
    }
}

/// Fills a magazine with `count` rounds of one kind, what it held gone.
pub fn fill_magazine(magazine: &mut Item, round: Arc<Kind>, count: u32) {
    magazine.loose.clear();
    if count > 0 {
        let mut stack = Item::new(round);
        stack.count = count;
        magazine.loose.push(stack);
    }
}

/// The magazine a reload takes for a weapon carried
/// (`FirearmHandsInputTranslator.ReloadExternalMagazine`; firing spec,
/// "The quick reload"): of the magazines lying directly in the pockets'
/// and the vest's grids, in that order, those holding a round, the
/// fullest first, and of them the first the weapon's magazine slot
/// takes. Of two that hold the same, the one met first (`PROVISIONAL`:
/// the game's sort keeps no order between them).
pub fn next_magazine(equipment: &Item, weapon: Id) -> Option<Id> {
    magazine_for(equipment, equipment.find(weapon)?)
}

/// The same for a weapon wherever it lies (the menu's Load and Reload:
/// `ItemUiContext.FindSuitableMagazine`, the magazines within reach).
pub fn magazine_for(equipment: &Item, weapon: &Item) -> Option<Id> {
    let takes = &weapon.kind.slots.get(weapon.slot_index(MAGAZINE_SLOT)?)?.takes;
    let worn = WITHIN_REACH.iter().filter_map(|slot| equipment.in_slot(slot));
    let mut found: Vec<&Item> = worn
        .flat_map(|holder| holder.grids.iter().flatten())
        .map(|(_, item)| item)
        .filter(|item| is(item, "Magazine") && magazine_rounds(item) > 0)
        .collect();
    found.sort_by_key(|magazine| std::cmp::Reverse(magazine_rounds(magazine)));
    found.into_iter().find(|magazine| magazine.kind.among(takes) && magazine.count == 1).map(|magazine| magazine.id)
}

/// Where the old magazine of a plain reload goes
/// (`GetPrioritizedGridsForUnloadedObject`, `FindLocationForItem`): the
/// first free place in the vest's grids and then the pockets', the
/// smallest grid first; the backpack is not among them. The holder, the
/// grid and the place.
fn unloaded_place(equipment: &Item, magazine: &Item) -> Option<(Id, usize, Place)> {
    unloaded_place_of(equipment, magazine, false)
}

/// The same, with the backpack's grids after the pockets' when asked
/// (`backpackIncluded`: the menu's Reload).
fn unloaded_place_of(equipment: &Item, magazine: &Item, backpack: bool) -> Option<(Id, usize, Place)> {
    let slots: &[&str] = if backpack { &["TacticalVest", "Pockets", "Backpack"] } else { &["TacticalVest", "Pockets"] };
    let holders = slots.iter().filter_map(|slot| equipment.in_slot(slot));
    let mut grids: Vec<(&Item, usize)> = holders.flat_map(|holder| (0..holder.grids.len()).map(move |grid| (holder, grid))).collect();
    grids.sort_by_key(|(holder, grid)| holder.kind.grids[*grid].cells[0] * holder.kind.grids[*grid].cells[1]);
    grids.into_iter().find_map(|(holder, grid)| holder.find_room(grid, magazine).map(|place| (holder.id, grid, place)))
}

/// Puts what a weapon gives up where the game puts it
/// (`placeToPutContainedAmmoMagazine`): an old magazine, or the live round
/// out of a single barrel, to the first free place in the vest's and the
/// pockets' grids, the smallest grid first. Hands it back where there is
/// none, to be thrown down.
pub fn put_unloaded(equipment: &mut Item, item: Item) -> Option<Item> {
    let Some((holder, grid, place)) = unloaded_place(equipment, &item) else { return Some(item) };
    match equipment.find_mut(holder) {
        Some(holder) => {
            holder.grids[grid].push((place, item));
            None
        }
        None => Some(item),
    }
}

/// The exchange a magazine's reload makes as it begins
/// (`ReloadExternalMagResult.Run`): the magazine fitted leaves the weapon
/// holding `left` rounds and is put where a plain reload puts it, unless
/// the reload is the quick one or there is no such place; then `next`
/// goes into the weapon's magazine slot. Returns the old magazine where
/// it has nowhere to go: it is to be let fall at the mark `MagHide`.
pub fn swap_magazine(equipment: &mut Item, weapon: Id, next: Option<Id>, left: Option<u32>, quick: bool) -> Option<Item> {
    let slot = equipment.find(weapon)?.slot_index(MAGAZINE_SLOT)?;
    let mut old = equipment.find_mut(weapon)?.slots[slot].take();
    if let (Some(old), Some(left)) = (old.as_mut(), left) {
        set_magazine_rounds(old, left);
    }
    let falls = match old {
        Some(old) if quick => Some(old),
        Some(old) => put_unloaded(equipment, old),
        None => None,
    };
    if let Some(next) = next.and_then(|id| equipment.take(id)) {
        if let Some(weapon) = equipment.find_mut(weapon) {
            weapon.slots[slot] = Some(next);
        }
    }
    falls
}

/// What the menu's Reload on a weapon comes to (`ItemUiContext.
/// ReloadWeapon`; firing spec, "A magazine taken out or put in from the
/// inventory").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuReload {
    /// The fitted magazine (the first) swapped for the found one (the
    /// second) at once: two things moved, the hands not asked.
    Swapped(Id, Id),
    /// No magazine fitted: the found one is to be moved into the slot (a
    /// move of one thing, the hands' if the weapon is in hand).
    Fit(Id),
}

/// The menu's Reload on a weapon lying in `roots`: the fullest magazine
/// within reach of the character (`equipment`) that the weapon's slot
/// takes; with one fitted, the two swapped, the old one to the first free
/// place of the smallest grid of the vest, the pockets and the backpack
/// that has one. Refused with no magazine to take (`NoAction`), and with
/// nowhere for the old one (`NoRoom`: the game then throws it down, which
/// is not built).
pub fn menu_reload(equipment: &mut Item, weapon: Id) -> Result<MenuReload, Refusal> {
    let gun = equipment.find(weapon).ok_or(Refusal::NoSuchPlace)?;
    let next = magazine_for(equipment, gun).ok_or(Refusal::NoAction)?;
    let slot = gun.slot_index(MAGAZINE_SLOT).ok_or(Refusal::NoSuchPlace)?;
    let Some(old) = gun.slots[slot].as_ref() else { return Ok(MenuReload::Fit(next)) };
    let old_id = old.id;
    // The place is found with the old one still in the weapon, as the game
    // finds it.
    let (holder, grid, place) = unloaded_place_of(equipment, old, true).ok_or(Refusal::NoRoom)?;
    let new = equipment.take(next).ok_or(Refusal::NoSuchPlace)?;
    let old = equipment.find_mut(weapon).ok_or(Refusal::NoSuchPlace)?.slots[slot].replace(new).ok_or(Refusal::NoSuchPlace)?;
    equipment.find_mut(holder).ok_or(Refusal::NoSuchPlace)?.grids[grid].push((place, old));
    Ok(MenuReload::Swapped(old_id, next))
}

/// What the menu's Install on a magazine comes to (`ItemUiContext.
/// InstallMod`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuInstall {
    /// Swapped at once for the magazine the weapon had (the hands not
    /// asked): the weapon, and the old magazine.
    Swapped(Id, Id),
    /// To be moved into the weapon's empty slot (the hands' if the weapon
    /// is in hand): the weapon and the slot.
    Fit(Id, usize),
}

/// The menu's Install on a magazine the character has: into the magazine
/// slot of the weapon in hand if it takes the magazine, else of the first
/// weapon carried whose slot does (`GetSuitableWeapons`); a slot holding
/// another has the two swapped, the old one to the first free place for it
/// in the equipment's containers in the order for loot (`QuickFind`,
/// `GetPrioritizedGridsForLoot`). Refused with no such weapon
/// (`NoAction`) or nowhere for the old one (`NoRoom`).
pub fn menu_install(equipment: &mut Item, magazine: Id, in_hand: Option<Id>) -> Result<MenuInstall, Refusal> {
    let item = equipment.find(magazine).ok_or(Refusal::NoSuchPlace)?;
    let accepts = |weapon: &Item| {
        let slot = weapon.slot_index(MAGAZINE_SLOT)?;
        (is(weapon, "Weapon") && item.kind.among(&weapon.kind.slots[slot].takes)).then_some((weapon.id, slot))
    };
    fn all(item: &Item) -> Vec<&Item> {
        item.contents().flat_map(|inside| std::iter::once(inside).chain(all(inside))).collect()
    }
    let found = in_hand.and_then(|id| equipment.find(id)).and_then(|weapon| accepts(weapon)).or_else(|| all(equipment).into_iter().find_map(|weapon| accepts(weapon)));
    let (weapon, slot) = found.ok_or(Refusal::NoAction)?;
    let gun = equipment.find(weapon).ok_or(Refusal::NoSuchPlace)?;
    let Some(old) = gun.slots[slot].as_ref() else { return Ok(MenuInstall::Fit(weapon, slot)) };
    if old.id == magazine {
        return Err(Refusal::AlreadyThere);
    }
    let old_id = old.id;
    let (holder, grid, place) = loot_place(equipment, old).ok_or(Refusal::NoRoom)?;
    let new = equipment.take(magazine).ok_or(Refusal::NoSuchPlace)?;
    let old = equipment.find_mut(weapon).ok_or(Refusal::NoSuchPlace)?.slots[slot].replace(new).ok_or(Refusal::NoSuchPlace)?;
    equipment.find_mut(holder).ok_or(Refusal::NoSuchPlace)?.grids[grid].push((place, old));
    Ok(MenuInstall::Swapped(weapon, old_id))
}

/// The first free place for a thing in the equipment's four containers
/// in the order for its kind (`GetPrioritizedGridsForLoot`,
/// `FindLocationForItem`): the holder, the grid and the place.
fn loot_place(equipment: &Item, item: &Item) -> Option<(Id, usize, Place)> {
    loot_order(item).iter().filter_map(|slot| equipment.in_slot(slot)).find_map(|holder| (0..holder.grids.len()).find_map(|grid| holder.find_room(grid, item).map(|place| (holder.id, grid, place))))
}

/// The slots a weapon is taken in hand from, in the order the game looks
/// through them for something to hold (`Player._slotPriority`).
pub const HAND_SLOTS: [&str; 4] = ["FirstPrimaryWeapon", "SecondPrimaryWeapon", "Holster", "Scabbard"];

/// Whether a thing is the round in the chamber of the weapon in hand whose
/// hands would not let it out now (`Rules::chamber_held`; `Idling.Execute`
/// refuses the move: "Can't perform operation while mag in the weapon").
fn chamber_held(root: &Item, id: Id, rules: &Rules) -> bool {
    let Some(weapon) = rules.chamber_held else { return false };
    let Some((holder, Spot::Slot(slot))) = root.holder_of(id) else { return false };
    holder == weapon && root.find(holder).and_then(|gun| gun.kind.slots.get(slot)).is_some_and(|slot| is_chamber(&slot.name))
}

/// Whether a place is the chamber of the weapon in hand whose hands would
/// not take a round into it now (`Rules::chamber_closed`).
fn chamber_closed(target: &Item, spot: Spot, rules: &Rules) -> bool {
    rules.chamber_closed == Some(target.id) && matches!(spot, Spot::Slot(slot) if target.kind.slots.get(slot).is_some_and(|holder| is_chamber(&holder.name)))
}

/// Drops one item on another (`ItemController.ExecutePossibleAction`,
/// `CompoundItem.Apply`): two stacks of one kind become one if the other
/// has the room; else it goes into the first of the other's slots that
/// takes it, a weapon's chambers last (`Weapon.AllSlots`) and a round of a
/// stack split off for a chamber (`SplitMax`), else into the first of its
/// grids with room; else as much of a stack as the other holds goes over.
/// Swapping two things by dropping one on the other is switched off in the
/// game.
pub fn apply_item(roots: &mut [&mut Item], id: Id, onto: Id, rules: &Rules) -> Result<Applied, Refusal> {
    if id == onto {
        return Err(Refusal::NoAction);
    }
    let (from, to) = (root_of(roots, id)?, root_of(roots, onto)?);
    if chamber_held(roots[from], id, rules) {
        return Err(Refusal::HandsBusy);
    }
    let (item, target) = (roots[from].find(id).ok_or(Refusal::NoSuchPlace)?, roots[to].find(onto).ok_or(Refusal::NoSuchPlace)?);
    let alike = item.kind.stack_max > 1 && item.kind.template == target.kind.template;
    let (count, room) = (item.count, target.stack_room());
    if alike && room >= count {
        roots[from].take(id);
        roots[to].find_mut(onto).ok_or(Refusal::NoSuchPlace)?.count += count;
        return Ok(Applied::Merged);
    }
    let slots: Vec<usize> = (0..target.kind.slots.len())
        .filter(|slot| !is_chamber(&target.kind.slots[*slot].name) && target.slot_takes(*slot, item).is_ok())
        .collect();
    // A chamber takes one round: of a stack, one is split off for it.
    let one = Item { count: 1, ..item.clone() };
    let chambers: Vec<usize> = (0..target.kind.slots.len())
        .filter(|slot| is_chamber(&target.kind.slots[*slot].name) && target.slot_takes(*slot, &one).is_ok())
        .collect();
    let closed = chambers.iter().any(|slot| chamber_closed(target, Spot::Slot(*slot), rules));
    let many = count > 1;
    let cells = (0..target.kind.grids.len()).find_map(|grid| target.find_room(grid, item).map(|place| Spot::Cell(grid, place)));
    let mut refused = Refusal::NoAction;
    for spot in slots.into_iter().map(Spot::Slot).chain(chambers.into_iter().map(Spot::Slot)).chain(cells) {
        let chamber = matches!(spot, Spot::Slot(slot) if is_chamber(&roots[to].find(onto).map_or(String::new(), |target| target.kind.slots[slot].name.clone())));
        let done = if chamber && closed {
            Err(Refusal::HandsBusy)
        } else if chamber && many {
            split_item(roots, id, 1, onto, spot, rules).map(|_| ())
        } else {
            move_item(roots, id, onto, spot, rules)
        };
        match done {
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

    /// A weapon taken apart in a raid: each of its own parts that may come
    /// off goes where Ctrl and a click sends it, with what is fitted to it;
    /// a vital part stays, and so does the round in the chamber.
    #[test]
    fn a_weapon_comes_apart_but_for_its_vital_parts() {
        let part_of = |name: &str| {
            let mut kind = kind(name, "mod", 0.2, [1, 1]);
            kind.class_names.push("Mod".into());
            kind.slots = vec![holder("mod_mount", [0, 0], &["mount"], &[])];
            Item::new(Arc::new(kind))
        };
        let slots = vec![
            Holder { required: true, ..holder("mod_barrel", [0, 0], &["barrel"], &[]) },
            holder("mod_stock", [0, 0], &["stock"], &[]),
            holder("patron_in_weapon", [0, 0], &["round"], &[]),
        ];
        let mut rifle = Item::new(Arc::new(Kind { slots, merges: true, ..kind("rifle", "weapon", 3.0, [3, 1]) }));
        let mut stock = part_of("stock");
        stock.slots[0] = Some(part_of("pad"));
        let (stock_id, pad_id) = (stock.id, stock.slots[0].as_ref().unwrap().id);
        rifle.slots[0] = Some(part_of("barrel"));
        rifle.slots[1] = Some(stock);
        rifle.slots[2] = Some(thing("round", "ammo", 0.01, [1, 1]));
        let rifle_id = rifle.id;
        let mut crate_ = pack("crate", [6, 6]);
        crate_.stow(rifle).unwrap();
        let mut bag = pack("bag", [4, 4]);
        let bag_id = bag.id;
        let mut roots = [&mut crate_, &mut bag];
        let came = disassemble(&mut roots, rifle_id, &[bag_id], Order::OTHER_SIDE, &RAID);
        assert_eq!(came.len(), 2);
        assert!(matches!(came[0].1, Err(Refusal::Vital(_))));
        assert!(matches!(came[1], (id, Ok((holder, Applied::Put(_)))) if id == stock_id && holder == bag_id));
        assert!(roots[1].find(pad_id).is_some());
        let rifle = roots[0].find(rifle_id).unwrap();
        assert!(rifle.slots[0].is_some() && rifle.slots[1].is_none() && rifle.slots[2].is_some());
    }

    /// The rounds a magazine holds are counted from its top, the last
    /// stack's first; a weapon's chamber is filled and emptied.
    #[test]
    fn a_magazine_s_rounds_from_the_top_and_a_chamber_filled() {
        let round = |name: &str, count: u32| Item { count, ..Item::new(Arc::new(Kind { stack_max: 60, ..kind(name, "ammo", 0.01, [1, 1]) })) };
        let mut magazine = thing("magazine", "magazine", 0.1, [1, 2]);
        magazine.loose = vec![round("bottom", 20), round("top", 2)];
        let named = |at: u32| magazine_round_at(&magazine, at).map(|kind| kind.name.clone());
        assert_eq!((named(0), named(1), named(2), named(22)), (Some("top".into()), Some("top".into()), Some("bottom".into()), None));
        let slots = vec![holder("patron_in_weapon", [0, 0], &["ammo"], &[])];
        let mut rifle = Item::new(Arc::new(Kind { slots, ..kind("rifle", "weapon", 3.0, [4, 1]) }));
        let rifle_id = rifle.id;
        let kind = magazine_round_at(&magazine, 0);
        assert!(set_chambered(&mut rifle, rifle_id, kind) && chambered(&rifle).is_some_and(|round| round.count == 1));
        assert!(set_chambered(&mut rifle, rifle_id, None) && chambered(&rifle).is_none());
    }

    /// A round let go on a weapon goes into its empty chamber, one split
    /// off a stack; not into a full one, nor into the chamber of the weapon
    /// in hand whose hands would not take it.
    #[test]
    fn a_round_let_go_on_a_weapon_goes_into_its_chamber() {
        let slots = vec![holder("mod_magazine", [0, 0], &["magazine"], &[]), holder("patron_in_weapon", [0, 0], &["round"], &[])];
        let rifle = Item::new(Arc::new(Kind { slots, merges: true, ..kind("rifle", "weapon", 3.0, [4, 1]) }));
        let rifle_id = rifle.id;
        let mut rounds = Item::new(Arc::new(Kind { stack_max: 60, ..kind("round", "round", 0.01, [1, 1]) }));
        rounds.count = 20;
        let rounds_id = rounds.id;
        let mut bag = pack("bag", [6, 4]);
        bag.stow(rifle).unwrap();
        bag.stow(rounds).unwrap();
        let mut roots = [&mut bag];
        let closed = Rules { chamber_closed: Some(rifle_id), ..RAID };
        assert_eq!(apply_item(&mut roots, rounds_id, rifle_id, &closed), Err(Refusal::HandsBusy));
        assert_eq!(apply_item(&mut roots, rounds_id, rifle_id, &RAID), Ok(Applied::Put(Spot::Slot(1))));
        let rifle = roots[0].find(rifle_id).unwrap();
        assert_eq!(rifle.slots[1].as_ref().map(|round| round.count), Some(1));
        assert_eq!(roots[0].find(rounds_id).map(|stack| stack.count), Some(19));
        assert!(apply_item(&mut roots, rounds_id, rifle_id, &RAID).is_err(), "the chamber is full");
        // The round in it stays where the hands would not let it out: not
        // moved, merged or thrown out.
        let chambered_id = roots[0].find(rifle_id).and_then(|rifle| rifle.slots[1].as_ref()).map(|round| round.id).unwrap();
        let held = Rules { chamber_held: Some(rifle_id), ..RAID };
        let bag_id = roots[0].id;
        let cell = Spot::Cell(0, roots[0].find_room(0, roots[0].find(chambered_id).unwrap()).unwrap());
        assert_eq!(move_item(&mut roots, chambered_id, bag_id, cell, &held), Err(Refusal::HandsBusy));
        assert_eq!(apply_item(&mut roots, chambered_id, rounds_id, &held), Err(Refusal::HandsBusy));
        assert_eq!(take_out(&mut roots, chambered_id, &held).err(), Some(Refusal::HandsBusy));
        assert_eq!(apply_item(&mut roots, chambered_id, rounds_id, &RAID), Ok(Applied::Merged));
        assert!(roots[0].find(rifle_id).unwrap().slots[1].is_none());
    }

    /// A malfunctioned weapon keeps its magazine with a failure to feed and
    /// takes no round into its chamber with any (`IncompatibleByMalfunction`);
    /// it and such a magazine are drawn unusable once the shooter knows.
    #[test]
    fn a_malfunctioned_weapon_keeps_its_magazine_and_its_chamber() {
        use crate::malfunction::Malf;
        let slots = vec![holder("mod_magazine", [0, 0], &["magazine"], &[]), holder("patron_in_weapon", [0, 0], &["ammo"], &[])];
        let mut rifle = Item::new(Arc::new(Kind { slots, merges: true, ..kind("rifle", "weapon", 3.0, [4, 1]) }));
        let magazine = thing("magazine", "magazine", 0.1, [1, 2]);
        let magazine_id = magazine.id;
        rifle.slots[0] = Some(magazine);
        let rifle_id = rifle.id;
        let round = thing("round", "ammo", 0.01, [1, 1]);
        let round_id = round.id;
        let mut bag = pack("bag", [6, 4]);
        bag.stow(rifle).unwrap();
        bag.stow(round).unwrap();
        let bag_id = bag.id;
        let mut roots = [&mut bag];
        let set = |roots: &mut [&mut Item], malf: Malf, known: bool| {
            let rifle = roots[0].find_mut(rifle_id).unwrap();
            (rifle.malfunction.malf, rifle.malfunction.known) = (malf, known);
        };
        // A failure to feed: the magazine stays, and no round goes in.
        set(&mut roots, Malf::Feed, false);
        let out = Spot::Cell(0, roots[0].find_room(0, roots[0].find(magazine_id).unwrap()).unwrap());
        assert_eq!(move_item(&mut roots, magazine_id, bag_id, out, &RAID), Err(Refusal::Malfunction));
        assert_eq!(move_item(&mut roots, round_id, rifle_id, Spot::Slot(1), &RAID), Err(Refusal::Malfunction));
        assert!(!unusable_by_malfunction(roots[0], roots[0].find(rifle_id).unwrap()), "not known: drawn as it is");
        set(&mut roots, Malf::Feed, true);
        assert!(unusable_by_malfunction(roots[0], roots[0].find(rifle_id).unwrap()));
        assert!(unusable_by_malfunction(roots[0], roots[0].find(magazine_id).unwrap()), "its magazine with a failure to feed");
        // A jam: the magazine may leave, still no round in.
        set(&mut roots, Malf::Jam, true);
        assert!(!unusable_by_malfunction(roots[0], roots[0].find(magazine_id).unwrap()));
        assert_eq!(move_item(&mut roots, round_id, rifle_id, Spot::Slot(1), &RAID), Err(Refusal::Malfunction));
        assert_eq!(move_item(&mut roots, magazine_id, bag_id, out, &RAID), Ok(()));
        // Put right, the round goes in.
        set(&mut roots, Malf::None, false);
        assert_eq!(move_item(&mut roots, round_id, rifle_id, Spot::Slot(1), &RAID), Ok(()));
    }

    /// A magazine is known once a round in or out leaves it empty or
    /// full; a shot that leaves it neither unknows it, anything else
    /// leaves it as it was.
    #[test]
    fn a_magazine_is_known_empty_or_full_and_unknown_after_a_shot() {
        assert!(known_after(0, 30, false, true) && known_after(30, 30, false, false));
        assert!(!known_after(29, 30, true, true) && known_after(29, 30, true, false));
        assert!(!known_after(12, 30, false, false));
    }

    /// Forced extra cells add up beside the largest of the rest
    /// (`ExtraSizeForceAdd`); a thing that does not merge with its parts
    /// counts none of them, whatever its places say of themselves; the
    /// chamber's round never counts.
    #[test]
    fn forced_cells_add_up_and_only_a_merging_thing_counts_its_parts() {
        let part = |name: &str, extra: [u32; 4], forced_extra: bool| Item::new(Arc::new(Kind { extra, forced_extra, ..kind(name, "mod", 0.1, [1, 1]) }));
        let slots = vec![holder("mod_handguard", [0, 0], &["mod"], &[]), holder("mod_muzzle", [0, 0], &["mod"], &[]), holder("patron_in_weapon", [0, 0], &["mod"], &[])];
        let mut rifle = Item::new(Arc::new(Kind { slots: slots.clone(), merges: true, ..kind("rifle", "weapon", 2.0, [2, 1]) }));
        rifle.slots[0] = Some(part("handguard", [1, 0, 0, 0], false));
        rifle.slots[1] = Some(part("silencer", [1, 0, 0, 0], true));
        assert_eq!(rifle.cells(), [4, 1]);
        rifle.slots[2] = Some(part("round", [3, 0, 0, 0], false));
        assert_eq!(rifle.cells(), [4, 1]);
        let mut merging_slots = slots;
        merging_slots[0].merges = Some(true);
        let mut armour = Item::new(Arc::new(Kind { slots: merging_slots, ..kind("armour", "armoredEquipment", 2.0, [3, 3]) }));
        armour.slots[0] = Some(part("plate", [1, 1, 1, 1], false));
        assert_eq!(armour.cells(), [3, 3]);
    }

    /// A stock folded takes its reduction off the right, of the weapon it
    /// is on or of itself alone; a weapon that folds leaves its folded slot
    /// out. In a grid the weapon is made its new size where it lies, and a
    /// fold with no room for that is refused with nothing changed; a part
    /// that blocks folding keeps the weapon from it.
    #[test]
    fn a_fold_shortens_the_weapon_where_it_lies() {
        // A stock is a part (`Stock` under `Mod`), which the fold's walk up
        // goes by.
        let part_of = |mut kind: Kind| {
            kind.class_names.push("Mod".into());
            kind
        };
        let stock_kind = |reduce: u32| Arc::new(Kind { extra: [0, 1, 0, 0], foldable: true, fold_reduce: reduce, ..part_of(kind("stock", "stock", 0.9, [2, 2])) });
        let slots = vec![holder("mod_stock", [0, 0], &["stock"], &[]), holder("mod_barrel", [0, 0], &["mod"], &[])];
        let mut shotgun = Item::new(Arc::new(Kind { slots: slots.clone(), merges: true, ..kind("shotgun", "weapon", 3.0, [4, 2]) }));
        shotgun.slots[0] = Some(Item::new(stock_kind(1)));
        assert_eq!(shotgun.cells(), [5, 2]);
        let mut pack = Item::new(Arc::new(Kind { grids: vec![holder("main", [5, 4], &[], &[])], ..kind("pack", "backpack", 1.0, [5, 5]) }));
        pack.put(0, shotgun.clone(), Place { x: 0, y: 0, turned: false }).unwrap();
        let (shotgun_id, stock_id) = (shotgun.id, shotgun.slots[0].as_ref().unwrap().id);
        assert_eq!(fold(&mut [&mut pack], shotgun_id), Ok((stock_id, true)));
        let folded = &pack.grids[0][0].1;
        assert_eq!((folded.cells(), folded.slots[0].as_ref().unwrap().cells()), ([4, 2], [1, 2]));
        // Something now lies where the stock would come out to: no room.
        pack.put(0, thing("box", "item", 0.1, [1, 1]), Place { x: 4, y: 1, turned: false }).unwrap();
        assert_eq!(fold(&mut [&mut pack], shotgun_id), Err(Refusal::NoRoom));
        assert!(pack.find(stock_id).unwrap().folded && pack.find(shotgun_id).unwrap().cells() == [4, 2]);
        pack.grids[0].retain(|(_, item)| item.id == shotgun_id);
        assert_eq!(fold(&mut [&mut pack], stock_id), Ok((stock_id, false)));
        assert_eq!(pack.find(shotgun_id).unwrap().cells(), [5, 2]);
        // A weapon that folds itself, its stock in its folded slot.
        let mut folding = Item::new(Arc::new(Kind { slots: slots.clone(), merges: true, foldable: true, folded_slot: "mod_stock".into(), ..kind("smg", "weapon", 2.0, [3, 2]) }));
        folding.slots[0] = Some(Item::new(Arc::new(Kind { extra: [0, 2, 0, 0], ..kind("fixed", "stock", 0.5, [2, 2]) })));
        assert_eq!(folding.cells(), [5, 2]);
        let folding_id = folding.id;
        assert_eq!(fold(&mut [&mut folding], folding_id), Ok((folding_id, true)));
        assert_eq!(folding.cells(), [3, 2]);
        // Its folded slot empty, it has nothing to fold.
        folding.slots[0] = None;
        assert!(!can_be_folded(&folding) && fold(&mut [&mut folding], folding_id) == Err(Refusal::NoAction));
        // A part that blocks folding.
        let mut blocked = Item::new(Arc::new(Kind { slots, merges: true, ..kind("shotgun", "weapon", 3.0, [4, 2]) }));
        blocked.slots[0] = Some(Item::new(stock_kind(1)));
        blocked.slots[1] = Some(Item::new(Arc::new(Kind { blocks_folding: true, ..kind("barrel", "mod", 0.5, [2, 1]) })));
        assert!(can_be_folded(&blocked) && !can_fold(&blocked));
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

    const RAID: Rules = Rules { in_raid: true, backpacks_deep: 2, busy_weapon: None, chamber_closed: None, chamber_held: None };

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
        // What is taken, by kind, goes onto a tube's top.
        let mut tube = thing("tube", "magazine", 0.1, [1, 1]);
        let tube_id = tube.id;
        let shell = Arc::new(kind("shell", "ammo", 0.04, [1, 1]));
        assert!(put_rounds_into(&mut tube, tube_id, &[(shell.clone(), 2), (shell, 1)]));
        assert_eq!((tube.loose.len(), magazine_rounds(&tube)), (1, 3));
        assert_eq!(rounds_in_reach(&worn, gun_id), 0);
    }

    /// A rifle on the character, its magazine fitted with `fitted` rounds;
    /// and a magazine of the kind it takes holding so many.
    fn rifle_worn(worn: &mut Item, fitted: u32) -> (Id, Id) {
        let rifle = Item::new(Arc::new(Kind { slots: vec![holder(MAGAZINE_SLOT, [0, 0], &["mag"], &[])], ..kind("rifle", "weapon", 3.0, [4, 1]) }));
        let (rifle_id, fitted) = (rifle.id, loaded("mag", fitted));
        let fitted_id = fitted.id;
        worn.slots[0] = Some(rifle);
        worn.find_mut(rifle_id).unwrap().slots[0] = Some(fitted);
        (rifle_id, fitted_id)
    }

    fn loaded(name: &str, rounds: u32) -> Item {
        let mut magazine = thing(name, "magazine", 0.2, [1, 2]);
        fill_magazine(&mut magazine, Arc::new(kind("round", "ammo", 0.01, [1, 1])), rounds);
        magazine
    }

    /// The reload's magazine is the fullest one directly in the pockets
    /// or the vest that the weapon takes, with a round in it; the
    /// backpack's, an empty one and one of another kind are passed over.
    #[test]
    fn a_reload_takes_the_fullest_magazine_within_reach() {
        let (mut worn, [vest_id, _, bag_id], _) = dressed();
        let (rifle_id, _) = rifle_worn(&mut worn, 3);
        let (ten, twenty, empty, in_bag, other) = (loaded("mag", 10), loaded("mag", 20), loaded("mag", 0), loaded("mag", 30), loaded("drum", 30));
        let (ten_id, twenty_id) = (ten.id, twenty.id);
        for (holder, item) in [(vest_id, ten), (vest_id, empty), (bag_id, in_bag), (vest_id, other)] {
            worn.find_mut(holder).unwrap().stow(item).unwrap();
        }
        assert_eq!(next_magazine(&worn, rifle_id), Some(ten_id));
        worn.find_mut(vest_id).unwrap().grids[2].push((Place { x: 1, y: 0, turned: false }, twenty));
        assert_eq!(next_magazine(&worn, rifle_id), Some(twenty_id));
        // With nothing within reach that it takes, none.
        let (mut bare, _, _) = dressed();
        let (rifle_id, _) = rifle_worn(&mut bare, 3);
        assert_eq!(next_magazine(&bare, rifle_id), None);
    }

    /// The plain reload's old magazine goes to the smallest grid of the
    /// vest and the pockets with room, keeping what is left in it; the
    /// quick one's, or one that finds no room, is handed back to be let
    /// fall. The new one is in the weapon either way.
    #[test]
    fn the_old_magazine_is_put_away_or_let_fall() {
        let (mut worn, [vest_id, pockets_id, _], _) = dressed();
        let (rifle_id, fitted_id) = rifle_worn(&mut worn, 30);
        let next = loaded("mag", 30);
        let next_id = next.id;
        worn.find_mut(vest_id).unwrap().put(0, next, Place { x: 0, y: 0, turned: false }).unwrap();
        // The pockets' grids are one cell each: the magazine (one by two)
        // goes to the vest's second grid, the smallest with room.
        assert_eq!(swap_magazine(&mut worn, rifle_id, Some(next_id), Some(12), false), None);
        assert_eq!(worn.holder_of(fitted_id), Some((vest_id, Spot::Cell(1, Place { x: 0, y: 0, turned: false }))));
        assert_eq!(magazine_rounds(worn.find(fitted_id).unwrap()), 12);
        assert_eq!(worn.holder_of(next_id), Some((rifle_id, Spot::Slot(0))));
        assert!(worn.find(pockets_id).unwrap().grids.iter().all(Vec::is_empty));
        // The quick way it is handed back with its rounds, though there is room.
        let back = swap_magazine(&mut worn, rifle_id, Some(fitted_id), Some(4), true).expect("let fall");
        assert_eq!((back.id, magazine_rounds(&back)), (next_id, 4));
        assert_eq!(worn.holder_of(fitted_id), Some((rifle_id, Spot::Slot(0))));
        // With no room it falls from a plain reload too.
        let (mut full, [vest_id, _, _], _) = dressed();
        let (rifle_id, fitted_id) = rifle_worn(&mut full, 5);
        for grid in 0..3 {
            full.find_mut(vest_id).unwrap().grids[grid].push((Place { x: 0, y: 0, turned: false }, thing("brick", "item", 1.0, [2, 2])));
        }
        assert_eq!(swap_magazine(&mut full, rifle_id, None, Some(5), false).map(|old| old.id), Some(fitted_id));
    }

    /// A magazine is loaded a round at a time from a stack, onto its top
    /// stack where the round is the same, as many as it has room for; it
    /// refuses a round its filter does not take, and one more when full. It
    /// is unloaded from the top, each round onto a stack of its kind with
    /// room in the vest, else into a free cell; a round with nowhere to go
    /// stays in.
    #[test]
    fn a_magazine_is_loaded_and_unloaded_a_round_at_a_time() {
        let (mut worn, [vest_id, _, _], _) = dressed();
        let worn_id = worn.id;
        let round = Arc::new(Kind { stack_max: 60, ..kind("round", "ammo", 0.01, [1, 1]) });
        let slug = Arc::new(Kind { stack_max: 60, classes: vec!["slug".into(), "round".into(), "ammo".into(), "item".into()], ..kind("slug", "ammo", 0.03, [1, 1]) });
        let magazine = Item::new(Arc::new(Kind { cartridges: vec![holder("cartridges", [0, 0], &["round"], &[])], capacity: 3, ..kind("mag", "magazine", 0.2, [1, 2]) }));
        let (mut stack, mut slugs, shells) = (Item::new(round.clone()), Item::new(slug), thing("shell", "ammo", 0.04, [1, 1]));
        (stack.count, slugs.count) = (2, 5);
        let (mag_id, stack_id, slugs_id, shells_id) = (magazine.id, stack.id, slugs.id, shells.id);
        let vest = worn.find_mut(vest_id).unwrap();
        vest.put(0, magazine, Place { x: 0, y: 0, turned: false }).unwrap();
        vest.put(2, stack, Place { x: 0, y: 0, turned: false }).unwrap();
        vest.put(2, slugs, Place { x: 1, y: 0, turned: false }).unwrap();
        vest.put(2, shells, Place { x: 0, y: 1, turned: false }).unwrap();
        assert_eq!(rounds_to_load(worn.find(mag_id).unwrap(), worn.find(stack_id).unwrap()), 2);
        assert_eq!(rounds_to_load(worn.find(mag_id).unwrap(), worn.find(slugs_id).unwrap()), 3);
        for _ in 0..2 {
            load_round(&mut [&mut worn], stack_id, mag_id).unwrap();
        }
        assert_eq!(load_round(&mut [&mut worn], shells_id, mag_id), Err(Refusal::Filter));
        load_round(&mut [&mut worn], slugs_id, mag_id).unwrap();
        assert_eq!(load_round(&mut [&mut worn], slugs_id, mag_id), Err(Refusal::NoRoom));
        let loaded = worn.find(mag_id).unwrap();
        assert_eq!(loaded.loose.iter().map(|stack| (stack.kind.name.as_str(), stack.count)).collect::<Vec<_>>(), [("round", 2), ("slug", 1)]);
        assert!(worn.find(stack_id).is_none());
        assert_eq!(worn.find(slugs_id).unwrap().count, 4);
        // The slug on top goes onto the slugs' stack; the rounds, whose
        // stack is gone, into the vest's first free cell, then onto it.
        assert_eq!(unload_round(&mut [&mut worn], mag_id, worn_id, &RAID), Ok((slugs_id, Applied::Merged)));
        assert_eq!(worn.find(slugs_id).unwrap().count, 5);
        let (put, _) = unload_round(&mut [&mut worn], mag_id, worn_id, &RAID).unwrap();
        assert_eq!(put, vest_id);
        let new_stack = worn.find(vest_id).unwrap().grids.iter().flatten().find(|(_, item)| item.kind.name == "round").map(|(_, item)| item.id).unwrap();
        assert_eq!(unload_round(&mut [&mut worn], mag_id, worn_id, &RAID), Ok((new_stack, Applied::Merged)));
        assert_eq!((worn.find(new_stack).unwrap().count, magazine_rounds(worn.find(mag_id).unwrap())), (2, 0));
        assert_eq!(unload_round(&mut [&mut worn], mag_id, worn_id, &RAID), Err(Refusal::NoAction));
        // With every cell taken the round stays in its magazine.
        let (mut full, [vest_id, pockets_id, bag_id], _) = dressed();
        let mut magazine = Item::new(Arc::new(Kind { cartridges: vec![holder("cartridges", [0, 0], &["round"], &[])], capacity: 3, ..kind("mag", "magazine", 0.2, [1, 2]) }));
        fill_magazine(&mut magazine, round, 3);
        let (mag_id, full_id) = (magazine.id, full.id);
        full.find_mut(vest_id).unwrap().put(0, magazine, Place { x: 0, y: 0, turned: false }).unwrap();
        for (holder, grid, cells) in [(vest_id, 1, [1, 2]), (vest_id, 2, [2, 2]), (pockets_id, 0, [1, 1]), (pockets_id, 1, [1, 1]), (bag_id, 0, [4, 4])] {
            let Some(holder) = full.find_mut(holder) else { continue };
            holder.grids[grid].push((Place { x: 0, y: 0, turned: false }, thing("brick", "item", 1.0, cells)));
        }
        assert!(unload_round(&mut [&mut full], mag_id, full_id, &RAID).is_err());
        assert_eq!(magazine_rounds(full.find(mag_id).unwrap()), 3);
    }

    /// The menu's Reload on a weapon swaps its magazine for the fullest
    /// within reach at once, the old one to the smallest grid with room of
    /// the vest, the pockets and the backpack; with none fitted it names the
    /// found one to be moved in; with nothing to take it is refused. Install
    /// puts a magazine into the weapon in hand's empty slot, or swaps the
    /// one there out to the first place for it.
    #[test]
    fn the_menu_swaps_or_fits_a_weapon_s_magazine() {
        let place = |x, y| Place { x, y, turned: false };
        let (mut worn, [vest_id, _, bag_id], _) = dressed();
        let (rifle_id, fitted_id) = rifle_worn(&mut worn, 12);
        let next = loaded("mag", 30);
        let next_id = next.id;
        worn.find_mut(vest_id).unwrap().put(0, next, place(0, 0)).unwrap();
        assert_eq!(menu_reload(&mut worn, rifle_id), Ok(MenuReload::Swapped(fitted_id, next_id)));
        assert_eq!(worn.holder_of(next_id), Some((rifle_id, Spot::Slot(0))));
        // Its place found before the swap, as the game finds it: the new one
        // still fills the vest's first grid, so the second, as small.
        assert_eq!(worn.holder_of(fitted_id), Some((vest_id, Spot::Cell(1, place(0, 0)))));
        assert_eq!(magazine_rounds(worn.find(fitted_id).unwrap()), 12);
        // None fitted: the found one is named, to be moved in.
        worn.find_mut(rifle_id).unwrap().slots[0] = None;
        assert_eq!(menu_reload(&mut worn, rifle_id), Ok(MenuReload::Fit(fitted_id)));
        // Nothing with a round within reach: refused.
        let (mut bare, _, _) = dressed();
        let (bare_rifle, _) = rifle_worn(&mut bare, 0);
        assert_eq!(menu_reload(&mut bare, bare_rifle), Err(Refusal::NoAction));
        // Install: into the empty slot of the weapon in hand, by a move.
        assert_eq!(menu_install(&mut worn, fitted_id, Some(rifle_id)), Ok(MenuInstall::Fit(rifle_id, 0)));
        // With one fitted, swapped at once; the old one to the first place
        // for it, the vest first.
        let other = loaded("mag", 7);
        let other_id = other.id;
        worn.find_mut(vest_id).unwrap().put(2, other, place(0, 0)).unwrap();
        worn.find_mut(rifle_id).unwrap().slots[0] = worn.take(fitted_id);
        assert_eq!(menu_install(&mut worn, other_id, Some(rifle_id)), Ok(MenuInstall::Swapped(rifle_id, fitted_id)));
        assert_eq!(worn.holder_of(other_id), Some((rifle_id, Spot::Slot(0))));
        assert_eq!(worn.holder_of(fitted_id).map(|(holder, _)| holder), Some(vest_id));
        // While the weapon's hands are busy, nothing leaves it or goes in.
        let busy = Rules { busy_weapon: Some(rifle_id), ..RAID };
        let worn_id = worn.id;
        assert_eq!(quick_find(&mut [&mut worn], other_id, &[worn_id], Order::UNLOAD_WEAPON, &busy), Err(Refusal::HandsBusy));
        assert_eq!(worn.holder_of(other_id), Some((rifle_id, Spot::Slot(0))));
        assert!(quick_find(&mut [&mut worn], other_id, &[worn_id], Order::UNLOAD_WEAPON, &RAID).is_ok());
        assert_eq!(move_item(&mut [&mut worn], fitted_id, rifle_id, Spot::Slot(0), &busy), Err(Refusal::HandsBusy));
        // A magazine no weapon takes.
        let drum = loaded("drum", 50);
        let drum_id = drum.id;
        worn.find_mut(bag_id).unwrap().put(0, drum, place(0, 0)).unwrap();
        assert_eq!(menu_install(&mut worn, drum_id, Some(rifle_id)), Err(Refusal::NoAction));
    }

    #[test]
    fn a_magazine_s_rounds_go_from_the_top() {
        let mut magazine = loaded("mag", 10);
        magazine.loose.push(Item { count: 5, ..again(&magazine.loose[0]) });
        set_magazine_rounds(&mut magazine, 12);
        assert_eq!(magazine.loose.iter().map(|stack| stack.count).collect::<Vec<_>>(), [10, 2]);
        set_magazine_rounds(&mut magazine, 7);
        assert_eq!(magazine.loose.iter().map(|stack| stack.count).collect::<Vec<_>>(), [7]);
        set_magazine_rounds(&mut magazine, 9);
        assert_eq!(magazine_rounds(&magazine), 9);
        set_magazine_rounds(&mut magazine, 0);
        assert!(magazine.loose.is_empty());
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

    /// What goes together goes either way round: a rifle with the
    /// magazines its slot takes, the rounds its chamber or its magazine
    /// takes, and the parts that fit it or a part on it; a magazine with its
    /// rounds. A refused kind, another calibre, a hat, a backpack's room and
    /// the thing itself do not.
    #[test]
    fn what_goes_together_goes_either_way() {
        let rail = |name: &str| holder(name, [0, 0], &["sight"], &["big_sight"]);
        let mount = Item::new(Arc::new(Kind { slots: vec![rail("mod_scope")], ..kind("mount", "mod", 0.1, [1, 1]) }));
        let slots = vec![holder(MAGAZINE_SLOT, [0, 0], &["mag"], &[]), holder("mod_mount", [0, 0], &["mount"], &[]), holder("patron_in_weapon", [0, 0], &["round"], &[])];
        let mut rifle = Item::new(Arc::new(Kind { slots, ..kind("rifle", "weapon", 3.0, [4, 1]) }));
        rifle.slots[1] = Some(mount);
        let magazine = Item::new(Arc::new(Kind { cartridges: vec![holder("cartridges", [0, 0], &["round"], &[])], ..kind("mag", "magazine", 0.2, [1, 2]) }));
        let (round, other_round) = (thing("round", "ammo", 0.01, [1, 1]), thing("shell", "ammo", 0.04, [1, 1]));
        let (sight, big_sight, hat) = (thing("red_dot", "sight", 0.1, [1, 1]), Item::new(Arc::new(Kind { classes: vec!["big".into(), "big_sight".into(), "sight".into()], ..kind("big", "sight", 0.4, [2, 1]) })), thing("hat", "headwear", 0.1, [1, 1]));
        let bag = Item::new(Arc::new(Kind { grids: vec![holder("main", [4, 4], &["item"], &[])], ..kind("bag", "backpack", 1.0, [3, 3]) }));
        for (a, b) in [(&rifle, &magazine), (&rifle, &round), (&rifle, &sight), (&magazine, &round)] {
            assert!(go_together(a, b) && go_together(b, a), "{} and {}", a.kind.name, b.kind.name);
        }
        for (a, b) in [(&rifle, &other_round), (&rifle, &big_sight), (&rifle, &hat), (&magazine, &sight), (&bag, &round), (&rifle, &rifle)] {
            assert!(!go_together(a, b) && !go_together(b, a), "{} and {}", a.kind.name, b.kind.name);
        }
        // Rounds a rifle has no chamber for, which its magazine takes.
        let mut tube = Item::new(Arc::new(Kind { slots: vec![holder(MAGAZINE_SLOT, [0, 0], &["mag"], &[])], ..kind("tube", "weapon", 3.0, [5, 1]) }));
        tube.slots[0] = Some(again(&magazine));
        assert!(go_together(&tube, &round));
    }
}
