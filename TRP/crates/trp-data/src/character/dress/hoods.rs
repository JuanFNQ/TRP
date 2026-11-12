//! Armbands and the hood with its levels.

use super::*;

/// An armband as worn (`ArmBandView`, a `Dress` on the worn half of its
/// `DressItem`; body animator spec, "The armband, the hood and a suit's
/// holster"): its skins (`_main`), each read as a piece of clothing (its
/// renderer saves no mesh: the game gives it one before it is drawn; here
/// the first set's), and for each upper body by its customization id a
/// mesh for each skin, as the view lists them (`_customizationIdPair`).
pub struct ArmBand {
    pub skins: Vec<Arms>,
    pub sets: Vec<(String, Vec<ObjRef>)>,
}

impl ArmBand {
    /// The meshes for an upper body by its customization id
    /// (`SetBodyHandler`, bound to the body's `BodyCustomizationId`): its
    /// set, else the first set listed (the game's fallback for an id the
    /// view has no set for, `_dict.Values.First()`, with an error in its
    /// log). None for a view that lists no set.
    pub fn meshes_for(&self, id: Option<&str>) -> Option<&[ObjRef]> {
        let own = id.and_then(|id| self.sets.iter().find(|(of, _)| of == id));
        own.or(self.sets.first()).map(|(_, meshes)| meshes.as_slice())
    }

    /// The place among `sets` of the set `meshes_for` takes.
    pub fn set_for(&self, id: Option<&str>) -> Option<usize> {
        id.and_then(|id| self.sets.iter().position(|(of, _)| of == id)).or((!self.sets.is_empty()).then_some(0))
    }
}

/// Reads an armband's view (`ArmBandView`).
pub(super) fn arm_band(assets: &Assets, view: ObjRef) -> Result<ArmBand> {
    let value = assets.read(view)?;
    let pointed = |v: &Value| v.as_pptr().and_then(|p| assets.resolve_loaded(view.file, p));
    let mut sets = Vec::new();
    for pair in value.req("_customizationIdPair")?.as_array().unwrap_or(&[]) {
        let id = pair.get("CustomizationId").and_then(Value::as_str).ok_or_else(|| anyhow!("an armband's pair names no customization"))?.to_owned();
        let meshes: Vec<ObjRef> = pair.get("Meshes").and_then(Value::as_array).unwrap_or(&[]).iter().filter_map(pointed).collect();
        sets.push((id, meshes));
    }
    let first = |skin: usize| sets.first().and_then(|(_, meshes): &(String, Vec<ObjRef>)| meshes.get(skin).copied());
    let skins = value.req("_main")?.as_array().unwrap_or(&[]).iter().filter_map(pointed).enumerate().map(|(at, skin)| arms::read_skin_with(assets, skin, first(at))).collect::<Result<Vec<_>>>()?;
    Ok(ArmBand { skins, sets })
}

/// A body part's hooded form (`HoodedDress` beside its `LoddedSkin`: two
/// lists of objects, `_hooded` switched on and `_notHooded` off as the hood
/// goes up, and back as it comes down; body animator spec, "The armband,
/// the hood and a suit's holster"): the fullest of the skins the part's
/// `LoddedSkin` lists whose object is among the hooded, read as a piece of
/// clothing (`arms::read_skin`). None where the part has no hood, or where
/// the skin `arms::load` takes (the list's first) is not among those the
/// hood keeps on while down.
pub fn hood(assets: &mut Assets, bundle_key: &str) -> Result<Option<Arms>> {
    let files = assets.load(bundle_key)?;
    let scripts: Vec<ObjRef> = files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect();
    let of_class = |name: &str| scripts.iter().copied().find(|&obj| script_class(assets, obj).as_deref() == Some(name));
    let (Some(hooded), Some(levels)) = (of_class("HoodedDress"), of_class("LoddedSkin")) else { return Ok(None) };
    let value = assets.read(hooded)?;
    let objects = |name: &str| -> Vec<ObjRef> { value.get(name).and_then(Value::as_array).unwrap_or(&[]).iter().filter_map(|v| v.as_pptr().and_then(|p| assets.resolve_loaded(hooded.file, p))).collect() };
    let (up, down) = (objects("_hooded"), objects("_notHooded"));
    let listed: Vec<ObjRef> = assets.read(levels)?.req("_lods")?.as_array().unwrap_or(&[]).iter().filter_map(|skin| skin.as_pptr().and_then(|p| assets.resolve_loaded(levels.file, p))).collect();
    let object = |skin: ObjRef| game_object_of(assets, skin);
    if !listed.first().and_then(|&skin| object(skin)).is_some_and(|first| down.contains(&first)) {
        return Ok(None);
    }
    match listed.iter().copied().find(|&skin| object(skin).is_some_and(|at| up.contains(&at))) {
        Some(skin) => Ok(Some(arms::read_skin(assets, skin)?)),
        None => Ok(None),
    }
}

