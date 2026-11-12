//! A level's colliders as its scenes lay them.
//! After `docs/spec/particles.md`, "Meeting the world": every box, sphere, capsule and mesh collider that
//! is on, on an object on in its scene, near a point, with its layer and
//! where it lies in the world. Read from the build's own scene files, which
//! keep no type trees: each collider by the tree of its class in a bundle
//! of the install's, written by the same engine (`screen::tree_of`), a mesh
//! by the effects bundle's, as `scene_props` reads one.
//!
//! What the world's physics is given is the engine's, from these: a mesh
//! collider's triangles in its object's axes, scaled, turned and moved into
//! the world's here (the engine gives PhysX the object's place, turn and
//! scale; the corners come out the same), a box's, sphere's or capsule's
//! middle and size scaled by its object's size in the world
//! (`lossyScale`).
//!
//! `PROVISIONAL`: each collider taken as on or off as its scene saves it;
//! the game switches parts of a level on and off as it plays (its culling,
//! not read: `courses3`'s chips fall through floors read here as on).

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};

use crate::setup::install::Install;
use crate::interface::screen::{scene_paths, tree_of, Scene, ScenePictures};
use crate::unity::class;
use crate::unity::extract::{mesh_streamed, MeshData};
use crate::unity::typetree::TypeTree;
use crate::unity::Value;

/// The engine's class numbers of the colliders.
pub const MESH_COLLIDER: i32 = 64;
pub const BOX_COLLIDER: i32 = 65;
pub const SPHERE_COLLIDER: i32 = 135;
pub const CAPSULE_COLLIDER: i32 = 136;

/// Bundles of the install that carry each collider class's tree (found by
/// `probe classfind`): small ones, read for the tree alone.
const TREES: [(i32, &str); 4] = [
    (MESH_COLLIDER, "assets/content/items/spec/keys/item_key_3.bundle"),
    (BOX_COLLIDER, "assets/content/commonprefabs/defaultballisticcollider.bundle"),
    (SPHERE_COLLIDER, "assets/content/commonprefabs/playersimple.bundle"),
    (CAPSULE_COLLIDER, "assets/content/items/ammo/patrons/patron_12x70.bundle"),
];

/// Factory as the game loads it by day (`factory4_day`: the reworked
/// level's scenes; the night's, the older `Factory_*` and the sound's, the
/// AI's and the culling's left out). The same as `location_scenes` gives
/// for `factory4_day`, less `with_colliders`'.
pub const FACTORY_DAY: &[&str] = &[
    "Factory_Rework/Factory_Rework_Areas.unity",
    "Factory_Rework/Factory_Rework_Main_Building.unity",
    "Factory_Rework/Factory_Rework_Basement.unity",
    "Factory_Rework/Factory_Rework_Admin_Office.unity",
    "Factory_Rework/Factory_Rework_Laboratory.unity",
    "Factory_Rework/Factory_Rework_Background.unity",
    "Factory_Rework/Factory_Rework_DesignStuff.unity",
    "Factory_Rework/Factory_Rework_DesignMain.unity",
    "Factory_Rework/Factory_Rework_Quests.unity",
    "Factory_Rework/Factory_Rework_Day_Scripts.unity",
    "Factory_Rework/Factory_Rework_Day_Light.unity",
];

