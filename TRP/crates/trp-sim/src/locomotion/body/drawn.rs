//! The drawn bones, the ribcage, the weapon and the arm poses.

use super::*;

impl Body {
    /// The rotation of the last spine bone in the body's axes.
    fn spine(&self) -> [f32; 4] {
        (0..SPINE).fold([0.0, 0.0, 0.0, 1.0], |acc, i| quat_mul(acc, self.animator.bone(i).1))
    }

    /// A bone's place and turning in the body's axes, down a chain of the
    /// bones followed: each turns and shifts what hangs from it.
    fn through(&self, chain: &[usize]) -> ([f32; 3], [f32; 4]) {
        chain.iter().fold(([0.0; 3], [0.0, 0.0, 0.0, 1.0]), |(at, turn), &i| {
            let (position, rotation) = self.animator.bone(i);
            let shifted = quat_rotate(turn, position);
            (std::array::from_fn(|k| at[k] + shifted[k]), quat_mul(turn, rotation))
        })
    }

    /// The further bones asked for at loading (`BodyData::load_with`), in
    /// the order asked: each one's place and turning in its parent, as the
    /// body's animation has it.
    pub fn drawn_bones(&self) -> Vec<([f32; 3], [f32; 4])> {
        self.drawn_from(self.animator.pose())
    }

    /// The further bones as `drawn_bones` gives them, made from an update's
    /// mix (`posing_mix`) whenever asked: what `drawn_bones` gave after that
    /// update with nothing deferred, the free look's turn of the neck and
    /// the head as it now stands.
    pub fn drawn_bones_of(&self, mix: &Mix) -> Vec<([f32; 3], [f32; 4])> {
        self.drawn_bones_turned(mix, self.head_turn)
    }

    /// The further bones as `drawn_bones` gives them, made from an update's
    /// mix with the free look's turn `head_turn` as it was then
    /// (`head_turn`): what `drawn_bones` gave after that update with
    /// nothing deferred, whenever asked (`Puppet`'s recipes).
    pub fn drawn_bones_turned(&self, mix: &Mix, head_turn: [f32; 3]) -> Vec<([f32; 3], [f32; 4])> {
        self.drawn_turned(&self.animator.pose_of(mix), head_turn)
    }

    /// Leaves the further bones asked for at loading, which nothing of the
    /// body goes by, to be made from each update's mix when asked
    /// (`drawn_bones_of`), or works them out at each update (off, as a body
    /// is made): `Animator::set_deferred`. Where on, `drawn_bones` gives
    /// them as last worked out.
    pub fn set_deferred_posing(&mut self, on: bool) {
        self.animator.set_deferred(on);
    }

    pub fn posing_deferred(&self) -> bool {
        self.animator.deferred()
    }

    /// The last update's mix (`Animator::mix`).
    pub fn posing_mix(&self) -> &Mix {
        self.animator.mix()
    }

    fn drawn_from(&self, pose: &[([f32; 3], [f32; 4])]) -> Vec<([f32; 3], [f32; 4])> {
        self.drawn_turned(pose, self.head_turn)
    }

    fn drawn_turned(&self, pose: &[([f32; 3], [f32; 4])], turn: [f32; 3]) -> Vec<([f32; 3], [f32; 4])> {
        // The root is the animator's own root bone, which has had the
        // clips' travel and turning over the ground taken off it; asked
        // for a second time it is a bone like any other and keeps them.
        let bone = |index: usize| match self.drawn_root {
            Some(root) if root == index => pose[0],
            _ => pose[DRAWN_FROM + index],
        };
        let mut bones: Vec<([f32; 3], [f32; 4])> = (0..self.drawn).map(bone).collect();
        // The free look's turn of the neck and the head, where it is more
        // than the game's least.
        if turn.iter().map(|angle| angle * angle).sum::<f32>() >= HEAD_TURN_LEAST {
            for (at, share) in self.head_bones.iter().zip(self.head_shares) {
                if let Some(bone) = at.and_then(|at| bones.get_mut(at)) {
                    bone.1 = quat_mul(bone.1, crate::base::math::quat_euler(turn.map(|angle| angle * share)));
                }
            }
        }
        bones
    }

    /// The left shoulder's curve, which its `LS` layer's clips ramp from 0
    /// to 1 as the stance comes in (`PlayerAnimator.LEFT_STANCE_CURVE`): the
    /// weapon goes to the shoulder by it (`hands::LeftShoulder`).
    pub fn left_stance_curve(&self) -> f32 {
        self.ids.left_stance_curve.map_or(0.0, |id| self.animator.get(id))
    }

    /// The ribcage's own place in its parent, the last spine bone, as the
    /// animation has it (`PlayerBones.Ribcage.Original.localPosition`).
    pub fn ribcage_local(&self) -> [f32; 3] {
        self.animator.bone(RIBCAGE).0
    }

    /// Where the ribcage is, from the feet, in the body's axes. The
    /// first-person rig hangs there, and the camera beside it
    /// (`camera_container`).
    pub fn ribcage(&self) -> [f32; 3] {
        self.through(&TO_RIBCAGE).0
    }

