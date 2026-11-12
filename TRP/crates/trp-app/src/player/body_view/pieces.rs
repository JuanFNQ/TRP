//! The body's parts, suits, outfit pieces, heads and worn pieces.

use super::*;

/// The ribcage, by the end of its path on the skeleton, and the two bones
/// the arms start at.
pub(super) const RIBCAGE: &str = "/Base HumanRibcage";

pub(super) const COLLARBONES: [&str; 2] = ["/Base HumanRibcage/Base HumanLCollarbone", "/Base HumanRibcage/Base HumanRCollarbone"];

/// Whether a path is a collarbone's own.
pub(super) fn a_collarbone(path: &str) -> bool {
    COLLARBONES.iter().any(|collarbone| path.ends_with(collarbone))
}

/// The render layer the camera and the sun see. What only casts a shadow
/// in first person stays on it, drawn by a material that keeps no pixel
/// (`shadow_only`): Bevy 0.17 on lets a camera's shadow maps take only what
/// is on that camera's layers.
pub const SEEN: usize = 0;

/// A layer nothing draws and no light shines on: where the body is put
/// for the character's own eyes with the options' `FIRST-PERSON BODY`
/// off.
pub const NOWHERE: usize = 2;

/// A mesh of the upper body: drawn from outside, a shadow and nothing
/// more in first person.
#[derive(Component)]
#[require(crate::rendering::shadow_only::ShadowOnly)]
pub struct UpperBody;

/// A mesh of the rest of the body: drawn in every view.
#[derive(Component)]
pub struct LowerBody;

/// Which of the two suits a piece of the body is (`UpperBodySuit`, its
/// body; `LowerBodySuit`, its legs), which a suit chosen on the OVERALL tab
/// takes the place of (`clothing`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SuitRole {
    Upper,
    Lower,
}

#[derive(Component, Clone, Copy)]
pub struct SuitPiece(pub SuitRole);

/// A piece of a torso of the newer kind (`TorsoSkin`): drawn while its mesh
/// is the one the game draws for what is worn (`show_torso`).
#[derive(Component, Clone, Copy)]
pub struct TorsoPiece(pub trp_data::character::arms::Torso);

/// The bones the install's suits hang on beyond the default suits' (every
/// one of the 165 parts read, inventory spec, "The OVERALL tab"), by their
/// paths as the parts name them: in the skeleton from the start where the
/// options' `CLOTHING` is on, for a suit chosen later.
pub const SUIT_BONES: [&str; 7] = [
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanGear1",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanGear2",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanGear3",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanGear4/Base HumanGear4_1",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanGear5",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3/Base HumanBackpack",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3/Base HumanRibcage/Weapon_root",
];

