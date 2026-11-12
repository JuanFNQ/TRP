//! Tests of the tactical devices' keys as the app and the replay run them.
//! After the tactical-devices spec, "The keys", through `trp_sim::tactical::Devices`, with
//! the install's stock AKM, its motion and body, an X400 (a combo unit, three
//! modes) put in its first slot: the press and its switch, Hold mode's keys
//! and its let-go, and the hip penalty worked out as each is taken. These
//! pass trivially, with a note, where no install is configured.

use std::sync::Arc;

use trp_data::setup::engine::EngineSettings;
use trp_data::setup::hard_settings::HardSettings;
use trp_data::character::player::HandsData;
use trp_data::unity::Assets;
use trp_sim::items::inventory::{Equipment, Item, Kind};
use trp_sim::weapons::tactical::{Around, Devices, Event, InputMode, Keys, WeaponKey};
use trp_sim::{Firearm, Hands, Movement, ViewSettings};

const AKM: &str = "59d6088586f774275f37482f";
const X400: &str = "56def37dd2720bec348b456a";
const DT: f32 = 1.0 / 120.0;

struct Rig {
    movement: Movement,
    hands: Hands,
    firearm: Firearm,
    root: Item,
    weapon: u64,
}

fn rig() -> Option<Rig> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database().expect("database");
    let hard = HardSettings::load(&install).expect("settings object");
    let engine = EngineSettings::load(&install).expect("engine settings");
    let mut assets = Assets::new(install).expect("manifest");
    let settings = trp_sim::load_movement_settings(&mut assets, db, &hard, &engine, 0.0).expect("movement settings");
    let body = trp_sim::locomotion::body::BodyData::load(&mut assets).expect("body animator");
    let movement = Movement::new(settings, trp_sim::Body::new(&body, 5.0));
    let built = trp_data::weapons::weapon::assemble(&mut assets, db, AKM).expect("weapon");
    let data = HandsData::load(&mut assets).expect("player prefab");
    let hands = Hands::new(trp_sim::hands_settings(data, db, &hard, &engine, &built, ViewSettings::default()).expect("hands settings"));
    let animator = trp_sim::weapons::weapon_animator::WeaponAnimatorData::load(&assets, &built).expect("the weapon's animator");
    let mut firearm = Firearm::new(trp_sim::firearm_settings(&assets, db, &hard, &built).expect("firearm settings"), &animator);
    firearm.start_drawn();
    // The device in the weapon's first slot, the weapon worn: what the keys
    // walk takes no slot's rule.
    let mut weapon = Item::new(Arc::new(Kind::of(db, AKM).expect("the AKM's kind")));
    weapon.slots[0] = Some(Item::new(Arc::new(Kind::of(db, X400).expect("the X400's kind"))));
    let id = weapon.id;
    let mut root = Equipment::empty(db).expect("equipment").item;
    let slot = root.slot_index("FirstPrimaryWeapon").expect("a weapon slot");
    root.slots[slot] = Some(weapon);
    Some(Rig { movement, hands, firearm, root, weapon: id })
}

impl Rig {
    fn frame(&mut self, devices: &mut Devices, mode: InputMode, keys: Keys, let_go: bool, live: bool) -> Vec<Event> {
        let around = Around { held: Some(self.weapon), let_go, live, dt: DT };
        let events = devices.frame(mode, keys, around, &mut self.root, Some(&mut self.firearm), Some(&mut self.hands), &mut self.movement);
        self.firearm.step(DT);
        events
    }
}

fn hip(events: &[Event]) -> Option<(f32, bool)> {
    events.iter().find_map(|e| match e {
        Event::HipPenalty { penalty, combo } => Some((*penalty, *combo)),
        _ => None,
    })
}

/// Press mode: T switches the combo unit on, the hands go through their
/// switch, and the hip penalty drops by the ergonomics as the request is
/// taken (`UpdateHipInaccuracy`); Ctrl+T takes its mode one up, on.
#[test]
fn a_press_switches_the_device_and_the_hip_penalty_with_it() {
    let Some(mut r) = rig() else { return };
    let mut devices = Devices::holding(Some(r.weapon));
    let before = r.hands.hip_penalty();
    let events = r.frame(&mut devices, InputMode::Press, Keys { tactical: true, ..Keys::default() }, false, true);
    assert!(events.contains(&Event::Weapon { key: WeaponKey::Toggle, states: vec![(true, 0)], refused_by: None }), "{events:?}");
    let (penalty, combo) = hip(&events).expect("the hip penalty worked out again");
    assert!(combo && penalty < before, "{before} -> {penalty}");
    assert!(!events.contains(&Event::Sound), "Press mode's switch is the hands' click, not the player's sound");
    let mut entered = false;
    for _ in 0..30 {
        entered |= r.firearm.animator().state().0.ends_with("MOD SWITCH");
        r.frame(&mut devices, InputMode::Press, Keys::default(), false, true);
    }
    assert!(entered, "the hands' switch");
    let events = r.frame(&mut devices, InputMode::Press, Keys { next: true, ..Keys::default() }, false, true);
    assert!(events.contains(&Event::Weapon { key: WeaponKey::Next, states: vec![(true, 1)], refused_by: None }), "{events:?}");
}

