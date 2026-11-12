//! A custom weapon's definition: model, parts, bones, slots, hands and holds.

use super::*;

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
    /// Slots of its own that take the install's parts, each with a bone
    /// of its own on the model, beside its magazine's: of the donor's
    /// slots only the magazine's is kept, its model being one piece.
    #[serde(default)]
    pub slots: Vec<SlotDef>,
    /// Where the left hand holds this weapon, set beside where it holds
    /// the donor.
    #[serde(default)]
    pub hands: Option<HandsDef>,
    /// The install's weapon whose muzzle effects it has (its flash and its
    /// smoke, read with that weapon's own `MuzzleManager`), laid at this
    /// weapon's `fireport` as they lie from that weapon's (`take_muzzle`).
    #[serde(default)]
    pub muzzle_from: Option<String>,
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

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Model {
    /// A binary FBX file whose barrel runs along -z with y up, as it is
    /// read (`frame`, `turn`).
    pub file: String,
    /// The folder of its pictures, and how one is named: `{material}`
    /// and `{map}` stand for the material's name in the model and for
    /// one of `BaseColor`, `Normal`, `Roughness`, `Metallic`, `AO`.
    pub textures: String,
    pub texture: String,
    /// Where the model's own origin lies on the donor's weapon bone, in
    /// centimetres: ahead along the barrel, up, and to the right. A
    /// weapon's; a file of parts places each part by its slot instead
    /// (`PartDefinition::at_slot`).
    #[serde(default)]
    pub origin: [f32; 3],
    /// The node of the file whose axes and units the model is read in
    /// (`fbx::Scene::seen_from`): a file exported from a viewer carries the
    /// viewer's turn and scale above the model as it was made. None for
    /// the file's own.
    #[serde(default)]
    pub frame: Option<String>,
    /// Degrees about x, then y, then z the model is turned by after that
    /// (`fbx::Scene::turned`), to have its barrel along -z with y up.
    #[serde(default)]
    pub turn: Option<[f64; 3]>,
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

/// A slot of a weapon or a part of one's own: its template takes the
/// parts named, and a part in it hangs on its bone as the game hangs one
/// on any slot's (`crate::weapon::attach_at`), with the part's own slots
/// below it.
#[derive(Debug, Clone, Deserialize)]
pub struct SlotDef {
    /// The slot's name, which is its bone's (`mod_mount`).
    pub name: String,
    /// The templates of the parts it takes, the install's or one's own.
    pub takes: Vec<String>,
    /// Where its bone is on the model, in the model's units and axes. It
    /// hangs from the weapon's bone unturned, as the donor's own slots do,
    /// so that a part stands as it stands on the install's weapons.
    pub at: [f32; 3],
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

/// A file of parts of one's own (the `custom/parts` folder): the model
/// they are drawn from, and the parts, each a piece of it.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct PartsFile {
    pub(super) model: Model,
    pub(super) items: Vec<PartDefinition>,
}
