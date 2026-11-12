//! Hits, blasts, the load, the kit's ergonomics, damaged arms and the hip penalty.

use super::*;

/// A hit's push (`ForceEffector.AddForce`; health spec, "A hit and health's
/// other screen effects"): barbed wire's 1 to a leg turns the camera less
/// than a head's 15, both settle, and the hands turn too.
#[test]
fn a_hit_pushes_the_camera_and_the_hands() {
    let Some((mut leg, _)) = rig() else { return };
    let Some((mut head, _)) = rig() else { return };
    for _ in 0..(1.0 / DT) as usize {
        leg.advance(DT, &still());
        head.advance(DT, &still());
    }
    let wire = trp_sim::effects::screen_effects::hit_reaction(1.0, 0.0, trp_sim::condition::health::Part::LeftLeg, trp_sim::condition::health::DamageKind::Barbed, false).expect("a push");
    let shot = trp_sim::effects::screen_effects::hit_reaction(15.0, 0.0, trp_sim::condition::health::Part::Head, trp_sim::condition::health::DamageKind::Bullet, false).expect("a push");
    leg.add_force(wire.strength, wire.hands, wire.camera);
    head.add_force(shot.strength, shot.hands, shot.camera);
    let (mut most_leg, mut most_head, mut hands) = (0.0f32, 0.0f32, 0.0f32);
    for _ in 0..(1.0 / DT) as usize {
        leg.advance(DT, &still());
        head.advance(DT, &still());
        most_leg = most_leg.max(length(leg.camera_rotation.current));
        most_head = most_head.max(length(head.camera_rotation.current));
        hands = hands.max(length(head.hands_rotation.current));
    }
    println!("the camera turned at most {most_leg:.3} by the wire, {most_head:.3} by the shot; the hands {hands:.3}");
    assert!(most_head > most_leg && most_leg > 0.0 && hands > 0.0);
    for _ in 0..(4.0 / DT) as usize {
        head.advance(DT, &still());
    }
    assert!(length(head.camera_rotation.current) < 0.05, "settled: {:?}", head.camera_rotation.current);
}

/// A blast's shake at full strength (`GrenadeShake_CO`, weapon-handling
/// spec, "Force reaction") turns the camera well past what the breath
/// does standing still, and the camera settles again within seconds.
#[test]
fn a_blast_s_shake_turns_the_camera_and_it_settles() {
    let Some((mut calm, _)) = rig() else { return };
    let Some((mut shaken, _)) = rig() else { return };
    for _ in 0..(1.0 / DT) as usize {
        calm.advance(DT, &still());
        shaken.advance(DT, &still());
    }
    shaken.grenade_shake(1.0);
    let (mut most, mut most_calm, mut position) = (0.0f32, 0.0f32, 0.0f32);
    for _ in 0..(1.5 / DT) as usize {
        calm.advance(DT, &still());
        shaken.advance(DT, &still());
        most = most.max(length(shaken.camera_rotation.current));
        most_calm = most_calm.max(length(calm.camera_rotation.current));
        position = position.max(length(shaken.camera_position.current));
    }
    println!("the camera turned at most {most:.3} degrees shaken, {most_calm:.3} calm; its place moved {position:.6} m");
    assert!(most > 1.0 && most > 10.0 * most_calm.max(0.01));
    assert!(position > 0.0);
    for _ in 0..(4.0 / DT) as usize {
        shaken.advance(DT, &still());
    }
    assert!(length(shaken.camera_rotation.current) < 0.05, "settled: {:?}", shaken.camera_rotation.current);
}

/// The load (`Physical.Overweight`) and the gear's share of the ergonomics
/// (weapon handling spec, "The load", "The gear's share of the ergonomics"):
/// a turn swings a loaded weapon further, the aim comes in at the load's
/// share of its pace, and handing the same figures over again changes
/// nothing.
#[test]
fn the_load_swings_the_weapon_further_and_slows_the_aim() {
    let (Some((mut light, _)), Some((mut loaded, _))) = (rig(), rig()) else { return };
    let ergonomics = loaded.settings.weapon_ergonomics;
    loaded.set_load(0.5, -0.05);
    assert!((loaded.settings.total_ergonomics() - ergonomics * 0.95).abs() < 1e-4, "{}", loaded.settings.total_ergonomics());
    // The aim's pace: the load's share, 1 to 0.4 at the top (`Stamina.AimingSpeedMultiplier`).
    let pace = |hands: &Hands| hands.camera_aim_rate() / hands.aiming_speed;
    assert!((pace(&loaded) / pace(&light) - 0.7).abs() < 1e-4, "{} against {}", pace(&loaded), pace(&light));
    let speed = loaded.aiming_speed;
    loaded.set_load(0.5, -0.05);
    assert_eq!(loaded.aiming_speed, speed);
    let mut peaks = [0.0f32; 2];
    let mut yaw = 0.0;
    for frame in 0..120 {
        if frame < 30 {
            yaw += 120.0 * DT;
        }
        for (peak, hands) in peaks.iter_mut().zip([&mut light, &mut loaded]) {
            hands.advance(DT, &HandsInput { yaw, ..still() });
            *peak = peak.max(length(hands.sway.position));
        }
    }
    println!("a turn's sway: {:.4} unloaded, {:.4} at half the load ({:.3} times)", peaks[0], peaks[1], peaks[1] / peaks[0]);
    assert!(peaks[1] > peaks[0] * 1.2, "the load swung the weapon {} against {}", peaks[1], peaks[0]);
}

