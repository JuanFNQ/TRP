//! The head's chain, the capsule, camera box, head colliders and limbs.

use super::*;

/// The shares of the free look's turn that the neck and the head each take
/// (`PlayerBones.NeckChain`, `HeadChain` on the player prefab).
pub fn head_chain(assets: &mut Assets) -> Result<[f32; 2]> {
    let files = assets.load(PLAYER_BUNDLE)?;
    let bones = files
        .iter()
        .flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR))
        .find(|&obj| script_class(assets, obj).as_deref() == Some("PlayerBones"))
        .ok_or_else(|| anyhow!("player prefab has no PlayerBones"))?;
    let v = assets.read(bones)?;
    Ok([float(&v, "NeckChain")?, float(&v, "HeadChain")?])
}

/// Where the parts of the body are hung (`PlayerBody._meshTransform`, the
/// prefab's `Mesh` object, under `Player`): its own place and turn
/// (`xyzw`), in the game's axes. Each part's prefab is put under it with
/// its own place kept (`PlayerBody.SetSkin`, `SetParent(_, false)`), and a
/// worn skin's dress too, so their groups of levels of detail are placed
/// by it (third-person spec, "Levels of detail").
pub fn mesh_transform(assets: &mut Assets) -> Result<([f32; 3], [f32; 4])> {
    let files = assets.load(PLAYER_BUNDLE)?;
    let body = files
        .iter()
        .flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR))
        .find(|&obj| script_class(assets, obj).as_deref() == Some("PlayerBody"))
        .ok_or_else(|| anyhow!("player prefab has no PlayerBody"))?;
    let v = assets.read(body)?;
    let transform = v.req("_meshTransform")?.as_pptr().and_then(|p| assets.resolve_loaded(body.file, p)).ok_or_else(|| anyhow!("PlayerBody names no mesh transform"))?;
    let t = assets.read(transform)?;
    let place = t.req("m_LocalPosition")?.as_vec3().ok_or_else(|| anyhow!("the mesh transform's place"))?;
    let turn = t.req("m_LocalRotation")?.as_vec4().ok_or_else(|| anyhow!("the mesh transform's turn"))?;
    Ok((place, turn))
}

/// The capsule the game moves the character about in (`CharacterControllerSpawner`
/// on the player prefab), in metres.
#[derive(Debug, Clone, Copy)]
pub struct CapsuleData {
    pub radius: f32,
    pub height: f32,
    /// How far outside the capsule the game's controller keeps things.
    pub skin_width: f32,
    /// The tallest step it walks up.
    pub step_offset: f32,
    /// The steepest slope it walks up, in degrees.
    pub slope_limit: f32,
}

impl CapsuleData {
    pub fn load(assets: &mut Assets) -> Result<Self> {
        let files = assets.load(PLAYER_BUNDLE)?;
        let spawner = files
            .iter()
            .flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR))
            .find(|&obj| script_class(assets, obj).as_deref() == Some("CharacterControllerSpawner"))
            .ok_or_else(|| anyhow!("player prefab has no character controller"))?;
        let v = assets.read(spawner)?;
        Ok(Self {
            radius: float(&v, "_radius")?,
            height: float(&v, "_height")?,
            skin_width: float(&v, "_skinWidth")?,
            step_offset: float(&v, "_stepOffset")?,
            slope_limit: float(&v, "_slopeLimit")?,
        })
    }
}

/// The box that keeps the first-person camera out of the world: the box
/// collider the player prefab's `Cam` object pushes clear of things
/// (`ColliderExtrusion`), in the camera's own axes (x right, y up, z
/// ahead) and in metres.
#[derive(Debug, Clone, Copy)]
pub struct CameraBoxData {
    /// Its full width, height and depth.
    pub size: [f32; 3],
    /// Its centre from the camera.
    pub centre: [f32; 3],
}

