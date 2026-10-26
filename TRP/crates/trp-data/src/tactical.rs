//! Tactical devices (tactical-devices spec): the parts that carry a light
//! (`LightComponent`: the classes `Flashlight`, `TacticalCombo` and
//! `LightLaser`), how many modes each has (`ModesCount`), which objects of
//! its model each mode shows (`TacticalComboVisualController`: `mode_000`
//! off, `mode_00<n+1>` mode n), and the figures of its lights (Unity's
//! `Light`, `IkLight`, `VolumetricLight`) and lasers (`LaserBeam` and the
//! materials of `Custom/LaserBeam` and `Custom/LaserPoint`) as its prefab
//! has them.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};

use crate::database::{Database, Preset};
use crate::unity::extract::{read_hierarchy, read_material, read_texture, Hierarchy, MaterialData, TextureData};
use crate::unity::{class, Assets, ObjRef, Value};

/// The three item classes that carry a light (`HasLightComponent`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    Flashlight,
    TacticalCombo,
    LightLaser,
}

impl DeviceClass {
    /// The class of an item's template, where it is one of the three.
    pub fn of(db: &Database, template: &str) -> Option<Self> {
        let t = db.items.get(template)?;
        if db.is_a(t, "TacticalCombo") {
            Some(Self::TacticalCombo)
        } else if db.is_a(t, "Flashlight") {
            Some(Self::Flashlight)
        } else if db.is_a(t, "LightLaser") {
            Some(Self::LightLaser)
        } else {
            None
        }
    }
}

/// How many modes a device's template has (`ModesCount`), 0 where it says
/// nothing.
pub fn modes_of(db: &Database, template: &str) -> u32 {
    db.items.get(template).and_then(|t| t.props.get("ModesCount")).and_then(|count| count.as_u64()).unwrap_or(0) as u32
}

/// A light's shaft (`VolumetricLight`, on the white lights; ray-marched by
/// the camera where the graphics option is on): its script's figures.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shaft {
    pub sample_count: u32,
    pub scattering: f32,
    pub extinction: f32,
    pub skybox_extinction: f32,
    pub mie_g: f32,
    pub height_fog: bool,
    pub height_scale: f32,
    pub ground_level: f32,
    pub noise: bool,
    pub noise_scale: f32,
    pub noise_intensity: f32,
    pub noise_intensity_offset: f32,
    pub noise_velocity: [f32; 2],
    pub max_ray_length: f32,
}

/// One light of a device (Unity's `Light`): its object, kind, colour,
/// intensity, reach, cone, shadows and cookie as its prefab has them, and
/// what drives it.
#[derive(Debug, Clone)]
pub struct DeviceLight {
    pub node: usize,
    /// The mode whose object it is under (0 for `mode_001`), or none: under
    /// the off object, or under none.
    pub mode: Option<usize>,
    pub enabled: bool,
    /// A spot (`m_Type` 0); otherwise a point (2).
    pub spot: bool,
    pub color: [f32; 3],
    /// The light's own intensity (`m_Intensity`), and the one it is drawn
    /// with: its distance fade's (`CullingLightObject`, on the same object)
    /// `_maxLightIntensity` where that takes its own figure
    /// (`_useLightIntensityFromEditor`, every device's), times its fade by
    /// the camera's distance, 1 within 45 m (one's own lights always).
    pub intensity: f32,
    pub drawn_intensity: f32,
    pub range: f32,
    /// The cone's whole angle and its inner one, in degrees.
    pub spot_angle: f32,
    pub inner_spot_angle: f32,
    /// `LightShadows`: 0 none, 1 hard, 2 soft; their resolution (`m_Resolution`:
    /// −1 the quality's, 0 low, 1 medium, 2 high, 3 very high), strength,
    /// bias, normal bias and near plane.
    pub shadows: u32,
    pub shadow_resolution: i32,
    pub shadow_strength: f32,
    pub shadow_bias: f32,
    pub shadow_normal_bias: f32,
    pub shadow_near: f32,
    pub cookie: Option<Arc<TextureData>>,
    pub culling_mask: u32,
    /// An infrared light (`IkLight`): it lights nothing for a camera without
    /// night vision.
    pub infrared: bool,
    pub shaft: Option<Shaft>,
    /// Whether a flare sits beside it (`FlareLight` on a sibling object).
    pub flare: bool,
}

impl DeviceLight {
    /// A device's LED: a point light that lights only its own spot (a
    /// reach of a centimetre).
    pub fn is_led(&self) -> bool {
        !self.spot && self.range <= 0.05
    }
}