/// The kit `courses8` was recorded in (a rig, a headset helmet and a 35 l
/// backpack) takes 5% off the weapon's ergonomics, all of it the backpack's:
/// the game's trace has the AKM's 40 at 38.
#[test]
fn the_recorded_kit_takes_its_share_off_the_ergonomics() {
    let Some(db) = crate::fixture::database() else { return };
    let worn = |template: &str, slot: &str| trp_sim::condition::armor::Worn { template: template.into(), slot: slot.into(), children: Vec::new(), ..Default::default() };
    let kit = [worn("592c2d1a86f7746dbe2af32a", "TacticalVest"), worn("5ea05cf85ad9772e6624305d", "Headwear"), worn("639346cc1c8f182ad90c8972", "Backpack")];
    let share = trp_sim::condition::armor::ergonomics_share(db, &kit);
    assert!((share + 0.05).abs() < 1e-6, "{share}");
    assert_eq!(trp_sim::condition::armor::ergonomics_share(db, &kit[..2]), 0.0);
}

/// An arm damaged has the firearm's controller work the weapon out again on
/// the very call (weapon handling spec, "The gear's share of the
/// ergonomics"): the ergonomics 0, the ergonomic weight the weapon's whole
/// weight, the aiming speed, the aim sway's and the sway's strengths with
/// them, and the recoil's rotation spring put back to nought (the breath's
/// hip wander held in it goes); the hip penalty is left as it was. The same
/// arms again change nothing; the other arm too is a change, still at 0; a
/// painkiller gives everything back at once. The AKM at 3.991 kg and
/// ergonomics 0 with Aim Drills 40's 0.40 comes up at 0.79134 a second:
/// `courses8\pain-and-analgin`'s recorded aiming speed, 0.7913357.
#[test]
fn an_arm_damaged_works_the_weapon_out_again_at_once() {
    use trp_sim::condition::health::Conditions;
    let Some((mut hands, _)) = rig() else { return };
    hands.set_skills(0.40, 0.0, 0.0);
    let healthy = (hands.settings.total_ergonomics(), hands.aiming_speed, hands.aim_sway().0, hands.sway_strengths(), hands.hip_penalty());
    assert_eq!(healthy.0, hands.settings.weapon_ergonomics);
    for _ in 0..(5.0 / DT) as usize {
        hands.advance(DT, &still());
    }
    assert!(hands.recoil.hand_rotation.offset.iter().any(|v| v.abs() > 1e-3), "the hip wanders: {:?}", hands.recoil.hand_rotation.offset);
    let broken = Conditions { right_arm: true, ..Conditions::default() };
    assert!(hands.set_health(broken), "worked out again");
    assert_eq!(hands.settings.total_ergonomics(), 0.0);
    assert_eq!(hands.settings.ergonomic_weight(), hands.settings.weapon_weight);
    println!(
        "the AKM with the right arm broken: aiming speed {:.5} (healthy {:.5}), aim sway {:.4} ({:.4}), sway {:?} ({:?})",
        hands.aiming_speed,
        healthy.1,
        hands.aim_sway().0,
        healthy.2,
        hands.sway_strengths(),
        healthy.3
    );
    assert!((hands.settings.weapon_weight - 3.991).abs() < 1e-3, "{}", hands.settings.weapon_weight);
    assert!((hands.aiming_speed - 0.7913357).abs() < 1e-4, "{}", hands.aiming_speed);
    let ((sway, displacement), (sway_before, displacement_before)) = (hands.sway_strengths(), healthy.3);
    // (The settings' displacement curve falls with the weight.)
    assert!(hands.aim_sway().0 > healthy.2 && sway > sway_before && displacement != displacement_before);
    assert_eq!(hands.recoil.hand_rotation.offset, [0.0; 3]);
    assert_eq!(hands.recoil.hand_rotation.current, [0.0; 3]);
    assert_eq!(hands.hip_penalty(), healthy.4, "the hip penalty is not worked out again");
    assert!(!hands.set_health(broken), "the same arms again");
    assert!(hands.set_health(Conditions { left_arm: true, ..broken }), "the other arm is a change of its own");
    assert_eq!(hands.settings.total_ergonomics(), 0.0);
    // A painkiller clears both arms (`Health::conditions`).
    assert!(hands.set_health(Conditions { on_painkillers: true, ..Conditions::default() }));
    assert_eq!((hands.settings.total_ergonomics(), hands.aiming_speed, hands.aim_sway().0, hands.sway_strengths()), (healthy.0, healthy.1, healthy.2, healthy.3));
}