impl CameraBoxData {
    pub fn load(assets: &mut Assets) -> Result<Self> {
        let files = assets.load(PLAYER_BUNDLE)?;
        for obj in files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect::<Vec<_>>() {
            if script_class(assets, obj).as_deref() != Some("ColliderExtrusion") {
                continue;
            }
            let script = assets.read(obj)?;
            let named = |pointer: &str| -> Option<Value> {
                let target = script.get(pointer)?.as_pptr().and_then(|p| assets.resolve_loaded(obj.file, p))?;
                assets.read(target).ok()
            };
            // The prefab has two of these scripts; the camera's is on `Cam`.
            let on_camera = named("m_GameObject").is_some_and(|owner| owner.get("m_Name").and_then(Value::as_str) == Some("Cam"));
            if !on_camera {
                continue;
            }
            let collider = named("_collider").ok_or_else(|| anyhow!("the camera's collider is not in the player prefab"))?;
            let vec3 = |name: &str| collider.req(name)?.as_vec3().with_context(|| format!("{name} is not a vector"));
            return Ok(Self { size: vec3("m_Size")?, centre: vec3("m_Center")? });
        }
        Err(anyhow!("the player prefab's camera has no collider"))
    }
}

/// One of the head's capsules (`PlayerOverlapManager._headColliders`): the
/// capsule collider's own figures, and its object's place and turn in its
/// parent (Unity's `CapsuleCollider`: `direction` 0 x, 1 y, 2 z).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadCapsuleData {
    pub radius: f32,
    pub height: f32,
    pub direction: usize,
    pub centre: [f32; 3],
    pub position: [f32; 3],
    pub rotation: [f32; 4],
}

/// The capsule about the head that the movement keeps out of walls
/// (movement spec, "The head's push out of walls"): standing, the one on
/// `playercollider` under the camera's container; lying, the one on
/// `ProneCollisionTest` under the player, which is moved over the bone
/// named `lying_pivot` (`_proneColliderPivot`) as it is used.
#[derive(Debug, Clone, PartialEq)]
pub struct HeadCollidersData {
    pub standing: HeadCapsuleData,
    pub lying: HeadCapsuleData,
    pub lying_pivot: String,
}

impl HeadCollidersData {
    pub fn load(assets: &mut Assets) -> Result<Self> {
        let files = assets.load(PLAYER_BUNDLE)?;
        for obj in files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect::<Vec<_>>() {
            if script_class(assets, obj).as_deref() != Some("PlayerOverlapManager") {
                continue;
            }
            let script = assets.read(obj)?;
            let resolve = |v: &Value| v.as_pptr().and_then(|p| assets.resolve_loaded(obj.file, p));
            let colliders: Vec<ObjRef> =
                script.req("_headColliders")?.as_array().unwrap_or(&[]).iter().filter_map(resolve).collect();
            anyhow::ensure!(colliders.len() >= 2, "the head has {} colliders, not two", colliders.len());
            let transform_of = |owner: &Value| -> Result<Value> {
                let go = owner.get("m_GameObject").and_then(resolve).ok_or_else(|| anyhow!("a collider without its object"))?;
                let go_value = assets.read(go)?;
                let transform = crate::unity::extract::components(assets, go, &go_value)
                    .into_iter()
                    .find(|c| assets.info(*c).is_some_and(|i| i.class_id == class::TRANSFORM))
                    .ok_or_else(|| anyhow!("an object without its transform"))?;
                assets.read(transform)
            };
            let capsule = |at: ObjRef| -> Result<HeadCapsuleData> {
                let c = assets.read(at)?;
                let t = transform_of(&c)?;
                let float = |name: &str| c.req(name)?.as_f32().with_context(|| format!("{name} is not a number"));
                Ok(HeadCapsuleData {
                    radius: float("m_Radius")?,
                    height: float("m_Height")?,
                    direction: c.req("m_Direction")?.as_i64().context("m_Direction")? as usize,
                    centre: c.req("m_Center")?.as_vec3().context("m_Center")?,
                    position: t.req("m_LocalPosition")?.as_vec3().context("position")?,
                    rotation: t.req("m_LocalRotation")?.as_vec4().context("rotation")?,
                })
            };
            let pivot = script.get("_proneColliderPivot").and_then(resolve).ok_or_else(|| anyhow!("no pivot for the lying capsule"))?;
            let pivot = assets.read(pivot)?;
            let pivot_go = pivot.get("m_GameObject").and_then(resolve).ok_or_else(|| anyhow!("the pivot without its object"))?;
            return Ok(Self {
                standing: capsule(colliders[0])?,
                lying: capsule(colliders[1])?,
                lying_pivot: assets.read(pivot_go)?.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned(),
            });
        }
        Err(anyhow!("the player prefab has no PlayerOverlapManager"))
    }
}

