//! A weapon of one's own: a model that is not the install's, hung on the
//! rig of one of the install's weapons and worked by that weapon's
//! animator (`docs/spec/custom-weapons.md`).
//!
//! Such a weapon is described by a file in the workspace's `custom`
//! folder. It becomes an item of the database like any other (its
//! template the donor's with what the file changes, a magazine of its
//! own, a stock preset), and it is assembled as the game assembles a
//! weapon, with two differences: what is drawn is the model's own parts,
//! each hung on one of the donor's bones, and the pose the donor's
//! animator gives is edited, by rules the file states, so that the hands
//! and the moving parts fit the model.
//!
//! Nothing here is read from the install but through the donor; the model
//! and its pictures stay where the file says they are.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, bail, ensure, Context, Result};
use serde::Deserialize;
use serde_json::Value as Json;

use crate::database::{Database, ItemTemplate, Preset, PresetItem};
use crate::fbx;
use crate::unity::extract::{quat_mul, Hierarchy, MeshData, Node};
use crate::unity::ObjRef;

/// The bone of a weapon's rig the weapon itself hangs from, by its path.
pub const WEAPON_BONE: &str = "Weapon_root/Weapon_root_anim/weapon";
/// The markers the left hand goes by: where it is to be, and the fixed
/// reference for "on the weapon".
const LEFT_MARKER: &str = "weapon_L_IK_marker";
const LEFT_REFERENCE: &str = "weapon_L_hand_marker";
/// How near a palm counts as on a hold, in metres: what keeps the weight
/// of a hold the palm is exactly on from being without end.
const ON_A_HOLD: f32 = 0.005;

