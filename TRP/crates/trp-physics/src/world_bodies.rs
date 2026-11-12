//! Making a world and adding its static, floor, plane and moving bodies.

use super::*;

impl World {
    pub fn new(settings: Settings) -> Self {
        unsafe {
            let physics = engine().physics;
            let mut desc = PxSceneDesc_new(PxPhysics_getTolerancesScale(physics));
            desc.gravity = vec3(settings.gravity);
            // One thread: the same order of work every step.
            let dispatcher = phys_PxDefaultCpuDispatcherCreate(0, null_mut());
            desc.cpuDispatcher = dispatcher as *mut PxCpuDispatcher;
            enable_custom_filter_shader(&mut desc, layer_filter, 0);
            let mut touches: Box<Vec<Touch>> = Box::default();
            let told = SimulationEventCallbackInfo { collision_callback: Some(on_contact), collision_user_data: &mut *touches as *mut Vec<Touch> as *mut std::ffi::c_void, ..Default::default() };
            let events = create_simulation_event_callbacks(&told);
            desc.simulationEventCallback = events;
            desc.bounceThresholdVelocity = settings.bounce_threshold;
            desc.frictionType = settings.friction_type;
            desc.solverType = settings.solver_type;
            let mut flags = desc.flags.mBits;
            if settings.pcm {
                flags |= PxSceneFlag::eENABLE_PCM;
            } else {
                flags &= !PxSceneFlag::eENABLE_PCM;
            }
            if settings.adaptive_force {
                flags |= PxSceneFlag::eADAPTIVE_FORCE;
            } else {
                flags &= !PxSceneFlag::eADAPTIVE_FORCE;
            }
            if settings.enhanced_determinism {
                flags |= PxSceneFlag::eENABLE_ENHANCED_DETERMINISM;
            }
            // Unity's scenes allow continuous detection for the bodies that
            // ask for it.
            flags |= PxSceneFlag::eENABLE_CCD;
            desc.flags.mBits = flags;
            let scene = PxPhysics_createScene_mut(physics, &desc);
            // The layers: PhysX's 32 groups, meeting as the matrix says.
            for a in 0..32u16 {
                for b in 0..32u16 {
                    phys_PxSetGroupCollisionFlag(a, b, settings.layer_matrix[a as usize] >> b & 1 == 1);
                }
            }
            Self { physics, dispatcher, scene, settings, convention: JointConvention::GAME, bodies: Vec::new(), statics: Vec::new(), joints: Vec::new(), materials: Vec::new(), touches, events }
        }
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Whether two layers meet (the layer matrix: all meet until told).
    pub fn set_layers_meet(&mut self, a: u16, b: u16, meet: bool) {
        unsafe { phys_PxSetGroupCollisionFlag(a, b, meet) }
    }

    pub(super) fn material(&mut self, m: Material) -> *mut PxMaterial {
        if let Some((_, px)) = self.materials.iter().find(|(known, _)| *known == m) {
            return *px;
        }
        unsafe {
            let px = PxPhysics_createMaterial_mut(self.physics, m.static_friction, m.dynamic_friction, m.bounciness);
            PxMaterial_setFrictionCombineMode_mut(px, m.friction_combine.px());
            PxMaterial_setRestitutionCombineMode_mut(px, m.bounce_combine.px());
            if self.settings.improved_patch_friction {
                PxMaterial_setFlag_mut(px, PxMaterialFlag::eIMPROVED_PATCH_FRICTION, true);
            }
            self.materials.push((m, px));
            px
        }
    }

    pub(super) fn shape(&mut self, c: &Collider) -> *mut PxShape {
        let material = self.material(c.material);
        unsafe {
            let flags = PxShapeFlags { mBits: (PxShapeFlag::eSIMULATION_SHAPE | PxShapeFlag::eSCENE_QUERY_SHAPE) as u8 };
            // PhysX's capsule lies along x; Unity's along its direction.
            let (shape, along) = match c.shape {
                Shape::Box { half } => {
                    let g = PxBoxGeometry_new_1(half[0], half[1], half[2]);
                    (PxPhysics_createShape_mut(self.physics, &g as *const _ as *const PxGeometry, material, true, flags), [0.0, 0.0, 0.0, 1.0])
                }
                Shape::Sphere { radius } => {
                    let g = PxSphereGeometry_new_1(radius);
                    (PxPhysics_createShape_mut(self.physics, &g as *const _ as *const PxGeometry, material, true, flags), [0.0, 0.0, 0.0, 1.0])
                }
                // A capsule no taller than its two caps is a sphere (Unity
                // makes it one; PhysX will not take it).
                Shape::Capsule { radius, height, .. } if height * 0.5 - radius <= 1e-5 => {
                    let g = PxSphereGeometry_new_1(radius);
                    (PxPhysics_createShape_mut(self.physics, &g as *const _ as *const PxGeometry, material, true, flags), [0.0, 0.0, 0.0, 1.0])
                }
                Shape::Capsule { radius, height, direction } => {
                    let half = (height * 0.5) - radius;
                    let g = PxCapsuleGeometry_new_1(radius, half);
                    let s = std::f32::consts::FRAC_1_SQRT_2;
                    let along = match direction {
                        1 => [0.0, 0.0, s, s],
                        2 => [0.0, -s, 0.0, s],
                        _ => [0.0, 0.0, 0.0, 1.0],
                    };
                    (PxPhysics_createShape_mut(self.physics, &g as *const _ as *const PxGeometry, material, true, flags), along)
                }
                Shape::Convex { ref points, scale } => {
                    let mut desc = PxConvexMeshDesc_new();
                    desc.points.count = points.len() as u32;
                    desc.points.stride = std::mem::size_of::<V3>() as u32;
                    desc.points.data = points.as_ptr() as *const std::ffi::c_void;
                    desc.flags = PxConvexFlags { mBits: PxConvexFlag::eCOMPUTE_CONVEX as u16 };
                    let mut result = PxConvexMeshCookingResult::eSUCCESS;
                    let mesh = PxCooking_createConvexMesh(engine().cooking, &desc, PxPhysics_getPhysicsInsertionCallback_mut(self.physics), &mut result);
                    if mesh.is_null() {
                        return null_mut();
                    }
                    let g = PxConvexMeshGeometry_new_1(mesh, &PxMeshScale_new_2(&vec3(scale)), PxConvexMeshGeometryFlags { mBits: 0 });
                    let shape = PxPhysics_createShape_mut(self.physics, &g as *const _ as *const PxGeometry, material, true, flags);
                    // The shape holds the mesh now.
                    PxConvexMesh_release_mut(mesh);
                    (shape, [0.0, 0.0, 0.0, 1.0])
                }
                Shape::Mesh { ref points, ref triangles, scale } => {
                    if points.is_empty() || triangles.is_empty() {
                        return null_mut();
                    }
                    let mut desc = PxTriangleMeshDesc_new();
                    desc.points.count = points.len() as u32;
                    desc.points.stride = std::mem::size_of::<V3>() as u32;
                    desc.points.data = points.as_ptr() as *const std::ffi::c_void;
                    desc.triangles.count = triangles.len() as u32;
                    desc.triangles.stride = std::mem::size_of::<[u32; 3]>() as u32;
                    desc.triangles.data = triangles.as_ptr() as *const std::ffi::c_void;
                    let mut result = PxTriangleMeshCookingResult::eSUCCESS;
                    let mesh = PxCooking_createTriangleMesh(engine().cooking, &desc, PxPhysics_getPhysicsInsertionCallback_mut(self.physics), &mut result);
                    if mesh.is_null() {
                        return null_mut();
                    }
                    let g = PxTriangleMeshGeometry_new_1(mesh, &PxMeshScale_new_2(&vec3(scale)), PxMeshGeometryFlags { mBits: 0 });
                    let shape = PxPhysics_createShape_mut(self.physics, &g as *const _ as *const PxGeometry, material, true, flags);
                    PxTriangleMesh_release_mut(mesh);
                    (shape, [0.0, 0.0, 0.0, 1.0])
                }
            };
            if shape.is_null() {
                return shape;
            }
            let turn = mul(c.turn, along);
            PxShape_setLocalPose_mut(shape, &pose(c.at, turn));
            PxShape_setContactOffset_mut(shape, self.settings.contact_offset);
            PxShape_setRestOffset_mut(shape, 0.0);
            shape
        }
    }

    /// Something of the world that does not move, on a layer.
    pub fn add_static(&mut self, at: V3, turn: Q4, colliders: &[Collider], layer: u16) {
        unsafe {
            let actor = PxPhysics_createRigidStatic_mut(self.physics, &pose(at, turn));
            for c in colliders {
                let shape = self.shape(c);
                if shape.is_null() {
                    continue;
                }
                PxRigidActor_attachShape_mut(actor as *mut PxRigidActor, shape);
                PxShape_release_mut(shape);
            }
            phys_PxSetGroup(actor as *mut PxActor, layer);
            (*(actor as *mut PxActor)).userData = tag(TAG_STILL, self.statics.len());
            PxScene_addActor_mut(self.scene, actor as *mut PxActor, null_mut());
            self.statics.push(actor);
        }
    }

    /// The ground: a floor at a height, facing up (`add_floor`).
    pub fn add_ground(&mut self, height: f32, material: Material, layer: u16) {
        self.add_floor([0.0, height, 0.0], [0.0, 1.0, 0.0], material, layer);
    }

    /// A floor as a level's is, a mesh (`FLOOR_HALF` across each way,
    /// squares of `FLOOR_CELL`), through a point, facing along a normal.
    /// Not a plane: PhysX never sweeps a body against a plane (its
    /// threshold for continuous detection is infinite there,
    /// `Gu::computeBoundsWithCCDThreshold`) but does against a mesh or a
    /// terrain (nought), so a swept thing meets a plane later than the
    /// game's floors and turns as it meets it (`docs/spec/physics.md`,
    /// "Loose things"). The mesh is cooked once and shared.
    pub fn add_floor(&mut self, point: V3, normal: V3, material: Material, layer: u16) {
        let n = norm(normal);
        // The turn that takes up to the normal.
        let (axis, cos) = ([n[2], 0.0, -n[0]], n[1]);
        let turn = if dot(axis, axis) < 1e-12 {
            if cos > 0.0 { [0.0, 0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0, 0.0] }
        } else {
            let w = 1.0 + cos;
            let l = (dot(axis, axis) + w * w).sqrt();
            [axis[0] / l, axis[1] / l, axis[2] / l, w / l]
        };
        let m = self.material(material);
        unsafe {
            let flags = PxShapeFlags { mBits: (PxShapeFlag::eSIMULATION_SHAPE | PxShapeFlag::eSCENE_QUERY_SHAPE) as u8 };
            let g = PxTriangleMeshGeometry_new_1(floor_mesh(), &PxMeshScale_new_2(&vec3([1.0; 3])), PxMeshGeometryFlags { mBits: 0 });
            let shape = PxPhysics_createShape_mut(self.physics, &g as *const _ as *const PxGeometry, m, true, flags);
            if shape.is_null() {
                return;
            }
            PxShape_setContactOffset_mut(shape, self.settings.contact_offset);
            PxShape_setRestOffset_mut(shape, 0.0);
            let actor = PxPhysics_createRigidStatic_mut(self.physics, &pose(point, turn));
            PxRigidActor_attachShape_mut(actor as *mut PxRigidActor, shape);
            PxShape_release_mut(shape);
            phys_PxSetGroup(actor as *mut PxActor, layer);
            (*(actor as *mut PxActor)).userData = tag(TAG_STILL, self.statics.len());
            PxScene_addActor_mut(self.scene, actor as *mut PxActor, null_mut());
            self.statics.push(actor);
        }
    }

    /// A plane through a point, facing along a normal (a slope's ground).
    pub fn add_plane(&mut self, point: V3, normal: V3, material: Material, layer: u16) {
        let n = norm(normal);
        unsafe {
            let m = self.material(material);
            let plane = phys_PxCreatePlane(self.physics, &PxPlane_new_1(n[0], n[1], n[2], -dot(n, point)), m);
            let mut shape: *mut PxShape = null_mut();
            PxRigidActor_getShapes(plane as *const PxRigidActor, &mut shape, 1, 0);
            if !shape.is_null() {
                PxShape_setContactOffset_mut(shape, self.settings.contact_offset);
                PxShape_setRestOffset_mut(shape, 0.0);
            }
            phys_PxSetGroup(plane as *mut PxActor, layer);
            (*(plane as *mut PxActor)).userData = tag(TAG_STILL, self.statics.len());
            PxScene_addActor_mut(self.scene, plane as *mut PxActor, null_mut());
            self.statics.push(plane);
        }
    }

    /// A rigid body as Unity makes one: its mass given, its centre of mass
    /// and its turning worked out from its colliders
    /// (`setMassAndUpdateInertia`), the engine's iterations, sleep and
    /// spin, its drags, and set going.
    pub fn add_body(&mut self, desc: &BodyDesc) -> Body {
        unsafe {
            let actor = PxPhysics_createRigidDynamic_mut(self.physics, &pose(desc.at, desc.turn));
            for c in &desc.colliders {
                let shape = self.shape(c);
                if shape.is_null() {
                    continue;
                }
                PxRigidActor_attachShape_mut(actor as *mut PxRigidActor, shape);
                PxShape_release_mut(shape);
            }
            let body = actor as *mut PxRigidBody;
            PxRigidBodyExt_setMassAndUpdateInertia_mut_1(body, desc.mass, std::ptr::null(), false);
            PxRigidBody_setLinearDamping_mut(body, desc.drag);
            PxRigidBody_setAngularDamping_mut(body, desc.angular_drag);
            PxRigidBody_setMaxAngularVelocity_mut(body, self.settings.max_angular_speed);
            PxRigidBody_setMaxDepenetrationVelocity_mut(body, desc.max_depenetration);
            PxRigidDynamic_setSolverIterationCounts_mut(actor, self.settings.solver[0], self.settings.solver[1]);
            PxRigidDynamic_setSleepThreshold_mut(actor, self.settings.sleep_threshold);
            if desc.continuous {
                PxRigidBody_setRigidBodyFlag_mut(body, PxRigidBodyFlag::eENABLE_CCD, true);
            }
            phys_PxSetGroup(actor as *mut PxActor, desc.layer);
            (*(actor as *mut PxActor)).userData = tag(TAG_BODY, self.bodies.len());
            PxScene_addActor_mut(self.scene, actor as *mut PxActor, null_mut());
            PxRigidBody_setLinearVelocity_mut(body, &vec3(desc.velocity), true);
            PxRigidBody_setAngularVelocity_mut(body, &vec3(desc.spin), true);
            self.bodies.push(Some(actor));
            Body(self.bodies.len() - 1)
        }
    }

    pub(super) fn actor(&self, body: Body) -> *mut PxRigidDynamic {
        self.bodies[body.0].expect("a body taken away")
    }
}