    /// Where the pelvis is, from the feet, in the body's axes: the lower
    /// end of a host's broad test's capsule (`Puppet::bound`).
    pub fn pelvis(&self) -> [f32; 3] {
        self.through(&[0, 1]).0
    }

    /// Where the second spine bone is, from the feet, in the body's axes:
    /// the head's lying capsule is put over it (`PlayerOverlapManager.
    /// _proneColliderPivot`; movement spec, "The head's push out of walls").
    pub fn spine2(&self) -> [f32; 3] {
        self.through(&[0, 1, 2, 3]).0
    }

    /// Where the first-person camera's own object is, from the feet, in the
    /// body's axes (`Player.CameraContainer`): on the last spine bone at the
    /// ribcage's rest place, so at the ribcage itself unless a clip has
    /// shifted the ribcage along its spine.
    pub fn camera_container(&self) -> [f32; 3] {
        self.through(&[0, 1, 2, 3, 4, CAMERA_CONTAINER]).0
    }

    /// The angle from the body round to the view, and the view's pitch,
    /// that the pose was last made for (the animator's `DeltaRotation` and
    /// `Aim_angle` as the last update had them).
    pub fn posed_for(&self) -> [f32; 2] {
        let value = |id: Option<usize>| id.map_or(0.0, |id| self.animator.get(id));
        [value(self.ids.delta_rotation), value(self.ids.aim_angle)]
    }

    /// Where the ribcage would be now had the pose been made for another
    /// angle to the view and another pitch, with everything else as it is.
    /// Costs a working-out of the bones down to the ribcage alone
    /// (`Animator::bones_with`), which are where the whole pose would put
    /// them (`ribcage_posed_whole`), to the last bit.
    pub fn ribcage_posed_for(&self, delta_rotation: f32, aim_angle: f32) -> [f32; 3] {
        let (Some(delta), Some(aim)) = (self.ids.delta_rotation, self.ids.aim_angle) else { return self.ribcage() };
        let pose = self.animator.bones_with(&[(delta, delta_rotation), (aim, aim_angle)], &TO_RIBCAGE);
        Self::ribcage_of(&pose)
    }

    /// `ribcage_posed_for` from a working-out of the whole pose
    /// (`Animator::pose_with`), as it was made before: for the test that
    /// the two agree.
    #[doc(hidden)]
    pub fn ribcage_posed_whole(&self, delta_rotation: f32, aim_angle: f32) -> [f32; 3] {
        let (Some(delta), Some(aim)) = (self.ids.delta_rotation, self.ids.aim_angle) else { return self.ribcage() };
        Self::ribcage_of(&self.animator.pose_with(&[(delta, delta_rotation), (aim, aim_angle)]))
    }

    /// The ribcage's place down its chain of a pose's bones.
    fn ribcage_of(pose: &[([f32; 3], [f32; 4])]) -> [f32; 3] {
        TO_RIBCAGE
            .iter()
            .fold(([0.0; 3], [0.0, 0.0, 0.0, 1.0]), |(at, turn), &i| {
                let (position, rotation) = pose[i];
                let shifted = quat_rotate(turn, position);
                (std::array::from_fn(|k| at[k] + shifted[k]), quat_mul(turn, rotation))
            })
            .0
    }

    /// One bone of an arm as the body's animation has it, from the feet in
    /// the body's axes: `joint` 0 is the collarbone, 1 the upper arm, 2 to
    /// 4 the three bones of the forearm and 5 the palm.
    pub fn arm_bone(&self, right: bool, joint: usize) -> ([f32; 3], [f32; 4]) {
        let mut chain = TO_RIBCAGE.to_vec();
        chain.extend_from_slice(&ARMS[usize::from(!right)][..=joint.min(5)]);
        self.through(&chain)
    }

    /// The body's own weapon bone, in the same axes.
    pub fn weapon_bone(&self) -> ([f32; 3], [f32; 4]) {
        self.through(&[0, 1, 2, 3, 4, WEAPON])
    }

    /// The body's own camera bone, which lends its rotation to the view.
    pub fn camera_bone(&self) -> [f32; 4] {
        self.animator.bone(CAMERA).1
    }

    /// Where the body holds the weapon now, if it holds it at all.
    ///
    /// `into_view` turns the body's axes into those of the frame the
    /// weapon's rig hangs in: at the ribcage, turned to where the player
    /// looks. `Movement::body_in_view` gives it.
    pub fn carried(&self, into_view: [f32; 4]) -> Option<Carried> {
        Some(self.weapon_root_third(into_view)).filter(|carried| carried.authority > 0.0)
    }

