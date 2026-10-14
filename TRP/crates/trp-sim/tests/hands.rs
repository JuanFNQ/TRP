//! Weapon motion measured with the real install's data and the stock AKM.
//! These pass trivially, with a note, where no install is configured.
//!
//! Run with `cargo test -p trp-sim --test hands -- --nocapture` for the numbers.

use trp_data::engine::EngineSettings;
use trp_data::hard_settings::HardSettings;
use trp_data::player::HandsData;
use trp_data::unity::Assets;
use trp_data::{Database, Install};
use trp_sim::math::quat_euler;
use trp_sim::{Firearm, FirearmEvent, Flat, Hands, HandsInput, Input, Movement, State, ViewSettings};

const AKM: &str = "59d6088586f774275f37482f";
const DT: f32 = 1.0 / 60.0;

fn rig() -> Option<(Hands, Movement)> {
    full_rig().map(|(hands, movement, _)| (hands, movement))
}

/// Standing upright and still.
fn still() -> HandsInput {
    HandsInput { idle: true, pose_level: 1.0, smoothed_pose_level: 1.0, ..HandsInput::default() }
}

/// The weapon's animated camera bone at rest.
fn camera_bone() -> [f32; 4] {
    quat_euler([-90.0, 0.0, 0.0])
}

fn full_rig() -> Option<(Hands, Movement, Firearm)> {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return None;
    };
    let db = Database::load(&install).expect("database");
    let hard = HardSettings::load(&install).expect("settings object");
    let engine = EngineSettings::load(&install).expect("engine settings");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, &db, AKM).expect("weapon");
    let data = HandsData::load(&mut assets).expect("player prefab");
    let hands = Hands::new(
        trp_sim::hands_settings(data, &db, &hard, &engine, &weapon, ViewSettings::default()).expect("hands settings"),
    );
    let animator = trp_sim::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("the weapon's animator");
    let mut firearm = Firearm::new(trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("firearm settings"), &animator);
    firearm.start_drawn();
    let body = trp_sim::body::BodyData::load(&mut assets).expect("body animator");
    let movement = Movement::new(
        trp_sim::load_movement_settings(&mut assets, &db, &hard, &engine, 0.0).expect("movement"),
        trp_sim::Body::new(&body, weapon.cell_width as f32),
    );
    Some((hands, movement, firearm))
}

/// Angle of a rotation `xyzw`, in degrees.
fn angle(q: [f32; 4]) -> f32 {
    2.0 * q[3].abs().min(1.0).acos().to_degrees()
}

