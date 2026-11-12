//! Raid limits and conflicts between equipped items.

use super::*;

/// The class of backpacks, which go into one another only so deep.
pub(super) const BACKPACK: &str = "5448e53e4bdc2d60728b4567";

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
    /// How many of a kind the character may carry in a raid, by template
    /// (`raid_limits`; the options' `RAID LIMITS`, off unless chosen):
    /// none, no limit.
    pub limits: Option<&'static std::collections::HashMap<String, u32>>,
    /// The character's own equipment, where another may be among the roots
    /// (a body's): only its own slots refuse a thing not examined, and
    /// only onto it does the raid's limit count. None: any equipment is the
    /// character's.
    pub own: Option<Id>,
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
        Self { in_raid: true, backpacks_deep: named(&db.config, "MaxBackpackInserting").unwrap_or(0) as u32, busy_weapon: None, chamber_closed: None, chamber_held: None, limits: None, own: None }
    }
}

/// How many of each kind the character may carry in a raid, by template
/// (`GlobalConfiguration.RestrictionsInRaid`: each one's `MaxInRaid`,
/// raised to its `MaxInLobby` where that is more; a negative one is no
/// limit). The game's own controller of a raid goes by `MaxInRaid`
/// (`InventoryController.IsLimitedAtAddress`); the stash's out of a raid
/// by `MaxInLobby` (`BackEndInventoryController`), which the app has no
/// use for.
pub fn raid_limits(db: &Database) -> std::collections::HashMap<String, u32> {
    let list = db.config.get("RestrictionsInRaid").and_then(|list| list.as_array()).cloned().unwrap_or_default();
    list.iter()
        .filter_map(|entry| {
            let template = entry.get("TemplateId")?.as_str()?.to_owned();
            let raid = entry.get("MaxInRaid")?.as_i64()?;
            let lobby = entry.get("MaxInLobby").and_then(|lobby| lobby.as_i64()).unwrap_or(-1);
            let limit = if raid != -1 && raid < lobby { lobby } else { raid };
            (limit >= 0).then_some((template, limit as u32))
        })
        .collect()
}

/// Whether so many of a thing may come onto the character from elsewhere
/// (`ItemManipulator.CheckItemCountLimit`, `InventoryController.
/// CheckOverLimit`): of each kind among it and everything in it that has
/// a limit, in the order first met, what is coming (the thing's own
/// `count`, the rest's stacks whole) may not be more than the limit less
/// what the character has of it already. The first kind over its limit.
pub(super) fn within_limits(equipment: &Item, coming: &Item, count: u32, rules: &Rules) -> Result<(), Refusal> {
    let Some(limits) = rules.limits else { return Ok(()) };
    let mut kinds: Vec<(&str, u32)> = Vec::new();
    fn gather<'a>(item: &'a Item, count: u32, kinds: &mut Vec<(&'a str, u32)>) {
        match kinds.iter_mut().find(|(template, _)| *template == item.kind.template) {
            Some((_, sum)) => *sum += count,
            None => kinds.push((&item.kind.template, count)),
        }
        for inside in item.contents() {
            gather(inside, inside.count, kinds);
        }
    }
    gather(coming, count, &mut kinds);
    for (template, sum) in kinds {
        let Some(&limit) = limits.get(template) else { continue };
        if i64::from(sum) > i64::from(limit) - i64::from(equipment.count_within(template)) {
            return Err(Refusal::OverLimit { template: template.to_owned(), limit });
        }
    }
    Ok(())
}

/// Whether a place merges with the item that holds it (`IContainer.
/// MergesWithChildren`): a slot by its own word or its item's (`merges`),
/// a grid and a magazine's rounds by their item's.
fn place_merges(holder: &Item, spot: Spot) -> bool {
    match spot {
        Spot::Slot(slot) => holder.kind.slots.get(slot).is_some_and(|slot| merges(holder, slot)),
        Spot::Cell(..) | Spot::Loose => holder.kind.merges,
    }
}

/// The whole whose things a slot of `holder` is looked over against for
/// conflicting items (`Slot.CheckConflictingItems`): from the holder up
/// through the places that merge, never into the character's equipment
/// (`GetRootItem`); then, where that item's own place merges, what holds
/// it (the equipment, for the things worn on the head, whose slots merge).
fn conflict_root(root: &Item, holder: Id) -> Option<&Item> {
    let mut at = holder;
    while let Some((up, spot)) = root.holder_of(at) {
        let held_by = root.find(up)?;
        if !place_merges(held_by, spot) || held_by.kind.template == EQUIPMENT_TEMPLATE {
            break;
        }
        at = up;
    }
    if let Some((up, spot)) = root.holder_of(at) {
        if root.find(up).is_some_and(|held_by| place_merges(held_by, spot)) {
            at = up;
        }
    }
    root.find(at)
}

/// An item and what merges with it, depth first in its places' order
/// (`GetMergedItemContent`), less a thing that is leaving and what is in
/// it.
fn merged_content<'a>(item: &'a Item, leaving: Option<Id>, out: &mut Vec<&'a Item>) {
    if Some(item.id) == leaving {
        return;
    }
    out.push(item);
    for (index, part) in item.slots.iter().enumerate() {
        if let Some(part) = part.as_ref().filter(|_| place_merges(item, Spot::Slot(index))) {
            merged_content(part, leaving, out);
        }
    }
    if item.kind.merges {
        for (_, inside) in item.grids.iter().flatten() {
            merged_content(inside, leaving, out);
        }
        for inside in &item.loose {
            merged_content(inside, leaving, out);
        }
    }
}

/// The first pair of things that may not be together (`Slot.
/// CheckConflictingItems`, by templates): of the things there, the first
/// that a thing coming names among its conflicting items; else of the
/// things coming, the first that a thing there names. The template of the
/// one there, and of the one coming.
pub fn conflict_between(there: &[&Item], coming: &[&Item]) -> Option<(String, String)> {
    let names = |namer: &Item, named: &Item| namer.kind.conflicting.iter().any(|id| *id == named.kind.template);
    for old in there {
        if let Some(new) = coming.iter().find(|new| names(new, old)) {
            return Some((old.kind.template.clone(), new.kind.template.clone()));
        }
    }
    for new in coming {
        if let Some(old) = there.iter().find(|old| names(old, new)) {
            return Some((old.kind.template.clone(), new.kind.template.clone()));
        }
    }
    None
}

/// Whether a slot of `holder` (somewhere in `root`) may take `item` by the
/// conflicting items (`Slot.CheckConflictingItems`): a slot that does not
/// merge with its item asks nothing (the character's weapon slots, a
/// helmet's own); else nothing of the whole it joins (`conflict_root`)
/// may conflict with anything of the item's. `leaving`: a thing taken out
/// first (the item itself, on a move; the part replaced, at the bench).
pub fn conflict_at(root: &Item, holder: Id, slot: usize, item: &Item, leaving: Option<Id>) -> Result<(), Refusal> {
    let Some(held_by) = root.find(holder) else { return Ok(()) };
    if !place_merges(held_by, Spot::Slot(slot)) {
        return Ok(());
    }
    let Some(whole) = conflict_root(root, holder) else { return Ok(()) };
    let (mut there, mut coming) = (Vec::new(), Vec::new());
    merged_content(whole, leaving, &mut there);
    merged_content(item, None, &mut coming);
    match conflict_between(&there, &coming) {
        Some((existing, new)) => Err(Refusal::Conflicting { existing, new }),
        None => Ok(()),
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
