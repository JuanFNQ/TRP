//! The menu of a right click on a thing in the inventory, in a raid
//! (`docs/spec/inventory.md`, "The menu of a right click"): which lines it
//! shows, in the game's order (`InventoryItemContextInteractions`), and
//! whether each can be done now (`ItemContextInteractionsSwitcher`'s
//! `IsActive` and `IsInteractive`). Only the lines a raid can show are
//! here; a thing counts as examined.

use crate::inventory::{is, Id, Item};

/// A line of the menu (`EItemInfoButton`). Its name is also its caption's
/// key in the install's words and its icon's (`Characteristics/Icons/`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Line {
    Inspect,
    ViewMap,
    Reload,
    Load,
    Unload,
    Equip,
    Unequip,
    Overlook,
    Use,
    UseAll,
    Install,
    Uninstall,
    Open,
    UnloadAmmo,
    Examine,
    CheckMagazine,
    TurnOn,
    TurnOff,
    Fold,
    Unfold,
    Disassemble,
    Tag,
    ResetTag,
    WatchProfile,
    AddToWishlist,
    Discard,
}

impl Line {
    /// In the game's order.
    pub const ALL: [Line; 26] = [
        Line::Inspect,
        Line::ViewMap,
        Line::Reload,
        Line::Load,
        Line::Unload,
        Line::Equip,
        Line::Unequip,
        Line::Overlook,
        Line::Use,
        Line::UseAll,
        Line::Install,
        Line::Uninstall,
        Line::Open,
        Line::UnloadAmmo,
        Line::Examine,
        Line::CheckMagazine,
        Line::TurnOn,
        Line::TurnOff,
        Line::Fold,
        Line::Unfold,
        Line::Disassemble,
        Line::Tag,
        Line::ResetTag,
        Line::WatchProfile,
        Line::AddToWishlist,
        Line::Discard,
    ];

    /// The game's name for it.
    pub fn name(self) -> &'static str {
        match self {
            Line::Inspect => "Inspect",
            Line::ViewMap => "ViewMap",
            Line::Reload => "Reload",
            Line::Load => "Load",
            Line::Unload => "Unload",
            Line::Equip => "Equip",
            Line::Unequip => "Unequip",
            Line::Overlook => "Overlook",
            Line::Use => "Use",
            Line::UseAll => "UseAll",
            Line::Install => "Install",
            Line::Uninstall => "Uninstall",
            Line::Open => "Open",
            Line::UnloadAmmo => "UnloadAmmo",
            Line::Examine => "Examine",
            Line::CheckMagazine => "CheckMagazine",
            Line::TurnOn => "TurnOn",
            Line::TurnOff => "TurnOff",
            Line::Fold => "Fold",
            Line::Unfold => "Unfold",
            Line::Disassemble => "Disassemble",
            Line::Tag => "Tag",
            Line::ResetTag => "ResetTag",
            Line::WatchProfile => "WatchProfile",
            Line::AddToWishlist => "AddToWishlist",
            Line::Discard => "Discard",
        }
    }

    /// Whether it opens lines of its own as the mouse comes onto it
    /// (`SubInteractions`): of those a raid shows, the wish list's.
    pub fn has_lines(self) -> bool {
        self == Line::AddToWishlist
    }
}

/// What the menu goes by besides the thing itself.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    /// It is in one of the equipment's slots (`Equipped`).
    pub worn: bool,
    /// It lies in a slot of something that is not a container: a part
    /// fitted, a magazine in a weapon (`ModInstalled`).
    pub fitted: bool,
    /// It is the character's own (`Owner == _itemController`).
    pub own: bool,
    /// Something is in the hands, and the magazine of the weapon in them.
    pub something_in_hands: bool,
    pub magazine_in_hands: Option<Id>,
    /// Its magazine has been checked (`CheckedMagazine`).
    pub checked: bool,
    /// What health says of using it now (`HasPartsToApply`): none without
    /// health, else nothing or its error's key.
    pub health: Option<Result<(), &'static str>>,
}