fn length(v: [f32; 3]) -> f32 {
    v.iter().map(|c| c * c).sum::<f32>().sqrt()
}

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
                FirearmEvent::Shot { strength } => {
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
/// The arms as the game's solver leaves them, standing with the weapon at
/// rest: both palms on their holds, the left on the handguard's grip and
/// the right on the weapon's marker, with each bone at its own length.
/// And the choice of clip the hold depends on: the resting clip is the
/// one the animator's blend tree gives with its parameter left alone.
#[test]
fn the_arms_hold_the_weapon_as_the_game_solves_them() {
    use trp_data::player::LimbData;
    use trp_sim::arms;
    use trp_sim::rig::{ArmNodes, WeaponRig};

    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    let hard = HardSettings::load(&install).expect("settings object");
    let engine = EngineSettings::load(&install).expect("engine settings");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, &db, AKM).expect("weapon");
    let limbs = LimbData::load(&mut assets).expect("limb solvers");
    for limb in &limbs {
        arms::check(limb).expect("a solver this follows");
        println!(
            "limb solver: bend rule {}, weight {}, twist bones at {} and {} of the way to the palm",
            limb.bend_modifier, limb.bend_weight, limb.twists[0].crossfade, limb.twists[1].crossfade
        );
        assert!((limb.twists[0].crossfade - 0.2).abs() < 1e-4 && (limb.twists[1].crossfade - 0.4).abs() < 1e-4);
    }
    let (rest, _) = trp_sim::rest_clip(&assets, &weapon).expect("resting clip");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("firearm settings");
    println!("the weapon rests in {rest}");
    assert_eq!(rest, "akm_launcher_0_idle_off");
    assert_eq!(settings.modes[..2], [trp_sim::FireMode::Single, trp_sim::FireMode::FullAuto]);

    let body = trp_sim::body::BodyData::load(&mut assets).expect("body animator");
    let mut movement = Movement::new(
        trp_sim::load_movement_settings(&mut assets, &db, &hard, &engine, 0.0).expect("movement"),
        trp_sim::Body::new(&body, weapon.cell_width as f32),
    );
    for _ in 0..60 {
        movement.step(DT, &Input::default(), &Flat);
    }
    // The rig as the weapon's animator has it at rest.
    let animator = trp_sim::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("the weapon's animator");
    let mut firearm = Firearm::new(settings, &animator);
    firearm.start_drawn();
    firearm.step(DT);
    let mut rig = WeaponRig::new(&weapon.hierarchy);
    rig.pose(firearm.animator().pose(), rig.moved_bone());
    for (index, right) in [true, false].into_iter().enumerate() {
        let nodes = ArmNodes::find(&weapon.hierarchy, right).expect("the arm's bones on the rig");
        let on_rig = rig.arm(&nodes);
        let pose = movement.arm_pose(right);
        let (bones, grip) = arms::hold(&pose, &on_rig, &limbs[index]);
        let (target, _) = arms::hand_target(&on_rig);
        let apart = |a: [f32; 3], b: [f32; 3]| length([a[0] - b[0], a[1] - b[1], a[2] - b[2]]);
        let (upper, fore) = (apart(bones[0].0, bones[1].0), apart(bones[1].0, bones[4].0));
        println!(
            "{} arm: palm {:.2} mm from its hold, on the part's grip by {grip:.2}; upper arm {upper:.3} m, forearm {fore:.3} m; shoulder {:.2} mm from the rig's",
            if right { "right" } else { "left" },
            apart(bones[4].0, target.0) * 1000.0,
            apart(bones[0].0, on_rig.upper.0) * 1000.0
        );
        assert!(apart(bones[4].0, target.0) < 1e-3, "the palm is not on its hold");
        assert!((upper - 0.2965).abs() < 1e-3 && (fore - 0.251).abs() < 2e-3, "{upper} {fore}");
        assert!(apart(bones[0].0, on_rig.upper.0) < 1e-4, "standing, the shoulder is the rig's");
        // The stock rifle's handguard has a grip for the left hand; its
        // pistol grip has none for the right.
        assert_eq!(grip > 0.99, !right, "{grip}");
    }
}

/// Level ground with a wall across the way, `0` metres ahead of where the
/// character starts.
struct Wall(f32);

impl trp_sim::World for Wall {
    fn ground_height(&self, _x: f32, _z: f32) -> f32 {
        0.0
    }

    fn line(&self, from: [f32; 3], to: [f32; 3]) -> Option<f32> {
        let length = (0..3).map(|k| (to[k] - from[k]).powi(2)).sum::<f32>().sqrt();
        let through = |a: f32, b: f32, at: f32| (a < at && b >= at).then(|| (at - a) / (b - a));
        // The wall from this side, the ground from above.
        let hit = [through(from[2], to[2], self.0), through(-from[1], -to[1], 0.0)].into_iter().flatten().reduce(f32::min)?;
        Some(hit * length)
    }
}

/// One frame of a character standing still at a wall: the springs, then
/// the weapon's line against the wall, as a host does it.
fn frame_at(wall: &Wall, hands: &mut Hands, movement: &mut Movement, firearm: &mut Firearm, aim_key: Option<bool>) -> f32 {
    if let Some(want) = aim_key {
        firearm.set_aim(want, movement.can_aim());
    }
    movement.step(DT, &Input { aiming: firearm.aiming(), ..Input::default() }, wall);
    hands.advance(
        DT,
        &HandsInput { yaw: movement.yaw, pitch: movement.pitch, aiming: firearm.aiming(), ..still() },
    );
    firearm.step(DT);
    let depth = movement.weapon_overlap(wall, hands.weapon_line());
    hands.set_overlap(depth);
    firearm.overlap(depth, hands.turn_away_value(), movement.can_aim());
    depth
}

