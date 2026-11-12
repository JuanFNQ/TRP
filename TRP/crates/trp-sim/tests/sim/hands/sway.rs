//! Breathing at rest, mounts, turning sway, the walk bob and the time to aim.

use super::*;

#[test]
fn engine_runs_at_sixty_steps_with_standard_gravity() {
    let Some((hands, movement)) = rig() else { return };
    assert!((hands.settings.fixed_timestep - DT).abs() < 1e-5);
    assert!((movement.settings.gravity + 9.81).abs() < 1e-3);
}

#[test]
fn standing_still_the_weapon_only_breathes() {
    let Some((mut hands, _)) = rig() else { return };
    let (mut dip, mut rise, mut wander) = (0.0f32, 0.0f32, 0.0f32);
    let (mut peaks, mut rising, mut last) = (0, false, 0.0f32);
    for _ in 0..(30.0 / DT) as usize {
        hands.advance(DT, &still());
        let pitch = hands.hands_rotation.current[0];
        dip = dip.max(pitch);
        rise = rise.max(hands.hands_position.current[2]);
        wander = wander.max(hands.recoil.hand_rotation.offset[0].abs());
        if rising && pitch < last && pitch > 0.3 {
            peaks += 1;
        }
        rising = pitch > last;
        last = pitch;
    }
    println!(
        "standing still for 30 s: each breath dips the muzzle {dip:.2} degrees and lifts the weapon {:.1} mm, {peaks} breaths; the muzzle wanders up to {wander:.2} degrees from the hip",
        rise * 1000.0
    );
    // The recorded game shows 0.5 degrees and 10 mm, one breath every 3.4 s.
    assert!((0.4..0.6).contains(&dip), "{dip}");
    assert!((0.008..0.012).contains(&rise), "{rise}");
    assert!((7..=10).contains(&peaks), "{peaks} breaths in 30 s");
    assert!(wander > 0.05 && wander < 3.5, "{wander}");
}

/// A mount puts the recoil's rotation spring back to nought
/// (`SetMountingData`, `UpdateWeaponVariables`, `HandRotationRecoil.Reset`):
/// the breath's hip wander held in its offset goes at once, and with the
/// breath stopped on the mount it stays gone (recorded on the M700's bipod:
/// mounting spec, "The weapon's motion while mounted"). Leaving resets it
/// again.
#[test]
fn a_mount_drops_the_hip_wander() {
    let Some((mut hands, _)) = rig() else { return };
    for _ in 0..(5.0 / DT) as usize {
        hands.advance(DT, &still());
    }
    assert!(hands.recoil.hand_rotation.offset.iter().any(|v| v.abs() > 1e-3), "the hip wanders: {:?}", hands.recoil.hand_rotation.offset);
    let mount = trp_sim::weapons::mounting::HandsMount { pivot: [0.0, -0.74, -0.22], side: false, bipod: true };
    hands.set_mounting(Some(mount));
    assert_eq!(hands.recoil.hand_rotation.offset, [0.0; 3]);
    assert_eq!(hands.recoil.hand_rotation.current, [0.0; 3]);
    for _ in 0..(2.0 / DT) as usize {
        hands.advance(DT, &still());
    }
    assert_eq!(hands.recoil.hand_rotation.offset, [0.0; 3], "no breath on the mount with breath to spare");
    hands.set_mounting(None);
    for _ in 0..(2.0 / DT) as usize {
        hands.advance(DT, &still());
    }
    assert!(hands.recoil.hand_rotation.offset.iter().any(|v| v.abs() > 1e-4), "off the mount it wanders again");
}

#[test]
fn turning_swings_the_weapon_and_it_settles() {
    let Some((mut hands, _)) = rig() else { return };
    let (mut peak, mut peak_offset) = (0.0f32, 0.0f32);
    let mut yaw = 0.0;
    // Turn right at 120 degrees per second for half a second, then hold.
    for frame in 0..300 {
        if frame < 30 {
            yaw += 120.0 * DT;
        }
        hands.advance(DT, &HandsInput { yaw, ..still() });
        let (position, rotation) = hands.weapon_transform();
        peak = peak.max(angle(rotation));
        peak_offset = peak_offset.max(length(position));
    }
    println!("turning at 120 deg/s: weapon swings up to {peak:.2} degrees and shifts up to {:.1} mm", peak_offset * 1000.0);
    assert!(peak > 0.2, "no visible sway: {peak}");
    assert!(peak < 30.0, "sway out of range: {peak}");
    // Breathing never stops; the swing from the turn does.
    assert!(length(hands.sway.position) < 0.01, "the sway did not settle: {:?}", hands.sway.position);
}

