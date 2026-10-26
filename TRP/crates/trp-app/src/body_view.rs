//! The body as drawn: its clothing skinned to a skeleton that the
//! simulation's body animator poses.
//!
//! A piece of clothing is a skinned mesh that names its bones by their
//! path on the player's skeleton (`trp_data::arms::load` reads one; the
//! arms are the same kind of thing). The bones those meshes hang on are
//! asked of the body's animator when the simulation is made
//! (`BodyData::load_with`), and each drawn frame they are put where the
//! animation has them, between the simulation's last two frames like
//! everything else drawn.
//!
//! In first person the game does three things to the body, done here by
//! which entity a mesh's bone is tied to. The arms are solved onto the
//! weapon: a bone of an arm below the collarbone is the weapon rig's own,
//! which the arms' solver places. The ribcage is turned to the view and
//! squeezed along it with the weapon (`FirstPersonStrategy`): the ribcage
//! is the weapon rig's root, which is exactly that. And when the game
//! turns the ribcage it keeps the two collarbones where the body's
//! animation had them (they are on the player prefab's list of bones to
//! keep, `PlayerBones.FovSpecialTransforms`), though the squeeze, which
//! comes after, still draws them in: each collarbone is an entity of its
//! own under the rig's root, put each frame where the animated one is.
//! Whatever else hangs from the ribcage would turn with it, and so hangs
//! from the rig's root; nothing of the default clothing does (the neck
//! and head hang from the last spine bone, beside the ribcage, and are
//! the body's animation's alone).
//!
//! And the game does not draw all of it to its own player
//! (`PlayerBody.UpdatePlayerRenders`): in first person the legs are drawn
//! and the hands are, and every other part (the upper body, the head) is
//! left to cast its shadow and nothing more. The upper body's meshes are
//! on a render layer of their own that the light sees and, in first
//! person, the camera does not.
//!
//! Seen by anyone else (`docs/spec/third-person.md`) the game does none of
//! the first two: the ribcage is where the body's animation has it, the
//! collarbones on it, and the arms are solved onto the weapon from the
//! body's own shoulders. So the ribcage drawn is an entity of its own
//! under the rig's root, like the kept collarbones: left on that root for
//! the character's own eyes, and put where the animated ribcage is for
//! another's.
//!
//! Not in this: the feet set on the ground (the game runs a solver for
//! them, which moves nothing on level ground) and equipment.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use trp_data::dress::{Under, Worn};
use trp_data::unity::anim::path_hash;
use trp_data::unity::extract::Hierarchy;
use trp_data::unity::Assets as UnityAssets;

use crate::controller::Controller;
use crate::inventory_view::Carried;
use crate::park::{EvenParked, Park};
use crate::render_data::Converted;
use crate::weapon::{FirstPersonHands, WeaponRig};
use crate::{convert, GameData, View};

/// The ribcage, by the end of its path on the skeleton, and the two bones
/// the arms start at.
const RIBCAGE: &str = "/Base HumanRibcage";
const COLLARBONES: [&str; 2] = ["/Base HumanRibcage/Base HumanLCollarbone", "/Base HumanRibcage/Base HumanRCollarbone"];

/// Whether a path is a collarbone's own.
fn a_collarbone(path: &str) -> bool {
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
#[require(crate::shadow_only::ShadowOnly)]
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
pub struct TorsoPiece(pub trp_data::arms::Torso);

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
    pub parts: Vec<trp_data::arms::Arms>,
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
    armour: Vec<String>,
    vests: Vec<String>,
    pub vestlike: Vec<String>,
    /// For each part, the suit it is of, where it is the upper suit's body
    /// or the lower suit's legs.
    pub suit: Vec<Option<SuitRole>>,
    /// The outfits that are not the install's, each with the item it is:
    /// its meshes, and the rig they are drawn on, which has the bones the
    /// parts above hang on at the outfit's own places.
    pub outfits: Vec<(String, trp_data::outfit::Outfit)>,
    /// For each part, the outfit it is the top of, where it is one of the
    /// install's own tops worn as an outfit; and whether it is the
    /// character's own top, which such an outfit is drawn in place of.
    pub of_outfit: Vec<Option<String>>,
    pub top: Vec<bool>,
    /// The install's own hands worn as an outfit's arms for the
    /// character's own eyes, each with its item.
    pub their_hands: Vec<(String, trp_data::arms::Arms)>,
    /// The bones of the character model on the gear page (`figure_view`),
    /// each by its path, a parent always before what hangs from it: every
    /// bone a mesh of the body hangs on, the arms' among them (there the
    /// arms are the animation's, not the weapon's), and every bone above
    /// one; and the pose the game's screens stand them in, once read.
    pub standing: Vec<String>,
    pub standing_pose: Option<trp_sim::body::Pose>,
    /// The same pose for the dummy on the range, with its switch on
    /// (`dummy`).
    pub dummy_pose: Option<trp_sim::body::Pose>,
    /// Which of `parts` is the head's, if one is.
    pub head: Option<usize>,
}

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
    item: String,
    rig: trp_data::outfit::Rig,
    theirs: Vec<Entity>,
    their_rest: Vec<bevy::math::Affine3A>,
    squeezed: Vec<bool>,
    ours: Vec<Entity>,
    /// What the squeezed bones hang from: under the weapon rig's root,
    /// and kept at that root's own place and turn undone, so that only
    /// the root's squeeze is left on them.
    unsqueeze: Entity,
}

#[derive(Resource, Default)]
pub struct OutfitSkeletons(Vec<OutfitSkeleton>);

/// Where the bones of the outfit that is on go this frame.
#[derive(Resource, Default)]
pub struct OutfitPose(Vec<(Entity, Transform)>);