/// The arms' damage is the character's: hands that hold no firearm keep it
/// without working anything out (no firearm's controller listens), and the
/// firearm taken in hand after them starts at ergonomics 0, as a controller
/// made with an arm damaged does, nothing reset and nothing more to change.
/// Its animator's two skill speeds are 1 while an arm is damaged, told at
/// once in hand, and the skill's again as it heals.
#[test]
fn a_rig_taken_in_hand_with_an_arm_damaged_starts_at_nought() {
    use trp_sim::condition::health::Conditions;
    let (Some((mut other, _)), Some((mut drawn, _, mut firearm))) = (rig(), full_rig()) else { return };
    let speed = drawn.aiming_speed;
    other.settings.recoil_force = None;
    let broken = Conditions { left_arm: true, ..Conditions::default() };
    assert!(!other.set_health(broken), "no firearm's controller hears it");
    assert_eq!(other.aiming_speed, speed);
    drawn.carry_on_from(&other);
    assert_eq!(drawn.settings.total_ergonomics(), 0.0);
    assert!(drawn.aiming_speed < speed);
    assert!(!drawn.set_health(broken), "nothing more to change");
    firearm.set_skill_speeds(1.2, 1.4);
    assert_eq!((firearm.animator().get("SpeedReload"), firearm.animator().get("SpeedDraw")), (1.2, 1.4));
    firearm.set_arms_damaged(true, false);
    assert_eq!((firearm.animator().get("SpeedReload"), firearm.animator().get("SpeedDraw")), (1.0, 1.0));
    firearm.set_skill_speeds(1.3, 1.5);
    assert_eq!((firearm.animator().get("SpeedReload"), firearm.animator().get("SpeedDraw")), (1.0, 1.0), "a skill's change while hurt");
    firearm.set_arms_damaged(false, false);
    assert_eq!((firearm.animator().get("SpeedReload"), firearm.animator().get("SpeedDraw")), (1.3, 1.5));
}

/// A weapon's motion as built from the install, with no animator.
fn hands_of(template: &str) -> Option<Hands> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database().expect("database");
    let hard = HardSettings::load(&install).expect("settings object");
    let engine = EngineSettings::load(&install).expect("engine settings");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, template).expect("weapon");
    let data = HandsData::load(&mut assets).expect("player prefab");
    Some(Hands::new(trp_sim::hands_settings(data, &db, &hard, &engine, &weapon, ViewSettings::default()).expect("hands settings")))
}

/// Each firearm its own hip penalty (weapon handling spec, "The hip
/// penalty, each weapon's own"): a firearm's controller made in another
/// thing's place works its own out, by its own total ergonomics and its own
/// combo unit (`FirearmController.Setup`: `UpdateHipInaccuracy`), where the
/// motion carried on alone keeps the last one's; hands that hold no firearm
/// keep the last firearm's. The game's, on the new controller's first frame:
/// `courses8\pistol-quickdraw` 0.9524 (ergonomics 49.4) to 0.8156 (83.6),
/// `rhino-s160` 0.8325 (79.38) to 0.9462 (50.96).
#[test]
fn a_firearm_drawn_works_its_own_hip_penalty_out() {
    const RHINO: &str = "61a4c8884f95bc3b2c5dc96f";
    let (Some(rifle), Some(mut revolver), Some(mut empty)) = (hands_of(AKM), hands_of(RHINO), hands_of(AKM)) else { return };
    let (rifle_e, revolver_e) = (rifle.settings.total_ergonomics(), revolver.settings.total_ergonomics());
    println!("the AKM's ergonomics {rifle_e}, hip penalty {:.4}; the Rhino's {revolver_e}, {:.4}", rifle.hip_penalty(), revolver.hip_penalty());
    assert!((rifle_e - revolver_e).abs() > 10.0, "two weapons of different ergonomics");
    assert!((rifle.hip_penalty() - revolver.hip_penalty()).abs() > 0.01);
    // The motion carried on: the rifle's figure, until the controller is made.
    revolver.carry_on_from(&rifle);
    assert_eq!(revolver.hip_penalty(), rifle.hip_penalty());
    revolver.firearm_made(false);
    assert_eq!(revolver.hip_penalty(), trp_sim::weapons::tactical::hip_penalty(revolver_e, false));
    assert!(!revolver.combo_on());
    // With a combo unit on it, the lower figure.
    revolver.firearm_made(true);
    assert!((revolver.hip_penalty() - (0.3 - revolver_e / 400.0)).abs() < 1e-6 && revolver.combo_on());
    revolver.firearm_made(false);
    // Hands that hold no firearm next keep the revolver's.
    empty.settings.recoil_force = None;
    empty.carry_on_from(&revolver);
    empty.keep_from(revolver.kept());
    assert_eq!(empty.hip_penalty(), revolver.hip_penalty());
}