/// Whether a line can be done now, and if not the key of the game's words
/// for why (shown in its tooltip; empty for none).
pub type Can = Result<(), &'static str>;

/// Every key of the game's words a line's reason may be.
pub const REASONS: [&str; 8] = [
    "You can't do this to weapon with internal magazine",
    "InventoryError/You can't unload ammo from an installed magazine",
    "InventoryError/You can't unload from this item",
    "You can't fold this item",
    "You can't disassemble equipped item",
    "Inventory/IncompatibleItem",
    "Health/ItemResourceDepleted",
    "Health/PlayerIsDead",
];

/// The key of the game's words for health's refusing a thing
/// (`HasPartsToApply`'s errors).
pub fn refusal_key(refused: crate::health::Refused) -> &'static str {
    match refused {
        crate::health::Refused::Dead => "Health/PlayerIsDead",
        crate::health::Refused::Incompatible => "Inventory/IncompatibleItem",
        crate::health::Refused::Depleted => "Health/ItemResourceDepleted",
    }
}

/// The lines of a thing's menu, in order, each with whether it can be
/// done now.
pub fn lines(item: &Item, facts: &Facts) -> Vec<(Line, Can)> {
    Line::ALL.into_iter().filter(|line| shown(item, facts, *line)).map(|line| (line, can(item, facts, line))).collect()
}

/// The weapon's magazine, where its slot holds one (`GetCurrentMagazine`).
fn magazine(item: &Item) -> Option<&Item> {
    item.kind.slots.iter().zip(&item.slots).find(|(slot, _)| slot.name == "mod_magazine").and_then(|(_, held)| held.as_ref())
}

/// A weapon whose magazine is part of it (`EReloadMode.InternalMagazine`).
fn internal(item: &Item) -> bool {
    item.kind.reload_mode == "InternalMagazine"
}

/// The thing and everything fitted to it, through its slots.
fn with_parts(item: &Item) -> Vec<&Item> {
    let mut all = vec![item];
    let mut at = 0;
    while at < all.len() {
        let next: Vec<&Item> = all[at].slots.iter().flatten().collect();
        all.extend(next);
        at += 1;
    }
    all
}

/// A part with a switch (`TogglableComponent`: armoured equipment with a
/// hinge, night and thermal vision).
fn switched(item: &Item) -> bool {
    item.kind.hinge || is(item, "NightVision") || is(item, "ThermalVision")
}

/// Whether the thing can be switched (`CanToggle`): something in it has a
/// switch, and a face shield is so only as a helmet.
fn can_toggle(item: &Item) -> bool {
    with_parts(item).into_iter().any(switched) && (!item.kind.face_shield || is(item, "Helmet"))
}

/// The first part that folds (`FoldableComponent`, in the thing or its
/// parts), and whether it can fold now (`CanBeFolded`: its folded slot,
/// where it names one, holds something).
fn foldable(item: &Item) -> Option<bool> {
    let part = with_parts(item).into_iter().find(|part| part.kind.foldable)?;
    let slot = &part.kind.folded_slot;
    Some(slot.is_empty() || part.kind.slots.iter().zip(&part.slots).any(|(held, what)| held.name == *slot && what.is_some()))
}