/// The game works a frame's physics steps before it turns the weapon's
/// rig to the view, so the sway takes a turn of the view a frame after
/// the view shows it (weapon-handling spec, "Motion reaction").
#[test]
fn the_sway_sees_the_view_a_frame_late() {
    let (Some((mut late, _)), Some((mut prompt, _))) = (rig(), rig()) else { return };
    // The same breath on both, so only the turn sets them apart.
    let quiet = |hands: &Hands| [hands.sway.position[1], hands.sway.position[2]];
    let mut yaw = 0.0;
    let mut seen = Vec::new();
    for frame in 0..40 {
        if (20..26).contains(&frame) {
            yaw += 3.5;
        }
        let input = HandsInput { yaw, ..still() };
        // One calls on the rig's turn after the steps, as a host should;
        // the other never does, and its steps see the view they are given.
        late.advance(DT, &input);
        late.turn_rig(yaw, 0.0);
        prompt.advance(DT, &input);
        seen.push((quiet(&late), quiet(&prompt)));
    }
    let moved = |sway: [f32; 2]| sway[0].abs() > 1e-4;
    let first = |pick: fn(&([f32; 2], [f32; 2])) -> [f32; 2]| seen.iter().position(|s| moved(pick(s))).unwrap();
    let (late_at, prompt_at) = (first(|s| s.0), first(|s| s.1));
    println!(
        "a turn begun on frame 20: the sway first moves on frame {prompt_at} given the view at once, on frame {late_at} with the rig turned after the steps"
    );
    assert_eq!((prompt_at, late_at), (20, 21));
    // A frame late and otherwise the same.
    for frame in 21..40 {
        let (late, prompt) = (seen[frame].0, seen[frame - 1].1);
        assert!((late[0] - prompt[0]).abs() < 1e-4 && (late[1] - prompt[1]).abs() < 1e-4, "frame {frame}: {late:?} against {prompt:?}");
    }
}

#[test]
fn walking_bobs_the_weapon_in_time_with_the_steps() {
    let Some((mut hands, mut movement)) = rig() else { return };
    let input = Input { move_dir: [0.0, 1.0], ..Input::default() };
    let (mut peak_angle, mut peak_offset, mut peak_camera) = (0.0f32, 0.0f32, 0.0f32);
    let mut sign_changes = 0;
    let mut last_sign = 0.0f32;
    let mut gaps = Vec::new();
    for frame in 0..(6.0 / DT) as usize {
        movement.step(DT, &input, &Flat);
        let footstep = movement.take_footstep();
        if frame >= 120 {
            gaps.extend(footstep);
        }
        hands.advance(
            DT,
            &HandsInput {
                yaw: movement.yaw,
                pitch: movement.pitch,
                motion: movement.input_velocity(),
                velocity: movement.velocity,
                speed: movement.smoothed_speed,
                walking: movement.state == State::Move,
                footstep,
                pose_level: movement.pose_level,
                smoothed_pose_level: movement.smoothed_pose_level,
                ..HandsInput::default()
            },
        );
        if frame < 120 {
            continue;
        }
        let (position, rotation) = hands.weapon_transform();
        peak_angle = peak_angle.max(angle(rotation));
        peak_offset = peak_offset.max(length(position));
        peak_camera = peak_camera.max(angle(hands.frame(DT, camera_bone(), None).rotation));
        let roll = hands.hands_rotation.current[1];
        if roll.abs() > 1e-4 {
            if last_sign != 0.0 && roll.signum() != last_sign {
                sign_changes += 1;
            }
            last_sign = roll.signum();
        }
    }
    // Footsteps come from the body's clips: one each time its footstep
    // curve changes sign.
    assert!(gaps.len() >= 8, "only {} footsteps in 4 s", gaps.len());
    let interval = gaps.iter().sum::<f32>() / gaps.len() as f32;
    println!(
        "walking at {:.2} m/s: a step every {interval:.2} s, bob tempo {:.2} cycles/s, weapon swings up to {peak_angle:.2} degrees and shifts up to {:.1} mm, camera up to {peak_camera:.2} degrees, roll reversed {sign_changes} times in 4 s",
        movement.ground_speed(),
        hands.step_frequency,
        peak_offset * 1000.0,
    );
    // The recorded game steps every 0.375 s at a Strength of 40; an
    // untrained character walks a little slower.
    assert!((0.3..0.5).contains(&interval), "{interval}");
    assert!(peak_angle > 0.05 && peak_angle < 20.0, "{peak_angle}");
    assert!(sign_changes >= 3, "the bob does not oscillate");
}