/// The clothing the body is drawn in, read before the simulation is made.
#[derive(Resource, Default)]
pub struct BodyParts {
    pub parts: Vec<trp_data::character::arms::Arms>,
    /// For each part, whether it is one the game leaves a shadow in
    /// first person: the upper body's and the head's.
    pub upper: Vec<bool>,
    /// The bones the body's animator is asked to pose for them, each by
    /// its path below the animated object, a parent always before what
    /// hangs from it.
    pub bones: Vec<String>,
    /// Worn items that are models of their own, for the head bone, each
    /// with the item it is, the parts it may have fitted and its hinge.
    pub on_head: Vec<HeadModel>,
    /// For each part, the item it is and what it is the mesh over, where
    /// it is something worn: such a part is drawn only while the
    /// character has that item on over just that.
    pub item: Vec<Option<WornPiece>>,
    /// Which of the items that may be worn are armour and which vests:
    /// what is worn over them takes its mesh by them. And which armour is
    /// worn like a vest (`VestlikeArmor`), under which a torso of the newer
    /// kind takes its mesh for a vest.
    pub(super) armour: Vec<String>,
    pub(super) vests: Vec<String>,
    pub vestlike: Vec<String>,
    /// For each part, the suit it is of, where it is the upper suit's body
    /// or the lower suit's legs.
    pub suit: Vec<Option<SuitRole>>,
    /// The outfits that are not the install's, each with the item it is:
    /// its meshes, and the rig they are drawn on, which has the bones the
    /// parts above hang on at the outfit's own places.
    pub outfits: Vec<(String, trp_data::character::outfit::Outfit)>,
    /// For each part, the outfit it is the top of, where it is one of the
    /// install's own tops worn as an outfit; and whether it is the
    /// character's own top, which such an outfit is drawn in place of.
    pub of_outfit: Vec<Option<String>>,
    pub top: Vec<bool>,
    /// The install's own hands worn as an outfit's arms for the
    /// character's own eyes, each with its item.
    pub their_hands: Vec<(String, trp_data::character::arms::Arms)>,
    /// The bones of the character model on the gear page (`figure_view`),
    /// each by its path, a parent always before what hangs from it: every
    /// bone a mesh of the body hangs on, the arms' among them (there the
    /// arms are the animation's, not the weapon's), and every bone above
    /// one; and the pose the game's screens stand them in, once read.
    pub standing: Vec<String>,
    pub standing_pose: Option<trp_sim::locomotion::body::Pose>,
    /// The game's menu figure for the OVERALL tab's own model, with
    /// `OVERALL FIGURE` on (`figure_view::Overall`): its body on the menu
    /// player's skeleton, those bones among it (`trp_sim::figure`).
    pub overall: Option<trp_sim::characters::figure::FigureData>,
    /// The same pose for the dummy on the range, with its switch on
    /// (`dummy`).
    pub dummy_pose: Option<trp_sim::locomotion::body::Pose>,
    /// Which of `parts` is the head's, if one is.
    pub head: Option<usize>,
    /// For each part, the dummy whose own it is and which part of its look
    /// (`LOOK_UPPER`, `LOOK_LOWER`, `LOOK_HEAD`), where it is one of a
    /// dummy's own (`dummy`, so that a dummy does not
    /// copy the player's clothing): drawn on that dummy
    /// alone. And for each dummy, which of the three it has of its own, in
    /// place of the character's.
    pub dummy_own: Vec<Option<(usize, usize)>>,
    pub dummy_has: Vec<[bool; 3]>,
    /// The options' `HEAD FITTING` as the run began (`fit_head_things`):
    /// off, no fitting or changer is read. The skinned worn things' parts
    /// that have a second form, by part (`trp_data::dress::skin_fitting`).
    /// And the bits each part of a look adds (`trp_data::dress::
    /// part_clipping`): the character's as it began (upper body, legs,
    /// head), and each dummy's own (where `dummy_has` says it has one).
    pub fitting: bool,
    pub fittings: Vec<(usize, trp_data::character::dress::Fitting)>,
    pub suit_bits: [u8; 3],
    pub dummy_bits: Vec<[u8; 3]>,
    /// The hooded form of each part that has a hood (`trp_data::dress::
    /// hood`: the cultist top's), by part; read with `HEAD FITTING` on, and
    /// drawn by `fit_head_things` as a second form (`fit::HOOD_UP`).
    pub hoods: Vec<(usize, trp_data::character::arms::Arms)>,
    /// The options' `ARMBAND` as the run began (`fit_armbands`): off, no
    /// armband is read. The armbands' skins among `parts`, each with its
    /// mesh for every upper body by its customization id
    /// (`trp_data::dress::ArmBand::sets`), by part.
    pub armband: bool,
    pub bands: Vec<(usize, Vec<(String, trp_data::unity::ObjRef)>)>,
    /// The bundles of the character's look as it began (upper body, legs,
    /// head) and of each dummy's own (empty where it has none), and every
    /// upper body's customization ids by its bundle (`trp_data::suits::
    /// Customizations::body_ids`, given by the run once read): whose body
    /// an armband's meshes go by (`character_body_id`, `dummy_body_id`).
    pub look_keys: [String; 3],
    pub dummy_keys: Vec<[String; 3]>,
    pub body_ids: HashMap<String, Vec<String>>,
    /// With `DISTANCE DETAIL` on (`lod_view::on`), each part's group of
    /// levels of detail and the levels it is drawn at, where it is one of a
    /// part's levels (`trp_data::arms::load_levels`, `dress::load_levels`):
    /// every level is a part of its own, after its fullest, with the same
    /// item, suit and wearer. And for each part, its fullest's place (its
    /// own for a fullest), by which a coarser level is the head, a look's
    /// part, the legs.
    pub lods: Vec<Option<(std::sync::Arc<crate::assets::lod_view::LodSpec>, u8)>>,
    pub fullest: Vec<usize>,
}

