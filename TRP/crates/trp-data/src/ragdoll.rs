//! The ragdoll a dead character becomes, as the player prefab sets it up
//! (`PlayerPoolObject`'s `RigidbodySpawners` and `JointSpawners`, made
//! into rigid bodies and character joints by `CorpseRagdoll`; health spec,
//! "Death"): each body on a bone with its mass and drag and the colliders
//! it takes, and each joint between two of them with its axes and limits.
//!
//! A body's colliders are the body's hit colliders (`BodyPartCollider`):
//! the one on its own bone, which is the one the corpse finds a body by
//! (`CorpseRagdoll.FindRigidbodySpawner`), and those on objects hung under
//! it that have no body of their own. The prefab keeps them switched off;
//! a corpse switches its colliders on.

use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};

use crate::player::PLAYER_BUNDLE;
use crate::unity::extract::{quat_mul, script_class};
use crate::unity::{class, Assets, ObjRef, Value};

/// The engine's collider classes.
const BOX_COLLIDER: i32 = 65;
const SPHERE_COLLIDER: i32 = 135;
const CAPSULE_COLLIDER: i32 = 136;

/// A collider's shape in its object's own axes, metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// Its axis along x, y or z (`m_Direction`: 0, 1, 2), its height end
    /// to end with the caps.
    Capsule { centre: [f32; 3], radius: f32, height: f32, direction: u32 },
    Box { centre: [f32; 3], size: [f32; 3] },
    Sphere { centre: [f32; 3], radius: f32 },
}

/// A collider a body takes.
#[derive(Debug, Clone, PartialEq)]
pub struct ColliderData {
    /// The object it is on.
    pub object: String,
    /// That object's place and turn (`xyzw`) in the body's bone: none for
    /// one on the bone itself.
    pub local: Option<([f32; 3], [f32; 4])>,
    pub shape: Shape,
}

/// A rigid body of the ragdoll (`RigidbodySpawner`).
#[derive(Debug, Clone, PartialEq)]
pub struct BodyData {
    /// The bone it is on.
    pub bone: String,
    /// Where that bone stands in the prefab, at rest, in the prefab's own
    /// axes: its place and turn (`xyzw`).
    pub rest: ([f32; 3], [f32; 4]),
    pub mass: f32,
    pub drag: f32,
    pub angular_drag: f32,
    pub colliders: Vec<ColliderData>,
}

/// A soft limit (`SoftJointLimit`): degrees, and its bounce and contact
/// distance.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Limit {
    pub limit: f32,
    pub bounciness: f32,
    pub contact_distance: f32,
}

/// A joint between two bodies (`CharacterJointSpawner`).
#[derive(Debug, Clone, PartialEq)]
pub struct JointData {
    /// The bone it is on, and the bone of the body it holds to.
    pub bone: String,
    pub connected: String,
    /// In the bone's own axes: where the joint is, its twist axis and the
    /// swing axis across it.
    pub anchor: [f32; 3],
    pub axis: [f32; 3],
    pub swing_axis: [f32; 3],
    pub low_twist: Limit,
    pub high_twist: Limit,
    pub swing1: Limit,
    pub swing2: Limit,
    /// Spring and damper of the twist's and the swings' limits.
    pub twist_spring: [f32; 2],
    pub swing_spring: [f32; 2],
    pub enable_collision: bool,
    pub projection: Option<[f32; 2]>,
    /// Whether the connected body's anchor is worked out where the joint
    /// is made (`autoConfigureConnectedAnchor`), and if not, where it is in
    /// the connected body's axes.
    pub auto_anchor: bool,
    pub connected_anchor: [f32; 3],
    /// Whether the engine's preprocessing is on (`enablePreprocessing`).
    pub preprocessing: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RagdollData {
    pub bodies: Vec<BodyData>,
    pub joints: Vec<JointData>,
}

fn vec3(v: &Value, name: &str) -> Result<[f32; 3]> {
    v.req(name)?.as_vec3().with_context(|| format!("{name} is not a vector"))
}

fn float(v: &Value, name: &str) -> Result<f32> {
    v.req(name)?.as_f32().with_context(|| format!("{name} is not a number"))
}

/// A point turned by a quaternion (`xyzw`).
fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let [x, y, z, w] = q;
    let t = [2.0 * (y * v[2] - z * v[1]), 2.0 * (z * v[0] - x * v[2]), 2.0 * (x * v[1] - y * v[0])];
    [v[0] + w * t[0] + (y * t[2] - z * t[1]), v[1] + w * t[1] + (z * t[0] - x * t[2]), v[2] + w * t[2] + (x * t[1] - y * t[0])]
}

