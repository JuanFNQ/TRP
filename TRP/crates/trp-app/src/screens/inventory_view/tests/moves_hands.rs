//! Tests of moves and their sounds, clicks, the hands' takes, chambers and split stacks.

use super::*;

/// The sounds of moves and windows as the game's
/// (`ItemUiContext.PlayOperationSound`, `OpenItem`, `Inspect`): a part
/// into a weapon's slot the interface's installing by its class, a
/// thing put on its group's `use`, one sent into a grid its `drop`; a
/// container's window its group's opening, a thing's the inspector's.
/// And where the game's window that shows a thing opens
/// (`OpenWindow`): in the middle of the screen, the next 40 units right
/// of and below the one opened last, kept on the screen.
#[test]
fn moves_sound_and_windows_open_as_the_game_s() {
    let (mut carried, [bag, rifle, rounds, case]) = carried();
    let into_rifle = Some((rifle, Spot::Slot(0)));
    let sound = |carried: &Carried, classes: &[&str], address| move_sound(carried, &kind("part", classes, [1, 1]), address);
    assert!(matches!(sound(&carried, &["Mod", "GearMod", "Magazine"], into_rifle), Sound::Ui(Ui::InstallMag)));
    assert!(matches!(sound(&carried, &["Mod", "FunctionalMod", "Sights"], into_rifle), Sound::Ui(Ui::InstallModFunc)));
    assert!(matches!(sound(&carried, &["Mod", "GearMod", "Stock"], into_rifle), Sound::Ui(Ui::InstallModGear)));
    assert!(matches!(sound(&carried, &["Mod", "MasterMod", "Barrel"], into_rifle), Sound::Ui(Ui::InstallModVital)));
    assert!(matches!(sound(&carried, &["Mod", "GearMod", "Magazine"], Some((case, Spot::Cell(0, Place { x: 0, y: 0, turned: false })))), Sound::Item(_, Way::Drop)));
    let alt = [false, false, true];
    assert!(matches!(click(&mut carried, bag, false, alt, Vec2::ZERO, None), Some(Sound::Item(_, Way::Use))));
    assert!(matches!(click(&mut carried, rounds, false, [true, false, false], Vec2::ZERO, None), Some(Sound::Item(_, Way::Drop))));
    assert!(matches!(click(&mut carried, case, true, [false; 3], Vec2::ZERO, None), Some(Sound::Item(_, Way::Open))));
    carried.windows.clear();
    carried.info_size = Some(Vec2::new(670.0, 500.0));
    let (screen, s) = (Vec2::new(2560.0, 1440.0), 1440.0 / 1080.0);
    assert!(matches!(click(&mut carried, rifle, true, [false; 3], Vec2::ZERO, Some((screen, s))), Some(Sound::Ui(Ui::InspectorOpen))));
    let first = carried.windows[0].at;
    assert!((first - (screen - Vec2::new(670.0, 500.0) * s) / 2.0).length() < 1e-3, "{first}");
    click(&mut carried, rounds, true, [false; 3], Vec2::ZERO, Some((screen, s)));
    assert!((carried.windows[1].at - first - Vec2::splat(40.0 * s)).length() < 1e-3);
    // Brought to the top, the first is still not the one opened last.
    carried.open_window(rifle, WinKind::Info, first);
    assert!((game_window_at(&carried, Vec2::new(670.0, 500.0), screen, s) - first - Vec2::splat(80.0 * s)).length() < 1e-3);
    // Far down the screen, kept on it.
    carried.windows[0].at = Vec2::new(2400.0, 1300.0);
    carried.windows[0].opened = u64::MAX;
    assert_eq!(game_window_at(&carried, Vec2::new(670.0, 500.0), screen, s), screen - Vec2::new(670.0, 500.0) * s);
}

/// A frame with nothing to load by hand leaves what is carried as it
/// was: marked changed, the inventory's page is drawn afresh on every
/// frame, and a box drawn afresh is never under the mouse (nothing
/// hovers: 0.211.0 to 0.247.0).
#[test]
fn a_frame_with_nothing_to_load_by_hand_leaves_what_is_carried_unchanged() {
    use bevy::ecs::system::RunSystemOnce;
    let (carried, [_, rifle, ..]) = carried();
    let mut world = World::new();
    world.insert_resource(carried);
    world.insert_resource(Hands { item: Some(rifle), ..Default::default() });
    world.clear_trackers();
    world.run_system_once(|mut carried: ResMut<Carried>, mut hands: ResMut<Hands>| chamber_by_hand(false, &mut carried, &mut hands, None, false)).expect("the system ran");
    assert!(!world.is_resource_changed::<Carried>(), "a quiet frame marked what is carried changed");
}

