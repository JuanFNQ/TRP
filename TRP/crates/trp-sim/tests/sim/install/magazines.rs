//! Magazines and rounds moved from the inventory, quick reloads and magazine data.

use super::*;

/// The AKM's magazine taken out and put in from the inventory (firing spec,
/// "A magazine taken out or put in from the inventory"), against course 3
/// of `courses2`: the hands take them only while the logic idles.
#[test]
fn the_akm_s_magazine_is_moved_by_the_hands_from_the_inventory() {
    use trp_sim::weapons::weapon_animator::WeaponAnimatorData;
    use trp_sim::{Firearm, FirearmEvent};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, "59d6088586f774275f37482f").expect("the AKM");
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    const FRAME: f32 = 1.0 / 120.0;
    // When each of the events wanted came, from the operation's start,
    // until the move is made.
    let run = |w: &mut Firearm, wanted: &[FirearmEvent]| {
        let (mut said, mut at) = (Vec::new(), 0.0);
        while at < 4.0 {
            let events = w.step(FRAME);
            at += FRAME;
            let done = events.contains(&FirearmEvent::MagazineMoved);
            said.extend(events.into_iter().filter(|event| wanted.contains(event)).map(|event| (event, at)));
            if done {
                break;
            }
        }
        said
    };
    let mut w = Firearm::new(settings.clone(), &data);
    w.start_drawn();
    w.set_inventory(true);
    for _ in 0..240 {
        w.step(FRAME);
    }
    assert!(w.in_inventory() && w.can_move_magazine());
    // Out: the magazine out of the weapon's animation 0.885 s after the
    // operation began, the operation over at 1.435 s (course 3,
    // `3-menu-weapon`, the rifle in hand unloaded from the menu).
    assert!(w.pull_out_magazine());
    assert!(!w.can_move_magazine() && !w.pull_out_magazine() && !w.insert_magazine(30));
    let said = run(&mut w, &[FirearmEvent::MagazineOut, FirearmEvent::MagazineMoved]);
    eprintln!("out: {said:?}");
    let when = |said: &[(FirearmEvent, f32)], what: FirearmEvent| said.iter().find(|(event, _)| *event == what).map(|(_, at)| *at).expect("said");
    assert!((when(&said, FirearmEvent::MagazineOut) - 0.885).abs() <= 2.0 * FRAME);
    assert!((when(&said, FirearmEvent::MagazineMoved) - 1.435).abs() <= 2.0 * FRAME);
    assert_eq!((w.magazine, w.chamber), (None, true));
    assert!(w.can_move_magazine() && !w.pull_out_magazine());
    for _ in 0..120 {
        w.step(FRAME);
    }
    assert!(w.in_inventory(), "back down in its inventory pose");
    // In, with a round chambered: in at its mark, and over there.
    assert!(w.insert_magazine(30));
    // Until its `MagShow` the magazine going in is out of sight (a build of
    // another magazine may take over unseen); from it, in sight to the end
    // (the game's model shows the new magazine from there).
    assert!(w.magazine_out_of_sight() && !w.magazine_shown_going_in());
    let mut shown = false;
    for _ in 0..480 {
        if w.step(FRAME).contains(&FirearmEvent::MagazineShown) {
            shown = true;
            break;
        }
        assert!(w.magazine_out_of_sight() && !w.magazine_shown_going_in());
    }
    assert!(shown && !w.magazine_out_of_sight() && w.magazine_shown_going_in(), "in sight from its MagShow");
    let said = run(&mut w, &[FirearmEvent::MagazineIn, FirearmEvent::MagazineMoved]);
    assert!(!w.magazine_shown_going_in(), "over with the operation");
    eprintln!("in: {said:?}");
    assert_eq!(when(&said, FirearmEvent::MagazineIn), when(&said, FirearmEvent::MagazineMoved));
    assert_eq!((w.magazine, w.chamber), (Some(30), true));
    // With the chamber empty (the weapon fired dry) a round is chambered
    // from it, and the move is made then.
    let mut w = Firearm::new(settings.clone(), &data);
    w.magazine = Some(1);
    w.start_drawn();
    for _ in 0..3 {
        w.set_trigger(true, true);
        for _ in 0..30 {
            w.step(FRAME);
        }
        w.set_trigger(false, true);
        for _ in 0..90 {
            w.step(FRAME);
        }
    }
    assert_eq!((w.magazine, w.chamber), (Some(0), false));
    w.set_inventory(true);
    for _ in 0..240 {
        w.step(FRAME);
    }
    assert!(w.pull_out_magazine());
    run(&mut w, &[]);
    for _ in 0..120 {
        w.step(FRAME);
    }
    assert!(w.insert_magazine(30));
    let said = run(&mut w, &[FirearmEvent::MagazineIn, FirearmEvent::MagazineMoved]);
    eprintln!("in, chambering: {said:?}");
    assert!(when(&said, FirearmEvent::MagazineMoved) > when(&said, FirearmEvent::MagazineIn));
    assert_eq!((w.magazine, w.chamber), (Some(29), true));
}

