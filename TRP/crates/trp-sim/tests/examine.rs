//! Tests of examining things: what is known, how long an examine takes and what is refused.
//! After `docs/spec/inventory.md`, "Examining": the list of what is known, the examine's time, and what a thing not known is refused. A
//! test binary of its own, the list being the process's: its tests take
//! turns.

use std::sync::{Arc, Mutex, MutexGuard};

use trp_data::character::inventory::Holder;
use trp_sim::items::examine::{self, Examiner, Refused};
use trp_sim::items::inventory::{self, apply_item, move_item, take_out, Applied, Item, Kind, Place, Refusal, Rules, Spot, EQUIPMENT_TEMPLATE};
use trp_sim::items::item_menu::{lines, Facts, Line};

static TURN: Mutex<()> = Mutex::new(());

/// The list set to know nothing but what is known by default; held for
/// the test's length.
fn knowing_nothing() -> MutexGuard<'static, ()> {
    let turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    examine::set_list(Vec::new(), Vec::new());
    turn
}

fn holder(name: &str, cells: [u32; 2], takes: &[&str]) -> Holder {
    Holder { name: name.into(), cells, takes: takes.iter().map(|s| s.to_string()).collect(), ..Holder::default() }
}

/// A kind of thing of a class (its class's name the word with its first
/// letter large), not known by default unless said.
fn kind(name: &str, class: &str, tweak: impl FnOnce(&mut Kind)) -> Arc<Kind> {
    let named = format!("{}{}", class[..1].to_uppercase(), &class[1..]);
    let mut kind = Kind { template: name.into(), name: name.into(), classes: vec![name.into(), class.into(), "item".into()], class_names: vec![name.into(), named], cells: [1, 1], examine_time: 2.0, ..Kind::default() };
    tweak(&mut kind);
    Arc::new(kind)
}

/// The character's equipment: a weapon's slot, a vest with a grid, the
/// pockets.
fn equipment() -> Item {
    let rig = kind("rig", "vest", |k| {
        k.examined_by_default = true;
        k.grids = vec![holder("1", [4, 4], &["item"])];
    });
    let pockets = kind("pockets", "pockets", |k| {
        k.examined_by_default = true;
        k.grids = vec![holder("1", [2, 2], &["item"])];
    });
    let mut equipment = Item::new(kind(EQUIPMENT_TEMPLATE, "equipment", |k| {
        k.examined_by_default = true;
        k.slots = vec![holder("FirstPrimaryWeapon", [0, 0], &["weapon"]), holder("TacticalVest", [0, 0], &["vest"]), holder("Pockets", [0, 0], &["pockets"])];
    }));
    equipment.slots[1] = Some(Item::new(rig));
    equipment.slots[2] = Some(Item::new(pockets));
    equipment
}

fn rifle() -> Item {
    Item::new(kind("rifle", "weapon", |k| {
        k.slots = vec![holder("mod_magazine", [0, 0], &["magazine"]), holder("patron_in_weapon", [0, 0], &["ammo"])];
    }))
}

fn rules() -> Rules {
    Rules { in_raid: true, backpacks_deep: 0, busy_weapon: None, chamber_closed: None, chamber_held: None, limits: None, own: None }
}

fn vest(equipment: &Item) -> inventory::Id {
    equipment.slots[1].as_ref().unwrap().id
}

