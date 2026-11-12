//! Tests of mounting with the install's body animator, on a sill set before the character.
//! After `docs/spec/mounting.md`: the search's frames, the approach to the
//! target place, the stance by the pitch and the lean by the yaw while held,
//! the exits with and without the 0.2 s, the walk's lock, and the bipod's
//! gesture and its mark. These pass trivially, with a note, where no install
//! is configured.

use trp_data::setup::engine::EngineSettings;
use trp_data::setup::hard_settings::HardSettings;
use trp_data::unity::Assets;
use trp_sim::locomotion::collision::Block;
use trp_sim::weapons::mounting::{Hit, Layer, MountEvent, Side, WeaponMount};
use trp_sim::{Acted, Flat, Input, Movement, World};

const DT: f32 = 1.0 / 60.0;

fn movement() -> Option<Movement> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let hard = HardSettings::load(&assets.install).expect("settings object");
    let engine = EngineSettings::load(&assets.install).expect("engine settings");
    let settings = trp_sim::load_movement_settings(&mut assets, db, &hard, &engine, 0.0).expect("movement settings");
    let body = trp_sim::locomotion::body::BodyData::load(&mut assets).expect("body animator");
    let mut m = Movement::new(settings, trp_sim::Body::new(&body, 5.0));
    // A rifle's figures: the pivot 0.69 m ahead of the weapon's root and
    // 4 cm under it, as the M700's left hand's marker puts it.
    let weapon = WeaponMount { mountable: true, pivot: [0.0, -0.69, -0.04], ..WeaponMount::default() };
    m.set_mount_weapon(Some(weapon), false);
    Some(m)
}

/// A sill across the character's way, its face 1 m ahead (z), its top at
/// 1.05 m, on level ground.
struct Sill;

impl Sill {
    fn block() -> Block {
        Block::square([-3.0, 0.0, 1.0], [3.0, 1.05, 1.4])
    }
}

impl World for Sill {
    fn ground_height(&self, _x: f32, _z: f32) -> f32 {
        0.0
    }

    fn ray(&self, from: [f32; 3], direction: [f32; 3], length: f32, layer: Layer) -> Option<Hit> {
        let to: [f32; 3] = std::array::from_fn(|k| from[k] + direction[k] * length);
        let sill = Self::block().entered_face(from, to).map(|(share, normal)| (share * length, normal));
        let ground = Flat.ray(from, direction, length, layer).map(|hit| (hit.distance, hit.normal));
        let (distance, normal) = sill.into_iter().chain(ground).min_by(|a, b| a.0.total_cmp(&b.0))?;
        Some(Hit { distance, point: std::array::from_fn(|k| from[k] + direction[k] * distance), normal })
    }
}

/// A frame's input with a mountable firearm at rest in hand.
fn armed() -> Input {
    Input { mount_may: true, blind_firearm: true, blind_may_start: true, hands_idle: true, ..Input::default() }
}

fn run(m: &mut Movement, seconds: f32, input: Input, world: &dyn World) -> Vec<MountEvent> {
    let mut events = Vec::new();
    for _ in 0..(seconds / DT).round() as usize {
        m.step(DT, &input, world);
        events.extend(m.take_mount_events());
    }
    events
}

fn press(m: &mut Movement, world: &dyn World) -> Vec<MountEvent> {
    m.step(DT, &Input { mount: true, ..armed() }, world);
    m.take_mount_events()
}

