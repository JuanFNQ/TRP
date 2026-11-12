//! Sweeping spheres and boxes through a world, and kinematic bodies.

use super::*;

impl World {
    /// The first thing a sphere meets going from `origin` along the unit
    /// way `dir` within `distance`, among every shape of the world, still
    /// or moving (`PxSceneQueryExt::sweepSingle` with the normal and the
    /// least way out of a shape it starts in, as the engine sweeps a
    /// particle: `PhysXRaycast::ParticleIntersect`, its flags `eNORMAL |
    /// eMTD`): the distance along the way (below nought where it starts in
    /// a shape, by how far) and the surface's normal.
    pub fn sweep_sphere(&self, origin: V3, dir: V3, distance: f32, radius: f32) -> Option<(f32, V3)> {
        self.sweep_sphere_met(origin, dir, distance, radius).map(|(d, n, _)| (d, n))
    }

    /// The same, with what it met.
    pub fn sweep_sphere_met(&self, origin: V3, dir: V3, distance: f32, radius: f32) -> Option<(f32, V3, Option<Met>)> {
        self.sweep_sphere_among(origin, dir, distance, radius, false)
    }

    /// The same among the still shapes alone (the query's flags `eSTATIC`,
    /// not `eDYNAMIC`: the engine's sweep for a particle system meeting the
    /// world at medium or low quality, `CollisionModule::Update` forcing
    /// `collidesWithDynamic` off there; particles spec, "Meeting the world
    /// at medium and low quality").
    pub fn sweep_sphere_still(&self, origin: V3, dir: V3, distance: f32, radius: f32) -> Option<(f32, V3, Option<Met>)> {
        self.sweep_sphere_among(origin, dir, distance, radius, true)
    }

    /// The nearest thing a box meets going from its place along the unit way
    /// `dir` within `distance`, among every shape of the world (Unity's
    /// `Physics.BoxCast`, the single cast): the distance along the way, the
    /// surface's normal and the point met. `half` is the box's half size,
    /// `turn` its turn. PhysX's sweep with the place and the normal asked
    /// for. A knife's blade is swept for every touch in the engine's order
    /// instead (`sweep_box_touches`), as the game's is; the flags Unity
    /// passes there, read in the engine (`BoxCastInternal`), add the faces'
    /// coordinates and, where queries hit back faces, a mesh's both sides,
    /// which nothing here asks.
    pub fn sweep_box(&self, centre: V3, half: V3, turn: Q4, dir: V3, distance: f32) -> Option<(f32, V3, V3)> {
        unsafe {
            let geometry = PxBoxGeometry_new_1(half[0].max(1e-6), half[1].max(1e-6), half[2].max(1e-6));
            let mut hit = PxSweepHit_new();
            let filter = PxQueryFilterData_new();
            let flags = PxSceneQueryFlags { mBits: (PxHitFlag::ePOSITION | PxHitFlag::eNORMAL) as u16 };
            let found = PxSceneQueryExt_sweepSingle_mut(
                self.scene,
                &geometry as *const PxBoxGeometry as *const PxGeometry,
                &pose(centre, turn),
                &vec3(dir),
                distance.max(0.0),
                flags,
                &mut hit,
                &filter as *const PxQueryFilterData as *const PxSceneQueryFilterData,
                null_mut(),
                std::ptr::null(),
                0.0,
            );
            found.then(|| (hit.distance, [hit.normal.x, hit.normal.y, hit.normal.z], [hit.position.x, hit.position.y, hit.position.z]))
        }
    }

