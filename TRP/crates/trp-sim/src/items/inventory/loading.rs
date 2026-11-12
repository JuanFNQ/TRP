//! Loading and unloading rounds, weapon builds, chambering and unfit rounds.

use super::*;

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

/// What rounds dropped on a magazine come to (`Magazine.Apply`): how many
/// it loads, or why it takes none. The game asks in this order: a round
/// its cartridges do not take (`Magazine.ItemFiltersWontAllowError`), then
/// a full magazine (`Magazine.FullMagazineError`). None where the one is
/// no round or the other no magazine that holds rounds.
pub fn rounds_onto_magazine(magazine: &Item, rounds: &Item) -> Option<Result<u32, Refusal>> {
    let holder = magazine.kind.cartridges.first().filter(|_| is(rounds, "Ammo") && is(magazine, "Magazine"))?;
    Some(if !rounds.kind.among(&holder.takes) || rounds.kind.among(&holder.refuses) {
        Err(Refusal::WrongAmmo)
    } else if magazine_rounds(magazine) >= magazine.kind.capacity {
        Err(Refusal::MagazineFull)
    } else {
        Ok(rounds_to_load(magazine, rounds))
    })
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
    // A cylinder's round into its first empty chamber (`PROVISIONAL`: the
    // order the game's inventory fills one in is not read).
    if is_cylinder(target) {
        into_cylinder(target, kind);
        return Ok(());
    }
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
    // A cylinder's round out of its last loaded chamber (`PROVISIONAL`, as
    // its filling's order).
    let chamber = camoras(target).filter(|&index| target.slots.get(index).is_some_and(Option::is_some)).last();
    let kind = match chamber {
        Some(index) => target.slots[index].take().map(|round| round.kind).ok_or(Refusal::NoAction)?,
        None if is_cylinder(target) => return Err(Refusal::NoAction),
        None => {
            let top = target.loose.last_mut().ok_or(Refusal::NoAction)?;
            let kind = top.kind.clone();
            top.count -= 1;
            if top.count == 0 {
                target.loose.pop();
            }
            kind
        }
    };
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
            match (chamber, target.loose.last_mut()) {
                (Some(index), _) => target.slots[index] = Some(Item::new(kind)),
                (None, Some(top)) if top.kind.template == kind.template => top.count += 1,
                (None, _) => target.loose.push(Item::new(kind)),
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
pub fn build_of(weapon: &Item) -> trp_data::setup::database::Preset {
    use trp_data::setup::database::{Preset, PresetItem};
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
    let key = trp_data::weapons::weapon::build_key(&build_of(item));
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
    // A cylinder's from its last loaded chamber back.
    if is_cylinder(magazine) {
        let mut rounds = camoras(magazine).filter_map(|index| magazine.slots.get(index).and_then(Option::as_ref)).collect::<Vec<_>>();
        rounds.reverse();
        return rounds.get(from_top as usize).map(|round| round.kind.clone());
    }
    let mut left = from_top;
    for stack in magazine.loose.iter().rev() {
        if left < stack.count {
            return Some(stack.kind.clone());
        }
        left -= stack.count;
    }
    None
}

/// Whether a weapon's chamber takes a round of this kind (`Slot.CanAccept`
/// on one of `Weapon.Chambers`: by its filters alone, whatever lies in it).
/// `None` for a weapon with no chamber of its own (a revolver, whose rounds
/// lie in its cylinder; a weapon of no chamber), which refuses no round
/// (`Magazine.IsAmmoCompatible`: no chambers, compatible).
pub fn chamber_takes(weapon: &Item, round: &Kind) -> Option<bool> {
    let mut chambers = weapon.kind.slots.iter().filter(|slot| is_chamber(&slot.name)).peekable();
    chambers.peek()?;
    Some(chambers.any(|slot| round.among(&slot.takes) && !round.among(&slot.refuses)))
}

/// How many of a magazine's rounds, counted from its foot, lie up to and
/// with the first, from its top, that the weapon's chamber does not take
/// (`chamber_takes`): 0 where it takes them all, and for a cylinder or a
/// weapon with no chamber. The game feeds a chamber only from a magazine
/// whose top round it takes (`Magazine.IsAmmoCompatible`; firing spec,
/// "Rounds the chamber does not take"), so the magazine feeds while it
/// holds more than this; counted from the foot, the figure holds as rounds
/// leave the top (`Firearm::set_magazine_unfit`).
pub fn unfit_rounds(weapon: &Item, magazine: &Item) -> u32 {
    if is_cylinder(magazine) {
        return 0;
    }
    let mut left = magazine_rounds(magazine);
    for stack in magazine.loose.iter().rev() {
        if chamber_takes(weapon, &stack.kind) == Some(false) {
            return left;
        }
        left = left.saturating_sub(stack.count);
    }
    0
}

/// The same of the magazine fitted to a weapon; 0 with none.
pub fn fitted_unfit_rounds(weapon: &Item) -> u32 {
    weapon.in_slot(MAGAZINE_SLOT).map_or(0, |magazine| unfit_rounds(weapon, magazine))
}

/// Whether a weapon's fitted magazine has on its top a round the chamber
/// does not take (`Weapon.CompatibleAmmo` false, by which the inventory
/// draws the weapon unusable: `GridItemView.UpdateInfo`).
pub fn ammo_incompatible(weapon: &Item) -> bool {
    weapon.in_slot(MAGAZINE_SLOT).is_some_and(|magazine| {
        let rounds = magazine_rounds(magazine);
        rounds > 0 && unfit_rounds(weapon, magazine) >= rounds
    })
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

/// Fills a magazine with `count` rounds of one kind, what it held gone: a
/// cylinder's first `count` chambers a round each.
pub fn fill_magazine(magazine: &mut Item, round: Arc<Kind>, count: u32) {
    magazine.loose.clear();
    if is_cylinder(magazine) {
        let places: Vec<usize> = camoras(magazine).collect();
        for (nth, index) in places.into_iter().enumerate() {
            magazine.slots[index] = (nth < count as usize).then(|| Item::new(round.clone()));
        }
        return;
    }
    if count > 0 {
        let mut stack = Item::new(round);
        stack.count = count;
        magazine.loose.push(stack);
    }
}