#[test]
fn a_sill_ahead_is_found_on_the_second_frame_and_the_feet_go_to_its_place() {
    let Some(mut m) = movement() else { return };
    run(&mut m, 0.5, armed(), &Sill);
    assert!(press(&mut m, &Sill).is_empty(), "nothing on the frame of the key");
    assert!(run(&mut m, DT, armed(), &Sill).is_empty(), "nor on the next");
    let events = run(&mut m, DT, armed(), &Sill);
    let Some(MountEvent::Entered { approach, side, .. }) = events.first().copied() else { panic!("entered on the second frame: {events:?}") };
    assert_eq!(side, Side::Forward);
    let (data, _) = m.mount_figures().expect("mounted");
    // Back from the point by the pivot's reach and the standing offset.
    assert!((data.target[2] - (data.point.at[2] - 0.69 - 0.074)).abs() < 1e-3, "{data:?}");
    assert!((data.point.at[1] - 1.07).abs() < 1e-3, "the top raised 2 cm: {data:?}");
    // The stance the mount sets: the point's height over the place less the
    // pivot's, as a share of the grid (a level look is the limits' middle).
    let share = ((1.07 + 0.04) - 0.81) / (1.4 - 0.81);
    assert!((m.pose_level - share).abs() < 1e-3, "{} against {share}", m.pose_level);
    run(&mut m, approach + 0.1, armed(), &Sill);
    let view = m.mount_view().expect("still mounted");
    assert!(!view.moving && view.progress >= 1.0, "{view:?}");
    let gone = ((m.position[0] - data.target[0]).powi(2) + (m.position[2] - data.target[2]).powi(2)).sqrt();
    assert!(gone < 1e-4, "the feet put on the target place: {:?} {:?}", m.position, data.target);
}

#[test]
fn held_the_stance_follows_the_pitch_and_the_lean_the_yaw() {
    let Some(mut m) = movement() else { return };
    run(&mut m, 0.5, armed(), &Sill);
    press(&mut m, &Sill);
    run(&mut m, 1.2, armed(), &Sill);
    let (data, _) = m.mount_figures().expect("mounted");
    let level = m.pose_level;
    // Looking down 10 degrees raises the body by a fifth of a unit.
    run(&mut m, 0.5, Input { look_delta: [0.0, 10.0 * DT / 0.5], ..armed() }, &Sill);
    assert!((m.pitch - 10.0).abs() < 0.05, "{}", m.pitch);
    assert!((m.pose_level - (level + 10.0 / 50.0)).abs() < 2e-3, "{} from {level}", m.pose_level);
    // Turning right by half the half-span leans left by 2.5.
    let half = (data.yaw_limit[1] - data.yaw_limit[0]) / 2.0;
    run(&mut m, 0.5, Input { look_delta: [half / 2.0 * DT / 0.5, 0.0], ..armed() }, &Sill);
    assert!((m.tilt + 2.5).abs() < 0.05, "{}", m.tilt);
    // And no further than the limit, however far it is turned.
    run(&mut m, 1.0, Input { look_delta: [0.9, 0.0], ..armed() }, &Sill);
    assert!((m.yaw - data.yaw_limit[1]).abs() < 1e-3, "{} against {:?}", m.yaw, data.yaw_limit);
    assert!((m.tilt + 5.0).abs() < 1e-3, "{}", m.tilt);
}

