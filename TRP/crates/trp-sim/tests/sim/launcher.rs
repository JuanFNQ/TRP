//! Tests of the underbarrel launcher in the hands, on the install's own controllers.
//! After `docs/spec/underbarrel-launcher.md`: the AKM with the GP-25 and the M4A1 with the M203. Set
//! beside `courses3\1-launched` (recorder 0.11.0, by hand, the AKM with the GP-25 and
//! VOG-25s: `local\ubgl_s234\rec_states.txt`): each shot's `FIRE` from a fifth of its
//! clip on the press's frame and the rest again 0.167 s later, each reload's `RELOAD`
//! 2.508 s, the switch back's `WEAPON TO GRIP` 0.658 s, and the AKM resting after it on
//! `akm_idle_armed` (its `IDLE` at 1.667 cycles a second: `LauncherID` -1). Passes with a
//! note where no install is configured.

use trp_sim::weapons::firearm::{LauncherData, LauncherEvent};
use trp_sim::weapons::weapon_animator::WeaponAnimatorData;
use trp_sim::{Firearm, FirearmEvent};

/// The AKM's encyclopedia build with the GP-25 on its `mod_launcher`.
const AKM_GP25: &str = "59d6088586f774275f37482f[mod_gas_block=59d64ec286f774171d1e0a42[mod_handguard=59d64f2f86f77417193ef8b3[]],mod_launcher=62e7e7bbe6da9612f743f1e0[],mod_magazine=59d625f086f774661516605d[],mod_muzzle=59d64fc686f774171b243fe2[],mod_pistol_grip=59e62cc886f77440d40b52a1[],mod_reciever=59d6507c86f7741b846413a2[],mod_sight_rear=59d650cf86f7741b846413a4[],mod_stock=59d6514b86f774171a068a08[]]";
/// The Arena trader's M4A1 with the M203 on its 14.5" barrel.
const M4_M203: &str = "5447a9cd4bdc2dbd208b4567[mod_charge=5b2240bf5acfc40dc528af69[],mod_magazine=55802d5f4bdc2dac148b458e[],mod_pistol_grip=55802f5d4bdc2dac148b458f[],mod_reciever=55d355e64bdc2d962f8b4569[mod_barrel=55d3632e4bdc2d972f8b4569[mod_gas_block=56eabcd4d2720b66698b4574[],mod_launcher=6357c98711fb55120211f7e1[],mod_muzzle=5cf6937cd7f00c056c53fb39[]],mod_handguard=5d122e7bd7ad1a07102d6d7f[mod_mount_000=5d124c0ed7ad1a10d168dd9b[],mod_mount_001=5d124c0ed7ad1a10d168dd9b[],mod_sight_front=5c17804b2e2216152006c02f[],mod_tactical_000=544909bb4bdc2d6f028b4577[]],mod_scope=558022b54bdc2dac148b458d[],mod_sight_rear=5c1780312e221602b66cc189[]],mod_stock=5947e98b86f774778f1448bc[]]";

/// The recording's frame (`1-launched`'s `dt`, about 120 a second).
const FRAME: f32 = 0.0083339;

/// The weapon built from its key, drawn and at rest, its launcher loaded.
fn weapon(key: &str) -> Option<Firearm> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = trp_data::unity::Assets::new(install).expect("manifest");
    let build = trp_data::weapons::weapon::assemble_key(&mut assets, db, key).expect("the build");
    assert!(build.launcher.is_some(), "the build has its launcher");
    let data = WeaponAnimatorData::load(&assets, &build).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, db, &hard, &build).expect("its settings");
    let launcher = LauncherData::load(&assets, &build).expect("the launcher read").expect("the launcher's controller");
    let mut w = Firearm::new(settings, &data);
    w.fit_launcher(&launcher, true);
    w.start_drawn();
    Some(w)
}

/// Steps until `done` holds after a frame, at most `limit` seconds: the time it took
/// and the events on the way.
fn until(w: &mut Firearm, limit: f32, done: impl Fn(&Firearm, &[FirearmEvent]) -> bool) -> (f32, Vec<FirearmEvent>) {
    let mut said = Vec::new();
    let mut time = 0.0;
    while time < limit {
        let events = w.step(FRAME);
        time += FRAME;
        let stop = done(w, &events);
        said.extend(events);
        if stop {
            return (time, said);
        }
    }
    panic!("not done in {limit} s; operation {}, launcher {:?}", w.operation_said(), w.launcher_animator().map(|a| a.state().0.to_owned()));
}

