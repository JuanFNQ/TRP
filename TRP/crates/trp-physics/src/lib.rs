//! The game's physics engine, PhysX 4.1 (the engine of its Unity, 2022.3),
//! set up as that Unity sets it up from the game's settings
//! (`globalgamemanagers`: `probe layers`), for what the game hands to it: a
//! corpse's ragdoll, things let fall and thrown (health spec, "Death";
//! `docs/spec/physics.md`).
//!
//! Everything here is in the game's own axes (Unity's: y up, z ahead, the
//! numbers as the game has them): the app turns its own into these at the
//! door. Rotations are quaternions `[x, y, z, w]`.
//!
//! How Unity drives PhysX is not published; what this does with a Unity
//! setting is written beside it, and whether it holds is measured against
//! the recordings (`bin/corpse`).

use std::ptr::null_mut;

use physx_sys::*;

pub use physx_sys as sys;

pub mod corpse;
pub mod loose;
pub mod recorded;

pub type V3 = [f32; 3];
pub type Q4 = [f32; 4];

/// The engine's settings, as the game's build has them.
#[derive(Debug, Clone, Copy)]
pub struct Settings {
    pub gravity: V3,
    /// Contacts slower than this do not bounce (`m_BounceThreshold`).
    pub bounce_threshold: f32,
    /// A body's solver iterations for position and velocity
    /// (`m_DefaultSolverIterations`, `...VelocityIterations`).
    pub solver: [u32; 2],
    /// Mass-normalised kinetic energy under which a body may sleep.
    pub sleep_threshold: f32,
    /// Every shape's contact offset.
    pub contact_offset: f32,
    /// Every body's most spin, radians a second (`m_DefaultMaxAngularSpeed`).
    pub max_angular_speed: f32,
    /// A new body's most speed out of what it is in
    /// (`m_DefaultMaxDepenetrationVelocity`).
    pub max_depenetration: f32,
    /// Persistent contact manifolds (`m_ContactsGeneration` 1).
    pub pcm: bool,
    pub adaptive_force: bool,
    /// Which layer meets which (`m_LayerCollisionMatrix`): a mask a layer.
    pub layer_matrix: [u32; 32],
    /// PhysX's friction type and solver (`m_FrictionType`, `m_SolverType`),
    /// enhanced determinism and improved patch friction.
    pub friction_type: u32,
    pub solver_type: u32,
    pub enhanced_determinism: bool,
    pub improved_patch_friction: bool,
}

impl Settings {
    /// As the install's physics settings have it (`globalgamemanagers`).
    pub fn of(p: &trp_data::engine::PhysicsSettings) -> Self {
        Self {
            gravity: p.gravity,
            bounce_threshold: p.bounce_threshold,
            solver: [p.solver_iterations, p.solver_velocity_iterations],
            sleep_threshold: p.sleep_threshold,
            contact_offset: p.contact_offset,
            max_angular_speed: p.max_angular_speed,
            max_depenetration: p.default_max_depenetration,
            pcm: p.pcm,
            adaptive_force: p.adaptive_force,
            layer_matrix: p.layer_matrix,
            friction_type: p.friction_type,
            solver_type: p.solver_type,
            enhanced_determinism: p.enhanced_determinism,
            improved_patch_friction: p.improved_patch_friction,
        }
    }

    /// The figures of the install read on 2026-10-15 (`probe layers`):
    /// gravity -9.81, bounce threshold 2, 3 and 1 solver iterations, sleep
    /// 0.005, contact offset 0.01, spin at most 7, a new body out of what
    /// it is in at 10 m/s at most, PCM on, adaptive force
    /// off; patch friction, the PGS solver and no enhanced determinism are
    /// PhysX's own defaults.
    pub fn game() -> Self {
        Self {
            gravity: [0.0, -9.81, 0.0],
            bounce_threshold: 2.0,
            solver: [3, 1],
            sleep_threshold: 0.005,
            contact_offset: 0.01,
            max_angular_speed: 7.0,
            max_depenetration: 10.0,
            pcm: true,
            adaptive_force: false,
            layer_matrix: [u32::MAX; 32],
            friction_type: 0,
            solver_type: 0,
            enhanced_determinism: false,
            improved_patch_friction: false,
        }
    }
}