/// Up against a wall the weapon is drawn back and turned aside, and the
/// sights come down; with room again they come back by themselves, unless
/// the aim key was let go in between.
#[test]
fn a_wall_turns_the_weapon_away_and_puts_the_sights_down() {
    let Some((mut hands, mut movement, mut firearm)) = full_rig() else { return };
    let length = hands.settings.weapon_length;
    println!("the weapon's line is {length:.3} m long; the sights come down {:.2} m into something", firearm.settings.stop_aiming_at);
    assert!((length - 0.894).abs() < 0.005, "{length}");

    // In the open: nothing.
    let open = Wall(50.0);
    frame_at(&open, &mut hands, &mut movement, &mut firearm, Some(true));
    for _ in 0..120 {
        assert_eq!(frame_at(&open, &mut hands, &mut movement, &mut firearm, None), 0.0);
    }
    assert!(firearm.aiming() && hands.turn_away_value() == 0.0);
    let (free, _) = hands.weapon_transform();

    // A wall the weapon reaches a little way into: turned away, still aiming.
    let (from, to) = hands.weapon_line();
    let reach = movement.rig_frame().0[2] + to[2];
    println!("standing, the line runs from {:.3} to {:.3} m ahead of the ribcage, {reach:.3} m ahead of the feet", from[2], to[2]);
    let near = Wall(reach - 0.06);
    let mut depth = 0.0;
    for _ in 0..180 {
        depth = frame_at(&near, &mut hands, &mut movement, &mut firearm, None);
    }
    let (drawn, _) = hands.weapon_transform();
    println!(
        "6 cm into a wall: depth {depth:.3}, value {:.3}, the weapon drawn back {:.1} mm, sights {}",
        hands.turn_away_value(),
        (drawn[1] - free[1]) * 1000.0,
        if firearm.aiming() { "up" } else { "down" }
    );
    assert!(depth > 0.03 && depth < firearm.settings.stop_aiming_at && firearm.aiming(), "{depth}");
    assert!(drawn[1] - free[1] > 0.01, "{}", drawn[1] - free[1]);

    // Deeper than the limit: the sights go down, and the weapon comes
    // further back and is canted.
    let close = Wall(reach - 0.35);
    let mut lowered_after = None;
    for frame in 0..240 {
        depth = frame_at(&close, &mut hands, &mut movement, &mut firearm, None);
        if lowered_after.is_none() && !firearm.aiming() {
            lowered_after = Some(frame);
        }
    }
    let (position, rotation) = hands.turn_away_offsets();
    println!(
        "35 cm into a wall: depth {depth:.3}, value {:.3}; the sights went down on frame {lowered_after:?}; the turn-away adds {:.0}, {:.0}, {:.0} mm and {:.1}, {:.1}, {:.1} degrees; the elbows' goals move {:.0} and {:.0} mm",
        hands.turn_away_value(),
        position[0] * 1000.0,
        position[1] * 1000.0,
        position[2] * 1000.0,
        rotation[0],
        rotation[1],
        rotation[2],
        length3(hands.elbow_shift(false)) * 1000.0,
        length3(hands.elbow_shift(true)) * 1000.0,
    );
    assert_eq!(lowered_after, Some(0));
    assert!(!firearm.aiming());
    assert!(position[1] > 0.2 && rotation[1] < -10.0, "{position:?} {rotation:?}");
    // The aim key does nothing against the wall.
    frame_at(&close, &mut hands, &mut movement, &mut firearm, Some(true));
    assert!(!firearm.aiming());

    // Room again: nothing comes back, because the key was used.
    for _ in 0..240 {
        frame_at(&open, &mut hands, &mut movement, &mut firearm, None);
    }
    assert!(!firearm.aiming());

    // Once more with the key left alone: down at the wall, and up again
    // by themselves once the turn-away has run down.
    frame_at(&open, &mut hands, &mut movement, &mut firearm, Some(true));
    for _ in 0..60 {
        frame_at(&close, &mut hands, &mut movement, &mut firearm, None);
    }
    assert!(!firearm.aiming());
    let mut back_after = None;
    for frame in 0..240 {
        frame_at(&open, &mut hands, &mut movement, &mut firearm, None);
        if back_after.is_none() && firearm.aiming() {
            back_after = Some(frame);
            println!("with room again the sights came back after {:.2} s, the turn-away's value at {:.3}", frame as f32 * DT, hands.turn_away_value());
        }
    }
    let back_after = back_after.expect("the sights did not come back");
    assert!(back_after > 6 && back_after < 90, "{back_after}");
    assert!(hands.turn_away_value() < 0.01);

    // Flat against it: past a value of 1 the far pose takes over, the
    // muzzle swung aside.
    let flat = Wall(reach - 0.7);
    for _ in 0..240 {
        frame_at(&flat, &mut hands, &mut movement, &mut firearm, None);
    }
    let (position, rotation) = hands.turn_away_offsets();
    let (at, turn) = hands.weapon_transform();
    let muzzle = trp_sim::math::quat_rotate(turn, [0.0, -length, 0.0]);
    println!(
        "70 cm into a wall: value {:.3}; the turn-away adds {:.0}, {:.0}, {:.0} mm and {:.1}, {:.1}, {:.1} degrees; in the bone's parent the bone is at {:.3}, {:.3}, {:.3} and its muzzle {:.3}, {:.3}, {:.3} from it",
        hands.turn_away_value(),
        position[0] * 1000.0,
        position[1] * 1000.0,
        position[2] * 1000.0,
        rotation[0],
        rotation[1],
        rotation[2],
        at[0],
        at[1],
        at[2],
        muzzle[0],
        muzzle[1],
        muzzle[2],
    );
    assert!(hands.turn_away_value() > 1.0 && rotation[2] < -40.0, "{rotation:?}");
}