fn launcher_state(w: &Firearm) -> String {
    w.launcher_animator().map(|a| a.state().0.to_owned()).unwrap_or_default()
}

fn has(events: &[FirearmEvent], wanted: LauncherEvent) -> bool {
    events.iter().any(|e| *e == FirearmEvent::Launcher(wanted))
}

#[test]
fn the_gp25_is_used_as_recorded() {
    let Some(mut w) = weapon(AKM_GP25) else { return };
    // The rifle's animator told of its launcher as the hands were made.
    assert_eq!(w.animator().get("LauncherID"), -1.0);
    assert!(!w.launcher_speaks());

    // In: the slot key again, ended at `LauncherAppeared`, near the switch's start.
    assert!(w.toggle_launcher());
    assert!(w.launcher_speaks(), "the launcher's animator is the hands' from the press");
    let (took, said) = until(&mut w, 1.0, |w, _| w.in_launcher_mode());
    assert!(has(&said, LauncherEvent::Switched(true)));
    assert!(took < 0.05, "LauncherAppeared at 0.003 of the switch: {took} s");
    until(&mut w, 2.0, |w, _| launcher_state(w) == "Hands.IDLE WEAPON");

    // A shot: `FIRE` from a fifth of its clip on the press's frame, the round out at
    // `FiringBullet`, the rest's state again 0.167 s after the press (recorded).
    w.set_aim(true, true);
    w.set_trigger(true, true);
    let first = w.step(FRAME);
    let animator = w.launcher_animator().unwrap();
    assert_eq!(animator.state().0, "Hands.FIRE");
    assert!(has(&first, LauncherEvent::Shot { case_kept: false }), "the GP-25's round leaves no case");
    assert_eq!(w.launcher_chamber(), Some((false, false)));
    let (back, said) = until(&mut w, 1.0, |w, _| launcher_state(w) == "Hands.IDLE WEAPON");
    let back = back + FRAME;
    assert!((back - 0.167).abs() <= FRAME * 1.5, "the rest again {back} s after the press, recorded 0.167 s");
    assert!(has(&said, LauncherEvent::Recoil), "the recoil a frame after the shot");
    assert!(w.aiming(), "the sights held through the shot (recorded)");
    w.set_trigger(false, true);
    w.set_aim(false, true);
    w.step(FRAME);

    // Empty, the trigger only clicks.
    w.set_trigger(true, true);
    let said = w.step(FRAME);
    assert!(said.contains(&FirearmEvent::DryFire));
    assert!(matches!(w.launcher_chamber(), Some((false, _))));
    w.set_trigger(false, true);
    // (The click plays the fire state's clip, empty.)
    until(&mut w, 1.0, |w, _| launcher_state(w) == "Hands.IDLE WEAPON");

    // A reload by the key: the round in at once, `RELOAD` for 2.508 s (recorded),
    // `AddAmmoInChamber` at 0.795 of it.
    assert!(w.reload_launcher(true));
    assert_eq!(w.launcher_chamber(), Some((true, false)), "the round goes in as the reload begins");
    let (began, _) = until(&mut w, 1.0, |w, _| launcher_state(w) == "Hands.RELOAD");
    assert!(began <= FRAME * 1.5, "RELOAD from the press's frame (recorded), here after {began} s");
    let (took, _) = until(&mut w, 4.0, |w, _| launcher_state(w) == "Hands.IDLE WEAPON");
    let took = took + began - FRAME;
    assert!((took - 2.508).abs() <= FRAME * 2.0, "RELOAD for {took} s, recorded 2.508 s");
    // Loaded, the key does nothing: no exchange.
    assert!(!w.reload_launcher(true));

    // The range: one step a press, held at the ends; the animator's follows only on
    // the rest, at 400 m a second in whole metres.
    assert_eq!(w.launcher_animator().unwrap().get("SightingRange"), 0.0, "untouched until a press (recorded: 0 throughout)");
    assert_eq!(w.launcher_range_step(false), Some(100));
    assert_eq!(w.launcher_range_step(true), Some(150));
    until(&mut w, 1.0, |w, _| w.launcher_animator().unwrap().get("SightingRange") == 150.0);

    // Out: `WEAPON TO GRIP`, ended at its `LauncherDisappeared`, 0.658 s (recorded).
    assert!(w.toggle_launcher());
    let (took, said) = until(&mut w, 2.0, |w, _| !w.launcher_speaks());
    assert!(has(&said, LauncherEvent::Switched(false)));
    assert!((took - 0.658).abs() <= FRAME * 1.5, "the switch back took {took} s, recorded 0.658 s");
    assert!(w.is_idling());
    // The rifle's rest on `akm_idle_armed`: 0.300 s at speed 0.5, as recorded.
    assert_eq!(w.animator().get("LauncherID"), -1.0);
}