/// The laser's beam material (`Custom/LaserBeam`): its colour, its factor
/// without and with night vision (`_Factor`, `_IkFactor`), the noise's
/// scale, and its pictures: the soft spot across the ribbon (`_MainTex`)
/// and the 3D noise (`_Tex3D`).
#[derive(Debug, Clone)]
pub struct BeamMaterial {
    pub color: [f32; 4],
    pub factor: f32,
    pub ik_factor: f32,
    pub noise_scale: f32,
    pub main: Option<Arc<TextureData>>,
    pub noise: Option<Arc<Volume>>,
}

/// The laser's dot material (`Custom/LaserPoint`): its colour, factors,
/// the field of view below which it shrinks (`_MinFov`), its size by the
/// camera's distance (`_SizeFactorViewMin`, `_SizeFactorViewMax`,
/// `_MaxDist`), and its pictures (`_MainTex`, `_NoiseTex`).
#[derive(Debug, Clone)]
pub struct PointMaterial {
    pub color: [f32; 4],
    pub factor: f32,
    pub ik_factor: f32,
    pub min_fov: f32,
    pub size_view: [f32; 2],
    pub max_dist: f32,
    pub main: Option<Arc<TextureData>>,
    pub noise: Option<Arc<TextureData>>,
}

/// One laser of a device (`LaserBeam`, on its `beam` object, whose z is the
/// beam's way): its script's figures and its two materials.
#[derive(Debug, Clone)]
pub struct DeviceLaser {
    pub node: usize,
    pub mode: Option<usize>,
    pub ray_start: f32,
    pub max_distance: f32,
    pub use_point_light: bool,
    pub mask: u32,
    /// The ribbon's half width.
    pub beam_size: f32,
    /// The dot's size near and far (`PointSizeClose`, `PointSizeFar`).
    pub point_size: [f32; 2],
    pub surface_offset: f32,
    pub light_range: f32,
    pub light_intensity: f32,
    pub intensity_factor: f32,
    /// The dot's light's cone near and far, in degrees (`AngleCloseFar`).
    pub angle_close_far: [f32; 2],
    pub cookie: Option<Arc<TextureData>>,
    pub beam: Option<BeamMaterial>,
    pub point: Option<PointMaterial>,
    /// Its own `IkLight` (an infrared beam).
    pub infrared: bool,
}

/// A device as fitted: its item, template, class and modes, its model's
/// root, the off object and each mode's object, and its lights and lasers.
#[derive(Debug, Clone)]
pub struct Device {
    pub item: String,
    pub template: String,
    pub class: DeviceClass,
    pub modes: u32,
    pub root: usize,
    /// `mode_000`, shown while off.
    pub off_node: Option<usize>,
    /// Each mode's object, mode 0 first.
    pub mode_nodes: Vec<usize>,
    pub lights: Vec<DeviceLight>,
    pub lasers: Vec<DeviceLaser>,
    /// The flares' objects (`FlareLight`) and the mode each is under.
    pub flares: Vec<(usize, Option<usize>)>,
    /// One's own weapon casts no shadow in this device's lights
    /// (`GunShadowDisabler` on its root).
    pub hides_gun_shadow: bool,
}

/// A 3D texture (`Texture3D`): its size, Unity format and levels.
#[derive(Debug, Clone)]
pub struct Volume {
    pub name: String,
    pub size: [u32; 3],
    pub format: i32,
    pub data: Vec<u8>,
}

const TEXTURE_3D: i32 = 117;

fn read_volume(assets: &Assets, obj: ObjRef) -> Result<Volume> {
    let v = assets.read(obj)?;
    let inline = v.req("image data")?.as_bytes().unwrap_or(&[]);
    let data = if !inline.is_empty() {
        inline.to_vec()
    } else {
        let stream = v.req("m_StreamData")?;
        let (path, offset, size) = (stream.req("path")?.as_str().unwrap_or(""), stream.req("offset")?.as_usize().unwrap_or(0), stream.req("size")?.as_usize().unwrap_or(0));
        assets.resource(path, offset, size)?.to_vec()
    };
    let dim = |name: &str| v.get(name).and_then(Value::as_u32).unwrap_or(0);
    Ok(Volume {
        name: v.req("m_Name")?.as_str().unwrap_or("").to_owned(),
        size: [dim("m_Width"), dim("m_Height"), dim("m_Depth")],
        format: v.get("m_Format").and_then(Value::as_i64).unwrap_or(0) as i32,
        data,
    })
}

