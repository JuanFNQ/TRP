//! Tests of rays, colliders, a blast's reach, the size on screen and where the crowd stands.

use super::*;

#[test]
fn a_ray_enters_each_shape_where_its_face_is() {
    let d = Vec3::Z;
    let o = Vec3::new(0.0, 0.0, -5.0);
    let (t, n) = enter(Placed::Sphere { centre: Vec3::ZERO, radius: 1.0 }, o, d, 10.0).expect("the sphere");
    assert!((t - 4.0).abs() < 1e-5 && (n - Vec3::NEG_Z).length() < 1e-5);
    let (t, n) = enter(Placed::Box { centre: Vec3::ZERO, axes: [Vec3::X, Vec3::Y, Vec3::Z], half: Vec3::splat(0.5) }, o, d, 10.0).expect("the box");
    assert!((t - 4.5).abs() < 1e-5 && (n - Vec3::NEG_Z).length() < 1e-5);
    // A capsule along x, met on its side and on a cap.
    let capsule = Placed::Capsule { a: Vec3::new(-1.0, 0.0, 0.0), b: Vec3::new(1.0, 0.0, 0.0), radius: 0.25 };
    let (t, n) = enter(capsule, o, d, 10.0).expect("its side");
    assert!((t - 4.75).abs() < 1e-5 && (n - Vec3::NEG_Z).length() < 1e-5);
    let (t, n) = enter(capsule, Vec3::new(-5.0, 0.0, 0.0), Vec3::X, 10.0).expect("its cap");
    assert!((t - 3.75).abs() < 1e-5 && (n - Vec3::NEG_X).length() < 1e-5);
    // Nothing from inside, nothing past the ray's end.
    assert!(enter(capsule, Vec3::ZERO, d, 10.0).is_none());
    assert!(enter(Placed::Sphere { centre: Vec3::ZERO, radius: 1.0 }, o, d, 3.0).is_none());
}

