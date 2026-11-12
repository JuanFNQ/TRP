//! The puppet's pose, how it is posed and the bound around its colliders.

use super::*;

/// One arm of the rig: what its solver reads, the palm's bone, the bones
/// below the palm in the rig's order, and the finger pose of the grip it
/// takes, if any (the grip's own turn first).
pub(super) struct RigHand {
    pub(super) nodes: ArmNodes,
    pub(super) fingers: Vec<usize>,
    pub(super) grip_pose: Vec<[f32; 4]>,
}

/// What the character leaves of its last frame, for drawing, in the game's
/// axes.
#[derive(Debug, Clone)]
pub struct PuppetPose {
    /// The feet as drawn, and the view's yaw and pitch.
    pub position: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    /// The body: where its skeleton hangs (the feet lifted by the lag in
    /// the body's height) and its turn, and each bone the animator gives in
    /// its parent (`Body::drawn_bones`).
    pub body_at: [f32; 3],
    pub body_turn: [f32; 4],
    pub bones: Vec<Placed>,
    /// The ribcage from the feet, in the frame turned with the view: where
    /// the rig hangs, turned by the view's pitch.
    pub ribcage: [f32; 3],
    /// Every bone of the rig in its parent, the moved bone where the
    /// weapon's motion and the body put it, the fingers on the grip.
    pub rig: Vec<Placed>,
    /// Each arm as its solver leaves it, right then left: the upper arm,
    /// the three forearm bones and the palm, in the rig's frame. Solved on
    /// the rig as drawn (`SMOOTH ANIMATIONS`: the pose for drawing).
    pub arms: [[Placed; 5]; 2],
    /// The same arms solved on the rig as the animator's straight pose has
    /// it, which the hit colliders go by (a drawing option moves no hit). The same as `arms` without
    /// smoothing.
    pub straight_arms: [[Placed; 5]; 2],
}

impl PuppetPose {
    /// The pose `t` of the way from this one to `to` (0 this one, 1 `to`),
    /// as the app draws the player's body, rig and arms between two frames
    /// (`Controller::shown`, `body_shown`, `rig_shown`): each place on the
    /// straight line, each turn by Unity's `Quaternion.Lerp`, the view's
    /// yaw the shorter way round. Across a change of what the hands hold
    /// (a rig of other bones, as the player's `rig_shown` tells it), or a
    /// body of other bones, `to` alone: nothing is drawn between them.
    pub fn blend(&self, to: &PuppetPose, t: f32) -> PuppetPose {
        if self.rig.len() != to.rig.len() || self.bones.len() != to.bones.len() || t >= 1.0 {
            return to.clone();
        }
        if t <= 0.0 {
            return self.clone();
        }
        let lerp = |a: f32, b: f32| a + (b - a) * t;
        let lerp3 = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] { std::array::from_fn(|k| lerp(a[k], b[k])) };
        let placed = |a: &Placed, b: &Placed| -> Placed { (lerp3(a.0, b.0), quat_lerp(a.1, b.1, t)) };
        let all = |a: &[Placed], b: &[Placed]| -> Vec<Placed> { a.iter().zip(b).map(|(a, b)| placed(a, b)).collect() };
        let arms = |a: &[[Placed; 5]; 2], b: &[[Placed; 5]; 2]| -> [[Placed; 5]; 2] { std::array::from_fn(|side| std::array::from_fn(|k| placed(&a[side][k], &b[side][k]))) };
        PuppetPose {
            position: lerp3(self.position, to.position),
            yaw: lerp_angle(self.yaw, to.yaw, t),
            pitch: lerp(self.pitch, to.pitch),
            body_at: lerp3(self.body_at, to.body_at),
            body_turn: quat_lerp(self.body_turn, to.body_turn, t),
            bones: all(&self.bones, &to.bones),
            ribcage: lerp3(self.ribcage, to.ribcage),
            rig: all(&self.rig, &to.rig),
            arms: arms(&self.arms, &to.arms),
            straight_arms: arms(&self.straight_arms, &to.straight_arms),
        }
    }
}