    /// The body's own weapon bone (`PlayerBones.Weapon_Root_Third`) in the
    /// same frame as `carried`, whatever its authority: the left shoulder
    /// puts the weapon there by its own curve (`hands::LeftShoulder`).
    pub fn weapon_root_third(&self, into_view: [f32; 4]) -> Carried {
        // Each curve at most 1 as the game reads it (`Player.GetCurveValue`),
        // then their product: a gesture takes the first-person curve to 2.
        let curve = |id: Option<usize>| id.map_or(0.0, |id| self.animator.get(id)).min(1.0);
        let first_person = self.first_person_weight().min(1.0);
        // A mount takes the weapon from the body's hold whatever the clips
        // say (recorded: lying on a bipod the curves give 2 and 1, and the
        // first-person weapon stays where the mount puts it).
        let authority = if self.mounted[0] { 0.0 } else { (curve(self.ids.weapon_3rd) * first_person).clamp(0.0, 1.0) };
        let position_cache = (curve(self.ids.position_cache) * first_person).clamp(0.0, 1.0);
        let spine = self.spine();
        let (weapon, ribcage) = (self.animator.bone(WEAPON), self.animator.bone(RIBCAGE));
        let offset: [f32; 3] = std::array::from_fn(|k| weapon.0[k] - ribcage.0[k]);
        Carried {
            position: quat_rotate(into_view, quat_rotate(spine, offset)),
            rotation: quat_mul(into_view, quat_mul(spine, weapon.1)),
            authority,
            position_cache,
        }
    }

    /// An arm as the body's animation has it, while that animation takes
    /// any of it from the weapon: the crawl takes both arms whole. `None`
    /// while the arm is the weapon's alone. The frame is that of `carried`.
    pub fn arm(&self, right: bool, into_view: [f32; 4]) -> Option<ArmPose> {
        let pose = self.arm_pose(right, into_view);
        (pose.hand > 0.0 || pose.shoulder > 0.0 || pose.elbow > 0.0).then_some(pose)
    }

    /// An arm as the body's animation has it, whatever share of it that
    /// animation claims. The arm's solver starts from this even when the
    /// arm is all the weapon's: the forearm and palm it finds are the
    /// body's (`arms::hold`). The frame is that of `carried`.
    pub fn arm_pose(&self, right: bool, into_view: [f32; 4]) -> ArmPose {
        let curve = |id: Option<usize>| id.map_or(0.0, |id| self.animator.get(id)).clamp(0.0, 1.0);
        let (hand, shoulder, elbow) = if right {
            (self.ids.hand_right, self.ids.shoulder_right, self.ids.elbow_right)
        } else {
            (self.ids.hand_left, self.ids.shoulder_left, self.ids.elbow_left)
        };
        let first_person = self.first_person_weight().clamp(0.0, 1.0);
        // The game puts the upper arm on the weapon rig's only for the
        // character's own eyes (`SetShoulders`): seen from outside the
        // shoulder is all the body's.
        // Mounted upright, the game holds both upper arms on the rig.
        let shoulder = match self.point_of_view {
            PointOfView::FirstPerson if self.mounted == [true, false] => 0.0,
            PointOfView::FirstPerson => curve(shoulder),
            PointOfView::ThirdPerson => 1.0,
        };
        let (hand, elbow) = (curve(hand) * first_person, curve(elbow) * first_person);
        let bones = ARMS[usize::from(!right)];
        // The upper arm from the ribcage, by way of the collarbone, and the
        // bones below it each in the one above.
        let (upper, turn) = self.through(&[0, 1, 2, 3, 4, RIBCAGE, bones[0], bones[1]]);
        let below = std::array::from_fn(|i| self.animator.bone(bones[i + 2]));
        let ribcage = self.ribcage();
        let (goal, _) = self.through(&[0, 1, 2, 3, 4, BEND_GOALS + usize::from(!right)]);
        ArmPose {
            hand,
            shoulder,
            elbow,
            bend_goal: quat_rotate(into_view, std::array::from_fn(|k| goal[k] - ribcage[k])),
            upper: (
                quat_rotate(into_view, std::array::from_fn(|k| upper[k] - ribcage[k])),
                quat_mul(into_view, turn),
            ),
            below,
            fingers: (!right).then(|| std::array::from_fn(|i| self.animator.bone(LEFT_FINGERS + i).1)),
        }
    }

    /// The body's right palm, while its animation has that hand to itself
    /// and the weapon has to go where the hand is. The frame is that of
    /// `carried`.
    pub fn held(&self, into_view: [f32; 4]) -> Option<Held> {
        // Each curve at most 1 (`Player.GetCurveValue`), then their product.
        let curve = |id: Option<usize>| id.map_or(0.0, |id| self.animator.get(id)).min(1.0);
        let weight = 1.0 - (curve(self.ids.hand_right) * self.first_person_weight().min(1.0)).clamp(0.0, 1.0);
        if weight >= 1.0 {
            return None;
        }
        let (palm, turn) = self.through(&TO_RIGHT_PALM);
        let ribcage = self.ribcage();
        Some(Held {
            position: quat_rotate(into_view, std::array::from_fn(|k| palm[k] - ribcage[k])),
            rotation: quat_mul(into_view, turn),
            weight,
        })
    }
}
