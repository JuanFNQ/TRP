//! Character, spring and fixed joints between bodies.

use super::*;

impl World {
    /// Unity's character joint, as PhysX's six-way joint: the three ways
    /// along locked, the twist about the axis between its two limits, each
    /// swing limited by its own angle and locked where that is none, the
    /// joint frames as the two bodies stand now.
    pub fn character_joint(&mut self, j: &CharacterJointDesc) {
        unsafe {
            let own = self.actor(j.own) as *mut PxRigidActor;
            let connected = self.actor(j.connected) as *mut PxRigidActor;
            let x = norm(j.axis);
            let y0 = j.swing_axis;
            let mut y = norm([y0[0] - x[0] * dot(x, y0), y0[1] - x[1] * dot(x, y0), y0[2] - x[2] * dot(x, y0)]);
            if dot(y, y) < 0.5 {
                // No swing axis to speak of: any across the twist axis.
                let other = if x[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
                y = norm(cross(cross(x, other), x));
            }
            let z = cross(x, y);
            let frame_own = pose(j.anchor, turn_of_axes(x, y, z));
            // The connected body's frame where the own one's is at rest, or
            // now.
            let frame_connected = match j.rest {
                Some((at, turn)) => {
                    let relation = pose(at, turn);
                    PxTransform_transform_1(&relation, &frame_own)
                }
                None => {
                    let own_pose = PxRigidActor_getGlobalPose(own);
                    let connected_pose = PxRigidActor_getGlobalPose(connected);
                    let world = PxTransform_transform_1(&own_pose, &frame_own);
                    let inverse = PxTransform_getInverse(&connected_pose);
                    PxTransform_transform_1(&inverse, &world)
                }
            };
            let (a0, f0, a1, f1) = if self.convention.own_first { (own, frame_own, connected, frame_connected) } else { (connected, frame_connected, own, frame_own) };
            let joint = phys_PxD6JointCreate(self.physics, a0, &f0, a1, &f1);
            for axis in [PxD6Axis::eX, PxD6Axis::eY, PxD6Axis::eZ] {
                PxD6Joint_setMotion_mut(joint, axis, PxD6Motion::eLOCKED);
            }
            let (mut low, mut high) = (j.low_twist.to_radians(), j.high_twist.to_radians());
            if self.convention.twist_negated {
                (low, high) = (-high, -low);
            }
            let twist_free = (high - low).abs() > 1e-6;
            PxD6Joint_setMotion_mut(joint, PxD6Axis::eTWIST, if twist_free { PxD6Motion::eLIMITED } else { PxD6Motion::eLOCKED });
            if twist_free {
                let mut limit = PxJointAngularLimitPair_new(low, high, -1.0);
                limit.restitution = j.bounciness[0].max(j.bounciness[1]);
                PxD6Joint_setTwistLimit_mut(joint, &limit);
            }
            let (s1, s2) = if self.convention.swings_swapped { (j.swing2.to_radians(), j.swing1.to_radians()) } else { (j.swing1.to_radians(), j.swing2.to_radians()) };
            PxD6Joint_setMotion_mut(joint, PxD6Axis::eSWING1, if s1 > 1e-4 { PxD6Motion::eLIMITED } else { PxD6Motion::eLOCKED });
            PxD6Joint_setMotion_mut(joint, PxD6Axis::eSWING2, if s2 > 1e-4 { PxD6Motion::eLIMITED } else { PxD6Motion::eLOCKED });
            if s1 > 1e-4 || s2 > 1e-4 {
                // A locked swing's angle is not looked at; the cone wants
                // both above nothing.
                let mut cone = PxJointLimitCone_new(s1.max(1e-3), s2.max(1e-3), -1.0);
                cone.restitution = j.bounciness[2].max(j.bounciness[3]);
                PxD6Joint_setSwingLimit_mut(joint, &cone);
            }
            let joint = joint as *mut PxJoint;
            let (own_scale, connected_scale) = if self.convention.scales_inverted { (1.0 / j.mass_scale, 1.0 / j.connected_mass_scale) } else { (j.mass_scale, j.connected_mass_scale) };
            let (scale0, scale1) = if self.convention.own_first { (own_scale, connected_scale) } else { (connected_scale, own_scale) };
            PxJoint_setInvMassScale0_mut(joint, scale0);
            PxJoint_setInvInertiaScale0_mut(joint, scale0);
            PxJoint_setInvMassScale1_mut(joint, scale1);
            PxJoint_setInvInertiaScale1_mut(joint, scale1);
            PxJoint_setConstraintFlag_mut(joint, PxConstraintFlag::eCOLLISION_ENABLED, j.collision);
            PxJoint_setConstraintFlag_mut(joint, PxConstraintFlag::eDISABLE_PREPROCESSING, !j.preprocessing);
            if let Some([distance, angle]) = j.projection {
                PxJoint_setConstraintFlag_mut(joint, PxConstraintFlag::ePROJECTION, true);
                PxD6Joint_setProjectionLinearTolerance_mut(joint as *mut PxD6Joint, distance);
                PxD6Joint_setProjectionAngularTolerance_mut(joint as *mut PxD6Joint, angle.to_radians());
            }
            self.joints.push(joint);
        }
    }

    /// Unity's spring joint: a spring between two points, pulling them to
    /// within `[min, max]` of each other (both nothing: together).
    #[allow(clippy::too_many_arguments)]
    pub fn spring_joint(&mut self, own: Body, anchor: V3, connected: Body, connected_anchor: V3, spring: f32, damper: f32, distance: [f32; 2], collision: bool, mass_scale: f32) {
        unsafe {
            let a0 = self.actor(own) as *mut PxRigidActor;
            let a1 = self.actor(connected) as *mut PxRigidActor;
            let joint = phys_PxDistanceJointCreate(self.physics, a0, &pose(anchor, [0.0, 0.0, 0.0, 1.0]), a1, &pose(connected_anchor, [0.0, 0.0, 0.0, 1.0]));
            PxDistanceJoint_setMinDistance_mut(joint, distance[0]);
            PxDistanceJoint_setMaxDistance_mut(joint, distance[1]);
            PxDistanceJoint_setStiffness_mut(joint, spring);
            PxDistanceJoint_setDamping_mut(joint, damper);
            PxDistanceJoint_setDistanceJointFlag_mut(joint, PxDistanceJointFlag::eSPRING_ENABLED, true);
            PxDistanceJoint_setDistanceJointFlag_mut(joint, PxDistanceJointFlag::eMAX_DISTANCE_ENABLED, true);
            let joint = joint as *mut PxJoint;
            PxJoint_setConstraintFlag_mut(joint, PxConstraintFlag::eCOLLISION_ENABLED, collision);
            PxJoint_setConstraintFlag_mut(joint, PxConstraintFlag::eDISABLE_PREPROCESSING, true);
            PxJoint_setInvMassScale0_mut(joint, mass_scale);
            PxJoint_setInvInertiaScale0_mut(joint, mass_scale);
            self.joints.push(joint);
        }
    }

    /// Unity's fixed joint: the two held as they stand now.
    pub fn fixed_joint(&mut self, own: Body, connected: Body, mass_scale: f32) {
        unsafe {
            let a0 = self.actor(own) as *mut PxRigidActor;
            let a1 = self.actor(connected) as *mut PxRigidActor;
            let p0 = PxRigidActor_getGlobalPose(a0);
            let p1 = PxRigidActor_getGlobalPose(a1);
            let inverse = PxTransform_getInverse(&p1);
            let frame1 = PxTransform_transform_1(&inverse, &p0);
            let joint = phys_PxFixedJointCreate(self.physics, a0, &PxTransform_new_2(PxIdentity), a1, &frame1) as *mut PxJoint;
            PxJoint_setConstraintFlag_mut(joint, PxConstraintFlag::eDISABLE_PREPROCESSING, true);
            PxJoint_setInvMassScale0_mut(joint, mass_scale);
            PxJoint_setInvInertiaScale0_mut(joint, mass_scale);
            self.joints.push(joint);
        }
    }
}