/// A worn thing's model for the head bone (`BodyParts::on_head`): the item
/// it is, its objects, the parts it may have fitted (each hidden until
/// fitted) and its hinges (a helmet's, or a face shield's own).
pub struct HeadModel {
    pub item: String,
    pub model: Hierarchy,
    pub parts: Vec<trp_data::dress::WornPart>,
    pub hinges: Vec<trp_data::dress::Hinge>,
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
/// the hinge's two turns in the engine's axes, its filter, and that thing
/// as last seen, its face shield and the way the filter was last set, as
/// they stood; whether this copy is the gear page's figure's.
#[derive(Component, Clone)]
pub struct HelmetHinge {
    item: String,
    owner: Vec<(String, String)>,
    off: [f32; 4],
    on: [f32; 4],
    filter: trp_sim::hysteresis::Hysteresis,
    worn: Option<trp_sim::inventory::Id>,
    shield: Option<trp_sim::inventory::Id>,
    down: bool,
    figure: bool,
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
const ARMOUR: &str = "5448e54d4bdc2dcc718b4568";
const VEST: &str = "5448e5284bdc2dcb718b4567";
const HEAD: &str = "Base HumanHead";

/// Whether a bone is one of an arm's below the collarbone: those are the
/// weapon rig's.
fn of_an_arm(path: &str) -> bool {
    COLLARBONES.iter().any(|collarbone| path.contains(collarbone)) && !a_collarbone(path)
}

impl BodyParts {
    /// Reads the parts in the bundles named: the upper body's, the
    /// legs', then the head's. One that cannot be read is left out with a
    /// note. All but the legs are the upper body's for drawing: seen from
    /// outside, a shadow in first person.
    ///
    /// `worn` is what the character has on over that, each an item's
    /// template id or a bundle: armour, vests and backpacks are skinned to
    /// the skeleton with the clothing, and what goes on the head is kept
    /// as a model for the head bone. All of it is the upper body's for
    /// drawing (body animator spec, "Worn items").
    ///
    /// `custom_hands` is the options' switch `CUSTOM FIRST PERSON HANDS` as
    /// the run begins: off, no outfit's arms are read or fitted at all.
    ///
    /// `present` is every thing the run has, by its template: of the parts
    /// a model on the head may have fitted, those are read.
    ///
    /// `suits` is the options' `CLOTHING` as the run begins: on, the bones
    /// the install's other suits hang on are in the skeleton too
    /// (`SUIT_BONES`).
    #[allow(clippy::too_many_arguments)]
    pub fn load(unity: &mut UnityAssets, db: &trp_data::Database, keys: &[&str], worn: &[String], present: &HashSet<String>, custom_hands: bool, suits: bool) -> Self {
        let (mut parts, mut upper, mut items) = (Vec::new(), Vec::new(), Vec::new());
        let (mut of_outfit, mut top, mut suit) = (Vec::new(), Vec::new(), Vec::new());
        let (mut legs, mut head) = (None, None);
        for (index, key) in keys.iter().enumerate().filter(|(_, key)| !key.is_empty() && **key != "none") {
            match trp_data::arms::load(unity, key) {
                Ok(part) => {
                    if index == 1 {
                        legs = Some(parts.len());
                    }
                    if index == 2 {
                        head = Some(parts.len());
                    }
                    parts.push(part);
                    upper.push(index != 1);
                    items.push(None);
                    of_outfit.push(None);
                    top.push(index == 0);
                    suit.push(match index {
                        0 => Some(SuitRole::Upper),
                        1 => Some(SuitRole::Lower),
                        _ => None,
                    });
                }
                Err(e) => eprintln!("the body's part {key} could not be read: {e:#}"),
            }
        }
        // A vest sits out over armour and a backpack over either, by a
        // mesh of its own for each case. What is worn can change while
        // running, so each item is read for every case that may come up
        // with the others listed, once for each mesh.
        let of_class = |parent: &str| -> Vec<String> {
            let class = |item: &String| db.items.get(item).map(|template| template.parent.as_str());
            worn.iter().filter(|item| class(item) == Some(parent)).cloned().collect()
        };
        let (armour, vests) = (of_class(ARMOUR), of_class(VEST));
        let vestlike: Vec<String> = armour
            .iter()
            .filter(|item| db.items.get(*item).and_then(|template| template.prefab_key()).is_some_and(|key| trp_data::dress::vestlike(unity, key).unwrap_or(false)))
            .cloned()
            .collect();
        let either = |any: bool| if any { vec![false, true] } else { vec![false] };
        let cases: Vec<Under> =
            either(!armour.is_empty()).into_iter().flat_map(|armour| either(!vests.is_empty()).into_iter().map(move |vest| Under { armour, vest })).collect();
        // An outfit that is not the install's is fitted to the skeleton
        // as the character's own clothing has it at rest: every bone one
        // of those meshes hangs on, with that mesh's own matrix for it.
        // The box about the legs' own mesh at rest is given with it: its
        // top is the waist, under which the character's own eyes see the
        // body (the install draws the legs' mesh for them and no more).
        // Whatever of an outfit cannot be drawn is left to the
        // character's own body and arms.
        let mut outfits = Vec::new();
        let mut their_hands = Vec::new();
        if !db.outfits.is_empty() {
            let mut skeleton: Vec<trp_data::outfit::RestBone> = Vec::new();
            let mut waist = None;
            for (index, part) in parts.iter().enumerate() {
                match trp_data::unity::extract::read_mesh(unity, part.mesh) {
                    Ok(mesh) => {
                        for (path, bind) in part.bone_paths.iter().zip(&mesh.bind_poses).filter(|(path, _)| !skeleton.iter().any(|known| &known.path == *path)).collect::<Vec<_>>() {
                            skeleton.push(trp_data::outfit::RestBone { path: path.clone(), bind: *bind });
                        }
                        if legs == Some(index) {
                            waist = trp_data::outfit::box_about(&mesh.positions);
                        }
                    }
                    Err(e) => eprintln!("a mesh of the body could not be read for the outfits: {e:#}"),
                }
            }
            let stalker = trp_data::xray::Stalker::locate();
            let mut ids: Vec<&String> = db.outfits.keys().collect();
            ids.sort();
            for id in ids {
                let mut definition = (*db.outfits[id]).clone();
                // (Both kinds of hands: the other game's arms, the install's
                // own bare ones. `|`, that both are taken.)
                if !custom_hands && (definition.arms.take().is_some() | definition.their_hands.take().is_some()) {
                    eprintln!("the outfit {}: its arms for first person are not read (CUSTOM FIRST PERSON HANDS is off)", definition.name);
                }
                let definition = &definition;
                let outfit = trp_data::outfit::build(definition, &stalker, &skeleton, waist);
                for note in &outfit.notes {
                    eprintln!("the outfit {}: {note}", definition.name);
                }
                let mut has = Vec::new();
                if outfit.has_body() {
                    has.push(format!("a body of {} meshes on a rig of {} of the body's bones", outfit.pieces.len(), outfit.rig.bones.len()));
                }
                if let Some(arms) = outfit.arms.as_ref().filter(|_| outfit.has_arms()) {
                    has.push(format!("arms for first person, {} meshes on {} bones, at {:.2} times their file's size", arms.pieces.len(), arms.rig.bones.len(), arms.size));
                }
                // The install's own clothing, where the outfit is made of
                // that: skinned to the skeleton as the character's own.
                if let Some(key) = definition.their_top.as_ref().filter(|_| !outfit.has_body()) {
                    match trp_data::arms::load(unity, key) {
                        Ok(part) => {
                            parts.push(part);
                            upper.push(true);
                            items.push(None);
                            of_outfit.push(Some(id.clone()));
                            top.push(false);
                            suit.push(None);
                            has.push("one of the install's own tops".to_owned());
                        }
                        Err(e) => eprintln!("the outfit {}: its top {key} could not be read, and the character's own is drawn: {e:#}", definition.name),
                    }
                }
                if let Some(key) = definition.their_hands.as_ref().filter(|_| !outfit.has_arms()) {
                    match trp_data::arms::load(unity, key) {
                        Ok(hands) => {
                            their_hands.push((id.clone(), hands));
                            has.push("the install's own hands for first person".to_owned());
                        }
                        Err(e) => eprintln!("the outfit {}: its hands {key} could not be read, and the character's own are drawn: {e:#}", definition.name),
                    }
                }
                if has.is_empty() {
                    eprintln!("the outfit {} has nothing to draw: worn, the character is as without it", definition.name);
                } else {
                    println!("the outfit {}: {}", definition.name, has.join("; "));
                }
                outfits.push((id.clone(), outfit));
            }
        }
        let mut on_head = Vec::new();
        // (An outfit of one's own has its donor's prefab for a name only:
        // none of the install's meshes is drawn for it.)
        let listed_once = worn.iter().enumerate().filter(|(index, item)| !worn[..*index].contains(item) && !db.outfits.contains_key(*item)).map(|(_, item)| item);
        for item in listed_once {
            let bundle = db.items.get(item).and_then(|template| template.prefab_key()).unwrap_or(item.as_str()).to_owned();
            let first = parts.len();
            for under in &cases {
                match trp_data::dress::load(unity, &bundle, *under) {
                    Ok(Worn::Skinned(part)) => {
                        // The same mesh as for a case before: that piece's too.
                        let same = (first..parts.len()).find(|at| parts[*at].mesh == part.mesh);
                        match same.and_then(|at| items[at].as_mut()) {
                            Some(WornPiece { over, .. }) => over.push(*under),
                            None => {
                                parts.push(part);
                                upper.push(true);
                                items.push(Some(WornPiece { item: item.clone(), over: vec![*under] }));
                                of_outfit.push(None);
                                top.push(false);
                                suit.push(None);
                            }
                        }
                    }
                    Ok(Worn::Rigid(model)) => {
                        let (model, fitted, hinges) = trp_data::dress::head_model(unity, db, item, model, &|part| present.contains(part));
                        if !fitted.is_empty() || !hinges.is_empty() {
                            let named: String = hinges.iter().map(|hinge| format!(", a hinge on {}{}", model.nodes[hinge.node].name, hinge.owner.last().map_or(String::new(), |(_, of)| format!(" (its part {of}'s own)")))).collect();
                            println!("worn on the head, {item}: {} parts it may have fitted{named}", fitted.len());
                        }
                        on_head.push(HeadModel { item: item.clone(), model, parts: fitted, hinges });
                        break;
                    }
                    Err(e) => {
                        eprintln!("{item} is not worn: {e:#}");
                        break;
                    }
                }
            }
        }
        // Every bone a mesh hangs on that is not an arm's, and every bone
        // above one of those. The collarbones are wanted whatever the
        // meshes say: the arms' own mesh hangs on them too.
        let mut bones: Vec<String> = Vec::new();
        let ribcage = parts.iter().flat_map(|part| &part.bone_paths).find_map(|path| path.find(RIBCAGE).map(|at| &path[..at]));
        let mut wanted: Vec<String> =
            ribcage.into_iter().flat_map(|above| COLLARBONES.iter().map(move |collarbone| format!("{above}{collarbone}"))).collect();
        // What rides on the head needs the head's bone, whether or not a
        // head is drawn: the neck hangs beside the ribcage.
        if !on_head.is_empty() {
            wanted.extend(ribcage.map(|above| format!("{above}/Base HumanNeck/{HEAD}")));
        }
        if suits {
            wanted.extend(SUIT_BONES.map(str::to_owned));
        }
        let of_outfits = outfits.iter().flat_map(|(_, outfit)| outfit.rig.bones.iter().chain(outfit.arms.iter().flat_map(|arms| &arms.rig.bones)).map(|bone| &bone.path));
        let of_hands = their_hands.iter().flat_map(|(_, hands)| &hands.bone_paths);
        for path in parts.iter().flat_map(|part| &part.bone_paths).chain(&wanted).chain(of_outfits).chain(of_hands).filter(|path| !of_an_arm(path)) {
            for (end, _) in path.match_indices('/').chain(std::iter::once((path.len(), ""))) {
                if !bones.iter().any(|known| known == &path[..end]) {
                    bones.push(path[..end].to_owned());
                }
            }
        }
        bones.sort_by_key(|path| path.matches('/').count());
        // The character model's: the same and the arms, less what only the
        // character's own eyes see (an outfit's arms for them).
        let mut standing: Vec<String> = Vec::new();
        let of_bodies = outfits.iter().flat_map(|(_, outfit)| &outfit.rig.bones).map(|bone| &bone.path);
        for path in parts.iter().flat_map(|part| &part.bone_paths).chain(&wanted).chain(of_bodies) {
            for (end, _) in path.match_indices('/').chain(std::iter::once((path.len(), ""))) {
                if !standing.iter().any(|known| known == &path[..end]) {
                    standing.push(path[..end].to_owned());
                }
            }
        }
        standing.sort_by_key(|path| path.matches('/').count());
        Self { parts, upper, bones, on_head, item: items, armour, vests, vestlike, suit, outfits, of_outfit, top, their_hands, standing, standing_pose: None, dummy_pose: None, head }
    }