#[test]
fn a_put_away_on_the_launcher_switches_back_first() {
    let Some(mut w) = weapon(AKM_GP25) else { return };
    assert!(w.toggle_launcher());
    until(&mut w, 1.0, |w, _| w.in_launcher_mode());
    w.hide(false);
    assert!(w.launcher_speaks(), "the switch back plays first");
    let (_, said) = until(&mut w, 4.0, |w, _| !w.in_hand());
    assert!(has(&said, LauncherEvent::Switched(false)));
    assert!(said.contains(&FirearmEvent::Hidden));
}

#[test]
fn the_m203_keeps_its_case_until_the_next_load() {
    let Some(mut w) = weapon(M4_M203) else { return };
    assert!(w.toggle_launcher());
    until(&mut w, 1.0, |w, _| w.in_launcher_mode());
    until(&mut w, 2.0, |w, _| launcher_state(w) == "Hands.IDLE WEAPON");
    w.set_trigger(true, true);
    let said = w.step(FRAME);
    assert!(has(&said, LauncherEvent::Shot { case_kept: true }));
    assert_eq!(w.launcher_chamber(), Some((false, true)));
    w.set_trigger(false, true);
    until(&mut w, 1.0, |w, _| launcher_state(w) == "Hands.IDLE WEAPON");
    // The reload throws it out at its `ShellEject`, at a third of the player's speed.
    assert!(w.reload_launcher(true), "a spent case alone is an empty chamber");
    let (_, said) = until(&mut w, 5.0, |w, _| launcher_state(w) == "Hands.IDLE WEAPON");
    assert!(has(&said, LauncherEvent::CaseThrown(0.33)));
    assert_eq!(w.launcher_chamber(), Some((true, false)));
}