/// A round put into the AKM's empty chamber from the inventory (firing
/// spec, "A round into the chamber from the inventory"): refused while a
/// magazine is in or the chamber is full; with neither, the hands play
/// their load of one from the inventory pose, the round in at the state's
/// mark (0.885 of 3.767 s), the logic idle then and back down in the
/// inventory pose. A put-away meanwhile gives it up at once.
#[test]
fn a_round_is_put_into_the_akm_s_chamber_from_the_inventory() {
    use trp_sim::weapons::weapon_animator::WeaponAnimatorData;
    use trp_sim::{Firearm, FirearmEvent};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, "59d6088586f774275f37482f").expect("the AKM");
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    assert!(settings.chamber_load);
    const FRAME: f32 = 1.0 / 120.0;
    let settle = |w: &mut Firearm, seconds: f32| {
        for _ in 0..(seconds / FRAME) as usize {
            w.step(FRAME);
        }
    };
    let mut w = Firearm::new(settings.clone(), &data);
    w.start_drawn();
    w.set_inventory(true);
    settle(&mut w, 2.0);
    // A magazine in, or a round chambered already: refused.
    assert!(w.chamber && !w.can_load_chamber() && !w.load_chamber());
    w.chamber = false;
    assert!(!w.can_load_chamber(), "a magazine is in");
    assert!(w.pull_out_magazine());
    settle(&mut w, 2.5);
    assert_eq!(w.magazine, None);
    assert!(w.can_load_chamber() && w.load_chamber());
    let (mut at, mut entered, mut chambered) = (0.0, None, None);
    while at < 6.0 && chambered.is_none() {
        let events = w.step(FRAME);
        at += FRAME;
        let (state, _, next) = w.animator().state();
        if entered.is_none() && next.unwrap_or(state).ends_with("LOAD ONE") {
            entered = Some(at);
        }
        if events.contains(&FirearmEvent::Chambered) {
            chambered = Some(at);
        }
    }
    let (entered, chambered) = (entered.expect("LOAD ONE played"), chambered.expect("chambered"));
    eprintln!("LOAD ONE from {entered:.3} s, the round in at {chambered:.3} s");
    assert!((chambered - entered - 0.885 * 3.767).abs() < 0.15, "{entered} {chambered}");
    assert!(w.chamber && w.can_move_magazine() && !w.can_load_chamber());
    settle(&mut w, 1.5);
    assert!(w.in_inventory(), "back down in its inventory pose");
    // Given up by a put-away.
    w.chamber = false;
    w.set_inventory(false);
    settle(&mut w, 1.5);
    assert!(w.load_chamber());
    settle(&mut w, 0.2);
    w.hide(false);
    let mut gave_up = false;
    for _ in 0..240 {
        gave_up |= w.step(FRAME).contains(&FirearmEvent::ChamberGivenUp);
    }
    assert!(gave_up && !w.chamber);
}

