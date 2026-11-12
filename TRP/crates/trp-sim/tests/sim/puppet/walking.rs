//! Puppets standing and walking, capped by hits and legs, with load inertia and skills.

use super::*;

/// Standing, the body holds the weapon: both palms where the solver puts
/// them, close by the weapon's markers, and the character stays put.
#[test]
fn a_puppet_stands_with_its_weapon_in_hand() {
    let Some(mut p) = puppet() else { return };
    for _ in 0..120 {
        p.step(DT, [0.0, 0.0], &Flat, None);
    }
    let pose = p.pose().expect("a pose");
    println!("feet {:?}, ribcage {:?}, {} body bones, {} rig bones", pose.position, pose.ribcage, pose.bones.len(), pose.rig.len());
    assert_eq!(p.movement.state, State::Idle);
    assert!(distance(pose.position, [0.0; 3]) < 1e-3);
    assert!(pose.ribcage[1] > 1.0 && pose.ribcage[1] < 1.6, "the ribcage {:?}", pose.ribcage);
    for (side, arm) in pose.arms.iter().enumerate() {
        let palm = arm[4].0;
        // The palm within reach of the shoulder: the solver held on.
        let reach = distance(palm, arm[0].0);
        println!("arm {side}: upper at {:?}, palm at {:?}, {reach:.3} m apart", arm[0].0, palm);
        assert!(reach > 0.2 && reach < 0.7, "arm {side} reaches {reach}");
    }
}

/// A character made as the app makes a dummy's stood up with its AKM gone
/// (`Puppet::empty_handed`), and how many bones the empty hands' rig has.
pub(super) fn empty_handed() -> Option<(Puppet, usize)> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database().expect("database");
    let hard = HardSettings::load(&install).expect("settings object");
    let engine = EngineSettings::load(&install).expect("engine settings");
    let mut assets = Assets::new(install).expect("manifest");
    let rig = trp_data::weapons::weapon::assemble_prefab(&mut assets, trp_data::weapons::weapon::EMPTY_HANDS).expect("the empty hands' rig");
    let data = HandsData::load(&mut assets).expect("player prefab");
    let mut hands = Hands::new(trp_sim::empty_hands_settings(data, db, &hard, &engine, &rig, ViewSettings::default()).expect("the empty hands' motion"));
    hands.keep_from(hands.kept());
    let animator = trp_sim::weapons::weapon_animator::WeaponAnimatorData::load(&assets, &rig).expect("the empty hands' animator");
    let body = trp_sim::locomotion::body::BodyData::load(&mut assets).expect("body animator");
    let read = trp_sim::load_movement_settings(&mut assets, db, &hard, &engine, 0.0).expect("movement");
    let settings = read.relimited(&hard, &db.config, 0.0, WeightLimits { unencumbered: true, ..WeightLimits::BASE }).expect("the load's settings");
    let movement = Movement::new(settings, trp_sim::Body::new(&body, rig.cell_width as f32));
    let limbs: [LimbData; 2] = LimbData::load(&mut assets).expect("the arms' solver");
    let pacer = Pacer::new(trp_sim::REFERENCE_FRAME_RATE, engine.fixed_timestep, engine.maximum_timestep);
    let p = Puppet::empty_handed(movement, hands, trp_sim::EmptyHands::new(&animator), &rig.hierarchy, limbs, rig.cell_width as f32, pacer, true).expect("the puppet");
    Some((p, rig.hierarchy.nodes.len()))
}

/// A character with nothing in its hands (`Puppet::empty_handed`, as the
/// app's dummy stood up with its AKM gone): the empty hands' rig with its
/// own animator and controller, the body told so at once; it stands and
/// walks, holds no firearm, and both arms are solved onto the empty hands'
/// markers, within reach.
#[test]
fn a_puppet_with_empty_hands_stands_and_walks() {
    let Some((mut p, nodes)) = empty_handed() else { return };
    assert!(p.firearm().is_none());
    for _ in 0..120 {
        p.step(DT, [0.0, 0.0], &Flat, None);
    }
    let pose = p.pose().expect("a pose");
    println!("empty hands: {} rig bones, ribcage {:?}", pose.rig.len(), pose.ribcage);
    assert_eq!(p.movement.state, State::Idle);
    assert_eq!(pose.rig.len(), nodes);
    for (side, arm) in pose.arms.iter().enumerate() {
        let reach = distance(arm[4].0, arm[0].0);
        println!("arm {side}: upper at {:?}, palm at {:?}, {reach:.3} m apart", arm[0].0, arm[4].0);
        assert!(reach > 0.2 && reach < 0.7, "arm {side} reaches {reach}");
    }
    for _ in 0..90 {
        p.step(DT, [1.0, 0.0], &Flat, None);
    }
    let right = p.pose().expect("a pose").position;
    assert_eq!(p.movement.state, State::Move);
    assert!(right[0] > 0.5, "{right:?}");
}

