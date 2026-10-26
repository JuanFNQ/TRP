//! The head's push out of walls (movement spec, "The head's push out of
//! walls"): the capsule about the head that the player prefab's
//! `PlayerOverlapManager` keeps, standing on the camera's container and
//! lying over the second spine bone; how far a frame's motion would take it
//! into something, which pushes the motion back on the level
//! (`MovementContext.LimitMotionXZ`); and whether a turn would swing it into
//! something (`RotationOverlapPrediction`), which a lying body's turn and a
//! mounted weapon's turn are refused by.

use trp_data::player::HeadCapsuleData;

use super::{Movement, World};
use crate::math::{quat_axis, quat_euler, quat_mul, quat_rotate, quat_to_euler};

/// What the game adds to each push, in metres
/// (`ColliderExtrusion.CalculateDepenetration`).
pub const MARGIN: f32 = 0.001;

/// A level push whose square is under this is let be (`LimitMotionXZ`'s
/// threshold); lying still, under the larger figure
/// (`PronePlayerState.ProcessAnimatorMovement`).
const THRESHOLD: f32 = 0.0001;
const LYING_STILL_THRESHOLD: f32 = 0.0009;

/// The push added is held to the motion's length and a centimetre, never
/// less than five centimetres (`LimitMotionXZ`).
const LEAST_HOLD: f32 = 0.05;
const HOLD_BEYOND: f32 = 0.01;

/// The lying capsule's object is put over the pivot bone no higher than
/// this in the body's axes (`UpdateProneColliderPosition`).
const LYING_TOP: f32 = 0.2;

/// A turn's prediction refuses a lying body's turn for a level push whose
/// square is this or more (`IsAbleToRotateProne`).
const TURN_LEAST: f32 = 1e-6;

/// The head's capsule in the world: the two ends of its middle line and its
/// radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadCapsule {
    pub ends: [[f32; 3]; 2],
    pub radius: f32,
}

impl HeadCapsule {
    /// The capsule `data` describes, its object at `place` turned by `turn`.
    pub fn of(data: &HeadCapsuleData, place: [f32; 3], turn: [f32; 4]) -> Self {
        let centre = quat_rotate(turn, data.centre);
        let mut axis = [0.0; 3];
        axis[data.direction.min(2)] = 1.0;
        let axis = quat_rotate(turn, axis);
        let half = (data.height / 2.0 - data.radius).max(0.0);
        let middle: [f32; 3] = std::array::from_fn(|k| place[k] + centre[k]);
        Self {
            ends: [std::array::from_fn(|k| middle[k] - axis[k] * half), std::array::from_fn(|k| middle[k] + axis[k] * half)],
            radius: data.radius,
        }
    }

    fn shifted(self, by: [f32; 3]) -> Self {
        Self { ends: self.ends.map(|end| std::array::from_fn(|k| end[k] + by[k])), radius: self.radius }
    }
}

/// Where the standing capsule's object is, on the camera's container at
/// `origin` turned by `turn` (`playercollider`'s place in its parent).
pub fn on_container(data: &HeadCapsuleData, origin: [f32; 3], turn: [f32; 4]) -> [f32; 3] {
    let beside = quat_rotate(turn, data.position);
    std::array::from_fn(|k| origin[k] + beside[k])
}

/// Where the lying capsule's object is in the body's own axes, for the
/// pivot bone at `pivot` in them: over it, no higher than 0.2 m
/// (`UpdateProneColliderPosition`).
pub fn over_pivot(pivot: [f32; 3]) -> [f32; 3] {
    [pivot[0], pivot[1].min(LYING_TOP), pivot[2]]
}

/// A frame's motion with the head's push added (`LimitMotionXZ`): the
/// push's level part, if its square reaches the threshold (lying still,
/// the larger), held to the motion's length and a centimetre, five at least.
pub fn limited(motion: [f32; 3], push: [f32; 3], lying_still: bool) -> [f32; 3] {
    let level = [push[0], 0.0, push[2]];
    let square = level[0] * level[0] + level[2] * level[2];
    let threshold = if lying_still { LYING_STILL_THRESHOLD } else { THRESHOLD };
    if square < threshold {
        return motion;
    }
    let length = (motion[0] * motion[0] + motion[1] * motion[1] + motion[2] * motion[2]).sqrt();
    let most = (length + HOLD_BEYOND).max(LEAST_HOLD);
    let size = square.sqrt();
    let hold = if size > most { most / size } else { 1.0 };
    std::array::from_fn(|k| motion[k] + level[k] * hold)
}