/// A weapon of one's own, as its file describes it.
#[derive(Debug, Clone, Deserialize)]
pub struct Definition {
    /// The item template's id and its name, in the database's manner.
    pub id: String,
    pub name: String,
    /// What it is called where a short name is shown.
    #[serde(default)]
    pub short_name: String,
    /// The install's weapon whose rig and animator it borrows.
    pub donor: String,
    pub model: Model,
    /// The model's parts, each on one of the donor's bones.
    pub parts: Vec<PartDef>,
    /// Bones the donor's bare prefab has not, which a weapon's parts
    /// would bring: the muzzle, the sights' points.
    #[serde(default)]
    pub bones: Vec<BoneDef>,
    /// Where the left hand holds this weapon, set beside where it holds
    /// the donor.
    #[serde(default)]
    pub hands: Option<HandsDef>,
    /// What its item template has other than the donor's.
    #[serde(default)]
    pub template: Changes,
    /// What it weighs with its empty magazine fitted, in kilograms: the
    /// template's own weight is this less the magazine's.
    #[serde(default)]
    pub built_weight: Option<f32>,
    pub magazine: MagazineDef,
    /// The folder the file's paths are from: the workspace's root.
    #[serde(skip)]
    pub root: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Model {
    /// A binary FBX file whose barrel runs along -z with y up.
    pub file: String,
    /// The folder of its pictures, and how one is named: `{material}`
    /// and `{map}` stand for the material's name in the model and for
    /// one of `BaseColor`, `Normal`, `Roughness`, `Metallic`, `AO`.
    pub textures: String,
    pub texture: String,
    /// Where the model's own origin lies on the donor's weapon bone, in
    /// centimetres: ahead along the barrel, up, and to the right.
    pub origin: [f32; 3],
    /// The most pixels a picture is drawn with along a side; larger ones
    /// are halved until they are no larger.
    #[serde(default = "default_texture_size")]
    pub texture_size: u32,
    /// Whether the normal pictures have green pointing down the picture
    /// (the convention of DirectX) and not up it.
    #[serde(default)]
    pub normal_green_down: bool,
}

fn default_texture_size() -> u32 {
    2048
}

#[derive(Debug, Clone, Deserialize)]
pub struct PartDef {
    /// The model's name in the file.
    pub model: String,
    /// The donor's bone it moves with, by its path below the weapon's
    /// bone (`weapon` for that bone itself).
    pub bone: String,
    /// Where on the model that bone's own origin is to be, in the
    /// model's units and axes: the point a part turns about. The bone is
    /// moved there from where the donor has it.
    #[serde(default)]
    pub pivot: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BoneDef {
    pub name: String,
    /// The bone it hangs from, as a part's.
    pub parent: String,
    /// Where it is on the model, in the model's units and axes.
    pub at: [f32; 3],
    /// A bone of the donor's whose turning it takes; turned as the
    /// weapon's own bone if none.
    #[serde(default)]
    pub turn_as: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HandsDef {
    /// How sharply the nearest hold has the say: the weight of a hold
    /// falls with the palm's distance from it to this power.
    #[serde(default = "default_exponent")]
    pub exponent: f32,
    pub holds: Vec<HoldDef>,
}

fn default_exponent() -> f32 {
    3.0
}

/// A thing the left hand holds: on the donor it is at a bone, and the
/// palm's marker is at one of `palms` from that bone while the hand is on
/// it; on this weapon the same thing lies `shift` from there, or, where
/// none is given, as far as a part's pivot has moved that bone. All in
/// centimetres ahead, up and to the right on the weapon's bone.
#[derive(Debug, Clone, Deserialize)]
pub struct HoldDef {
    pub name: String,
    pub bone: String,
    pub palms: Vec<[f32; 3]>,
    #[serde(default)]
    pub shift: Option<[f32; 3]>,
    /// The hold the hand rests in: the fixed reference for "on the
    /// weapon" moves with it.
    #[serde(default)]
    pub rest: bool,
}

/// What a template has other than the one it is made from: properties
/// taken from other templates of the install, the figures the game adds
/// up over a build taken as those of another weapon's stock build, and
/// properties set outright, in that order.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Changes {
    #[serde(default)]
    pub take: Vec<Take>,
    #[serde(default)]
    pub as_built: Option<String>,
    #[serde(default)]
    pub set: serde_json::Map<String, Json>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Take {
    pub from: String,
    pub props: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MagazineDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub short_name: String,
    /// The install's magazine its template is made from.
    pub clone: String,
    #[serde(flatten)]
    pub changes: Changes,
}

/// The workspace's folder of such files: the nearest `custom` folder
/// beside a `Cargo.toml`, from where the program runs upwards.
pub fn folder() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if dir.join("custom").is_dir() && dir.join("Cargo.toml").is_file() {
            return Some(dir.join("custom"));
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Reads every definition of a folder. One that cannot be read, or whose
/// model is not there, is left out with a word on why.
pub fn definitions(folder: &Path) -> (Vec<Definition>, Vec<String>) {
    let (mut found, mut notes) = (Vec::new(), Vec::new());
    let root = folder.parent().unwrap_or(folder).to_path_buf();
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder).into_iter().flatten().flatten().map(|entry| entry.path()).filter(|path| path.extension().is_some_and(|e| e == "json")).collect();
    files.sort();
    for file in files {
        let read = || -> Result<Definition> {
            let text = std::fs::read_to_string(&file)?;
            let mut definition: Definition = serde_json::from_str(&text)?;
            definition.root = root.clone();
            ensure!(definition.model_file().is_file(), "its model {} is not there", definition.model_file().display());
            Ok(definition)
        };
        match read() {
            Ok(definition) => found.push(definition),
            Err(e) => notes.push(format!("{}: {e:#}", file.display())),
        }
    }
    (found, notes)
}

impl Definition {
    pub fn model_file(&self) -> PathBuf {
        self.root.join(&self.model.file)
    }

    /// A point of the model, in its own units and axes, on the donor's
    /// weapon bone: metres to the right, back and up. `unit` is how many
    /// centimetres one of the model's units is.
    fn on_bone(&self, p: [f32; 3], unit: f32) -> [f32; 3] {
        let [ahead, up, right] = self.model.origin;
        [(p[0] * unit + right) * 0.01, (p[2] * unit - ahead) * 0.01, (p[1] * unit + up) * 0.01]
    }

    /// The item templates this adds to the database, the weapon's and
    /// its magazine's, each with what it is called for short.
    pub fn names(&self) -> [(&str, &str); 2] {
        fn short<'a>(short: &'a str, long: &'a str) -> &'a str {
            if short.is_empty() {
                long
            } else {
                short
            }
        }
        [(&self.id, short(&self.short_name, &self.name)), (&self.magazine.id, short(&self.magazine.short_name, &self.magazine.name))]
    }
}

/// Makes a template's properties what the changes say. `fitted` is what
/// is always fitted to the thing (a weapon's own magazine), whose shares
/// are taken off what a build's figures are set to: the weapon with it
/// fitted then has that build's figures.
fn change(db: &Database, props: &mut Json, changes: &Changes, fitted: Option<&ItemTemplate>) -> Result<()> {
    let map = props.as_object_mut().ok_or_else(|| anyhow!("a template without properties"))?;
    for take in &changes.take {
        let from = db.item(&take.from)?;
        for name in &take.props {
            let value = from.props.get(name).ok_or_else(|| anyhow!("{} has no {name}", from.name))?;
            map.insert(name.clone(), value.clone());
        }
    }
    if let Some(built) = &changes.as_built {
        // What the game adds up over a build: the parts' ergonomics, and
        // their shares of the recoil, which scale both of its forces.
        let preset = db.default_preset(built)?;
        let base = db.item(built)?;
        let parts = || preset.items.iter().filter(|item| item.id != preset.root).filter_map(|item| db.items.get(&item.template));
        let own = |name: &str| fitted.and_then(|part| part.prop_f32(name)).unwrap_or(0.0) as f64;
        let ergonomics: f64 = base.prop_f32("Ergonomics").unwrap_or(0.0) as f64 + parts().filter_map(|part| part.prop_f32("Ergonomics")).map(f64::from).sum::<f64>() - own("Ergonomics");
        let recoil: f64 = (1.0 + parts().filter_map(|part| part.prop_f32("Recoil")).map(f64::from).sum::<f64>() / 100.0).max(0.0) / (1.0 + own("Recoil") / 100.0).max(0.01);
        map.insert("Ergonomics".into(), ergonomics.into());
        for force in ["RecoilForceUp", "RecoilForceBack"] {
            let own = base.prop_f32(force).ok_or_else(|| anyhow!("{} has no {force}", base.name))? as f64;
            map.insert(force.into(), (own * recoil).into());
        }
    }
    for (name, value) in &changes.set {
        map.insert(name.clone(), value.clone());
    }
    Ok(())
}

/// Adds a weapon of one's own to the database: its template, its
/// magazine's and its stock preset.
pub fn register(db: &mut Database, definition: Definition) -> Result<()> {
    ensure!(!db.items.contains_key(&definition.id) && !db.items.contains_key(&definition.magazine.id), "its ids are taken");
    let named = |props: &mut Json, name: &str| {
        for key in ["Name", "ShortName", "Description"] {
            props[key] = name.into();
        }
    };
    let mut magazine: ItemTemplate = db.item(&definition.magazine.clone)?.clone();
    magazine.id = definition.magazine.id.clone();
    magazine.name = definition.magazine.name.clone();
    change(db, &mut magazine.props, &definition.magazine.changes, None).context("its magazine")?;
    named(&mut magazine.props, &definition.magazine.name);

    let mut weapon: ItemTemplate = db.item(&definition.donor)?.clone();
    weapon.id = definition.id.clone();
    weapon.name = definition.name.clone();
    change(db, &mut weapon.props, &definition.template, Some(&magazine))?;
    named(&mut weapon.props, &definition.name);
    // It is one piece: of the donor's slots only the magazine's is kept,
    // and that takes its own magazine.
    let slots = weapon.props.get_mut("Slots").and_then(Json::as_array_mut).ok_or_else(|| anyhow!("the donor has no slots"))?;
    slots.retain(|slot| slot.get("_name").and_then(Json::as_str) == Some("mod_magazine"));
    let slot = slots.first_mut().ok_or_else(|| anyhow!("the donor takes no magazine"))?;
    slot["_parent"] = definition.id.clone().into();
    slot["_props"]["filters"] = serde_json::json!([{ "Filter": [definition.magazine.id] }]);
    weapon.props["defMagType"] = definition.magazine.id.clone().into();
    if let Some(built) = definition.built_weight {
        let own = built - magazine.prop_f32("Weight").unwrap_or(0.0);
        ensure!(own > 0.0, "it would weigh nothing without its magazine");
        // Kept to the gram, as the database keeps weights.
        weapon.props["Weight"] = ((own as f64 * 1000.0).round() / 1000.0).into();
    }

    let instance = |n: u32| format!("{}-{n}", definition.id);
    let preset = Preset {
        id: format!("{}-preset", definition.id),
        name: definition.name.clone(),
        root: instance(0),
        items: vec![
            PresetItem { id: instance(0), template: definition.id.clone(), parent: None, slot: None },
            PresetItem { id: instance(1), template: definition.magazine.id.clone(), parent: Some(instance(0)), slot: Some("mod_magazine".into()) },
        ],
        encyclopedia: Some(definition.id.clone()),
    };
    db.items.insert(magazine.id.clone(), magazine);
    db.items.insert(weapon.id.clone(), weapon);
    db.presets.push(preset);
    db.custom.insert(definition.id.clone(), Arc::new(definition));
    Ok(())
}

/// Adds every weapon of the workspace's folder to the database. Whatever
/// is left out is noted in `Database::custom_notes`.
pub fn register_all(db: &mut Database) {
    let Some(folder) = folder() else { return };
    let (found, mut notes) = definitions(&folder);
    for definition in found {
        let name = definition.name.clone();
        if let Err(e) = register(db, definition) {
            notes.push(format!("{name}: {e:#}"));
        }
    }
    db.custom_notes.extend(notes);
}

type Placed = ([f32; 3], [f32; 4]);

fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let t = [2.0 * (q[1] * v[2] - q[2] * v[1]), 2.0 * (q[2] * v[0] - q[0] * v[2]), 2.0 * (q[0] * v[1] - q[1] * v[0])];
    [
        v[0] + q[3] * t[0] + q[1] * t[2] - q[2] * t[1],
        v[1] + q[3] * t[1] + q[2] * t[0] - q[0] * t[2],
        v[2] + q[3] * t[2] + q[0] * t[1] - q[1] * t[0],
    ]
}

fn back(q: [f32; 4]) -> [f32; 4] {
    [-q[0], -q[1], -q[2], q[3]]
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Centimetres ahead, up and to the right, as metres on the weapon's
/// bone: to the right, back and up.
fn of_centimetres(v: [f32; 3]) -> [f32; 3] {
    [v[2] * 0.01, -v[0] * 0.01, v[1] * 0.01]
}

/// One of the left hand's holds, on the rig's own bones.
#[derive(Debug, Clone)]
struct Hold {
    /// The bone it is at, or none for the weapon's own.
    bone: Option<usize>,
    /// Where the palm's marker is from that bone while the hand is on
    /// it, in the bone's own axes.
    palms: Vec<[f32; 3]>,
    /// How far from there the same thing lies on this weapon, on the
    /// weapon's bone.
    shift: [f32; 3],
}

/// The edits made to every pose the donor's animator gives
/// (`docs/spec/custom-weapons.md`, "The animation, edited"). They are
/// rules on the pose, not on a clip, so that they hold through every
/// state and every blend alike:
///
/// - **A bone moved**: the same shift is added to where the animator
///   has the bone, whatever it is doing. A part then turns about a point
///   of its own and travels the donor's path from the model's own place.
/// - **The left hand follows what it holds**: the palm's marker is
///   shifted by what the things it is near are shifted by, each weighted
///   by how near the palm is to where it holds that thing on the donor.
#[derive(Debug, Clone)]
pub struct PoseEdits {
    parents: Vec<Option<usize>>,
    weapon: usize,
    /// Each bone moved, with its shift on the weapon's bone.
    moved: Vec<(usize, [f32; 3])>,
    /// The left palm's marker and its fixed reference, with the holds.
    hand: Option<(usize, usize, Vec<Hold>, [f32; 3], f32)>,
    /// Where the donor's prefab has each bone these edits move, in its
    /// parent: what its animator is to take as the bone's rest.
    donor_rest: Vec<(usize, [f32; 3])>,
}

impl PoseEdits {
    /// A bone on the weapon's own bone, as a pose has it.
    fn on_weapon(&self, pose: &[Placed], bone: usize) -> Placed {
        let (mut at, mut turn) = pose[bone];
        let mut above = self.parents[bone];
        while let Some(parent) = above.filter(|parent| *parent != self.weapon) {
            let (place, parent_turn) = pose[parent];
            at = add(place, rotate(parent_turn, at));
            turn = quat_mul(parent_turn, turn);
            above = self.parents[parent];
        }
        (at, turn)
    }

    /// How a bone's parent is turned on the weapon's bone.
    fn parent_turn(&self, pose: &[Placed], bone: usize) -> [f32; 4] {
        match self.parents[bone].filter(|parent| *parent != self.weapon) {
            Some(parent) => self.on_weapon(pose, parent).1,
            None => [0.0, 0.0, 0.0, 1.0],
        }
    }

    /// What the left palm's marker is shifted by in a pose as the
    /// donor's clips have it.
    fn hand_shift(&self, pose: &[Placed]) -> [f32; 3] {
        let Some((marker, _, holds, _, exponent)) = &self.hand else { return [0.0; 3] };
        let palm = pose[*marker].0;
        let (mut total, mut sum) = (0.0f32, [0.0f32; 3]);
        for hold in holds {
            let (at, turn) = hold.bone.map_or(([0.0; 3], [0.0, 0.0, 0.0, 1.0]), |bone| self.on_weapon(pose, bone));
            let away = hold
                .palms
                .iter()
                .map(|offset| {
                    let d = sub(palm, add(at, rotate(turn, *offset)));
                    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
                })
                .fold(f32::MAX, f32::min);
            let weight = (away + ON_A_HOLD).powf(-exponent);
            total += weight;
            sum = add(sum, hold.shift.map(|s| s * weight));
        }
        if total > 0.0 {
            sum.map(|s| s / total)
        } else {
            [0.0; 3]
        }
    }

    /// Edits a pose the donor's animator has given: every bone of the
    /// rig, each in its parent.
    pub fn apply(&self, pose: &mut [Placed]) {
        // The hand first, from the pose as the donor has it.
        let shift = self.hand_shift(pose);
        let turns: Vec<[f32; 4]> = self.moved.iter().map(|(bone, _)| self.parent_turn(pose, *bone)).collect();
        for ((bone, by), parent_turn) in self.moved.iter().zip(turns) {
            pose[*bone].0 = add(pose[*bone].0, rotate(back(parent_turn), *by));
        }
        if let Some((marker, reference, _, rest, _)) = &self.hand {
            pose[*marker].0 = add(pose[*marker].0, shift);
            pose[*reference].0 = add(pose[*reference].0, *rest);
        }
    }

    /// Where the donor's own prefab has the bones these edits move, each
    /// in its parent. The weapon's hierarchy has them as edited.
    pub fn donor_rest(&self) -> &[(usize, [f32; 3])] {
        &self.donor_rest
    }
}

/// A picture file of each kind a material has.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Material {
    pub name: String,
    pub base_color: Option<PathBuf>,
    pub normal: Option<PathBuf>,
    pub roughness: Option<PathBuf>,
    pub metallic: Option<PathBuf>,
    pub occlusion: Option<PathBuf>,
    /// The largest side a picture is drawn with, and which way green
    /// points in the normal picture.
    pub size: u32,
    pub normal_green_down: bool,
}

/// One of the model's parts as it is drawn: a mesh on a node of the
/// weapon's hierarchy, in the axes of the weapon's own bone, with the
/// material of each of its submeshes.
#[derive(Debug, Clone)]
pub struct Part {
    pub node: usize,
    pub mesh: Arc<MeshData>,
    pub materials: Vec<usize>,
}

/// What a weapon of one's own has beyond a weapon of the install's.
#[derive(Debug, Clone)]
pub struct Build {
    pub parts: Vec<Part>,
    pub materials: Vec<Material>,
    pub edits: Arc<PoseEdits>,
}

/// A model's mesh on the weapon's bone, in the manner of the install's
/// own meshes: left-handed, a submesh for each material, a tangent at
/// every vertex.
fn mesh_of(definition: &Definition, model: &fbx::Model, unit: f32) -> Option<MeshData> {
    let source = model.mesh.as_ref()?;
    let mut mesh = MeshData { name: model.name.clone(), ..MeshData::default() };
    let slots = source.materials.iter().copied().max().map_or(1, |most| most as usize + 1);
    mesh.submeshes = vec![Vec::new(); slots];
    // Corners that are the same in place, normal and picture are one vertex.
    let mut seen: HashMap<[u32; 8], u32> = HashMap::new();
    let mut vertex = |corner: usize| -> u32 {
        let (p, n, uv) = (source.positions[corner], source.normals[corner], source.uvs[corner]);
        let key = [p[0].to_bits(), p[1].to_bits(), p[2].to_bits(), n[0].to_bits(), n[1].to_bits(), n[2].to_bits(), uv[0].to_bits(), uv[1].to_bits()];
        let next = mesh.positions.len() as u32;
        *seen.entry(key).or_insert_with(|| {
            mesh.positions.push(definition.on_bone(p, unit));
            // The same change of axes as a point's, without its place.
            mesh.normals.push([n[0], n[2], n[1]]);
            mesh.uv0.push(uv);
            next
        })
    };
    // The change of axes is a mirroring (the model's are right-handed,
    // the rig's left-handed), which turns every triangle's face the
    // other way: two of its corners change places to turn it back.
    let mut triangles: Vec<(usize, [u32; 3])> = Vec::with_capacity(source.positions.len() / 3);
    for triangle in 0..source.positions.len() / 3 {
        let corner = triangle * 3;
        triangles.push((source.materials[triangle] as usize, [vertex(corner), vertex(corner + 2), vertex(corner + 1)]));
    }
    for (slot, corners) in triangles {
        mesh.submeshes[slot].extend(corners);
    }
    // A tangent along each vertex's picture, as the engine keeps one: the
    // way the picture's u runs over the surface, square to the normal,
    // and a sign for which way its v then runs.
    let count = mesh.positions.len();
    let (mut along, mut across) = (vec![[0.0f32; 3]; count], vec![[0.0f32; 3]; count]);
    for triangle in mesh.submeshes.iter().flat_map(|indices| indices.chunks_exact(3)) {
        let [a, b, c] = [triangle[0] as usize, triangle[1] as usize, triangle[2] as usize];
        let (e1, e2) = (sub(mesh.positions[b], mesh.positions[a]), sub(mesh.positions[c], mesh.positions[a]));
        let (du1, dv1) = (mesh.uv0[b][0] - mesh.uv0[a][0], mesh.uv0[b][1] - mesh.uv0[a][1]);
        let (du2, dv2) = (mesh.uv0[c][0] - mesh.uv0[a][0], mesh.uv0[c][1] - mesh.uv0[a][1]);
        let det = du1 * dv2 - du2 * dv1;
        if det.abs() < 1e-12 {
            continue;
        }
        let t: [f32; 3] = std::array::from_fn(|k| (e1[k] * dv2 - e2[k] * dv1) / det);
        let s: [f32; 3] = std::array::from_fn(|k| (e2[k] * du1 - e1[k] * du2) / det);
        for vertex in [a, b, c] {
            along[vertex] = add(along[vertex], t);
            across[vertex] = add(across[vertex], s);
        }
    }
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    mesh.tangents = (0..count)
        .map(|vertex| {
            let n = mesh.normals[vertex];
            let flat = sub(along[vertex], n.map(|c| c * dot(n, along[vertex])));
            let length = dot(flat, flat).sqrt();
            // A vertex whose picture does not move over the surface has
            // none of its own: any way square to the normal.
            let t = if length > 1e-9 {
                flat.map(|c| c / length)
            } else {
                let other = if n[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
                let side = cross(n, other);
                let length = dot(side, side).sqrt().max(1e-9);
                side.map(|c| c / length)
            };
            let sign = if dot(cross(n, t), across[vertex]) < 0.0 { -1.0 } else { 1.0 };
            [t[0], t[1], t[2], sign]
        })
        .collect();
    Some(mesh)
}

/// Makes the donor's bare rig this weapon's: nothing of the donor's is
/// drawn, the model's parts hang on its bones, the bones a weapon's
/// parts would bring are added, and every bone the edits move rests
/// where the edits have it.
pub fn build(definition: &Definition, hierarchy: &mut Hierarchy) -> Result<Build> {
    let data = std::fs::read(definition.model_file()).with_context(|| format!("reading {}", definition.model_file().display()))?;
    let scene = fbx::read(&data)?;
    ensure!(scene.up == (1, 1.0), "the model's up is not +y");
    let unit = scene.unit as f32;

    let weapon = (0..hierarchy.nodes.len()).find(|&i| hierarchy.path(i) == WEAPON_BONE).ok_or_else(|| anyhow!("the donor has no weapon bone"))?;
    let below = |hierarchy: &Hierarchy, mut node: usize| loop {
        if node == weapon {
            return true;
        }
        match hierarchy.nodes[node].parent {
            Some(parent) => node = parent,
            None => return false,
        }
    };
    // Nothing of the donor's own is drawn.
    for index in 0..hierarchy.nodes.len() {
        if below(hierarchy, index) {
            hierarchy.nodes[index].renderer = None;
        }
    }
    let bone = |hierarchy: &Hierarchy, path: &str| -> Result<usize> {
        let full = if path == "weapon" { WEAPON_BONE.to_owned() } else { format!("{WEAPON_BONE}/{path}") };
        (0..hierarchy.nodes.len()).find(|&i| hierarchy.path(i) == full).ok_or_else(|| anyhow!("the donor has no bone {path}"))
    };

    // The donor's bones at rest, and the edits on them.
    let parents: Vec<Option<usize>> = hierarchy.nodes.iter().map(|node| node.parent).collect();
    let mut edits = PoseEdits { parents, weapon, moved: Vec::new(), hand: None, donor_rest: Vec::new() };
    let rest: Vec<Placed> = hierarchy.nodes.iter().map(|node| (node.translation, node.rotation)).collect();
    let at_rest = |edits: &PoseEdits, node: usize| if node == weapon { ([0.0; 3], [0.0, 0.0, 0.0, 1.0]) } else { edits.on_weapon(&rest, node) };
    for part in &definition.parts {
        let Some(pivot) = part.pivot else { continue };
        let on = bone(hierarchy, &part.bone)?;
        ensure!(on != weapon, "the weapon's own bone is not moved");
        let by = sub(definition.on_bone(pivot, unit), at_rest(&edits, on).0);
        match edits.moved.iter().position(|(moved, _)| *moved == on) {
            Some(already) => ensure!(edits.moved[already].1 == by, "two parts on {} turn about different points", part.bone),
            None => edits.moved.push((on, by)),
        }
    }
    if let Some(hands) = &definition.hands {
        let direct = |name: &str| -> Result<usize> {
            let node = bone(hierarchy, name)?;
            ensure!(hierarchy.nodes[node].parent == Some(weapon), "{name} does not hang from the weapon's bone");
            Ok(node)
        };
        let (marker, reference) = (direct(LEFT_MARKER)?, direct(LEFT_REFERENCE)?);
        let mut holds = Vec::new();
        let mut resting = [0.0; 3];
        for hold in &hands.holds {
            let on = bone(hierarchy, &hold.bone)?;
            let turn = at_rest(&edits, on).1;
            // Given on the weapon's bone at rest; kept in the bone's own
            // axes, so that it turns with the bone.
            let palms = hold.palms.iter().map(|palm| rotate(back(turn), of_centimetres(*palm))).collect();
            let moved_by = edits.moved.iter().find(|(moved, _)| *moved == on).map(|(_, by)| *by);
            let shift = hold.shift.map(of_centimetres).or(moved_by).ok_or_else(|| anyhow!("the hold on {} says no shift, and no part moves that bone", hold.bone))?;
            if hold.rest {
                resting = shift;
            }
            holds.push(Hold { bone: (on != weapon).then_some(on), palms, shift });
        }
        ensure!(!holds.is_empty(), "the hands have no holds");
        edits.hand = Some((marker, reference, holds, resting, hands.exponent));
    }
    // Every bone the edits move rests where they have it, and the
    // donor's own rest is kept for its animator.
    let mut edited = rest.clone();
    edits.apply(&mut edited);
    for (index, (before, after)) in rest.iter().zip(&edited).enumerate() {
        if before.0 != after.0 {
            edits.donor_rest.push((index, before.0));
            hierarchy.nodes[index].translation = after.0;
        }
    }
    let placed = |hierarchy: &Hierarchy, node: usize| -> Placed {
        if node == weapon {
            return ([0.0; 3], [0.0, 0.0, 0.0, 1.0]);
        }
        let pose: Vec<Placed> = hierarchy.nodes.iter().map(|node| (node.translation, node.rotation)).collect();
        edits.on_weapon(&pose, node)
    };
    let mut made = 0i64;
    let mut new_node = |hierarchy: &mut Hierarchy, name: &str, parent: usize, at: [f32; 3], turn: [f32; 4]| -> usize {
        made += 1;
        hierarchy.nodes.push(Node {
            name: name.to_owned(),
            parent: Some(parent),
            translation: at,
            rotation: turn,
            scale: [1.0; 3],
            active: true,
            renderer: None,
            scripts: Vec::new(),
            grip: None,
            placer: None,
            // No object of the install's.
            transform: ObjRef { file: usize::MAX, path_id: made },
        });
        hierarchy.nodes.len() - 1
    };

    // The bones a weapon's parts would bring.
    for added in &definition.bones {
        let parent = bone(hierarchy, &added.parent)?;
        let (parent_at, parent_turn) = placed(hierarchy, parent);
        let turn = match &added.turn_as {
            Some(other) => placed(hierarchy, bone(hierarchy, other)?).1,
            None => [0.0, 0.0, 0.0, 1.0],
        };
        let at = rotate(back(parent_turn), sub(definition.on_bone(added.at, unit), parent_at));
        new_node(hierarchy, &added.name, parent, at, quat_mul(back(parent_turn), turn));
    }

    // The model's parts, each a node on its bone that undoes the bone's
    // own place at rest: the mesh is in the axes of the weapon's bone.
    let mut build = Build { parts: Vec::new(), materials: Vec::new(), edits: Arc::new(edits.clone()) };
    for part in &definition.parts {
        let model = scene.models.iter().find(|model| model.name == part.model).ok_or_else(|| anyhow!("the model has no part {}", part.model))?;
        let mesh = mesh_of(definition, model, unit).ok_or_else(|| anyhow!("the part {} has no mesh", part.model))?;
        let parent = bone(hierarchy, &part.bone)?;
        let (parent_at, parent_turn) = placed(hierarchy, parent);
        let node = new_node(hierarchy, &part.model, parent, rotate(back(parent_turn), parent_at.map(|c| -c)), back(parent_turn));
        let materials = (0..mesh.submeshes.len())
            .map(|slot| {
                let name = model.materials.get(slot).cloned().unwrap_or_default();
                build.materials.iter().position(|material| material.name == name).unwrap_or_else(|| {
                    build.materials.push(material_of(definition, &name));
                    build.materials.len() - 1
                })
            })
            .collect();
        build.parts.push(Part { node, mesh: Arc::new(mesh), materials });
    }
    Ok(build)
}

/// The pictures a material of the model has, by the file's naming.
fn material_of(definition: &Definition, name: &str) -> Material {
    let folder = definition.root.join(&definition.model.textures);
    let file = |map: &str| {
        let path = folder.join(definition.model.texture.replace("{material}", name).replace("{map}", map));
        path.is_file().then_some(path)
    };
    Material {
        name: name.to_owned(),
        base_color: file("BaseColor"),
        normal: file("Normal"),
        roughness: file("Roughness"),
        metallic: file("Metallic"),
        occlusion: file("AO"),
        size: definition.model.texture_size.max(1),
        normal_green_down: definition.model.normal_green_down,
    }
}

/// A picture ready to be drawn with: its rows from the bottom up, as the
/// install's own are kept, and every smaller size after the first.
#[derive(Debug, Clone)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub mips: u32,
    /// Bytes to a pixel: four, or two for a normal picture's red and
    /// green.
    pub channels: usize,
    pub data: Vec<u8>,
    /// How it is compressed, if it is (`Pictures::compressed`): its data is
    /// then blocks, every level's after the one before.
    pub blocks: Option<Blocks>,
}

/// A picture's compression, as the graphics card reads it (`compress`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocks {
    /// Three channels, half a byte a pixel (four bytes a pixel before).
    Bc1,
    /// Two channels, a byte a pixel (two before).
    Bc5,
}

impl Picture {
    /// Compressed as the install's pictures are: four channels as BC1 (the
    /// fourth is not kept: a weapon's colour and surface have none), two
    /// as BC5; every level. One whose sides are not multiples of four is
    /// left as it is (the card takes compressed pictures in whole blocks).
    pub fn compressed(self) -> Picture {
        let (blocks, encode): (Blocks, fn(u32, u32, &[u8]) -> Vec<u8>) = match self.channels {
            4 => (Blocks::Bc1, crate::compress::bc1),
            2 => (Blocks::Bc5, crate::compress::bc5),
            _ => return self,
        };
        if self.blocks.is_some() || self.width % 4 != 0 || self.height % 4 != 0 {
            return self;
        }
        let mut data = Vec::new();
        let (mut width, mut height, mut at) = (self.width, self.height, 0);
        for _ in 0..self.mips {
            let size = width as usize * height as usize * self.channels;
            data.extend(encode(width, height, &self.data[at..at + size]));
            at += size;
            (width, height) = ((width / 2).max(1), (height / 2).max(1));
        }
        Picture { data, blocks: Some(blocks), ..self }
    }
}

/// A material's pictures: its colour; its normals; and one of its
/// occlusion (red), its roughness (green) and its metal (blue).
#[derive(Debug, Clone, Default)]
pub struct Pictures {
    pub colour: Option<Picture>,
    pub normal: Option<Picture>,
    pub surface: Option<Picture>,
}

impl Pictures {
    /// Each compressed (`Picture::compressed`): what is drawn with.
    pub fn compressed(self) -> Pictures {
        Pictures { colour: self.colour.map(Picture::compressed), normal: self.normal.map(Picture::compressed), surface: self.surface.map(Picture::compressed) }
    }
}

/// A PNG file as rows of red, green, blue and alpha, top row first.
fn read_png(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let size = reader.output_buffer_size().ok_or_else(|| anyhow!("the picture is too large"))?;
    let mut buffer = vec![0; size];
    let info = reader.next_frame(&mut buffer)?;
    buffer.truncate(info.buffer_size());
    let pixels = (info.width * info.height) as usize;
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => buffer,
        png::ColorType::Rgb => buffer.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::Grayscale => buffer.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => buffer.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        other => bail!("a picture of an unknown kind ({other:?})"),
    };
    ensure!(rgba.len() == pixels * 4, "the picture's size is not what it says");
    Ok((info.width, info.height, rgba))
}