fn shown(item: &Item, facts: &Facts, line: Line) -> bool {
    let weapon = is(item, "Weapon");
    let wearable = weapon || is(item, "Knife") || is(item, "Equipment") || is(item, "SearchableItem");
    let food = is(item, "FoodDrink");
    let magazine_class = is(item, "Magazine");
    let rounds = item.loose.iter().map(|round| round.count).sum::<u32>();
    match line {
        Line::Inspect => true,
        Line::ViewMap => is(item, "Map"),
        Line::Reload => weapon,
        Line::Load => weapon && magazine(item).is_none(),
        Line::Unload => weapon && magazine(item).is_some() && !internal(item),
        Line::Equip => wearable && !facts.worn,
        Line::Unequip => wearable && facts.worn,
        Line::Overlook => item.kind.quest,
        Line::Use => {
            facts.health.is_some()
                && if food {
                    item.kind.max_resource > 1.0
                } else {
                    is(item, "Meds") && (item.kind.max_hp_resource <= 0.0 || item.kind.max_hp_resource >= 1.0)
                }
        }
        Line::UseAll => facts.health.is_some() && food && item.kind.max_resource >= 1.0,
        Line::Install => magazine_class && !facts.fitted,
        Line::Uninstall => magazine_class && facts.fitted,
        Line::Open => is(item, "SearchableItem") || is(item, "LockableContainer"),
        Line::UnloadAmmo => magazine_class || is(item, "AmmoBox"),
        // Every thing counts as examined.
        Line::Examine => false,
        Line::CheckMagazine => magazine_class && facts.something_in_hands && !facts.checked && facts.magazine_in_hands != Some(item.id) && rounds > 0,
        // Nothing is switched on or folded here: each stands as made.
        Line::TurnOn => can_toggle(item),
        Line::TurnOff => false,
        Line::Fold => foldable(item) == Some(true),
        Line::Unfold => false,
        Line::Disassemble => weapon && facts.own,
        Line::Tag => magazine_class || is(item, "LockableContainer"),
        // Nothing is tagged here.
        Line::ResetTag => false,
        // A dog tag's owner's profile is outside a raid's.
        Line::WatchProfile => false,
        Line::AddToWishlist => !item.kind.quest && !is(item, "Money") && !is(item, "SpecItem") && !is(item, "MobContainer"),
        Line::Discard => true,
    }
}

