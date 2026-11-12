//! Tests of the character's own collision: walls, boxes, steps, edges and other characters.
use super::*;

const RADIUS: f32 = 0.366;
const STANDING: Capsule = Capsule { underside: 0.0, top: 1.6, lift: true };

/// The yard of the tests below with width to it: ground at 0 and a
/// box a metre high from 2 to 3 m ahead and from 1 m left to 1 m right.
#[test]
fn another_character_standing_pushes_the_capsule_out_sideways() {
    // A character standing 0.5 m ahead (both 0.366 m wide, 1.6 m high):
    // walked into, the capsule is held 0.732 m from its axis, level.
    let other = Upright { feet: [0.0, 0.0, 3.0], height: 1.6, centre: 0.8, radius: RADIUS };
    let site = Site { ground: 0.0, blocks: Vec::new(), others: vec![other] };
    let at = site.settle([0.0, 0.0, 2.2], [0.0, 0.0, 2.5], STANDING, RADIUS);
    assert!((at[2] - (3.0 - 2.0 * RADIUS)).abs() < 1e-5 && at[1].abs() < 1e-6, "{at:?}");
    // Beside it, off its line: out along the line between the two.
    let at = site.settle([0.6, 0.0, 2.75], [0.5, 0.0, 2.8], STANDING, RADIUS);
    let off = ((at[0]).powi(2) + (at[2] - 3.0).powi(2)).sqrt();
    assert!((off - 2.0 * RADIUS).abs() < 1e-4, "{at:?}");
    // Clear of it: not moved.
    assert_eq!(site.settle([2.0, 0.0, 3.0], [2.0, 0.0, 3.0], STANDING, RADIUS), [2.0, 0.0, 3.0]);
    // A line toward it enters it at its side, at its radius; one that
    // starts inside it does not.
    let entered = other.entered([0.0, 1.0, 1.0], [0.0, 1.0, 5.0]).expect("met");
    assert!((entered * 4.0 - (2.0 - RADIUS)).abs() < 1e-5, "{entered}");
    assert_eq!(other.entered([0.0, 1.0, 3.1], [0.0, 1.0, 5.0]), None);
    // Over its head, clear.
    assert_eq!(other.entered([0.0, 2.0, 1.0], [0.0, 2.0, 5.0]), None);
    // Its own move does not meet itself.
    assert_eq!(site.settle([0.0, 0.0, 3.0], [0.0, 0.0, 3.02], STANDING, RADIUS), [0.0, 0.0, 3.02]);
}

fn site() -> Site {
    Site { ground: 0.0, blocks: vec![Block::square([-1.0, 0.0, 2.0], [1.0, 1.0, 3.0])], others: Vec::new() }
}

/// Walked at square, a box stops the capsule its radius from the face
/// and leaves it on the ground, as a line's wall does; walked at
/// aslant, it lets the capsule along its face; and its upright edge
/// is rounded by the capsule's radius.
#[test]
fn a_box_stops_the_capsule_and_lets_it_along_its_face() {
    let site = site();
    let mut at = [0.0, 0.0, 1.0];
    for _ in 0..60 {
        at = site.settle(at, [at[0], at[1], at[2] + 0.025], STANDING, RADIUS);
    }
    assert!((at[2] - (2.0 - RADIUS)).abs() < 1e-4 && at[0] == 0.0 && at[1] == 0.0, "{at:?}");
    assert!(!site.held(at, STANDING, RADIUS));
    for _ in 0..30 {
        at = site.settle(at, [at[0] + 0.02, at[1], at[2] + 0.02], STANDING, RADIUS);
    }
    assert!((at[0] - 0.6).abs() < 1e-4 && (at[2] - (2.0 - RADIUS)).abs() < 1e-4, "{at:?}");
    // On past the box's edge it comes round the corner, never nearer
    // the edge than its radius.
    let mut nearest = f32::MAX;
    for _ in 0..90 {
        at = site.settle(at, [at[0] + 0.02, at[1], at[2] + 0.02], STANDING, RADIUS);
        nearest = nearest.min(((at[0] - 1.0).powi(2) + (at[2] - 2.0).powi(2)).sqrt());
    }
    assert!(at[2] > 2.5 && at[0] > 1.0 + RADIUS - 1e-4 && nearest > RADIUS - 1e-4, "{at:?}, {nearest}");
}

