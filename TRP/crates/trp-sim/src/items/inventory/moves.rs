//! Finding items, and moving, taking out and using them up.

use super::*;

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
    pub(super) fn chain(&self, id: Id) -> Vec<&Item> {
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
    pub(super) fn take(&mut self, id: Id) -> Option<Item> {
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
    pub(super) fn set(&mut self, spot: Spot, item: Item) {
        match spot {
            Spot::Slot(slot) => self.slots[slot] = Some(item),
            Spot::Cell(grid, place) => self.grids[grid].push((place, item)),
            Spot::Loose => self.loose.push(item),
        }
    }

    /// For the character's equipment: nothing worn blocks that slot, and
    /// the item would block no slot that has something in it.
    pub(super) fn blocking(&self, slot: usize, item: &Item) -> Result<(), Refusal> {
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
    pub(super) fn stack_room(&self) -> u32 {
        self.kind.stack_max.saturating_sub(self.count)
    }
}

pub(super) fn root_of(roots: &[&mut Item], id: Id) -> Result<usize, Refusal> {
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
    // A locked slot gives nothing up (`Slot.RemoveItemInternal`, after the
    // raid's own rules: `MovePathCheck` comes first).
    if matches!(was, Spot::Slot(slot) if roots[from].find(holder).and_then(|held_by| held_by.kind.slots.get(slot)).is_some_and(|slot| slot.locked)) {
        return Err(Refusal::Locked);
    }
    // Nor does a slot of one's own give up a thing not known (after its
    // lock, before a malfunction).
    if leaves.is_some() && unknown_to_own(roots[from], item, rules) {
        return Err(Refusal::TakeNotExamined { equipment: roots[from].id == holder });
    }
    if item.find(into).is_some() {
        return Err(Refusal::Cycle);
    }
    // A weapon in hand whose hands are busy keeps its parts, and its place
    // (`FirearmController.CanExecute`: a move the weapon is part of,
    // `HasPartOfWeapon`, the weapon itself among them, waits for idling).
    if let Some(weapon) = rules.busy_weapon {
        let within = |root: &Item, holder: Id| root.find(weapon).is_some_and(|weapon| weapon.find(holder).is_some());
        if id == weapon || within(roots[from], holder) || within(roots[to], into) {
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
    // In a raid, a plate's slot of armour worn takes nothing
    // (`ArmorSlot.CanAcceptRaid`, asked after the slot's lock).
    if let Spot::Slot(slot) = spot {
        let (holder_of_slot, open) = (target.kind.slots.get(slot), target.slots.get(slot).is_some_and(Option::is_none));
        let plate = holder_of_slot.is_some_and(|holder| holder.armour && !holder.locked);
        if open && plate && rules.in_raid && worn && roots[to].slots.iter().flatten().any(|on| on.id == into) {
            return Err(Refusal::PlateWorn);
        }
    }
    // Whether the place takes it, and only then out and in.
    let fits = match spot {
        Spot::Slot(slot) => {
            let blocked = if target.kind.template == EQUIPMENT_TEMPLATE { target.blocking(slot, item) } else { Ok(()) };
            // A slot of one's own takes no thing not known, after its
            // being taken and its lock, before its filter.
            let open = target.slots.get(slot).is_some_and(Option::is_none) && target.kind.slots.get(slot).is_some_and(|holder| !holder.locked);
            if open && unknown_to_own(roots[to], item, rules) {
                Err(Refusal::NotExamined { equipment: roots[to].id == into })
            } else {
                target.slot_takes(slot, item).and(blocked)
            }
        }
        Spot::Cell(grid, place) => target.grid_takes(grid, item, place),
        Spot::Loose => Err(Refusal::NoSuchPlace),
    };
    fits?;
    if let Spot::Slot(slot) = spot {
        conflict_at(roots[to], into, slot, item, Some(id))?;
    }
    // Onto the character from elsewhere, so many of a kind and no more.
    if own(roots[to], rules) && !own(roots[from], rules) {
        within_limits(roots[to], item, item.count, rules)?;
    }
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
    // A locked slot gives nothing up, the raid's own rules asked first;
    // then one's own gives up no thing not known.
    let locked = if held_by.kind.slots.get(slot).is_some_and(|slot| slot.locked) { Err(Refusal::Locked) } else { Ok(()) };
    let unknown = if unknown_to_own(roots[from], item, rules) { Err(Refusal::TakeNotExamined { equipment: roots[from].id == holder }) } else { Ok(()) };
    let locked = locked.and(unknown);
    let Some(slot) = held_by.kind.slots.get(slot).filter(|_| rules.in_raid) else { return locked };
    if item.kind.not_raid_moddable {
        return Err(Refusal::NotInRaid);
    }
    if slot.required {
        return Err(Refusal::Vital(slot.name.clone()));
    }
    if roots[from].id == holder && held_by.kind.template == EQUIPMENT_TEMPLATE && item.kind.kept_in.contains(&slot.name) {
        return Err(Refusal::KeptOn(slot.name.clone()));
    }
    locked
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
    // Nor is the weapon in hand whose hands are busy, or a thing in it.
    if rules.busy_weapon.is_some_and(|weapon| id == weapon || roots[from].find(weapon).is_some_and(|weapon| weapon.find(id).is_some())) {
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
    let known = |weapon: &Item| weapon.malfunction.malf != crate::weapons::malfunction::Malf::None && weapon.malfunction.known;
    if is(item, "Weapon") {
        return known(item);
    }
    let Some(weapon) = weapon_fitted_with(root, item.id) else { return false };
    if is(item, "Magazine") {
        return weapon.malfunction.malf == crate::weapons::malfunction::Malf::Feed && known(weapon);
    }
    is(item, "Ammo") && known(weapon)
}

/// Whether a weapon's malfunction keeps a thing out of, or in, a slot of
/// `holder` (`Weapon.IncompatibleByMalfunction`): a magazine with a failure
/// to feed (or any malfunction on a weapon fitted with a magazine that comes
/// out though it has one of its own, `ExternalMagazineWithInternalReloadSupport`:
/// the Model 700), a round with any.
pub(super) fn malfunction_refuses(root: &Item, holder: Id, kind: &Kind) -> bool {
    let Some(weapon) = weapon_of(root, holder) else { return false };
    let malf = weapon.malfunction.malf;
    let class = |name: &str| kind.class_names.iter().any(|class| class == name);
    if class("Magazine") {
        return malf == crate::weapons::malfunction::Malf::Feed || (malf != crate::weapons::malfunction::Malf::None && reload_mode(weapon) == MIXED_RELOAD);
    }
    class("Ammo") && malf != crate::weapons::malfunction::Malf::None
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