fn length3(v: [f32; 3]) -> f32 {
    v.iter().map(|c| c * c).sum::<f32>().sqrt()
}

/// The camera is kept out of the world by a box of its own. Standing it
/// touches nothing. Lying, with the eye drawn to a sight that lies in the
/// ground, the camera comes to rest where the box's lowest face is on the
/// ground, a millimetre clear, and stays there.
#[test]
fn the_cameras_box_rests_on_the_ground() {
    let Some((mut hands, mut movement, _)) = full_rig() else { return };
    let Ok(install) = Install::locate() else { return };
    let shape = trp_data::player::CameraBoxData::load(&mut Assets::new(install).expect("manifest")).expect("camera box");
    println!("the camera's box: {:?} m, centred {:?} from the camera", shape.size, shape.centre);
    assert!((shape.size[0] - 0.13).abs() < 1e-4 && (shape.size[1] - 0.23).abs() < 1e-4 && (shape.size[2] - 0.23).abs() < 1e-4);
    assert!(shape.centre[0].abs() < 1e-4 && (shape.centre[1] + 0.0674).abs() < 1e-4 && (shape.centre[2] + 0.05).abs() < 1e-4);
    let push = |movement: &Movement, camera: &trp_sim::CameraPose| {
        movement.camera_push(&Flat, camera.position, camera.rotation, shape.size, shape.centre)
    };

    let standing = hands.frame(DT, camera_bone(), None);
    assert_eq!(push(&movement, &standing), [0.0; 3], "standing, the box is clear of everything");

    movement.step(DT, &Input { prone: true, ..Input::default() }, &Flat);
    for _ in 0..150 {
        movement.step(DT, &Input::default(), &Flat);
    }
    assert_eq!(movement.state, State::ProneIdle);
    let lying = hands.frame(DT, camera_bone(), None);
    assert_eq!(push(&movement, &lying), [0.0; 3], "lying, from the hip, too");

    // A sight a quarter of a metre under the ribcage: under the ground.
    let sight = trp_sim::AimPoint { bone: [0.0, -0.25, 0.1], origin: [0.0, -0.25, 0.0], barrel: [0.0, 0.0, 1.0] };
    let eye_over_ground = |movement: &Movement, at: [f32; 3]| {
        let (origin, turn) = movement.rig_frame();
        origin[1] + trp_sim::math::quat_rotate(turn, at)[1]
    };
    let (mut pushed, mut resting, mut wanted) = (0, 0.0, 0.0);
    for _ in 0..300 {
        hands.advance(
            DT,
            &HandsInput { yaw: movement.yaw, pitch: movement.pitch, prone: true, aiming: true, idle: false, ..still() },
        );
        let mut camera = hands.frame(DT, camera_bone(), Some(sight));
        wanted = eye_over_ground(&movement, sight.bone);
        let by = push(&movement, &camera);
        pushed += usize::from(by != [0.0; 3]);
        camera.position = hands.push_camera(by);
        resting = eye_over_ground(&movement, camera.position);
        // Once pushed, the box is clear.
        assert!(length(push(&movement, &camera)) < 1e-5, "{:?}", push(&movement, &camera));
    }
    println!(
        "lying, the eye drawn to a sight {wanted:.3} m over the ground: the camera rests {resting:.4} m over it, pushed on {pushed} of 300 frames"
    );
    assert!(wanted < 0.05, "{wanted}");
    assert!(pushed > 200, "{pushed}");
    // The box reaches 0.182 m under a level camera.
    assert!((resting - 0.183).abs() < 0.004, "{resting}");
}

