//! The arms' solve, walls turning the weapon away and the ground under the camera.

use super::*;

/// The arms as the game's solver leaves them, standing with the weapon at
/// rest: both palms on their holds, the left on the handguard's grip and
/// the right on the weapon's marker, with each bone at its own length.
/// And the choice of clip the hold depends on: the resting clip is the
/// one the animator's blend tree gives with its parameter left alone.
#[test]
fn the_arms_hold_the_weapon_as_the_game_solves_them() {
    use trp_data::character::player::LimbData;
    use trp_sim::weapons::arms;
    use trp_sim::base::rig::{ArmNodes, WeaponRig};

    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let hard = HardSettings::load(&install).expect("settings object");
    let engine = EngineSettings::load(&install).expect("engine settings");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, AKM).expect("weapon");
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

    let body = trp_sim::locomotion::body::BodyData::load(&mut assets).expect("body animator");
    let mut movement = Movement::new(
        trp_sim::load_movement_settings(&mut assets, &db, &hard, &engine, 0.0).expect("movement"),
        trp_sim::Body::new(&body, weapon.cell_width as f32),
    );
    for _ in 0..60 {
        movement.step(DT, &Input::default(), &Flat);
    }
    // The rig as the weapon's animator has it at rest.
    let animator = trp_sim::weapons::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("the weapon's animator");
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
    let muzzle = trp_sim::base::math::quat_rotate(turn, [0.0, -length, 0.0]);
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
    let Ok(install) = crate::fixture::install().ok_or(()) else { return };
    let shape = trp_data::character::player::CameraBoxData::load(&mut Assets::new(install).expect("manifest")).expect("camera box");
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
    let sight = trp_sim::AimPoint { bone: [0.0, -0.25, 0.1], origin: [0.0, -0.25, 0.0], barrel: [0.0, 0.0, 1.0], line: [0.0, 0.0, 1.0] };
    let eye_over_ground = |movement: &Movement, at: [f32; 3]| {
        let (origin, turn) = movement.rig_frame();
        origin[1] + trp_sim::base::math::quat_rotate(turn, at)[1]
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