/// A standing dummy's hit colliders go by its character's newest frame
/// (`Dummy::placement`), not by what is drawn between two of its frames
///: placed so, a collider is where that pose drawn puts it by
/// the bones' own entities, to the last bit (`in_world`, put together
/// as the engine's `TransformHelper` puts them), and it stays there
/// whatever the skeleton is drawn at; with no frame yet, it goes by the
/// skeleton as it stands.
#[test]
fn a_standing_dummys_colliders_go_by_its_newest_frame() {
    let Some((_, db)) = installed() else { return };
    let turned = |x: f32, y: f32, at: Vec3| Transform { translation: at, rotation: Quat::from_rotation_y(y) * Quat::from_rotation_x(x), scale: Vec3::ONE };
    // Its newest frame, and a pose drawn a little behind it.
    let newest_root = turned(0.1, 3.0, Vec3::new(1.3, 0.02, -15.0));
    let newest = vec![turned(0.2, 0.1, Vec3::new(0.0, 0.95, 0.01)), turned(-0.3, 0.4, Vec3::new(0.0, 0.31, 0.02)), turned(0.7, -0.2, Vec3::new(0.05, 0.12, -0.03))];
    let drawn_root = turned(0.1, 3.0, Vec3::new(1.28, 0.02, -15.0));
    let drawn: Vec<Transform> = newest.iter().map(|t| Transform { rotation: t.rotation * Quat::from_rotation_z(0.05), ..*t }).collect();
    let mut world = bevy::ecs::world::World::new();
    let root = world.spawn(newest_root).id();
    let mut bones: Vec<Entity> = Vec::new();
    for (k, local) in newest.iter().enumerate() {
        let bone = world.spawn(*local).id();
        world.entity_mut(if k == 0 { root } else { bones[k - 1] }).add_child(bone);
        bones.push(bone);
    }
    let rig = DummyRig { root, paths: vec!["a".into(), "a/b".into(), "a/b/c".into()], bones: bones.clone(), parents: vec![None, Some(0), Some(1)], rest: newest.clone(), outfits: Vec::new() };
    let data = HitColliderData {
        object: "c".into(),
        bone: "c".into(),
        local: None,
        shape: Shape::Sphere { centre: [0.0, 0.1, 0.0], radius: 0.1 },
        part: 0,
        collider_type: 0,
        plate: 0,
        figures: [0.0; 6],
        damage_mod: 1.0,
    };
    let collider = Collider { data, bone: 2, local: Affine3A::IDENTITY, body: BodyRef { character: 1, part: 0, collider: 0, plate: 0, damage_mod: 1.0 }, figures: [0.0; 6] };
    let dummy = Dummy {
        which: 0,
        colliders: vec![collider],
        placed: Vec::new(),
        objects: Vec::new(),
        wearer: Wearer::dress(db, Arc::new(ArmorRules::new(&db.config).expect("the armour's rules")), &[], VestSkills::default()),
        worn: false,
        health: Health::new(Arc::new(HealthSettings::new(&db.config).expect("the health's settings")), 1),
        followed: false,
        load: (0.0, Vec::new()),
        dressed: false,
        taken: Vec::new(),
        killing: None,
        fall: None,
        hits: 0,
        puppet: None,
        heading: 1.0,
        model: None,
        empty_grip: None,
        own: None,
        holds: None,
        drawn: Vec::new(),
        arm_bones: [[None; 6]; 2],
        locals: newest.clone(),
        parents: Arc::from(vec![None, Some(0), Some(1)]),
        colliders_frame: None,
        before_frame: None,
        placements: Mutex::new(Vec::new()),
        drawn_by: PosingDrawn::EveryFrame,
        held_frame: None,
        heard: Vec::new(),
    };
    world.insert_resource(Dummies { list: vec![dummy], bleeding_drops: Vec::new(), experience: Vec::new(), marks: Vec::new(), listened: false, economy: true, posing_log: PosingLog::default() });
    world.insert_resource(DummyRigs(vec![rig]));
    world.insert_resource(Time::<()>::default());
    let centre = |world: &mut bevy::ecs::world::World| {
        world.run_system_once(place_dummies).expect("the system ran");
        let dummy = &world.resource::<Dummies>().list[0];
        let Placed::Sphere { centre, .. } = dummy.placed[0] else { panic!("a sphere") };
        (centre, dummy.objects[0])
    };
    // By a frame's pose (`placed_by`, as `placement` places it).
    let by_frame = |world: &mut bevy::ecs::world::World| {
        let (placed, objects) = world.resource::<Dummies>().list[0].placed_by(&newest_root, &newest);
        let Placed::Sphere { centre, .. } = placed[0] else { panic!("a sphere") };
        (centre, objects[0])
    };
    // No frame yet: by the skeleton as it stands (the newest frame's pose).
    let standing = centre(&mut world);
    // By its newest frame: the same place, to the bit.
    assert_eq!(by_frame(&mut world), standing);
    // Drawn between frames: the collider stays where its newest frame put it.
    *world.get_mut::<Transform>(root).expect("the root") = drawn_root;
    for (bone, local) in bones.iter().zip(&drawn) {
        *world.get_mut::<Transform>(*bone).expect("a bone") = *local;
    }
    assert_eq!(by_frame(&mut world), standing);
    // Going by what is drawn, it would have moved.
    let as_drawn = centre(&mut world);
    assert!((as_drawn.0 - standing.0).length() > 0.01, "{as_drawn:?} against {standing:?}");
}