/// How two figures of two touching materials are combined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combine {
    Average,
    Minimum,
    Multiply,
    Maximum,
}

impl Combine {
    fn px(self) -> PxCombineMode::Enum {
        match self {
            Combine::Average => PxCombineMode::eAVERAGE,
            Combine::Minimum => PxCombineMode::eMIN,
            Combine::Multiply => PxCombineMode::eMULTIPLY,
            Combine::Maximum => PxCombineMode::eMAX,
        }
    }
}

/// A material (`PhysicMaterial`). The engine's default is Unity's: 0.6 of
/// friction either way, no bounce, both averaged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    pub dynamic_friction: f32,
    pub static_friction: f32,
    pub bounciness: f32,
    pub friction_combine: Combine,
    pub bounce_combine: Combine,
}

impl Default for Material {
    fn default() -> Self {
        Self { dynamic_friction: 0.6, static_friction: 0.6, bounciness: 0.0, friction_combine: Combine::Average, bounce_combine: Combine::Average }
    }
}

/// A collider's shape in its own axes.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Box { half: V3 },
    Sphere { radius: f32 },
    /// Along x, y or z (0, 1, 2), its height end to end with the caps.
    Capsule { radius: f32, height: f32, direction: u8 },
    /// A convex `MeshCollider`: the hull of its mesh's points, scaled as
    /// its object is (Unity cooks the hull when the collider is made; the
    /// cooking's settings here are PhysX's own, `PROVISIONAL`).
    Convex { points: std::sync::Arc<[V3]>, scale: V3 },
}

/// A collider of a body or of the world: its shape, where its own axes
/// stand in the body's (or the world's), its material and its layer.
#[derive(Debug, Clone, PartialEq)]
pub struct Collider {
    pub shape: Shape,
    pub at: V3,
    pub turn: Q4,
    pub material: Material,
}

/// A rigid body as Unity's `Rigidbody` is set up.
#[derive(Debug, Clone)]
pub struct BodyDesc {
    pub at: V3,
    pub turn: Q4,
    pub colliders: Vec<Collider>,
    pub mass: f32,
    /// `drag`, `angularDrag`.
    pub drag: f32,
    pub angular_drag: f32,
    pub velocity: V3,
    pub spin: V3,
    pub max_depenetration: f32,
    /// Continuous collision detection (`CollisionDetectionMode.Continuous`
    /// and the like).
    pub continuous: bool,
    /// Its layer (`gameObject.layer`), which goes by the layer matrix.
    pub layer: u16,
}

/// Unity's `CharacterJoint` between a body (`own`) and the one it hangs
/// from (`connected`), in the own body's axes, the angles in degrees.
#[derive(Debug, Clone, Copy)]
pub struct CharacterJointDesc {
    pub own: Body,
    pub connected: Body,
    pub anchor: V3,
    pub axis: V3,
    pub swing_axis: V3,
    pub low_twist: f32,
    pub high_twist: f32,
    pub swing1: f32,
    pub swing2: f32,
    /// Each limit's bounciness (low twist, high twist, swing1, swing2).
    pub bounciness: [f32; 4],
    /// `massScale`, `connectedMassScale`.
    pub mass_scale: f32,
    pub connected_mass_scale: f32,
    /// `enableProjection` with its distance and angle (degrees).
    pub projection: Option<[f32; 2]>,
    pub preprocessing: bool,
    pub collision: bool,
    /// Where the own body stands in the connected one's axes at the
    /// skeleton's rest (its place and turn): the limits count from there,
    /// as the recordings show the game's do; without it, from how the two
    /// stand when the joint is made.
    pub rest: Option<(V3, Q4)>,
}