/// Lying down and looking at the ground pushes the weapon back by the
/// pose for lying, with nothing but the ground's height asked of the
/// host; crawling, the depth is measured all the same and the turn-away
/// is switched off.
#[test]
fn lying_the_ground_turns_the_weapon_away_but_not_in_a_crawl() {
    let Some((mut hands, mut movement, mut firearm)) = full_rig() else { return };
    let frame = |movement: &mut Movement, hands: &mut Hands, firearm: &mut Firearm, input: Input| -> f32 {
        movement.step(DT, &input, &Flat);
        hands.advance(
            DT,
            &HandsInput {
                yaw: movement.yaw,
                pitch: movement.pitch,
                prone: movement.prone,
                crawling: movement.state == State::ProneMove,
                idle: false,
                ..still()
            },
        );
        firearm.step(DT);
        let depth = movement.weapon_overlap(&Flat, hands.weapon_line());
        hands.set_overlap(depth);
        firearm.overlap(depth, hands.turn_away_value(), movement.can_aim());
        depth
    };
    frame(&mut movement, &mut hands, &mut firearm, Input { prone: true, ..Input::default() });
    for _ in 0..150 {
        assert_eq!(frame(&mut movement, &mut hands, &mut firearm, Input::default()), 0.0, "level, lying: nothing");
    }
    assert_eq!(movement.state, State::ProneIdle);
    // Look down, a degree a frame until the view will go no further.
    let mut depth = 0.0;
    for _ in 0..240 {
        depth = frame(&mut movement, &mut hands, &mut firearm, Input { look_delta: [0.0, 1.0], ..Input::default() });
    }
    let (position, rotation) = hands.turn_away_offsets();
    let lying = hands.turn_away_value();
    println!(
        "lying, looking {:.1} degrees down: the weapon's line is {depth:.3} m into the ground, value {lying:.3}; the turn-away adds {:.0}, {:.0}, {:.0} mm and {:.1}, {:.1}, {:.1} degrees",
        movement.pitch,
        position[0] * 1000.0,
        position[1] * 1000.0,
        position[2] * 1000.0,
        rotation[0],
        rotation[1],
        rotation[2],
    );
    assert!(depth > firearm.settings.stop_aiming_at && lying > 0.3, "{depth} {lying}");
    // The pose for lying: canted about x too, which the rifle's is not.
    assert!(rotation[0] < 0.0 && position[1] > 0.05, "{position:?} {rotation:?}");

    // Crawling forward with the view still down.
    let mut crawling = 0.0;
    for _ in 0..240 {
        crawling = frame(&mut movement, &mut hands, &mut firearm, Input { move_dir: [0.0, 1.0], ..Input::default() });
    }
    assert_eq!(movement.state, State::ProneMove);
    println!("crawling with the view there: depth {crawling:.3}, value {:.4}", hands.turn_away_value());
    assert!(crawling > 0.0 && hands.turn_away_value() < 0.01, "{crawling} {}", hands.turn_away_value());
    assert_eq!(hands.turn_away_offsets(), ([0.0; 3], [0.0; 3]));
}

/// What the breath does to the weapon over half a minute standing still:
/// how far it dips the muzzle, how far it lifts the weapon, how many
/// breaths, and how far the roll and the yaw are pushed about.
fn breathe(wind: trp_sim::Wind, aiming: bool) -> Option<(f32, f32, usize, f32)> {
    let (mut hands, _) = rig()?;
    let (mut dip, mut rise, mut shake) = (0.0f32, 0.0f32, 0.0f32);
    let (mut peaks, mut rising, mut last) = (0, false, 0.0f32);
    let input = HandsInput { wind, aiming, ..still() };
    // A few rested breaths first: a held breath keeps the pace of the
    // last one drawn.
    for _ in 0..(4.0 / DT) as usize {
        hands.advance(DT, &HandsInput { aiming, ..still() });
    }
    for _ in 0..(30.0 / DT) as usize {
        hands.advance(DT, &input);
        let pitch = hands.hands_rotation.current[0];
        dip = dip.max(pitch);
        rise = rise.max(hands.hands_position.current[2]);
        shake = shake.max(hands.hands_rotation.current[1].abs()).max(hands.hands_rotation.current[2].abs());
        if rising && pitch < last && pitch > 0.3 * dip.max(0.05) {
            peaks += 1;
        }
        rising = pitch > last;
        last = pitch;
    }
    Some((dip, rise, peaks, shake))
}

