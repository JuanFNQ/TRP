//! Collider arms, threads of their own, the bound around colliders and ragdolls.

use super::*;

/// The hit colliders' arms go by the animator's straight pose
/// (`SMOOTH ANIMATIONS` moves no hit), the drawn arms by the pose
/// for drawing: a character drawn smooth and one not, stepped alike through
/// a reload and a walk, have the same straight arms on every frame to the
/// bit, the one not smooth its drawn arms the same, and the smooth one's
/// drawn arms a few millimetres off the straight ones at most.
#[test]
fn the_colliders_arms_go_by_the_straight_pose() {
    let Some(k) = makings() else { return };
    let (mut smooth, mut plain) = (made_for_economy(k, true, true), made_for_economy(k, true, false));
    let (mut hs, mut hp) = (economy_health(k), economy_health(k));
    let (mut most, mut frames_apart) = (0.0f32, 0);
    let dt = 1.0 / 120.0;
    for step in 0..900 {
        if step == 60 {
            smooth.firearm_mut().unwrap().reload_with(30);
            plain.firearm_mut().unwrap().reload_with(30);
        }
        let input = Input { move_dir: if step > 500 { [1.0, 0.0] } else { [0.0, 0.0] }, ..Input::default() };
        smooth.step_with(dt, &input, &Flat, Some(&mut hs));
        plain.step_with(dt, &input, &Flat, Some(&mut hp));
        let (s, p) = (smooth.pose().unwrap(), plain.pose().unwrap());
        assert_eq!(format!("{:?}", s.straight_arms), format!("{:?}", p.straight_arms), "step {step}");
        assert_eq!(format!("{:?}", p.straight_arms), format!("{:?}", p.arms), "step {step}");
        let apart = (0..2).flat_map(|side| (0..5).map(move |bone| (side, bone))).map(|(side, bone)| distance(s.arms[side][bone].0, s.straight_arms[side][bone].0)).fold(0.0f32, f32::max);
        if apart > 0.0 {
            frames_apart += 1;
        }
        most = most.max(apart);
    }
    println!("drawn smooth, the drawn arms off the straight on {frames_apart} of 900 frames, {:.2} mm at most", most * 1000.0);
    assert!(frames_apart > 0 && most < 0.01, "{most}");
}

/// The economy's characters may be stepped on threads of a host's own: a
/// character's step touches nothing but itself. Eight stepped each on a
/// thread of its own, posed when asked, end as the same eight stepped one
/// after another, to the bit, their newest poses with them.
#[test]
fn characters_step_on_threads_of_their_own() {
    fn shared<T: Send + Sync>() {}
    shared::<Puppet>();
    let Some(k) = makings() else { return };
    let dt = 1.0 / 60.0;
    let run = |scene: Scene| {
        let mut p = made_for_economy(k, scene.firearm(), true);
        p.posing(Posing::Asked);
        let mut h = economy_health(k);
        for step in 0..180 {
            let input = scene.act(step, dt, &mut p, &mut h);
            p.step_with(dt, &input, &Flat, Some(&mut h));
        }
        (simulation_of(&p, &h), pose_bits(p.pose().unwrap()))
    };
    let threaded: Vec<(String, Vec<u32>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = SCENES.into_iter().map(|scene| scope.spawn(move || run(scene))).collect();
        handles.into_iter().map(|handle| handle.join().expect("a thread")).collect()
    });
    for (scene, on_thread) in SCENES.into_iter().zip(&threaded) {
        assert!(run(scene) == *on_thread, "{scene:?}");
    }
    println!("{} characters stepped on threads of their own, the same as one after another", threaded.len());
}

/// A place and a turn put after another (the second in the first's frame).
fn then(a: ([f32; 3], [f32; 4]), b: ([f32; 3], [f32; 4])) -> ([f32; 3], [f32; 4]) {
    let shifted = trp_sim::base::math::quat_rotate(a.1, b.0);
    (std::array::from_fn(|i| a.0[i] + shifted[i]), trp_sim::base::math::quat_mul(a.1, b.1))
}