/// How Unity's joint is laid on PhysX's, where it is not known: which
/// body is PhysX's first, and which way the twist is counted. Settled by
/// the recordings (`bin/corpse`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JointConvention {
    pub own_first: bool,
    pub twist_negated: bool,
    /// `swing1` as PhysX's cone angle about z, and `swing2` about y.
    pub swings_swapped: bool,
    /// The mass scales given to PhysX as their inverses.
    pub scales_inverted: bool,
}

impl JointConvention {
    pub const GAME: Self = Self { own_first: true, twist_negated: false, swings_swapped: false, scales_inverted: false };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Body(usize);

/// The engine itself, one for the whole program as PhysX allows (one
/// foundation a process): its foundation, its physics, which makes every
/// world's objects, and its cooking of convex meshes. Made when a world is
/// first wanted and kept to the end.
struct Engine {
    physics: *mut PxPhysics,
    cooking: *mut PxCooking,
}

// Made once and never changed; what it makes is reached through a world.
unsafe impl Send for Engine {}
unsafe impl Sync for Engine {}

fn engine() -> &'static Engine {
    static ENGINE: std::sync::OnceLock<Engine> = std::sync::OnceLock::new();
    ENGINE.get_or_init(|| unsafe {
        let foundation = physx_create_foundation();
        let physics = physx_create_physics(foundation);
        phys_PxInitExtensions(physics, null_mut());
        let params = PxCookingParams_new(PxPhysics_getTolerancesScale(physics));
        // PhysX 4.1.1's version, as the cooking asks for it.
        let cooking = phys_PxCreateCooking((4 << 24) + (1 << 16) + (1 << 8), foundation, &params);
        Engine { physics, cooking }
    })
}

/// Which pairs of shapes touch (Unity's own filter, as far as it shows):
/// those whose layers meet in the matrix (PhysX's groups, set from it);
/// a trigger is told of what enters it and pushes nothing; every other
/// pair touches, and a body that asks for continuous detection is swept
/// against it.
unsafe extern "C" fn layer_filter(info: *mut FilterShaderCallbackInfo) -> u16 {
    let info = &mut *info;
    if !phys_PxGetGroupCollisionFlag(info.filterData0.word0 as u16, info.filterData1.word0 as u16) {
        return PxFilterFlag::eSUPPRESS as u16;
    }
    (*info.pairFlags).mBits = if phys_PxFilterObjectIsTrigger(info.attributes0) || phys_PxFilterObjectIsTrigger(info.attributes1) {
        PxPairFlag::eTRIGGER_DEFAULT as u16
    } else {
        // A pair with a body that reports its touches is told of when it
        // first touches, and where (asked for alone: it changes nothing of
        // the pair's motion).
        let report = (info.filterData0.word1 | info.filterData1.word1) & REPORTS_TOUCHES != 0;
        let told = if report { PxPairFlag::eNOTIFY_TOUCH_FOUND | PxPairFlag::eNOTIFY_TOUCH_CCD | PxPairFlag::eNOTIFY_CONTACT_POINTS } else { 0 };
        (PxPairFlag::eCONTACT_DEFAULT | PxPairFlag::eDETECT_CCD_CONTACT | told) as u16
    };
    PxFilterFlag::eDEFAULT as u16
}

/// The mark on a shape's filter (its second word) of a body that reports
/// its touches (`World::report_touches`).
const REPORTS_TOUCHES: u32 = 1;

/// What an actor is to its world, kept in its user data: a body or a still
/// thing by its place, or nothing.
const TAG_BODY: usize = 1;
const TAG_STILL: usize = 2;

fn tag(kind: usize, index: usize) -> *mut std::ffi::c_void {
    ((index << 2) | kind) as *mut std::ffi::c_void
}

