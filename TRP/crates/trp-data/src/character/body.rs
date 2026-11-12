//! The player's body: its animator controller and the bones the
//! first-person view depends on. See `docs/spec/body-animator.md`.

use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};

use crate::weapons::states::StateTable;
use crate::unity::extract::{components, exported_game_objects, read_hierarchy, read_mesh, read_texture, script_class, MeshData, TextureData};
use crate::unity::{class, Assets, ObjRef, Value};

const PLAYER_BUNDLE: &str = "assets/content/commonprefabs/playersuperior.bundle";
const CONTROLLER_BUNDLE: &str = "assets/content/characters/controllers/player_anim_controller.bundle";
/// The controller the game asks that bundle for. The bundle also holds a
/// reworked copy and a backup under other names.
const CONTROLLER: &str = "Player_anim_controller";
/// The object the body's animator sits on; its clips name bones from there.
const ANIMATED_OBJECT: &str = "Player";

/// A bone of the player's skeleton at rest.
#[derive(Debug, Clone)]
pub struct Bone {
    /// Path below the animated object, as clips name it.
    pub path: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
}

/// The body animator's controller, read in full.
pub fn controller(assets: &mut Assets) -> Result<StateTable> {
    let files = assets.load(CONTROLLER_BUNDLE)?;
    let controller = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::ANIMATOR_CONTROLLER).collect::<Vec<_>>())
        .find(|obj| assets.name_of(*obj).as_deref() == Some(CONTROLLER))
        .ok_or_else(|| anyhow!("no controller named {CONTROLLER}"))?;
    StateTable::from_controller(assets, controller, &HashMap::new())
}

/// The rest pose of the named bones, each given by its path below the
/// animated object.
pub fn bones(assets: &mut Assets, paths: &[&str]) -> Result<Vec<Bone>> {
    bones_of(assets, PLAYER_BUNDLE, ANIMATED_OBJECT, paths)
}

/// The menus' own copy of the player (`PlayerModelLoader.CreatePlayerBody`
/// makes it from this bundle), whose animator is the body's controller on
/// an object of its own name: the game's screens show the character by it.
pub const MENU_PLAYER_BUNDLE: &str = "assets/content/commonprefabs/menuplayer.bundle";
const MENU_ANIMATED_OBJECT: &str = "MenuPlayer";

/// The rest pose of the named bones of the menus' copy of the player, each
/// by its path below its animated object (the same paths as the player's:
/// the clips name them so).
pub fn menu_bones(assets: &mut Assets, paths: &[&str]) -> Result<Vec<Bone>> {
    bones_of(assets, MENU_PLAYER_BUNDLE, MENU_ANIMATED_OBJECT, paths)
}

/// The shadow at the menu figure's feet (`MenuPlayerPoser.BottomShadow`,
/// the object `BootShadow` of the menu player's prefab, which
/// `PlayerModelLoader.CreatePlayerBody` switches on for the screens'
/// figure): flat planes on the prefab's root, under the soles, which turn
/// with the figure as it is dragged (inventory spec, "The shadow at the
/// feet").
#[derive(Debug, Clone)]
pub struct BootShadow {
    /// Its objects: those from below the prefab's root down to the shadow's
    /// own, then everything below that, each after its parent.
    pub nodes: Vec<ShadowNode>,
    /// The shadow's own object among them, which the game switches on
    /// whatever its own flag.
    pub switched: usize,
    /// The meshes and pictures its planes draw, each once.
    pub meshes: Vec<MeshData>,
    pub pictures: Vec<TextureData>,
}

/// One object of the shadow's: its parent among them (none: the prefab's
/// root), its own place, turn and scale (the engine's axes), whether it is
/// switched on itself (`m_IsActive`), and what it draws, where it draws.
#[derive(Debug, Clone)]
pub struct ShadowNode {
    pub name: String,
    pub parent: Option<usize>,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub active: bool,
    pub plane: Option<ShadowPlane>,
}

/// What an object of the shadow's draws: its mesh and its material's
/// picture (`_MainTex`) among the shadow's, and that picture's scale and
/// offset (`m_Scale`, `m_Offset`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowPlane {
    pub mesh: usize,
    pub picture: usize,
    pub scale: [f32; 2],
    pub offset: [f32; 2],
}

/// The shader the shadow's materials are drawn by: the texel times the
/// vertex colour, blended by its alpha for colour and alpha alike, unlit.
pub const BOOT_SHADOW_SHADER: &str = "Unlit/Transparent Colored";

