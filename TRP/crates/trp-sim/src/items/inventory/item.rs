//! The item, its tags, lights and repairs, and room in its grids and slots.

use super::*;

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
    pub malfunction: crate::weapons::malfunction::State,
    /// The round a failure to feed took out of its magazine
    /// (`MalfState.MalfunctionedAmmo`), which the repair throws out onto
    /// the ground.
    pub fed_round: Option<Arc<Kind>>,
    /// On a weapon whose rounds lie in chambers of their own (a cylinder's,
    /// barrels'), the case of the round last fired in each, by the
    /// chambers' numbers, of that round's kind (`Weapon.ShellsInChambers`:
    /// kept on the weapon, not the cylinder, and apart from the rounds, so
    /// that a chamber may hold a round and a case). None as made; the
    /// inventory never moves them, the hands do (`chamber_cases`).
    pub shells: Vec<Option<Arc<Kind>>>,
    /// On a face shield (`FaceShieldComponent`, saved with it; body hits
    /// spec, "A face shield's marks"): its hits, from nought, a byte that
    /// comes back to nought at the 256th, and the seed its cracks are drawn
    /// by, taken from the clock as it is made and kept with it ever after.
    /// Both nought on anything else.
    pub hits: u8,
    pub hit_seed: u8,
    /// A repair kit's enhancement, on a thing a kit's repair buffed
    /// (`BuffComponent`, saved with it: `crate::repair::RepairBuff`);
    /// none as made, and none once it is gone.
    pub buff: Option<crate::items::repair::RepairBuff>,
    /// A weapon's fire mode (`FireModeComponent.FireMode`); none until
    /// hands have held it, which is its template's first.
    pub fire_mode: Option<crate::weapons::firearm::FireMode>,
    /// The way a weapon is aimed by, among its sights' (`Weapon.AimIndex`).
    pub aim_index: usize,
    /// What a sight keeps of itself (`SightComponent`): its scope, each
    /// scope's mode and zeroing, its zoom. All nought as made.
    pub sight: crate::weapons::sights::SightKept,
}

/// The seed a face shield is made with (`FaceShieldComponent`'s maker,
/// where the seed is still nought): the low byte of the clock's time in
/// ten-millionths of a second (`DateTime.Now.Ticks & 0xFF`; the .NET epoch
/// and a time zone's offset are whole multiples of 256 ticks, so the
/// Unix clock's low byte is the same).
fn clock_seed() -> u8 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| (since.as_nanos() / 100) as u8)
}