/// What the launcher's model shows (underbarrel-launcher spec, "Reloading and
/// unloading"; `UnderbarrelContainer`, read from the code): the round's model in its
/// chamber as the hands are made with one; at the shot moved to the shell port as the
/// case (the M203's) or gone (the GP-25's); the reload's round drawn from its `MagShow`,
/// though it is in the chamber from the start; a look into the M203's chamber with a
/// case in it (`CHECK SHELL`) throws the case at its `ShellEject`, so the reload after
/// has none to throw; the discharge's round gone at its `DelAmmoChamber`.
#[test]
fn the_launcher_s_round_and_case_are_drawn_as_its_marks_move_them() {
    let Some(mut w) = weapon(M4_M203) else { return };
    assert_eq!(w.launcher_models(), Some((true, false)), "the round drawn as the hands are made with it");
    assert!(w.toggle_launcher());
    until(&mut w, 1.0, |w, _| w.in_launcher_mode());
    until(&mut w, 2.0, |w, _| launcher_state(w) == "Hands.IDLE WEAPON");
    w.set_trigger(true, true);
    w.step(FRAME);
    assert_eq!(w.launcher_models(), Some((false, true)), "the round to the shell port as its case");
    w.set_trigger(false, true);
    until(&mut w, 1.0, |w, _| launcher_state(w) == "Hands.IDLE WEAPON");

    // A look into the chamber: `CHECK SHELL`, its `ShellEject` at 0.682 of 4.267 s.
    w.check_chamber();
    let thrown = FirearmEvent::Launcher(LauncherEvent::CaseThrown(0.33));
    let (began, said) = until(&mut w, 1.0, |w, _| launcher_state(w) == "Hands.CHECK SHELL");
    assert!(!said.contains(&thrown));
    let (took, said) = until(&mut w, 4.0, |_, said| said.contains(&thrown));
    let took = took + began - FRAME;
    assert!((took - 0.682 * 4.267).abs() < 0.05, "thrown {took} s into the look");
    assert_eq!(said.iter().filter(|e| **e == thrown).count(), 1);
    assert_eq!((w.launcher_models(), w.launcher_chamber()), (Some((false, false)), Some((false, false))));
    until(&mut w, 3.0, |w, _| launcher_state(w) == "Hands.IDLE WEAPON");
    for _ in 0..30 {
        w.step(FRAME);
    }

    // The reload: the round in at once, its model from `MagShow`; no case to throw.
    assert!(w.reload_launcher(true));
    assert_eq!((w.launcher_chamber(), w.launcher_models()), (Some((true, false)), Some((false, false))));
    let (_, said) = until(&mut w, 5.0, |w, _| w.launcher_models() == Some((true, false)));
    assert!(has(&said, LauncherEvent::RoundShown));
    let (_, said) = until(&mut w, 5.0, |w, _| launcher_state(w) == "Hands.IDLE WEAPON");
    assert!(!has(&said, LauncherEvent::CaseThrown(0.33)), "the look threw it");

    // The discharge: the round's model gone at `DelAmmoChamber`.
    assert!(w.discharge_launcher());
    let (_, said) = until(&mut w, 5.0, |w, _| w.launcher_models() == Some((false, false)));
    assert!(has(&said, LauncherEvent::RoundRemoved));
    assert_eq!(w.launcher_chamber(), Some((true, false)), "the round still the chamber's until the end");

    // The GP-25's round leaves no case: its model gone at the shot.
    let Some(mut w) = weapon(AKM_GP25) else { return };
    assert!(w.toggle_launcher());
    until(&mut w, 1.0, |w, _| w.in_launcher_mode());
    until(&mut w, 2.0, |w, _| launcher_state(w) == "Hands.IDLE WEAPON");
    w.set_trigger(true, true);
    w.step(FRAME);
    assert_eq!(w.launcher_models(), Some((false, false)));
}

/// The rig posed as a host poses it: the rifle's animator's pose with the
/// launcher's over its bones, the weapon's bone carried by the prop bone while
/// the launcher's bones are in use; and the angle in degrees from the launcher's
/// sight's line (its aim bone's -z, as the eye looks along a `mod_aim_camera`) up
/// to the way its barrel points (its fireport's -y, the shot's direction with no
/// sight's solution: underbarrel-launcher spec, "Firing").
fn barrel_above_sight(w: &Firearm, rig: &mut trp_sim::base::rig::WeaponRig) -> f32 {
    use trp_sim::base::math::quat_rotate;
    let bones = w.launcher_bones().expect("its bones");
    let mut pose = w.animator().pose().to_vec();
    w.put_launcher_pose(&mut pose, false);
    let weapon = rig.bone("Weapon_root/Weapon_root_anim/weapon").expect("the weapon's bone");
    if w.launcher_bones_in_use() {
        rig.carry_by(&mut pose, weapon, bones.prop.expect("its prop bone"));
    }
    rig.pose(&pose, None);
    let fire = quat_rotate(rig.placed(bones.fireport.expect("its fireport")).1, [0.0, -1.0, 0.0]);
    let look = quat_rotate(rig.placed(bones.sight.expect("its sight")).1, [0.0, 0.0, -1.0]);
    let dot = (0..3).map(|k| fire[k] * look[k]).sum::<f32>().clamp(-1.0, 1.0);
    dot.acos().to_degrees()
}