    /// Every shape a box touches going from its place along the unit way
    /// `dir` within `distance`, in the order the engine finds them (Unity's
    /// `Physics.BoxCastNonAlloc`, which a knife's blade is swept by: read in
    /// the engine itself, `BoxCastInternal` asks PhysX for touches alone,
    /// its filter's hit type 1, and `SweepCallbackNonAlloc` hands them on in
    /// the order given). PhysX's order is its query trees': the still
    /// things' first, then the moving bodies', a kinematic one's among them
    /// (`NpSceneQueries::multiQuery`), each tree visited nearer child first
    /// (`AABBTreeRaycast`: "push the one with the further center on the
    /// stack for later"), so not by distance; a shape touched once, at its
    /// first contact. What a box begins inside comes at nought, its normal
    /// against the way. The place and the normal asked for (Unity's flags
    /// add the faces' coordinates, which nothing here reads).
    pub fn sweep_box_touches(&self, centre: V3, half: V3, turn: Q4, dir: V3, distance: f32) -> Vec<SweepTouch> {
        let mut kept: Vec<PxSweepHit> = Vec::new();
        unsafe {
            let geometry = PxBoxGeometry_new_1(half[0].max(1e-6), half[1].max(1e-6), half[2].max(1e-6));
            let mut buffer: Vec<PxSweepHit> = (0..TOUCHES_AT_ONCE).map(|_| PxSweepHit_new()).collect();
            let callback = create_sweep_callback(keep_touches, touches_done, buffer.as_mut_ptr(), TOUCHES_AT_ONCE as u32, &mut kept as *mut Vec<PxSweepHit> as *mut std::ffi::c_void);
            let filter = PxQueryFilterData_new();
            let flags = PxHitFlags { mBits: (PxHitFlag::ePOSITION | PxHitFlag::eNORMAL) as u16 };
            PxScene_sweep(
                self.scene,
                &geometry as *const PxBoxGeometry as *const PxGeometry,
                &pose(centre, turn),
                &vec3(dir),
                distance.max(0.0),
                callback,
                flags,
                &filter,
                null_mut(),
                std::ptr::null(),
                0.0,
            );
            delete_sweep_callback(callback);
            kept.iter()
                .map(|hit| SweepTouch {
                    distance: hit.distance,
                    normal: [hit.normal.x, hit.normal.y, hit.normal.z],
                    point: [hit.position.x, hit.position.y, hit.position.z],
                    met: tagged(hit.actor),
                    layer: if hit.actor.is_null() { 0 } else { phys_PxGetGroup(hit.actor as *const PxActor) },
                })
                .collect()
        }
    }

    /// A body the engine moves only where it is put (a kinematic rigid body,
    /// Unity's `isKinematic`), on a layer: a living character's hit colliders
    /// ride such bodies (every ragdoll bone's is made kinematic with the
    /// character's object, `PlayerPoolObject.CreateRigidbodiesForRagdoll`,
    /// and its root carries one, `CharacterControllerSpawner.CreateRigidbody`),
    /// so a query meets them among the moving bodies, after the still things
    /// (`sweep_box_touches`). Moved with `set_pose`.
    pub fn add_kinematic(&mut self, at: V3, turn: Q4, colliders: &[Collider], layer: u16) -> Body {
        unsafe {
            let actor = PxPhysics_createRigidDynamic_mut(self.physics, &pose(at, turn));
            for c in colliders {
                let shape = self.shape(c);
                if shape.is_null() {
                    continue;
                }
                PxRigidActor_attachShape_mut(actor as *mut PxRigidActor, shape);
                PxShape_release_mut(shape);
            }
            PxRigidBody_setRigidBodyFlag_mut(actor as *mut PxRigidBody, PxRigidBodyFlag::eKINEMATIC, true);
            phys_PxSetGroup(actor as *mut PxActor, layer);
            (*(actor as *mut PxActor)).userData = tag(TAG_BODY, self.bodies.len());
            PxScene_addActor_mut(self.scene, actor as *mut PxActor, null_mut());
            self.bodies.push(Some(actor));
            Body(self.bodies.len() - 1)
        }
    }

    /// The engine's query trees built again at once, the still things' and
    /// the moving bodies' (`PxScene::forceDynamicTreeRebuild`): what was put
    /// in waits in a list of its own until the engine's steps have built its
    /// tree, and a query visits that list after the tree, in an order of its
    /// own. A world that is never stepped builds them so.
    pub fn build_query_trees(&mut self) {
        unsafe { PxScene_forceDynamicTreeRebuild_mut(self.scene, true, true) }
    }

    fn sweep_sphere_among(&self, origin: V3, dir: V3, distance: f32, radius: f32, still_only: bool) -> Option<(f32, V3, Option<Met>)> {
        unsafe {
            let geometry = PxSphereGeometry_new_1(radius.max(1e-6));
            let mut hit = PxSweepHit_new();
            let filter = if still_only { PxQueryFilterData_new_2(PxQueryFlags { mBits: PxQueryFlag::eSTATIC as u16 }) } else { PxQueryFilterData_new() };
            let flags = PxSceneQueryFlags { mBits: (PxHitFlag::eNORMAL | PxHitFlag::eMTD) as u16 };
            let found = PxSceneQueryExt_sweepSingle_mut(
                self.scene,
                &geometry as *const PxSphereGeometry as *const PxGeometry,
                &pose(origin, [0.0, 0.0, 0.0, 1.0]),
                &vec3(dir),
                distance.max(0.0),
                flags,
                &mut hit,
                &filter as *const PxQueryFilterData as *const PxSceneQueryFilterData,
                null_mut(),
                std::ptr::null(),
                0.0,
            );
            found.then(|| (hit.distance, [hit.normal.x, hit.normal.y, hit.normal.z], tagged(hit.actor)))
        }
    }
}