/// One object of the prefab's tree: its name, its parent, its place and
/// turn in that parent, and its components.
struct Node {
    name: String,
    parent: Option<ObjRef>,
    place: [f32; 3],
    turn: [f32; 4],
    components: Vec<ObjRef>,
}

impl RagdollData {
    pub fn load(assets: &mut Assets) -> Result<Self> {
        let files = assets.load(PLAYER_BUNDLE)?;
        // Every object of the prefab by its own game object, with what it
        // hangs from.
        let mut nodes: HashMap<ObjRef, Node> = HashMap::new();
        let mut of_transform: HashMap<ObjRef, ObjRef> = HashMap::new();
        let game_objects: Vec<ObjRef> = files.iter().flat_map(|&f| assets.objects_of(f, class::GAME_OBJECT)).collect();
        let mut fathers = Vec::new();
        for object in game_objects {
            let v = assets.read(object)?;
            let components: Vec<ObjRef> = v
                .get("m_Component")
                .and_then(Value::as_array)
                .unwrap_or(&[])
                .iter()
                .filter_map(|c| c.get("component")?.as_pptr())
                .filter_map(|p| assets.resolve_loaded(object.file, p))
                .collect();
            let Some(transform) = components.iter().copied().find(|c| assets.class_id(*c) == Some(class::TRANSFORM)) else { continue };
            let t = assets.read(transform)?;
            of_transform.insert(transform, object);
            if let Some(father) = t.get("m_Father").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(transform.file, p)) {
                fathers.push((object, father));
            }
            nodes.insert(object, Node {
                name: v.get("m_Name").and_then(Value::as_str).unwrap_or("").to_owned(),
                parent: None,
                place: vec3(&t, "m_LocalPosition")?,
                turn: t.req("m_LocalRotation")?.as_vec4().context("a turn")?,
                components,
            });
        }
        for (object, father) in fathers {
            let parent = of_transform.get(&father).copied();
            if let Some(node) = nodes.get_mut(&object) {
                node.parent = parent;
            }
        }
        let script_on = |assets: &Assets, node: &Node, wanted: &str| -> Option<ObjRef> {
            node.components.iter().copied().find(|c| assets.class_id(*c) == Some(class::MONO_BEHAVIOUR) && script_class(assets, *c).as_deref() == Some(wanted))
        };
        // The bodies and the joints.
        let mut bodies: Vec<BodyData> = Vec::new();
        let mut body_of: HashMap<ObjRef, usize> = HashMap::new();
        let mut joints = Vec::new();
        let mut spawner_owner: HashMap<ObjRef, String> = HashMap::new();
        for (&object, node) in &nodes {
            if let Some(spawner) = script_on(assets, node, "RigidbodySpawner") {
                let v = assets.read(spawner)?;
                spawner_owner.insert(spawner, node.name.clone());
                body_of.insert(object, bodies.len());
                // The bone's rest in the prefab: its own place and turn,
                // then each parent's over it.
                let (mut place, mut turn) = (node.place, node.turn);
                let mut above = node.parent;
                while let Some(parent) = above {
                    let p = &nodes[&parent];
                    let moved = rotate(p.turn, place);
                    place = [p.place[0] + moved[0], p.place[1] + moved[1], p.place[2] + moved[2]];
                    turn = quat_mul(p.turn, turn);
                    above = p.parent;
                }
                bodies.push(BodyData { bone: node.name.clone(), rest: (place, turn), mass: float(&v, "mass")?, drag: float(&v, "drag")?, angular_drag: float(&v, "angularDrag")?, colliders: Vec::new() });
            }
        }
        for node in nodes.values() {
            let Some(spawner) = script_on(assets, node, "CharacterJointSpawner") else { continue };
            let v = assets.read(spawner)?;
            let connected = v.get("connectedBody").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(spawner.file, p)).and_then(|body| spawner_owner.get(&body).cloned()).unwrap_or_default();
            let limit = |field: &str| -> Result<Limit> {
                let l = v.req(field)?;
                Ok(Limit { limit: float(l, "limit")?, bounciness: float(l, "bounciness")?, contact_distance: float(l, "contactDistance")? })
            };
            let spring = |field: &str| -> Result<[f32; 2]> {
                let s = v.req(field)?;
                Ok([float(s, "spring")?, float(s, "damper")?])
            };
            let projection = (v.get("enableProjection").and_then(Value::as_i64).unwrap_or(0) != 0).then(|| Some([float(&v, "projectionDistance").ok()?, float(&v, "projectionAngle").ok()?])).flatten();
            joints.push(JointData {
                bone: node.name.clone(),
                connected,
                anchor: vec3(&v, "anchor")?,
                axis: vec3(&v, "axis")?,
                swing_axis: vec3(&v, "swingAxis")?,
                low_twist: limit("lowTwistLimit")?,
                high_twist: limit("highTwistLimit")?,
                swing1: limit("swing1Limit")?,
                swing2: limit("swing2Limit")?,
                twist_spring: spring("twistLimitSpring")?,
                swing_spring: spring("swingLimitSpring")?,
                enable_collision: v.get("enableCollision").and_then(Value::as_i64).unwrap_or(0) != 0,
                projection,
                auto_anchor: v.get("autoConfigureConnectedAnchor").and_then(Value::as_i64).unwrap_or(1) != 0,
                connected_anchor: vec3(&v, "connectedAnchor").unwrap_or([0.0; 3]),
                preprocessing: v.get("enablePreprocessing").and_then(Value::as_i64).unwrap_or(1) != 0,
            });
        }
        // Each hit collider to the nearest body at or above it, with its
        // place in that body's bone.
        for (&object, node) in &nodes {
            if script_on(assets, node, "BodyPartCollider").is_none() {
                continue;
            }
            let (mut at, mut place, mut turn) = (object, [0.0f32; 3], [0.0, 0.0, 0.0, 1.0f32]);
            let body = loop {
                if let Some(&body) = body_of.get(&at) {
                    break Some(body);
                }
                let here = &nodes[&at];
                // Up one level: this object's place in its parent, then
                // what was gathered below it.
                place = {
                    let moved = rotate(here.turn, place);
                    [here.place[0] + moved[0], here.place[1] + moved[1], here.place[2] + moved[2]]
                };
                turn = quat_mul(here.turn, turn);
                match here.parent {
                    Some(parent) => at = parent,
                    None => break None,
                }
            };
            let Some(body) = body else { continue };
            for component in &node.components {
                let kind = assets.class_id(*component);
                if !matches!(kind, Some(BOX_COLLIDER | SPHERE_COLLIDER | CAPSULE_COLLIDER)) {
                    continue;
                }
                let c = assets.read(*component)?;
                if c.get("m_IsTrigger").and_then(Value::as_i64).unwrap_or(0) != 0 {
                    continue;
                }
                let shape = match kind {
                    Some(BOX_COLLIDER) => Shape::Box { centre: vec3(&c, "m_Center")?, size: vec3(&c, "m_Size")? },
                    Some(SPHERE_COLLIDER) => Shape::Sphere { centre: vec3(&c, "m_Center")?, radius: float(&c, "m_Radius")? },
                    _ => Shape::Capsule {
                        centre: vec3(&c, "m_Center")?,
                        radius: float(&c, "m_Radius")?,
                        height: float(&c, "m_Height")?,
                        direction: c.get("m_Direction").and_then(Value::as_i64).unwrap_or(1) as u32,
                    },
                };
                let local = (at != object).then_some((place, turn));
                bodies[body].colliders.push(ColliderData { object: node.name.clone(), local, shape });
            }
        }
        bodies.sort_by(|a, b| a.bone.cmp(&b.bone));
        joints.sort_by(|a, b| a.bone.cmp(&b.bone));
        if bodies.is_empty() {
            return Err(anyhow!("the player prefab has no ragdoll"));
        }
        Ok(Self { bodies, joints })
    }
}