/// The same of a blast (`blast_on_dummies`): a dummy whose feet the
/// blast does not reach (`trp_sim::blast::reaches`), which the economy
/// leaves unposed, takes nothing from it whatever its colliders; one it
/// reaches is met on its colliders of the frame before, kept for the
/// blast however many frames the step made (`Puppet::keep_from`).
#[test]
fn a_blast_on_a_dummy_its_feet_out_of_reach_of_does_nothing() {
    let Some(k) = economy_makings() else { return };
    let grenade = crate::effects::blast_view::grenade_of(k.db, "5710c24ad2720bc3458b45a3").expect("the F-1");
    let mut dummies = economy_dummies(k, true);
    let mut seeded = Seeded(0x5eed_0003);
    let (mut out_of_reach, mut reached) = (0, 0);
    for step in 0..60 {
        step_economy(&mut dummies, step, true);
        for _ in 0..10 {
            let at = near(&dummies, &mut seeded, 14.0);
            for dummy in &dummies.list {
                let feet = dummy.puppet.as_ref().expect("its character").movement.position;
                let frame = dummy.before_frame.or(dummy.colliders_frame);
                let colliders = dummy.colliders_on(frame);
                assert!(matches!(colliders, Colliders::Posed(_)), "the frame before kept");
                let points: Vec<trp_sim::combat::blast::HitPoint> = dummy
                    .colliders
                    .iter()
                    .zip(colliders.objects())
                    .filter(|(c, _)| c.body.plate == 0)
                    .filter_map(|(c, at)| Some(trp_sim::combat::blast::HitPoint { part: *Part::ALL.get(c.body.part as usize)?, at: *at, collider: c.body.collider, plate: c.body.plate }))
                    .collect();
                let outcome = trp_sim::combat::blast::blast(&grenade.explosive, at, feet, &points, &|_, _| trp_sim::combat::blast::Line::Through(0.0), [1.0, 2.0]);
                if trp_sim::combat::blast::reaches(&grenade.explosive, at, feet) {
                    reached += usize::from(!outcome.damage.is_empty());
                } else {
                    assert!(outcome.damage.is_empty() && outcome.armor.is_empty(), "{at:?} {feet:?}");
                    out_of_reach += 1;
                }
            }
        }
    }
    println!("{out_of_reach} dummies out of a blast's reach left alone, {reached} reached and hurt");
    assert!(out_of_reach > 100 && reached > 50, "{out_of_reach} {reached}");
}

/// A character's size on screen for its posing (`size_on_screen`): in a
/// view, its size over the view's pixel; behind it, none, unless within
/// the sun's shadow reach or, its light lit, its light's reach of the
/// view.
#[test]
fn a_dummys_size_on_screen_goes_by_the_views() {
    let projection = Projection::Perspective(PerspectiveProjection { fov: 50f32.to_radians(), aspect_ratio: 16.0 / 9.0, near: 0.05, far: 1000.0, ..default() });
    let clip = projection.get_clip_from_view();
    let view = SeenBy { eye: Vec3::ZERO, frustum: Frustum::from_clip_from_world(&clip), pixels: 1440.0, clip_y: clip.y_axis.y, orthographic: false };
    // Bevy's -z ahead is the game's +z.
    let bound = |z: f32, x: f32| trp_sim::characters::puppet::Bound { ends: [[x, 0.9, z], [x, 1.4, z]], radius: 1.16 };
    let tall = view.pixels * view.clip_y * 2.0 / (2.0 * 50.0f32.hypot(0.0).max(1e-3));
    let ahead = size_on_screen(std::slice::from_ref(&view), &bound(50.0, 0.0), 2.0, 0.0, 0.0).expect("seen");
    assert!((ahead - tall).abs() / tall < 0.01, "{ahead} {tall}");
    assert_eq!(size_on_screen(std::slice::from_ref(&view), &bound(-20.0, 0.0), 2.0, 0.0, 0.0), None, "behind");
    assert!(size_on_screen(std::slice::from_ref(&view), &bound(-20.0, 0.0), 2.0, 30.0, 0.0).is_some(), "its shadow");
    assert!(size_on_screen(std::slice::from_ref(&view), &bound(10.0, 60.0), 2.0, 0.0, 0.0).is_none(), "far to the side");
    assert!(size_on_screen(std::slice::from_ref(&view), &bound(10.0, 60.0), 2.0, 0.0, 60.0).is_some(), "its light");
    // Just out of view, within the margin: posed before it comes in.
    let edge = 50.0 * (25f32.to_radians().tan() * 16.0 / 9.0).atan().tan();
    assert!(size_on_screen(std::slice::from_ref(&view), &bound(50.0, edge + 1.16 + 4.0), 2.0, 0.0, 0.0).is_some(), "within the margin");
    assert_eq!(size_on_screen(&[], &bound(50.0, 0.0), 2.0, 100.0, 0.0), None, "no view");
}

