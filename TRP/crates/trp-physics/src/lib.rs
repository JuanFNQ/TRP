//! The game's physics engine, PhysX 4.1, set up as the game's Unity sets it up from its settings.
//! PhysX 4.1 is the engine of the game's Unity, 2022.3; its settings are `globalgamemanagers`'
//! (`probe layers`). It is given what the game hands to it: a corpse's ragdoll, things let fall
//! and thrown (health spec, "Death"; `docs/spec/physics.md`).
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
pub mod recorded_loose;
pub mod setup;

mod engine_setup;
mod touches;
mod world_bodies;
mod joints;
mod stepping;
mod sweeps;

pub use engine_setup::*;
pub use touches::*;
use engine_setup::{engine, floor_mesh, layer_filter};
use touches::{REPORTS_TOUCHES, TAG_BODY, TAG_STILL, TOUCHES_AT_ONCE, keep_touches, on_contact, tag, tagged, touches_done};

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
    pub fn of(p: &trp_data::setup::engine::PhysicsSettings) -> Self {
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

    /// The figures of the install as read (`probe layers`):
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

impl Material {
    /// The material a collider names in the install, or the engine's
    /// default where it names none.
    pub fn of(m: Option<trp_data::unity::extract::PhysicMaterial>) -> Self {
        let Some(m) = m else { return Self::default() };
        let combine = |mode: u32| match mode {
            1 => Combine::Minimum,
            2 => Combine::Multiply,
            3 => Combine::Maximum,
            _ => Combine::Average,
        };
        Self { dynamic_friction: m.dynamic_friction, static_friction: m.static_friction, bounciness: m.bounciness, friction_combine: combine(m.friction_combine), bounce_combine: combine(m.bounce_combine) }
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
    /// A `MeshCollider` that is not convex: its triangles, each three
    /// corners' places in `points` in the engine's order (PhysX takes a
    /// triangle's face as the side its corners' cross product points to,
    /// the side Unity draws), scaled as its object is; only for what does
    /// not move (`add_static`). Cooked with PhysX's own settings
    /// (`PROVISIONAL`: Unity's `m_CookingOptions` clean and weld the mesh
    /// as well).
    Mesh { points: std::sync::Arc<[V3]>, triangles: std::sync::Arc<[[u32; 3]]>, scale: V3 },
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
mod tests;