#[test]
fn time_to_aim_follows_weight_and_ergonomics() {
    let Some((hands, _)) = rig() else { return };
    let time = 1.0 / hands.aiming_speed;
    println!(
        "AKM, {:.2} kg, ergonomics {}: time to aim {time:.2} s",
        hands.settings.weapon_weight, hands.settings.weapon_ergonomics
    );
    assert!((0.3..2.5).contains(&time));
}

/// The features' two switches (`trp_sim::Systems`): with the weapon's
/// motion off a weapon turned with the view, walked with and leant over
/// stands where a still one does, and with the recoil off a shot moves
/// neither the weapon nor the camera.
#[test]
fn the_weapon_s_motion_and_the_recoil_switched_off() {
    let Some((mut moved, _, firearm)) = full_rig() else { return };
    let Some((mut still_one, _)) = rig() else { return };
    let off = trp_sim::Systems { weapon_motion: false, recoil: false, ..trp_sim::Systems::default() };
    moved.set_systems(&off);
    still_one.set_systems(&off);
    let mut yaw = 0.0;
    for frame in 0..240 {
        yaw += 120.0 * DT;
        let footstep = (frame % 30 == 0).then_some(1.0);
        let walking = HandsInput { yaw, speed: 1.0, walking: true, idle: false, footstep, velocity: [0.0, 0.0, 1.0], tilt: 1.0, tilt_target: 1.0, ..still() };
        moved.advance(DT, &walking);
        still_one.advance(DT, &still());
        if frame == 120 {
            moved.shoot(firearm.settings.shot_strength);
        }
        let (a, b) = (moved.weapon_transform(), still_one.weapon_transform());
        let apart = length(std::array::from_fn(|i| a.0[i] - b.0[i]));
        assert!(apart < 1e-6 && angle(quat_mul_inverse(a.1, b.1)) < 1e-3, "frame {frame}: {a:?} against {b:?}");
        let (camera, rest) = (moved.frame(DT, camera_bone(), None), still_one.frame(DT, camera_bone(), None));
        assert!(angle(quat_mul_inverse(camera.rotation, rest.rotation)) < 1e-3, "frame {frame}");
    }
    assert_eq!(moved.recoil.weapon_rotation(), [0.0; 3]);
    // And with both on, the same moves it.
    let (Some((mut moved, _)), Some((mut still_one, _))) = (rig(), rig()) else { return };
    let (mut yaw, mut most) = (0.0, 0.0f32);
    for frame in 0..240 {
        yaw += 120.0 * DT;
        let footstep = (frame % 30 == 0).then_some(1.0);
        moved.advance(DT, &HandsInput { yaw, speed: 1.0, walking: true, idle: false, footstep, velocity: [0.0, 0.0, 1.0], tilt: 1.0, tilt_target: 1.0, ..still() });
        still_one.advance(DT, &still());
        let (a, b) = (moved.weapon_transform(), still_one.weapon_transform());
        most = most.max(angle(quat_mul_inverse(a.1, b.1)));
    }
    println!("with the motion on the same input turns the weapon up to {most:.2} degrees from the still one");
    assert!(most > 0.5, "{most}");
}