/// A picture at half its size each way: every pixel the mean of four.
fn halved(width: u32, height: u32, channels: usize, data: &[u8]) -> (u32, u32, Vec<u8>) {
    let (w, h) = ((width / 2).max(1), (height / 2).max(1));
    let mut out = Vec::with_capacity(w as usize * h as usize * channels);
    let at = |x: u32, y: u32, c: usize| data[(y.min(height - 1) as usize * width as usize + x.min(width - 1) as usize) * channels + c] as u32;
    for y in 0..h {
        for x in 0..w {
            for c in 0..channels {
                let sum = at(2 * x, 2 * y, c) + at(2 * x + 1, 2 * y, c) + at(2 * x, 2 * y + 1, c) + at(2 * x + 1, 2 * y + 1, c);
                out.push(((sum + 2) / 4) as u8);
            }
        }
    }
    (w, h, out)
}

/// Makes a picture ready: no larger than `size` a side, rows from the
/// bottom up, every smaller size after it.
fn ready(mut width: u32, mut height: u32, channels: usize, mut data: Vec<u8>, size: u32) -> Picture {
    while width.max(height) > size {
        (width, height, data) = halved(width, height, channels, &data);
    }
    let row = width as usize * channels;
    let mut flipped = Vec::with_capacity(data.len());
    for line in data.chunks_exact(row).rev() {
        flipped.extend_from_slice(line);
    }
    let mut picture = Picture { width, height, mips: 1, channels, data: flipped.clone(), blocks: None };
    let (mut w, mut h, mut level) = (width, height, flipped);
    while w > 1 || h > 1 {
        (w, h, level) = halved(w, h, channels, &level);
        picture.data.extend_from_slice(&level);
        picture.mips += 1;
    }
    picture
}