/// The line's rules hold for a box: a low one is ridden up, a rim
/// holds a foot that overhangs it, and a capsule deep in a box comes
/// out a twentieth of a metre a move, by its nearest side.
#[test]
fn a_box_is_ridden_up_held_onto_and_come_out_of() {
    let low = Site { ground: 0.0, blocks: vec![Block::square([-1.0, 0.0, 2.0], [1.0, 0.1, 9.0])], others: Vec::new() };
    let mut at = [0.0, 0.0, 1.0];
    for _ in 0..80 {
        at = low.settle(at, [at[0], at[1], at[2] + 0.025], STANDING, RADIUS);
    }
    assert!(at[2] > 2.5 && (at[1] - 0.1).abs() < 2e-3, "{at:?}");

    let site = site();
    let rest = |out: f32| 1.0 - (RADIUS - (RADIUS * RADIUS - out * out).sqrt());
    assert!(site.held([0.0, rest(0.1), 3.1], STANDING, RADIUS));
    assert!(site.held([1.1, rest(0.1), 2.5], STANDING, RADIUS));
    assert!(!site.held([0.0, rest(0.1) + 0.01, 3.1], STANDING, RADIUS));
    assert!(!site.held([0.0, 0.9, 3.5], STANDING, RADIUS));

    let at = site.settle([0.0, 0.0, 3.05], [0.0, 0.0, 3.05], STANDING, RADIUS);
    assert!((at[2] - 3.10).abs() < 1e-5 && at[0] == 0.0 && at[1] == 0.0, "{at:?}");
    let at = site.settle([1.05, 0.0, 2.5], [1.05, 0.0, 2.5], STANDING, RADIUS);
    assert!((at[0] - 1.10).abs() < 1e-5 && at[2] == 2.5 && at[1] == 0.0, "{at:?}");
    // In a climb the upward part of every way out is dropped.
    let climbing = Capsule { underside: 0.7, top: 1.6, lift: false };
    assert_eq!(site.settle([0.0, 0.0, 1.95], [0.0, 0.0, 1.95], climbing, RADIUS), [0.0, 0.0, 1.95]);
    // A foot that had sunk into the top under it is put back on that
    // top and its move dropped; under a top over its head it is not.
    let at = site.settle([0.0, 0.99, 2.5], [0.0, 0.99, 2.6], Capsule { lift: false, ..STANDING }, RADIUS);
    assert_eq!(at, [0.0, 1.0, 2.5]);
}

/// Ground at 0 with a box a metre high from 2 to 3 m out.
fn yard() -> Course {
    Course { treads: vec![Tread::level(f32::MIN, 2.0, 0.0), Tread::level(2.0, 3.0, 1.0), Tread::level(3.0, f32::MAX, 0.0)] }
}

/// A path with a step in it splits into pieces at the step.
#[test]
fn a_path_is_cut_at_its_steps() {
    let course = Course::from_path(&[[0.0, 0.0], [1.0, 0.02], [1.96, 0.02], [2.04, 1.0], [2.96, 1.0], [3.04, 0.0], [4.0, 0.0]]);
    assert_eq!(course.treads.len(), 3);
    assert!((course.treads[0].far - 2.0).abs() < 1e-5 && (course.treads[1].far - 3.0).abs() < 1e-5);
    assert!((course.height(0.5) - 0.01).abs() < 1e-5 && course.height(2.5) == 1.0 && course.height(3.5) == 0.0);
}

/// A walk into a wall stops with the axis the capsule's radius from
/// its face, and stays on the ground.
#[test]
fn a_wall_stops_the_capsule_at_its_radius() {
    let course = yard();
    let mut at = [1.0, 0.0];
    for _ in 0..60 {
        at = course.settle(at, [at[0] + 0.025, at[1]], STANDING, RADIUS);
    }
    assert!((at[0] - (2.0 - RADIUS)).abs() < 1e-4 && at[1] == 0.0, "{at:?}");
    assert!(!course.held(at, STANDING, RADIUS));
}