/// Every level's hooded form of a part with a hood, as `hood` reads the
/// fullest's: each skin its `LoddedSkin` lists whose object is among the
/// hooded, with the levels the bundle's group draws its renderer at (the
/// hood's form of the level that draws at those: `arms::load_levels`).
/// Empty where `hood` has none.
pub fn hood_levels(assets: &mut Assets, bundle_key: &str) -> Result<Vec<Arms>> {
    if hood(assets, bundle_key)?.is_none() {
        return Ok(Vec::new());
    }
    let files = assets.load(bundle_key)?;
    let scripts: Vec<ObjRef> = files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect();
    let of_class = |name: &str| scripts.iter().copied().find(|&obj| script_class(assets, obj).as_deref() == Some(name));
    let (Some(hooded), Some(levels)) = (of_class("HoodedDress"), of_class("LoddedSkin")) else { return Ok(Vec::new()) };
    let up: Vec<ObjRef> = assets.read(hooded)?.get("_hooded").and_then(Value::as_array).unwrap_or(&[]).iter().filter_map(|v| v.as_pptr().and_then(|p| assets.resolve_loaded(hooded.file, p))).collect();
    let listed: Vec<ObjRef> = assets.read(levels)?.req("_lods")?.as_array().unwrap_or(&[]).iter().filter_map(|skin| skin.as_pptr().and_then(|p| assets.resolve_loaded(levels.file, p))).collect();
    let groups = crate::drawing::lod::of_files(assets, &files);
    let mut all = Vec::new();
    for skin in listed.into_iter().filter(|&skin| game_object_of(assets, skin).is_some_and(|at| up.contains(&at))) {
        let mut form = arms::read_skin(assets, skin)?;
        if let Some(group) = groups.iter().find(|group| group.lists(form.renderer)) {
            form.lod = Some(crate::drawing::lod::SkinLod { group: group.clone(), levels: group.levels_of(form.renderer) });
        }
        all.push(form);
    }
    // TRP's measure (`lod::measured`): the part's own, kept by its group as
    // its levels were read (`arms::load_levels`, read first); unmeasured
    // else (its fullest at any distance), the hooded forms being only some
    // of the levels.
    if let Some(group) = all.iter().find_map(|form| form.lod.as_ref().map(|lod| lod.group.clone())) {
        let measured = std::sync::Arc::new(crate::drawing::lod::measured(Some(&*assets), &group, || None));
        for form in &mut all {
            if let Some(lod) = form.lod.as_mut().filter(|lod| std::sync::Arc::ptr_eq(&lod.group, &group)) {
                lod.group = measured.clone();
            }
        }
    }
    Ok(all)
}

/// Whether a hood is up for what its wearer has on (`PlayerBody.
/// ValidateHoodedDress`, run as the body is made and whenever the headwear
/// or the face cover slot's model changes): down under headwear; else up,
/// unless a face cover is worn that names conflicting items or blocks the
/// headwear slot (`SlotBlockerComponent.ConflictingSlotNames`). The face
/// cover by its template's conflicting items and the slots it blocks.
pub fn hood_up(headwear: bool, face_cover: Option<(&[String], &[String])>) -> bool {
    !headwear && !face_cover.is_some_and(|(conflicting, blocks)| !conflicting.is_empty() || blocks.iter().any(|slot| slot == "Headwear"))
}