/// Every scene a location loads, in the order the game loads them, each
/// by its path in the build (`Assets/Content/Locations/...unity`): the
/// location's base in the database names its preset (`Scene`: the bundle
/// and the preset's name, `rcid`), and the game loads the preset's scenes
/// (`_scenesResourceKeys`) and then each child preset's in turn
/// (`LoadScenesFromPresetOperation`), all of them offline (`ScenesPreset.
/// ShouldLoadScene`: the scenes marked `_onlyOffline` are left out only
/// online). `location` is the base's folder or its `Id` in any case
/// (`factory4_day`, `RezervBase`, `bigmap`).
pub fn location_scenes(install: &Install, location: &str) -> Result<Vec<String>> {
    let base = install.database_dir().join("locations").join(location.to_lowercase()).join("base.json");
    let text = std::fs::read_to_string(&base).with_context(|| format!("{}", base.display()))?;
    let json: serde_json::Value = serde_json::from_str(&text).with_context(|| format!("{}", base.display()))?;
    let bundle = json.pointer("/Scene/path").and_then(|v| v.as_str()).with_context(|| format!("{}: no Scene.path", base.display()))?;
    let rcid = json.pointer("/Scene/rcid").and_then(|v| v.as_str()).with_context(|| format!("{}: no Scene.rcid", base.display()))?;
    // `Rezerv_Base.scenespreset.asset` names the object `Rezerv_Base.ScenesPreset`.
    let wanted = rcid.trim_end_matches(".asset").to_lowercase();
    let mut assets = crate::unity::assets::Assets::new(install.clone())?;
    let files = assets.load(bundle).with_context(|| bundle.to_owned())?;
    let mut presets = Vec::new();
    for &file in &files {
        for obj in assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>() {
            let value = assets.read(obj)?;
            if value.get("ServerName").is_some() {
                presets.push((obj, value));
            }
        }
    }
    let first = presets
        .iter()
        .position(|(_, v)| v.get("m_Name").and_then(Value::as_str).is_some_and(|n| n.to_lowercase() == wanted))
        .with_context(|| format!("{bundle}: no preset {rcid}"))?;
    let mut scenes = Vec::new();
    let mut queue = vec![first];
    let mut seen = std::collections::HashSet::new();
    while let Some(at) = queue.pop() {
        if !seen.insert(at) {
            continue;
        }
        let (obj, value) = &presets[at];
        for key in value.get("_scenesResourceKeys").and_then(Value::as_array).unwrap_or(&[]) {
            if let Some(path) = key.get("path").and_then(Value::as_str) {
                scenes.push(path.to_owned());
            }
        }
        // Its children after it, the first child first.
        let mut children = Vec::new();
        for child in value.get("ChildPresets").and_then(Value::as_array).unwrap_or(&[]) {
            let Some(pointer) = child.as_pptr().filter(|p| !p.is_null()) else { continue };
            let Some(target) = assets.resolve_loaded(obj.file, pointer) else { continue };
            if let Some(i) = presets.iter().position(|(o, _)| o.file == target.file && o.path_id == target.path_id) {
                children.push(i);
            }
        }
        queue.extend(children.into_iter().rev());
    }
    Ok(scenes)
}

/// Whether a scene of a location is read for the world's colliders: all
/// but its AI's, sound's and culling's (a part of its name `AI`, `Sound` or
/// `Culling`: `Factory_Rework_AI`, `Factory_Sound_Rework`,
/// `Reserve_Base_Culling`; as Factory's were left out: the game moves its
/// audio and culling triggers into other physics scenes, findings).
/// `PROVISIONAL` for the AI's.
pub fn with_colliders(scene: &str) -> bool {
    let stem = scene.rsplit('/').next().unwrap_or(scene).trim_end_matches(".unity");
    !stem.split('_').any(|part| matches!(part.to_lowercase().as_str(), "ai" | "sound" | "culling"))
}

/// A collider's shape, in its object's axes (a mesh's: in the world's).
#[derive(Debug, Clone)]
pub enum Shape {
    /// `m_Center`, `m_Size`, scaled.
    Box { centre: [f32; 3], size: [f32; 3] },
    Sphere { centre: [f32; 3], radius: f32 },
    /// Along x, y or z (0, 1, 2); its height end to end with the caps.
    Capsule { centre: [f32; 3], radius: f32, height: f32, direction: u8 },
    /// Its corners in the world's axes and its triangles; `convex`, the
    /// engine cooks its hull instead.
    Mesh { convex: bool, mesh: Arc<ColliderMesh> },
}