/// With the walk's key held to its right it steps sideways, and back.
#[test]
fn a_puppet_walks_sideways() {
    let Some(mut p) = puppet() else { return };
    for _ in 0..90 {
        p.step(DT, [1.0, 0.0], &Flat, None);
    }
    let right = p.pose().expect("a pose").position;
    println!("after 1.5 s to the right: {right:?}, state {:?}", p.movement.state);
    assert_eq!(p.movement.state, State::Move);
    assert!(right[0] > 0.5, "{right:?}");
    for _ in 0..180 {
        p.step(DT, [-1.0, 0.0], &Flat, None);
    }
    let left = p.pose().expect("a pose").position;
    println!("then 3 s to the left: {left:?}");
    assert!(left[0] < right[0] - 1.0, "{left:?}");
}

/// A weapon's hit that burns 32.5 of stamina (halfway from 10 to 55) holds
/// the walker's speed to 0.5 for 0.66 s (the game's scale, on which its own
/// top is the walk's 0.625: a fifth off), as the app's dummy takes a bullet
/// (`Dummies::take` → `Movement::hit`); its legs pay the burn.
#[test]
fn a_hit_caps_the_walker_s_speed_for_a_while() {
    let Some((mut p, _, db)) = made(0.0, true) else { return };
    let mut h = health(db);
    for _ in 0..90 {
        p.step(DT, [1.0, 0.0], &Flat, Some(&mut h));
    }
    let (top, walking) = (p.movement.speed_limit(), ground_speed(&p));
    let legs = p.movement.stamina.legs.current;
    p.movement.hit(32.5, Part::Chest, DamageKind::Bullet);
    let burnt = legs - p.movement.stamina.legs.current;
    let mut slowest = f32::MAX;
    let mut held = 0.0;
    for _ in 0..30 {
        p.step(DT, [1.0, 0.0], &Flat, Some(&mut h));
        if p.movement.speed_limit() < top {
            held += DT;
            assert!((p.movement.speed_limit() - 0.5).abs() < 1e-6, "{}", p.movement.speed_limit());
        }
        slowest = slowest.min(ground_speed(&p));
    }
    for _ in 0..60 {
        p.step(DT, [1.0, 0.0], &Flat, Some(&mut h));
        if p.movement.speed_limit() < top {
            held += DT;
        }
    }
    let after = ground_speed(&p);
    println!("the walker: limit {top:.3}, {walking:.3} m/s before; held to 0.5 for {held:.3} s, down to {slowest:.3} m/s; {after:.3} m/s after; the legs {legs:.2}, {burnt:.2} burnt");
    assert!((top - p.movement.settings.max_speed).abs() < 1e-6 && top > 0.5, "{top}");
    assert!((held - 0.66).abs() <= 2.0 * DT, "held {held}");
    assert!(slowest < 0.95 * walking, "{slowest} of {walking}");
    assert!((after - walking).abs() < 0.05 * walking, "{after} against {walking}");
    assert!((burnt - 32.5).abs() < 1e-3, "{burnt}");
}