/// Each hit collider of the player prefab where `pose` puts it, as the app
/// places a dummy's (`dummy::posed`, `place_dummies`): the body's bones
/// from where its skeleton hangs, each arm's from the frame the rig hangs in
/// (the feet turned by the view's yaw, the ribcage, the view's pitch, the
/// rig's root's turn) by the solver's arms (the straight ones where
/// `straight`); each as balls (a sphere's, a capsule's two ends, a box's
/// corners with none), named by its object.
fn colliders_placed(k: &Makings, rig_root: [f32; 4], pose: &PuppetPose, straight: bool) -> Vec<(String, [f32; 3], f32)> {
    let paths = collider_bones();
    let root = (pose.body_at, pose.body_turn);
    let mut world: HashMap<&str, ([f32; 3], [f32; 4])> = HashMap::new();
    for (path, local) in paths.iter().zip(&pose.bones) {
        let parent = path.rsplit_once('/').map_or(root, |(above, _)| world[above]);
        world.insert(path.as_str(), then(parent, *local));
    }
    let frame = then(then((pose.position, trp_sim::base::math::quat_axis(1, pose.yaw)), (pose.ribcage, trp_sim::base::math::quat_axis(0, pose.pitch))), ([0.0; 3], rig_root));
    let arms = if straight { &pose.straight_arms } else { &pose.arms };
    let arm_bones = ["Upperarm", "Forearm1", "Forearm2", "Forearm3", "Palm"];
    let mut out = Vec::new();
    for c in &k.colliders {
        let bone = if let Some((side, at)) = ['R', 'L'].iter().enumerate().find_map(|(side, s)| arm_bones.iter().position(|name| c.bone == format!("Base Human{s}{name}")).map(|at| (side, at))) {
            then(frame, arms[side][at])
        } else {
            let path = paths.iter().find(|path| path.ends_with(&format!("/{}", c.bone)) || **path == c.bone).unwrap_or_else(|| panic!("no bone placed for {}'s {}", c.object, c.bone));
            world[path.as_str()]
        };
        let object = c.local.map_or(bone, |local| then(bone, local));
        let at = |p: [f32; 3]| then(object, (p, [0.0, 0.0, 0.0, 1.0])).0;
        match c.shape {
            trp_data::character::ragdoll::Shape::Sphere { centre, radius } => out.push((c.object.clone(), at(centre), radius)),
            trp_data::character::ragdoll::Shape::Capsule { centre, radius, height, direction } => {
                let mut along = [0.0f32; 3];
                along[direction.min(2) as usize] = (height * 0.5 - radius).max(0.0);
                for sign in [-1.0, 1.0] {
                    out.push((c.object.clone(), at(std::array::from_fn(|i| centre[i] + sign * along[i])), radius));
                }
            }
            trp_data::character::ragdoll::Shape::Box { centre, size } => {
                for corner in 0..8 {
                    let p: [f32; 3] = std::array::from_fn(|i| centre[i] + if corner >> i & 1 == 1 { 0.5 } else { -0.5 } * size[i]);
                    out.push((c.object.clone(), at(p), 0.0));
                }
            }
        }
    }
    out
}

/// A wall to vault: a metre high, 0.35 m thick, half a metre ahead (the
/// install tests' yard), the character kept out of it as the game's own
/// controller keeps it.
struct Wall;

impl Wall {
    const BOX: [f32; 4] = [0.5, 0.85, 0.0, 1.0];

    fn course() -> trp_sim::locomotion::collision::Course {
        use trp_sim::locomotion::collision::Tread;
        let [near, far, _, high] = Self::BOX;
        trp_sim::locomotion::collision::Course { treads: vec![Tread::level(f32::MIN, near, 0.0), Tread::level(near, far, high), Tread::level(far, f32::MAX, 0.0)] }
    }
}

/// The game's capsule, from the player prefab.
const WALL_CAPSULE_RADIUS: f32 = 0.366;

impl trp_sim::World for Wall {
    fn ground_height(&self, _x: f32, z: f32) -> f32 {
        let [near, far, _, high] = Self::BOX;
        if z >= near && z <= far { high } else { 0.0 }
    }

    fn line(&self, from: [f32; 3], to: [f32; 3]) -> Option<f32> {
        let length = (0..3).map(|k| (to[k] - from[k]).powi(2)).sum::<f32>().sqrt();
        let mut nearest: Option<f32> = (from[1] >= 0.0 && to[1] < 0.0).then(|| from[1] / (from[1] - to[1]));
        let [near, far, low, high] = Self::BOX;
        let (mut enter, mut leave, mut misses) = (0.0f32, 1.0f32, false);
        for (k, (a, b)) in [(2, (near, far)), (1, (low, high))] {
            let (start, travel) = (from[k], to[k] - from[k]);
            if travel.abs() < 1e-9 {
                misses |= start <= a || start >= b;
                continue;
            }
            let (t0, t1) = ((a - start) / travel, (b - start) / travel);
            enter = enter.max(t0.min(t1));
            leave = leave.min(t0.max(t1));
        }
        if !misses && enter <= leave && enter > 0.0 {
            nearest = Some(nearest.map_or(enter, |n| n.min(enter)));
        }
        nearest.map(|share| share * length)
    }

    fn terrain(&self, at: [f32; 3]) -> bool {
        at[1] <= 1e-3
    }

    fn obstructed(&self, from: [f32; 3], to: [f32; 3], capsule: trp_sim::locomotion::movement::Capsule) -> [f32; 3] {
        let settled = Self::course().settle([from[2], from[1]], [to[2], to[1]], capsule, WALL_CAPSULE_RADIUS);
        [to[0], settled[1], settled[0]]
    }