/// The chambered round taken out of the AKM from the inventory (firing
/// spec, "The chambered round taken out from the inventory";
/// `DischargeOperation`): refused with a magazine in; with it out, the
/// hands play `DISCHARGE` (`akm_ammo_out`, 2.6 s) from the inventory pose
/// and the chamber is empty at its `MagHide` mark (0.695 of it), for good:
/// nothing chambers it again (a PS round was once put back in the chamber
/// however often it was taken out).
#[test]
fn the_akm_s_chambered_round_is_taken_out_from_the_inventory() {
    use trp_sim::weapons::weapon_animator::WeaponAnimatorData;
    use trp_sim::{Firearm, FirearmEvent};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, "59d6088586f774275f37482f").expect("the AKM");
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    const FRAME: f32 = 1.0 / 120.0;
    let settle = |w: &mut Firearm, seconds: f32| {
        for _ in 0..(seconds / FRAME) as usize {
            w.step(FRAME);
        }
    };
    let mut w = Firearm::new(settings, &data);
    w.start_drawn();
    w.set_inventory(true);
    settle(&mut w, 2.0);
    assert!(w.chamber && !w.can_discharge() && !w.discharge(), "a magazine is in");
    assert!(w.pull_out_magazine());
    settle(&mut w, 2.5);
    assert_eq!(w.magazine, None);
    assert!(w.chamber && w.can_discharge() && w.discharge());
    assert!(!w.can_discharge() && !w.can_load_chamber(), "the hands are busy");
    let (mut at, mut entered, mut emptied) = (0.0, None, None);
    while at < 6.0 && emptied.is_none() {
        let events = w.step(FRAME);
        at += FRAME;
        let (state, _, next) = w.animator().state();
        if entered.is_none() && next.unwrap_or(state).ends_with("DISCHARGE") {
            entered = Some(at);
        }
        if events.contains(&FirearmEvent::Discharged) {
            emptied = Some(at);
        }
        assert!(w.chamber == emptied.is_none(), "the round is in until the mark");
    }
    let (entered, emptied) = (entered.expect("DISCHARGE played"), emptied.expect("emptied"));
    eprintln!("DISCHARGE from {entered:.3} s, the chamber empty at {emptied:.3} s");
    assert!((emptied - entered - 0.695 * 2.6).abs() < 0.15, "{entered} {emptied}");
    assert!(!w.chamber && w.can_move_magazine() && !w.can_discharge() && w.can_load_chamber());
    settle(&mut w, 3.0);
    assert!(!w.chamber && w.in_inventory(), "empty, and back down in its inventory pose");
}