    /// The torso's mesh the game draws for what the character has on
    /// (`TorsoSkin.ApplySkin`: the slots of the body armour, the tactical
    /// vest and the backpack, and whether the armour is worn like a vest).
    pub fn torso_for(&self, carried: &Carried) -> trp_data::arms::Torso {
        let worn = |slot: &str| carried.equipment.worn(slot);
        let armour = worn("ArmorVest");
        let vestlike = armour.is_some_and(|armour| self.vestlike.contains(&armour.kind.template));
        trp_data::arms::TorsoMeshes::drawn(armour.is_some(), vestlike, worn("TacticalVest").is_some(), worn("Backpack").is_some())
    }

    /// What an outfit has to draw in place of the character's own: a
    /// body of its own, one of the install's tops, and arms for the
    /// character's own eyes (its own, or the install's hands).
    fn drawn_of(&self, item: &str, outfit: &trp_data::outfit::Outfit) -> (bool, bool, bool) {
        let top = self.of_outfit.iter().any(|of| of.as_deref() == Some(item));
        let hands = self.their_hands.iter().any(|(of, _)| of == item);
        (outfit.has_body(), top, outfit.has_arms() || hands)
    }
}

/// Draws an outfit that is not the install's while the character has it
/// on (in the slot the game draws armour from, or named by `--wear`), and
/// the character's own body while it has none.
///
/// **An outfit's arms for the character's own eyes go by a switch of the
/// options** (`CUSTOM FIRST PERSON HANDS`, `Features::custom_hands`; the
/// user, 2026-10-13): off, the hands do not change with what is worn, and
/// are the install's default arms.
pub fn show_outfit(
    carried: Res<Carried>,
    parts: Res<BodyParts>,
    features: Res<crate::options_view::Features>,
    mut on: ResMut<OutfitOn>,
    made: Query<(), (Or<(Added<OutfitPiece>, Added<OwnBody>)>, EvenParked)>,
    mut pieces: Query<(&OutfitPiece, &mut Visibility), (Without<OwnBody>, EvenParked)>,
    mut own: Query<(&OwnBody, &mut Visibility), (Without<OutfitPiece>, Without<TorsoPiece>, EvenParked)>,
) {
    if parts.outfits.is_empty() || (!carried.is_changed() && made.is_empty() && !features.is_changed()) {
        return;
    }
    let worn: Vec<&str> = carried.fixed_wear.iter().map(String::as_str).chain(carried.equipment.drawn()).collect();
    // The first outfit worn that has anything to draw. What it has not
    // got stays the character's own: the body where it has none (not
    // named, not found, or none of its pictures there), the arms for its
    // own eyes where it has none of those.
    let wearing = parts
        .outfits
        .iter()
        .map(|(item, outfit)| (item.as_str(), parts.drawn_of(item, outfit)))
        .map(|(item, (body, top, arms))| (item, (body, top, arms && features.custom_hands)))
        .filter(|(_, (body, top, arms))| *body || *top || *arms)
        .find(|(item, _)| worn.contains(item));
    let now = match wearing {
        Some((item, (body, top, arms))) => OutfitOn { item: Some(item.to_owned()), body, top: top && !body, arms },
        None => OutfitOn::default(),
    };
    if *on != now {
        *on = now;
    }
    let set = |visibility: &mut Mut<Visibility>, shown: bool| {
        let wanted = if shown { Visibility::Inherited } else { Visibility::Hidden };
        if **visibility != wanted {
            **visibility = wanted;
        }
    };
    for (piece, mut visibility) in &mut pieces {
        set(&mut visibility, on.item.as_deref() == Some(piece.item.as_str()));
    }
    for (piece, mut visibility) in &mut own {
        set(&mut visibility, !(on.body || (on.top && piece.top)));
    }
}

/// Draws of a torso of the newer kind the mesh the game draws for what is
/// worn (`TorsoSkin.ApplySkin`, as the armour, the vest or the backpack
/// comes and goes), where the character's own body is drawn at all
/// (`show_outfit`'s rule).
pub fn show_torso(
    carried: Res<Carried>,
    parts: Res<BodyParts>,
    on: Res<OutfitOn>,
    made: Query<(), (Added<TorsoPiece>, EvenParked)>,
    mut pieces: Query<(&OwnBody, &TorsoPiece, &mut Visibility), EvenParked>,
) {
    if !carried.is_changed() && !on.is_changed() && made.is_empty() {
        return;
    }
    let drawn = parts.torso_for(&carried);
    for (own, torso, mut visibility) in &mut pieces {
        let shown = torso.0 == drawn && !(on.body || (on.top && own.top));
        let wanted = if shown { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// Draws of the worn items those the character has on: what the
/// equipment holds in the slots the game draws (inventory spec,
/// "Equipment slots") and what `--wear` named, each by the mesh for what
/// it is then worn over.
pub fn show_worn(
    carried: Res<Carried>,
    parts: Res<BodyParts>,
    made: Query<(), (Added<WornPiece>, EvenParked)>,
    mut pieces: Query<(&WornPiece, &mut Visibility), EvenParked>,
) {
    if !carried.is_changed() && made.is_empty() {
        return;
    }
    let on: Vec<&str> = carried.fixed_wear.iter().map(String::as_str).chain(carried.equipment.drawn()).collect();
    let wearing = |listed: &[String]| listed.iter().any(|item| on.contains(&item.as_str()));
    let under = Under { armour: wearing(&parts.armour), vest: wearing(&parts.vests) };
    for (piece, mut visibility) in &mut pieces {
        let shown = on.contains(&piece.item.as_str()) && (piece.over.is_empty() || piece.over.contains(&under));
        let wanted = if shown { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// The thing worn in a slot the game draws that is of that template.
fn worn_of<'a>(carried: &'a Carried, template: &str) -> Option<&'a trp_sim::inventory::Item> {
    trp_sim::inventory::WORN.iter().filter_map(|slot| carried.equipment.worn(slot)).find(|item| item.kind.template == template)
}

/// Draws of the parts a worn thing's model may have fitted those the thing
/// worn has fitted so (`ObjectsFactory.AttachMods`): each slot down to the
/// part holding what it names.
pub fn show_head_parts(carried: Res<Carried>, made: Query<(), (Added<HeadPart>, EvenParked)>, mut parts: Query<(&HeadPart, &mut Visibility), EvenParked>) {
    if !carried.is_changed() && made.is_empty() {
        return;
    }
    for (part, mut visibility) in &mut parts {
        let fitted = worn_of(&carried, &part.item).and_then(|worn| part.path.iter().try_fold(worn, |at, (slot, template)| at.in_slot(slot).filter(|held| held.kind.template == *template)));
        let wanted = if fitted.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// Turns each hinge as its face shield stands (`HelmetDress`; body animator
/// spec, "A worn thing's parts and a helmet's hinge"), by the thing whose
/// model carries it (the helmet; or a face shield, its own): that thing put
/// on or fitted is set at once (`Init`, `UpdateState(initial)`, as its
/// model is made); while it is worn, its face shield changed or switched
/// sets the filter's way (`SetFaceShield`, `OnFaceShieldChanged`), and each
/// frame the filter moves on (`HysteresisFilter.Update`); a value given
/// turns the hinge to the engine's blend of its two turns
/// (`Quaternion.Lerp`). A turn of the gear page's figure asks for its
/// drawing.
pub fn turn_hinges(carried: Res<Carried>, time: Res<Time>, mut hinges: Query<(&mut HelmetHinge, &mut Transform), EvenParked>, mut figure: Option<ResMut<crate::figure_view::Figure>>) {
    for (mut hinge, mut transform) in &mut hinges {
        let owner = worn_of(&carried, &hinge.item).and_then(|worn| hinge.owner.iter().try_fold(worn, |at, (slot, template)| at.in_slot(slot).filter(|held| held.kind.template == *template)));
        let Some(worn) = owner else {
            if hinge.worn.is_some() {
                hinge.worn = None;
            }
            continue;
        };
        let shield = trp_sim::inventory::face_shield(worn).map(|shield| shield.id);
        let down = trp_sim::inventory::hinge_down(worn);
        let hinge = &mut *hinge;
        let given = if hinge.worn != Some(worn.id) {
            (hinge.worn, hinge.shield, hinge.down) = (Some(worn.id), shield, down);
            hinge.filter.set(down, true)
        } else {
            if hinge.shield != shield || hinge.down != down {
                (hinge.shield, hinge.down) = (shield, down);
                hinge.filter.set(down, false);
            }
            hinge.filter.update(time.delta_secs())
        };
        if let Some(value) = given {
            transform.rotation = convert::rotation(trp_sim::math::quat_lerp(hinge.off, hinge.on, value));
            if hinge.figure {
                if let Some(figure) = figure.as_mut() {
                    figure.ask();
                }
            }
        }
    }
}

/// The drawn body's skeleton.
#[derive(Resource)]
pub struct BodyRig {
    /// What the skeleton hangs from: at the feet, turned as the body is.
    root: Entity,
    /// One entity for each bone of `BodyParts::bones`, in that order.
    bones: Vec<Entity>,
    /// The collarbones: each as the animation has it, and the entity
    /// under the weapon rig's root that is kept where that one is.
    collarbones: Vec<(Entity, Entity)>,
    /// The weapon rig's root, which stands for the ribcage in first
    /// person.
    ribcage: Entity,
    /// The ribcage as the animation has it, and the entity under the
    /// weapon rig's root that the meshes' ribcage is: on that root for
    /// the character's own eyes, where the animated one is for another's.
    ribcage_animated: Option<Entity>,
    ribcage_drawn: Entity,
}

impl BodyRig {
    /// The entity of the bone whose path ends so, if the body has it.
    pub fn bone(&self, parts: &BodyParts, end: &str) -> Option<Entity> {
        parts.bones.iter().position(|path| path.ends_with(end)).map(|at| self.bones[at])
    }

    /// The entity a bone of the skeleton (by its path) is drawn on, as
    /// `spawn_body` ties the meshes to it: the ribcage as drawn, a kept
    /// collarbone, the weapon rig's own bone for an arm's (the nearest
    /// above it that the rig in hand has), or the body's own.
    pub fn drawn(&self, parts: &BodyParts, weapon: &WeaponRig, path: &str) -> Option<Entity> {
        if path.ends_with(RIBCAGE) {
            return Some(self.ribcage_drawn);
        }
        if a_collarbone(path) {
            let index = parts.bones.iter().filter(|known| a_collarbone(known)).position(|known| known == path)?;
            return self.collarbones.get(index).map(|(_, kept)| *kept);
        }
        if of_an_arm(path) {
            let mut on_rig = &path[path.find(RIBCAGE).map_or(0, |at| at + RIBCAGE.len() + 1)..];
            loop {
                if let Some(entity) = weapon.by_hash.get(&path_hash(on_rig)) {
                    return Some(*entity);
                }
                on_rig = &on_rig[..on_rig.rfind('/')?];
            }
        }
        parts.bones.iter().position(|known| known == path).map(|at| self.bones[at])
    }

    /// What the drawn body hangs from.
    pub fn root(&self) -> Entity {
        self.root
    }

    /// Each collarbone the meshes are drawn on (kept where the animated
    /// one is), in the order the body lists them.
    pub fn kept_collarbones(&self) -> impl Iterator<Item = Entity> + '_ {
        self.collarbones.iter().map(|(_, kept)| *kept)
    }

    /// The ribcage as the body's animation has it, and as it is drawn.
    pub fn ribcages(&self) -> (Option<Entity>, Entity) {
        (self.ribcage_animated, self.ribcage_drawn)
    }
}

/// Where the kept collarbones go this frame, in the rig root's frame, and
/// the drawn ribcage with them.
#[derive(Resource, Default)]
pub struct KeptCollarbones {
    collarbones: Vec<Transform>,
    ribcage: Transform,
}

/// Spawns the skeleton and the clothing skinned to it. Runs after the
/// weapon is spawned: the arms' bones are the weapon rig's.
pub fn spawn_body(
    mut commands: Commands,
    mut game: ResMut<GameData>,
    parts: Res<BodyParts>,
    rig: Option<Res<WeaponRig>>,
    mut skinned: Query<&mut SkinnedMesh>,
    mut converted: ResMut<Converted>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
) {
    let (Some(rig), false) = (rig, parts.parts.is_empty()) else { return };
    let unity = &mut game.unity;
    let root = commands.spawn((Name::new("Body"), Transform::default(), Visibility::default())).id();
    let ribcage_drawn = commands.spawn((Name::new("Ribcage as drawn"), Transform::default(), Visibility::default())).id();
    commands.entity(rig.root).add_child(ribcage_drawn);
    let mut by_path: HashMap<&str, Entity> = HashMap::new();
    let mut bones = Vec::with_capacity(parts.bones.len());
    for path in &parts.bones {
        let name = path.rsplit('/').next().unwrap_or(path);
        let entity = commands.spawn((Name::new(name.to_owned()), Transform::default(), Visibility::default())).id();
        // What hangs from the ribcage turns with it to the view, so it
        // hangs from the ribcage as drawn; the collarbones do not (they
        // are kept where the animation has them), so the animated ones
        // stay under the animated ribcage.
        let above = path.rfind('/').map(|at| &path[..at]);
        let parent = match above {
            Some(above) if above.ends_with(RIBCAGE) && !a_collarbone(path) => ribcage_drawn,
            Some(above) => by_path.get(above).copied().unwrap_or(root),
            None => root,
        };
        commands.entity(parent).add_child(entity);
        by_path.insert(path.as_str(), entity);
        bones.push(entity);
    }

    // A collarbone that is kept where the animated one is, for each.
    let mut collarbones = Vec::new();
    let mut kept: HashMap<&str, Entity> = HashMap::new();
    for path in parts.bones.iter().filter(|path| a_collarbone(path)) {
        let entity = commands.spawn((Name::new("Collarbone as kept"), Transform::default(), Visibility::default())).id();
        commands.entity(rig.root).add_child(entity);
        collarbones.push((by_path[path.as_str()], entity));
        kept.insert(path.as_str(), entity);
        // The arms' own mesh hangs its shoulders on the weapon rig's
        // collarbone; it goes on the kept one too.
        let on_rig = &path[path.find(RIBCAGE).map_or(0, |at| at + RIBCAGE.len() + 1)..];
        if let Some(rigs) = rig.by_hash.get(&path_hash(on_rig)) {
            for mut mesh in &mut skinned {
                for joint in mesh.joints.iter_mut().filter(|joint| *joint == rigs) {
                    *joint = entity;
                }
            }
        }
    }

    // The character model on the gear page, where the options have it: the
    // same meshes on a skeleton of its own, standing (`figure_view`).
    let mut figure = parts.standing_pose.as_ref().map(|pose| crate::figure_view::Builder::new(&mut commands, &parts.standing, pose));
    // And the corpse the character becomes, likewise, hidden until then
    // (`death`).
    let mut corpse = crate::figure_view::Builder::corpse(&mut commands, &parts.standing);
    // And the dummies on the range, with their switch on: the same again
    // for each, standing as the model does where it stands until its
    // character poses it (`dummy`).
    let mut dummies: Vec<crate::figure_view::Builder> = parts
        .dummy_pose
        .as_ref()
        .map(|pose| (0..crate::dummy::PLACES.len()).map(|which| crate::figure_view::Builder::dummy(&mut commands, &parts.standing, pose, crate::dummy::place(which), which)).collect())
        .unwrap_or_default();

    // The entity a mesh's bone is tied to.
    let joint = |path: &str| -> Entity {
        if path.ends_with(RIBCAGE) {
            return ribcage_drawn;
        }
        if let Some(entity) = kept.get(path) {
            return *entity;
        }
        if of_an_arm(path) {
            // Named on the weapon's rig from the collarbone down. A bone
            // the rig lacks is taken as the nearest one above it.
            let mut on_rig = &path[path.find(RIBCAGE).map_or(0, |at| at + RIBCAGE.len() + 1)..];
            loop {
                if let Some(entity) = rig.by_hash.get(&path_hash(on_rig)) {
                    return *entity;
                }
                match on_rig.rfind('/') {
                    Some(at) => on_rig = &on_rig[..at],
                    None => return rig.root,
                }
            }
        }
        by_path.get(path).copied().unwrap_or(root)
    };

    let mut drawn = 0;
    for (index, (((part, upper), item), (of_outfit, top))) in parts.parts.iter().zip(&parts.upper).zip(&parts.item).zip(parts.of_outfit.iter().zip(&parts.top)).enumerate() {
        // A torso of the newer kind has a mesh for each case of what is
        // worn, each drawn by itself (`show_torso`).
        let meshes_of: Vec<(Option<trp_data::arms::Torso>, trp_data::unity::ObjRef)> = match &part.torso {
            Some(torso) => trp_data::arms::Torso::ALL.iter().filter_map(|&case| Some((Some(case), torso.mesh(case)?))).collect(),
            None => vec![(None, part.mesh)],
        };
        let joints: Vec<Entity> = part.bone_paths.iter().map(|path| joint(path)).collect();
        for (case, mesh_ref) in meshes_of {
            let entry = match converted.mesh(unity, mesh_ref, &mut meshes) {
                Ok(entry) => entry,
                Err(e) => {
                    warn!("a mesh of the body: {e:#}");
                    continue;
                }
            };
            let inverse_bindposes = bindposes.add(SkinnedMeshInverseBindposes::from(entry.bind_poses.clone()));
            for (slot, mesh) in entry.parts.iter().enumerate() {
                let material_ref = part.materials.get(slot).copied().flatten();
                let material = converted.material(unity, material_ref, &mut images, &mut materials);
                let piece = commands
                    .spawn((
                        Name::new("Clothing"),
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        SkinnedMesh { inverse_bindposes: inverse_bindposes.clone(), joints: joints.clone() },
                        // Skinned bounds are not tracked.
                        NoFrustumCulling,
                        // Out of the engine's work while hidden (`park`).
                        Park,
                    ))
                    .id();
                if *upper {
                    commands.entity(piece).insert((UpperBody, RenderLayers::layer(SEEN)));
                } else {
                    commands.entity(piece).insert((LowerBody, RenderLayers::layer(SEEN)));
                }
                match (item, of_outfit) {
                    (Some(worn), _) => commands.entity(piece).insert(worn.clone()),
                    // One of the install's own tops, worn as an outfit.
                    (None, Some(outfit)) => commands.entity(piece).insert((OutfitPiece { item: outfit.clone() }, Visibility::Hidden)),
                    (None, None) => commands.entity(piece).insert(OwnBody { top: *top }),
                };
                if let Some(role) = parts.suit.get(index).copied().flatten() {
                    commands.entity(piece).insert(SuitPiece(role));
                }
                if let Some(case) = case {
                    commands.entity(piece).insert((TorsoPiece(case), Visibility::Hidden));
                }
                commands.entity(root).add_child(piece);
                let hidden = (item.is_none() && of_outfit.is_some()) || case.is_some();
                if let Some(figure) = figure.as_mut() {
                    figure.skinned(&mut commands, piece, (mesh, &material), &inverse_bindposes, &part.bone_paths, hidden);
                }
                let corpse_piece = corpse.skinned(&mut commands, piece, (mesh, &material), &inverse_bindposes, &part.bone_paths, hidden);
                if parts.head == Some(index) {
                    commands.entity(corpse_piece).insert(crate::death::CorpseHead);
                }
                for dummy in &mut dummies {
                    dummy.skinned(&mut commands, piece, (mesh, &material), &inverse_bindposes, &part.bone_paths, hidden);
                }
                drawn += 1;
            }
        }
    }
    // The outfits that are not the install's (`docs/spec/outfits.md`).
    // Each has a skeleton of its own to be drawn on: an entity for each
    // bone of its rig, which every frame is given the move, from rest, of
    // what is on that bone (`see_outfit`). Its meshes hang on those, each
    // in its three parts, and are not drawn until its item is on. What
    // of it lies under the waist is seen by the character's own eyes too;
    // the rest, its arms with it, is the upper body's, a shadow there and
    // no more. Its arms for those eyes are a model of their own (the
    // other game's for first person, on a second such skeleton), seen in
    // first person and nowhere else; an outfit without one leaves the
    // install's own arms there.
    let mut skeletons = Vec::new();
    let weapon_root = rig.root;
    let mut skeleton_of = |commands: &mut Commands, item: &str, rig: &trp_data::outfit::Rig| -> Vec<Entity> {
        // What the game squeezes with the weapon hangs, through two
        // entities, from the weapon rig's root: the first is that root
        // again, the second undoes its place and its turn.
        let squeezed_root = commands.spawn((Name::new("Outfit, as the weapon is squeezed"), Transform::default(), Visibility::default())).id();
        let unsqueeze = commands.spawn((Name::new("Outfit, the root undone"), Transform::default(), Visibility::default())).id();
        commands.entity(weapon_root).add_child(squeezed_root);
        commands.entity(squeezed_root).add_child(unsqueeze);
        let squeezed: Vec<bool> = rig.bones.iter().map(|bone| of_an_arm(&bone.path) || a_collarbone(&bone.path)).collect();
        let ours: Vec<Entity> = rig
            .bones
            .iter()
            .zip(&squeezed)
            .map(|(bone, squeezed)| {
                let name = bone.path.rsplit('/').next().unwrap_or(&bone.path);
                let entity = commands.spawn((Name::new(format!("{name}, the outfit's")), Transform::default(), Visibility::default())).id();
                if *squeezed {
                    commands.entity(unsqueeze).add_child(entity);
                }
                entity
            })
            .collect();
        skeletons.push(OutfitSkeleton {
            item: item.to_owned(),
            // The rig's points in the engine's axes, as the meshes are.
            rig: rig.in_space(|p| convert::position(p).to_array()),
            theirs: rig.bones.iter().map(|bone| joint(&bone.path)).collect(),
            their_rest: rig.bones.iter().map(|bone| bevy::math::Affine3A::from_mat4(convert::matrix(bone.bind))).collect(),
            squeezed,
            ours: ours.clone(),
            unsqueeze,
        });
        ours
    };
    for (item, outfit) in &parts.outfits {
        let ours = if outfit.has_body() { skeleton_of(&mut commands, item, &outfit.rig) } else { Vec::new() };
        // The model's own skeleton for it, stood once.
        let rest: Vec<bevy::math::Affine3A> = outfit.rig.bones.iter().map(|bone| bevy::math::Affine3A::from_mat4(convert::matrix(bone.bind))).collect();
        let model_ours = figure.as_mut().filter(|_| outfit.has_body()).map(|figure| figure.outfit(&mut commands, item, &outfit.rig.in_space(|p| convert::position(p).to_array()), &rest));
        let corpse_ours = outfit.has_body().then(|| corpse.outfit(&mut commands, item, &outfit.rig.in_space(|p| convert::position(p).to_array()), &rest));
        let dummies_ours: Vec<Vec<Entity>> = if outfit.has_body() {
            dummies.iter_mut().map(|dummy| dummy.outfit(&mut commands, item, &outfit.rig.in_space(|p| convert::position(p).to_array()), &rest)).collect()
        } else {
            Vec::new()
        };
        for piece in outfit.pieces.iter().filter(|_| outfit.has_body()) {
            // A mesh whose picture was not found is not drawn at all: it
            // would have no colour of its own.
            let Some(picture) = &piece.picture else { continue };
            let inverse_bindposes = bindposes.add(SkinnedMeshInverseBindposes::from(piece.mesh.bind_poses.iter().map(|rows| convert::matrix(*rows)).collect::<Vec<_>>()));
            let material = converted.outfit_material(picture, &mut images, &mut materials);
            for (zone, mesh) in converted.custom_mesh(&piece.mesh, &mut meshes).into_iter().enumerate() {
                if piece.mesh.submeshes[zone].is_empty() {
                    continue;
                }
                let entity = commands
                    .spawn((
                        Name::new("Outfit"),
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        SkinnedMesh { inverse_bindposes: inverse_bindposes.clone(), joints: ours.clone() },
                        NoFrustumCulling,
                        Visibility::Hidden,
                        OutfitPiece { item: item.clone() },
                        Park,
                    ))
                    .id();
                if let (Some(figure), Some(model_ours)) = (figure.as_mut(), model_ours.as_ref()) {
                    figure.skinned_on(&mut commands, entity, (&mesh, &material), &inverse_bindposes, model_ours.clone(), true);
                }
                if let Some(corpse_ours) = corpse_ours.as_ref() {
                    corpse.skinned_on(&mut commands, entity, (&mesh, &material), &inverse_bindposes, corpse_ours.clone(), true);
                }
                for (dummy, ours) in dummies.iter_mut().zip(&dummies_ours) {
                    dummy.skinned_on(&mut commands, entity, (&mesh, &material), &inverse_bindposes, ours.clone(), true);
                }
                if zone != trp_data::outfit::LEGS {
                    commands.entity(entity).insert((UpperBody, RenderLayers::layer(SEEN)));
                } else {
                    commands.entity(entity).insert((LowerBody, RenderLayers::layer(SEEN)));
                }
                commands.entity(root).add_child(entity);
                drawn += 1;
            }
        }
        let Some(arms) = outfit.arms.as_ref().filter(|_| outfit.has_arms()) else { continue };
        let ours = skeleton_of(&mut commands, item, &arms.rig);
        for piece in &arms.pieces {
            let Some(picture) = &piece.picture else { continue };
            let inverse_bindposes = bindposes.add(SkinnedMeshInverseBindposes::from(piece.mesh.bind_poses.iter().map(|rows| convert::matrix(*rows)).collect::<Vec<_>>()));
            let material = converted.outfit_material(picture, &mut images, &mut materials);
            for (zone, mesh) in converted.custom_mesh(&piece.mesh, &mut meshes).into_iter().enumerate() {
                if piece.mesh.submeshes[zone].is_empty() {
                    continue;
                }
                // The shadow is the body's own arms': these cast none.
                let entity = commands
                    .spawn((
                        Name::new("Outfit, its arms for its own eyes"),
                        Mesh3d(mesh),
                        MeshMaterial3d(material.clone()),
                        SkinnedMesh { inverse_bindposes: inverse_bindposes.clone(), joints: ours.clone() },
                        NoFrustumCulling,
                        bevy::light::NotShadowCaster,
                        Visibility::Hidden,
                        OutfitArms { item: item.clone() },
                    ))
                    .id();
                commands.entity(root).add_child(entity);
                drawn += 1;
            }
        }
    }
    // The install's own hands as an outfit's arms: skinned to the arms'
    // bones as the character's own hands are, and drawn in their place
    // in first person while the item is on.
    for (item, hands) in &parts.their_hands {
        let entry = match converted.mesh(unity, hands.mesh, &mut meshes) {
            Ok(entry) => entry,
            Err(e) => {
                warn!("the hands of an outfit: {e:#}");
                continue;
            }
        };
        let joints: Vec<Entity> = hands.bone_paths.iter().map(|path| joint(path)).collect();
        let inverse_bindposes = bindposes.add(SkinnedMeshInverseBindposes::from(entry.bind_poses.clone()));
        for (slot, mesh) in entry.parts.iter().enumerate() {
            let material_ref = hands.materials.get(slot).copied().flatten();
            let material = converted.material(unity, material_ref, &mut images, &mut materials);
            let entity = commands
                .spawn((
                    Name::new("Outfit, the install's own hands"),
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    SkinnedMesh { inverse_bindposes: inverse_bindposes.clone(), joints: joints.clone() },
                    NoFrustumCulling,
                    bevy::light::NotShadowCaster,
                    Visibility::Hidden,
                    OutfitArms { item: item.clone() },
                ))
                .id();
            commands.entity(root).add_child(entity);
            drawn += 1;
        }
    }
    commands.insert_resource(OutfitSkeletons(skeletons));
    // What rides on the head: the item's own model, parented to the head
    // bone with no offset and the one fixed turn the game gives it.
    let head = parts.bones.iter().position(|path| path.ends_with(HEAD)).map(|at| bones[at]);
    let model_head = figure.as_ref().and_then(|figure| figure.bone(HEAD));
    let corpse_head = corpse.bone(HEAD);
    let dummy_heads: Vec<Option<Entity>> = dummies.iter().map(|dummy| dummy.bone(HEAD)).collect();
    for HeadModel { item, model, parts: fitted, hinges } in parts.on_head.iter().filter(|_| head.is_some()) {
        let mut entities: Vec<Entity> = Vec::with_capacity(model.nodes.len());
        let mut model_entities: Vec<Entity> = Vec::with_capacity(model.nodes.len());
        let mut corpse_entities: Vec<Entity> = Vec::with_capacity(model.nodes.len());
        let mut dummy_entities: Vec<Vec<Entity>> = vec![Vec::with_capacity(model.nodes.len()); dummies.len()];
        for (index, node) in model.nodes.iter().enumerate() {
            let transform = match index {
                0 => convert::transform([0.0; 3], [0.5, -0.5, -0.5, -0.5], [1.0; 3]),
                _ => convert::transform(node.translation, node.rotation, node.scale),
            };
            let visibility = if node.active { Visibility::Inherited } else { Visibility::Hidden };
            let entity = commands.spawn((Name::new(node.name.clone()), transform, visibility)).id();
            // The whole model is shown and hidden by its first object, a
            // part it may have fitted by the part's.
            if index == 0 {
                commands.entity(entity).insert((WornPiece { item: item.clone(), over: Vec::new() }, Park));
            }
            let part = fitted.iter().find(|part| part.root == index);
            if let Some(part) = part {
                commands.entity(entity).insert((HeadPart { item: item.clone(), path: part.path.clone() }, Park));
            }
            let shown_by = (index == 0 || part.is_some()).then_some(entity);
            // Its hinge, on each copy of it.
            let hinged = hinges.iter().find(|hinge| hinge.node == index).map(|hinge| HelmetHinge {
                item: item.clone(),
                owner: hinge.owner.clone(),
                off: hinge.off,
                on: hinge.on,
                filter: trp_sim::hysteresis::Hysteresis::new(hinge.forward.clone(), hinge.backward.clone()),
                worn: None,
                shield: None,
                down: true,
                figure: false,
            });
            if let Some(hinged) = &hinged {
                commands.entity(entity).insert(hinged.clone());
            }
            commands.entity(node.parent.map_or(head.unwrap_or(root), |p| entities[p])).add_child(entity);
            entities.push(entity);
            if let Some(figure) = figure.as_mut() {
                let on = node.parent.map(|p| model_entities[p]).or(model_head);
                let copy = figure.hung(&mut commands, on, transform, visibility, shown_by);
                if let Some(hinged) = &hinged {
                    commands.entity(copy).insert(HelmetHinge { figure: true, ..hinged.clone() });
                }
                model_entities.push(copy);
            }
            for (which, dummy) in dummies.iter_mut().enumerate() {
                let on = node.parent.map(|p| dummy_entities[which][p]).or(dummy_heads[which]);
                let copy = dummy.hung(&mut commands, on, transform, visibility, shown_by);
                dummy_entities[which].push(copy);
            }
            let on = node.parent.map(|p| corpse_entities[p]).or(corpse_head);
            let copy = corpse.hung(&mut commands, on, transform, visibility, shown_by);
            if let Some(hinged) = hinged {
                commands.entity(copy).insert(hinged);
            }
            corpse_entities.push(copy);
            let Some(renderer) = &node.renderer else { continue };
            let entry = match converted.mesh(unity, renderer.mesh, &mut meshes) {
                Ok(entry) => entry,
                Err(e) => {
                    warn!("mesh on {}: {e:#}", node.name);
                    continue;
                }
            };
            // Hung on the head and drawn as it lies: a mesh with bones and
            // none to move them stops the renderer.
            crate::render_data::unskin_all(&mut meshes, &entry.parts);
            for (slot, mesh) in entry.parts.iter().enumerate() {
                let material_ref = renderer.materials.get(slot).copied().flatten();
                let material = converted.material(unity, material_ref, &mut images, &mut materials);
                let piece = commands
                    .spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default(), UpperBody, RenderLayers::layer(SEEN)))
                    .id();
                commands.entity(entity).add_child(piece);
                if let (Some(figure), Some(on)) = (figure.as_mut(), model_entities.last()) {
                    figure.rigid(&mut commands, *on, mesh, &material);
                }
                if let Some(on) = corpse_entities.last() {
                    let corpse_piece = corpse.rigid(&mut commands, *on, mesh, &material);
                    commands.entity(corpse_piece).insert(crate::death::CorpseHead);
                }
                for (which, dummy) in dummies.iter_mut().enumerate() {
                    if let Some(on) = dummy_entities[which].last() {
                        dummy.rigid(&mut commands, *on, mesh, &material);
                    }
                }
                drawn += 1;
            }
        }
    }
    info!("the body: {} bones posed by its animator, {} meshes", bones.len(), drawn);
    if let Some(figure) = figure {
        figure.finish(&mut commands);
    }
    corpse.finish_corpse(&mut commands);
    if !dummies.is_empty() {
        let (rigs, skeletons): (Vec<_>, Vec<_>) = dummies.into_iter().map(|dummy| dummy.finish_dummy()).unzip();
        commands.insert_resource(crate::dummy::DummyRigs(rigs));
        commands.insert_resource(crate::dummy::DummySkeletons(skeletons));
    }
    let ribcage_animated = parts.bones.iter().position(|path| path.ends_with(RIBCAGE)).map(|at| bones[at]);
    commands.insert_resource(BodyRig { root, bones, collarbones, ribcage: rig.root, ribcage_animated, ribcage_drawn });
}

/// Works out where each kept collarbone goes: where the animated one is,
/// seen from the weapon rig's root with its squeeze left out, so that the
/// squeeze draws it in as the game's does. And the drawn ribcage: on that
/// root for the character's own eyes, and for another's where the animated
/// ribcage is, seen the same way.
///
/// For the character's own eyes on a mount, the root is moved off the
/// ribcage onto the mount's point (`Shown::rig_apart`), and the arms go
/// with it whole: recorded, the game's upper arms stay as far from the
/// weapon as off a mount while they leave the ribcage by up to half a
/// metre (`courses7\10-anything`). So the collarbones are seen from where
/// the root would be without the mount's move, and carried with it; seen
/// from the moved root they stayed on the body and the sleeves between
/// them and the upper arms stretched.
pub fn see_collarbones(
    rig: Option<Res<BodyRig>>,
    controller: Res<Controller>,
    helper: TransformHelper,
    mut kept: ResMut<KeptCollarbones>,
) {
    kept.collarbones.clear();
    kept.ribcage = Transform::IDENTITY;
    let Some(rig) = rig else { return };
    let Ok(ribcage) = helper.compute_global_transform(rig.ribcage) else { return };
    let (_, turn, mut at) = ribcage.to_scale_rotation_translation();
    let own_eyes = controller.movement.body.point_of_view() == trp_sim::PointOfView::FirstPerson;
    if own_eyes {
        at -= turn * crate::convert::position(controller.shown().rig_apart);
    }
    let seen = |entity: Entity| -> Option<Transform> {
        let (_, bone_turn, bone_at) = helper.compute_global_transform(entity).ok()?.to_scale_rotation_translation();
        Some(Transform { translation: turn.inverse() * (bone_at - at), rotation: turn.inverse() * bone_turn, scale: Vec3::ONE })
    };
    for (animated, _) in &rig.collarbones {
        let Some(place) = seen(*animated) else { return };
        kept.collarbones.push(place);
    }
    if controller.movement.body.point_of_view() == trp_sim::PointOfView::ThirdPerson {
        if let Some(place) = rig.ribcage_animated.and_then(seen) {
            kept.ribcage = place;
        }
    }
}

/// Puts each kept collarbone there, and the drawn ribcage.
pub fn keep_collarbones(rig: Option<Res<BodyRig>>, kept: Res<KeptCollarbones>, mut transforms: Query<&mut Transform>) {
    let Some(rig) = rig else { return };
    for ((_, entity), place) in rig.collarbones.iter().zip(&kept.collarbones) {
        if let Ok(mut transform) = transforms.get_mut(*entity) {
            *transform = *place;
        }
    }
    if let Ok(mut transform) = transforms.get_mut(rig.ribcage_drawn) {
        *transform = kept.ribcage;
    }
}

/// Works out where the bones of the outfit that is on go: the install's
/// bones as they stand this frame, each as the move from rest of what is
/// on it, are given to the outfit's rig, which answers with its own
/// (`trp_data::outfit::Rig::pose`: the hips, the hands and the feet as the
/// install's, everything between hung and turned and bent at the outfit's
/// own lengths).
///
/// What the game squeezes along the view in first person (the arms, with
/// the weapon) is taken out of that squeeze first and put back into it
/// after: the rig's lengths are true lengths.
pub fn see_outfit(
    on: Res<OutfitOn>,
    skeletons: Option<Res<OutfitSkeletons>>,
    weapon: Option<Res<WeaponRig>>,
    helper: TransformHelper,
    mut pose: ResMut<OutfitPose>,
) {
    use bevy::math::Affine3A;
    use trp_data::outfit::Move;
    pose.0.clear();
    let (Some(item), Some(skeletons), Some(weapon)) = (&on.item, skeletons, weapon) else { return };
    let Ok(root) = helper.compute_global_transform(weapon.root) else { return };
    let (_, turn, at) = root.to_scale_rotation_translation();
    let rigid = Affine3A::from_rotation_translation(turn, at);
    let unsqueezed = rigid * root.affine().inverse();
    // The body's, and the arms' for first person where it has those.
    for skeleton in skeletons.0.iter().filter(|skeleton| &skeleton.item == item) {
        let mut theirs = Vec::with_capacity(skeleton.theirs.len());
        for ((entity, rest), squeezed) in skeleton.theirs.iter().zip(&skeleton.their_rest).zip(&skeleton.squeezed) {
            let Ok(now) = helper.compute_global_transform(*entity) else { return };
            let moved = if *squeezed { unsqueezed * now.affine() * *rest } else { now.affine() * *rest };
            theirs.push(Move { axes: [moved.matrix3.x_axis.to_array(), moved.matrix3.y_axis.to_array(), moved.matrix3.z_axis.to_array()], shift: moved.translation.to_array() });
        }
        for (entity, moved) in skeleton.ours.iter().zip(skeleton.rig.pose(&theirs)) {
            let turn = Quat::from_mat3(&Mat3::from_cols(moved.axes[0].into(), moved.axes[1].into(), moved.axes[2].into())).normalize();
            pose.0.push((*entity, Transform { translation: moved.shift.into(), rotation: turn, scale: Vec3::ONE }));
        }
        // And the root's own place and turn, undone, for the squeezed
        // bones.
        pose.0.push((skeleton.unsqueeze, Transform { translation: turn.inverse() * -at, rotation: turn.inverse(), scale: Vec3::ONE }));
    }
}

/// Puts the outfit's bones there.
pub fn pose_outfit(pose: Res<OutfitPose>, mut transforms: Query<&mut Transform>) {
    for (entity, place) in &pose.0 {
        if let Ok(mut transform) = transforms.get_mut(*entity) {
            *transform = *place;
        }
    }
}

/// Shows the upper body from outside and leaves it a shadow in first
/// person; and the other way about for the hands as the player sees them,
/// which the game switches off from outside (`UpdatePlayerRenders`: the
/// hands' renderers are on in first person only, and the upper body's own
/// sleeves and hands are what an outside view has). Without a body drawn
/// there is nothing to take their place, and they stay.
///
/// With the options' `FIRST-PERSON BODY` off, none of it is drawn for the
/// character's own eyes: neither the legs, nor the upper body's shadow,
/// nor either kind of arms (settings spec, "Features").
pub fn show_upper_body(
    view: Res<View>,
    rig: Option<Res<BodyRig>>,
    outfit: Res<OutfitOn>,
    features: Res<crate::options_view::Features>,
    mut pieces: Query<(&mut RenderLayers, Option<&mut crate::shadow_only::ShadowOnly>), (Or<(With<UpperBody>, With<LowerBody>)>, EvenParked)>,
    mut hands: Query<&mut Visibility, With<FirstPersonHands>>,
    mut outfit_arms: Query<(&OutfitArms, &mut Visibility), Without<FirstPersonHands>>,
) {
    let unseen = !features.first_person_body && *view == View::FirstPerson;
    // An outfit's own arms for first person: there, while it is on, and
    // while the options have them (`OutfitOn::arms`).
    // (`EFT_OUTFIT_ARMS=always` shows them in every view and leaves the
    // upper body a shadow in every view, to look at them from outside:
    // with `--pov first --view side`, the rig as the character's own eyes
    // have it, seen from beside.)
    let always = std::env::var("EFT_OUTFIT_ARMS").is_ok_and(|value| value == "always") && outfit.arms;
    for (arms, mut visibility) in &mut outfit_arms {
        let on = (always || *view == View::FirstPerson) && outfit.arms && outfit.item.as_deref() == Some(arms.item.as_str());
        let shown = if on && !unseen { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != shown {
            *visibility = shown;
        }
    }
    // The upper body a shadow alone for these eyes; all of it nowhere with
    // the switch off.
    let shadow_only = *view == View::FirstPerson || always;
    for (mut layers, upper) in &mut pieces {
        let layer = if unseen { NOWHERE } else { SEEN };
        if *layers != RenderLayers::layer(layer) {
            *layers = RenderLayers::layer(layer);
        }
        if let Some(mut upper) = upper {
            let wish = crate::shadow_only::ShadowOnly(shadow_only && !unseen);
            if *upper != wish {
                *upper = wish;
            }
        }
    }
    // The install's own arms: there unless the outfit that is on has arms
    // of its own for these eyes. An outfit without them leaves these.
    // (`EFT_OUTFIT_ARMS=both` draws them as well as the outfit's, one
    // through the other, to see where the outfit's skin lies inside the
    // install's and where outside it.)
    let both = std::env::var("EFT_OUTFIT_ARMS").is_ok_and(|value| value == "both");
    let no_upper_body = pieces.iter().all(|(_, upper)| upper.is_none());
    let shown = if !unseen && (!outfit.arms || both) && (*view == View::FirstPerson || rig.is_none() || no_upper_body) {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut hands {
        if *visibility != shown {
            *visibility = shown;
        }
    }
}

/// Puts the skeleton where the body's animation has it, drawn down onto
/// the ground with the options' `GROUNDED FEET` (`feet_down`).
pub fn pose_body(controller: Res<Controller>, rig: Option<Res<BodyRig>>, down: Res<crate::feet_down::FeetDown>, mut transforms: Query<&mut Transform>) {
    let Some(rig) = rig else { return };
    let Some(pose) = controller.body_shown() else { return };
    if let Ok(mut root) = transforms.get_mut(rig.root) {
        *root = Transform { translation: pose.at - Vec3::Y * down.by, rotation: pose.turn, scale: Vec3::ONE };
    }
    for (entity, (position, rotation)) in rig.bones.iter().zip(&pose.bones) {
        if let Ok(mut transform) = transforms.get_mut(*entity) {
            *transform = convert::transform(*position, *rotation, [1.0; 3]);
        }
    }
    if let Some((bone, kept)) = down.spine {
        if let Some(mut transform) = rig.bones.get(bone).and_then(|entity| transforms.get_mut(*entity).ok()) {
            *transform = kept;
        }
    }
}