/// The shadow at the menu figure's feet, read from the menu player's
/// prefab: the object its `MenuPlayerPoser` names, each object below it
/// with a mesh and a renderer switched on (its mesh the prefab's or one of
/// Unity's own, the plane's), each material checked to be drawn by
/// `BOOT_SHADOW_SHADER`, whose one property is its picture.
pub fn boot_shadow(assets: &mut Assets) -> Result<BootShadow> {
    let files = assets.load(MENU_PLAYER_BUNDLE)?;
    // Its materials' shader is in the shaders' bundle.
    assets.load("shaders")?;
    let poser = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>())
        .find(|&obj| script_class(assets, obj).as_deref() == Some("MenuPlayerPoser"))
        .ok_or_else(|| anyhow!("the menu player has no MenuPlayerPoser"))?;
    let named = assets.read(poser)?.get("BottomShadow").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(poser.file, p)).ok_or_else(|| anyhow!("the menu player's MenuPlayerPoser names no BottomShadow"))?;
    // A game object, or its transform.
    let transform = match assets.class_id(named) {
        Some(class::TRANSFORM) => named,
        Some(class::GAME_OBJECT) => {
            let go = assets.read(named)?;
            components(assets, named, &go).into_iter().find(|c| assets.class_id(*c) == Some(class::TRANSFORM)).ok_or_else(|| anyhow!("BottomShadow has no transform"))?
        }
        other => return Err(anyhow!("BottomShadow is of class {other:?}, not a game object")),
    };
    for file in files {
        for (_, root) in exported_game_objects(assets, file)? {
            let hierarchy = read_hierarchy(assets, root)?;
            let Some(at) = hierarchy.nodes.iter().position(|node| node.transform == transform) else { continue };
            let below = |mut node: usize| loop {
                match hierarchy.nodes[node].parent {
                    Some(parent) if parent == at => break true,
                    Some(parent) => node = parent,
                    None => break false,
                }
            };
            let mut chain = Vec::new();
            let mut node = at;
            while let Some(parent) = hierarchy.nodes[node].parent {
                chain.push(node);
                node = parent;
            }
            chain.reverse();
            let switched = chain.len() - 1;
            chain.extend((at + 1..hierarchy.nodes.len()).filter(|&node| below(node)));
            let mut shadow = BootShadow { nodes: Vec::new(), switched, meshes: Vec::new(), pictures: Vec::new() };
            let (mut meshes, mut pictures): (Vec<((usize, i32, i64), usize)>, Vec<ObjRef>) = (Vec::new(), Vec::new());
            for &index in &chain {
                let node = &hierarchy.nodes[index];
                let plane = shadow_plane(assets, node.transform, &mut shadow, &mut meshes, &mut pictures).with_context(|| format!("the menu player's shadow, {}", node.name))?;
                shadow.nodes.push(ShadowNode {
                    name: node.name.clone(),
                    parent: node.parent.and_then(|parent| chain.iter().position(|&kept| kept == parent)),
                    translation: node.translation,
                    rotation: node.rotation,
                    scale: node.scale,
                    active: node.active,
                    plane,
                });
            }
            return Ok(shadow);
        }
    }
    Err(anyhow!("the menu player prefab has no BottomShadow"))
}

/// What an object of the shadow's draws (`BootShadow`), its mesh and
/// picture read once each: `meshes` keeps each mesh read by the file
/// naming it, its reference there and its place in `shadow.meshes`;
/// `pictures` each picture's object, in `shadow.pictures`' order.
fn shadow_plane(assets: &Assets, transform: ObjRef, shadow: &mut BootShadow, meshes: &mut Vec<((usize, i32, i64), usize)>, pictures: &mut Vec<ObjRef>) -> Result<Option<ShadowPlane>> {
    let go_ref = assets.read(transform)?.req("m_GameObject")?.as_pptr().and_then(|p| assets.resolve_loaded(transform.file, p)).context("a transform without its game object")?;
    let go = assets.read(go_ref)?;
    let (mut mesh, mut material) = (None, None);
    for comp in components(assets, go_ref, &go) {
        match assets.class_id(comp) {
            Some(class::MESH_FILTER) => mesh = assets.read(comp)?.req("m_Mesh")?.as_pptr().filter(|p| p.path_id != 0).map(|p| (comp.file, p)),
            Some(class::MESH_RENDERER) => {
                let renderer = assets.read(comp)?;
                if renderer.get("m_Enabled").and_then(Value::as_bool).unwrap_or(true) {
                    material = renderer.req("m_Materials")?.as_array().and_then(|list| list.first()).and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(comp.file, p));
                }
            }
            _ => {}
        }
    }
    let (Some((file, pointer)), Some(material)) = (mesh, material) else { return Ok(None) };
    // Its material: the shader checked, its picture and that picture's place.
    let m = assets.read(material)?;
    let shader = m.req("m_Shader")?.as_pptr().and_then(|p| assets.resolve_loaded(material.file, p)).context("its material's shader is in no bundle loaded")?;
    let name = assets.read(shader)?.at("m_ParsedForm.m_Name").and_then(Value::as_str).unwrap_or("").to_owned();
    anyhow::ensure!(name == BOOT_SHADOW_SHADER, "its material is drawn by {name:?}, not {BOOT_SHADOW_SHADER:?}");
    let env = m.req("m_SavedProperties.m_TexEnvs")?.as_array().unwrap_or(&[]).iter().find(|pair| pair.get("first").and_then(Value::as_str) == Some("_MainTex")).map(|pair| pair.req("second")).transpose()?.context("its material has no _MainTex")?;
    let picture = env.req("m_Texture")?.as_pptr().and_then(|p| assets.resolve_loaded(material.file, p)).context("its material's picture is in no bundle loaded")?;
    let pair = |name: &str| -> [f32; 2] { [env.at(&format!("{name}.x")).and_then(Value::as_f32).unwrap_or(0.0), env.at(&format!("{name}.y")).and_then(Value::as_f32).unwrap_or(0.0)] };
    let (scale, offset) = (pair("m_Scale"), pair("m_Offset"));
    let picture = match pictures.iter().position(|&had| had == picture) {
        Some(at) => at,
        None => {
            shadow.pictures.push(read_texture(assets, picture)?);
            pictures.push(picture);
            pictures.len() - 1
        }
    };
    // Its mesh: the prefab's, or one of Unity's own.
    let key = (file, pointer.file_id, pointer.path_id);
    let mesh = match meshes.iter().find(|had| had.0 == key) {
        Some(had) => had.1,
        None => {
            let data = match assets.resolve_loaded(file, pointer) {
                Some(obj) => read_mesh(assets, obj)?,
                None => crate::drawing::particles::builtin_mesh(assets, file, pointer)?,
            };
            shadow.meshes.push(data);
            meshes.push((key, shadow.meshes.len() - 1));
            shadow.meshes.len() - 1
        }
    };
    Ok(Some(ShadowPlane { mesh, picture, scale, offset }))
}