    fn sets_feet(&self) -> bool {
        true
    }

    fn held(&self, at: [f32; 3], capsule: trp_sim::locomotion::movement::Capsule) -> bool {
        Self::course().held([at[2], at[1]], capsule, WALL_CAPSULE_RADIUS)
    }
}

/// The bound (`Puppet::bound`, made without a pose) holds every hit
/// collider of the player prefab, placed as the app places a dummy's, on
/// every frame of a sweep of poses: standing, walking, sprinting,
/// crouching, lying down and crawling, leaning both ways, vaulting a wall,
/// reloading, the empty hands standing and walking, and killed; the arms
/// both as drawn and straight.
#[test]
fn the_bound_holds_every_collider_in_any_pose() {
    let Some(k) = makings() else { return };
    let dt = 1.0 / 120.0;
    type Plan = fn(usize, &mut Puppet, &mut Health) -> Input;
    let sweeps: [(&str, bool, bool, usize, Plan); 11] = [
        ("standing", true, false, 120, |_, _, _| Input::default()),
        ("walking", true, false, 240, |_, _, _| Input { move_dir: [1.0, 0.0], ..Input::default() }),
        ("sprinting", true, false, 300, |step, _, _| Input { move_dir: [0.0, 1.0], sprint: step > 60, ..Input::default() }),
        ("crouching", true, false, 300, |step, _, _| Input { crouch: step == 10, move_dir: if step > 150 { [1.0, 0.0] } else { [0.0; 2] }, ..Input::default() }),
        ("lying down", true, false, 480, |step, _, _| Input { prone: step == 10, move_dir: if step > 240 { [0.0, 1.0] } else { [0.0; 2] }, ..Input::default() }),
        ("leaning", true, false, 360, |step, _, _| Input { lean: if step < 180 { 1.0 } else { -1.0 }, ..Input::default() }),
        ("vaulting", true, true, 480, |_, _, _| Input { move_dir: [0.0, 1.0], vault: true, ..Input::default() }),
        ("reloading", true, false, 480, |step, p, _| {
            if step == 20 {
                p.firearm_mut().unwrap().reload_with(30);
            }
            Input::default()
        }),
        ("the empty hands standing", false, false, 120, |_, _, _| Input::default()),
        ("the empty hands walking", false, false, 240, |_, _, _| Input { move_dir: [-1.0, 0.0], ..Input::default() }),
        ("killed", true, false, 240, |step, _, h| {
            if step == 60 {
                h.apply_damage(Part::Head, 100.0, DamageKind::Bullet);
            }
            Input::default()
        }),
    ];
    let mut least = (f32::MAX, String::new());
    let mut placed = 0;
    for (name, firearm, wall, steps, plan) in sweeps {
        let mut p = made_for_economy(k, firearm, true);
        let mut h = economy_health(k);
        let rig_root = if firearm { k.akm.hierarchy.nodes[0].rotation } else { k.empty.hierarchy.nodes[0].rotation };
        let (mut highest, mut vaulted, mut sprinted, mut lay, mut crouched, mut leaned) = (0.0f32, false, false, false, false, 0.0f32);
        for step in 0..steps {
            let input = plan(step, &mut p, &mut h);
            if wall {
                p.step_with(dt, &input, &Wall, Some(&mut h));
            } else {
                p.step_with(dt, &input, &Flat, Some(&mut h));
            }
            vaulted |= p.movement.state == State::Vault;
            sprinted |= p.movement.state == State::Sprint;
            lay |= p.movement.prone;
            crouched |= p.movement.pose_level < 0.5;
            leaned = leaned.max(p.movement.tilt.abs());
            let bound = p.bound().expect("a bound");
            let pose = p.pose().expect("a pose");
            for straight in [false, true] {
                for (object, at, radius) in colliders_placed(k, rig_root, pose, straight) {
                    let slack = bound.radius - bound.distance_to_axis(at) - radius;
                    if slack < least.0 {
                        least = (slack, format!("{object} {name} step {step}"));
                    }
                    highest = highest.max(at[1]);
                    assert!(bound.holds(at, radius), "{name}, step {step}: {object} at {at:?} (radius {radius}) outside {bound:?}");
                    placed += 1;
                }
            }
        }
        // Each pose was had.
        match name {
            "vaulting" => assert!(vaulted, "the wall was vaulted"),
            "sprinting" => assert!(sprinted),
            "lying down" => assert!(lay && p.movement.state == State::ProneMove, "{:?}", p.movement.state),
            "crouching" => assert!(crouched),
            "leaning" => assert!(leaned > 0.5, "{leaned}"),
            "killed" => assert!(!h.alive),
            "reloading" => assert!(!p.firearm().unwrap().reloading()),
            _ => {}
        }
        println!("{name}: the colliders up to {highest:.2} m; bound radius {:.3}; sprinted {sprinted}, lay {lay}, crouched {crouched}, leaned {leaned:.2}, vaulted {vaulted}", p.bound().unwrap().radius);
    }
    println!("{placed} collider places inside; the least room {:.3} m ({})", least.0, least.1);
    assert!(placed > 100_000);
}

