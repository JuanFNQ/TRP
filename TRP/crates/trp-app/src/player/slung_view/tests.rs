//! Tests of what is hung on a body and where.

use super::*;
use std::sync::Arc;
use trp_sim::items::inventory::Kind;

/// The install and its database, read once for these tests; none where
/// no install is configured (the test then passes with a note).
fn installed() -> Option<&'static (trp_data::Install, trp_data::Database)> {
    static READ: std::sync::OnceLock<Option<(trp_data::Install, trp_data::Database)>> = std::sync::OnceLock::new();
    let read = READ
        .get_or_init(|| {
            let install = trp_data::Install::locate().ok()?;
            let db = trp_data::Database::load(&install).expect("database");
            Some((install, db))
        })
        .as_ref();
    if read.is_none() {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
    }
    read
}

/// The hanging's rules as the run reads them: the player prefab's slot
/// bones, the character's legs' holster on the right, dummy 0's on the
/// right and dummy 1's legs with none.
fn slung(install: &trp_data::Install, db: &trp_data::Database) -> Slung {
    let mut assets = trp_data::unity::Assets::new(install.clone()).expect("manifest");
    let bones = trp_data::character::body::slot_bones(&mut assets).expect("the slot bones");
    Slung::new(db, bones, Some(RIGHT), vec![Some(RIGHT), None])
}

/// A holster on the right leg, and one on the left (the place and turn
/// do not choose the bone).
const RIGHT: LegsHolster = LegsHolster { right: true, position: [0.3258, 0.0947, 0.167], rotation: [0.0, 0.0, 0.0, 1.0] };
const LEFT: LegsHolster = LegsHolster { right: false, position: [0.3258, 0.0947, 0.167], rotation: [0.0, 0.0, 0.0, 1.0] };
/// The PM, a pistol.
const PM: &str = "5448bd6b4bdc2dfc2f8b4569";

fn thing(db: &trp_data::Database, template: &str) -> Item {
    Item::new(Arc::new(Kind::of(db, template).expect("its kind")))
}

fn put(gear: &mut Item, slot: &str, item: Item) -> Id {
    let id = item.id;
    let at = gear.slot_index(slot).expect("the slot");
    gear.slots[at] = Some(item);
    id
}

fn by_slot(wanted: &[Want]) -> Vec<(&'static str, Id, String)> {
    wanted.iter().map(|want| (want.slot, want.item, want.bone.clone())).collect()
}

/// A pistol put in a dummy's holster by searching hangs on its legs'
/// holster's leg's bone (`LegsView`: the right thigh's
/// `Right_weapon_holster_pistol`, the left's for a left holster), and
/// on legs without one not at all (`HaveHolster`); the kit's own knife
/// is not shown on the body (`DisplayOnModel`), and it begins with no
/// pistol.
#[test]
fn a_pistol_put_in_a_dummys_holster_hangs_on_its_leg_bone() {
    let Some((install, db)) = installed() else { return };
    let slung = slung(install, db);
    let mut kit = crate::dummy_kit(install, db, "bear").expect("the BEAR's kit");
    let akm = kit.in_slot("FirstPrimaryWeapon").expect("its AKM").id;
    assert!(kit.in_slot("Holster").is_none(), "a starting kit carries no pistol");
    let before = slung.hanging(Body::Dummy(0), Some(&kit), Some(akm), Some(RIGHT));
    assert!(before.is_empty(), "nothing hung but what is in hand: {:?}", by_slot(&before));
    let pm = put(&mut kit, "Holster", thing(db, PM));
    let right = by_slot(&slung.hanging(Body::Dummy(0), Some(&kit), Some(akm), Some(RIGHT)));
    assert_eq!(right.len(), 1, "{right:?}");
    assert_eq!((right[0].0, right[0].1), ("Holster", pm));
    assert!(right[0].2.ends_with("Right_weapon_holster_pistol") && right[0].2.contains("Base HumanRThigh"), "{}", right[0].2);
    assert_eq!(Some(right[0].2.as_str()), slung.bones.pistol.as_deref());
    let left = by_slot(&slung.hanging(Body::Dummy(0), Some(&kit), Some(akm), Some(LEFT)));
    assert!(left[0].2.ends_with("Left_weapon_holster_pistol") && left[0].2.contains("Base HumanLThigh"), "{}", left[0].2);
    // Dummy 1's legs as read for it (none), and legs with none.
    let none = slung.hanging(Body::Dummy(1), Some(&kit), Some(akm), slung.dummy_holsters[1]);
    assert!(none.is_empty(), "no holster, no pistol: {:?}", by_slot(&none));
    // Its holster bone takes the holster's place (`AdjustPistolHolster`).
    assert_eq!(slung.holster_bone(Some(RIGHT)).map(|(path, _)| path), slung.bones.pistol.as_deref());
    assert_eq!(slung.holster_bone(None), None);
    eprintln!("the PM on {}", right[0].2);
}

