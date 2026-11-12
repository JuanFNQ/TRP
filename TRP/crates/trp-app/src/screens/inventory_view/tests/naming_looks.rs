//! Tests of item views, blinking, tooltips, refusals and the names of slots and weapons.

use super::*;

/// A thing's view shows a repair buff on it or on anything in it, a
/// rare one first (`GridItemView.TryGetBuff`); a thing not known
/// lists no buff row and nothing enhanced (`unknown_figures`).
#[test]
fn a_view_shows_the_rare_buff_first_and_an_unknown_thing_none() {
    use trp_sim::items::repair::{BuffKind, Rarity, RepairBuff};
    let common = RepairBuff { rarity: Rarity::Common, kind: BuffKind::DamageReduction, value: 0.95, threshold: 30.0 };
    let rare = RepairBuff { rarity: Rarity::Rare, ..common };
    let kind = |slots: usize| Arc::new(Kind { slots: vec![Default::default(); slots], ..Kind::default() });
    let mut vest = Item::new(kind(2));
    assert_eq!(shown_buff(&vest), None);
    let mut front = Item::new(kind(0));
    front.buff = Some(common);
    vest.slots[0] = Some(front);
    assert_eq!(shown_buff(&vest), Some(common), "a plate's");
    let mut back = Item::new(kind(0));
    back.buff = Some(rare);
    vest.slots[1] = Some(back);
    vest.buff = Some(common);
    assert_eq!(shown_buff(&vest), Some(rare), "the rare first");
    let row = |id: &'static str, enhanced: bool| trp_sim::items::item_attributes::Row { id, table: Default::default(), name: String::new(), value: Some("5 %".into()), show: trp_sim::items::item_attributes::Show::Compact, colored: None, less_is_good: false, tip: Vec::new(), enhanced };
    let mut figures = trp_sim::items::item_attributes::Figures { rows: vec![row("CenterOfImpact", true), row("WeaponSpreadCommonBuff", true)], ..Default::default() };
    unknown_figures(&mut figures);
    assert_eq!(figures.rows.iter().map(|row| (row.id, row.enhanced)).collect::<Vec<_>>(), [("CenterOfImpact", false)]);
}

/// A thing examined blinks as `courses10`'s `dead-scav-examine`'s book:
/// its clock from frame 680 (5.978), released on 681; 0.722 on 681
/// (5.987), 0.3225 on 700 (6.150), its fade done on 740 (6.484), the
/// rise from there: 0.0331 on 741 (6.492), 0.2194 on 746 (6.542),
/// 0.2488 on 747 (6.551).
/// A weapon being reloaded keeps the picture it had as the reload began,
/// until its key is let go at the reload's end (`Carried::reload_key`: the
/// game's views refresh on the move's success, which a reload raises as it
/// ends). Any other thing is shown by its own key meanwhile.
#[test]
fn a_reloaded_weapon_keeps_its_picture_until_the_reload_ends() {
    let (mut carried, [bag, rifle, ..]) = carried();
    let (pack, gun) = (carried.find(bag).expect("the bag").clone(), carried.find(rifle).expect("the rifle").clone());
    let own = crate::screens::icons::picture_key(&gun);
    assert_eq!(shown_key(&carried, &gun), own);
    carried.reload_key = Some((rifle, "as the reload began".to_owned()));
    assert_eq!(shown_key(&carried, &gun), "as the reload began");
    assert_eq!(shown_key(&carried, &pack), crate::screens::icons::picture_key(&pack));
    carried.reload_key = None;
    assert_eq!(shown_key(&carried, &gun), own);
}

#[test]
fn an_examined_thing_blinks_as_the_game() {
    let mut tween = BlinkTween { from: EXAMINED_BLINK_FROM, to: 0.0, began: 5.978 };
    let frames = [(5.987, 0.722), (6.150, 0.3225), (6.476, 0.0), (6.484, 0.0), (6.492, 0.0331), (6.542, 0.2194), (6.551, 0.2488)];
    for (at, take) in frames {
        let alpha = tween.step(at);
        assert!((alpha - take).abs() < 0.003, "at {at}: {alpha} against {take}");
    }
    assert_eq!((tween.from, tween.to), (0.0, 1.0), "rising");
}