/// The prop bone and the launcher's sight. At rest on the rifle the prop bone is
/// the weapon's own bone (its rest inverts the slot's place), so carrying by it
/// moves nothing; on the launcher's rest its bones are in use from the press. The
/// barrel stands above the sight's line by the leaf's turn alone (the prop bone
/// carries both): 5 degrees at the first step, where the animator's range stays
/// until a key is pressed (as in `1-launched`), 8 at 150 m (each range clip held
/// at its end). Not matched: the recording's grenades left 7.3 degrees (±0.4)
/// above the camera's forward at that first step (`bin/bullets`), so
/// the camera does not look along the sight's line there; how the aim puts it is
/// the grenade step's to compare (underbarrel-launcher spec, "Compared with").
#[test]
fn the_gp25_s_barrel_stands_above_its_sight_by_the_leaf() {
    let Some(install) = crate::fixture::install() else { return };
    let db = crate::fixture::database().expect("database");
    let mut assets = trp_data::unity::Assets::new(install).expect("manifest");
    let build = trp_data::weapons::weapon::assemble_key(&mut assets, db, AKM_GP25).expect("the build");
    let Some(mut w) = weapon(AKM_GP25) else { return };
    let mut rig = trp_sim::base::rig::WeaponRig::new(&build.hierarchy);
    let bones = w.launcher_bones().expect("its bones");
    let weapon = rig.bone("Weapon_root/Weapon_root_anim/weapon").expect("the weapon's bone");
    let (at, prop) = (rig.placed(weapon), rig.placed(bones.prop.expect("its prop bone")));
    let apart = (0..3).map(|k| (at.0[k] - prop.0[k]).powi(2)).sum::<f32>().sqrt();
    let turned = trp_sim::base::math::quat_mul([-at.1[0], -at.1[1], -at.1[2], at.1[3]], prop.1)[3].abs().min(1.0).acos().to_degrees() * 2.0;
    assert!(apart < 0.001 && turned < 0.1, "at rest the prop bone is {apart} m and {turned} degrees from the weapon's bone");
    assert!(!w.launcher_bones_in_use());
    let resting = barrel_above_sight(&w, &mut rig);
    assert!(w.toggle_launcher());
    assert!(w.launcher_bones_in_use(), "its bones from the press");
    until(&mut w, 1.0, |w, _| w.in_launcher_mode());
    until(&mut w, 2.0, |w, _| launcher_state(w) == "Hands.IDLE WEAPON");
    let first = barrel_above_sight(&w, &mut rig);
    w.launcher_range_step(true);
    for _ in 0..60 {
        w.step(FRAME);
    }
    let next = barrel_above_sight(&w, &mut rig);
    println!("the barrel above the sight's line: {resting:.2} degrees at rest on the rifle, {first:.2} on the launcher's rest at the first step, {next:.2} at 150 m (recorded at the first step: 7.3 +- 0.4 above the camera's forward)");
    assert!((first - 5.0).abs() < 0.05 && (next - 8.0).abs() < 0.05, "{first}, {next}");
    // Back on the rifle its bones are the rifle's again.
    assert!(w.toggle_launcher());
    until(&mut w, 2.0, |w, _| !w.launcher_bones_in_use());
    assert!(w.has_launcher() && !w.in_launcher_mode());
}

/// The sights through the switch (underbarrel-launcher spec, "The switch": nothing
/// on either way puts them down; the aim key does nothing while it plays). Read
/// from the code; not recorded.
#[test]
fn the_sights_stay_up_through_the_switch() {
    let Some(mut w) = weapon(AKM_GP25) else { return };
    w.set_aim(true, true);
    for _ in 0..30 {
        w.step(FRAME);
    }
    assert!(w.aiming(), "aimed on the rifle");
    assert!(w.toggle_launcher());
    until(&mut w, 1.0, |w, _| w.in_launcher_mode());
    assert!(w.aiming(), "still aimed on the launcher's rest");
    for _ in 0..60 {
        w.step(FRAME);
    }
    assert!(w.aiming(), "still aimed after it");
}