/// A broken leg, followed by the character's health after its frames,
/// holds its speed to 0.3 (the walk's top 0.625) and lets no sprint start;
/// the walker walks the slower for it.
#[test]
fn a_broken_leg_caps_the_walker_and_stops_its_sprint() {
    let Some((mut p, _, db)) = made(0.0, true) else { return };
    let mut h = health(db);
    for _ in 0..90 {
        p.step(DT, [1.0, 0.0], &Flat, Some(&mut h));
    }
    let (top, walking) = (p.movement.speed_limit(), ground_speed(&p));
    assert!(!p.movement.conditions.leg_damaged() && p.movement.stamina.health_lets_sprint);
    h.do_fracture(Part::LeftLeg);
    for _ in 0..120 {
        p.step(DT, [1.0, 0.0], &Flat, Some(&mut h));
    }
    let limping = ground_speed(&p);
    println!("the walker: limit {top:.3} and {walking:.3} m/s whole; a broken left leg: limit {:.3}, {limping:.3} m/s", p.movement.speed_limit());
    assert!(p.movement.conditions.left_leg && !p.movement.conditions.on_painkillers);
    assert!((p.movement.speed_limit() - 0.3).abs() < 1e-6, "{}", p.movement.speed_limit());
    assert!(limping < 0.6 * walking, "{limping} of {walking}");
    assert!(!p.movement.stamina.health_lets_sprint);
    // The sprint's key held (as a host's keys would) starts nothing on the
    // broken leg, where the same keys start one on a whole character.
    let (forward, sprint) = (Input { move_dir: [0.0, 1.0], ..Input::default() }, Input { move_dir: [0.0, 1.0], sprint: true, ..Input::default() });
    let Some(mut whole) = puppet() else { return };
    for frame in 0..120 {
        let keys = if frame < 30 { &forward } else { &sprint };
        p.movement.frame(DT, keys, &Flat);
        h.follow(p.movement.player_dt(), &mut p.movement);
        whole.movement.frame(DT, keys, &Flat);
    }
    println!("forward 0.5 s, then the sprint's key held 1.5 s: {:?} on the broken leg, {:?} whole", p.movement.state, whole.movement.state);
    assert_ne!(p.movement.state, State::Sprint);
    assert_eq!(whole.movement.state, State::Sprint);
}

/// A dummy's load as a bot's (`EncumberDisabled`): its kit's weight gives
/// its inertia as the player's would, and no overweight however heavy,
/// the sprint's included, where the player's encumbrance makes one.
#[test]
fn a_bot_s_load_gives_its_inertia_and_no_overweight() {
    let Some(install) = crate::fixture::install() else { return };
    let db = crate::fixture::database().expect("database");
    // The BEAR's starting kit (the walker's), its magazines the AKM's; with
    // its own rifle (the app's dummy holds the AKM in its place).
    let mut list = trp_data::character::inventory::Loadout::load(&install, "Standard", "bear").expect("the BEAR's kit");
    list.fit_to(db, AKM).expect("fitted to the AKM");
    let (kit, _) = trp_sim::items::inventory::Equipment::of_loadout(db, &list).expect("the kit");
    let kit_weight = trp_sim::items::inventory::carried_weight(&kit.item, false);
    for weight in [kit_weight, 60.0] {
        let (Some((bot, ..)), Some((player, ..))) = (made(weight, true), made(weight, false)) else { return };
        let (b, s) = (&bot.movement.settings, &player.movement.settings);
        println!(
            "{weight:.2} kg: the inertia {:.4} (the player's {:.4}); overweight {:.3}, walk's {:.3}, sprint's speed {:.3}, the state's limit {:.3} (the player's {:.3}, {:.3}, {:.3}, {:.3})",
            b.inertia, s.inertia, b.overweight, b.stamina.walk_overweight, b.physical_sprint_speed, b.state_speed_limit, s.overweight, s.stamina.walk_overweight, s.physical_sprint_speed, s.state_speed_limit
        );
        assert_eq!(b.inertia, s.inertia);
        assert_eq!(b.floor_key, b.inertia);
        assert!(b.inertia > 0.0);
        assert_eq!((b.overweight, b.stamina.overweight, b.stamina.walk_overweight), (0.0, 0.0, 0.0));
        assert_eq!((b.physical_sprint_speed, b.sprint_sensitivity, b.state_speed_limit), (1.0, 1.0, 1.0));
        assert!(b.limits.unencumbered && !s.limits.unencumbered);
        if weight == 60.0 {
            // The player's encumbrance at 60 kg: overweight on every count.
            assert!(s.overweight > 0.0 && s.stamina.walk_overweight > 0.0 && s.physical_sprint_speed < 1.0 && s.state_speed_limit < 1.0);
        }
    }
    // The limits as such: out of reach whatever the carrying factor.
    let limits = WeightLimits { carry: 1.3, unencumbered: true, ..WeightLimits::BASE };
    assert_eq!(limits.overweight_limit([20.0, 40.0]), [9000.0, 10000.0]);
    assert_eq!(WeightLimits { carry: 1.3, ..WeightLimits::BASE }.overweight_limit([20.0, 40.0]), [26.0, 52.0]);
}