/// The devices of a build, in the order the game walks its items
/// (`CompoundItem.AllSlots`). `roots` gives each placed item's root in the
/// assembled tree.
pub fn read(assets: &Assets, db: &Database, preset: &Preset, roots: &HashMap<&str, usize>, hierarchy: &Hierarchy) -> Result<Vec<Device>> {
    let mut devices = Vec::new();
    let mut textures = Textures::default();
    for item in crate::sight::all_slots_order(db, preset) {
        let Some(class) = DeviceClass::of(db, &item.template) else { continue };
        let Some(&root) = roots.get(item.id.as_str()) else { continue };
        let others: Vec<usize> = roots.values().copied().filter(|&r| r != root).collect();
        let device = device(assets, hierarchy, root, &others, &mut textures).with_context(|| format!("the device {}", item.template))?;
        devices.push(Device { item: item.id.clone(), template: item.template.clone(), class, modes: modes_of(db, &item.template), ..device });
    }
    Ok(devices)
}

/// A device read from its own prefab, apart from any weapon (a head
/// light's: its model is not drawn in first person, its lights are placed
/// by the camera).
pub fn read_alone(assets: &mut Assets, db: &Database, template: &str) -> Result<Device> {
    let class = DeviceClass::of(db, template).ok_or_else(|| anyhow!("{template} carries no light"))?;
    let key = db.items.get(template).and_then(|t| t.prefab_key()).ok_or_else(|| anyhow!("{template} has no prefab"))?.to_owned();
    let files = assets.load(&key)?;
    let root = files
        .iter()
        .find_map(|&file| crate::weapon::part_root(assets, file).transpose())
        .transpose()?
        .ok_or_else(|| anyhow!("{key} exports no prefab"))?;
    let hierarchy = read_hierarchy(assets, root)?;
    let mut textures = Textures::default();
    let device = device(assets, &hierarchy, 0, &[], &mut textures)?;
    Ok(Device { item: String::new(), template: template.to_owned(), class, modes: modes_of(db, template), ..device })
}

/// The pictures read so far, by object, so that a cookie or a noise many
/// lights share is read once.
#[derive(Default)]
struct Textures {
    flat: HashMap<ObjRef, Arc<TextureData>>,
    volumes: HashMap<ObjRef, Arc<Volume>>,
}

impl Textures {
    fn flat(&mut self, assets: &Assets, obj: ObjRef) -> Option<Arc<TextureData>> {
        if let Some(t) = self.flat.get(&obj) {
            return Some(t.clone());
        }
        let t = Arc::new(read_texture(assets, obj).ok()?);
        self.flat.insert(obj, t.clone());
        Some(t)
    }

    fn volume(&mut self, assets: &Assets, obj: ObjRef) -> Option<Arc<Volume>> {
        if let Some(t) = self.volumes.get(&obj) {
            return Some(t.clone());
        }
        if assets.class_id(obj) != Some(TEXTURE_3D) {
            return None;
        }
        let t = Arc::new(read_volume(assets, obj).ok()?);
        self.volumes.insert(obj, t.clone());
        Some(t)
    }
}

/// The nodes of a part's model, breadth first in the order of its children
/// (`GetComponentsInChildren`'s walk is depth first, but the controller
/// gathers its `mode_` objects breadth first), not into another part.
fn breadth_first(hierarchy: &Hierarchy, root: usize, others: &[usize]) -> Vec<usize> {
    let mut out = vec![root];
    let mut at = 0;
    while at < out.len() {
        let parent = out[at];
        out.extend((0..hierarchy.nodes.len()).filter(|&i| hierarchy.nodes[i].parent == Some(parent) && !others.contains(&i)));
        at += 1;
    }
    out
}