/// `courses10`'s `drag-and-drop`, at 2560 by 1440 (4/3 pixels a unit,
/// a cell 84): the backpack's 4 by 5 grid at 1051.59, 865.33.
#[test]
fn a_turn_keeps_the_corner_from_the_last_move() {
    let (origin, cells, u) = (Vec2::new(1051.59, 865.33), [4, 5], 4.0 / 3.0);
    let cell = CELL * u;
    let corner = |cursor: Vec2, [w, h]: [f32; 2]| {
        let half = (Vec2::new(w, h) * CELL + Vec2::ONE) * u / 2.0;
        Vec2::new(cursor.x - half.x, cursor.y + half.y)
    };
    // Frame 2686: the M4A1 held turned (2 by 6) at 1307, 1094, turned
    // back to 6 by 2 by `R` with no move: lit at (2, 4).
    let kept = corner(Vec2::new(1307.0, 1094.0), [2.0, 6.0]);
    assert_eq!(grid_location(kept, origin, cell, 2, cells), (2, 4));
    // Frame 2713: the mouse moved to 1307, 1095: (0, 2).
    let moved = corner(Vec2::new(1307.0, 1095.0), [6.0, 2.0]);
    assert_eq!(grid_location(moved, origin, cell, 2, cells), (0, 2));
    // Frame 2926: the bottle (1 by 2) at 1104, 945 turned by `R` and let
    // go unmoved: put at (0, 1), where its middle would give (0, 0).
    let kept = corner(Vec2::new(1104.0, 945.0), [1.0, 2.0]);
    assert_eq!(grid_location(kept, origin, cell, 1, cells), (0, 1));
    let centred = corner(Vec2::new(1104.0, 945.0), [2.0, 1.0]);
    assert_eq!(grid_location(centred, origin, cell, 1, cells), (0, 0));
}

/// A slot's refusal at the mouse in the game's words, red; none where
/// the game closes it (`courses10`'s `drag-and-drop`: the M4A1 over the
/// holster, `<color=red>Item is incompatible</color>`).
#[test]
fn a_refusal_says_the_game_s_words_in_red() {
    let (mut carried, _) = carried();
    std::sync::Arc::make_mut(&mut carried.texts.words).insert("inventory errors/item is incompatible".into(), "Item is incompatible".into());
    assert_eq!(refusal_tip(&Refusal::Filter, &carried, Over::Slot).as_deref(), Some("<color=red>Item is incompatible</color>"));
    // A key the install lacks is said as it is (`Localized`).
    assert_eq!(refusal_tip(&Refusal::Locked, &carried, Over::Slot).as_deref(), Some("<color=red>Inventory Errors/Slot is locked</color>"));
    assert_eq!(refusal_tip(&Refusal::AlreadyThere, &carried, Over::Slot), None);
    assert_eq!(refusal_tip(&Refusal::NoAction, &carried, Over::Slot), None);
    // Over a grid its own filter and a magazine's refusals are said; a
    // place taken, a thing that does not fit and no room are not.
    std::sync::Arc::make_mut(&mut carried.texts.words).insert("inventory errors/magazine is full".into(), "Magazine is full".into());
    std::sync::Arc::make_mut(&mut carried.texts.words).insert("inventory errors/ammo is incompatible".into(), "Wrong ammo type".into());
    assert_eq!(refusal_tip(&Refusal::Filter, &carried, Over::Grid).as_deref(), Some("<color=red>Item is incompatible</color>"));
    assert_eq!(refusal_tip(&Refusal::MagazineFull, &carried, Over::Grid).as_deref(), Some("<color=red>Magazine is full</color>"));
    assert_eq!(refusal_tip(&Refusal::WrongAmmo, &carried, Over::Grid).as_deref(), Some("<color=red>Wrong ammo type</color>"));
    for silent in [Refusal::Taken, Refusal::OutsideGrid, Refusal::NoRoom, Refusal::AlreadyThere, Refusal::NoAction] {
        assert_eq!(refusal_tip(&silent, &carried, Over::Grid), None, "{silent:?}");
    }
    assert!(refusal_tip(&Refusal::Taken, &carried, Over::Slot).is_some(), "a slot says its place is taken");
}