/// The crowd (`--start crowd-<n>`): the two dummies where they stand,
/// the rest in rows behind the start, near and far, none on another.
#[test]
fn the_crowd_stands_near_far_and_behind() {
    assert_eq!(crowd_of("crowd-50"), Some(50));
    assert_eq!(crowd_of("crowd-1"), None);
    assert_eq!(crowd_of("dummy-80"), None);
    let places = crowd(200);
    assert_eq!(places.len(), 200);
    assert_eq!(&places[..2], &PLACES[..]);
    let start_z = PLACES[0].at.z + 3.5;
    let behind = places.iter().filter(|p| p.at.z > start_z).count();
    let near = places.iter().filter(|p| p.at.z <= -18.0 && p.at.z > -60.0 && p.at.x > -10.0).count();
    let far = places.iter().filter(|p| p.at.z <= -100.0).count();
    let aside = places.iter().filter(|p| p.at.x <= -70.0).count();
    assert!(behind >= 20 && near >= 45 && far >= 70 && aside >= 45, "{behind} {near} {far} {aside}");
    for (i, a) in places.iter().enumerate() {
        for b in &places[i + 1..] {
            assert!(a.at.distance(b.at) >= 0.69, "{a:?} {b:?}");
        }
    }
    assert_eq!(crowd(8).len(), 8);
}

/// A dummy's placed colliders as the stroke's world takes them
/// (`blade_shape`, the game's axes, z turned over): swept in the engine,
/// each is met where the bullets' own line meets it (`enter`).
#[test]
fn a_dummys_colliders_stand_in_the_blades_world_where_a_bullet_meets_them() {
    let turned = Quat::from_rotation_y(0.6) * Quat::from_rotation_x(0.3);
    let axes = [turned * Vec3::X, turned * Vec3::Y, turned * Vec3::Z];
    let shapes = [
        Placed::Sphere { centre: Vec3::new(0.1, 1.2, -3.0), radius: 0.12 },
        Placed::Box { centre: Vec3::new(-0.05, 1.3, -3.0), axes, half: Vec3::new(0.15, 0.2, 0.1) },
        Placed::Capsule { a: Vec3::new(-0.3, 1.0, -3.1), b: Vec3::new(0.25, 1.15, -2.9), radius: 0.06 },
    ];
    for placed in shapes {
        let (at, turn, shape) = blade_shape(placed);
        let mut world = trp_physics::World::new(trp_physics::Settings::game());
        world.add_kinematic(at, turn, &[PhysicsCollider { shape, at: [0.0; 3], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() }], 16);
        world.build_query_trees();
        // A thin box swept from the player's side (game z 0) along +z,
        // through the shape's middle.
        let middle = match placed {
            Placed::Sphere { centre, .. } | Placed::Box { centre, .. } => centre,
            Placed::Capsule { a, b, .. } => (a + b) / 2.0,
        };
        let from = [middle.x, middle.y, 0.0];
        let touches = world.sweep_box_touches(from, [1e-4; 3], [0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0], 6.0);
        let (t, _) = enter(placed, Vec3::new(middle.x, middle.y, 0.0), Vec3::NEG_Z, 6.0).expect("the line meets it");
        assert_eq!(touches.len(), 1);
        assert!((touches[0].distance - t).abs() < 2e-3, "{placed:?}: swept {} against {t}", touches[0].distance);
    }
}