/// The device below `root`: its mode objects, lights, lasers and flares.
fn device(assets: &Assets, hierarchy: &Hierarchy, root: usize, others: &[usize], textures: &mut Textures) -> Result<Device> {
    let nodes = breadth_first(hierarchy, root, others);
    // `TacticalComboVisualController`: every object whose name holds
    // `mode_`, the first the off one, the rest the modes in order.
    let mut mode_objects: Vec<usize> = nodes.iter().copied().filter(|&n| hierarchy.nodes[n].name.contains("mode_")).collect();
    let off_node = (!mode_objects.is_empty()).then(|| mode_objects.remove(0));
    let mode_of = |node: usize| -> Option<usize> {
        let mut at = Some(node);
        while let Some(n) = at {
            if let Some(index) = mode_objects.iter().position(|&m| m == n) {
                return Some(index);
            }
            if Some(n) == off_node || n == root {
                return None;
            }
            at = hierarchy.nodes[n].parent;
        }
        None
    };
    let mut lights = Vec::new();
    let mut lasers = Vec::new();
    let mut flares = Vec::new();
    for &node in &nodes {
        let n = &hierarchy.nodes[node];
        let script = |class: &str| n.scripts.iter().zip(&n.script_objects).find(|(name, _)| name.as_str() == class).map(|(_, &obj)| obj);
        let infrared = script("IkLight").is_some();
        if script("FlareLight").is_some() {
            flares.push((node, mode_of(node)));
        }
        for obj in node_components(assets, hierarchy, node)?.into_iter().filter(|&c| assets.class_id(c) == Some(class::LIGHT)) {
            let l = assets.read(obj)?;
            let f = |name: &str| l.at(name).and_then(Value::as_f32).unwrap_or(0.0);
            let shadows = l.req("m_Shadows")?;
            let sf = |name: &str| shadows.get(name).and_then(Value::as_f32).unwrap_or(0.0);
            let color = l.req("m_Color")?;
            let channel = |c: &str| color.get(c).and_then(Value::as_f32).unwrap_or(0.0);
            let cookie = l.get("m_Cookie").and_then(Value::as_pptr).filter(|p| !p.is_null()).and_then(|p| assets.resolve_loaded(obj.file, p)).and_then(|t| textures.flat(assets, t));
            // The distance fade's own figure, where it takes it.
            let drawn_intensity = match script("CullingLightObject") {
                Some(s) => {
                    let c = assets.read(s)?;
                    let own = c.get("_useLightIntensityFromEditor").and_then(Value::as_i64).unwrap_or(0) != 0;
                    let most = c.get("_maxLightIntensity").and_then(Value::as_f32);
                    match (own, most) {
                        (true, Some(most)) => most,
                        _ => f("m_Intensity"),
                    }
                }
                None => f("m_Intensity"),
            };
            let shaft = match script("VolumetricLight") {
                Some(s) => Some(shaft(&assets.read(s)?)),
                None => None,
            };
            // A flare is the light's sibling (`NewMultiFlareLight` beside
            // `Spotlight` under the light's object).
            let parent = n.parent;
            let flare = parent.is_some_and(|p| hierarchy.nodes.iter().enumerate().any(|(i, s)| s.parent == Some(p) && i != node && s.scripts.iter().any(|c| c == "FlareLight")));
            lights.push(DeviceLight {
                node,
                mode: mode_of(node),
                enabled: l.get("m_Enabled").and_then(Value::as_i64).unwrap_or(1) != 0,
                spot: l.get("m_Type").and_then(Value::as_i64).unwrap_or(0) == 0,
                color: [channel("r"), channel("g"), channel("b")],
                intensity: f("m_Intensity"),
                drawn_intensity,
                range: f("m_Range"),
                spot_angle: f("m_SpotAngle"),
                inner_spot_angle: f("m_InnerSpotAngle"),
                shadows: shadows.get("m_Type").and_then(Value::as_i64).unwrap_or(0) as u32,
                shadow_resolution: shadows.get("m_Resolution").and_then(Value::as_i64).unwrap_or(-1) as i32,
                shadow_strength: sf("m_Strength"),
                shadow_bias: sf("m_Bias"),
                shadow_normal_bias: sf("m_NormalBias"),
                shadow_near: sf("m_NearPlane"),
                cookie,
                culling_mask: l.at("m_CullingMask.m_Bits").and_then(Value::as_u32).unwrap_or(u32::MAX),
                infrared,
                shaft,
                flare,
            });
        }
        if let Some(obj) = script("LaserBeam") {
            let b = assets.read(obj)?;
            let f = |name: &str| b.at(name).and_then(Value::as_f32).unwrap_or(0.0);
            let pair = |name: &str| [b.at(&format!("{name}.x")).and_then(Value::as_f32).unwrap_or(0.0), b.at(&format!("{name}.y")).and_then(Value::as_f32).unwrap_or(0.0)];
            let material = |name: &str| b.get(name).and_then(Value::as_pptr).filter(|p| !p.is_null()).and_then(|p| assets.resolve_loaded(obj.file, p)).and_then(|m| Some((m, read_material(assets, m).ok()?)));
            let beam = material("BeamMaterial").map(|(_, data)| beam_material(assets, &data, textures));
            let point = material("PointMaterial").map(|(_, data)| point_material(assets, &data, textures));
            let cookie = b.get("Cookie").and_then(Value::as_pptr).filter(|p| !p.is_null()).and_then(|p| assets.resolve_loaded(obj.file, p)).and_then(|t| textures.flat(assets, t));
            lasers.push(DeviceLaser {
                node,
                mode: mode_of(node),
                ray_start: f("RayStart"),
                max_distance: f("MaxDistance"),
                use_point_light: b.get("UsePointLight").and_then(Value::as_i64).unwrap_or(1) != 0,
                mask: b.at("Mask.m_Bits").and_then(Value::as_u32).unwrap_or(0),
                beam_size: f("BeamSize"),
                point_size: [f("PointSizeClose"), f("PointSizeFar")],
                surface_offset: f("SurfaceOffsetForLight"),
                light_range: f("LightRange"),
                light_intensity: f("LightIntensity"),
                intensity_factor: f("IntensityFactor"),
                angle_close_far: pair("AngleCloseFar"),
                cookie,
                beam,
                point,
                infrared,
            });
        }
    }
    let hides_gun_shadow = hierarchy.nodes[root].scripts.iter().any(|s| s == "GunShadowDisabler");
    Ok(Device { item: String::new(), template: String::new(), class: DeviceClass::TacticalCombo, modes: 0, root, off_node, mode_nodes: mode_objects, lights, lasers, flares, hides_gun_shadow })
}