/// A weapon's tooltip (`GridItemView.ShowTooltip`): with a malfunction
/// known, at once and in red, the look asked for while its kind is not
/// known, then the fix by the kind's name, in the install's words; with
/// none, a magazine whose top round the chamber refuses, at once and in
/// red, the round and the caliber in white; else its name after 0.6 s.
#[test]
fn a_jammed_or_misfed_weapon_s_tooltip_is_red_at_once() {
    let (mut carried, [_, rifle, ..]) = carried();
    std::sync::Arc::make_mut(&mut carried.texts.words).extend([
        ("notification/malfunctionrepairrequired".to_owned(), "Fix malfunction: {0}".to_owned()),
        ("emalfunctionstate/jam".to_owned(), "Failure to eject".to_owned()),
    ]);
    let (said, wait) = carried.tooltip_words(carried.find(rifle).unwrap());
    assert_eq!(wait, ITEM_TIP_DELAY, "a plain weapon by its name");
    assert!(!said.contains("<color"), "{said}");
    let gun = carried.loot.find_mut(rifle).unwrap();
    gun.malfunction.malf = trp_sim::weapons::malfunction::Malf::Jam;
    assert_eq!(carried.tooltip_words(carried.find(rifle).unwrap()).1, ITEM_TIP_DELAY, "a malfunction not known says nothing");
    carried.loot.find_mut(rifle).unwrap().malfunction.known = true;
    assert_eq!(carried.tooltip_words(carried.find(rifle).unwrap()), ("<color=red>Malfunction! Examine your weapon</color>".to_owned(), 0.0));
    carried.loot.find_mut(rifle).unwrap().malfunction.type_known = true;
    assert_eq!(carried.tooltip_words(carried.find(rifle).unwrap()), ("<color=red>Fix malfunction: Failure to eject</color>".to_owned(), 0.0));
    // Rounds the chamber does not take on the magazine's top.
    let round = |name: &str, count: u32| Item { count, ..Item::new(Arc::new(Kind { stack_max: 60, ..kind(name, &["Ammo"], [1, 1]) })) };
    let slots = vec![holder(trp_sim::items::inventory::MAGAZINE_SLOT, [0, 0], "Magazine"), holder("patron_in_weapon", [0, 0], "fits")];
    let mut gun = Item::new(Arc::new(Kind { slots, caliber: "Caliber556x45NATO".into(), ..kind("m4", &["Weapon"], [4, 1]) }));
    let mut magazine = Item::new(Arc::new(kind("magazine", &["Magazine"], [1, 2])));
    magazine.loose = vec![round("fits", 2), round("other", 28)];
    gun.slots[0] = Some(magazine.clone());
    let words = "<color=red>Ammo (<color=white>other</color>) is not compatible. Need: <color=white>Caliber556x45NATO</color></color>";
    assert_eq!(carried.weapon_warning(&gun).as_deref(), Some(words));
    magazine.loose.reverse();
    gun.slots[0] = Some(magazine);
    assert_eq!(carried.weapon_warning(&gun), None, "a round it takes on top");
}

pub(super) fn kind(name: &str, classes: &[&str], cells: [u32; 2]) -> Kind {
    let named: Vec<String> = std::iter::once(name).chain(classes.iter().copied()).chain(["Item"]).map(String::from).collect();
    Kind { template: name.into(), name: name.into(), classes: named.clone(), class_names: named, weight: 0.1, cells, ..Kind::default() }
}

/// A slot is named by the first kind of weapon part its name holds, in
/// the game's order; a name that holds none is its own.
#[test]
fn a_slot_is_named_as_the_game_names_it() {
    assert_eq!(slot_name("mod_mount_004"), "mod_mount");
    assert_eq!(slot_name("mod_tactical_2"), "mod_tactical");
    assert_eq!(slot_name("mod_muzzle_001"), "mod_muzzle");
    assert_eq!(slot_name("patron_in_weapon_000"), "patron_in_weapon");
    assert_eq!(slot_name("Soft_armor_front"), "Soft_armor_front");
    assert_eq!(slot_name("mod_charge"), "mod_charge");
}

/// A weapon's name with its build's (`ItemFactory.BriefItemName`): the
/// stock M4A1's own preset renames it ("Carbine"); `courses10`'s
/// `hover-tooltips` M4A1, built as the install's T.O 2K24, is "Colt
/// M4A1 5.56x45 assault rifle T.O 2K24" (frame 2633); anything but a
/// weapon keeps its name; a build's name past 30 letters is cut.
#[test]
fn a_weapon_is_named_with_its_build() {
    let Ok(install) = trp_data::Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let db = trp_data::Database::load(&install).expect("database");
    let texts = trp_data::character::inventory::item_texts(&install, "en").expect("texts");
    let named = named_builds(&db, &texts);
    let name = "Colt M4A1 5.56x45 assault rifle".to_owned();
    let build = |id: &str| {
        let preset = db.presets.iter().find(|preset| preset.id == id).expect("the preset");
        Item::of_list(&db, &trp_data::character::inventory::Loadout::of_build(preset)).expect("the weapon").0
    };
    assert_eq!(brief_name(&named, &build("5af08cf886f774223c269184"), name.clone()), "Colt M4A1 5.56x45 assault rifle Carbine");
    assert_eq!(brief_name(&named, &build("66b0e484c84cc7141324fd6c"), name.clone()), "Colt M4A1 5.56x45 assault rifle T.O 2K24");
    let bottle = Item::new(Arc::new(kind("water", &["FoodDrink"], [1, 2])));
    assert_eq!(brief_name(&named, &bottle, "Bottle of water (0.6L)".into()), "Bottle of water (0.6L)");
    let rifle = Item::new(Arc::new(kind("rifle", &["Weapon"], [4, 1])));
    let long = vec![("A name of thirty-one letters ok".to_owned(), vec!["rifle".to_owned()])];
    assert_eq!(brief_name(&long, &rifle, "R".into()), "R A name of thirty-one letters...");
}