/// An examine takes the item's `ExamineTime` over 1 + Attention's buff;
/// it learns the kind when its time is out, every thing of the kind known
/// at once. A second is refused while one is under way, and a thing known
/// is not examined.
#[test]
fn an_examine_takes_its_time_over_attention() {
    let _turn = knowing_nothing();
    let (one, other) = (rifle(), rifle());
    assert!(!examine::known(&one) && !examine::known(&other));
    let mut examiner = Examiner::default();
    let time = examiner.begin(&one, 0.5, 10.0).expect("begun").time;
    assert!((time - 2.0 / 1.5).abs() < 1e-6, "{time}");
    assert!(examiner.examining(one.id) && !examiner.examining(other.id));
    assert_eq!(examiner.begin(&other, 0.5, 10.5).err(), Some(Refused::Busy));
    let done = examiner.under_way.as_ref().unwrap().done(10.0 + time as f64 / 2.0);
    assert!((done - 0.5).abs() < 1e-4, "{done}");
    assert!(examiner.finish(10.0 + time as f64 - 0.01).is_none());
    assert!(examiner.finish(10.0 + time as f64).is_some());
    assert!(examine::known(&one) && examine::known(&other), "the kind is learned, not the thing");
    assert_eq!(examiner.begin(&other, 0.0, 20.0).err(), Some(Refused::Known));
    assert_eq!(examine::learned(), vec!["rifle".to_owned()]);
    assert_eq!(examine::forget(), 1);
    assert!(!examine::known(&one));
    // Known by default, or with no list at all, nothing is to examine.
    let plain = Item::new(kind("plain", "item", |k| k.examined_by_default = true));
    assert!(examine::known(&plain));
    examine::know_everything();
    assert!(examine::known(&one));
}

/// The character's own slots take and give up no thing not known; a grid
/// takes it as anything; known, the slot takes it.
#[test]
fn ones_own_slots_refuse_a_thing_not_known() {
    let _turn = knowing_nothing();
    let mut equipment = equipment();
    let rifle = rifle();
    let (id, vest) = (rifle.id, vest(&equipment));
    let equipment_id = equipment.id;
    let mut ground = Item::new(kind("crate", "container", |k| {
        k.examined_by_default = true;
        k.grids = vec![holder("1", [6, 6], &["item"])];
    }));
    ground.grids[0].push((Place { x: 0, y: 0, turned: false }, rifle));
    let refused = move_item(&mut [&mut equipment, &mut ground], id, equipment_id, Spot::Slot(0), &rules());
    assert_eq!(refused, Err(Refusal::NotExamined { equipment: true }));
    // Into the vest's grid it goes.
    move_item(&mut [&mut equipment, &mut ground], id, vest, Spot::Cell(0, Place { x: 0, y: 0, turned: false }), &rules()).expect("a grid has no such rule");
    examine::learn("rifle");
    move_item(&mut [&mut equipment, &mut ground], id, equipment_id, Spot::Slot(0), &rules()).expect("known, it goes on");
    examine::forget();
    let out = move_item(&mut [&mut equipment, &mut ground], id, vest, Spot::Cell(0, Place { x: 0, y: 0, turned: false }), &rules());
    assert_eq!(out, Err(Refusal::TakeNotExamined { equipment: true }));
    assert_eq!(take_out(&mut [&mut equipment, &mut ground], id, &rules()).err(), Some(Refusal::TakeNotExamined { equipment: true }));
}

/// A body's equipment is no one's own to the looter (`Rules::own` names
/// the character's): its slots take and give up a thing not known, while
/// the character's own still refuse it.
#[test]
fn a_bodys_slots_take_and_give_up_a_thing_not_known() {
    let _turn = knowing_nothing();
    let mut own = equipment();
    let mut body = equipment();
    let rifle = rifle();
    let id = rifle.id;
    let (own_id, body_id) = (own.id, body.id);
    let mut ground = Item::new(kind("crate", "container", |k| {
        k.examined_by_default = true;
        k.grids = vec![holder("1", [6, 6], &["item"])];
    }));
    ground.grids[0].push((Place { x: 0, y: 0, turned: false }, rifle));
    let rules = Rules { own: Some(own_id), ..rules() };
    move_item(&mut [&mut own, &mut body, &mut ground], id, body_id, Spot::Slot(0), &rules).expect("a body's slot takes it");
    let refused = move_item(&mut [&mut own, &mut body, &mut ground], id, own_id, Spot::Slot(0), &rules);
    assert_eq!(refused, Err(Refusal::NotExamined { equipment: true }), "one's own slot refuses it");
    take_out(&mut [&mut own, &mut body, &mut ground], id, &rules).expect("a body's slot gives it up");
}