/// The weight as the inventory tells of it (movement spec, "The weight, as
/// the inventory tells of it"), as the app's player goes by it: five shots
/// from the BEAR's AKM leave the character's weight and inertia as they
/// were (the game's recorded weight stands over 25 shots), and the reload's
/// end, the emptied magazine stowed in the gear, works them out again with
/// the rounds gone; a change of 0.01 kg or less changes nothing.
#[test]
fn shots_leave_the_inertia_and_the_reload_s_stowing_changes_it() {
    use trp_sim::items::inventory::{carried_weight, set_rounds_of, magazine_rounds, Weighed};
    let Some(install) = crate::fixture::install() else { return };
    let db = crate::fixture::database().expect("database");
    let mut list = trp_data::character::inventory::Loadout::load(&install, "Standard", "bear").expect("the BEAR's kit");
    list.fit_to(db, AKM).expect("fitted to the AKM");
    let (mut kit, _) = trp_sim::items::inventory::Equipment::of_loadout(db, &list).expect("the kit");
    let before = carried_weight(&kit.item, false);
    let Some((mut p, hard, db)) = made(before, false) else { return };
    let mut weighed = Weighed::default();
    let weigh = |p: &mut Puppet, weighed: &mut Weighed, kit: &trp_sim::items::inventory::Equipment| -> bool {
        weighed.told(&kit.item) && p.movement.weigh(&hard, &db.config, carried_weight(&kit.item, false)).expect("weighed")
    };
    assert!(!weigh(&mut p, &mut weighed, &kit), "the character made with the kit's weight");
    let inertia = p.movement.settings.inertia;
    let gun = kit.item.in_slot("FirstPrimaryWeapon").expect("the AKM").id;
    let magazine = kit.item.find(gun).and_then(|gun| gun.in_slot("mod_magazine")).expect("its magazine");
    let (magazine, rounds) = (magazine.id, magazine_rounds(magazine));
    assert!(rounds >= 5, "{rounds} rounds");
    // Five shots, the rounds out of the magazine as the hands take them,
    // nothing having changed before: passed over.
    let quiet = weighed.as_told(&kit.item);
    assert!(quiet && set_rounds_of(&mut kit.item, magazine, rounds - 5));
    weighed.pass(&kit.item);
    let fired = carried_weight(&kit.item, false);
    assert!(before - fired > 0.05, "five rounds weigh {}", before - fired);
    assert!(!weigh(&mut p, &mut weighed, &kit));
    assert_eq!((p.movement.settings.weight, p.movement.settings.inertia), (before, inertia));
    // The reload's end: the emptied magazine out of the AKM and stowed.
    let gun_item = kit.item.find_mut(gun).unwrap();
    let at = gun_item.slot_index("mod_magazine").unwrap();
    let out = gun_item.slots[at].take().unwrap();
    let stowed = ["TacticalVest", "Pockets", "Backpack"].into_iter().any(|slot| {
        let at = kit.item.slot_index(slot);
        at.and_then(|at| kit.item.slots[at].as_mut()).is_some_and(|holder| holder.stow(out.clone()).is_ok())
    });
    assert!(stowed, "room for the magazine");
    assert!(weigh(&mut p, &mut weighed, &kit), "the stowing works it out again");
    println!(
        "the kit {before:.3} kg, inertia {inertia:.5}; five shots: {fired:.3} kg, unheard; the magazine stowed: {:.3} kg, inertia {:.5}",
        p.movement.settings.weight, p.movement.settings.inertia
    );
    assert!((p.movement.settings.weight - fired).abs() < 1e-5);
    assert!(p.movement.settings.inertia < inertia);
    // A change within the game's step of 0.01 kg changes nothing; past it, it does.
    let inertia = p.movement.settings.inertia;
    assert!(!p.movement.weigh(&hard, &db.config, fired + 0.009).expect("weighed"));
    assert_eq!(p.movement.settings.inertia, inertia);
    assert!(p.movement.weigh(&hard, &db.config, fired + 0.011).expect("weighed"));
    // What it takes to tell: the kit's number, a thousand times.
    let began = std::time::Instant::now();
    for _ in 0..1000 {
        std::hint::black_box(weighed.as_told(&kit.item));
    }
    println!("the kit told of in {:.1} us", began.elapsed().as_secs_f64() * 1000.0);
}

