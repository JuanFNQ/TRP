//! The arms' hold on the weapon: the limb solver as the game runs it, with
//! no engine types. See `docs/spec/hands-on-weapon.md`, "The limb solver,
//! as the game runs it".
//!
//! Everything is in one frame, the one the weapon's rig hangs in: at the
//! body's ribcage, turned to where the player looks, at true size.

use anyhow::{bail, Result};
use trp_data::player::{LimbData, TwistData};

use crate::body::ArmPose;
use crate::math::{inverse_lerp, quat_lerp, quat_look, quat_mul, quat_normalize, quat_rotate, quat_slerp};

/// A bone's place and turning, in the common frame or in its parent.
pub type Placed = ([f32; 3], [f32; 4]);

/// The bend rule the solver here follows: toward a goal.
const BEND_BY_GOAL: i64 = 4;
/// How far a hand's marker may be from its fixed reference on the weapon
/// before the hand counts as off the part's grip, in metres.
const GRIP_RELEASE_DISTANCE: f32 = 0.1;

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn length(v: [f32; 3]) -> f32 {
    dot(v, v).sqrt()
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

fn inverse(q: [f32; 4]) -> [f32; 4] {
    [-q[0], -q[1], -q[2], q[3]]
}

/// `Quaternion.AngleAxis`.
fn about(axis: [f32; 3], degrees: f32) -> [f32; 4] {
    let size = length(axis);
    if size < 1e-9 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let (sin, cos) = (degrees.to_radians() * 0.5).sin_cos();
    [axis[0] / size * sin, axis[1] / size * sin, axis[2] / size * sin, cos]
}

/// `Vector3.Slerp` between two directions of the same length.
fn swing(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    let (la, lb) = (length(a), length(b));
    if la < 1e-9 || lb < 1e-9 {
        return mix(a, b, t);
    }
    let angle = (dot(a, b) / la / lb).clamp(-1.0, 1.0).acos();
    let sin = angle.sin();
    if sin.abs() < 1e-6 {
        return mix(a, b, t);
    }
    let (from, to) = (((1.0 - t) * angle).sin() / sin, (t * angle).sin() / sin);
    std::array::from_fn(|i| a[i] * from + b[i] * to)
}

/// An arm as the solver finds it: the upper arm placed, and the three
/// forearm bones and the palm each in its parent.
#[derive(Debug, Clone, Copy)]
pub struct Arm {
    pub upper: Placed,
    pub below: [Placed; 4],
}

impl Arm {
    /// The five bones in the common frame: the upper arm, the forearm's
    /// three, the palm.
    pub fn placed(&self) -> [Placed; 5] {
        let mut out = [self.upper; 5];
        for i in 0..4 {
            let (at, turn) = out[i];
            let (offset, local) = self.below[i];
            out[i + 1] = (add(at, quat_rotate(turn, offset)), quat_mul(turn, local));
        }
        out
    }

    /// Turns one of the bones below the upper arm to `rotation` in the
    /// common frame, whatever hangs from it turning with it.
    fn turn(&mut self, bone: usize, rotation: [f32; 4]) {
        let parent = self.placed()[bone - 1].1;
        self.below[bone - 1].1 = quat_normalize(quat_mul(inverse(parent), rotation));
    }
}

/// Refuses a solver set up in a way this does not follow.
pub fn check(limb: &LimbData) -> Result<()> {
    if limb.bend_modifier != BEND_BY_GOAL || limb.bend_weight < 1.0 || limb.maintain_rotation > 0.0 {
        bail!(
            "the player prefab's limb solver bends by rule {} at weight {} and keeps {} of the palm's turning; only a goal at full weight, keeping none, is implemented",
            limb.bend_modifier,
            limb.bend_weight,
            limb.maintain_rotation
        );
    }
    Ok(())
}

/// One update of the limb solver, then the forearm's twist bones.
///
/// `arm` is the pose found, `target` where the palm is to go and how it is
/// to be turned, `goal` the point the elbow bends toward and `weight` how
/// much of all that is done, 0 to 1. Returns the five bones in the common
/// frame.
pub fn solve(arm: &Arm, target: Placed, goal: [f32; 3], weight: f32, limb: &LimbData) -> [Placed; 5] {
    let mut arm = *arm;
    let weight = weight.clamp(0.0, 1.0);
    if weight > 0.0 {
        reach(&mut arm, target.0, goal, weight);
    }
    // The palm takes the target's turning.
    if weight > 0.0 {
        let palm = arm.placed()[4].1;
        arm.turn(4, quat_slerp(palm, target.1, weight));
    }
    for (index, twist) in limb.twists.iter().enumerate() {
        relax(&mut arm, 2 + index, twist);
    }
    arm.placed()
}

/// Turns the upper arm and the first forearm bone so that the palm comes
/// to `target`, or toward it by `weight`, with the elbow on `goal`'s side.
fn reach(arm: &mut Arm, target: [f32; 3], goal: [f32; 3], weight: f32) {
    let found = arm.placed();
    let ((shoulder, upper), (elbow, fore), palm) = (found[0], found[1], found[4].0);
    let normal = cross(sub(goal, shoulder), sub(target, shoulder));
    let (upper_bone, fore_bone) = (sub(elbow, shoulder), sub(palm, elbow));
    // How each bone is turned against the plane as found.
    let (Some(upper_frame), Some(fore_frame)) = (quat_look(upper_bone, normal), quat_look(fore_bone, normal)) else {
        return;
    };
    let (keep_upper, keep_fore) = (quat_mul(inverse(upper_frame), upper), quat_mul(inverse(fore_frame), fore));

    let wanted = mix(palm, target, weight);
    let to = sub(wanted, shoulder);
    let distance = length(to);
    let (a, b) = (dot(upper_bone, upper_bone), dot(fore_bone, fore_bone));
    // The way to the elbow: in the plane, at the two lengths, on the
    // goal's side.
    let bent = (distance > 0.0)
        .then(|| {
            let along = (distance * distance + a - b) / 2.0 / distance;
            let aside = (a - along * along).max(0.0).sqrt();
            quat_look(to, cross(to, normal)).map(|frame| quat_rotate(frame, [0.0, aside, along]))
        })
        .flatten()
        .unwrap_or([0.0; 3]);
    let mut way = mix(upper_bone, bent, weight);
    if length(way) < 1e-9 {
        way = upper_bone;
    }
    let Some(aimed) = quat_look(way, normal) else { return };
    let new_upper = quat_normalize(quat_mul(aimed, keep_upper));

    // The forearm has moved with the upper arm, and its normal with it.
    let carried = quat_mul(new_upper, inverse(upper));
    let new_elbow = add(shoulder, quat_rotate(carried, upper_bone));
    let Some(aimed) = quat_look(sub(wanted, new_elbow), quat_rotate(carried, normal)) else {
        arm.upper.1 = new_upper;
        return;
    };
    let new_fore = quat_mul(aimed, keep_fore);
    arm.upper.1 = new_upper;
    arm.below[0].1 = quat_normalize(quat_mul(inverse(new_upper), new_fore));
}

/// Turns one of the forearm's twist bones about its own length to lie
/// between the first forearm bone and the palm, and puts the palm back
/// where it pointed.
fn relax(arm: &mut Arm, bone: usize, twist: &TwistData) {
    if twist.weight <= 0.0 {
        return;
    }
    let bones = arm.placed();
    let (fore, own, palm) = (bones[1].1, bones[bone].1, bones[4].1);
    let wanted = swing(quat_rotate(fore, twist.in_parent), quat_rotate(palm, twist.in_child), twist.crossfade);
    let along = quat_rotate(own, twist.twist_axis);
    let Some(frame) = quat_look(quat_rotate(own, twist.axis), along) else { return };
    let seen = quat_rotate(inverse(frame), wanted);
    let angle = seen[0].atan2(seen[2]).to_degrees() * twist.weight;
    arm.turn(bone, quat_mul(about(along, angle), own));
    arm.turn(4, palm);
}

/// What the weapon's rig gives one arm.
#[derive(Debug, Clone, Copy)]
pub struct RigArm {
    /// The rig's own upper arm, as the weapon's clip has it.
    pub upper: Placed,
    /// The marker the weapon's clips move for this hand, and the fixed
    /// point beside it that counts as "on the weapon".
    pub marker: Placed,
    pub reference: [f32; 3],
    /// The fitted part's grip for this hand, if it has one.
    pub grip: Option<Placed>,
    /// The weapon's own point for the elbow to bend toward.
    pub goal: [f32; 3],
}

/// Where a hand goes: its marker, or the part's grip while the marker is
/// on the weapon. Also how far the hand is on that grip, 0 to 1, which is
/// how far its fingers take the grip's pose.
pub fn hand_target(rig: &RigArm) -> (Placed, f32) {
    let Some(grip) = rig.grip else { return (rig.marker, 0.0) };
    let away = length(sub(rig.marker.0, rig.reference));
    let on = inverse_lerp(GRIP_RELEASE_DISTANCE, 0.0, away);
    ((mix(rig.marker.0, grip.0, on), quat_lerp(rig.marker.1, grip.1, on)), on)
}

/// One arm for one frame, as the game does it in first person: the
/// shoulder put on the rig's, the hand's target, the elbow's goal, the
/// solver and the twist bones. `body` is the arm as the body's animation
/// has it. Returns the arm's five bones (upper arm, the forearm's three,
/// palm) and how far the hand is on the part's grip.
pub fn hold(body: &ArmPose, rig: &RigArm, limb: &LimbData) -> ([Placed; 5], f32) {
    let weight = hand_weight(body);
    let (target, grip) = if weight > f32::EPSILON { hand_target(rig) } else { (rig.marker, 0.0) };
    (solve(&found(body, rig), target, bend_goal(body, rig), weight, limb), grip)
}

/// The arm as the solver finds it: the body's own, with its upper arm put
/// on the rig's by what the body's animation leaves of the shoulder.
pub fn found(body: &ArmPose, rig: &RigArm) -> Arm {
    let on_rig = (1.0 - body.shoulder).clamp(0.0, 1.0);
    let upper = if on_rig > 0.0 {
        (mix(body.upper.0, rig.upper.0, on_rig), quat_slerp(body.upper.1, rig.upper.1, on_rig))
    } else {
        body.upper
    };
    Arm { upper, below: body.below }
}

/// How much of the solver's work is done for this arm: all of it unless
/// the body's animation has the hand.
pub fn hand_weight(body: &ArmPose) -> f32 {
    (1.0 - body.hand).clamp(0.0, 1.0)
}

/// The point an arm's elbow bends toward: the weapon's goal, or the
/// body's own by as much as the body's animation claims the elbow.
pub fn bend_goal(body: &ArmPose, rig: &RigArm) -> [f32; 3] {
    if body.elbow < 1.0 {
        mix(rig.goal, body.bend_goal, body.elbow.max(0.0))
    } else {
        body.bend_goal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::quat_axis;

    const IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

    fn limb() -> LimbData {
        // Twist bones that do nothing: the solver alone is under test.
        let twist = TwistData {
            weight: 0.0,
            crossfade: 0.0,
            twist_axis: [1.0, 0.0, 0.0],
            axis: [0.0, 1.0, 0.0],
            in_parent: [0.0, 1.0, 0.0],
            in_child: [0.0, 1.0, 0.0],
        };
        LimbData { bend_modifier: BEND_BY_GOAL, bend_weight: 1.0, maintain_rotation: 0.0, twists: [twist; 2] }
    }

    /// A straight arm along -x, as the game's bones run: 0.3 m of upper
    /// arm, then 0.1, 0.1 and 0.08 m of forearm to the palm.
    fn straight() -> Arm {
        let step = |x: f32| ([-x, 0.0, 0.0], IDENTITY);
        Arm { upper: ([0.0; 3], IDENTITY), below: [step(0.3), step(0.1), step(0.1), step(0.08)] }
    }

    #[test]
    fn the_palm_reaches_the_target_with_the_elbow_on_the_goals_side() {
        let arm = straight();
        let target = ([-0.3, 0.0, 0.25], quat_axis(1, 40.0));
        let goal = [-0.2, -0.5, 0.0];
        let solved = solve(&arm, target, goal, 1.0, &limb());
        let (shoulder, elbow, palm) = (solved[0].0, solved[1].0, solved[4].0);
        assert!(length(sub(palm, target.0)) < 1e-4, "{palm:?}");
        assert!((length(sub(elbow, shoulder)) - 0.3).abs() < 1e-4);
        assert!((length(sub(palm, elbow)) - 0.28).abs() < 1e-4);
        // Below the line from shoulder to palm, where the goal is.
        assert!(elbow[1] < -0.05, "{elbow:?}");
        // The palm is turned as the target is.
        let same: f32 = (0..4).map(|i| solved[4].1[i] * target.1[i]).sum();
        assert!(same.abs() > 0.9999, "{:?}", solved[4].1);
    }

    #[test]
    fn at_no_weight_the_arm_is_left_as_found() {
        let arm = straight();
        let solved = solve(&arm, ([-0.3, 0.0, 0.25], IDENTITY), [-0.2, -0.5, 0.0], 0.0, &limb());
        assert_eq!(solved, arm.placed());
    }

    /// The upper arm's turning about its own length comes from the pose
    /// found: found twisted a quarter turn, it comes out a quarter turn
    /// from where an untwisted one does, the elbow in the same place.
    #[test]
    fn the_upper_arm_keeps_the_twist_it_was_found_with() {
        let (target, goal) = (([-0.3, 0.0, 0.25], IDENTITY), [-0.2, -0.5, 0.0]);
        let plain = solve(&straight(), target, goal, 1.0, &limb());
        let mut twisted = straight();
        twisted.upper.1 = quat_axis(0, 90.0);
        // The forearm is turned back so that the arm found is the same line.
        twisted.below[0].1 = quat_axis(0, -90.0);
        let turned = solve(&twisted, target, goal, 1.0, &limb());
        assert!(length(sub(plain[1].0, turned[1].0)) < 1e-4);
        let same: f32 = (0..4).map(|i| plain[0].1[i] * turned[0].1[i]).sum();
        let apart = 2.0 * same.abs().min(1.0).acos().to_degrees();
        assert!((apart - 90.0).abs() < 0.01, "{apart}");
    }

    /// A twist bone half way between a forearm that has not turned and a
    /// palm turned 60 degrees about the arm's length turns 30.
    #[test]
    fn a_twist_bone_goes_part_of_the_way_to_the_palm() {
        let mut arm = straight();
        arm.below[3].1 = quat_axis(0, 60.0);
        let twist = TwistData {
            weight: 1.0,
            crossfade: 0.5,
            twist_axis: [1.0, 0.0, 0.0],
            axis: [0.0, 1.0, 0.0],
            in_parent: [0.0, 1.0, 0.0],
            in_child: quat_rotate(quat_axis(0, -60.0), [0.0, 1.0, 0.0]),
        };
        // At rest, by those stored directions, the palm is the one turned.
        let palm_before = arm.placed()[4].1;
        relax(&mut arm, 2, &twist);
        let bones = arm.placed();
        let up = quat_rotate(bones[2].1, [0.0, 1.0, 0.0]);
        assert!(up[0].abs() < 1e-5 && (up[1] - 1.0).abs() < 1e-5, "at rest nothing turns: {up:?}");
        // Now the palm turns another 60 degrees.
        arm.turn(4, quat_mul(quat_axis(0, 60.0), palm_before));
        relax(&mut arm, 2, &twist);
        let bones = arm.placed();
        let up = quat_rotate(bones[2].1, [0.0, 1.0, 0.0]);
        let turned = up[2].atan2(up[1]).to_degrees();
        assert!((turned - 30.0).abs() < 0.01, "{turned}");
        let same: f32 = (0..4).map(|i| bones[4].1[i] * quat_mul(quat_axis(0, 60.0), palm_before)[i]).sum();
        assert!(same.abs() > 0.9999, "the palm stays where it pointed");
    }
}