/// How the host has a character posed (`Puppet::posing`), told before each
/// step: TRP's own, the game's posing by distance not built (third-person
/// spec, "Posing by distance and off screen").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Posing {
    /// Every frame's pose made as the frame ends: its
    /// animators work out every bone at each update.
    #[default]
    Full,
    /// A frame's pose made only when asked for (`Puppet::pose`,
    /// `pose_of_frame`, `drawn`), from its recipe, and kept with it: its
    /// animators work out only the bones the simulation goes by
    /// (`Animator::set_deferred`).
    Asked,
}

/// A capsule that holds every hit collider of the character in the pose
/// its frame makes, whatever that pose (a host's broad test before it asks
/// for one: a bullet's path, a blade's sweep, a blast), in the game's axes:
/// the axis from the pelvis to the ribcage as the frame left them, and a
/// radius of the farthest a limb reaches from either end at rest
/// (`Body::reach`) with `BOUND_MARGIN`. Made in the simulation's half, so
/// that it costs no pose.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bound {
    pub ends: [[f32; 3]; 2],
    pub radius: f32,
}

impl Bound {
    /// The distance from a point to the capsule's axis.
    pub fn distance_to_axis(&self, point: [f32; 3]) -> f32 {
        let [a, b] = self.ends;
        let ab: [f32; 3] = std::array::from_fn(|k| b[k] - a[k]);
        let ap: [f32; 3] = std::array::from_fn(|k| point[k] - a[k]);
        let length = dot(ab, ab);
        let t = if length > 0.0 { (dot(ap, ab) / length).clamp(0.0, 1.0) } else { 0.0 };
        let near: [f32; 3] = std::array::from_fn(|k| a[k] + ab[k] * t - point[k]);
        dot(near, near).sqrt()
    }

    /// Whether a ball at `point` of `radius` lies wholly inside.
    pub fn holds(&self, point: [f32; 3], radius: f32) -> bool {
        self.distance_to_axis(point) + radius <= self.radius
    }

    /// Whether the segment from `from` to `to` comes within the capsule: a
    /// bullet's path over a step, a blade's sweep. The nearest two points of
    /// the segment and the axis (Ericson's closest points of two segments).
    pub fn meets(&self, from: [f32; 3], to: [f32; 3]) -> bool {
        segments_apart(from, to, self.ends[0], self.ends[1]) <= self.radius
    }
}

pub(super) fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The least distance between the segments `p1`-`q1` and `p2`-`q2`
/// (Ericson, "Real-Time Collision Detection", 5.1.9).
fn segments_apart(p1: [f32; 3], q1: [f32; 3], p2: [f32; 3], q2: [f32; 3]) -> f32 {
    let d1: [f32; 3] = std::array::from_fn(|k| q1[k] - p1[k]);
    let d2: [f32; 3] = std::array::from_fn(|k| q2[k] - p2[k]);
    let r: [f32; 3] = std::array::from_fn(|k| p1[k] - p2[k]);
    let (a, e, f) = (dot(d1, d1), dot(d2, d2), dot(d2, r));
    let (s, t) = if a <= f32::EPSILON && e <= f32::EPSILON {
        (0.0, 0.0)
    } else if a <= f32::EPSILON {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = dot(d1, r);
        if e <= f32::EPSILON {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = dot(d1, d2);
            let denom = a * e - b * b;
            let mut s = if denom > 0.0 { ((b * f - c * e) / denom).clamp(0.0, 1.0) } else { 0.0 };
            let mut t = (b * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            }
            (s, t)
        }
    };
    let between: [f32; 3] = std::array::from_fn(|k| (p1[k] + d1[k] * s) - (p2[k] + d2[k] * t));
    dot(between, between).sqrt()
}
