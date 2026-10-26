//! Mounting (`docs/spec/mounting.md`) with the install's body animator, on a
//! sill set before the character: the search's frames, the approach to the
//! target place, the stance by the pitch and the lean by the yaw while held,
//! the exits with and without the 0.2 s, the walk's lock, and the bipod's
//! gesture and its mark. These pass trivially, with a note, where no install
//! is configured.

use trp_data::engine::EngineSettings;
use trp_data::hard_settings::HardSettings;
use trp_data::unity::Assets;
use trp_sim::collision::Block;
use trp_sim::mounting::{Hit, Layer, MountEvent, Side, WeaponMount};
use trp_sim::{Acted, Flat, Input, Movement, World};

const DT: f32 = 1.0 / 60.0;

fn movement() -> Option<Movement> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let hard = HardSettings::load(&assets.install).expect("settings object");
    let engine = EngineSettings::load(&assets.install).expect("engine settings");
    let settings = trp_sim::load_movement_settings(&mut assets, db, &hard, &engine, 0.0).expect("movement settings");
    let body = trp_sim::body::BodyData::load(&mut assets).expect("body animator");
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
    use trp_sim::math::quat_rotate;
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
    // Held still: off the point only by the share's overshoot past 1 (the
    // game's too: recorded 1.0055), along the line from the ribcage.
    let still = off(pivot_at(&m));
    assert!(view.progress > 1.0 && size(still) < 0.01, "{still:?} at {view:?}");
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
