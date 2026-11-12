//! Splitting, ordering, quick finding, picking up, equipping and binding items.

use super::*;

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
    let part = Item { id: next_id(), count, ..item.clone() };
    match spot {
        Spot::Slot(slot) => {
            target.slot_takes(slot, &part)?;
            conflict_at(roots[to], into, slot, &part, None)?;
        }
        Spot::Cell(grid, place) => target.grid_takes(grid, &part, place)?,
        Spot::Loose => return Err(Refusal::NoSuchPlace),
    }
    if own(roots[to], rules) && !own(roots[from], rules) {
        within_limits(roots[to], &part, count, rules)?;
    }
    let made = part.id;
    roots[from].find_mut(id).ok_or(Refusal::NoSuchPlace)?.count -= count;
    roots[to].find_mut(into).ok_or(Refusal::NoSuchPlace)?.set(spot, part);
    Ok(made)
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
        match apply_stack(roots, id, onto, rules) {
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
            // Another's equipment (a body's) offers none of its
            // `MobContainer`s (`ItemManipulator`'s quick find: where the
            // mover is not the equipment's owner; the secured container).
            Some(found) if found.kind.template == EQUIPMENT_TEMPLATE => {
                let another = !own(found, rules);
                loot_order(item).iter().filter_map(|slot| found.in_slot(slot)).filter(|holder| !(another && is(holder, "MobContainer"))).map(|holder| holder.id).collect()
            }
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
/// carried out with a count; onto the character from elsewhere, no more
/// than its limit leaves room for: `CheckItemCountLimit`).
pub fn transfer_item(roots: &mut [&mut Item], id: Id, onto: Id, count: u32, rules: &Rules) -> Result<(), Refusal> {
    if count == 0 || count > transfer_most(roots, id, onto) {
        return Err(Refusal::NoAction);
    }
    let (from, to) = (root_of(roots, id)?, root_of(roots, onto)?);
    stacked_within_limits(roots, from, to, id, count, rules)?;
    roots[from].find_mut(id).ok_or(Refusal::NoSuchPlace)?.count -= count;
    roots[to].find_mut(onto).ok_or(Refusal::NoSuchPlace)?.count += count;
    Ok(())
}

/// Whether so many of a stack may go onto a stack the character has, from
/// elsewhere (`CheckItemCountLimit` on a merge or a transfer: the count
/// moved against the room the limit leaves).
pub(super) fn stacked_within_limits(roots: &[&mut Item], from: usize, to: usize, id: Id, count: u32, rules: &Rules) -> Result<(), Refusal> {
    if !own(roots[to], rules) || own(roots[from], rules) {
        return Ok(());
    }
    let item = roots[from].find(id).ok_or(Refusal::NoSuchPlace)?;
    within_limits(roots[to], item, count, rules)
}

/// One stack put on another of its kind with no place chosen
/// (`TransferOrMerge`): the whole of it if the other has the room, else
/// as much as the other has room for.
fn apply_stack(roots: &mut [&mut Item], id: Id, onto: Id, rules: &Rules) -> Result<Applied, Refusal> {
    let (from, to) = (root_of(roots, id)?, root_of(roots, onto)?);
    let (count, room) = (roots[from].find(id).ok_or(Refusal::NoSuchPlace)?.count, roots[to].find(onto).ok_or(Refusal::NoSuchPlace)?.stack_room());
    if room >= count {
        stacked_within_limits(roots, from, to, id, count, rules)?;
        roots[from].take(id);
        roots[to].find_mut(onto).ok_or(Refusal::NoSuchPlace)?.count += count;
        return Ok(Applied::Merged);
    }
    let most = transfer_most(roots, id, onto);
    transfer_item(roots, id, onto, most, rules).map(|()| Applied::Topped(most))
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
    // (A thing not known is not bound: `IsAtBindablePlace`.)
    in_reach && equipment.find(id).is_some_and(|item| BINDABLE.iter().any(|class| is(item, class)) && !item.missing_vital() && crate::items::examine::known(item))
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