unsafe fn tagged(actor: *mut PxRigidActor) -> Option<Met> {
    if actor.is_null() {
        return None;
    }
    let tag = (*(actor as *mut PxActor)).userData as usize;
    match tag & 3 {
        TAG_BODY => Some(Met::Body(Body(tag >> 2))),
        TAG_STILL => Some(Met::Still(tag >> 2)),
        _ => None,
    }
}

/// What a body that reports its touches met, as the engine first found
/// them touching (Unity's `OnCollisionEnter`: once a pair of shapes, as
/// the pair begins to touch, by the discrete step or by a sweep;
/// `PROVISIONAL`: how Unity reports a touch a sweep found is not read, and
/// it is taken as a touch like any other).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Touch {
    pub body: Body,
    pub other: Met,
    /// The pair's first point of contact, and the normal there, pointing
    /// out of what it met towards the body; none where the engine gave no
    /// point.
    pub contact: Option<(V3, V3)>,
}

/// What was met: another body, or a still thing of the world by the order
/// it was put in (`add_static` and `add_ground` count together).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Met {
    Body(Body),
    Still(usize),
}

/// The engine's report of the pairs that began to touch in a step, kept as
/// touches of the bodies that asked (`World::take_touches`). Called from
/// within `World::step`, on its thread.
unsafe extern "C" fn on_contact(user: *mut std::ffi::c_void, header: *const PxContactPairHeader, pairs: *const PxContactPair, count: u32) {
    let (Some(touches), Some(header)) = ((user as *mut Vec<Touch>).as_mut(), header.as_ref()) else { return };
    let removed = (PxContactPairHeaderFlag::eREMOVED_ACTOR_0 | PxContactPairHeaderFlag::eREMOVED_ACTOR_1) as u16;
    if header.flags.mBits & removed != 0 || pairs.is_null() {
        return;
    }
    let sides = [tagged(header.actors[0]), tagged(header.actors[1])];
    let began = (PxPairFlag::eNOTIFY_TOUCH_FOUND | PxPairFlag::eNOTIFY_TOUCH_CCD) as u16;
    let gone = (PxContactPairFlag::eREMOVED_SHAPE_0 | PxContactPairFlag::eREMOVED_SHAPE_1) as u16;
    for pair in std::slice::from_raw_parts(pairs, count as usize) {
        if pair.events.mBits & began == 0 || pair.flags.mBits & gone != 0 {
            continue;
        }
        let mut point: PxContactPairPoint = std::mem::zeroed();
        let found = PxContactPair_extractContacts(pair, &mut point, 1) > 0;
        for side in 0..2 {
            let (Some(Met::Body(body)), Some(other)) = (sides[side], sides[1 - side]) else { continue };
            if PxShape_getSimulationFilterData(pair.shapes[side]).word1 & REPORTS_TOUCHES == 0 {
                continue;
            }
            // The engine's normal is the way the pair's first shape must
            // go to leave the second.
            let sign = if side == 0 { 1.0 } else { -1.0 };
            let contact = found.then(|| ([point.position.x, point.position.y, point.position.z], [point.normal.x * sign, point.normal.y * sign, point.normal.z * sign]));
            touches.push(Touch { body, other, contact });
        }
    }
}

/// A world of the engine's, stepped by hand as the game steps its own.
/// Several may stand at once (each its own scene); one world is used from
/// one thread at a time, and worlds are not made or changed at once on
/// two.
pub struct World {
    physics: *mut PxPhysics,
    dispatcher: *mut PxDefaultCpuDispatcher,
    scene: *mut PxScene,
    settings: Settings,
    pub convention: JointConvention,
    bodies: Vec<Option<*mut PxRigidDynamic>>,
    statics: Vec<*mut PxRigidStatic>,
    joints: Vec<*mut PxJoint>,
    materials: Vec<(Material, *mut PxMaterial)>,
    /// The engine's reports of touches go here (`on_contact`), behind a
    /// box so that their place stays as the world moves.
    touches: Box<Vec<Touch>>,
    events: *mut PxSimulationEventCallback,
}