fn can(item: &Item, facts: &Facts, line: Line) -> Can {
    match line {
        Line::Reload | Line::Load | Line::Unload => {
            let mode = item.kind.reload_mode.as_str();
            if mode != "ExternalMagazine" && mode != "ExternalMagazineWithInternalReloadSupport" {
                return Err("You can't do this to weapon with internal magazine");
            }
            Ok(())
        }
        Line::Use | Line::UseAll => facts.health.unwrap_or(Err("")),
        Line::UnloadAmmo => {
            if facts.fitted {
                return Err("InventoryError/You can't unload ammo from an installed magazine");
            }
            if item.loose.iter().map(|round| round.count).sum::<u32>() == 0 {
                return Err("InventoryError/You can't unload from this item");
            }
            Ok(())
        }
        Line::Fold | Line::Unfold => {
            if foldable(item) != Some(true) {
                return Err("You can't fold this item");
            }
            Ok(())
        }
        Line::Disassemble if facts.worn => Err("You can't disassemble equipped item"),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::Kind;
    use trp_data::inventory::Holder;
    use std::sync::Arc;

    fn thing(classes: &[&str], tweak: impl FnOnce(&mut Kind)) -> Item {
        let named: Vec<String> = classes.iter().map(|class| (*class).to_owned()).chain(["Item".to_owned()]).collect();
        let mut kind = Kind { template: "t".into(), name: "t".into(), classes: named.clone(), class_names: named, cells: [1, 1], ..Kind::default() };
        tweak(&mut kind);
        Item::new(Arc::new(kind))
    }

    fn names(lines: &[(Line, Can)]) -> Vec<&'static str> {
        lines.iter().map(|(line, _)| line.name()).collect()
    }

    #[test]
    fn a_medkit_in_a_raid_has_use_and_no_heal_all() {
        let kit = thing(&["MedKit", "Meds"], |kind| kind.max_hp_resource = 400.0);
        let facts = Facts { own: true, health: Some(Ok(())), ..Facts::default() };
        assert_eq!(names(&lines(&kit, &facts)), ["Inspect", "Use", "AddToWishlist", "Discard"]);
        // Nothing to help: the line is there, dimmed, with health's reason.
        let facts = Facts { health: Some(Err("Inventory/IncompatibleItem")), ..facts };
        assert_eq!(lines(&kit, &facts)[1], (Line::Use, Err("Inventory/IncompatibleItem")));
        // With health switched off, no use at all.
        assert_eq!(names(&lines(&kit, &Facts::default())), ["Inspect", "AddToWishlist", "Discard"]);
    }

    #[test]
    fn food_has_use_only_with_a_whole_above_one() {
        let water = thing(&["Drink", "FoodDrink"], |kind| kind.max_resource = 60.0);
        let facts = Facts { health: Some(Ok(())), ..Facts::default() };
        assert_eq!(names(&lines(&water, &facts)), ["Inspect", "Use", "UseAll", "AddToWishlist", "Discard"]);
        let candy = thing(&["Food", "FoodDrink"], |kind| kind.max_resource = 1.0);
        assert_eq!(names(&lines(&candy, &facts)), ["Inspect", "UseAll", "AddToWishlist", "Discard"]);
    }

    #[test]
    fn a_weapon_s_lines_go_by_its_magazine_and_where_it_is() {
        let mut rifle = thing(&["AssaultRifle", "Weapon"], |kind| {
            kind.reload_mode = "ExternalMagazine".into();
            kind.slots = vec![Holder { name: "mod_magazine".into(), cells: [0, 0], takes: Vec::new(), refuses: Vec::new(), merges: None, required: false }];
        });
        rifle.slots = vec![None];
        let worn = Facts { worn: true, own: true, ..Facts::default() };
        assert_eq!(names(&lines(&rifle, &worn)), ["Inspect", "Reload", "Load", "Unequip", "Disassemble", "AddToWishlist", "Discard"]);
        assert_eq!(lines(&rifle, &worn)[4], (Line::Disassemble, Err("You can't disassemble equipped item")));
        rifle.slots = vec![Some(thing(&["Magazine"], |_| {}))];
        let loose = Facts { own: true, ..Facts::default() };
        assert_eq!(names(&lines(&rifle, &loose)), ["Inspect", "Reload", "Unload", "Equip", "Disassemble", "AddToWishlist", "Discard"]);
        // A tube: no unloading, and the reload's lines dimmed.
        let shotgun = thing(&["Shotgun", "Weapon"], |kind| kind.reload_mode = "InternalMagazine".into());
        let shown = lines(&shotgun, &loose);
        assert_eq!(shown[1], (Line::Reload, Err("You can't do this to weapon with internal magazine")));
    }

    #[test]
    fn a_magazine_is_checked_only_from_the_screen_with_something_in_hand() {
        let mut magazine = thing(&["Magazine"], |_| {});
        let mut round = thing(&["Ammo"], |_| {});
        round.count = 30;
        magazine.loose = vec![round];
        let hands = Facts { own: true, something_in_hands: true, ..Facts::default() };
        assert_eq!(names(&lines(&magazine, &hands)), ["Inspect", "Install", "UnloadAmmo", "CheckMagazine", "Tag", "AddToWishlist", "Discard"]);
        // In the weapon in hand: neither checked from here nor unloaded.
        let fitted = Facts { fitted: true, magazine_in_hands: Some(magazine.id), ..hands };
        let shown = lines(&magazine, &fitted);
        assert_eq!(names(&shown), ["Inspect", "Uninstall", "UnloadAmmo", "Tag", "AddToWishlist", "Discard"]);
        assert_eq!(shown[2], (Line::UnloadAmmo, Err("InventoryError/You can't unload ammo from an installed magazine")));
    }

    #[test]
    fn a_secured_container_is_never_wished_for() {
        let alpha = thing(&["MobContainer", "SearchableItem"], |_| {});
        assert_eq!(names(&lines(&alpha, &Facts { worn: true, ..Facts::default() })), ["Inspect", "Unequip", "Open", "Discard"]);
    }
}