/// The quick reload on the install's AKM (firing spec, "The quick
/// reload"): `RELOAD OUT ALL` in place of `RELOAD START` and `RELOAD OUT`,
/// then the plain reload's states; set beside the plain one, with a round
/// chambered and from an empty chamber. Only the install's controller run
/// by the simulation: no recording has a quick reload.
#[test]
fn the_quick_reload_throws_the_akm_s_magazine_out() {
    use trp_sim::weapons::weapon_animator::WeaponAnimatorData;
    use trp_sim::{Firearm, FirearmEvent};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, "59d6088586f774275f37482f").expect("the AKM");
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    assert!(settings.fast_reload, "the AKM's template allows it (isFastReload)");
    const FRAME: f32 = 1.0 / 120.0;
    // The states entered and the marks' events, each with when, until
    // the weapon is back at rest.
    let reload = |quick: bool, chambered: bool| {
        let mut w = Firearm::new(settings.clone(), &data);
        (w.chamber, w.magazine) = (chambered, Some(if chambered { 12 } else { 0 }));
        w.start_drawn();
        if quick { w.quick_reload_with(30) } else { w.reload_with(30) }
        let name = |w: &Firearm| {
            let (state, _, next) = w.animator().state();
            next.unwrap_or(state).trim_start_matches("Hands.").to_owned()
        };
        let (mut entered, mut said, mut last, mut at) = (Vec::new(), Vec::new(), String::new(), 0.0);
        while at < 10.0 {
            said.extend(w.step(FRAME).into_iter().filter(|e| !matches!(e, FirearmEvent::Sound(_))).map(|e| (e, at)));
            at += FRAME;
            let now = name(&w);
            if now != last {
                entered.push((now.clone(), at));
                last = now;
            }
            if w.is_idle() && at > 0.5 {
                break;
            }
        }
        let when = |what: &FirearmEvent| said.iter().find(|(event, _)| event == what).map(|(_, at)| *at).expect("said");
        let names: Vec<String> = entered.iter().map(|entry| entry.0.clone()).collect();
        let swap = said.iter().find_map(|(event, _)| matches!(event, FirearmEvent::MagazineSwap { .. }).then(|| event.clone()));
        (names, [when(&FirearmEvent::MagazineOut), when(&FirearmEvent::MagazineHidden), when(&FirearmEvent::MagazineIn), at], swap, (w.magazine, w.chamber))
    };
    for chambered in [true, false] {
        let (plain, plain_at, _, plain_after) = reload(false, chambered);
        let (quick, quick_at, swap, quick_after) = reload(true, chambered);
        println!("chambered {chambered}: plain {plain:?}, out {:.3} hidden {:.3} in {:.3} at rest {:.3}", plain_at[0], plain_at[1], plain_at[2], plain_at[3]);
        println!("chambered {chambered}: quick {quick:?}, out {:.3} hidden {:.3} in {:.3} at rest {:.3}", quick_at[0], quick_at[1], quick_at[2], quick_at[3]);
        assert_eq!(quick[0], "RELOAD OUT ALL");
        assert_eq!(&quick[1..], &plain[2..], "from the magazine going in the two are one");
        assert_eq!(swap, Some(FirearmEvent::MagazineSwap { left: Some(if chambered { 12 } else { 0 }), quick: true }));
        assert_eq!(quick_after, plain_after);
        // `RELOAD OUT ALL`'s marks are 0.486 and 0.602 of a 1.0 s clip.
        assert!((quick_at[0] - 0.486).abs() < 0.03 && (quick_at[1] - 0.602).abs() < 0.03, "{quick_at:?}");
        // From the magazine out on, the quick one is the plain one moved
        // earlier: the plain one leaves `RELOAD OUT` at 0.78 of 1.43 s
        // after 0.13 s of `RELOAD START`; the quick one `RELOAD OUT ALL`
        // at 0.9 of 1.0 s.
        let earlier = plain_at[2] - quick_at[2];
        assert!((earlier - (0.133 + 1.117 - 0.9)).abs() < 0.03, "{earlier}");
        assert!((plain_at[3] - quick_at[3] - earlier).abs() < 0.03);
    }
}

/// Reweighing a character (`Movement::set_weight`, done whenever what is
/// carried changes) takes so long: the whole and the root motion table's
/// copy, which it makes. Printed, not held to a figure.
#[test]
fn reweighing_is_timed() {
    let Some(mut m) = movement(10.0) else { return };
    let Ok(install) = crate::fixture::install().ok_or(()) else { return };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("settings object");
    let began = std::time::Instant::now();
    for step in 0..20 {
        m.set_weight(&hard, &db.config, 10.0 + step as f32).expect("reweighed");
    }
    let whole = began.elapsed().as_secs_f32() * 1000.0 / 20.0;
    let began = std::time::Instant::now();
    for _ in 0..20 {
        std::hint::black_box(m.settings.table.clone());
    }
    let table = began.elapsed().as_secs_f32() * 1000.0 / 20.0;
    println!("reweighing: {whole:.2} ms a time, of which the table's copy {table:.2} ms");
}