// The engine's objects are only reached through `&mut World`.
unsafe impl Send for World {}
unsafe impl Sync for World {}

fn vec3(v: V3) -> PxVec3 {
    PxVec3 { x: v[0], y: v[1], z: v[2] }
}

fn quat(q: Q4) -> PxQuat {
    unsafe { PxQuat_new_3(q[0], q[1], q[2], q[3]) }
}

fn pose(at: V3, turn: Q4) -> PxTransform {
    unsafe { PxTransform_new_5(&vec3(at), &quat(turn)) }
}

/// The turn whose x, y and z axes are these (orthonormal, right-handed).
pub fn turn_of_axes(x: V3, y: V3, z: V3) -> Q4 {
    let (m00, m01, m02) = (x[0], y[0], z[0]);
    let (m10, m11, m12) = (x[1], y[1], z[1]);
    let (m20, m21, m22) = (x[2], y[2], z[2]);
    let trace = m00 + m11 + m22;
    let q = if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        [(m21 - m12) / s, (m02 - m20) / s, (m10 - m01) / s, 0.25 * s]
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        [0.25 * s, (m01 + m10) / s, (m02 + m20) / s, (m21 - m12) / s]
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        [(m01 + m10) / s, 0.25 * s, (m12 + m21) / s, (m02 - m20) / s]
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        [(m02 + m20) / s, (m12 + m21) / s, 0.25 * s, (m10 - m01) / s]
    };
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    [q[0] / n, q[1] / n, q[2] / n, q[3] / n]
}

fn norm(v: V3) -> V3 {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n > 1e-9 { [v[0] / n, v[1] / n, v[2] / n] } else { [0.0; 3] }
}

fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

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

    fn material(&mut self, m: Material) -> *mut PxMaterial {
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

    fn shape(&mut self, c: &Collider) -> *mut PxShape {
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

    /// The ground: a plane at a height, facing up.
    pub fn add_ground(&mut self, height: f32, material: Material, layer: u16) {
        unsafe {
            let m = self.material(material);
            let plane = phys_PxCreatePlane(self.physics, &PxPlane_new_1(0.0, 1.0, 0.0, -height), m);
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

    fn actor(&self, body: Body) -> *mut PxRigidDynamic {
        self.bodies[body.0].expect("a body taken away")
    }

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

    /// A body taken out of the world (its physics done: the game destroys
    /// the rigid body, `LootItem.StopPhysics`).
    pub fn remove(&mut self, body: Body) {
        if let Some(actor) = self.bodies[body.0].take() {
            unsafe {
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

    /// The first thing a sphere meets going from `origin` along the unit
    /// way `dir` within `distance`, among every shape of the world, still
    /// or moving (`PxSceneQueryExt::sweepSingle` with the normal and the
    /// least way out of a shape it starts in, as the engine sweeps a
    /// particle: `PhysXRaycast::ParticleIntersect`, its flags `eNORMAL |
    /// eMTD`): the distance along the way (below nought where it starts in
    /// a shape, by how far) and the surface's normal.
    pub fn sweep_sphere(&self, origin: V3, dir: V3, distance: f32, radius: f32) -> Option<(f32, V3)> {
        unsafe {
            let geometry = PxSphereGeometry_new_1(radius.max(1e-6));
            let mut hit = PxSweepHit_new();
            let filter = PxQueryFilterData_new();
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
            found.then(|| (hit.distance, [hit.normal.x, hit.normal.y, hit.normal.z]))
        }
    }
}

impl Drop for World {
    fn drop(&mut self) {
        unsafe {
            for joint in self.joints.drain(..) {
                PxJoint_release_mut(joint);
            }
            // The world's own objects go with it; the engine stays.
            for actor in self.bodies.drain(..).flatten() {
                PxRigidActor_release_mut(actor as *mut PxRigidActor);
            }
            for actor in self.statics.drain(..) {
                PxRigidActor_release_mut(actor as *mut PxRigidActor);
            }
            for (_, material) in self.materials.drain(..) {
                PxMaterial_release_mut(material);
            }
            PxScene_release_mut(self.scene);
            destroy_simulation_event_callbacks(self.events);
            PxDefaultCpuDispatcher_release_mut(self.dispatcher);
        }
    }
}

/// Two turns, `a` after `b` (`a * b`).
pub fn mul(a: Q4, b: Q4) -> Q4 {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

/// A point turned by a turn.
pub fn rotate(q: Q4, v: V3) -> V3 {
    let u = [q[0], q[1], q[2]];
    let t = cross(u, v);
    let t = [2.0 * t[0], 2.0 * t[1], 2.0 * t[2]];
    let c = cross(u, t);
    [v[0] + q[3] * t[0] + c[0], v[1] + q[3] * t[1] + c[1], v[2] + q[3] * t[2] + c[2]]
}

pub fn inverse(q: Q4) -> Q4 {
    [-q[0], -q[1], -q[2], q[3]]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A box let fall on the ground comes to rest on it, half its height
    /// up, and goes to sleep.
    #[test]
    fn a_box_falls_and_rests() {
        let mut world = World::new(Settings::game());
        world.add_ground(0.0, Material::default(), 0);
        let body = world.add_body(&BodyDesc {
            at: [0.0, 1.0, 0.0],
            turn: [0.0, 0.0, 0.0, 1.0],
            colliders: vec![Collider { shape: Shape::Box { half: [0.1, 0.05, 0.2] }, at: [0.0; 3], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() }],
            mass: 0.5,
            drag: 0.0,
            angular_drag: 0.05,
            velocity: [0.0; 3],
            spin: [0.0; 3],
            max_depenetration: 10.0,
            continuous: false,
            layer: 1,
        });
        for _ in 0..600 {
            world.step(1.0 / 120.0);
        }
        let (at, _) = world.pose(body);
        assert!((at[1] - 0.05).abs() < 0.01, "{at:?}");
        assert!(world.asleep(body));
        let (centre, inertia) = world.mass_properties(body);
        assert!(centre.iter().all(|c| c.abs() < 1e-5));
        // A box's own turning: m (b^2 + c^2) / 12 about each axis.
        assert!((inertia[0] - 0.5 * (0.01 + 0.16) / 12.0).abs() < 1e-5, "{inertia:?}");
    }

    /// A sphere swept down onto the ground and onto a box meets each where
    /// it should, with the surface's normal; one swept clear meets nothing.
    #[test]
    fn a_swept_sphere_meets_the_ground_and_a_box() {
        let mut world = World::new(Settings::game());
        world.add_ground(0.0, Material::default(), 0);
        world.add_static([2.0, 0.5, 0.0], [0.0, 0.0, 0.0, 1.0], &[Collider { shape: Shape::Box { half: [0.5; 3] }, at: [0.0; 3], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() }], 0);
        world.step(1.0 / 60.0);
        let (d, n) = world.sweep_sphere([0.0, 1.0, 0.0], [0.0, -1.0, 0.0], 2.0, 0.1).expect("the ground");
        assert!((d - 0.9).abs() < 1e-3 && (n[1] - 1.0).abs() < 1e-4, "{d} {n:?}");
        let (d, n) = world.sweep_sphere([0.0, 0.5, 0.0], [1.0, 0.0, 0.0], 3.0, 0.1).expect("the box's side");
        assert!((d - 1.4).abs() < 1e-3 && (n[0] + 1.0).abs() < 1e-4, "{d} {n:?}");
        assert!(world.sweep_sphere([0.0, 3.0, 0.0], [1.0, 0.0, 0.0], 3.0, 0.1).is_none());
    }
}