/// Reads a material's pictures and makes them ready to draw with.
pub fn pictures(material: &Material) -> Result<Pictures> {
    let read = |path: &Option<PathBuf>| -> Result<Option<(u32, u32, Vec<u8>)>> {
        path.as_ref().map(|path| read_png(path).with_context(|| format!("reading {}", path.display()))).transpose()
    };
    let colour = read(&material.base_color)?.map(|(w, h, data)| ready(w, h, 4, data, material.size));
    let normal = read(&material.normal)?.map(|(w, h, data)| {
        // Red and green are enough: what draws it works the third out.
        let two: Vec<u8> = data.chunks_exact(4).flat_map(|p| [p[0], if material.normal_green_down { 255 - p[1] } else { p[1] }]).collect();
        ready(w, h, 2, two, material.size)
    });
    // One picture of the three that say what the surface is like, each
    // in the channel it is read from: occlusion in red, roughness in
    // green, metal in blue. One that is missing counts as not occluded,
    // fully rough and no metal.
    let (occlusion, roughness, metallic) = (read(&material.occlusion)?, read(&material.roughness)?, read(&material.metallic)?);
    let first = [&roughness, &metallic, &occlusion].into_iter().flatten().next().map(|(w, h, _)| (*w, *h));
    let surface = match first {
        None => None,
        Some((w, h)) => {
            for (which, _) in [&roughness, &metallic, &occlusion].into_iter().flatten().map(|(pw, ph, _)| ((*pw, *ph), ())) {
                ensure!(which == (w, h), "{}'s pictures of its surface are not of one size", material.name);
            }
            let channel = |picture: &Option<(u32, u32, Vec<u8>)>, pixel: usize, missing: u8| picture.as_ref().map_or(missing, |(_, _, data)| data[pixel * 4]);
            let data: Vec<u8> = (0..(w * h) as usize).flat_map(|pixel| [channel(&occlusion, pixel, 255), channel(&roughness, pixel, 255), channel(&metallic, pixel, 0), 255]).collect();
            Some(ready(w, h, 4, data, material.size))
        }
    };
    Ok(Pictures { colour, normal, surface })
}

