//! Fitting head things, hoods and armbands, and turning helmet hinges.

use super::*;

/// Whose things a piece fitted to what else is worn goes by
/// (`fit_head_things`): the character's (its body, in every view), the
/// gear page's figure's (the character's again, its drawing asked for when
/// a piece changes), the corpse's (the character's as it died), or a
/// dummy's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wearer {
    Character,
    Figure,
    Corpse,
    Dummy(usize),
}

/// A piece of a thing on the head whose mesh has a second form, fitted to
/// what else its wearer has on (`trp_data::dress::Fitting`; body animator
/// spec, "The head's things fitted to one another"): whose things it goes
/// by, the kind of thing it is (`trp_data::dress::fit`), its two meshes
/// (the first as made, the prefab's), and a skinned piece's two meshes'
/// bind poses.
#[derive(Component, Clone)]
pub struct Fitted {
    pub wearer: Wearer,
    pub kind: u8,
    pub meshes: [Handle<Mesh>; 2],
    pub poses: Option<[Handle<SkinnedMeshInverseBindposes>; 2]>,
}

impl Fitted {
    /// The same piece of another wearer's.
    pub(crate) fn of(&self, wearer: Wearer) -> Self {
        Self { wearer, ..self.clone() }
    }
}

/// The meshes of a piece's second form for each of its first form's
/// submeshes, as the engine draws a renderer whose mesh has another count
/// of them (Unity's manual, `Mesh.subMeshCount`): each material its own
/// submesh, the last submesh again for a material beyond them, a submesh
/// beyond the materials not drawn. (Two drawn meshes of the install's
/// change their count: the Grom glasses' and a ULACH's, `probe fittings`.)
/// `PROVISIONAL`: the engine's documented rule, not seen on those two.
pub(crate) fn second_form(base: usize, custom: &[Handle<Mesh>]) -> Vec<Handle<Mesh>> {
    (0..base).filter_map(|slot| custom.get(slot.min(custom.len().saturating_sub(1))).cloned()).collect()
}

/// The hood's bit for what an equipment holds and the things `--wear`
/// named (`trp_data::dress::hood_up`, `fit::HOOD_UP`): anything in the
/// headwear slot, and the face cover's conflicting items and the slots it
/// blocks (`Kind::conflicting`, `Kind::blocks`). A thing `--wear` named
/// counts in the slot of its class.
pub(crate) fn hood_bit(equipment: &trp_sim::items::inventory::Item, fixed: &[&trp_sim::items::inventory::Kind]) -> u8 {
    let headwear = equipment.in_slot("Headwear").is_some() || fixed.iter().any(|kind| of_class(kind, "Headwear"));
    let cover = face_cover(equipment, fixed);
    let up = trp_data::character::dress::hood_up(headwear, cover.map(|kind| (kind.conflicting.as_slice(), kind.blocks.as_slice())));
    if up {
        trp_data::character::dress::fit::HOOD_UP
    } else {
        0
    }
}

fn of_class(kind: &trp_sim::items::inventory::Kind, name: &str) -> bool {
    kind.class_names.iter().any(|class| class == name)
}