/// Its skills at level 1: Strength's top
/// speeds, jump and inertia step, Endurance's stamina, Covert Movement's
/// steps, the weapon's class and Aim Drills on the hands and the firearm,
/// the health's and the vests' buffs; encumbrance stays off.
#[test]
fn its_skills_at_level_1_reach_its_movement_weapon_health_and_armour() {
    let Some((mut p, hard, db)) = made(20.0, true) else { return };
    let levels = SkillLevels::all_at(1);
    assert!(Skill::ALL.iter().all(|s| levels.level(*s) == 1));
    let buffs = levels.buffs();
    let class = Skill::of_weapon(&db.class_names(AKM));
    assert_eq!(class, Some(Skill::Assault));
    let (speed, inertia, capacity) = (p.movement.settings.max_speed, p.movement.settings.inertia, p.movement.stamina.legs.capacity);
    p.take_skills(&buffs, class, 1.0 + buffs.strength_lift, &hard, &db.config).expect("the skills");
    let s = &p.movement.settings;
    let firearm = p.firearm().expect("its AKM in hand");
    println!(
        "level 1: top speed {speed:.5} to {:.5}, jump x{:.3}, inertia {inertia:.5} to {:.5} (limits {:?}), legs {capacity:.2} to {:.2}, covert {:?}, aim speed +{:.3}, ergonomics +{:.3}, recoil -{:.3}, reload x{:.3}, draw x{:.3}",
        s.max_speed, s.jump_boost, s.inertia, s.limits, p.movement.stamina.legs.capacity, buffs.covert, p.hands.settings.aim_speed_bonus, p.hands.settings.ergonomics_buff, p.hands.settings.recoil_suppression, firearm.settings.reload_speed, firearm.settings.draw_speed
    );
    // The walk's top a sixtieth of the way up the globals' `WalkSpeed`.
    let walk = |k: &str| db.config["WalkSpeed"][k].as_f64().expect("WalkSpeed") as f32;
    assert!((s.max_speed - (walk("x") + (walk("y") - walk("x")) / 60.0)).abs() < 1e-6, "{}", s.max_speed);
    assert!((s.jump_boost - 1.004).abs() < 1e-6);
    assert!(s.limits.strength_level == 1.0 && (s.limits.carry - 1.006).abs() < 1e-6 && s.limits.unencumbered, "{:?}", s.limits);
    // The inertia's limits a step up: 20 kg carried no more heavily.
    assert!(s.inertia <= inertia);
    assert_eq!(s.overweight, 0.0);
    assert!((p.movement.stamina.legs.capacity - capacity * 1.01).abs() < 1e-3);
    assert!((p.hands.settings.aim_speed_bonus - 0.01).abs() < 1e-6);
    assert!((p.hands.settings.ergonomics_buff - 0.002).abs() < 1e-6);
    assert!((firearm.settings.reload_speed - 1.004).abs() < 1e-6 && (firearm.settings.draw_speed - 1.008).abs() < 1e-6);
    let health = HealthSkills::of_levels(|name| levels.level_of(name));
    assert!((health.stress_pain - 0.01).abs() < 1e-6 && (health.bleed_chance - 0.012).abs() < 1e-6 && !health.bleed_stop);
    let vests = trp_sim::condition::armor::VestSkills::of(&levels);
    assert!((vests.light_speed - 0.006).abs() < 1e-6 && (vests.heavy_speed - 0.005).abs() < 1e-6 && (vests.heavy_blunt - 0.004).abs() < 1e-6);
    assert!(!vests.heavy_deflect && !vests.light_bleed_block);
}
