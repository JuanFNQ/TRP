//! Holding the weapon in the figure's hands.

use super::*;

impl Figure {
    /// The poser's late update: the weapon placed on the body, the elbows'
    /// goals, both arms solved onto the markers, the fingers, the twist
    /// bones. Without a weapon animator the body is left as its animation
    /// has it.
    pub(super) fn hold(&mut self) {
        let mut locals = self.body.pose().to_vec();
        let Some(held) = &mut self.held else {
            self.pose = locals;
            return;
        };
        let parents = &self.data.skeleton.parents;
        let n = &self.data.named;
        let world = compose(parents, &locals);
        let curve = |name: &str| self.body.float(name);
        // Each arm's share that is the weapon's, right then left; the
        // hold's; each elbow goal's share that is the weapon's.
        let weights = HAND.map(|name| 1.0 - curve(name));
        let third = curve(WEAPON_3RD);
        let elbows = ELBOW.map(|name| 1.0 - curve(name));
        let data = &held.data;
        held.targets = [None; 2];
        held.rig.pose(held.animator.pose(), None);
        // The prefab's root is on the ribcage at nought.
        let ribcage = world[n.ribcage];
        let placed = |rig: &WeaponRig, bone: usize| then(ribcage, rig.placed(bone));
        let animated = placed(&held.rig, data.moved);
        let moved = match data.markers[0] {
            // The body's animation keeps the right hand: the weapon hangs by
            // its right marker from the palm (`PlayerBones.Kinematics` at
            // nought).
            Some(marker) if weights[0] < 1.0 => {
                let palm = world[n.arms[0][5]];
                let on = within(held.rig.placed(data.moved), held.rig.placed(marker));
                let turn = quat_mul(palm.1, inverse(on.1));
                let shifted = quat_rotate(turn, on.0);
                (std::array::from_fn(|k| palm.0[k] - shifted[k]), turn)
            }
            // On the body's bone by the hold's share (`ShiftWeaponRoot` seen
            // from outside; the menu's `Offset` nought, `DeltaRotation` none).
            _ if third > HOLD_LEAST => {
                let bone = world[n.weapon_3rd];
                (lerp3(animated.0, bone.0, third), quat_lerp(animated.1, bone.1, third))
            }
            // Otherwise the weapon is left as its animator has it.
            _ => animated,
        };
        held.rig.set_placed(data.moved, within(ribcage, moved));
        held.moved = moved;
        let at = |bone: usize| placed(&held.rig, bone);
        let markers = data.markers.map(|marker| marker.map(at));
        held.markers = markers;
        // The elbows' goals moved toward the weapon's.
        let mut goals = [world[n.goals[0]].0, world[n.goals[1]].0];
        if let [Some(right), Some(left)] = data.goals {
            for (side, goal) in [right, left].into_iter().enumerate() {
                goals[side] = lerp3(goals[side], at(goal).0, elbows[side]);
            }
        }
        for side in 0..2 {
            let parent = world[parents[n.goals[side]].expect("a goal's parent")];
            locals[n.goals[side]].0 = within(parent, (goals[side], [0.0, 0.0, 0.0, 1.0])).0;
        }
        // Each hand's target: its marker, pulled onto the part's grip as the
        // marker nears its reference.
        let blend = held.blend;
        let crutch = held.crutch.value(self.age);
        let grip_of = |choice: &GripChoice| -> (Placed, bool) {
            match choice {
                GripChoice::One { node, alternative } => (at(*node), *alternative),
                GripChoice::Blend { nodes, .. } => {
                    let (a, b) = (at(nodes[0]), at(nodes[1]));
                    let placed = if blend <= 0.0 {
                        a
                    } else if blend >= 1.0 {
                        b
                    } else {
                        (lerp3(a.0, b.0, blend), quat_lerp(a.1, b.1, blend))
                    };
                    (placed, true)
                }
            }
        };
        for side in 0..2 {
            let weight = weights[side];
            let arm = n.arms[side];
            let target = match markers[side] {
                Some(marker) if weight.abs() >= f32::EPSILON => match (&data.grips[side], data.references[side]) {
                    (Some(choice), Some(reference)) => {
                        let (grip, alternative) = grip_of(choice);
                        let mut away = distance(marker.0, at(reference).0);
                        if alternative {
                            away *= 1.0 - crutch;
                        }
                        let on = inverse_lerp(GRIP_RELEASE, 0.0, away);
                        held.grip_weight[side] = on;
                        (lerp3(marker.0, grip.0, on), quat_lerp(marker.1, grip.1, on))
                    }
                    _ => marker,
                },
                Some(marker) => marker,
                None => world[arm[5]],
            };
            if weight.abs() >= f32::EPSILON && markers[side].is_some() {
                held.targets[side] = Some(target);
            }
            let found = Arm { upper: world[arm[1]], below: std::array::from_fn(|k| locals[arm[k + 2]]) };
            let solved = arms::solve(&found, target, goals[side], if markers[side].is_some() { weight } else { 0.0 }, &self.data.limbs[side]);
            // Back into each bone's parent: only the turns change.
            locals[arm[1]].1 = quat_mul(inverse(world[arm[0]].1), solved[0].1);
            for k in 1..5 {
                locals[arm[k + 1]].1 = quat_mul(inverse(solved[k - 1].1), solved[k].1);
            }
        }
        // The fingers: the left hand's poser by the left arm's share, the
        // right's always whole (`UpdatePersonGrip` sets it so and nothing
        // after); each finger toward the weapon's own hand's, or the grip's
        // by the hand's share of it.
        let fingers_of = |node: usize| {
            data.grip_fingers.iter().find(|grip| grip.node == node).map(|grip| match &grip.live {
                Some(bones) => bones.iter().map(|&bone| held.rig.local(bone).1).collect::<Vec<_>>(),
                None => grip.stored.clone(),
            })
        };
        for side in 0..2 {
            let poser = if side == 1 { weights[1] } else { 1.0 };
            if poser <= POSER_LEAST {
                continue;
            }
            let rig_hand = &data.palms[side];
            let grip: Option<Vec<[f32; 4]>> = match &data.grips[side] {
                Some(GripChoice::One { node, .. }) => fingers_of(*node),
                Some(GripChoice::Blend { fingers, .. }) => Some(
                    (0..fingers[0].len().min(fingers[1].len()))
                        .map(|i| if blend <= 0.0 { fingers[0][i] } else if blend >= 1.0 { fingers[1][i] } else { quat_lerp(fingers[0][i], fingers[1][i], blend) })
                        .collect(),
                ),
                None => None,
            };
            let grip_weight = held.grip_weight[side];
            for (k, &bone) in n.fingers[side].iter().enumerate() {
                let index = k + 1;
                let Some(&rig_bone) = rig_hand.get(index) else { break };
                let own = held.rig.local(rig_bone).1;
                let wanted = match grip.as_ref().and_then(|grip| grip.get(index)) {
                    None => own,
                    Some(&fixed) if grip_weight >= 1.0 => fixed,
                    Some(&fixed) => quat_lerp(own, fixed, grip_weight),
                };
                locals[bone].1 = quat_lerp(locals[bone].1, wanted, poser);
            }
        }
        if let Some(blender) = &held.blender {
            held.blend = blender.value(self.age);
        }
        self.pose = locals;
    }
}

/// Each bone of a pose in the root's axes, from each in its parent.
pub(super) fn compose(parents: &[Option<usize>], locals: &[Placed]) -> Vec<Placed> {
    let mut world: Vec<Placed> = Vec::with_capacity(locals.len());
    for (bone, &local) in locals.iter().enumerate() {
        world.push(match parents[bone] {
            Some(parent) => then(world[parent], local),
            None => local,
        });
    }
    world
}