/// A part's levels as `BodyParts` keeps them: its group placed under the
/// player's mesh transform (`LodSpec::through`), one spec a group.
pub(super) struct LevelSpecs {
    pub(super) mesh: Option<([f32; 3], [f32; 4])>,
    pub(super) made: Vec<(usize, std::sync::Arc<crate::assets::lod_view::LodSpec>)>,
}

impl LevelSpecs {
    pub(super) fn of(&mut self, part: &trp_data::character::arms::Arms) -> Option<(std::sync::Arc<crate::assets::lod_view::LodSpec>, u8)> {
        let lod = part.lod.as_ref()?;
        let (place, turn) = self.mesh?;
        let key = std::sync::Arc::as_ptr(&lod.group) as usize;
        let spec = match self.made.iter().find(|(made, _)| *made == key) {
            Some((_, spec)) => spec.clone(),
            None => {
                let spec = crate::assets::lod_view::LodSpec::through(&lod.group, place, turn);
                self.made.push((key, spec.clone()));
                spec
            }
        };
        Some((spec, lod.levels))
    }
}

/// The parts of a look, as the run's keys and a dummy's list them: the
/// upper body, the legs, the head.
pub const LOOK_UPPER: usize = 0;

pub const LOOK_LOWER: usize = 1;

pub const LOOK_HEAD: usize = 2;

/// A piece of an outfit that is not the install's (`trp_data::outfit`;
/// `docs/spec/outfits.md`): a body of another game's, drawn while the
/// character has that item on, in place of its own.
#[derive(Component, Clone)]
pub struct OutfitPiece {
    pub item: String,
}

/// A piece of such an outfit's arms as the character's own eyes see them
/// (the other game's model for first person): drawn in first person
/// while the item is on, and in no other view.
#[derive(Component, Clone)]
pub struct OutfitArms {
    pub item: String,
}

/// A piece of the character's own body: its clothing and its head, which
/// are not drawn while an outfit with a body of its own is on. `top`: it
/// is the character's own top, which is not drawn either while an outfit
/// that is one of the install's own tops is on.
#[derive(Component)]
pub struct OwnBody {
    pub top: bool,
}

/// The outfit that is on, if one is, by its item, and what of the
/// character's own it is drawn in place of. **Whatever an outfit has not
/// got to draw is the character's own**: the install's default body, and
/// the install's default arms for its own eyes.
#[derive(Resource, Default, PartialEq)]
pub struct OutfitOn {
    pub item: Option<String>,
    /// It has a body of its own, drawn in place of the whole of the
    /// character's (clothing and head).
    pub body: bool,
    /// It is one of the install's own tops, drawn in place of the
    /// character's own top and of nothing else.
    pub top: bool,
    /// It has arms for the character's own eyes, drawn in first person in
    /// place of the character's own.
    pub arms: bool,
}

/// The skeleton one outfit is drawn on (`trp_data::outfit::Rig`): for each
/// of its bones the install's bone it follows, with that bone's own matrix
/// for a point of a mesh and whether the game squeezes it with the weapon
/// in first person (an arm's, a collarbone's), and the entity the
/// outfit's meshes hang on, which is given the move of what is on that
/// bone from rest.
pub struct OutfitSkeleton {
    pub(super) item: String,
    pub(super) rig: trp_data::character::outfit::Rig,
    pub(super) theirs: Vec<Entity>,
    pub(super) their_rest: Vec<bevy::math::Affine3A>,
    pub(super) squeezed: Vec<bool>,
    pub(super) ours: Vec<Entity>,
    /// What the squeezed bones hang from: under the weapon rig's root,
    /// and kept at that root's own place and turn undone, so that only
    /// the root's squeeze is left on them.
    pub(super) unsqueeze: Entity,
}

#[derive(Resource, Default)]
pub struct OutfitSkeletons(pub(super) Vec<OutfitSkeleton>);

/// Where the bones of the outfit that is on go this frame.
#[derive(Resource, Default)]
pub struct OutfitPose(pub(super) Vec<(Entity, Transform)>);

