//! The edits a custom weapon makes to the hands' pose.

use super::*;

pub(super) type Placed = ([f32; 3], [f32; 4]);

pub(super) fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let t = [2.0 * (q[1] * v[2] - q[2] * v[1]), 2.0 * (q[2] * v[0] - q[0] * v[2]), 2.0 * (q[0] * v[1] - q[1] * v[0])];
    [
        v[0] + q[3] * t[0] + q[1] * t[2] - q[2] * t[1],
        v[1] + q[3] * t[1] + q[2] * t[0] - q[0] * t[2],
        v[2] + q[3] * t[2] + q[0] * t[1] - q[1] * t[0],
    ]
}

pub(super) fn back(q: [f32; 4]) -> [f32; 4] {
    [-q[0], -q[1], -q[2], q[3]]
}

pub(super) fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(super) fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Centimetres ahead, up and to the right, as metres on the weapon's
/// bone: to the right, back and up.
pub(super) fn of_centimetres(v: [f32; 3]) -> [f32; 3] {
    [v[2] * 0.01, -v[0] * 0.01, v[1] * 0.01]
}

/// One of the left hand's holds, on the rig's own bones.
#[derive(Debug, Clone)]
pub(super) struct Hold {
    /// The bone it is at, or none for the weapon's own.
    pub(super) bone: Option<usize>,
    /// Where the palm's marker is from that bone while the hand is on
    /// it, in the bone's own axes.
    pub(super) palms: Vec<[f32; 3]>,
    /// How far from there the same thing lies on this weapon, on the
    /// weapon's bone.
    pub(super) shift: [f32; 3],
}

/// The edits made to every pose the donor's animator gives
/// (`docs/spec/custom-weapons.md`, "The animation, edited"). They are
/// rules on the pose, not on a clip, so that they hold through every
/// state and every blend alike:
///
/// - **A bone moved**: the same shift is added to where the animator
///   has the bone, whatever it is doing. A part then turns about a point
///   of its own and travels the donor's path from the model's own place.
/// - **The left hand follows what it holds**: the palm's marker is
///   shifted by what the things it is near are shifted by, each weighted
///   by how near the palm is to where it holds that thing on the donor.
#[derive(Debug, Clone)]
pub struct PoseEdits {
    pub(super) parents: Vec<Option<usize>>,
    pub(super) weapon: usize,
    /// Each bone moved, with its shift on the weapon's bone.
    pub(super) moved: Vec<(usize, [f32; 3])>,
    /// The left palm's marker and its fixed reference, with the holds.
    pub(super) hand: Option<(usize, usize, Vec<Hold>, [f32; 3], f32)>,
    /// Where the donor's prefab has each bone these edits move, in its
    /// parent: what its animator is to take as the bone's rest.
    pub(super) donor_rest: Vec<(usize, [f32; 3])>,
}

impl PoseEdits {
    /// A bone on the weapon's own bone, as a pose has it.
    pub(super) fn on_weapon(&self, pose: &[Placed], bone: usize) -> Placed {
        let (mut at, mut turn) = pose[bone];
        let mut above = self.parents[bone];
        while let Some(parent) = above.filter(|parent| *parent != self.weapon) {
            let (place, parent_turn) = pose[parent];
            at = add(place, rotate(parent_turn, at));
            turn = quat_mul(parent_turn, turn);
            above = self.parents[parent];
        }
        (at, turn)
    }

    /// How a bone's parent is turned on the weapon's bone.
    pub(super) fn parent_turn(&self, pose: &[Placed], bone: usize) -> [f32; 4] {
        match self.parents[bone].filter(|parent| *parent != self.weapon) {
            Some(parent) => self.on_weapon(pose, parent).1,
            None => [0.0, 0.0, 0.0, 1.0],
        }
    }

    /// What the left palm's marker is shifted by in a pose as the
    /// donor's clips have it.
    fn hand_shift(&self, pose: &[Placed]) -> [f32; 3] {
        let Some((marker, _, holds, _, exponent)) = &self.hand else { return [0.0; 3] };
        let palm = pose[*marker].0;
        let (mut total, mut sum) = (0.0f32, [0.0f32; 3]);
        for hold in holds {
            let (at, turn) = hold.bone.map_or(([0.0; 3], [0.0, 0.0, 0.0, 1.0]), |bone| self.on_weapon(pose, bone));
            let away = hold
                .palms
                .iter()
                .map(|offset| {
                    let d = sub(palm, add(at, rotate(turn, *offset)));
                    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
                })
                .fold(f32::MAX, f32::min);
            let weight = (away + ON_A_HOLD).powf(-exponent);
            total += weight;
            sum = add(sum, hold.shift.map(|s| s * weight));
        }
        if total > 0.0 {
            sum.map(|s| s / total)
        } else {
            [0.0; 3]
        }
    }

    /// Edits a pose the donor's animator has given: every bone of the
    /// rig, each in its parent.
    pub fn apply(&self, pose: &mut [Placed]) {
        // The hand first, from the pose as the donor has it.
        let shift = self.hand_shift(pose);
        let turns: Vec<[f32; 4]> = self.moved.iter().map(|(bone, _)| self.parent_turn(pose, *bone)).collect();
        for ((bone, by), parent_turn) in self.moved.iter().zip(turns) {
            pose[*bone].0 = add(pose[*bone].0, rotate(back(parent_turn), *by));
        }
        if let Some((marker, reference, _, rest, _)) = &self.hand {
            pose[*marker].0 = add(pose[*marker].0, shift);
            pose[*reference].0 = add(pose[*reference].0, *rest);
        }
    }

    /// Where the donor's own prefab has the bones these edits move, each
    /// in its parent. The weapon's hierarchy has them as edited.
    pub fn donor_rest(&self) -> &[(usize, [f32; 3])] {
        &self.donor_rest
    }
}

impl PoseEdits {
    /// Edits that change nothing: those of an install's weapon that is
    /// drawn with parts of one's own.
    pub fn none() -> PoseEdits {
        PoseEdits { parents: Vec::new(), weapon: 0, moved: Vec::new(), hand: None, donor_rest: Vec::new() }
    }
}