/// The pivot is put on the point as the rig was turned the frame before
/// (`MountingRotationFunction`'s `Weapon_root.TransformPoint`): on a frame
/// that turns the view it is off the point by that frame's turn of its
/// reach, and back on it on the next frame that does not turn (recorded,
/// 10.4 mm a degree at the M4A1's 0.61 m: mounting spec, "The weapon's
/// place").
#[test]
fn the_pivot_is_turned_by_the_frame_before_s_turn() {
    use trp_sim::base::math::quat_rotate;
    let Some(mut m) = movement() else { return };
    run(&mut m, 0.5, armed(), &Sill);
    press(&mut m, &Sill);
    run(&mut m, 1.2, armed(), &Sill);
    // In the rig's frame (+z ahead): 0.69 m ahead, 4 cm down.
    let pivot = [0.0, -0.04, 0.69];
    let pivot_at = |m: &Movement| -> [f32; 3] {
        let (origin, turn) = m.rig_frame();
        let apart = m.mounted_rig_apart(pivot);
        let reach = quat_rotate(turn, std::array::from_fn(|k| apart[k] + pivot[k]));
        std::array::from_fn(|k| origin[k] + reach[k])
    };
    let view = m.mount_view().expect("mounted");
    let point = view.point;
    let off = |a: [f32; 3]| -> [f32; 3] { std::array::from_fn(|k| a[k] - point[k]) };
    let size = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    // Held still: on the point, though the share has passed 1 on the
    // approach's last frame: the rig's move takes it held to 1
    // (`Vector3.Lerp`; recorded on the M700's bipod, its camera on the
    // mount's place to 0.1 mm at a share of 1.01).
    let still = off(pivot_at(&m));
    assert!(view.progress > 1.0 && size(still) < 1e-4, "{still:?} at {view:?}");
    // One frame turning a degree to the right: off further by the frame's
    // turn of the reach.
    let yaw = m.yaw;
    m.step(DT, &Input { look_delta: [1.0, 0.0], ..armed() }, &Sill);
    let (before, (_, turn)) = (m.rig_turn_before(), m.rig_frame());
    let (now, then) = (quat_rotate(turn, pivot), quat_rotate(before, pivot));
    let turned: [f32; 3] = std::array::from_fn(|k| off(pivot_at(&m))[k] - still[k]);
    let expected = size(std::array::from_fn(|k| now[k] - then[k]));
    // The reach (0.69 m) turned by the frame's turn.
    let angle = (m.yaw - yaw).abs().to_radians();
    assert!(angle > 0.0 && (expected - 0.69f32.hypot(0.04) * angle).abs() < 1e-4, "{expected} for {angle} rad");
    assert!((size(turned) - expected).abs() < 2e-4, "off by the frame's turn of the reach: {turned:?} against {expected}");
    // The next frame, still: back where it was held.
    m.step(DT, &armed(), &Sill);
    let back = off(pivot_at(&m));
    assert!(size(std::array::from_fn(|k| back[k] - still[k])) < 1e-3, "back: {back:?} against {still:?}");
}

#[test]
fn a_walk_key_leaves_in_two_tenths_and_lying_down_at_once() {
    let Some(mut m) = movement() else { return };
    run(&mut m, 0.5, armed(), &Sill);
    press(&mut m, &Sill);
    run(&mut m, 1.2, armed(), &Sill);
    let level = m.pose_level;
    let events = run(&mut m, DT, Input { move_dir: [0.0, -1.0], ..armed() }, &Sill);
    assert_eq!(events, vec![MountEvent::Leaving { side: Side::Forward }]);
    let events = run(&mut m, 0.25, armed(), &Sill);
    assert_eq!(events, vec![MountEvent::Left { side: Side::Forward }]);
    assert!(!m.mounted());
    assert!((m.pose_level - level).abs() < 1e-6, "the stance is not given back");
    // Lying down from a mount leaves it on the frame of the key.
    press(&mut m, &Sill);
    run(&mut m, 1.2, armed(), &Sill);
    assert!(m.mounted());
    let events = run(&mut m, DT, Input { prone: true, ..armed() }, &Sill);
    assert_eq!(events, vec![MountEvent::Left { side: Side::Forward }]);
}

#[test]
fn open_ground_answers_nothing_and_a_locked_walk_leaves_at_once() {
    let Some(mut m) = movement() else { return };
    run(&mut m, 0.5, armed(), &Flat);
    press(&mut m, &Flat);
    let events = run(&mut m, 3.0 * DT, armed(), &Flat);
    assert_eq!(events, vec![MountEvent::Refused], "nothing on the third frame");
    // Locked against the sill (walked into it), the approach cannot walk.
    m.set_blocked([0.0, 1.0]);
    press(&mut m, &Sill);
    let events = run(&mut m, 2.0 * DT, armed(), &Sill);
    assert!(matches!(events[..], [MountEvent::Entered { .. }, MountEvent::Left { .. }]), "{events:?}");
    assert!(!m.mounted());
}