/// A worn thing's model for the head bone (`BodyParts::on_head`): the item
/// it is, its objects, the parts it may have fitted (each hidden until
/// fitted) and its hinges (a helmet's, or a face shield's own).
pub struct HeadModel {
    pub item: String,
    pub model: Hierarchy,
    pub parts: Vec<trp_data::character::dress::WornPart>,
    pub hinges: Vec<trp_data::character::dress::Hinge>,
    /// Its objects' meshes that have a second form (`trp_data::dress::
    /// fittings`), by object; none with `HEAD FITTING` off.
    pub fittings: Vec<(usize, trp_data::character::dress::Fitting)>,
}

/// A part a worn thing's model may have fitted (body animator spec, "A
/// worn thing's parts and a helmet's hinge"): the worn thing's template
/// and the slots and templates from it down to the part. Drawn while the
/// thing worn has it fitted so (`show_head_parts`).
#[derive(Component, Clone)]
pub struct HeadPart {
    pub(crate) item: String,
    pub(crate) path: Vec<(String, String)>,
}

/// A hinge on a copy of a model worn on the head (`HelmetDress`): the worn
/// thing's template and the way down from it to the thing whose model
/// carries the hinge (none: the worn thing; a face shield's own, its slot),
/// the hinge's two turns in the engine's axes, its rule (`trp_sim::visor::
/// ShieldHinge`: that thing as last seen, its face shield, the way its
/// filter was last set, the filter); whether this copy is the gear page's
/// figure's.
#[derive(Component, Clone)]
pub struct HelmetHinge {
    pub(super) item: String,
    pub(super) owner: Vec<(String, String)>,
    pub(super) off: [f32; 4],
    pub(super) on: [f32; 4],
    pub(super) hinge: trp_sim::effects::visor::ShieldHinge,
    pub(super) figure: bool,
}

/// A piece of something worn: the item's template and what it is the mesh
/// over (any, if the list is empty). Drawn while the character has the
/// item on over that (`show_worn`).
#[derive(Component, Clone)]
pub struct WornPiece {
    pub item: String,
    pub over: Vec<Under>,
}

/// The item classes whose wearing changes the mesh of what is worn over
/// them: armour, and vests (their template ids in the item database).
/// Every case of what a thing may be worn over (`Under`), in the order a
/// thing's meshes are read for them.
pub(crate) const ALL_UNDER: [Under; 4] = [
    Under { armour: false, vest: false },
    Under { armour: false, vest: true },
    Under { armour: true, vest: false },
    Under { armour: true, vest: true },
];

pub(crate) const ARMOUR: &str = "5448e54d4bdc2dcc718b4568";

pub(super) const VEST: &str = "5448e5284bdc2dcb718b4567";

pub(super) const HEAD: &str = "Base HumanHead";

/// Whether a bone is one of an arm's below the collarbone: those are the
/// weapon rig's.
pub(super) fn of_an_arm(path: &str) -> bool {
    COLLARBONES.iter().any(|collarbone| path.contains(collarbone)) && !a_collarbone(path)
}

impl BodyParts {
    /// Says in the log, of a thing worn as a skin, whether the skeleton has
    /// every bone its skin names (by their paths; `bones`, asked of the
    /// player's own skeleton as the run began), and which it lacks. The
    /// game's skin finds each of its bones by its path on the player's
    /// skeleton (`Skin.Init`); here one the skeleton lacks is drawn on the
    /// nearest bone above it that the body has, its root at the last
    /// (`clothing::joint_of`), which bends the mesh wrongly: a line to
    /// look for when a worn thing sits badly. An arm's bones below the
    /// collarbone are the weapon rig's and are not counted.
    pub(crate) fn note_bones<'a>(&self, thing: &str, bone_paths: impl IntoIterator<Item = &'a String>) {
        let mut named: Vec<&str> = bone_paths.into_iter().map(String::as_str).filter(|path| !of_an_arm(path)).collect();
        named.sort_unstable();
        named.dedup();
        let lacking: Vec<&str> = named.iter().copied().filter(|path| !self.bones.iter().any(|known| known == path)).collect();
        if lacking.is_empty() {
            info!("worn as a skin, {thing}: the skeleton has every one of its {} bones", named.len());
        } else {
            warn!("worn as a skin, {thing}: the skeleton lacks {} of its {} bones, each drawn on the nearest above it: {}", lacking.len(), named.len(), lacking.join(", "));
        }
    }
}