/// Reads the pictures of several materials at once, each on a thread of
/// its own: the files are large and take their time to unpack. Compressed
/// as the install's are (`Pictures::compressed`), to be drawn with, and
/// kept so (`compressed_pictures`).
pub fn pictures_of(materials: &[Material]) -> Vec<Result<Pictures>> {
    std::thread::scope(|scope| {
        let running: Vec<_> = materials.iter().map(|material| scope.spawn(move || compressed_pictures(material))).collect();
        running.into_iter().map(|thread| thread.join().unwrap_or_else(|_| Err(anyhow!("reading a picture failed")))).collect()
    })
}

/// The folder compressed pictures are kept in between runs: a new one when
/// the encoders change what they make.
const PICTURE_CACHE: &str = "v1";

/// A material's pictures compressed, read from where they were kept the
/// last time if the files they come from are as they were then (their
/// paths, sizes and times, the material's size and the way its normals
/// are turned), else read, compressed and kept: `local\picture_cache` in
/// the folder the app runs in or any above it. Reading the PNG files and
/// compressing them takes the StG 44's about a second; what is kept, a
/// tenth of that.
fn compressed_pictures(material: &Material) -> Result<Pictures> {
    let kept = picture_cache_file(material);
    if let Some(pictures) = kept.as_ref().and_then(|file| read_kept(file).ok()) {
        return Ok(pictures);
    }
    let pictures = pictures(material)?.compressed();
    if let Some(file) = kept {
        // Not kept is only slower next time.
        let _ = file.parent().map(std::fs::create_dir_all).transpose().and_then(|_| write_kept(&file, &pictures));
    }
    Ok(pictures)
}