/// A dummy's rifle in hand is not hung (its slot's view leaves out the
/// item in hand, standing or dead); its hands empty, it hangs in its
/// slot; a second rifle put on its back hangs on the back's bone.
#[test]
fn a_dummys_rifle_in_hand_is_not_hung() {
    let Some((install, db)) = installed() else { return };
    let slung = slung(install, db);
    let mut kit = crate::dummy_kit(install, db, "usec").expect("the USEC's kit");
    let akm = kit.in_slot("FirstPrimaryWeapon").expect("its AKM").id;
    let held = slung.hanging(Body::Dummy(0), Some(&kit), Some(akm), Some(RIGHT));
    assert!(!held.iter().any(|want| want.item == akm), "in hand, not hung: {:?}", by_slot(&held));
    let empty = by_slot(&slung.hanging(Body::Dummy(0), Some(&kit), None, Some(RIGHT)));
    assert!(empty.iter().any(|(slot, id, _)| *slot == "FirstPrimaryWeapon" && *id == akm), "its hands empty, the AKM hangs: {empty:?}");
    let second = put(&mut kit, "SecondPrimaryWeapon", thing(db, crate::world::dummy::WEAPON));
    let back = by_slot(&slung.hanging(Body::Dummy(0), Some(&kit), Some(akm), Some(RIGHT)));
    assert_eq!(back.len(), 1, "{back:?}");
    assert_eq!((back[0].0, back[0].1), ("SecondPrimaryWeapon", second));
    // On the back's own bone or its alternative, by the backpack worn.
    assert!([&slung.bones.secondary, &slung.bones.secondary_alternative].iter().any(|bone| bone.as_deref() == Some(back[0].2.as_str())), "{}", back[0].2);
    eprintln!("the second AKM on {} (backpack {:?})", back[0].2, kit.in_slot("Backpack").map(|pack| pack.kind.name.clone()));
}

/// The figure's hung weapons follow the character's equipment, the
/// weapon in hand left off as the game's menu figure leaves it
/// (`PlayerModelLoader.CreatePlayerBody`, its item in hand): the same
/// slots as the character's own body hangs, a rifle put on the back
/// hung, taken off gone, the one in hand hung once put away.
#[test]
fn a_figures_hung_weapons_follow_the_equipment() {
    let Some((install, db)) = installed() else { return };
    let slung = slung(install, db);
    let mut equipment = crate::dummy_kit(install, db, "usec").expect("an equipment");
    let in_hand = equipment.in_slot("FirstPrimaryWeapon").expect("a rifle").id;
    let figure = |equipment: &Item, held: Option<Id>| by_slot(&slung.hanging(Body::Figure, Some(equipment), held, Some(RIGHT)));
    assert!(figure(&equipment, Some(in_hand)).is_empty(), "the weapon in hand left off");
    let back = put(&mut equipment, "SecondPrimaryWeapon", thing(db, crate::world::dummy::WEAPON));
    let hung = figure(&equipment, Some(in_hand));
    assert_eq!(hung.iter().map(|(slot, id, _)| (*slot, *id)).collect::<Vec<_>>(), vec![("SecondPrimaryWeapon", back)]);
    assert_eq!(hung, by_slot(&slung.hanging(Body::Character, Some(&equipment), Some(in_hand), Some(RIGHT))), "as the character's own body");
    let put_away = figure(&equipment, None);
    assert_eq!(put_away.iter().map(|(slot, id, _)| (*slot, *id)).collect::<Vec<_>>(), vec![("FirstPrimaryWeapon", in_hand), ("SecondPrimaryWeapon", back)]);
    let at = equipment.slot_index("SecondPrimaryWeapon").expect("the back's slot");
    equipment.slots[at] = None;
    assert!(figure(&equipment, Some(in_hand)).is_empty(), "taken off, gone");
}

/// The OVERALL tab's own figure (`OVERALL FIGURE`) hangs what the
/// character's equipment has but the thing it holds, so the weapon it
/// holds is never drawn twice: its rifle in hand not hung, the same
/// rifle hung once its hands are empty; its back's and holster's hung
/// as the gear page's figure hangs them.
#[test]
fn the_overall_figure_never_hangs_what_it_holds() {
    let Some((install, db)) = installed() else { return };
    let slung = slung(install, db);
    let mut equipment = crate::dummy_kit(install, db, "usec").expect("an equipment");
    let in_hand = equipment.in_slot("FirstPrimaryWeapon").expect("a rifle").id;
    put(&mut equipment, "Holster", thing(db, PM));
    let overall = |held: Option<Id>| by_slot(&slung.hanging(Body::Overall, Some(&equipment), held, Some(RIGHT)));
    let holding = overall(Some(in_hand));
    assert!(!holding.iter().any(|(_, id, _)| *id == in_hand), "the rifle it holds not hung: {holding:?}");
    assert_eq!(holding, by_slot(&slung.hanging(Body::Figure, Some(&equipment), Some(in_hand), Some(RIGHT))), "as the gear page's figure");
    assert!(overall(None).iter().any(|(slot, id, _)| *slot == "FirstPrimaryWeapon" && *id == in_hand), "hung once its hands are empty");
    assert!(Body::Overall.hangs());
}

