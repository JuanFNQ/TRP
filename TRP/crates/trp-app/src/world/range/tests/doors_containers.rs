//! Tests of the doors worked and the containers' gestures.

use super::*;

/// The doors: shut, they stop a walk through the doorway; looked at
/// from the lane's side within a metre they are offered, and each is
/// pulled with the hinge to its own hand; opened, the doorway is clear.
/// A door swung into a character standing in its way pushes it off,
/// with the cap on speed lifted while the leaf is in it.
#[test]
fn the_doors_are_worked_and_walked_through() {
    let mut range = Range::default();
    range.thing_settings = Some(thing_settings());
    let height = Capsule { underside: 0.0, top: 1.6, lift: true };
    let doors: Vec<usize> = (0..range.things.len()).filter(|&i| range.things[i].name == "door").collect();
    assert_eq!(doors.len(), 2);
    let walk = |range: &Range, mut at: [f32; 3], step: [f32; 2], frames: usize| {
        for _ in 0..frames {
            at = range.obstructed(at, [at[0] + step[0], at[1], at[2] + step[1]], height);
        }
        at
    };
    // Through the middle of the doorway from the start's side (towards
    // -z in the simulation's axes): stopped the radius off the shut
    // leaves' face.
    let start = range.place("doors").unwrap();
    let stopped = walk(&range, start, [0.0, -0.0235], 200);
    let face = DOOR_WALL_Z - WALL_THICK / 2.0;
    assert!((stopped[2] - (-face + range.radius)).abs() < 1e-3, "{stopped:?}");
    // Looked at from 0.9 m (the eye at 1.6 m, level): offered; from
    // 1.2 m, out of a door's reach.
    let eye = |x: f32, z: f32| Vec3::new(x, 1.6, z);
    let across = (DOORWAY[0] + DOORWAY[1]) / 2.0 + 0.4;
    let seen = range.looked_at(eye(across, face - 0.9 - 0.15), eye(across, face + 0.4), 1.15);
    assert!(seen.is_some_and(|i| doors.contains(&i)), "{seen:?}");
    let seen = range.looked_at(eye(across, face - 1.2 - 0.15), eye(across, face + 0.1), 1.15);
    assert_eq!(seen, None);
    // Pulled, the one hinged nearer the start with one hand and the
    // other with the other.
    let gestures: Vec<u8> = doors.iter().map(|&i| range.things[i].thing.door_gesture(start)).collect();
    assert_eq!(gestures.iter().copied().collect::<std::collections::BTreeSet<_>>(), [24, 26].into());
    // From beyond, pushed.
    let beyond = range.place("yard").unwrap();
    assert!(doors.iter().all(|&i| range.things[i].thing.door_gesture(beyond) == 27));
    // Both opened; 3 s on they are open and the doorway is clear.
    let settings = thing_settings();
    for &i in &doors {
        range.things[i].thing.work(true, start, &settings);
    }
    for _ in 0..400 {
        range.step_things(1.0 / 120.0);
    }
    assert!(doors.iter().all(|&i| range.things[i].thing.state() == ThingState::Open));
    let through = walk(&range, start, [0.0, -0.0235], 200);
    assert!(through[2] < -(DOOR_WALL_Z + 2.0), "{through:?}");
    // Shut on a character standing where the first door's leaf will
    // pass: it is pushed out of the leaf's way, the cap lifted.
    let door = doors[0];
    let hinge = range.things[door].thing.hinge.unwrap();
    let opening = turn(hinge.forward, hinge.axis, 90.0);
    let mut standing: [f32; 3] = std::array::from_fn(|k| hinge.at[k] + hinge.forward[k] * 0.25 + opening[k] * 0.55);
    let first = standing;
    range.things[door].thing.work(false, standing, &settings);
    let mut lifted = false;
    for _ in 0..400 {
        range.step_things(1.0 / 120.0);
        lifted |= range.lifts_cap(standing, height);
        standing = range.obstructed(standing, standing, height);
    }
    println!("a door shut on a character standing at {first:?}: pushed to {standing:?}");
    assert!(lifted && range.things[door].thing.state() == ThingState::Shut);
    assert!(!range.site.blocks[range.things[door].leaf.unwrap()].overlaps(standing, height, range.radius), "{standing:?}");
}

