//! The equipment, the carried weight and items made from lists.

use super::*;

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
        carried_weight(&self.item, elite_strength)
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

/// The same of equipment kept as its item (a body's gear: the dummies').
pub fn carried_weight(equipment: &Item, elite_strength: bool) -> f32 {
    let counted = SLOTS.into_iter().filter(|slot| !elite_strength || ELITE_WEIGHT.contains(slot));
    // (An empty sum is minus zero.)
    counted.filter_map(|slot| equipment.in_slot(slot)).map(Item::weight).sum::<f32>() + 0.0
}

/// What a character's weight was last worked out from, as the game's
/// inventory tells of it (movement spec, "The weight, as the inventory
/// tells of it"). The game's character reads its carried weight again only
/// on an event of its inventory for a thing inside the equipment, a thing
/// added, removed or refreshed (`Inventory.UpdateTotalWeight`, subscribed
/// in `InventoryController.ReplaceInventory`); a refresh is a stack merged
/// or split, a fold, a switch, a tag, a medkit's or food's use, armour's
/// wear, a repair, a face shield's mark, a magazine checked. Between two
/// the sum it read stands (`Inventory.TotalWeight`, deferred). The hands'
/// own work raises none: the rounds shots take out of the magazine and the
/// chamber, an underbarrel launcher's grenade fired and a weapon's wear
/// are counted at the next event (recording 11's 25 shots, `courses3`'s
/// launcher). A host keeps one with what it carries: `told` after any
/// change, `as_told` before and `pass` after what the hands do unheard,
/// and `forget` on an event that changes nothing kept in an item (a
/// magazine checked, known before or not).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Weighed(pub(super) u64);

impl Weighed {
    /// Whether the equipment differs from what was last told: an event,
    /// told now.
    pub fn told(&mut self, equipment: &Item) -> bool {
        let now = told_of(equipment);
        std::mem::replace(&mut self.0, now) != now
    }

    /// Whether nothing has changed since it was last told.
    pub fn as_told(&self, equipment: &Item) -> bool {
        self.0 == told_of(equipment)
    }

    /// The equipment as it now stands taken as told, with no event: what
    /// the hands have just done, unheard, where nothing changed before it.
    pub fn pass(&mut self, equipment: &Item) {
        self.0 = told_of(equipment);
    }

    /// An event whatever the equipment holds: the next `told` is one.
    pub fn forget(&mut self) {
        self.0 = 0;
    }
}

/// A number for all an event of the inventory tells of: each thing where
/// it lies (its holder and its slot, its grid and place, or its order
/// among a magazine's rounds), its stack, its use, switch, fold, tag,
/// marks, buff and wear (of a weapon its greatest durability alone: its
/// durability goes down with each shot, unheard). Summed over the things,
/// so that their order does not count; nought only by chance.
fn told_of(equipment: &Item) -> u64 {
    // (SplitMix64's mixer: any value changed changes the whole.)
    fn mix(state: u64, value: u64) -> u64 {
        let mut z = (state ^ value).wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    let mut sum = 0u64;
    let mut waiting = vec![(equipment, 0u64)];
    while let Some((item, at)) = waiting.pop() {
        let mut own = mix(mix(at, item.id), u64::from(item.count));
        own = mix(own, item.resource.map_or(u64::MAX, |left| u64::from(left.to_bits())));
        own = mix(own, u64::from(item.on) | u64::from(item.folded) << 1 | u64::from(item.hits) << 8 | u64::from(item.tag.color) << 16);
        own = item.tag.name.bytes().fold(own, |own, byte| mix(own, u64::from(byte)));
        if let Some(wear) = item.repairable {
            own = mix(own, u64::from(wear.max_durability.to_bits()));
            if !is(item, "Weapon") {
                own = mix(own, u64::from(wear.durability.to_bits()));
            }
        }
        if let Some(buff) = item.buff {
            own = mix(mix(own, buff.value.to_bits()), buff.threshold.to_bits());
        }
        sum = sum.wrapping_add(own);
        let holder = mix(2, item.id);
        for (slot, part) in item.slots.iter().enumerate() {
            if let Some(part) = part {
                waiting.push((part, mix(mix(holder, 1), slot as u64)));
            }
        }
        for (grid, held) in item.grids.iter().enumerate() {
            for (place, thing) in held {
                let spot = u64::from(place.x) | u64::from(place.y) << 24 | u64::from(place.turned) << 48;
                waiting.push((thing, mix(mix(mix(holder, 2), grid as u64), spot)));
            }
        }
        for (order, round) in item.loose.iter().enumerate() {
            waiting.push((round, mix(mix(holder, 3), order as u64)));
        }
    }
    sum
}

impl Item {
    /// Fills each locked slot left empty with the plate its filter names,
    /// as the game does where it makes one of every item
    /// (`ItemFactory.CreateAndFillItem`, for its profile editor's list and
    /// its loot test): the built-in inserts of a helmet or an armour that
    /// has no stock build to come with (six helmets of the install, the
    /// FLUX and the Strike among them). The plates go in with no rule
    /// asked (`AddWithoutRestrictions`), and are not filled in turn. Without
    /// them the thing misses a vital part and cannot be worn.
    pub fn fill_locked(&mut self, db: &Database) -> Result<()> {
        let kind = self.kind.clone();
        for (index, holder) in kind.slots.iter().enumerate() {
            let Some(plate) = holder.plate.as_deref().filter(|_| holder.locked) else { continue };
            if self.slots[index].is_none() {
                self.slots[index] = Some(Item::new(Arc::new(Kind::of(db, plate)?)));
            }
        }
        Ok(())
    }

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
                // A cylinder's rounds into its chambers, one a chamber.
                (None, None, None) if name == "cartridges" && is_cylinder(&holder) => {
                    for _ in 0..item.count.max(1) {
                        into_cylinder(&mut holder, item.kind.clone());
                    }
                }
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
