//! Colliders, physic materials and rigidbodies on a hierarchy's nodes.

use super::*;

/// A collider on an object of a model (`BoxCollider` and the rest): its
/// shape in the object's own axes, whether it is a trigger or switched
/// off, its object's layer, and its material where it names one.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeCollider {
    pub shape: ColliderShape,
    pub trigger: bool,
    pub enabled: bool,
    pub layer: u32,
    pub material: Option<PhysicMaterial>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColliderShape {
    Box { centre: [f32; 3], size: [f32; 3] },
    Sphere { centre: [f32; 3], radius: f32 },
    /// Along x, y or z (0, 1, 2), its height end to end with the caps.
    Capsule { centre: [f32; 3], radius: f32, height: f32, direction: u32 },
    /// A mesh, convex or not, and the mesh it names (in a file loaded with
    /// the model).
    Mesh { convex: bool, mesh: Option<ObjRef> },
}

/// A material for the physics (`PhysicMaterial`): frictions, bounce, and
/// how each is combined (0 average, 1 minimum, 2 multiply, 3 maximum).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicMaterial {
    pub dynamic_friction: f32,
    pub static_friction: f32,
    pub bounciness: f32,
    pub friction_combine: u32,
    pub bounce_combine: u32,
}

/// The material a collider component names (`m_Material`), read as the
/// collider `c` of component `comp`: none where it names none (the engine
/// then takes its default, Unity's 0.6 of friction and no bounce).
pub fn physic_material(assets: &Assets, comp: ObjRef, c: &Value) -> Option<PhysicMaterial> {
    let m = c.get("m_Material").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(comp.file, p)).and_then(|m| assets.read(m).ok())?;
    Some(PhysicMaterial {
        dynamic_friction: m.get("dynamicFriction")?.as_f32()?,
        static_friction: m.get("staticFriction")?.as_f32()?,
        bounciness: m.get("bounciness")?.as_f32()?,
        friction_combine: m.get("frictionCombine").and_then(Value::as_i64).unwrap_or(0) as u32,
        bounce_combine: m.get("bounceCombine").and_then(Value::as_i64).unwrap_or(0) as u32,
    })
}

pub(super) const BOX_COLLIDER: i32 = 65;

pub(super) const MESH_COLLIDER: i32 = 64;

pub(super) const SPHERE_COLLIDER: i32 = 135;

pub(super) const CAPSULE_COLLIDER: i32 = 136;

/// A collider component as a `NodeCollider`, if it is one.
pub(super) fn node_collider(assets: &Assets, comp: ObjRef, class_id: i32, layer: u32) -> Result<Option<NodeCollider>> {
    let c = assets.read(comp)?;
    let vec3 = |name: &str| -> Result<[f32; 3]> { c.req(name)?.as_vec3().with_context(|| format!("{name} is not a vector")) };
    let float = |name: &str| -> Result<f32> { c.req(name)?.as_f32().with_context(|| format!("{name} is not a number")) };
    let shape = match class_id {
        BOX_COLLIDER => ColliderShape::Box { centre: vec3("m_Center")?, size: vec3("m_Size")? },
        SPHERE_COLLIDER => ColliderShape::Sphere { centre: vec3("m_Center")?, radius: float("m_Radius")? },
        CAPSULE_COLLIDER => ColliderShape::Capsule {
            centre: vec3("m_Center")?,
            radius: float("m_Radius")?,
            height: float("m_Height")?,
            direction: c.get("m_Direction").and_then(Value::as_i64).unwrap_or(1) as u32,
        },
        MESH_COLLIDER => ColliderShape::Mesh {
            convex: c.get("m_Convex").and_then(Value::as_bool).unwrap_or(false),
            mesh: c.get("m_Mesh").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(comp.file, p)),
        },
        _ => return Ok(None),
    };
    let material = physic_material(assets, comp, &c);
    Ok(Some(NodeCollider {
        shape,
        trigger: c.get("m_IsTrigger").and_then(Value::as_bool).unwrap_or(false),
        enabled: c.get("m_Enabled").and_then(Value::as_bool).unwrap_or(true),
        layer,
        material,
    }))
}

/// A rigid body as Unity keeps one (`Rigidbody`): its mass, drag and
/// angular drag, and whether it is swept against fast motion
/// (`m_CollisionDetection`: 0 discrete, 1 continuous, 2 continuous
/// dynamic, 3 speculative).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidbodyData {
    pub mass: f32,
    pub drag: f32,
    pub angular_drag: f32,
    pub collision_detection: i64,
    pub use_gravity: bool,
    pub kinematic: bool,
}

const RIGIDBODY: i32 = 54;

/// The rigid body on a GameObject, if it has one.
pub fn rigidbody_of(assets: &Assets, game_object: ObjRef) -> Result<Option<RigidbodyData>> {
    let go = assets.read(game_object)?;
    let Some(comp) = components(assets, game_object, &go).into_iter().find(|c| assets.class_id(*c) == Some(RIGIDBODY)) else { return Ok(None) };
    let c = assets.read(comp)?;
    let float = |name: &str| -> Result<f32> { c.req(name)?.as_f32().with_context(|| format!("{name} is not a number")) };
    Ok(Some(RigidbodyData {
        mass: float("m_Mass")?,
        drag: float("m_Drag")?,
        angular_drag: float("m_AngularDrag")?,
        collision_detection: c.get("m_CollisionDetection").and_then(Value::as_i64).unwrap_or(0),
        use_gravity: c.get("m_UseGravity").and_then(Value::as_bool).unwrap_or(true),
        kinematic: c.get("m_IsKinematic").and_then(Value::as_bool).unwrap_or(false),
    }))
}