impl Movement {
    /// The head's capsule in use and its object's place and turn in the
    /// world: standing, `playercollider` on the camera's container; lying
    /// (from going down until getting up is over), `ProneCollisionTest`,
    /// which the game puts over the second spine bone as it is asked, in the
    /// body's own axes and no higher than 0.2 m.
    fn head_object(&self) -> Option<(HeadCapsuleData, [f32; 3], [f32; 4])> {
        let head = self.settings.head.as_ref()?;
        if self.head_lying {
            let yaw = quat_axis(1, self.body_yaw);
            let mut lay = quat_rotate(self.body_tilt, over_pivot(self.body.spine2()));
            lay[1] += self.body_height_offset;
            let turned = quat_rotate(yaw, lay);
            let place = std::array::from_fn(|k| self.position[k] + turned[k]);
            let turn = quat_mul(quat_mul(yaw, self.body_tilt), head.lying.rotation);
            Some((head.lying, place, turn))
        } else {
            let (origin, turn) = self.camera_frame();
            Some((head.standing, on_container(&head.standing, origin, turn), quat_mul(turn, head.standing.rotation)))
        }
    }

    /// The head's capsule as it stands, if the settings have it.
    pub fn head_capsule(&self) -> Option<HeadCapsule> {
        self.head_object().map(|(data, place, turn)| HeadCapsule::of(&data, place, turn))
    }

    /// The head's push as the last limit worked it out (`POM.GetOffset`).
    pub fn head_push(&self) -> [f32; 3] {
        self.head_push
    }

    /// A frame's motion with the head kept out of walls (`LimitMotionXZ`):
    /// the capsule moved by the motion is asked how far it would have to
    /// go to be clear (`World::capsule_push`), and `limited` adds that.
    pub(super) fn limit_motion_xz(&mut self, motion: [f32; 3], lying_still: bool, world: &dyn World) -> [f32; 3] {
        let Some(capsule) = self.head_capsule() else { return motion };
        let moved = capsule.shifted(motion);
        self.head_push = world.capsule_push(moved.ends, moved.radius);
        limited(motion, self.head_push, lying_still)
    }

    /// The head's level push were the body turned by `turn` degrees about
    /// the player's axis and moved by `shift` (`RotationOverlapPrediction`):
    /// the capsule's object carried round the axis, and turned upright to
    /// the view's yaw with the body's lay (`AnimatedTransform`'s pitch and
    /// roll) and the turn, not to the camera.
    pub(super) fn head_turn_push(&self, shift: [f32; 3], turn: f32, world: &dyn World) -> [f32; 3] {
        let Some((data, place, _)) = self.head_object() else { return [0.0; 3] };
        let animated = quat_to_euler(quat_mul(quat_axis(1, self.body_yaw), self.body_tilt));
        let upright = quat_euler([animated[0], self.yaw, animated[2]]);
        let rotation = quat_axis(1, turn);
        let inverse = [-upright[0], -upright[1], -upright[2], upright[3]];
        let from_root: [f32; 3] = std::array::from_fn(|k| place[k] - self.position[k]);
        let carried = quat_rotate(upright, quat_rotate(rotation, quat_rotate(inverse, from_root)));
        let moved: [f32; 3] = std::array::from_fn(|k| self.position[k] + carried[k] + shift[k]);
        let capsule = HeadCapsule::of(&data, moved, quat_mul(rotation, upright));
        let push = world.capsule_push(capsule.ends, capsule.radius);
        [push[0], 0.0, push[2]]
    }

    /// Whether a lying body's turn by `turn` degrees would swing the head
    /// into something (`IsAbleToRotateProne`, the head's half: the
    /// prediction for twice the turn).
    pub(super) fn head_turn_clear(&self, turn: f32, world: &dyn World) -> bool {
        let push = self.head_turn_push([0.0; 3], 2.0 * turn, world);
        push[0] * push[0] + push[2] * push[2] < TURN_LEAST
    }
}
