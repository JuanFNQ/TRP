//! Stepping a world, pushing bodies, reading poses, masses and touches.

use super::*;

impl World {
    /// One step of the engine, by `dt` (the game steps once a drawn frame,
    /// by its frames' average time: `PhysicsExtensions.Simulation`).
    pub fn step(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        unsafe {
            PxScene_simulate_mut(self.scene, dt, null_mut(), null_mut(), 0, true);
            let mut error = 0u32;
            PxScene_fetchResults_mut(self.scene, true, &mut error);
        }
    }

    /// A push at a point of the world (`AddForceAtPosition`, as an
    /// impulse).
    pub fn impulse_at(&mut self, body: Body, impulse: V3, point: V3) {
        unsafe { PxRigidBodyExt_addForceAtPos_mut(self.actor(body) as *mut PxRigidBody, &vec3(impulse), &vec3(point), PxForceMode::eIMPULSE, true) }
    }

    pub fn pose(&self, body: Body) -> (V3, Q4) {
        unsafe {
            let p = PxRigidActor_getGlobalPose(self.actor(body) as *const PxRigidActor);
            ([p.p.x, p.p.y, p.p.z], [p.q.x, p.q.y, p.q.z, p.q.w])
        }
    }

    pub fn velocity(&self, body: Body) -> (V3, V3) {
        unsafe {
            let b = self.actor(body) as *const PxRigidBody;
            let v = PxRigidBody_getLinearVelocity(b);
            let w = PxRigidBody_getAngularVelocity(b);
            ([v.x, v.y, v.z], [w.x, w.y, w.z])
        }
    }

    /// A body put somewhere at once, as setting its transform does.
    pub fn set_pose(&mut self, body: Body, at: V3, turn: Q4) {
        unsafe { PxRigidActor_setGlobalPose_mut(self.actor(body) as *mut PxRigidActor, &pose(at, turn), true) }
    }

    pub fn asleep(&self, body: Body) -> bool {
        unsafe { PxRigidDynamic_isSleeping(self.actor(body)) }
    }

    /// The centre of mass and the turning a body was given, in its axes.
    pub fn mass_properties(&self, body: Body) -> (V3, V3) {
        unsafe {
            let b = self.actor(body) as *const PxRigidBody;
            let c = PxRigidBody_getCMassLocalPose(b);
            let i = PxRigidBody_getMassSpaceInertiaTensor(b);
            ([c.p.x, c.p.y, c.p.z], [i.x, i.y, i.z])
        }
    }

    /// The centre of mass, the turn of the inertia's axes (Unity's
    /// `inertiaTensorRotation`) and the inertia about them, in its axes.
    pub fn mass_frame(&self, body: Body) -> (V3, Q4, V3) {
        unsafe {
            let b = self.actor(body) as *const PxRigidBody;
            let c = PxRigidBody_getCMassLocalPose(b);
            let i = PxRigidBody_getMassSpaceInertiaTensor(b);
            ([c.p.x, c.p.y, c.p.z], [c.q.x, c.q.y, c.q.z, c.q.w], [i.x, i.y, i.z])
        }
    }

    /// A body taken out of the world (its physics done: the game destroys
    /// the rigid body, `LootItem.StopPhysics`), each joint on it released
    /// first, as Unity has a joint destroyed before its rigid body (a
    /// corpse's weapon taken: `CorpseRagdoll.ClearWeapon`).
    pub fn remove(&mut self, body: Body) {
        if let Some(actor) = self.bodies[body.0].take() {
            unsafe {
                let own = actor as *mut PxRigidActor;
                self.joints.retain(|&joint| {
                    let (mut a0, mut a1) = (null_mut::<PxRigidActor>(), null_mut::<PxRigidActor>());
                    PxJoint_getActors(joint, &mut a0, &mut a1);
                    let on = a0 == own || a1 == own;
                    if on {
                        PxJoint_release_mut(joint);
                    }
                    !on
                });
                PxScene_removeActor_mut(self.scene, actor as *mut PxActor, true);
                PxRigidActor_release_mut(actor as *mut PxRigidActor);
            }
        }
    }

    /// A body from now on reports what it begins to touch
    /// (`take_touches`), as a body with a script that hears its collisions
    /// does in Unity. Its pairs are filtered again, so that one touching
    /// already is reported as it touches anew.
    pub fn report_touches(&mut self, body: Body) {
        unsafe {
            let actor = self.actor(body) as *mut PxRigidActor;
            let count = PxRigidActor_getNbShapes(actor);
            let mut shapes = vec![null_mut::<PxShape>(); count as usize];
            PxRigidActor_getShapes(actor, shapes.as_mut_ptr(), count, 0);
            for shape in shapes {
                let mut filter = PxShape_getSimulationFilterData(shape);
                filter.word1 |= REPORTS_TOUCHES;
                PxShape_setSimulationFilterData_mut(shape, &filter);
            }
            PxScene_resetFiltering_mut(self.scene, actor as *mut PxActor);
        }
    }

    /// The touches reported since they were last taken, in the order the
    /// engine found them.
    pub fn take_touches(&mut self) -> Vec<Touch> {
        std::mem::take(&mut *self.touches)
    }

    pub fn body_count(&self) -> usize {
        self.bodies.iter().filter(|b| b.is_some()).count()
    }
}