#[test]
fn the_bipod_s_gesture_passes_its_mark() {
    let Some(mut m) = movement() else { return };
    run(&mut m, 0.5, armed(), &Flat);
    assert!(m.gesture_free());
    assert!(m.bipod_gesture(true, false), "the backward bipod's gesture plays");
    assert_eq!(m.shown_interaction(), Some(33));
    let mut toggled = None;
    for frame in 0..120 {
        m.step(DT, &armed(), &Flat);
        if m.take_acted().contains(&Acted::BipodToggled) {
            toggled = Some(frame as f32 * DT);
            break;
        }
    }
    // Recorded on the M60E6: the operation back to idling 0.567 s after the key.
    let at = toggled.expect("the mark passed");
    assert!((at - 0.567).abs() < 0.05, "{at}");
}

/// The recorded M700 build of `m700-bipod`: the Atlas V8 on the Hogue
/// stock's `mod_tactical`.
const M700_ATLAS: &str = "5bfea6e90db834001b7347f3[mod_stock=5bfeb32b0db834001a6694d9[mod_tactical=6644920d49817dc7d505ca71[]]]";

/// The angle in degrees between two turns, taken from the sine of the half
/// angle (exact near nought, where the cosine of a dot is not in `f32`).
fn turn_between(a: [f32; 4], b: [f32; 4]) -> f32 {
    let q = trp_sim::base::math::quat_mul([-a[0], -a[1], -a[2], a[3]], b);
    2.0 * (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt().min(1.0).asin().to_degrees()
}

/// The nodes of a weapon's tree below `top` (not it) whose name ends so.
fn below_named(weapon: &trp_data::weapons::weapon::Weapon, top: usize, end: &str) -> Vec<usize> {
    let tree = &weapon.hierarchy;
    let root = format!("{}/", tree.path(top));
    (0..tree.nodes.len()).filter(|&i| tree.path(i).starts_with(&root) && tree.nodes[i].name.ends_with(end)).collect()
}

/// The bipod's `_procedural` nodes (spec, "The bipod"): no clip moves them,
/// so on every frame of the Atlas's legs going out (1.5 s, through
/// `BIPOD_OPEN` into `BIPOD_IDLE_OPEN`) and folding again they stand at the
/// prefab's rest in the pose given out, while the joints inside them move:
/// out, the left axis turned a quarter and the left leg slid 6 cm.
#[test]
fn the_bipod_s_procedural_nodes_stay_at_rest() {
    use trp_sim::weapons::weapon_animator::{WeaponAnimator, WeaponAnimatorData};
    let Some(install) = crate::fixture::install() else { return };
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble_key(&mut assets, db, M700_ATLAS).expect("the M700 with the Atlas");
    let bipod = weapon.bipod.as_ref().expect("its bipod");
    assert_eq!(bipod.template, "6644920d49817dc7d505ca71");
    let tree = &weapon.hierarchy;
    let procedural = below_named(&weapon, bipod.node, "_procedural");
    assert_eq!(procedural.len(), 6, "the Atlas's six");
    let named = |name: &str| *below_named(&weapon, bipod.node, name).iter().find(|&&i| tree.nodes[i].name == name).expect(name);
    let (axis, leg) = (named("mod_bipod_axis_left"), named("mod_bipod_leg_left"));
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its animator");
    let mut animator = WeaponAnimator::new(&data);
    let mut frames = 0;
    for (out, seconds) in [(true, 1.5f32), (false, 1.5)] {
        animator.set_flag("Bipod", out);
        for _ in 0..(seconds / DT).round() as usize {
            animator.update(DT);
            for &i in &procedural {
                let (at, turn) = animator.pose()[i];
                let rest = &tree.nodes[i];
                let off = (0..3).map(|k| (at[k] - rest.translation[k]).abs()).chain((0..4).map(|k| (turn[k] - rest.rotation[k]).abs())).fold(0.0f32, f32::max);
                assert!(off < 1e-6, "{} off its rest by {off} (bipod {out}): {at:?} {turn:?}", rest.name);
            }
            frames += 1;
        }
        let swung = turn_between(animator.pose()[axis].1, tree.nodes[axis].rotation);
        let (at, rest) = (animator.pose()[leg].0, tree.nodes[leg].translation);
        let slid = ((0..3).map(|k| (at[k] - rest[k]).powi(2)).sum::<f32>()).sqrt();
        println!("bipod {out}: the left axis {swung:.1} deg and the left leg {slid:.4} m from their rest");
        if out {
            assert!(swung > 45.0 && slid > 0.03, "the legs went out: the axis {swung} deg, the leg {slid} m from their rest");
        }
    }
    assert_eq!(frames, 180);
}

/// The RPD's own clips are the only hands' clips that write a bipod's nodes
/// (spec, "The bipod"), and they write its rest: with the hands' animator
/// alone (the bipod's own left out), each of the bipod's `_procedural`
/// nodes, carried up to the slot that holds the bipod, stands where the
/// prefab's rest puts it, within 0.1 mm and 0.01 deg, though the clips set
/// the bipod's root and first node otherwise (the root unturned).
#[test]
fn the_rpd_s_hands_hold_its_bipod_at_rest() {
    use trp_sim::base::math::{quat_mul, quat_rotate};
    use trp_sim::weapons::weapon_animator::{WeaponAnimator, WeaponAnimatorData};
    let Some(install) = crate::fixture::install() else { return };
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let mut weapon = trp_data::weapons::weapon::assemble_key(&mut assets, db, "6513ef33e06849f06c0957ca").expect("the RPD's stock build");
    let top = weapon.bipod.take().expect("its bipod").node;
    let procedural = below_named(&weapon, top, "_procedural");
    assert_eq!(procedural.len(), 4, "the RPD bipod's four");
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its hands' animator");
    let mut animator = WeaponAnimator::new(&data);
    for _ in 0..30 {
        animator.update(DT);
    }
    let tree = &weapon.hierarchy;
    let rest: Vec<([f32; 3], [f32; 4])> = tree.nodes.iter().map(|node| (node.translation, node.rotation)).collect();
    // A node's place and turn in the frame of the slot holding `top`.
    let in_slot = |pose: &[([f32; 3], [f32; 4])], node: usize| {
        let mut chain = vec![node];
        while *chain.last().unwrap() != top {
            chain.push(tree.nodes[*chain.last().unwrap()].parent.expect("below the bipod's root"));
        }
        chain.iter().rev().fold(([0.0f32; 3], [0.0, 0.0, 0.0, 1.0f32]), |(at, turn), &i| {
            let (local_at, local_turn) = pose[i];
            let moved = quat_rotate(turn, local_at);
            ([at[0] + moved[0], at[1] + moved[1], at[2] + moved[2]], quat_mul(turn, local_turn))
        })
    };
    let pose = animator.pose();
    let root_turn = turn_between(pose[top].1, rest[top].1);
    println!("the bipod's root: {root_turn:.2} deg from its rest in the hands' pose");
    assert!(root_turn > 45.0, "the hands' clips write the bipod's root (unturned): {root_turn} deg from its rest");
    for &i in &procedural {
        let (held, rested) = (in_slot(pose, i), in_slot(&rest, i));
        let off = (0..3).map(|k| (held.0[k] - rested.0[k]).abs()).fold(0.0f32, f32::max);
        let turned = turn_between(held.1, rested.1);
        println!("{}: {off:.6} m, {turned:.4} deg from the prefab's rest", tree.nodes[i].name);
        assert!(off < 1e-4 && turned < 0.01, "{} held {off} m, {turned} deg from its rest", tree.nodes[i].name);
    }
}