/// Where a material's compressed pictures are kept, by what they come
/// from; none where there is no `local` folder.
fn picture_cache_file(material: &Material) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let mut dir = std::env::current_dir().ok()?;
    let local = loop {
        if dir.join("local").is_dir() {
            break dir.join("local");
        }
        if !dir.pop() {
            return None;
        }
    };
    let mut hasher = std::hash::DefaultHasher::new();
    for path in [&material.base_color, &material.normal, &material.occlusion, &material.roughness, &material.metallic] {
        path.hash(&mut hasher);
        if let Some(meta) = path.as_ref().and_then(|path| std::fs::metadata(path).ok()) {
            meta.len().hash(&mut hasher);
            meta.modified().ok().hash(&mut hasher);
        }
    }
    (material.size, material.normal_green_down).hash(&mut hasher);
    let name: String = material.name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    Some(local.join("picture_cache").join(PICTURE_CACHE).join(format!("{name}_{:016x}.bin", hasher.finish())))
}

/// What a kept file begins with.
const KEPT_MARK: &[u8; 8] = b"TRPPIC1\0";

fn write_kept(file: &Path, pictures: &Pictures) -> std::io::Result<()> {
    let mut out = KEPT_MARK.to_vec();
    for picture in [&pictures.colour, &pictures.normal, &pictures.surface] {
        let Some(p) = picture else {
            out.push(0);
            continue;
        };
        out.push(1);
        for value in [p.width, p.height, p.mips, p.channels as u32] {
            out.extend(value.to_le_bytes());
        }
        out.push(match p.blocks {
            None => 0,
            Some(Blocks::Bc1) => 1,
            Some(Blocks::Bc5) => 2,
        });
        out.extend((p.data.len() as u64).to_le_bytes());
        out.extend_from_slice(&p.data);
    }
    std::fs::write(file, out)
}