/// The rounds' fuses as the install has them (`install.md`, "The ammunition"): the
/// VOG-25 armed after 0.05 s, its effect `smallgrenade_expl` and no material's hit
/// besides; the M406 after 0.26 s; the 12.7x108's dummy (`big_round_impact`, its own hit
/// played too); an ordinary round none.
#[test]
fn the_rounds_fuses_are_the_install_s() {
    let Some(db) = crate::fixture::database() else { return eprintln!("no install") };
    let fuse = |id: &str| db.items.get(id).and_then(trp_sim::combat::ballistics::Fuse::of);
    let vog = fuse("5656eb674bdc2d35148b457c").expect("the VOG-25's");
    assert_eq!((vog.arm_time, vog.effect.as_deref(), vog.hit_effect_too, vog.dummy), (0.05, Some("smallgrenade_expl"), false, false));
    let m406 = fuse("5ede4739e0350d05467f73e8").expect("the M406's");
    assert_eq!((m406.arm_time, m406.dummy), (0.26, false));
    let big = db.items.values().filter_map(trp_sim::combat::ballistics::Fuse::of).find(|f| f.effect.as_deref() == Some("big_round_impact")).expect("a big round's");
    assert!(big.dummy && big.hit_effect_too, "{big:?}");
    let akm_round = db.items["59d6088586f774275f37482f"].props["defAmmo"].as_str().expect("the AKM's round");
    assert!(fuse(akm_round).is_none(), "an ordinary round has none");
}

/// The GP-25's shot as `LauncherShot` takes it: its ghost's centre of impact 0.7, its
/// speed the round's (76 m/s recorded: no `Velocity`), its barrel's deviation 1 at full
/// durability (its ghost's curve 2.9 and greatest 15).
#[test]
fn the_gp25_s_shot_takes_its_ghost_s_figures() {
    let Some(db) = crate::fixture::database() else { return eprintln!("no install") };
    let f = trp_sim::combat::ballistics::WeaponFigures::of_launcher(db, "62e7e7bbe6da9612f743f1e0", &[]).expect("the GP-25's");
    assert!((f.centre_of_impact - 0.7).abs() < 1e-6, "{}", f.centre_of_impact);
    assert!((f.speed_factor - 1.0).abs() < 1e-6, "{}", f.speed_factor);
    assert!((f.deviation_curve - 2.9).abs() < 1e-6 && (f.deviation_max - 15.0).abs() < 1e-6, "{} {}", f.deviation_curve, f.deviation_max);
    assert!((f.barrel_deviation(100.0) - 1.0).abs() < 1e-4);
}

/// The muzzles of a rifle with a launcher (`MuzzleManager.UpdateJetsAndFumes`): the
/// rifle's is never under its `mod_launcher` (on the AKM the deepest effect in the whole
/// tree is the GP-25's), the launcher's own is read over its own tree, and the
/// launcher's fireport is its own (the M4A1's first is its barrel's).
#[test]
fn a_launcher_s_muzzle_is_its_own() {
    let Some(install) = crate::fixture::install() else { return eprintln!("no install") };
    let db = crate::fixture::database().expect("database");
    let mut assets = trp_data::unity::Assets::new(install).expect("manifest");
    for key in [AKM_GP25, M4_M203] {
        let build = trp_data::weapons::weapon::assemble_key(&mut assets, db, key).expect("the build");
        let h = &build.hierarchy;
        let part = build.launcher.as_ref().expect("its launcher");
        let under = |mut node: usize, root: usize| loop {
            if node == root {
                return true;
            }
            match h.nodes[node].parent {
                Some(parent) => node = parent,
                None => return false,
            }
        };
        let rifle = build.muzzle.as_ref().expect("the rifle's muzzle");
        assert!(!rifle.jets.is_empty(), "{}: the rifle's jets", build.name);
        assert!(rifle.jets.iter().all(|jet| !under(jet.node, part.node)), "{}: the rifle's jets are its own", build.name);
        let own = build.launcher_muzzle.as_ref().expect("the launcher's muzzle");
        assert!(!own.jets.is_empty() && own.jets.iter().all(|jet| under(jet.node, part.node)), "{}: the launcher's jets", build.name);
        let fireport = part.fireport.expect("the launcher's fireport");
        assert!(under(fireport, part.node) && h.nodes[fireport].name == "fireport");
        let first = h.nodes.iter().position(|node| node.name == "fireport").expect("a fireport");
        assert!(!under(first, part.node), "{}: the rifle's fireport comes first", build.name);
    }
}