/// A weapon or a magazine lying in the world's container is no slot of
/// one's own: its slots take a thing not known by a move; a drop of it on
/// the weapon is refused all the same (`Weapon.Apply`), and so are rounds
/// not known on a magazine; two stacks not known merge.
#[test]
fn drops_on_a_weapon_or_magazine_refuse_a_thing_not_known() {
    let _turn = knowing_nothing();
    let mut crate_ = Item::new(kind("crate", "container", |k| {
        k.examined_by_default = true;
        k.grids = vec![holder("1", [8, 8], &["item"])];
    }));
    let magazine_kind = kind("mag", "magazine", |k| k.cartridges = vec![holder("cartridges", [0, 0], &["ammo"])]);
    let (rifle, magazine) = (rifle(), Item::new(magazine_kind));
    let (rifle_id, magazine_id) = (rifle.id, magazine.id);
    let round = kind("round", "ammo", |k| k.stack_max = 60);
    let (mut a, mut b) = (Item::new(round.clone()), Item::new(round));
    a.count = 10;
    b.count = 5;
    let (a_id, b_id) = (a.id, b.id);
    for (x, item) in [rifle, magazine, a, b].into_iter().enumerate() {
        crate_.grids[0].push((Place { x: 0, y: x as u32 * 2, turned: false }, item));
    }
    examine::learn("rifle");
    assert_eq!(apply_item(&mut [&mut crate_], magazine_id, rifle_id, &rules()), Err(Refusal::NotExamined { equipment: false }));
    examine::forget();
    examine::learn("mag");
    assert_eq!(apply_item(&mut [&mut crate_], magazine_id, rifle_id, &rules()), Err(Refusal::NotExamined { equipment: false }), "the weapon not known");
    examine::learn("rifle");
    assert_eq!(apply_item(&mut [&mut crate_], magazine_id, rifle_id, &rules()), Ok(Applied::Put(Spot::Slot(0))));
    assert_eq!(apply_item(&mut [&mut crate_], a_id, magazine_id, &rules()), Err(Refusal::NotExamined { equipment: false }), "rounds not known");
    assert_eq!(apply_item(&mut [&mut crate_], b_id, a_id, &rules()), Ok(Applied::Merged), "a merge is never refused");
}

/// A thing not known is not bound to a quick-use key, and rounds not
/// known are not reached for in a reload.
#[test]
fn quick_slots_and_reloads_pass_over_a_thing_not_known() {
    let _turn = knowing_nothing();
    let mut equipment = equipment();
    let (pockets, rifle) = (equipment.slots[2].as_ref().unwrap().id, rifle());
    let mut rounds = Item::new(kind("round", "ammo", |k| k.stack_max = 60));
    rounds.count = 30;
    let rifle_id = rifle.id;
    examine::learn("rifle");
    equipment.slots[0] = Some(rifle);
    equipment.find_mut(pockets).unwrap().grids[0].push((Place { x: 0, y: 0, turned: false }, rounds));
    assert_eq!(inventory::rounds_in_reach(&equipment, rifle_id), 0);
    examine::learn("round");
    assert_eq!(inventory::rounds_in_reach(&equipment, rifle_id), 30);
    let knife = Item::new(kind("knife", "knife", |_| {}));
    let knife_id = knife.id;
    equipment.find_mut(pockets).unwrap().grids[0].push((Place { x: 1, y: 0, turned: false }, knife));
    assert!(!inventory::bindable(&equipment, knife_id));
    examine::learn("knife");
    assert!(inventory::bindable(&equipment, knife_id));
}

/// The menu of a thing not known: Inspect, Examine and Discard; Examine
/// refused while that thing is being examined.
#[test]
fn the_menu_of_a_thing_not_known() {
    let _turn = knowing_nothing();
    let rifle = rifle();
    let facts = Facts { unknown: true, own: true, ..Facts::default() };
    let shown: Vec<Line> = lines(&rifle, &facts).into_iter().map(|(line, _)| line).collect();
    assert_eq!(shown, vec![Line::Inspect, Line::Examine, Line::Discard]);
    let busy = lines(&rifle, &Facts { examining: true, ..facts });
    assert_eq!(busy.iter().find(|(line, _)| *line == Line::Examine).map(|(_, can)| *can), Some(Err("Item is being examined")));
    let known: Vec<Line> = lines(&rifle, &Facts { own: true, ..Facts::default() }).into_iter().map(|(line, _)| line).collect();
    assert!(!known.contains(&Line::Examine) && known.contains(&Line::Reload));
}