/// Marks the face shield of that number wherever it lies in `root`
/// (`FaceshieldMarkOperation`: its hits plus one, a byte). Whether there
/// was one; the limit on marks is the marking character's (`armor::
/// MarkLimit`).
pub fn mark(root: &mut Item, id: Id) -> bool {
    match root.find_mut(id).filter(|item| item.kind.face_shield) {
        Some(shield) => {
            shield.hits = shield.hits.wrapping_add(1);
            true
        }
        None => false,
    }
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
        let kind_face_shield = kind.face_shield;
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
            malfunction: crate::weapons::malfunction::State::default(),
            fed_round: None,
            shells: Vec::new(),
            hits: 0,
            hit_seed: if kind_face_shield { clock_seed() } else { 0 },
            buff: None,
            fire_mode: None,
            aim_index: 0,
            sight: Default::default(),
        }
    }

    /// Its durability and its greatest durability now: its own wear's where
    /// it wears (`repairable`), else its template's.
    pub fn durability(&self) -> [f32; 2] {
        self.repairable.map_or(self.kind.durability, |wear| [wear.durability, wear.max_durability])
    }

    /// A copy of it with a number of its own for it and for each thing in
    /// it, at any depth.
    pub fn renumbered(&self) -> Item {
        fn walk(item: &mut Item) {
            item.id = next_id();
            for inner in item.slots.iter_mut().flatten() {
                walk(inner);
            }
            for (_, inner) in item.grids.iter_mut().flatten() {
                walk(inner);
            }
            for inner in &mut item.loose {
                walk(inner);
            }
        }
        let mut copy = self.clone();
        walk(&mut copy);
        copy
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
    pub(super) fn extra(&self) -> [i32; 4] {
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

    /// Whether the slot takes the item, by what the slot alone says
    /// (`Slot.CheckConditions`, `AddInternal`): it is empty and not
    /// locked; its filter names the item or a class of it, and its
    /// excluded list names neither the item nor anything in it
    /// (`ItemFilterExtension.CheckItem`); a thing for the special slots
    /// alone goes into one of them; and the item is a single one. (The
    /// game asks each of a slot's filters in turn; every slot of the
    /// install has one.) What the whole the slot is part of says is
    /// `conflict_at`'s.
    pub fn slot_takes(&self, slot: usize, item: &Item) -> Result<(), Refusal> {
        let holder = self.kind.slots.get(slot).ok_or(Refusal::NoSuchPlace)?;
        if self.slots[slot].is_some() {
            return Err(Refusal::Taken);
        }
        if holder.locked {
            return Err(Refusal::Locked);
        }
        if !item.kind.among(&holder.takes) || item.kind.among(&holder.refuses) || item.any_within(&|inside| inside.kind.among(&holder.refuses)) {
            return Err(Refusal::Filter);
        }
        if item.kind.special_only && !holder.name.contains("SpecialSlot") {
            return Err(Refusal::Filter);
        }
        if item.count != 1 {
            return Err(Refusal::Stack);
        }
        Ok(())
    }

    /// Whether anything in it, at any depth, is so.
    fn any_within(&self, test: &dyn Fn(&Item) -> bool) -> bool {
        self.contents().any(|inside| test(inside) || inside.any_within(test))
    }

    /// How many of a kind it holds, at any depth, stacks counted whole
    /// (`GetAllItemsFromCollection`, `StackObjectsCount`).
    pub(super) fn count_within(&self, template: &str) -> u32 {
        self.contents().map(|inside| if inside.kind.template == template { inside.count } else { 0 } + inside.count_within(template)).sum()
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
    pub(super) fn grid_room(&self, grid: usize, item: &Item, place: Place, own_free: bool) -> Result<(), Refusal> {
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

    /// The places `grid_room` takes, tried in the same order; but the
    /// filter asked once (it goes by the item alone), and the taken cells
    /// marked once, so that a place is not measured against every thing
    /// lying in the grid, nor each thing's size worked out again from its
    /// parts at every place (the range's deep containers, filled at the
    /// start, spent 0.15 s here).
    pub(super) fn room(&self, grid: usize, item: &Item, own_free: bool) -> Option<Place> {
        let holder = self.kind.grids.get(grid)?;
        if (!holder.takes.is_empty() && !item.kind.among(&holder.takes)) || item.kind.among(&holder.refuses) {
            return None;
        }
        let [columns, rows] = holder.cells;
        // A cell taken where a thing lies on it (but the item itself, where
        // its own cells are free to it); a thing's cells past the grid's
        // edge are no place's, as every place is within it.
        let mut taken = vec![false; columns as usize * rows as usize];
        for (at, other) in &self.grids[grid] {
            if other.id == item.id && own_free {
                continue;
            }
            let [w, h] = other.cells_at(*at);
            for y in at.y..at.y.saturating_add(h).min(rows) {
                for x in at.x..at.x.saturating_add(w).min(columns) {
                    taken[y as usize * columns as usize + x as usize] = true;
                }
            }
        }
        let free = |x: u32, y: u32, [w, h]: [u32; 2]| (y..y + h).all(|y| (x..x + w).all(|x| !taken[y as usize * columns as usize + x as usize]));
        let [width, height] = item.cells();
        let turns: &[bool] = if width == height { &[false] } else { &[false, true] };
        turns.iter().find_map(|&turned| {
            let size = item.cells_at(Place { x: 0, y: 0, turned });
            if size[0] > columns || size[1] > rows {
                return None;
            }
            let places = (0..=rows - size[1]).flat_map(|y| (0..=columns - size[0]).map(move |x| (x, y)));
            places.into_iter().find(|&(x, y)| free(x, y, size)).map(|(x, y)| Place { x, y, turned })
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