/// A mesh collider's mesh, its corners already in the world's axes.
#[derive(Debug, Clone)]
pub struct ColliderMesh {
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

/// A collider of the level.
#[derive(Debug, Clone)]
pub struct LevelCollider {
    /// Its scene and its object's path there.
    pub scene: String,
    pub path: String,
    /// Its object's layer (`gameObject.layer`).
    pub layer: u32,
    pub trigger: bool,
    /// Its object on in its scene and the collider enabled (always, but
    /// where read with those that are off).
    pub on: bool,
    /// Its object's place and turn in the world (a box's, sphere's or
    /// capsule's shape is laid in these; a mesh's corners are the world's
    /// already).
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub shape: Shape,
    /// The box it lies in, in the world (lowest, highest).
    pub bounds: ([f32; 3], [f32; 3]),
}

/// What was read, and what could not be, with why.
#[derive(Debug, Default)]
pub struct Level {
    pub colliders: Vec<LevelCollider>,
    pub skipped: Vec<String>,
}

/// Every collider that is on, on an object on in its scene, whose box in
/// the world comes within `reach` of `near`, of the scenes whose path in
/// the build ends with one of `scenes`.
pub fn read(install: &Install, scenes: &[&str], near: [f32; 3], reach: f32) -> Result<Level> {
    read_with(install, scenes, near, reach, false)
}

/// The same, with those that are off as well where `off_too` (each one's
/// `on` saying which: its object on in the scene and the collider
/// enabled).
pub fn read_with(install: &Install, scenes: &[&str], near: [f32; 3], reach: f32, off_too: bool) -> Result<Level> {
    read_near(install, scenes, &[near], reach, off_too)
}

/// The same, near any of several points (a recording's hits, spread over a
/// level): a collider whose box comes within `reach` of one of them.
pub fn read_near(install: &Install, scenes: &[&str], points: &[[f32; 3]], reach: f32, off_too: bool) -> Result<Level> {
    read_where(install, scenes, points, reach, None, off_too)
}

/// Every collider that is on, on an object on in its scene, of those
/// names, wherever it lies (a recording's hits by the names they met, its
/// place in the level not known).
pub fn read_named(install: &Install, scenes: &[&str], names: &[String], off_too: bool) -> Result<Level> {
    read_where(install, scenes, &[], 0.0, Some(names), off_too)
}

/// Near the points, or of the names where they are given.
fn read_where(install: &Install, scenes: &[&str], points: &[[f32; 3]], reach: f32, names: Option<&[String]>, off_too: bool) -> Result<Level> {
    let paths = scene_paths(install)?;
    let mut trees: HashMap<i32, TypeTree> = HashMap::new();
    for (class, bundle) in TREES {
        trees.insert(class, tree_of(install, bundle, class).with_context(|| format!("the tree of class {class}"))?);
    }
    let mesh_tree = tree_of(install, crate::audio::sound::EFFECTS, class::MESH).context("a mesh's tree")?;
    let mut pictures = ScenePictures::new(install)?;
    let folder = install.root.join("EscapeFromTarkov_Data");
    let stream = |name: &str, path: &str, offset: usize, size: usize| -> Result<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        let beside = folder.join(path.rsplit('/').next().unwrap_or(path));
        let mut file = std::fs::File::open(&beside).with_context(|| format!("{name}: {}", beside.display()))?;
        file.seek(SeekFrom::Start(offset as u64))?;
        let mut data = vec![0; size];
        file.read_exact(&mut data).with_context(|| format!("{name}: {}", beside.display()))?;
        Ok(data)
    };
    let mut level = Level::default();
    let mut meshes: HashMap<(String, i64), Option<Arc<MeshData>>> = HashMap::new();
    let mut local_boxes: HashMap<(String, i64), Option<([f32; 3], [f32; 3])>> = HashMap::new();
    let near_box = |low: [f32; 3], high: [f32; 3]| -> bool {
        if names.is_some() {
            return true;
        }
        points.iter().any(|near| {
            let gap: f32 = (0..3).map(|k| (low[k] - near[k]).max(near[k] - high[k]).max(0.0).powi(2)).sum();
            gap <= reach * reach
        })
    };
    for wanted in scenes {
        let Some(index) = paths.iter().position(|p| p.ends_with(wanted)) else {
            level.skipped.push(format!("no scene {wanted} in the build"));
            continue;
        };
        let file = format!("level{index}");
        let scene = Scene::load(install, &file, false).with_context(|| file.clone())?;
        for class in [MESH_COLLIDER, BOX_COLLIDER, SPHERE_COLLIDER, CAPSULE_COLLIDER] {
            let tree = &trees[&class];
            for (id, node) in scene.parts_of_class(class) {
                if names.is_some_and(|names| !names.contains(&scene.nodes[node].name)) {
                    continue;
                }
                let active = scene.active_in_hierarchy(node);
                if !active && !off_too {
                    continue;
                }
                let what = || format!("{wanted}: {}", scene.path(node));
                let value = match pictures.read_by(&file, id, class, tree) {
                    Ok(value) => value,
                    Err(e) => {
                        level.skipped.push(format!("{}: {e:#}", what()));
                        continue;
                    }
                };
                let on = active && value.get("m_Enabled").and_then(Value::as_bool) == Some(true);
                if !on && !off_too {
                    continue;
                }
                let trigger = value.get("m_IsTrigger").and_then(Value::as_bool).unwrap_or(false);
                let position = scene.to_world(node, [0.0; 3]);
                let rotation = scene.world_rotation(node);
                let lossy = scene.lossy_scale(node);
                let scaled = |v: [f32; 3]| [v[0] * lossy[0], v[1] * lossy[1], v[2] * lossy[2]];
                let in_world = |v: [f32; 3]| {
                    let t = crate::level::level_light::rotate(rotation, v);
                    [t[0] + position[0], t[1] + position[1], t[2] + position[2]]
                };
                let vec = |name: &str| value.get(name).and_then(Value::as_vec3).unwrap_or([0.0; 3]);
                let f = |name: &str| value.get(name).and_then(Value::as_f32).unwrap_or(0.0);
                let corners_box = |centre: [f32; 3], half: [f32; 3]| {
                    let mut low = [f32::MAX; 3];
                    let mut high = [f32::MIN; 3];
                    for corner in 0..8 {
                        let sign = |k: usize| if corner >> k & 1 == 1 { 1.0 } else { -1.0 };
                        let p = in_world([centre[0] + sign(0) * half[0], centre[1] + sign(1) * half[1], centre[2] + sign(2) * half[2]]);
                        for k in 0..3 {
                            low[k] = low[k].min(p[k]);
                            high[k] = high[k].max(p[k]);
                        }
                    }
                    (low, high)
                };
                let (shape, bounds) = match class {
                    BOX_COLLIDER => {
                        let (centre, size) = (scaled(vec("m_Center")), scaled(vec("m_Size")).map(f32::abs));
                        (Shape::Box { centre, size }, corners_box(centre, size.map(|s| s * 0.5)))
                    }
                    SPHERE_COLLIDER => {
                        let centre = scaled(vec("m_Center"));
                        let radius = f("m_Radius") * lossy.iter().fold(0.0f32, |m, s| m.max(s.abs()));
                        (Shape::Sphere { centre, radius }, corners_box(centre, [radius; 3]))
                    }
                    CAPSULE_COLLIDER => {
                        let centre = scaled(vec("m_Center"));
                        let direction = value.get("m_Direction").and_then(Value::as_i64).unwrap_or(1).clamp(0, 2) as u8;
                        let d = direction as usize;
                        let across = (0..3).filter(|&k| k != d).fold(0.0f32, |m, k| m.max(lossy[k].abs()));
                        let radius = f("m_Radius") * across;
                        let height = (f("m_Height") * lossy[d].abs()).max(2.0 * radius);
                        let mut half = [radius; 3];
                        half[d] = height * 0.5;
                        (Shape::Capsule { centre, radius, height, direction }, corners_box(centre, half))
                    }
                    _ => {
                        let Some(pointer) = value.get("m_Mesh").and_then(Value::as_pptr).filter(|p| !p.is_null()) else { continue };
                        let mesh_file = match pictures.file_name(&file, pointer.file_id) {
                            Ok(name) => name,
                            Err(e) => {
                                level.skipped.push(format!("{}: {e:#}", what()));
                                continue;
                            }
                        };
                        let key = (mesh_file, pointer.path_id);
                        // Its box first: a mesh far off is not unpacked.
                        let local_box = match local_boxes.get(&key) {
                            Some(&b) => b,
                            None => {
                                let b = pictures.read_by(&key.0, key.1, class::MESH, &mesh_tree).ok().and_then(|v| Some((v.at("m_LocalAABB/m_Center")?.as_vec3()?, v.at("m_LocalAABB/m_Extent")?.as_vec3()?)));
                                local_boxes.insert(key.clone(), b);
                                b
                            }
                        };
                        if let Some((centre, extent)) = local_box {
                            let mut low = [f32::MAX; 3];
                            let mut high = [f32::MIN; 3];
                            for corner in 0..8 {
                                let sign = |k: usize| if corner >> k & 1 == 1 { 1.0 } else { -1.0 };
                                let p = scene.to_world(node, [centre[0] + sign(0) * extent[0], centre[1] + sign(1) * extent[1], centre[2] + sign(2) * extent[2]]);
                                for k in 0..3 {
                                    low[k] = low[k].min(p[k]);
                                    high[k] = high[k].max(p[k]);
                                }
                            }
                            if !near_box(low, high) {
                                continue;
                            }
                        }
                        if !meshes.contains_key(&key) {
                            let read = pictures.read_by(&key.0, key.1, class::MESH, &mesh_tree).and_then(|v| mesh_streamed(&v, &stream));
                            match read {
                                Ok(mesh) => {
                                    meshes.insert(key.clone(), Some(Arc::new(mesh)));
                                }
                                Err(e) => {
                                    level.skipped.push(format!("{}: {e:#}", what()));
                                    meshes.insert(key.clone(), None);
                                }
                            }
                        }
                        let Some(data) = meshes[&key].clone() else { continue };
                        let positions: Vec<[f32; 3]> = data.positions.iter().map(|&p| scene.to_world(node, p)).collect();
                        let mut low = [f32::MAX; 3];
                        let mut high = [f32::MIN; 3];
                        for p in &positions {
                            for k in 0..3 {
                                low[k] = low[k].min(p[k]);
                                high[k] = high[k].max(p[k]);
                            }
                        }
                        let triangles = data.submeshes.iter().flat_map(|part| part.chunks_exact(3).map(|t| [t[0], t[1], t[2]])).collect();
                        let convex = value.get("m_Convex").and_then(Value::as_bool).unwrap_or(false);
                        (Shape::Mesh { convex, mesh: Arc::new(ColliderMesh { name: data.name.clone(), positions, triangles }) }, (low, high))
                    }
                };
                if !near_box(bounds.0, bounds.1) {
                    continue;
                }
                level.colliders.push(LevelCollider { scene: (*wanted).to_owned(), path: scene.path(node), layer: scene.layer(node), trigger, on, position, rotation, shape, bounds });
            }
        }
    }
    Ok(level)
}

