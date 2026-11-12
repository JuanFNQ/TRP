//! Shots kicking the weapon and the camera, automatic fire and the reload's length.

use super::*;

/// The turn from `b` to `a`.
pub(super) fn quat_mul_inverse(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    trp_sim::base::math::quat_mul(a, [-b[0], -b[1], -b[2], b[3]])
}

#[test]
fn one_shot_from_the_hip_kicks_the_weapon_and_the_camera() {
    let Some((mut hands, _, firearm)) = full_rig() else { return };
    for _ in 0..30 {
        hands.advance(DT, &still());
    }
    hands.shoot(firearm.settings.shot_strength);
    let (mut climb, mut back, mut camera) = (0.0f32, 0.0f32, 0.0f32);
    for _ in 0..600 {
        hands.advance(DT, &still());
        climb = climb.min(hands.recoil.weapon_rotation()[0]);
        back = back.max(hands.recoil.weapon_position()[1]);
        camera = camera.max(angle(hands.frame(DT, camera_bone(), None).rotation));
    }
    let rest = hands.recoil.weapon_rotation();
    println!(
        "one shot, standing, from the hip: muzzle climbs {:.2} degrees, weapon comes back {:.1} mm, camera turns up to {camera:.2} degrees; ten seconds later the muzzle rests {:.2} degrees up and {:.2} aside",
        -climb,
        back * 1000.0,
        -rest[0],
        rest[2],
    );
    assert!(climb < -0.5 && climb > -15.0, "{climb}");
    assert!(back > 0.001 && back < 0.1, "{back}");
    assert!(camera > 0.05 && camera < 10.0, "{camera}");
    assert!(rest[0].abs() < 2.0 && rest[2].abs() < 2.0, "{rest:?}");
}

/// A shot is felt after the frame's late update has placed the weapon and
/// the camera (`Player.LateUpdate`: the procedural motion's
/// `LateTransformations`, then the firearm controller's `ManualLateUpdate`,
/// which calls `Shoot`): the weapon's own curves start again, and the
/// camera's rate drops, from the frame after. Course 6's M60E6 under held
/// fire shows it: on each shot's frame the game still draws the curves as
/// they ran (`docs/spec/weapon-handling.md`, "What a shot does").
#[test]
fn a_shot_is_felt_after_the_frame_s_pose() {
    let Some((mut hands, _, firearm)) = full_rig() else { return };
    let input = HandsInput { aiming: true, ..still() };
    let frame = |hands: &mut Hands, steps: usize| {
        for _ in 0..steps {
            hands.advance(DT, &input);
            hands.frame(DT, camera_bone(), None);
        }
    };
    frame(&mut hands, 60);
    hands.shoot(firearm.settings.shot_strength);
    hands.frame(DT, camera_bone(), None);
    // Half a second on the AKM's curves still run and the camera's rate is
    // back to steady.
    frame(&mut hands, 30);
    let (running, rate) = (hands.recoil.weapon_rotation(), hands.camera_aim_rate());
    hands.shoot(firearm.settings.shot_strength);
    assert_eq!(hands.recoil.weapon_rotation(), running, "the shot's own frame is placed as before it");
    assert_eq!(hands.camera_aim_rate(), rate);
    hands.frame(DT, camera_bone(), None);
    let (after, dropped) = (hands.recoil.weapon_rotation(), hands.camera_aim_rate());
    let moved = (0..3).map(|i| (after[i] - running[i]).abs()).fold(0.0f32, f32::max);
    println!("half a second after a shot the curves turn the weapon {running:?}; the next shot leaves {after:?} after its frame, the camera's rate {rate:.3} to {dropped:.3}");
    assert!(moved > 0.01, "{running:?} {after:?}");
    assert!(dropped < rate * 0.95, "{rate} {dropped}");
}

/// In a run of automatic fire the eye's target is slid along the sight's
/// own line to the aim plane's depth, not along the view
/// (`ProceduralWeaponAnimation`, `CheckAutoFire`): with the line pitched up
/// and the eye's plane behind the sight, the eye comes to rest lower by the
/// line's rise over the way back.
#[test]
fn in_automatic_fire_the_eye_slides_along_the_sight_s_line() {
    let pitch = 5.0f32.to_radians();
    let line = [0.0, pitch.sin(), pitch.cos()];
    let sight = trp_sim::AimPoint { bone: [0.0, 0.05, 0.12], origin: [0.0, -0.05, 0.0], barrel: [0.0, 0.0, 1.0], line };
    let rest = |auto: bool| -> Option<[f32; 3]> {
        let (mut hands, _) = rig()?;
        hands.set_auto_fire(auto);
        let input = HandsInput { aiming: true, ..still() };
        let mut camera = [0.0; 3];
        for _ in 0..900 {
            hands.advance(DT, &input);
            camera = hands.frame(DT, camera_bone(), Some(sight)).position;
        }
        Some(camera)
    };
    let (Some(view), Some(along)) = (rest(false), rest(true)) else { return };
    let reach = view[2] - sight.bone[2];
    println!("the eye at rest beside a sight pitched 5 degrees up: {view:?} single, {along:?} in automatic fire, {:+.1} mm up", (along[1] - view[1]) * 1000.0);
    assert!((view[1] - sight.bone[1]).abs() < 1e-4 && view[0].abs() < 1e-4, "{view:?}");
    assert!((along[1] - view[1] - reach * pitch.sin()).abs() < 1e-4, "{view:?} {along:?}");
    assert!((along[2] - sight.bone[2] - reach * pitch.cos()).abs() < 1e-4, "{view:?} {along:?}");
}