fn beam_material(assets: &Assets, data: &MaterialData, textures: &mut Textures) -> BeamMaterial {
    let tex = |name: &str| data.textures.get(name).copied();
    BeamMaterial {
        color: data.colors.get("_Color").copied().unwrap_or([1.0; 4]),
        factor: data.floats.get("_Factor").copied().unwrap_or(1.0),
        ik_factor: data.floats.get("_IkFactor").copied().unwrap_or(1.0),
        noise_scale: data.floats.get("_NoiseScale0").copied().unwrap_or(0.1),
        main: tex("_MainTex").and_then(|t| textures.flat(assets, t)),
        noise: tex("_Tex3D").and_then(|t| textures.volume(assets, t)),
    }
}

fn point_material(assets: &Assets, data: &MaterialData, textures: &mut Textures) -> PointMaterial {
    let tex = |name: &str| data.textures.get(name).copied();
    PointMaterial {
        color: data.colors.get("_Color").copied().unwrap_or([1.0; 4]),
        factor: data.floats.get("_Factor").copied().unwrap_or(1.0),
        ik_factor: data.floats.get("_IkFactor").copied().unwrap_or(1.0),
        min_fov: data.floats.get("_MinFov").copied().unwrap_or(-50.0),
        size_view: [data.floats.get("_SizeFactorViewMin").copied().unwrap_or(1.0), data.floats.get("_SizeFactorViewMax").copied().unwrap_or(1.0)],
        max_dist: data.floats.get("_MaxDist").copied().unwrap_or(100.0),
        main: tex("_MainTex").and_then(|t| textures.flat(assets, t)),
        noise: tex("_NoiseTex").and_then(|t| textures.flat(assets, t)),
    }
}

fn shaft(v: &Value) -> Shaft {
    let f = |name: &str| v.get(name).and_then(Value::as_f32).unwrap_or(0.0);
    let on = |name: &str| v.get(name).and_then(Value::as_i64).unwrap_or(0) != 0;
    Shaft {
        sample_count: v.get("SampleCount").and_then(Value::as_u32).unwrap_or(16),
        scattering: f("ScatteringCoef"),
        extinction: f("ExtinctionCoef"),
        skybox_extinction: f("SkyboxExtinctionCoef"),
        mie_g: f("MieG"),
        height_fog: on("HeightFog"),
        height_scale: f("HeightScale"),
        ground_level: f("GroundLevel"),
        noise: on("Noise"),
        noise_scale: f("NoiseScale"),
        noise_intensity: f("NoiseIntensity"),
        noise_intensity_offset: f("NoiseIntensityOffset"),
        noise_velocity: [v.at("NoiseVelocity.x").and_then(Value::as_f32).unwrap_or(0.0), v.at("NoiseVelocity.y").and_then(Value::as_f32).unwrap_or(0.0)],
        max_ray_length: f("MaxRayLength"),
    }
}

/// The components of a node's game object.
fn node_components(assets: &Assets, hierarchy: &Hierarchy, node: usize) -> Result<Vec<ObjRef>> {
    let transform = hierarchy.nodes[node].transform;
    let t = assets.read(transform)?;
    let go_ref = t.req("m_GameObject")?.as_pptr().and_then(|p| assets.resolve_loaded(transform.file, p)).ok_or_else(|| anyhow!("a transform without a game object"))?;
    let go = assets.read(go_ref)?;
    Ok(crate::unity::extract::components(assets, go_ref, &go))
}