/// What the inventory screen lights together, by the install's filters
/// (inventory spec, "The box under the mouse, and what goes with it"),
/// either way round: the AKM with a spare of its own magazine and its own
/// round, and that magazine with that round; the M4A1 likewise with its
/// own; neither weapon with the other's magazine or round, nor with the
/// other.
#[test]
fn a_weapon_goes_with_its_own_magazines_and_rounds_and_not_another_s() {
    use std::sync::Arc;
    use trp_sim::items::inventory::{go_together, Item, Kind, MAGAZINE_SLOT};
    let Ok(_install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let weapon = |template: &str| Item::of_list(&db, &trp_data::character::inventory::Loadout::of_preset(&db, template).expect("its preset")).expect("put together").0;
    // A spare of the magazine on it, and a stack of its own round.
    let own = |weapon: &Item| {
        let magazine = weapon.in_slot(MAGAZINE_SLOT).expect("a magazine").kind.clone();
        let round = db.items[&weapon.kind.template].props.get("defAmmo").and_then(|round| round.as_str()).expect("a round of its own");
        (Item::new(magazine), Item::new(Arc::new(Kind::of(&db, round).expect("the round"))))
    };
    let (akm, m4) = (weapon("59d6088586f774275f37482f"), weapon("5447a9cd4bdc2dbd208b4567"));
    let ((akm_magazine, akm_round), (m4_magazine, m4_round)) = (own(&akm), own(&m4));
    for (a, b) in [(&akm, &akm_magazine), (&akm, &akm_round), (&akm_magazine, &akm_round), (&m4, &m4_magazine), (&m4, &m4_round), (&m4_magazine, &m4_round)] {
        assert!(go_together(a, b) && go_together(b, a), "{} and {}", a.kind.name, b.kind.name);
    }
    for (a, b) in [(&akm, &m4_magazine), (&akm, &m4_round), (&m4, &akm_magazine), (&m4, &akm_round), (&akm_magazine, &m4_round), (&akm, &m4)] {
        assert!(!go_together(a, b) && !go_together(b, a), "{} and {}", a.kind.name, b.kind.name);
    }
}

/// A magazine's points for its rounds as its model has them, found breadth
/// first (the 6L10: five at its lips and one at its foot), its template's
/// visible ranges (1 to 3), and the AKM's own magazine's points under its
/// slot in the weapon as built.
#[test]
fn a_magazine_s_points_for_rounds_and_its_visible_ranges_are_read() {
    let Ok(install) = crate::fixture::install().ok_or(()) else { return };
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let key = db.items["5a01c29586f77474660c694c"].prefab_key().expect("the 6L10's model").to_owned();
    let (model, _) = trp_data::interface::icon::model_of(&mut assets, &key).expect("the 6L10");
    let points = trp_sim::items::magazine_view::points(model.nodes.iter().map(|node| (node.parent, node.name.as_str())), 0);
    let numbers: Vec<u32> = points.iter().map(|&(_, number)| number).collect();
    println!("the 6L10's points: {numbers:?}");
    assert_eq!(numbers, [1, 2, 3, 4, 5, 30]);
    let kind = trp_sim::items::inventory::Kind::of(&db, "5a01c29586f77474660c694c").expect("its kind");
    assert_eq!(kind.visible_rounds, vec![[1, 3]]);
    assert!(!kind.hide_entrails);
    // A rifle has no visible ranges of its own.
    assert!(trp_sim::items::inventory::Kind::of(&db, "59d6088586f774275f37482f").expect("the AKM").visible_rounds.is_empty());
    let akm = trp_data::weapons::weapon::assemble(&mut assets, &db, "59d6088586f774275f37482f").expect("the AKM");
    let slot = akm.hierarchy.nodes.iter().position(|node| node.name == "mod_magazine").expect("its magazine's slot");
    let magazine = akm.hierarchy.nodes.iter().position(|node| node.parent == Some(slot)).expect("its magazine");
    let points = trp_sim::items::magazine_view::points(akm.hierarchy.nodes.iter().map(|node| (node.parent, node.name.as_str())), magazine);
    println!("the AKM's own magazine's points: {:?}", points.iter().map(|&(_, number)| number).collect::<Vec<_>>());
    assert_eq!(points.len(), 6);
}

/// The player prefab's force reaction (`ForceReact`): what a blast's shake
/// pushes into the springs, read.
#[test]
fn the_force_reaction_is_read() {
    let Ok(install) = crate::fixture::install().ok_or(()) else { return };
    let mut assets = Assets::new(install).expect("manifest");
    let data = trp_data::character::player::HandsData::load(&mut assets).expect("the player prefab's tuning");
    let force = &data.force_react;
    println!("force reaction: intensity {}, noise {} {}, a hit's push shorter by up to {}, vals {:?}; mask {}", force.intensity, force.noise_xy, force.noise_z, force.delta_random, force.vals, data.mask);
    println!("camera rotation spring {:?}, camera position {:?}, hands rotation {:?}", data.camera_rotation, data.camera_position, data.hands_rotation);
    assert!(!force.vals.is_empty());
    assert!(data.mask & 8 != 0, "the prefab's mask has the force reaction");
}