/// A step lower than a third of the radius or so is ridden up as it
/// is walked into: its corner pushes mostly upward.
#[test]
fn a_low_step_is_ridden_up() {
    let course = Course { treads: vec![Tread::level(f32::MIN, 2.0, 0.0), Tread::level(2.0, f32::MAX, 0.1)] };
    let mut at = [1.0, 0.0];
    for _ in 0..80 {
        at = course.settle(at, [at[0] + 0.025, at[1]], STANDING, RADIUS);
    }
    assert!(at[0] > 2.5 && (at[1] - 0.1).abs() < 2e-3, "{at:?}");
}

/// On a top, with the axis past the edge by less than the radius, the
/// corner holds the capsule; further out it does not.
#[test]
fn an_edge_holds_a_foot_that_overhangs_it() {
    let course = yard();
    let rest = |out: f32| 1.0 - (RADIUS - (RADIUS * RADIUS - out * out).sqrt());
    assert!(course.held([3.1, rest(0.1)], STANDING, RADIUS));
    assert!(!course.held([3.1, rest(0.1) + 0.01], STANDING, RADIUS));
    // Sunk a little into the corner, it is put back on it: straight
    // up, since the corner is nearly under it.
    let at = course.settle([3.1, rest(0.1)], [3.1, rest(0.1) - 0.01], STANDING, RADIUS);
    assert!(at[0] == 3.1 && (at[1] - rest(0.1)).abs() < 2e-3, "{at:?}");
    assert!(!course.held([3.5, 0.9], STANDING, RADIUS));
}

/// A capsule put down well inside the box's reach comes out a
/// twentieth of a metre a move, and with no lift only sideways.
#[test]
fn a_capsule_deep_in_something_comes_out_by_steps() {
    let course = yard();
    let at = course.settle([3.05, 0.0], [3.05, 0.0], STANDING, RADIUS);
    assert!((at[0] - 3.10).abs() < 1e-5 && at[1] == 0.0, "{at:?}");
    // In a climb the corner of the top, once the foot is over it,
    // pushes back by the level part of the way out alone, and not at
    // all once it is mostly under the foot.
    let climbing = Capsule { underside: 0.7, top: 1.6, lift: false };
    let beside = course.settle([1.75, 0.0], [1.75, 0.0], climbing, RADIUS);
    assert!(beside[0] < 1.75 && beside[1] == 0.0, "{beside:?}");
    let over = course.settle([1.95, 0.0], [1.95, 0.0], climbing, RADIUS);
    assert_eq!(over, [1.95, 0.0]);
}

/// A box turned about the upright: a door's leaf 0.9 m wide and 4 cm
/// thick, lying along z, stops a capsule walked into it from the side
/// its radius off its face; turned an eighth, it is met at its turned
/// face and lets the capsule slide off it.
#[test]
fn a_turned_box_stops_the_capsule_at_its_face() {
    let leaf = Block::turned([0.0, 0.0, 3.0], [0.9, 2.0, 0.04], [0.0, 0.0, 1.0]);
    assert!(leaf.covers(0.0, 3.4) && !leaf.covers(0.0, 3.5) && !leaf.covers(0.1, 3.0));
    let site = Site { ground: 0.0, blocks: vec![leaf], others: Vec::new() };
    let mut at = [-2.0, 0.0, 3.0];
    for _ in 0..200 {
        at = site.settle(at, [at[0] + 0.0235, at[1], at[2]], STANDING, RADIUS);
    }
    assert!((at[0] - (-0.02 - RADIUS)).abs() < 1e-4 && (at[2] - 3.0).abs() < 1e-4, "{at:?}");
    // An eighth turned: the face's normal points back along x and on
    // along z, so a walk along x slides along it towards +z.
    let along = [std::f32::consts::FRAC_1_SQRT_2, 0.0, std::f32::consts::FRAC_1_SQRT_2];
    let site = Site { ground: 0.0, blocks: vec![Block::turned([0.0, 0.0, 3.0], [2.0, 2.0, 0.04], along)], others: Vec::new() };
    let mut at = [-2.0, 0.0, 3.0];
    for _ in 0..100 {
        at = site.settle(at, [at[0] + 0.0235, at[1], at[2]], STANDING, RADIUS);
    }
    let off_face = (at[0] * along[2] - (at[2] - 3.0) * along[0]).abs();
    assert!((off_face - (0.02 + RADIUS)).abs() < 2e-3 && at[2] > 3.3, "{at:?} is {off_face} off the face");
    assert!(site.blocks[0].overlaps([0.0, 0.0, 3.0], STANDING, RADIUS));
}