/// Hold mode: on with the key down and off as it is let go, each with the
/// player's sound and no switch of the hands; and let go, forced, by what
/// lets it go (a screen over the battle) with the keys not heard, the hip
/// penalty worked out again on that frame (recorded: `courses8\tactical-hold`,
/// the inventory at 5.777 s).
#[test]
fn hold_mode_lets_go_as_the_game_does() {
    let Some(mut r) = rig() else { return };
    let mut devices = Devices::holding(Some(r.weapon));
    let events = r.frame(&mut devices, InputMode::Hold, Keys { tactical: true, ..Keys::default() }, false, true);
    assert!(events.contains(&Event::Sound) && hip(&events).is_some_and(|(_, combo)| combo), "{events:?}");
    for _ in 0..30 {
        assert!(!r.firearm.animator().state().0.ends_with("MOD SWITCH"), "no switch of the hands in Hold mode");
        r.frame(&mut devices, InputMode::Hold, Keys::default(), false, true);
    }
    let events = r.frame(&mut devices, InputMode::Hold, Keys::default(), true, false);
    assert!(events.contains(&Event::LetGo) && events.contains(&Event::Sound), "{events:?}");
    assert_eq!(hip(&events).map(|(_, combo)| combo), Some(false), "worked out again with the keys unheard");
    // Nothing on: nothing more to let go.
    let events = r.frame(&mut devices, InputMode::Hold, Keys::default(), true, false);
    assert!(!events.contains(&Event::LetGo));
}

/// The eyes' protection the lights' flares take (`Player.GetBlindnessProtection`;
/// tactical-devices spec, "The light shaft and the flare"): glasses of the
/// install's, then the Altyn with its face shield (0.25) fitted, up and down.
#[test]
fn the_flares_protection_is_what_the_head_wears() {
    use trp_data::character::inventory::Loadout;
    use trp_sim::items::inventory::blindness_protection;
    let Ok(_install) = crate::fixture::install().ok_or(()) else { return };
    let db = crate::fixture::database().expect("database");
    let mut equipment = Equipment::empty(db).expect("an empty equipment");
    assert_eq!(blindness_protection(&equipment), 0.0);
    // The first glasses by number that keep some flash out, less than the shield.
    let mut glasses: Vec<_> = db
        .items
        .values()
        .filter(|t| db.is_a(t, "Visors") && t.props.get("BlindnessProtection").and_then(|b| b.as_f64()).is_some_and(|b| b > 0.0 && b < 0.25))
        .map(|t| t.id.clone())
        .collect();
    glasses.sort();
    let glasses = Item::new(Arc::new(Kind::of(db, &glasses[0]).expect("the glasses")));
    let worn_glasses = glasses.kind.blindness_protection;
    equipment.wear("Eyewear", glasses).expect("the glasses worn");
    assert_eq!(blindness_protection(&equipment), worn_glasses);
    // The Altyn's stock build, its face shield fitted, up.
    let (mut helmet, _) = Item::of_list(db, &Loadout::of_preset(db, "5aa7e276e5b5b000171d0647").expect("the Altyn's build")).expect("the Altyn");
    let slot = helmet.slot_index("mod_equipment").expect("the Altyn's shield slot");
    helmet.slots[slot] = Some(Item::new(Arc::new(Kind::of(db, "5aa7e373e5b5b000137b76f0").expect("the Altyn's shield"))));
    equipment.wear("Headwear", helmet).expect("the Altyn worn");
    assert_eq!(blindness_protection(&equipment), worn_glasses, "the shield up counts for nothing");
    // Down.
    let head = equipment.item.slot_index("Headwear").unwrap();
    equipment.item.slots[head].as_mut().unwrap().slots[slot].as_mut().unwrap().on = true;
    assert!((blindness_protection(&equipment) - 0.25).abs() < 1e-6, "the shield down");
}