/// The pose of a frame posed when asked is made only when asked, kept once
/// made, and made from the frame's own recipe whatever has happened since:
/// a character posed when asked walks on four frames after one is asked
/// for, and that frame's pose is the one a character posed in full made.
#[test]
fn an_asked_frame_is_posed_when_asked_and_kept() {
    let Some(k) = makings() else { return };
    let (mut a, mut b) = (made_for_economy(k, true, true), made_for_economy(k, true, true));
    b.posing(Posing::Asked);
    let walk = Input { move_dir: [1.0, 0.0], ..Input::default() };
    for _ in 0..30 {
        a.step_with(1.0 / 120.0, &walk, &Flat, None);
        b.step_with(1.0 / 120.0, &walk, &Flat, None);
    }
    let frame = b.newest_frame().expect("frames");
    assert!(a.is_posed(frame) && !b.is_posed(frame), "posed in full at once, asked not yet");
    assert_eq!(b.recipe().unwrap().bound, a.recipe().unwrap().bound, "the bound without a pose");
    for _ in 0..3 {
        a.step_with(1.0 / 120.0, &walk, &Flat, None);
        b.step_with(1.0 / 120.0, &walk, &Flat, None);
    }
    assert!(!b.is_posed(frame));
    let made = pose_bits(b.pose_of_frame(frame).expect("still kept"));
    assert!(b.is_posed(frame) && !b.is_posed(frame + 3));
    assert_eq!(made, pose_bits(a.pose_of_frame(frame).unwrap()));
    assert!(b.pose_of_frame(frame + 4).is_none() && b.pose_of_frame(frame.saturating_sub(10)).is_none(), "only what is kept");
}

/// A ragdoll taking over starts from the straight pose
/// (`SMOOTH ANIMATIONS` moves no result): a
/// character drawn smooth and one not, stepped alike through a reload and a
/// walk, the smooth one posed in full and when asked by turns: on every
/// frame the smooth one's straight pose is the plain one's pose to the bit;
/// and a frame the host keeps (`keep_from`) is kept past the frames drawn
/// between, however many steps come, and let go once it is not.
#[test]
fn a_ragdoll_starts_from_the_straight_pose() {
    let Some(k) = makings() else { return };
    let (mut smooth, mut plain) = (made_for_economy(k, true, true), made_for_economy(k, true, false));
    let (mut hs, mut hp) = (economy_health(k), economy_health(k));
    let dt = 1.0 / 120.0;
    let mut apart_frames = 0;
    for step in 0..600 {
        if step == 60 {
            smooth.firearm_mut().unwrap().reload_with(30);
            plain.firearm_mut().unwrap().reload_with(30);
        }
        smooth.posing(if (step / 50) % 2 == 0 { Posing::Full } else { Posing::Asked });
        let input = Input { move_dir: if step > 400 { [1.0, 0.0] } else { [0.0, 0.0] }, ..Input::default() };
        smooth.step_with(dt, &input, &Flat, Some(&mut hs));
        plain.step_with(dt, &input, &Flat, Some(&mut hp));
        let straight = smooth.straight_pose().unwrap();
        assert_eq!(pose_bits(&straight), pose_bits(plain.pose().unwrap()), "step {step}");
        if pose_bits(&straight) != pose_bits(smooth.pose().unwrap()) {
            apart_frames += 1;
        }
    }
    println!("drawn smooth, its straight pose the plain one's on 600 frames, its drawn pose apart from it on {apart_frames}");
    assert!(apart_frames > 0);
    // A frame kept for the host across a slow draw (ten frames a step).
    let mut p = made_for_economy(k, true, true);
    let mut h = economy_health(k);
    p.posing(Posing::Asked);
    p.step_with(1.0 / 60.0, &Input::default(), &Flat, Some(&mut h));
    let frame = p.newest_frame().unwrap();
    p.keep_from(Some(frame));
    for _ in 0..3 {
        p.step_with(10.0 / 120.0, &Input::default(), &Flat, Some(&mut h));
    }
    assert!(p.newest_frame().unwrap() >= frame + 30 && p.recipe_of_frame(frame).is_some() && p.pose_of_frame(frame).is_some(), "kept");
    p.keep_from(None);
    p.step_with(1.0 / 60.0, &Input::default(), &Flat, Some(&mut h));
    assert!(p.recipe_of_frame(frame).is_none(), "let go");
}