/// One of the forearm's two twist bones: how it is turned about its own
/// length after the arm is solved (`TwistRelax` on the player prefab).
#[derive(Debug, Clone, Copy)]
pub struct TwistData {
    pub weight: f32,
    /// How far from the first forearm bone's turning toward the palm's.
    pub crossfade: f32,
    /// The bone's own axis it is turned about, and another across it.
    pub twist_axis: [f32; 3],
    pub axis: [f32; 3],
    /// Where that second axis pointed at rest, in the first forearm bone's
    /// axes and in the palm's.
    pub in_parent: [f32; 3],
    pub in_child: [f32; 3],
}

/// How one arm's limb solver is set up on the player prefab (`LimbIK` on
/// the collarbone), with the forearm's second and third bones' twist.
#[derive(Debug, Clone, Copy)]
pub struct LimbData {
    /// How the elbow's side is chosen: 4 is toward a goal.
    pub bend_modifier: i64,
    pub bend_weight: f32,
    /// How much of the palm's own turning is kept through the solve.
    pub maintain_rotation: f32,
    pub twists: [TwistData; 2],
}

impl LimbData {
    /// The right arm's, then the left's.
    pub fn load(assets: &mut Assets) -> Result<[Self; 2]> {
        Self::load_from(assets, PLAYER_BUNDLE)
    }

    /// The same of another prefab with the player's skeleton and the same
    /// scripts on the same bones: the menus' copy of the player
    /// (`crate::body::MENU_PLAYER_BUNDLE`), whose `MenuPlayerPoser` solves
    /// its arms with solvers of its own (`Limbs`, `Twists`).
    pub fn load_from(assets: &mut Assets, bundle: &str) -> Result<[Self; 2]> {
        let files = assets.load(bundle)?;
        // Every script of the prefab with the name of the object it is on.
        let mut scripts = Vec::new();
        for obj in files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect::<Vec<_>>() {
            let Some(class_name) = script_class(assets, obj) else { continue };
            if class_name != "LimbIK" && class_name != "TwistRelax" {
                continue;
            }
            let value = assets.read(obj)?;
            let owner = value
                .req("m_GameObject")?
                .as_pptr()
                .and_then(|p| assets.resolve_loaded(obj.file, p))
                .and_then(|owner| assets.read(owner).ok())
                .and_then(|owner| owner.get("m_Name").and_then(Value::as_str).map(str::to_owned))
                .unwrap_or_default();
            scripts.push((class_name, owner, value));
        }
        let on = |class_name: &str, owner: &str| -> Result<&Value> {
            scripts
                .iter()
                .find(|(c, o, _)| c == class_name && o == owner)
                .map(|(_, _, value)| value)
                .ok_or_else(|| anyhow!("the player prefab has no {class_name} on {owner}"))
        };
        let vec3 = |v: &Value, name: &str| v.req(name)?.as_vec3().with_context(|| format!("{name} is not a vector"));
        let twist = |owner: &str| -> Result<TwistData> {
            let v = on("TwistRelax", owner)?;
            Ok(TwistData {
                weight: float(v, "weight")?,
                crossfade: float(v, "parentChildCrossfade")?,
                twist_axis: vec3(v, "twistAxis")?,
                axis: vec3(v, "axis")?,
                in_parent: vec3(v, "_axisRelativeToParentDefault")?,
                in_child: vec3(v, "_axisRelativeToChildDefault")?,
            })
        };
        let limb = |side: char| -> Result<Self> {
            let solver = on("LimbIK", &format!("Base Human{side}Collarbone"))?.req("solver")?;
            Ok(Self {
                bend_modifier: solver.req("bendModifier")?.as_i64().context("bendModifier is not a number")?,
                bend_weight: float(solver, "bendModifierWeight")?,
                maintain_rotation: float(solver, "maintainRotationWeight")?,
                twists: [twist(&format!("Base Human{side}Forearm2"))?, twist(&format!("Base Human{side}Forearm3"))?],
            })
        };
        Ok([limb('R')?, limb('L')?])
    }
}