/// What the hands may take (`HandSlots`, before the step from the empty
/// hands as after it): a weapon hanging in a slot a weapon is held from,
/// of a kind whose rig is built, in the slots' order; the one in hand
/// hangs where it is while its new build's rig is still being made.
#[test]
fn the_hands_take_what_hangs_of_a_kind_built() {
    let (mut carried, [_, rifle, ..]) = carried();
    click(&mut carried, rifle, false, [false, false, true], Vec2::ZERO, None);
    assert!(carried.equipment.worn("FirstPrimaryWeapon").is_some_and(|weapon| weapon.id == rifle));
    let none: [String; 0] = [];
    let unbuilt = HandSlots { carried: &carried, in_hand: None, built: &none, devices: false };
    assert_eq!((unbuilt.hangs("FirstPrimaryWeapon"), unbuilt.slot_of(rifle), unbuilt.first_but(None)), (None, None, None));
    let held = HandSlots { in_hand: Some(rifle), ..unbuilt };
    assert_eq!(held.slot_of(rifle), Some("FirstPrimaryWeapon"));
    let built = [trp_sim::items::inventory::rig_key(carried.find(rifle).unwrap())];
    let ready = HandSlots { carried: &carried, in_hand: None, built: &built, devices: false };
    assert_eq!(ready.first_but(None), Some((rifle, "FirstPrimaryWeapon")));
    assert_eq!(ready.first_but(Some(rifle)), None);
    assert_eq!(ready.kind_of(rifle).as_deref(), Some(built[0].as_str()));
    assert!(!ready.folded_of(rifle) && !ready.combo_of(rifle));
}

/// The clicks of the game's item view: Alt puts a thing on, Ctrl
/// sends it to the other side, a double click opens a container in a
/// window, which is then where Ctrl sends things, and shows what
/// anything else is.
#[test]
fn clicks_put_on_send_over_and_open() {
    let (mut carried, [bag, rifle, rounds, case]) = carried();
    let (ctrl, alt, plain) = ([true, false, false], [false, false, true], [false; 3]);
    let holder = |carried: &Carried, id: Id| carried.lies(id).map(|(holder, _)| holder);
    let worn = carried.equipment.item.id;
    // Ctrl on the rounds with nothing worn to hold them: they stay in
    // the crate (and go to its next free cell).
    click(&mut carried, rounds, false, ctrl, Vec2::ZERO, None);
    assert_eq!(holder(&carried, rounds), Some(carried.loot.id));
    click(&mut carried, bag, false, alt, Vec2::ZERO, None);
    click(&mut carried, rifle, false, alt, Vec2::ZERO, None);
    assert_eq!((holder(&carried, bag), holder(&carried, rifle)), (Some(worn), Some(worn)));
    assert!(carried.note.ends_with("put on"), "{}", carried.note);
    // Now the rounds have somewhere to go, and from there back.
    click(&mut carried, rounds, false, ctrl, Vec2::ZERO, None);
    assert_eq!(holder(&carried, rounds), Some(bag));
    click(&mut carried, rounds, false, ctrl, Vec2::ZERO, None);
    assert_eq!(holder(&carried, rounds), Some(carried.loot.id));
    // A double click on the rifle shows what it is; on the case, what
    // is in it, and the case is then where Ctrl sends the rounds.
    click(&mut carried, rifle, true, plain, Vec2::new(100.0, 100.0), None);
    assert_eq!((carried.windows.len(), carried.windows[0].kind, carried.priority), (1, WinKind::Info, None));
    click(&mut carried, case, true, plain, Vec2::new(200.0, 100.0), None);
    assert_eq!((carried.windows.len(), carried.windows[1].kind, carried.priority), (2, WinKind::Grids, Some(case)));
    click(&mut carried, rounds, false, ctrl, Vec2::ZERO, None);
    assert_eq!(holder(&carried, rounds), Some(case));
    // And out of it again: a thing in the priority container goes by
    // the plain rule, to the character.
    click(&mut carried, rounds, false, ctrl, Vec2::ZERO, None);
    assert_eq!(holder(&carried, rounds), Some(bag));
    // A Ctrl double click is no double click: it sends, twice.
    click(&mut carried, rounds, true, ctrl, Vec2::ZERO, None);
    assert_eq!(holder(&carried, rounds), Some(case));
}