#[test]
fn aiming_softens_the_kick() {
    let climb = |aiming: bool| -> Option<f32> {
        let (mut hands, _, firearm) = full_rig()?;
        let input = HandsInput { aiming, ..still() };
        for _ in 0..30 {
            hands.advance(DT, &input);
        }
        hands.shoot(firearm.settings.shot_strength);
        let mut low = 0.0f32;
        for _ in 0..120 {
            hands.advance(DT, &input);
            low = low.min(hands.recoil.hand_rotation.current[0]);
        }
        Some(-low)
    };
    let (Some(hip), Some(aimed)) = (climb(false), climb(true)) else { return };
    println!("muzzle climb of one shot: {hip:.2} degrees from the hip, {aimed:.2} aiming");
    // The same seed gives the same shot, so only the aiming factor differs.
    assert!((aimed / hip - 0.75).abs() < 0.03, "{aimed} / {hip}");
}

#[test]
fn a_magazine_on_automatic_empties_at_the_weapon_s_rate() {
    let Some((mut hands, _, mut firearm)) = full_rig() else { return };
    // The switch shows on the animator from its next update.
    firearm.next_fire_mode();
    firearm.step(DT);
    while !firearm.is_idle() {
        firearm.step(DT);
    }
    firearm.set_trigger(true, true);
    let (mut shots, mut last_shot_at, mut time) = (0, 0.0f32, 0.0f32);
    let mut climb_after: Vec<f32> = Vec::new();
    let mut deepest = 0.0f32;
    for _ in 0..(6.0 / DT) as usize {
        for event in firearm.step(DT) {
            match event {
                FirearmEvent::Shot { strength, .. } => {
                    if shots > 0 {
                        climb_after.push(-hands.recoil.weapon_rotation()[0]);
                    }
                    hands.shoot(strength);
                    shots += 1;
                    last_shot_at = time;
                }
                FirearmEvent::AutoFire(on) => hands.set_auto_fire(on),
                _ => {}
            }
        }
        hands.advance(DT, &still());
        deepest = deepest.min(hands.recoil.weapon_rotation()[0]);
        time += DT;
    }
    let rate = (shots - 1) as f32 / last_shot_at * 60.0;
    println!(
        "automatic: {shots} shots, the last after {last_shot_at:.2} s ({rate:.0} a minute); muzzle {:.1} degrees up when the 2nd shot fires, {:.1} at the 5th, {:.1} at the 10th, {:.1} at the 30th; highest {:.1}",
        climb_after[0],
        climb_after[3],
        climb_after[8],
        climb_after[28],
        -deepest,
    );
    assert_eq!(shots, 31, "thirty in the magazine and one in the chamber");
    assert!((rate - firearm.settings.fire_rate).abs() < 15.0, "{rate}");
    assert!(-deepest < 45.0, "the muzzle keeps climbing: {deepest}");
    assert!(!firearm.chamber && firearm.magazine == Some(0));
}

#[test]
fn reloading_takes_as_long_as_its_animator_s_states() {
    let Some((_, _, mut firearm)) = full_rig() else { return };
    firearm.reload();
    let mut time = 0.0;
    let (mut out_at, mut in_at) = (None, None);
    // Each state the animator enters, with when: a state is left where
    // its way to the next one opens, which is before its end, and the two
    // then blend.
    let mut entered: Vec<(String, f32)> = Vec::new();
    let mut steps = 0;
    while (steps < 2 || !firearm.is_idle()) && time < 20.0 {
        for event in firearm.step(DT) {
            match event {
                FirearmEvent::MagazineOut => out_at = Some(time),
                FirearmEvent::MagazineIn => in_at = Some(time),
                _ => {}
            }
        }
        let (state, _, next) = firearm.animator().state();
        let now = next.unwrap_or(state).to_owned();
        if entered.last().is_none_or(|(last, _)| *last != now) {
            entered.push((now, time));
        }
        time += DT;
        steps += 1;
    }
    println!(
        "reload with a round chambered: {time:.2} s, magazine out at {:.2} s and the new one in at {:.2} s; states {:?}",
        out_at.unwrap_or(-1.0),
        in_at.unwrap_or(-1.0),
        entered
    );
    let names: Vec<&str> = entered.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["Hands.RELOAD START", "Hands.RELOAD OUT", "Hands.RELOAD IN", "Hands.RELOAD END", "Hands.IDLE"]);
    // Recorded: 3.30 s from the key to the weapon at rest.
    assert!((time - 3.3).abs() < 0.1, "{time}");
    assert!(out_at.is_some() && in_at > out_at);
    assert_eq!(firearm.magazine, Some(firearm.settings.magazine_capacity));
}
