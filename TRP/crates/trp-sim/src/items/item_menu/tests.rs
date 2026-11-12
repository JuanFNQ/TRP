//! Tests of the right click's menu: which lines a thing shows and whether each can be done.
use super::*;
use crate::items::inventory::Kind;
use trp_data::character::inventory::Holder;
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

/// A helmet's face shield is switched from the helmet's menu: FLIP DOWN
/// while it is up, FLIP UP once it is down; the shield's own menu has
/// neither (`CanToggle`).
#[test]
fn a_face_shield_is_flipped_from_its_helmet() {
    let shield = thing(&["FaceShield", "ArmoredEquipment"], |kind| {
        kind.hinge = true;
        kind.face_shield = true;
    });
    let shield_id = shield.id;
    let mut helmet = thing(&["Helmet", "ArmoredEquipment"], |kind| kind.slots = vec![Holder { name: "mod_equipment".into(), ..Holder::default() }]);
    helmet.slots[0] = Some(shield.clone());
    let facts = Facts { own: true, worn: true, ..Facts::default() };
    let switches = |item: &Item| names(&lines(item, &facts)).into_iter().filter(|name| name.starts_with("Turn")).collect::<Vec<_>>();
    assert_eq!(switches(&helmet), ["TurnOn"]);
    assert!(switches(&shield).is_empty());
    let helmet_id = helmet.id;
    // Its hinge goes as the shield's switch, and is down with none.
    assert!(!crate::items::inventory::hinge_down(&helmet));
    assert_eq!(crate::items::inventory::toggle(&mut helmet, helmet_id), Some((shield_id, true)));
    assert_eq!(switches(&helmet), ["TurnOff"]);
    assert!(crate::items::inventory::hinge_down(&helmet));
    assert_eq!(crate::items::inventory::toggle(&mut helmet, helmet_id), Some((shield_id, false)));
    assert_eq!(switches(&helmet), ["TurnOn"]);
    helmet.slots[0] = None;
    assert!(crate::items::inventory::hinge_down(&helmet));
}

/// The wish list's own lines: each group's dimmed where the kind is in
/// it already, Remove's where it is in none; a group's line puts the
/// kind in that group or moves it there, Remove takes it off.
#[test]
fn the_wish_list_s_lines_go_by_the_kind_s_group() {
    let mut list = Wishlist::new();
    let dimmed = |list: &Wishlist| wish_lines("ak", list).into_iter().filter(|(_, can)| can.is_err()).map(|(line, _)| line.name()).collect::<Vec<_>>();
    assert_eq!(wish_lines("ak", &list).iter().map(|(line, _)| line.name()).collect::<Vec<_>>(), ["Quests", "Equipment", "Hideout", "Other", "Trading", "RemoveFromWishlist"]);
    assert_eq!(dimmed(&list), ["RemoveFromWishlist"]);
    assert!(wish(&mut list, "ak", WishLine::Group(WishGroup::Equipment)));
    assert_eq!(dimmed(&list), ["Equipment"]);
    assert!(!wish(&mut list, "ak", WishLine::Group(WishGroup::Equipment)));
    assert!(wish(&mut list, "ak", WishLine::Group(WishGroup::Trading)));
    assert_eq!((list.len(), dimmed(&list)), (1, vec!["Trading"]));
    assert!(wish(&mut list, "ak", WishLine::Remove));
    assert!(!wish(&mut list, "ak", WishLine::Remove));
    assert_eq!(dimmed(&list), ["RemoveFromWishlist"]);
}

/// A weapon with a stock that folds offers FOLD while unfolded and
/// UNFOLD once folded; a part that blocks folding, or no room for the
/// new size where it lies, dims the line with the game's reason.
#[test]
fn a_fold_is_offered_by_how_it_stands() {
    let slots = vec![Holder { name: "mod_stock".into(), ..Holder::default() }, Holder { name: "mod_barrel".into(), ..Holder::default() }];
    let mut gun = thing(&["Weapon"], |kind| {
        kind.slots = slots;
        kind.merges = true;
    });
    gun.slots[0] = Some(thing(&["Stock", "Mod"], |kind| kind.foldable = true));
    let facts = Facts { own: true, ..Facts::default() };
    let folds = |gun: &Item, facts: &Facts| lines(gun, facts).into_iter().filter(|(line, _)| matches!(line, Line::Fold | Line::Unfold)).collect::<Vec<_>>();
    assert_eq!(folds(&gun, &facts), [(Line::Fold, Ok(()))]);
    assert_eq!(folds(&gun, &Facts { fold_refused: true, ..facts.clone() }), [(Line::Fold, Err("Can't fold stock now"))]);
    let id = gun.id;
    crate::items::inventory::fold(&mut [&mut gun], id).unwrap();
    assert_eq!(folds(&gun, &facts), [(Line::Unfold, Ok(()))]);
    gun.slots[1] = Some(thing(&["Barrel", "Mod"], |kind| kind.blocks_folding = true));
    assert_eq!(folds(&gun, &facts), [(Line::Unfold, Err("You can't fold this item"))]);
}

/// A magazine, a lockable and a simple container take a tag: Tag
/// always, Reset tag once it is named; a simple container opens.
#[test]
fn a_tag_is_offered_and_reset_when_named() {
    let facts = Facts { own: true, ..Facts::default() };
    let tags = |item: &Item| names(&lines(item, &facts)).into_iter().filter(|name| name.ends_with("Tag") || *name == "Open").collect::<Vec<_>>();
    let mut case = thing(&["SimpleContainer"], |_| {});
    assert_eq!(tags(&case), ["Open", "Tag"]);
    let id = case.id;
    assert!(crate::items::inventory::set_tag(&mut case, id, "ammo", 2));
    assert_eq!(tags(&case), ["Open", "Tag", "ResetTag"]);
    let mut rifle = thing(&["Weapon"], |_| {});
    let rifle_id = rifle.id;
    assert!(tags(&rifle).is_empty() && !crate::items::inventory::set_tag(&mut rifle, rifle_id, "no", 0));
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
        kind.slots = vec![Holder { name: "mod_magazine".into(), cells: [0, 0], takes: Vec::new(), refuses: Vec::new(), merges: None, required: false, armour: false, locked: false, plate: None }];
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