/// A chamber's knowledge through shots (`FireOperation.UncheckOnShot`):
/// the chamber known as the magazine was before each shot that fed it.
/// A checked 30's first shot keeps it known (the magazine then not); a
/// second does not; a shot leaving the magazine empty keeps what it was
/// before that shot; with the game's `UncheckOnShot` off, both stay.
#[test]
fn a_chamber_is_known_as_the_magazine_was_before_the_shot() {
    assert!(chamber_after_shots(true, 30, 29, 30, true));
    assert!(!chamber_after_shots(true, 30, 28, 30, true));
    assert!(!chamber_after_shots(false, 30, 29, 30, true));
    assert!(!chamber_after_shots(true, 2, 0, 30, true));
    assert!(chamber_after_shots(true, 1, 0, 30, true));
    assert!(chamber_after_shots(true, 30, 20, 30, false));
}

/// The slots a chamber's check unlocks (`IsChamberLocked`): a chamber's
/// own, a cylinder's chambers only where the magazine is a cylinder.
#[test]
fn the_chambers_slots_are_the_ones_a_check_unlocks() {
    assert!(unchecked_slot("patron_in_weapon", false));
    assert!(unchecked_slot("patron_in_weapon_001", false));
    assert!(unchecked_slot("camora_003", true));
    assert!(!unchecked_slot("camora_003", false));
    assert!(!unchecked_slot("mod_magazine", true));
}

/// With Ctrl held a drop is part of a stack: all but one at the most
/// into a free cell, and a thing that is no stack goes nowhere.
#[test]
fn ctrl_lets_go_of_part_of_a_stack() {
    let (mut carried, [_, rifle, rounds, case]) = carried();
    let cell = Target::Cell(case, 0, Place { x: 0, y: 0, turned: false });
    let roots = &mut [&mut carried.equipment.item, &mut carried.loot];
    assert_eq!((most_of(roots, rounds, cell), most_of(roots, rifle, cell)), (59, 0));
    assert_eq!(act(roots, rifle, cell, &carried.rules, Some(1)), Err(Refusal::NoAction));
    assert_eq!(act(roots, rounds, cell, &carried.rules, Some(60)), Err(Refusal::NoAction));
    assert_eq!(act(roots, rounds, cell, &carried.rules, Some(25)), Ok("split"));
    let split = roots[1].find(case).unwrap().grids[0][0].1.id;
    assert_eq!((roots[1].find(rounds).unwrap().count, roots[1].find(split).unwrap().count), (35, 25));
    // Onto the 35 from the 25: all but one may go, and a count is taken.
    assert_eq!(most_of(roots, split, Target::Item(rounds)), 24);
    assert_eq!(act(roots, split, Target::Item(rounds), &carried.rules, Some(10)), Ok("handed over"));
    assert_eq!((roots[1].find(rounds).unwrap().count, roots[1].find(split).unwrap().count), (45, 15));
}

/// Ctrl and a click that finds no place leaves the game's warning for the
/// notices (`ItemUiContext.QuickFindAppropriatePlace`): with nothing worn
/// to look in, `NoPossibleActionsError`; with a backpack on and full,
/// `NoFreeSpaceError`, "No space for this item". A move that goes through
/// leaves none.
#[test]
fn a_quick_move_with_no_room_leaves_a_warning() {
    let (mut carried, [bag, _, rounds, case]) = carried();
    // With room the thing goes over, or elsewhere in its own holder.
    assert!(click(&mut carried, case, false, [true, false, false], Vec2::ZERO, None).is_some());
    assert_eq!(carried.warning, None);
    // The crate filled to its last cell, and nothing worn to look in.
    let filler = || Item::new(Arc::new(kind("filler", &["Item"], [1, 1])));
    while carried.loot.stow(filler()).is_ok() {}
    let sound = click(&mut carried, case, false, [true, false, false], Vec2::ZERO, None);
    assert!(sound.is_none(), "{} {:?}", carried.note, carried.lies(case));
    assert_eq!(carried.warning.take().as_deref(), Some("No actions available"));
    // A backpack put on and filled, the crate full again: no room.
    click(&mut carried, bag, false, [false, false, true], Vec2::ZERO, None);
    while carried.loot.stow(filler()).is_ok() {}
    while carried.equipment.item.find_mut(bag).expect("the backpack worn").stow(filler()).is_ok() {}
    assert!(click(&mut carried, rounds, false, [true, false, false], Vec2::ZERO, None).is_none());
    assert_eq!(carried.warning.take().as_deref(), Some("No space for this item"));
}
