//! The recipe a frame is made from, what is kept and the drawing between frames.

use super::*;

/// What one frame of the character leaves for its pose: all the pose is
/// made from (`Puppet::pose_of_frame`), the simulation's half done. Kept
/// for the frames a host may ask for (the last few: `Puppet::drawn`'s).
#[derive(Debug, Clone)]
pub struct Recipe {
    /// The frame's number among the character's own, from 0 at its first.
    pub frame: u64,
    /// As the frame's pose has them (`PuppetPose`).
    pub position: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub body_at: [f32; 3],
    pub body_turn: [f32; 4],
    pub ribcage: [f32; 3],
    /// The capsule that holds its hit colliders.
    pub bound: Bound,
    /// The body's and the thing in hand's animators' mixes, where the frame
    /// was deferred (`Posing::Asked`); a frame posed in full keeps its pose.
    pub(super) mixes: Option<(Mix, WeaponMix)>,
    /// The free look's turn of the neck and the head.
    pub(super) head_turn: [f32; 3],
    /// The moved bone in its parent, where the weapon's motion put it.
    pub(super) weapon: Placed,
    /// Each arm as the body's animation has it, right then left, and the
    /// elbows' shift by the weapon's motion.
    pub(super) arms: [ArmPose; 2],
    pub(super) elbows: [[f32; 3]; 2],
}

/// A frame kept: its recipe, and its pose once made.
#[derive(Debug)]
pub(super) struct Kept {
    pub(super) recipe: Recipe,
    pub(super) pose: OnceLock<PuppetPose>,
}

/// What a pose is made with that does not change: the rig at rest, the
/// moved bone, the hands on it and the arms' solver.
pub(super) struct Drawing {
    pub(super) rest: WeaponRig,
    pub(super) moved: usize,
    pub(super) hands_rig: [RigHand; 2],
    pub(super) limbs: [LimbData; 2],
    /// The animator's poses are drawn on curves without corners.
    pub(super) smooth: bool,
}

impl Drawing {
    /// The pose a frame's recipe makes, from the rig's pose as given out
    /// (`straight`) and as drawn (`drawn`), and the body's drawn bones: the
    /// moved bone where the weapon's motion and the body put it, the arms
    /// solved onto the rig so placed (on each of the two where they
    /// differ), the fingers closed on the grip as the hand comes onto it.
    pub(super) fn make(&self, recipe: &Recipe, straight: &[Placed], drawn: &[Placed], bones: Vec<Placed>) -> PuppetPose {
        let placed = |pose: &[Placed]| {
            let mut rig = self.rest.clone();
            rig.pose(pose, Some(self.moved));
            rig.set_local(self.moved, recipe.weapon);
            rig
        };
        let solve = |rig: &WeaponRig| {
            let mut arms_out = [[([0.0; 3], [0.0, 0.0, 0.0, 1.0]); 5]; 2];
            let mut grips = [0.0f32; 2];
            for side in 0..2 {
                let mut on_rig = rig.arm(&self.hands_rig[side].nodes);
                on_rig.goal = std::array::from_fn(|k| on_rig.goal[k] + recipe.elbows[side][k]);
                let (bones, grip) = arms::hold(&recipe.arms[side], &on_rig, &self.limbs[side]);
                arms_out[side] = bones;
                grips[side] = grip;
            }
            (arms_out, grips)
        };
        let rig = placed(drawn);
        let (arms_out, grips) = solve(&rig);
        let straight_arms = if self.smooth { solve(&placed(straight)).0 } else { arms_out };
        let mut locals: Vec<Placed> = (0..rig.len()).map(|bone| rig.local(bone)).collect();
        for (side, hand) in self.hands_rig.iter().enumerate() {
            if grips[side] <= 0.0 {
                continue;
            }
            for (finger, held) in hand.fingers.iter().zip(hand.grip_pose.iter().skip(1)) {
                if let Some(local) = locals.get_mut(*finger) {
                    local.1 = nlerp(local.1, *held, grips[side]);
                }
            }
        }
        PuppetPose {
            position: recipe.position,
            yaw: recipe.yaw,
            pitch: recipe.pitch,
            body_at: recipe.body_at,
            body_turn: recipe.body_turn,
            bones,
            ribcage: recipe.ribcage,
            rig: locals,
            arms: arms_out,
            straight_arms,
        }
    }
}

/// What the character's hands hold: a firearm, or nothing (the empty
/// hands' controller on their own animator).
pub(super) enum Held {
    Firearm(Firearm),
    Empty(EmptyHands),
}

impl Held {
    pub(super) fn animator(&self) -> &WeaponAnimator {
        match self {
            Held::Firearm(firearm) => firearm.animator(),
            Held::Empty(empty) => empty.animator(),
        }
    }

    /// The sights up: a firearm's alone.
    pub(super) fn aiming(&self) -> bool {
        matches!(self, Held::Firearm(firearm) if firearm.aiming())
    }
}
