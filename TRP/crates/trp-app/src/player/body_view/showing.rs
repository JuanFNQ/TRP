//! Which body and suit is drawn, and the outfit, torso, worn things and head parts shown.

use super::*;

impl BodyParts {
    /// A part's levels of detail as a mesh of it is tagged with
    /// (`lod_view::LodPiece`, placed by the mesh's own entity: the
    /// character's root, where a skinned piece hangs), where it has them.
    pub fn lod_piece(&self, part: usize) -> Option<crate::assets::lod_view::LodPiece> {
        let (group, levels) = self.lods.get(part)?.as_ref()?;
        Some(crate::assets::lod_view::LodPiece { group: group.clone(), anchor: None, levels: *levels })
    }

    /// The customization id of the upper body the character wears, which an
    /// armband's meshes go by (`PlayerBody.BodyCustomizationId`): its upper
    /// suit's body where the clothing names one, else the first id of the
    /// body it began in (one bundle of the install has two ids: the Black
    /// Knight's top, whose boss's own id has no armband set and is first).
    /// `PROVISIONAL`: a body known by its bundle alone (`--body`, a dummy's
    /// look) taken as its first id; the game always knows the id.
    pub fn character_body_id(&self, clothing: Option<&crate::player::clothing::Clothing>) -> Option<String> {
        let suited = clothing.and_then(|clothing| clothing.worn[0].and_then(|at| clothing.suits.get(at))).and_then(|suit| suit.body_id.clone());
        suited.or_else(|| self.body_id_of(&self.look_keys[LOOK_UPPER]))
    }

    /// The same of a dummy: its own upper body's, else the character's as
    /// it began (the copy it wears; the clothing chosen is not put on it).
    pub fn dummy_body_id(&self, which: usize) -> Option<String> {
        let own = self.dummy_has.get(which).is_some_and(|has| has[LOOK_UPPER]);
        match self.dummy_keys.get(which).filter(|_| own) {
            Some(keys) => self.body_id_of(&keys[LOOK_UPPER]),
            None => self.body_id_of(&self.look_keys[LOOK_UPPER]),
        }
    }

    fn body_id_of(&self, bundle: &str) -> Option<String> {
        self.body_ids.get(bundle).and_then(|ids| ids.first()).cloned()
    }

    /// The bits the character's own look adds to the head things' fittings
    /// (`PlayerBody.SetSkin`'s changers): its upper suit's and its legs' as
    /// the clothing chosen has them on (a body made with another suit is a
    /// body made anew, `TacticalClothingView.UpdatePreview`: only its own
    /// count), else those it began in; and its head's.
    pub fn character_suit_bits(&self, clothing: Option<&crate::player::clothing::Clothing>) -> u8 {
        let chosen = |role: usize| clothing.and_then(|clothing| clothing.bits[role]);
        chosen(LOOK_UPPER).unwrap_or(self.suit_bits[LOOK_UPPER]) | chosen(LOOK_LOWER).unwrap_or(self.suit_bits[LOOK_LOWER]) | self.suit_bits[LOOK_HEAD]
    }

    /// The same of a dummy: each part of its own look's, and the
    /// character's starting part's where it has none of its own (the copy
    /// it wears; the clothing chosen is not put on the dummies).
    pub fn dummy_suit_bits(&self, which: usize) -> u8 {
        let has = self.dummy_has.get(which).copied().unwrap_or_default();
        let own = self.dummy_bits.get(which).copied().unwrap_or_default();
        (0..3).fold(0, |bits, role| bits | if has[role] { own[role] } else { self.suit_bits[role] })
    }

    /// The torso's mesh the game draws for what the character has on
    /// (`TorsoSkin.ApplySkin`: the slots of the body armour, the tactical
    /// vest and the backpack, and whether the armour is worn like a vest).
    pub fn torso_for(&self, carried: &Carried) -> trp_data::character::arms::Torso {
        self.torso_of(&carried.equipment.item)
    }

    /// The same for any character's equipment (a dummy's).
    pub fn torso_of(&self, equipment: &trp_sim::items::inventory::Item) -> trp_data::character::arms::Torso {
        let worn = |slot: &str| equipment.in_slot(slot);
        let armour = worn("ArmorVest");
        let vestlike = armour.is_some_and(|armour| self.vestlike.contains(&armour.kind.template));
        trp_data::character::arms::TorsoMeshes::drawn(armour.is_some(), vestlike, worn("TacticalVest").is_some(), worn("Backpack").is_some())
    }

    /// What a thing worn over armour or a vest takes its mesh by, of the
    /// things on (`show_worn`).
    pub fn under(&self, on: &[&str]) -> Under {
        let wearing = |listed: &[String]| listed.iter().any(|item| on.contains(&item.as_str()));
        Under { armour: wearing(&self.armour), vest: wearing(&self.vests) }
    }

    /// The first outfit not the install's among the things on that has a
    /// body or one of the install's tops to draw (`show_outfit`'s rule, its
    /// arms for the character's own eyes aside): its item, and whether it
    /// draws a body, or else a top.
    pub fn outfit_drawn(&self, on: &[&str]) -> Option<(&str, bool, bool)> {
        self.outfits
            .iter()
            .map(|(item, outfit)| (item.as_str(), self.drawn_of(item, outfit)))
            .filter(|(_, (body, top, _))| *body || *top)
            .find(|(item, _)| on.contains(item))
            .map(|(item, (body, top, _))| (item, body, top && !body))
    }

    /// What an outfit has to draw in place of the character's own: a
    /// body of its own, one of the install's tops, and arms for the
    /// character's own eyes (its own, or the install's hands).
    fn drawn_of(&self, item: &str, outfit: &trp_data::character::outfit::Outfit) -> (bool, bool, bool) {
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
/// options** (`CUSTOM FIRST PERSON HANDS`, `Features::custom_hands`):
/// off, the hands do not change with what is worn, and
/// are the install's default arms.
pub fn show_outfit(
    carried: Res<Carried>,
    parts: Res<BodyParts>,
    features: Res<crate::screens::options_view::Features>,
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
    // (Armour read later may be worn like a vest: `worn_reads`.)
    if !carried.is_changed() && !on.is_changed() && !parts.is_changed() && made.is_empty() {
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
    let under = parts.under(&on);
    for (piece, mut visibility) in &mut pieces {
        let shown = on.contains(&piece.item.as_str()) && (piece.over.is_empty() || piece.over.contains(&under));
        let wanted = if shown { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// Each thing of that template worn in a slot the game draws, by the
/// character or by a dummy.
pub(super) fn worn_everywhere<'a>(carried: &'a Carried, template: &'a str) -> impl Iterator<Item = &'a trp_sim::items::inventory::Item> + 'a {
    let bodies = (0..carried.bodies.len()).filter_map(|which| carried.body(which));
    std::iter::once(&carried.equipment.item)
        .chain(bodies)
        .flat_map(|gear| trp_sim::items::inventory::WORN.iter().filter_map(|slot| gear.in_slot(slot)))
        .filter(move |item| item.kind.template == template)
}

/// The thing worn in a slot the game draws that is of that template.
pub(super) fn worn_of<'a>(carried: &'a Carried, template: &str) -> Option<&'a trp_sim::items::inventory::Item> {
    trp_sim::items::inventory::WORN.iter().filter_map(|slot| carried.equipment.worn(slot)).find(|item| item.kind.template == template)
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
