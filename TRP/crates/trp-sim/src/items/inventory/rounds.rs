//! Rounds within reach, magazine rounds and the words for ammunition.

use super::*;

/// The equipment slots whose things are within reach for a reload
/// (`Inventory.FastAccessSlots`), in the game's order: the pockets'
/// stacks are taken before the vest's.
pub(super) const WITHIN_REACH: [&str; 2] = ["Pockets", "TacticalVest"];

/// The stacks of rounds within reach that a weapon's chamber takes
/// (`InventoryController.GetReachableItemsOfType`, with the test the
/// reload key makes: `Chambers[0].CanAccept`): what lies directly in the
/// pockets and the vest worn, in the order the game goes through them;
/// or of one stack alone where a reload was given one (`AmmoPack` of the
/// stack the magazine picker chose: `GamePlayerOwner.SwitchMagazine`),
/// while it lies within reach.
fn rounds_within_reach_from(equipment: &Item, weapon: Id, pack: Option<Id>) -> Vec<Id> {
    let mut within = rounds_within_reach_all(equipment, weapon);
    if let Some(pack) = pack {
        within.retain(|id| *id == pack);
    }
    within
}

fn rounds_within_reach_all(equipment: &Item, weapon: Id) -> Vec<Id> {
    let Some(takes) = equipment.find(weapon).and_then(takes_rounds) else {
        return Vec::new();
    };
    // Rounds not known are passed over (`Examined`, in each of the reload
    // key's searches).
    let worn = WITHIN_REACH.iter().filter_map(|slot| equipment.in_slot(slot));
    worn.flat_map(|holder| holder.grids.iter().flatten()).filter(|(_, item)| item.kind.among(&takes) && crate::items::examine::known(item)).map(|(_, item)| item.id).collect()
}

/// How many rounds the character has within reach for a weapon carried:
/// what a tube is filled from (`FirearmHandsInputTranslator.ReloadWithAmmo`).
pub fn rounds_in_reach(equipment: &Item, weapon: Id) -> u32 {
    rounds_in_reach_from(equipment, weapon, None)
}

/// The same, of the reload's one stack where it was given one.
pub fn rounds_in_reach_from(equipment: &Item, weapon: Id, pack: Option<Id>) -> u32 {
    rounds_within_reach_from(equipment, weapon, pack).iter().filter_map(|id| equipment.find(*id)).map(|stack| stack.count).sum()
}

/// The stack of rounds the reload key's last resort puts one of into the
/// weapon's chamber (`FirearmHandsInputTranslator.LoadAmmoToChamber`: the
/// first of `GetReachableItemsOfTypeNonAlloc` whose round the first chamber
/// takes, `Chambers[0].CanAccept`): what lies directly in the pockets and the
/// vest worn, in the order the game goes through them, known or not (this
/// search does not ask; the weapon's `Apply` then refuses one not known,
/// `apply_item`). None for a weapon with no chamber of its own.
pub fn round_for_chamber(equipment: &Item, weapon: Id) -> Option<Id> {
    let chamber = equipment.find(weapon)?.kind.slots.iter().find(|slot| is_chamber(&slot.name))?;
    let worn = WITHIN_REACH.iter().filter_map(|slot| equipment.in_slot(slot));
    let takes = |item: &Item| item.count > 0 && item.kind.among(&chamber.takes) && !item.kind.among(&chamber.refuses);
    worn.flat_map(|holder| holder.grids.iter().flatten()).find(|(_, item)| takes(item)).map(|(_, item)| item.id)
}

/// The kinds of the first so many of those rounds, one for each round, in
/// the order a reload takes them (`AmmoPack.GetAmmoToReload`), nothing
/// taken.
pub fn rounds_in_reach_kinds(equipment: &Item, weapon: Id, count: usize, pack: Option<Id>) -> Vec<Arc<Kind>> {
    let stacks = rounds_within_reach_from(equipment, weapon, pack).into_iter().filter_map(|id| equipment.find(id));
    stacks.flat_map(|stack| std::iter::repeat_n(stack.kind.clone(), stack.count as usize)).take(count).collect()
}

/// Takes so many of those rounds for the weapon, the first stack first
/// (`AmmoPack.LoadAmmo`); a stack used up is gone. How many were taken.
pub fn take_rounds(equipment: &mut Item, weapon: Id, count: u32) -> u32 {
    take_rounds_of_kinds(equipment, weapon, count, None).iter().map(|(_, taken)| taken).sum()
}

/// The same, saying what was taken: each kind and how many of it, in the
/// order taken; of the reload's one stack where it was given one.
pub fn take_rounds_of_kinds(equipment: &mut Item, weapon: Id, count: u32, pack: Option<Id>) -> Vec<(Arc<Kind>, u32)> {
    let mut left = count;
    let mut taken_all = Vec::new();
    for id in rounds_within_reach_from(equipment, weapon, pack) {
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
    // A cylinder's into its empty chambers in turn.
    if is_cylinder(magazine) {
        for (kind, count) in rounds {
            for _ in 0..*count {
                into_cylinder(magazine, kind.clone());
            }
        }
        return true;
    }
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

/// The game's name for a weapon of a magazine of its own fitted with one
/// that comes out (`EReloadMode.ExternalMagazineWithInternalReloadSupport`).
pub const MIXED_RELOAD: &str = "ExternalMagazineWithInternalReloadSupport";

/// A weapon's way of being loaded as the game reads it with the magazine
/// fitted (`Weapon.ReloadMode`): its template's (`ReloadMode`), but one of
/// a magazine of its own (`InternalMagazine`) fitted with one that comes out
/// (`ReloadMagType` `ExternalMagazine`: every Model 700 magazine) is
/// `MIXED_RELOAD`. With no magazine fitted, the template's.
pub fn reload_mode(weapon: &Item) -> &str {
    let own = weapon.kind.reload_mode.as_str();
    match weapon.in_slot(MAGAZINE_SLOT) {
        Some(magazine) if own == "InternalMagazine" && magazine.kind.magazine_external => MIXED_RELOAD,
        _ => own,
    }
}

/// The rounds a magazine holds: a cylinder's, the rounds in its chambers
/// (`CylinderMagazine.Count`).
pub fn magazine_rounds(magazine: &Item) -> u32 {
    if is_cylinder(magazine) {
        return camoras(magazine).filter(|&index| magazine.slots.get(index).is_some_and(Option::is_some)).count() as u32;
    }
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
    // A cylinder's last loaded chambers emptied, or its empty ones filled
    // with the kind of its last round.
    if is_cylinder(magazine) {
        let full: Vec<usize> = camoras(magazine).filter(|&index| magazine.slots.get(index).is_some_and(Option::is_some)).collect();
        if count < held {
            for &index in full.iter().rev().take((held - count) as usize) {
                magazine.slots[index] = None;
            }
        } else if let Some(kind) = full.last().and_then(|&index| magazine.slots[index].as_ref()).map(|round| round.kind.clone()) {
            for _ in held..count {
                into_cylinder(magazine, kind.clone());
            }
        }
        return;
    }
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
