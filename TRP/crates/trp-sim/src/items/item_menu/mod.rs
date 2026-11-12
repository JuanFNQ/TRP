//! The menu of a right click on a thing in the inventory, in a raid.
//! After `docs/spec/inventory.md`, "The menu of a right click": which lines it
//! shows, in the game's order (`InventoryItemContextInteractions`), and
//! whether each can be done now (`ItemContextInteractionsSwitcher`'s
//! `IsActive` and `IsInteractive`). Only the lines a raid can show are
//! here; a thing not known shows only Inspect, Examine and Discard.

use crate::items::inventory::{is, Id, Item};

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
    /// Turning its fold would not fit where it lies: a weapon in a grid
    /// without the room to be its new size there (`ItemManipulator.Fold`
    /// refused, `Can't fold stock now`).
    pub fold_refused: bool,
    /// What health says of using it now (`HasPartsToApply`): none without
    /// health, else nothing or its error's key.
    pub health: Option<Result<(), &'static str>>,
    /// Its kind is not known (`examine`), and it is being examined now.
    pub unknown: bool,
    pub examining: bool,
}

/// Whether a line can be done now, and if not the key of the game's words
/// for why (shown in its tooltip; empty for none).
pub type Can = Result<(), &'static str>;

/// Every key of the game's words a line's reason may be (looked up
/// whatever its case, as the game's locale is: the install keeps the wish
/// list's first in lower case).
pub const REASONS: [&str; 13] = [
    "Item is being examined",
    "InventoryErrors/MalfunctionError",
    "Already in this group",
    "Not in wishlist",
    "You can't do this to weapon with internal magazine",
    "InventoryError/You can't unload ammo from an installed magazine",
    "InventoryError/You can't unload from this item",
    "You can't fold this item",
    "Can't fold stock now",
    "You can't disassemble equipped item",
    "Inventory/IncompatibleItem",
    "Health/ItemResourceDepleted",
    "Health/PlayerIsDead",
];

/// The key of the game's words for health's refusing a thing
/// (`HasPartsToApply`'s errors).
pub fn refusal_key(refused: crate::condition::health::Refused) -> &'static str {
    match refused {
        crate::condition::health::Refused::Dead => "Health/PlayerIsDead",
        crate::condition::health::Refused::Incompatible => "Inventory/IncompatibleItem",
        crate::condition::health::Refused::Depleted => "Health/ItemResourceDepleted",
    }
}

/// The lines of a thing's menu, in order, each with whether it can be
/// done now.
pub fn lines(item: &Item, facts: &Facts) -> Vec<(Line, Can)> {
    Line::ALL.into_iter().filter(|line| shown(item, facts, *line)).map(|line| (line, can(item, facts, line))).collect()
}

/// A group of the wish list (`EWishlistGroup`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WishGroup {
    Quests,
    Hideout,
    Trading,
    Equipment,
    Other,
}

impl WishGroup {
    /// The game's name for it.
    pub fn name(self) -> &'static str {
        match self {
            WishGroup::Quests => "Quests",
            WishGroup::Hideout => "Hideout",
            WishGroup::Trading => "Trading",
            WishGroup::Equipment => "Equipment",
            WishGroup::Other => "Other",
        }
    }
}

/// The wish list (`WishlistManager`'s own): kinds of thing, by template,
/// each in one group.
pub type Wishlist = std::collections::HashMap<String, WishGroup>;

/// A line of the wish list's own menu (`WishlistContextInteractions.
/// EWishlistInteractions`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WishLine {
    Group(WishGroup),
    Remove,
}

impl WishLine {
    /// In the game's order.
    pub const ALL: [WishLine; 6] = [
        WishLine::Group(WishGroup::Quests),
        WishLine::Group(WishGroup::Equipment),
        WishLine::Group(WishGroup::Hideout),
        WishLine::Group(WishGroup::Other),
        WishLine::Group(WishGroup::Trading),
        WishLine::Remove,
    ];

    /// The game's name for it, which its template goes by
    /// (`SimpleSpecifiedContextButtons`) and Remove's icon
    /// (`Characteristics/Icons/`).
    pub fn name(self) -> &'static str {
        match self {
            WishLine::Group(group) => group.name(),
            WishLine::Remove => "RemoveFromWishlist",
        }
    }

    /// Its caption's key in the install's words (`_customNames`).
    pub fn caption_key(self) -> &'static str {
        match self {
            WishLine::Group(WishGroup::Quests) => "EWishlistGroup/Quests",
            WishLine::Group(WishGroup::Hideout) => "EWishlistGroup/Hideout",
            WishLine::Group(WishGroup::Trading) => "EWishlistGroup/Trading",
            WishLine::Group(WishGroup::Equipment) => "EWishlistGroup/Equipment",
            WishLine::Group(WishGroup::Other) => "EWishlistGroup/Other",
            WishLine::Remove => "Wishlist/RemoveFromWishlist",
        }
    }
}

/// The wish list's own lines for a kind of thing, in order, each with
/// whether it can be done (`WishlistContextInteractions.IsInteractive`): a
/// group's not where the kind is in that group already, Remove's not
/// where it is in none.
pub fn wish_lines(template: &str, list: &Wishlist) -> Vec<(WishLine, Can)> {
    WishLine::ALL
        .into_iter()
        .map(|line| {
            let can = match line {
                WishLine::Group(group) if list.get(template) == Some(&group) => Err("Already in this group"),
                WishLine::Remove if !list.contains_key(template) => Err("Not in wishlist"),
                _ => Ok(()),
            };
            (line, can)
        })
        .collect()
}