/// A level's ballistic script (`BallisticCollider`, or its composer), on a
/// collider's object or its parent (`Shot.TryGetBallisticCollider`): what
/// a bullet meeting that object is given (`docs/spec/ballistics.md`,
/// "Surfaces"). Its fields as the scene saves them, in the class's order:
/// the base's `SurfaceSound`, then `_presetId`, `NetId`, `HitType`,
/// `_typeOfMaterial` and the six figures. A level's colliders keep their
/// own; only the terrain takes a preset's (`TerrainBallistic`).
#[derive(Debug, Clone)]
pub struct LevelBallistic {
    /// Its scene and its object's path there.
    pub scene: String,
    pub path: String,
    /// Its object's name (what the recorder writes as `met`).
    pub name: String,
    /// Its object's place in the world.
    pub position: [f32; 3],
    /// Its object on in its scene and the script enabled.
    pub on: bool,
    /// A composer (`BallisticColliderComposer`) rather than a plain one.
    pub composer: bool,
    /// `MaterialType`'s number.
    pub material: i32,
    /// The preset it was set from (-900: none).
    pub preset: i32,
    /// Penetration level, penetration chance, ricochet chance,
    /// fragmentation chance, deviation chance, deviation.
    pub figures: [f32; 6],
}

/// Every ballistic script of the scenes whose path in the build ends with
/// one of `scenes`.
pub fn ballistics(install: &Install, scenes: &[&str]) -> Result<Vec<LevelBallistic>> {
    let paths = scene_paths(install)?;
    let mut out = Vec::new();
    for wanted in scenes {
        let Some(index) = paths.iter().position(|p| p.ends_with(wanted)) else { continue };
        let file = format!("level{index}");
        let scene = Scene::load(install, &file, true).with_context(|| file.clone())?;
        for behaviour in &scene.behaviours {
            let composer = match scene.class_of(behaviour) {
                Some("BallisticCollider") => false,
                Some("BallisticColliderComposer") => true,
                _ => continue,
            };
            let node = behaviour.node;
            let what = || format!("{wanted}: {}", scene.path(node));
            let mut r = crate::unity::reader::Reader::new(scene.fields(behaviour));
            let mut ints = [0i32; 5];
            for int in &mut ints {
                *int = r.i32().with_context(what)?;
            }
            let mut figures = [0.0; 6];
            for figure in &mut figures {
                *figure = r.f32().with_context(what)?;
            }
            out.push(LevelBallistic {
                scene: (*wanted).to_owned(),
                path: scene.path(node),
                name: scene.nodes[node].name.clone(),
                position: scene.to_world(node, [0.0; 3]),
                on: behaviour.enabled && scene.active_in_hierarchy(node),
                composer,
                material: ints[4],
                preset: ints[1],
                figures,
            });
        }
    }
    Ok(out)
}