/// What is in an equipment's face cover's slot, or the face cover `--wear`
/// named (it counts in the slot of its class).
fn face_cover<'a>(equipment: &'a trp_sim::items::inventory::Item, fixed: &[&'a trp_sim::items::inventory::Kind]) -> Option<&'a trp_sim::items::inventory::Kind> {
    equipment.in_slot("FaceCover").map(|item| &*item.kind).or_else(|| fixed.iter().copied().find(|kind| of_class(kind, "FaceCover")))
}

/// The head's own bit for what an equipment holds and the things `--wear`
/// named (`HeadSkin.Init`, `fit::FACE_COVERED`; body animator spec, "The
/// head's own mesh under a face cover"): the face cover's slot holds a
/// thing whose template is a face cover's and not a half mask. Nothing
/// else counts: not headwear, not the look.
pub(crate) fn face_covered_bit(equipment: &trp_sim::items::inventory::Item, fixed: &[&trp_sim::items::inventory::Kind]) -> u8 {
    if face_cover(equipment, fixed).is_some_and(|kind| of_class(kind, "FaceCover") && !kind.half_mask) {
        trp_data::character::dress::fit::FACE_COVERED
    } else {
        0
    }
}

/// The bits last given to the pieces (`fit_head_things`), and the kinds of
/// the things `--wear` named, made once.
#[derive(Default)]
pub struct FittingState {
    applied: Option<(u8, u8, Vec<u8>)>,
    fixed: Option<Vec<std::sync::Arc<trp_sim::items::inventory::Kind>>>,
}

/// Draws each thing on the head that has a second mesh by what its wearer
/// has on (body animator spec, "The head's things fitted to one another";
/// `PlayerClippingRules.CalculateRules`, `CustomItem.ProceedItemChanged`):
/// the bits of the things in the four slots (`trp_sim::inventory::
/// fitting_bits_of`, with what `--wear` named for the character) and of
/// the wearer's look (`BodyParts::character_suit_bits`, `dummy_suit_bits`),
/// the second mesh where the piece's kind's bit is among them. The game
/// works it out as a thing it listens to comes or goes and as a body is
/// made (with a suit chosen, made anew); here the bits are worked out every
/// frame and the pieces set when they move or a piece is made: the
/// character's body (first person, where its head things are drawn at all,
/// and watched), the gear page's and OVERALL's figure, the corpse (as the
/// character died), each dummy by its own, standing or dead. A part's hood
/// rides on the same bits (`fit::HOOD_UP`, `hood_bit`: `PlayerBody.
/// ValidateHoodedDress`, worked out as the headwear or face cover changes):
/// its hooded form is the second. So does a head's own mesh for under a
/// face cover (`fit::FACE_COVERED`, `face_covered_bit`: `HeadSkin.Init`,
/// worked out as the face cover's slot changes): the second while a face
/// cover is worn. Off (`HEAD FITTING`), it does nothing, and every piece
/// keeps its first mesh (the hood down, the head bare).
#[allow(clippy::too_many_arguments)]
pub fn fit_head_things(
    features: Res<crate::screens::options_view::Features>,
    game: Res<GameData>,
    carried: Res<Carried>,
    parts: Res<BodyParts>,
    clothing: Option<Res<crate::player::clothing::Clothing>>,
    dying: Option<Res<crate::player::death::Dying>>,
    made: Query<(), (Added<Fitted>, EvenParked)>,
    mut pieces: Query<(&Fitted, &mut Mesh3d, Option<&mut SkinnedMesh>), EvenParked>,
    (mut figure, mut state): (Option<ResMut<crate::screens::figure_view::Figure>>, Local<FittingState>),
) {
    if !features.head_fitting || !parts.fitting {
        return;
    }
    let state = &mut *state;
    let fixed = state.fixed.get_or_insert_with(|| carried.fixed_wear.iter().filter_map(|template| trp_sim::items::inventory::Kind::of(&game.db, template).ok().map(std::sync::Arc::new)).collect());
    let fixed: Vec<&trp_sim::items::inventory::Kind> = fixed.iter().map(|kind| &**kind).collect();
    let character = trp_sim::items::inventory::fitting_bits_of(&carried.equipment.item, &fixed) | parts.character_suit_bits(clothing.as_deref()) | hood_bit(&carried.equipment.item, &fixed) | face_covered_bit(&carried.equipment.item, &fixed);
    // The corpse keeps what the character had on as it died.
    let dead = dying.as_ref().is_some_and(|dying| dying.since().is_some());
    let corpse = match (&state.applied, dead) {
        (Some((_, corpse, _)), true) => *corpse,
        _ => character,
    };
    let dummies: Vec<u8> = (0..crate::world::dummy::places().len()).map(|which| carried.body(which).map_or(0, |gear| trp_sim::items::inventory::fitting_bits_of(gear, &[]) | hood_bit(gear, &[]) | face_covered_bit(gear, &[])) | parts.dummy_suit_bits(which)).collect();
    let now = (character, corpse, dummies);
    if made.is_empty() && state.applied.as_ref() == Some(&now) {
        return;
    }
    let (mut figure_moved, mut set) = (false, 0);
    for (fitted, mut mesh, skinned) in &mut pieces {
        let bits = match fitted.wearer {
            Wearer::Character | Wearer::Figure => now.0,
            Wearer::Corpse => now.1,
            Wearer::Dummy(which) => now.2.get(which).copied().unwrap_or(0),
        };
        let form = usize::from(bits & fitted.kind == fitted.kind);
        if mesh.0 == fitted.meshes[form] {
            continue;
        }
        mesh.0 = fitted.meshes[form].clone();
        if let (Some(poses), Some(mut skinned)) = (&fitted.poses, skinned) {
            skinned.inverse_bindposes = poses[form].clone();
        }
        figure_moved |= fitted.wearer == Wearer::Figure;
        set += 1;
    }
    if state.applied.as_ref() != Some(&now) {
        info!("the head things' fittings by the bits of what is worn: the character's {}, the corpse's {}, the dummies' {:?}; {set} pieces set", now.0, now.1, now.2);
    }
    if figure_moved {
        if let Some(figure) = figure.as_mut() {
            figure.ask();
        }
    }
    state.applied = Some(now);
}

/// An armband's meshes for every upper body (`ArmBandView`'s sets; body
/// animator spec, "The armband, the hood and a suit's holster"): each set's
/// customization id with the place of its mesh among `forms`, in the
/// view's order (the first is the one a body with no set takes), and each
/// mesh as drawn: a piece for each of the first's submeshes (`second_form`'s
/// pairing), and its bind poses. `PROVISIONAL`: 338 of the install's
/// armband meshes have a bind pose fewer than their skin's five bones (no
/// point moved by the fifth); each is drawn on the first bones by number,
/// as the renderer's bone list and the mesh's poses pair here, the engine's
/// handling of the shorter list not read.
pub struct BandMeshes {
    pub sets: Vec<(String, usize)>,
    pub forms: Vec<(Vec<Handle<Mesh>>, Handle<SkinnedMeshInverseBindposes>)>,
}

impl BandMeshes {
    /// Made of an armband skin's meshes for every upper body, each mesh
    /// made once (`made`: its submeshes and bind poses, or none where it
    /// cannot be had, which leaves its sets out).
    pub(crate) fn of(sets: &[(String, trp_data::unity::ObjRef)], mut made: impl FnMut(trp_data::unity::ObjRef) -> Option<(Vec<Handle<Mesh>>, Handle<SkinnedMeshInverseBindposes>)>) -> Self {
        let mut meshes: Vec<trp_data::unity::ObjRef> = Vec::new();
        let mut forms = Vec::new();
        let mut placed = Vec::new();
        for (id, mesh) in sets {
            let at = match meshes.iter().position(|known| known == mesh) {
                Some(at) => Some(at),
                None => made(*mesh).map(|form| {
                    meshes.push(*mesh);
                    forms.push(form);
                    forms.len() - 1
                }),
            };
            if let Some(at) = at {
                placed.push((id.clone(), at));
            }
        }
        // Each form's pieces paired with the first's.
        let base = forms.first().map_or(0, |(pieces, _): &(Vec<Handle<Mesh>>, _)| pieces.len());
        let forms = forms.into_iter().map(|(pieces, poses)| (second_form(base, &pieces), poses)).collect();
        Self { sets: placed, forms }
    }

    /// The form for an upper body by its id (`trp_data::dress::ArmBand::
    /// meshes_for`): its set's, else the first set's.
    pub fn form_for(&self, id: Option<&str>) -> Option<&(Vec<Handle<Mesh>>, Handle<SkinnedMeshInverseBindposes>)> {
        let own = id.and_then(|id| self.sets.iter().find(|(of, _)| of == id));
        own.or(self.sets.first()).and_then(|(_, at)| self.forms.get(*at))
    }

    /// Every mesh it may draw, to be waited for on the graphics card.
    pub fn meshes(&self) -> impl Iterator<Item = &Handle<Mesh>> {
        self.forms.iter().flat_map(|(pieces, _)| pieces)
    }
}

/// A piece of an armband's skin (`BandMeshes`): whose body it goes by, and
/// which of the first form's submeshes it is.
#[derive(Component, Clone)]
pub struct Banded {
    pub wearer: Wearer,
    pub slot: usize,
    pub meshes: std::sync::Arc<BandMeshes>,
}

impl Banded {
    /// The same piece of another wearer's.
    pub(crate) fn of(&self, wearer: Wearer) -> Self {
        Self { wearer, ..self.clone() }
    }
}

/// Gives each armband piece the mesh its wearer's upper body takes
/// (`ArmBandView.SetBodyHandler`, bound to the body's
/// `BodyCustomizationId`; body animator spec, "The armband, the hood and a
/// suit's holster"): the character's (its body in every view, the gear
/// page's and OVERALL's figure, its corpse, which takes a suit chosen with
/// it: `clothing`) by its upper suit, a dummy's by its own look. Set as a
/// piece is made and as a body changes. Off (`ARMBAND`), there is none.
#[allow(clippy::type_complexity)]
pub fn fit_armbands(
    features: Res<crate::screens::options_view::Features>,
    parts: Res<BodyParts>,
    clothing: Option<Res<crate::player::clothing::Clothing>>,
    made: Query<(), (Added<Banded>, EvenParked)>,
    mut pieces: Query<(&Banded, &mut Mesh3d, Option<&mut SkinnedMesh>), EvenParked>,
    mut figure: Option<ResMut<crate::screens::figure_view::Figure>>,
    mut applied: Local<Option<(Option<String>, Vec<Option<String>>)>>,
) {
    if !features.armband || !parts.armband {
        return;
    }
    let now = (parts.character_body_id(clothing.as_deref()), (0..crate::world::dummy::places().len()).map(|which| parts.dummy_body_id(which)).collect::<Vec<_>>());
    if made.is_empty() && applied.as_ref() == Some(&now) {
        return;
    }
    let (mut figure_moved, mut set) = (false, 0);
    for (banded, mut mesh, skinned) in &mut pieces {
        let id = match banded.wearer {
            Wearer::Character | Wearer::Figure | Wearer::Corpse => now.0.as_deref(),
            Wearer::Dummy(which) => now.1.get(which).and_then(Option::as_deref),
        };
        let Some((form, poses)) = banded.meshes.form_for(id) else { continue };
        let Some(wanted) = form.get(banded.slot) else { continue };
        if mesh.0 == *wanted {
            continue;
        }
        mesh.0 = wanted.clone();
        if let Some(mut skinned) = skinned {
            skinned.inverse_bindposes = poses.clone();
        }
        figure_moved |= banded.wearer == Wearer::Figure;
        set += 1;
    }
    if applied.as_ref() != Some(&now) {
        info!("the armbands by the upper body worn: the character's {:?}, the dummies' {:?}; {set} pieces set", now.0, now.1);
    }
    if figure_moved {
        if let Some(figure) = figure.as_mut() {
            figure.ask();
        }
    }
    *applied = Some(now);
}

/// Turns each hinge as its face shield stands (`HelmetDress`; body animator
/// spec, "A worn thing's parts and a helmet's hinge"), by the thing whose
/// model carries it (the helmet; or a face shield, its own), by the
/// simulation's rule (`trp_sim::visor::ShieldHinge`: set at once as that
/// thing is put on or fitted; while it is worn, the filter moved on and
/// then a change of its face shield or switch heard, as the game's frame
/// has them); a value given turns the hinge to the engine's blend of its
/// two turns (`Quaternion.Lerp`). A turn of the gear page's figure asks
/// for its drawing.
pub fn turn_hinges(carried: Res<Carried>, time: Res<Time>, mut hinges: Query<(&mut HelmetHinge, &mut Transform), EvenParked>, mut figure: Option<ResMut<crate::screens::figure_view::Figure>>) {
    for (mut hinge, mut transform) in &mut hinges {
        let owner = worn_of(&carried, &hinge.item).and_then(|worn| hinge.owner.iter().try_fold(worn, |at, (slot, template)| at.in_slot(slot).filter(|held| held.kind.template == *template)));
        // (Not worn, only its last sight is let go of: nothing to turn.)
        if owner.is_none() && !hinge.hinge.worn() {
            continue;
        }
        let hinge = &mut *hinge;
        let given = hinge.hinge.frame(owner, time.delta_secs());
        if let Some(value) = given {
            transform.rotation = convert::rotation(trp_sim::base::math::quat_lerp(hinge.off, hinge.on, value));
            if hinge.figure {
                if let Some(figure) = figure.as_mut() {
                    figure.ask();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trp_data::character::dress::fit;
    use trp_sim::items::inventory::{Equipment, Item, Kind};

    /// The plain balaclava, the USEC cap and the ComTac II.
    const BALACLAVA: &str = "572b7f1624597762ae139822";
    const USEC_CAP: &str = "5aa2a7e8e5b5b00016327c16";
    const COMTAC_II: &str = "5645bcc04bdc2d363b8b4572";

    /// The head's own bit (`HeadSkin.Init`): a face cover in its slot gives
    /// it, and one `--wear` named; bare, or a cap and a headset alone, do
    /// not. It is no bit of the game's kinds, of a look's changer (1 and
    /// 32 in the install) or the hood's.
    #[test]
    fn a_face_cover_worn_gives_the_head_its_bit() {
        let Ok(install) = trp_data::Install::locate() else {
            eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
            return;
        };
        let db = trp_data::Database::load(&install).expect("database");
        let kind = |template: &str| std::sync::Arc::new(Kind::of(&db, template).expect("its kind"));
        let wearing = |worn: &[(&str, &str)]| {
            let mut equipment = Equipment::empty(&db).expect("the equipment");
            for (slot, template) in worn {
                let at = equipment.item.slot_index(slot).expect("the slot");
                equipment.item.slots[at] = Some(Item::new(kind(template)));
            }
            equipment
        };
        assert_eq!(face_covered_bit(&wearing(&[]).item, &[]), 0);
        assert_eq!(face_covered_bit(&wearing(&[("FaceCover", BALACLAVA)]).item, &[]), fit::FACE_COVERED);
        assert_eq!(face_covered_bit(&wearing(&[("Headwear", USEC_CAP), ("Earpiece", COMTAC_II)]).item, &[]), 0);
        let (named, cap) = (kind(BALACLAVA), kind(USEC_CAP));
        assert_eq!(face_covered_bit(&wearing(&[]).item, &[&*named]), fit::FACE_COVERED);
        assert_eq!(face_covered_bit(&wearing(&[]).item, &[&*cap]), 0);
        // A balaclava leaves a hood up (the hood's own rule), and the
        // head's bit is none of the others.
        assert_eq!(hood_bit(&wearing(&[("FaceCover", BALACLAVA)]).item, &[]), fit::HOOD_UP);
        assert_eq!(fit::FACE_COVERED & (fit::KINDS.iter().fold(32, |all, kind| all | kind) | fit::HOOD_UP), 0);
    }

    /// The default BEAR head and a USEC head, the character's and a
    /// dummy's own.
    const BEAR_HEAD: &str = "assets/content/characters/character/prefabs/bear_head_0.bundle";
    const USEC_HEAD: &str = "assets/content/characters/character/prefabs/usec_head_1.bundle";

    /// `BodyParts::load` gives the head's part a second form for under a
    /// face cover (`HeadSkin`'s `Data.FaceCover`, `fit::FACE_COVERED`), its
    /// first the part's own mesh: the character's head and a dummy's own,
    /// every level read. With `HEAD FITTING` off, none.
    #[test]
    fn the_head_s_part_is_given_its_mesh_for_under_a_face_cover() {
        let Ok(install) = trp_data::Install::locate() else {
            eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
            return;
        };
        let db = trp_data::Database::load(&install).expect("database");
        let mut unity = UnityAssets::new(install).expect("manifest");
        let looks = [[String::new(), String::new(), USEC_HEAD.to_owned()]];
        let load = |unity: &mut UnityAssets, head_fitting: bool| BodyParts::load(unity, &db, &["", "", BEAR_HEAD], &[], &HashSet::new(), false, &[], &looks, head_fitting, false);
        let parts = load(&mut unity, true);
        let covered = |parts: &BodyParts, at: usize| parts.fittings.iter().find(|(of, fitting)| *of == at && fitting.kind == fit::FACE_COVERED).map(|(_, fitting)| *fitting);
        let character = parts.head.expect("the character's head");
        let dummy: Vec<usize> = (0..parts.parts.len()).filter(|at| parts.dummy_own[*at] == Some((0, LOOK_HEAD))).collect();
        assert!(!dummy.is_empty(), "the dummy's own head");
        // The character's head's levels follow it, as the dummy's own do.
        let heads = (character..parts.parts.len()).take_while(|at| parts.fullest[*at] == character && parts.dummy_own[*at].is_none());
        for at in heads.chain(dummy.iter().copied()) {
            let fitting = covered(&parts, at).unwrap_or_else(|| panic!("part {at}: no mesh for under a face cover"));
            assert_eq!(fitting.base, parts.parts[at].mesh, "part {at}: its first form is its own mesh");
            assert_ne!(fitting.custom, fitting.base, "part {at}");
        }
        let off = load(&mut unity, false);
        assert!(off.fittings.iter().all(|(_, fitting)| fitting.kind != fit::FACE_COVERED), "HEAD FITTING off: none");
    }
}