/// The character's corpse hangs nothing, whatever it has on: the game's
/// corpse is its living body kept, its slots' renderers off as its own
/// player's (`Player.CreateCorpse`, `reinitBody: false`).
#[test]
fn the_players_corpse_hangs_none() {
    let Some((install, db)) = installed() else { return };
    let slung = slung(install, db);
    let mut equipment = crate::dummy_kit(install, db, "bear").expect("an equipment");
    put(&mut equipment, "SecondPrimaryWeapon", thing(db, crate::world::dummy::WEAPON));
    put(&mut equipment, "Holster", thing(db, PM));
    let alive = slung.hanging(Body::Character, Some(&equipment), None, Some(RIGHT));
    assert_eq!(alive.len(), 3, "{:?}", by_slot(&alive));
    assert!(slung.hanging(Body::Corpse, Some(&equipment), None, Some(RIGHT)).is_empty());
    assert!(!Body::Corpse.hangs() && Body::Character.hangs() && Body::Figure.hangs() && Body::Dummy(1).hangs());
    assert!(slung.hung_on(Body::Corpse).is_empty());
}

/// What a body's slots hang is worked out when the carried things changed
/// (their change's number), the thing in hand, the holster or the rounds'
/// switch, and is each body's own; otherwise it is what was kept, its
/// rounds found once.
#[test]
fn what_a_body_hangs_is_worked_out_when_something_changed() {
    let Some((install, db)) = installed() else { return };
    let mut slung = slung(install, db);
    let mut equipment = crate::dummy_kit(install, db, "usec").expect("an equipment");
    let rifle = equipment.in_slot("FirstPrimaryWeapon").expect("a rifle").id;
    let items = |wants: &Wants| wants.wanted.iter().map(|want| (want.slot, want.item)).collect::<Vec<_>>();
    let mut first = slung.take_wants(Body::Figure, Some(&equipment), 7, None, Some(RIGHT), true);
    assert_eq!(items(&first), vec![("FirstPrimaryWeapon", rifle)]);
    assert_eq!(first.rounds, vec![None], "its rounds yet to find");
    first.rounds[0] = Some(Vec::new());
    slung.wants.push(first);
    // Nothing changed: the kept one, though the equipment given differs
    // (it is not looked at), its rounds as found.
    let back = put(&mut equipment, "SecondPrimaryWeapon", thing(db, crate::world::dummy::WEAPON));
    let kept = slung.take_wants(Body::Figure, Some(&equipment), 7, None, Some(RIGHT), true);
    assert_eq!((items(&kept), kept.rounds.clone()), (vec![("FirstPrimaryWeapon", rifle)], vec![Some(Vec::new())]));
    assert!(slung.wants.is_empty(), "taken out for the frame's work");
    slung.wants.push(kept);
    // Another body's is its own.
    let character = slung.take_wants(Body::Character, Some(&equipment), 8, None, Some(RIGHT), true);
    assert_eq!(items(&character), vec![("FirstPrimaryWeapon", rifle), ("SecondPrimaryWeapon", back)]);
    slung.wants.push(character);
    // The carried things changed: afresh, the rounds to find again.
    let changed = slung.take_wants(Body::Figure, Some(&equipment), 8, None, Some(RIGHT), true);
    assert_eq!((items(&changed), changed.rounds.clone()), (vec![("FirstPrimaryWeapon", rifle), ("SecondPrimaryWeapon", back)], vec![None, None]));
    slung.wants.push(changed);
    // The thing in hand, the holster and the rounds' switch each change it too.
    let held = slung.take_wants(Body::Figure, Some(&equipment), 8, Some(rifle), Some(RIGHT), true);
    assert_eq!(items(&held), vec![("SecondPrimaryWeapon", back)]);
    slung.wants.push(held);
    for (holster, rounds_shown) in [(Some(LEFT), true), (Some(LEFT), false)] {
        let mut again = slung.take_wants(Body::Figure, Some(&equipment), 8, Some(rifle), holster, rounds_shown);
        assert_eq!(again.rounds, vec![None], "worked out afresh");
        again.rounds[0] = Some(Vec::new());
        slung.wants.push(again);
    }
    assert_eq!(slung.wants.len(), 2, "one a body");
}