/// A held breath stills the weapon to a small part of its rise and fall;
/// short of oxygen the breath is deeper and faster; and spent arms shake
/// (weapon-handling spec, "Breathing"). The game's own figures for these
/// are not on record: no recording holds a breath or runs a pool down.
#[test]
fn a_held_breath_stills_the_weapon_and_tiredness_stirs_it() {
    let rested = trp_sim::Wind::default();
    let Some(plain) = breathe(rested, true) else { return };
    let Some(held) = breathe(trp_sim::Wind { held: true, ..rested }, true) else { return };
    let Some(winded) = breathe(trp_sim::Wind { oxygen: 0.2, ..rested }, true) else { return };
    let Some(spent) = breathe(trp_sim::Wind { stamina: 0.05, ..rested }, true) else { return };
    let show = |name: &str, (dip, rise, peaks, shake): (f32, f32, usize, f32)| {
        println!("aiming, {name}: dips {dip:.3} degrees, lifts {:.2} mm, {peaks} breaths in 30 s, roll and yaw pushed up to {shake:.3} degrees", rise * 1000.0);
    };
    show("rested", plain);
    show("breath held", held);
    show("a fifth of the oxygen left", winded);
    show("the arms spent", spent);
    assert!(held.0 < plain.0 * 0.3 && held.1 < plain.1 * 0.3, "a held breath leaves a small part: {held:?} against {plain:?}");
    assert!(held.3 < plain.3 * 0.5, "and of the random pushes: {} against {}", held.3, plain.3);
    assert!(winded.0 > plain.0 * 1.2 && winded.2 > plain.2, "short of oxygen: deeper and faster: {winded:?} against {plain:?}");
    assert!(spent.3 > plain.3 * 1.1 && spent.0 > plain.0 * 2.0, "spent arms shake, and drop the muzzle: {} against {}", spent.3, plain.3);
}

/// The hold-breath key: with the sights up and standing still it holds
/// the breath at the oxygen's cost, a second press lets it go, and so do
/// the sights coming down and walking off.
#[test]
fn the_breath_is_held_only_standing_with_the_sights_up() {
    let Some((_, mut m)) = rig() else { return };
    let press = |m: &mut Movement, aiming: bool| m.step(DT, &Input { aiming, hold_breath: true, ..Input::default() }, &Flat);
    let wait = |m: &mut Movement, seconds: f32, input: Input| {
        for _ in 0..(seconds / DT) as usize {
            m.step(DT, &input, &Flat);
        }
    };
    let aimed = Input { aiming: true, ..Input::default() };
    press(&mut m, false);
    assert!(!m.wind().held, "not without the sights");
    press(&mut m, true);
    assert!(m.wind().held);
    let oxygen = m.stamina.oxygen.current;
    wait(&mut m, 5.0, aimed);
    println!("five seconds of held breath cost {:.1} of {:.0} oxygen", oxygen - m.stamina.oxygen.current, m.stamina.oxygen.capacity);
    assert!((oxygen - m.stamina.oxygen.current - 20.0).abs() < 0.5, "four a second on rested legs");
    press(&mut m, true);
    assert!(!m.wind().held, "a second press lets it go");
    press(&mut m, true);
    wait(&mut m, 0.2, Input::default());
    assert!(!m.wind().held, "so do the sights coming down");
    press(&mut m, true);
    assert!(m.wind().held);
    wait(&mut m, 1.0, Input { move_dir: [0.0, 1.0], ..aimed });
    assert_eq!(m.state, State::Move);
    assert!(!m.wind().held, "and walking off");
    press(&mut m, true);
    assert!(!m.wind().held, "nor is one taken in a walk");
}