fn read_kept(file: &Path) -> Result<Pictures> {
    let bytes = std::fs::read(file)?;
    ensure!(bytes.starts_with(KEPT_MARK), "not a kept picture");
    let mut at = KEPT_MARK.len();
    let mut take = |n: usize| -> Result<&[u8]> {
        let slice = bytes.get(at..at + n).ok_or_else(|| anyhow!("a kept picture ends early"))?;
        at += n;
        Ok(slice)
    };
    let mut read = || -> Result<Option<Picture>> {
        if take(1)?[0] == 0 {
            return Ok(None);
        }
        let mut word = || -> Result<u32> { Ok(u32::from_le_bytes(take(4)?.try_into()?)) };
        let (width, height, mips, channels) = (word()?, word()?, word()?, word()? as usize);
        let blocks = match take(1)?[0] {
            0 => None,
            1 => Some(Blocks::Bc1),
            2 => Some(Blocks::Bc5),
            other => bail!("a kept picture of an unknown kind ({other})"),
        };
        let length = u64::from_le_bytes(take(8)?.try_into()?) as usize;
        Ok(Some(Picture { width, height, mips, channels, data: take(length)?.to_vec(), blocks }))
    };
    Ok(Pictures { colour: read()?, normal: read()?, surface: read()? })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What is kept is read back as it was.
    #[test]
    fn compressed_pictures_are_kept_and_read_back() {
        let picture = |channels: usize, blocks| Some(Picture { width: 8, height: 4, mips: 4, channels, data: (0..40u8).collect(), blocks });
        let pictures = Pictures { colour: picture(4, Some(Blocks::Bc1)), normal: picture(2, Some(Blocks::Bc5)), surface: None };
        let file = std::env::temp_dir().join(format!("trp-kept-picture-{}.bin", std::process::id()));
        write_kept(&file, &pictures).unwrap();
        let back = read_kept(&file).unwrap();
        let _ = std::fs::remove_file(&file);
        for (a, b) in [(&pictures.colour, &back.colour), (&pictures.normal, &back.normal), (&pictures.surface, &back.surface)] {
            assert_eq!(a.as_ref().map(|p| (p.width, p.height, p.mips, p.channels, &p.data, p.blocks)), b.as_ref().map(|p| (p.width, p.height, p.mips, p.channels, &p.data, p.blocks)));
        }
    }

    #[test]
    fn a_picture_is_halved_to_its_limit_and_turned_over() {
        // Four pixels by two, the top row red, the bottom row blue.
        let data: Vec<u8> = [[255, 0, 0, 255]; 4].into_iter().chain([[0, 0, 255, 255]; 4]).flatten().collect();
        let picture = ready(4, 2, 4, data, 4);
        assert_eq!((picture.width, picture.height, picture.mips), (4, 2, 3));
        // The bottom row comes first.
        assert_eq!(&picture.data[..4], &[0, 0, 255, 255]);
        // 4 x 2, 2 x 1 and 1 x 1.
        assert_eq!(picture.data.len(), (8 + 2 + 1) * 4);
        let small = ready(4, 2, 4, picture.data[..32].to_vec(), 2);
        assert_eq!((small.width, small.height), (2, 1));
    }

    #[test]
    fn the_hand_follows_the_nearest_hold() {
        // A weapon's bone with a marker, a reference and a magazine below it.
        let parents = vec![None, Some(0), Some(0), Some(0)];
        let hold = |bone: Option<usize>, palm: [f32; 3], shift: [f32; 3]| Hold { bone, palms: vec![palm], shift };
        let edits = PoseEdits {
            parents,
            weapon: 0,
            moved: vec![(3, [0.0, -0.05, 0.0])],
            hand: Some((1, 2, vec![hold(None, [0.0, -0.4, 0.0], [0.0, -0.06, 0.01]), hold(Some(3), [0.0, 0.1, -0.2], [0.0, -0.05, 0.0])], [0.0, -0.06, 0.01], 3.0)),
            donor_rest: Vec::new(),
        };
        let still = [0.0, 0.0, 0.0, 1.0];
        // On the first hold the marker goes by it alone, the reference with it.
        let mut pose = vec![([0.0; 3], still), ([0.0, -0.4, 0.0], still), ([0.0, -0.4, 0.0], still), ([0.0, -0.3, 0.0], still)];
        edits.apply(&mut pose);
        assert!((pose[1].0[1] + 0.46).abs() < 1e-4 && (pose[1].0[2] - 0.01).abs() < 1e-4, "{:?}", pose[1].0);
        let near = |a: [f32; 3], b: [f32; 3]| sub(a, b).iter().all(|d| d.abs() < 1e-6);
        assert!(near(pose[2].0, [0.0, -0.46, 0.01]), "{:?}", pose[2].0);
        assert!(near(pose[3].0, [0.0, -0.35, 0.0]), "{:?}", pose[3].0);
        // Holding the magazine, wherever it has been carried, the hand
        // keeps its place on it.
        let mut pose = vec![([0.0; 3], still), ([0.3, 0.2, -0.6], still), ([0.0, -0.4, 0.0], still), ([0.3, 0.1, -0.4], still)];
        edits.apply(&mut pose);
        let between = sub(pose[1].0, pose[3].0);
        assert!((between[1] - 0.1).abs() < 2e-3 && (between[2] + 0.2).abs() < 2e-3, "{between:?}");
    }
}