/// The containers and the trunk each name the gestures recorded for
/// their kind, and are looked at by their bodies.
#[test]
fn the_containers_ask_for_their_own_gestures() {
    let range = Range::default();
    let gesture = |name: &str| {
        let placed = range.things.iter().find(|p| p.name == name).unwrap();
        placed.thing.gesture([0.0; 3]).map(Interaction::index)
    };
    assert_eq!(gesture("wooden crate"), Some(29));
    assert_eq!(gesture("technical supply crate"), Some(29));
    assert_eq!(gesture("dead Scav"), Some(21));
    assert_eq!(gesture("drawer"), Some(25));
    assert_eq!(gesture("rigs drawer"), Some(25));
    assert_eq!(gesture("plates box"), Some(29));
    assert_eq!(gesture("eyewear dresser"), Some(25));
    // Each place of the storage is looked at from where its start puts
    // the eye, at its drawer or its lid, within the game's reach of
    // 1.3 m from the eye.
    for (index, label) in CABINETS.iter().enumerate() {
        let name = label.to_lowercase().replace(' ', "-");
        let feet = range.place(&name).unwrap();
        let eye = in_bevy(feet) + Vec3::Y * 1.6;
        let [x, z] = cabinet_at(index);
        let furniture = Furniture::of(label);
        let size = furniture.size();
        let target = match furniture {
            Furniture::Box => Vec3::new(x + size.z / 2.0 - 0.05, size.y, z),
            _ => Vec3::new(x + size.z / 2.0, furniture.drawer_height(), z),
        };
        assert!(eye.distance(target) < 1.3, "{label}: {:.2} m from the eye", eye.distance(target));
        let seen = range.looked_at(eye, eye + (target - eye) * 1.2, 1.15);
        assert_eq!(seen.map(|i| range.things[i].name), Some(format!("{} {}", label.to_lowercase(), furniture.word()).as_str()), "{label}");
    }
    // The four past the car: the gestures their takes recorded (the
    // airdrop crate's the defaults); each seen from its start along its
    // view, within the game's reach of 1.3 m from the eye, the eye where
    // the takes had it looking so far down (1.26 to 1.32 m up and 0.40
    // to 0.44 ahead of the feet: 1.3 and 0.42).
    assert_eq!(gesture("weapon box"), Some(29));
    assert_eq!(gesture("toolbox"), Some(29));
    assert_eq!(gesture("safe"), Some(19));
    assert_eq!(gesture("airdrop weapon crate"), Some(27));
    for (start, name) in [("weapon-box", "weapon box"), ("toolbox", "toolbox"), ("safe", "safe"), ("airdrop-crate", "airdrop weapon crate")] {
        let [yaw, pitch] = range.start_view(start).unwrap().map(f32::to_radians);
        let level = Vec3::new(yaw.sin(), 0.0, -yaw.cos());
        let eye = in_bevy(range.place(start).unwrap()) + Vec3::Y * 1.3 + level * 0.42;
        let ahead = level * pitch.cos() - Vec3::Y * pitch.sin();
        let seen = range.looked_at(eye, eye + ahead * 1.3, 1.15);
        assert_eq!(seen.map(|i| range.things[i].name), Some(name), "{start}");
    }
    assert_eq!(gesture("trunk"), Some(27));
    // The car's trunk seen from behind it, a metre off.
    let rear = CAR[0] - 2.1;
    let seen = range.looked_at(Vec3::new(rear - 1.0, 1.5, CAR[1]), Vec3::new(rear + 0.2, 0.6, CAR[1]), 1.15);
    assert_eq!(seen.map(|i| range.things[i].name), Some("trunk"));
    // Every place before a thing is clear of everything solid.
    let height = Capsule { underside: 0.0, top: 1.6, lift: true };
    for name in ["doors", "yard", "window", "workbench", "armor-bench", "car", "wooden-crate", "supply-crate", "scav", "drawer", "table", "key-table", "high-shelf", "floor", "stairs", "rifles", "custom", "wire", "rigs", "plates", "headsets", "eyewear", "surfaces", "platform", "shelf", "launchers", "weapon-box", "toolbox", "safe", "airdrop-crate"] {
        let at = range.place(name).unwrap();
        assert_eq!(range.obstructed(at, at, height), at, "{name} stands in something");
    }
}