/// Does a line of the wish list's menu for a kind of thing
/// (`ItemUiContext.AddToWishList`, `RemoveFromWishList`, `WishlistManager`):
/// a group's puts the kind in that group, or moves it there from another;
/// Remove takes it off. Whether the list changed.
pub fn wish(list: &mut Wishlist, template: &str, line: WishLine) -> bool {
    match line {
        WishLine::Group(group) => list.insert(template.to_owned(), group) != Some(group),
        WishLine::Remove => list.remove(template).is_some(),
    }
}

/// The weapon's magazine, where its slot holds one (`GetCurrentMagazine`).
fn magazine(item: &Item) -> Option<&Item> {
    item.kind.slots.iter().zip(&item.slots).find(|(slot, _)| slot.name == "mod_magazine").and_then(|(_, held)| held.as_ref())
}

/// A weapon whose magazine is part of it (`EReloadMode.InternalMagazine`),
/// as the game reads its mode with the magazine fitted (`InternalMag`:
/// `Weapon.ReloadMode`; the Model 700 with a magazine in is not).
fn internal(item: &Item) -> bool {
    crate::items::inventory::reload_mode(item) == "InternalMagazine"
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

/// Whether the thing can be switched (`CanToggle`): something in it has a
/// switch, and a face shield is so only as a helmet.
fn can_toggle(item: &Item) -> bool {
    with_parts(item).into_iter().any(crate::items::inventory::has_switch) && (!item.kind.face_shield || is(item, "Helmet"))
}

/// How the thing's switch stands (`Togglable.On`).
fn switched_on(item: &Item) -> bool {
    crate::items::inventory::switched_part(item).is_some_and(|part| part.on)
}

/// How the thing's fold stands, where it has one that can be turned
/// (`Foldable.CanBeFolded`, `Folded`: the first part that folds, in the
/// thing or its parts).
fn folded(item: &Item) -> Option<bool> {
    crate::items::inventory::can_be_folded(item).then(|| crate::items::inventory::folding_part(item).is_some_and(|part| part.folded))
}

fn shown(item: &Item, facts: &Facts, line: Line) -> bool {
    let weapon = is(item, "Weapon");
    let wearable = weapon || is(item, "Knife") || is(item, "Equipment") || is(item, "SearchableItem");
    let food = is(item, "FoodDrink");
    let magazine_class = is(item, "Magazine");
    let rounds = item.loose.iter().map(|round| round.count).sum::<u32>();
    // A thing not known shows only Inspect, Examine and Discard
    // (`ItemContextInteractionsSwitcher.IsActive`).
    if facts.unknown && !matches!(line, Line::Inspect | Line::Examine | Line::Discard) {
        return false;
    }
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
        Line::Open => is(item, "SearchableItem") || is(item, "LockableContainer") || is(item, "SimpleContainer"),
        Line::UnloadAmmo => magazine_class || is(item, "AmmoBox"),
        // A thing not known, wherever it lies.
        Line::Examine => facts.unknown,
        Line::CheckMagazine => magazine_class && facts.something_in_hands && !facts.checked && facts.magazine_in_hands != Some(item.id) && rounds > 0,
        Line::TurnOn => can_toggle(item) && !switched_on(item),
        Line::TurnOff => can_toggle(item) && switched_on(item),
        Line::Fold => folded(item) == Some(false),
        Line::Unfold => folded(item) == Some(true),
        Line::Disassemble => weapon && facts.own,
        Line::Tag => crate::items::inventory::takes_tag(item),
        // Nothing is tagged here.
        Line::ResetTag => crate::items::inventory::takes_tag(item) && !item.tag.name.is_empty(),
        // A dog tag's owner's profile is outside a raid's.
        Line::WatchProfile => false,
        Line::AddToWishlist => !item.kind.quest && !is(item, "Money") && !is(item, "SpecItem") && !is(item, "MobContainer"),
        Line::Discard => true,
    }
}

fn can(item: &Item, facts: &Facts, line: Line) -> Can {
    match line {
        Line::Reload | Line::Load | Line::Unload => {
            let mode = crate::items::inventory::reload_mode(item);
            if mode != "ExternalMagazine" && mode != crate::items::inventory::MIXED_RELOAD {
                return Err("You can't do this to weapon with internal magazine");
            }
            // A malfunction the shooter knows of: a failure to feed, or any
            // on a weapon that loads its chamber alone too (`ItemContextInteractionsSwitcher`).
            let malf = item.malfunction.malf;
            if item.malfunction.known && (malf == crate::weapons::malfunction::Malf::Feed || (malf != crate::weapons::malfunction::Malf::None && mode == crate::items::inventory::MIXED_RELOAD)) {
                return Err("InventoryErrors/MalfunctionError");
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
            if !crate::items::inventory::can_fold(item) {
                return Err("You can't fold this item");
            }
            if facts.fold_refused {
                return Err("Can't fold stock now");
            }
            Ok(())
        }
        Line::Disassemble if facts.worn => Err("You can't disassemble equipped item"),
        Line::Examine if facts.examining => Err("Item is being examined"),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests;