fn bones_of(assets: &mut Assets, bundle: &str, animated: &str, paths: &[&str]) -> Result<Vec<Bone>> {
    for file in assets.load(bundle)? {
        for (_, root) in exported_game_objects(assets, file)? {
            let hierarchy = read_hierarchy(assets, root)?;
            // The animated object is the prefab's root (whose own path is
            // empty) or below it.
            let Some(at) = hierarchy.find_below(0, animated) else { continue };
            let prefix = match hierarchy.path(at) {
                own if own.is_empty() => own,
                own => format!("{own}/"),
            };
            let by_path: HashMap<String, usize> = (0..hierarchy.nodes.len())
                .filter_map(|i| Some((hierarchy.path(i).strip_prefix(&prefix)?.to_owned(), i)))
                .collect();
            return paths
                .iter()
                .map(|path| {
                    let node = &hierarchy.nodes[*by_path.get(*path).ok_or_else(|| anyhow!("the player has no bone {path}"))?];
                    Ok(Bone { path: (*path).to_owned(), position: node.translation, rotation: node.rotation })
                })
                .collect();
        }
    }
    Err(anyhow!("the player prefab was not found"))
}

/// The bones a weapon not in hand hangs on (`PlayerBones`: `HolsterPrimary`
/// and its alternative for the sling, `HolsterSecondary` and its
/// alternative for the back, `HolsterPistol` and `LeftLegHolsterPistol`
/// for the holster, `ScabbardTagillaMelee` for the scabbard;
/// `PlayerBody.GetSlotBone`, `GetAlternativeHolsterBone`), each by its
/// path below the animated object; none where the prefab names none.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SlotBones {
    pub primary: Option<String>,
    pub primary_alternative: Option<String>,
    pub secondary: Option<String>,
    pub secondary_alternative: Option<String>,
    pub pistol: Option<String>,
    pub pistol_left: Option<String>,
    pub scabbard: Option<String>,
}

/// The slot bones, from the player prefab's `PlayerBones`.
pub fn slot_bones(assets: &mut Assets) -> Result<SlotBones> {
    let files = assets.load(PLAYER_BUNDLE)?;
    let script = files
        .iter()
        .flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR))
        .find(|&obj| crate::unity::extract::script_class(assets, obj).as_deref() == Some("PlayerBones"))
        .ok_or_else(|| anyhow!("the player prefab has no PlayerBones"))?;
    let fields = assets.read(script)?;
    for file in files {
        for (_, root) in exported_game_objects(assets, file)? {
            let hierarchy = read_hierarchy(assets, root)?;
            if hierarchy.find_below(0, ANIMATED_OBJECT).is_none() {
                continue;
            }
            let prefix = format!("{ANIMATED_OBJECT}/");
            let path_of = |name: &str| -> Option<String> {
                let transform = fields.get(name)?.as_pptr().and_then(|p| assets.resolve_loaded(script.file, p))?;
                let node = hierarchy.nodes.iter().position(|node| node.transform == transform)?;
                hierarchy.path(node).strip_prefix(&prefix).map(str::to_owned)
            };
            return Ok(SlotBones {
                primary: path_of("HolsterPrimary"),
                primary_alternative: path_of("HolsterPrimaryAlternative"),
                secondary: path_of("HolsterSecondary"),
                secondary_alternative: path_of("HolsterSecondaryAlternative"),
                pistol: path_of("HolsterPistol"),
                pistol_left: path_of("LeftLegHolsterPistol"),
                scabbard: path_of("ScabbardTagillaMelee"),
            });
        }
    }
    Err(anyhow!("the player prefab was not found"))
}
