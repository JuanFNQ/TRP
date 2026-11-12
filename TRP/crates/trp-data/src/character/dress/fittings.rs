//! Fittings of a skin or a part, and a part's clipping.

use super::*;

/// A mesh of a thing worn on the head that has a second form, fitted to
/// what else the character wears (`CustomItem` on a plain mesh,
/// `CustomItemSkin` on a skinned one): the kind of thing it is (one of
/// `fit`'s, `_itemType`), the mesh the game draws while that kind's bit is
/// not among the character's (`Base`), and the one while it is (`Custom`).
/// Read only where both meshes are named and the kind is one of
/// `fit::KINDS`: otherwise the game leaves the mesh as the prefab saves it
/// (`SetIsCustom`, `Init`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fitting {
    pub kind: u8,
    pub base: ObjRef,
    pub custom: ObjRef,
}

impl Fitting {
    /// Whether the second mesh is drawn under these bits (`NeedCustom`:
    /// its kind's bit among them).
    pub fn custom_under(&self, bits: u8) -> bool {
        bits & self.kind == self.kind
    }

    /// A fitting's figures as its script holds them (`_data` or `Data`,
    /// `_itemType`), its meshes in the files loaded.
    pub(super) fn of(assets: &Assets, script: ObjRef, value: &Value, data: &str) -> Option<Self> {
        let data = value.get(data)?;
        let mesh = |name: &str| data.get(name).and_then(Value::as_pptr).filter(|p| !p.is_null()).and_then(|p| assets.resolve_loaded(script.file, p));
        let kind = u8::try_from(value.get("_itemType")?.as_i64()?).ok().filter(|kind| fit::KINDS.contains(kind))?;
        Some(Self { kind, base: mesh("Base")?, custom: mesh("Custom")? })
    }
}

/// The game object a component is on.
pub(super) fn game_object_of(assets: &Assets, component: ObjRef) -> Option<ObjRef> {
    let value = assets.read(component).ok()?;
    value.get("m_GameObject").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(component.file, p))
}

/// The fittings of a model on the head and the parts hung on it
/// (`CustomItem`): each object whose mesh has one, by its index in the
/// tree, with it. As the game starts them: a model's dresses are each
/// started as it is hung on the body (`PlayerBody.CreateAndParent`), and a
/// dress starts the fitting on each renderer it lists (`Dress.Init`); one
/// on a renderer no dress lists is never started. The mesh fitted is the
/// one the script names (`_meshFilter`), on its own object or another.
pub fn fittings(assets: &Assets, hierarchy: &Hierarchy) -> Vec<(usize, Fitting)> {
    let scripts = || hierarchy.nodes.iter().enumerate().flat_map(|(index, node)| node.scripts.iter().zip(&node.script_objects).map(move |(class, obj)| (index, class.as_str(), *obj)));
    // The objects whose renderers a dress lists.
    let mut listed = std::collections::HashSet::new();
    for (_, _, dress) in scripts().filter(|(_, class, _)| *class == "Dress") {
        let Ok(value) = assets.read(dress) else { continue };
        for renderer in value.get("Renderers").and_then(Value::as_array).unwrap_or(&[]) {
            if let Some(object) = renderer.as_pptr().and_then(|p| assets.resolve_loaded(dress.file, p)).and_then(|r| game_object_of(assets, r)) {
                listed.insert(object);
            }
        }
    }
    let mut found = Vec::new();
    for (index, _, script) in scripts().filter(|(_, class, _)| *class == "CustomItem") {
        let Ok(value) = assets.read(script) else { continue };
        let Some(fitting) = Fitting::of(assets, script, &value, "_data") else { continue };
        if !game_object_of(assets, script).is_some_and(|object| listed.contains(&object)) {
            continue;
        }
        // The mesh it swaps: its own object's, or the object's its filter
        // is on.
        let filter = value.get("_meshFilter").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(script.file, p)).and_then(|f| game_object_of(assets, f));
        let at = match filter {
            Some(object) if Some(object) != game_object_of(assets, script) => (0..hierarchy.nodes.len()).find(|&node| game_object_of(assets, hierarchy.nodes[node].transform) == Some(object)),
            _ => Some(index),
        };
        if let Some(at) = at {
            found.push((at, fitting));
        }
    }
    found
}

/// The fittings of every level of detail of a skinned thing worn: each skin
/// its dress lists (`SkinDress._lods`) that is a `CustomItemSkin`, read as
/// `skin_fitting` reads the fullest's (a level's mesh is its fitting's
/// `Base`; `load_levels`).
pub fn skin_fittings(assets: &mut Assets, bundle_key: &str) -> Result<Vec<Fitting>> {
    let files = assets.load(bundle_key)?;
    let scripts: Vec<ObjRef> = files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect();
    let Some(dress) = scripts.iter().copied().find(|&obj| script_class(assets, obj).as_deref() == Some("SkinDress")) else { return Ok(Vec::new()) };
    let listed = assets.read(dress)?;
    let skins: Vec<ObjRef> = listed.get("_lods").and_then(Value::as_array).unwrap_or(&[]).iter().filter_map(|v| v.as_pptr().and_then(|p| assets.resolve_loaded(dress.file, p))).collect();
    let mut all = Vec::new();
    for skin in skins.into_iter().filter(|&skin| script_class(assets, skin).as_deref() == Some("CustomItemSkin")) {
        let value = assets.read(skin)?;
        all.extend(Fitting::of(assets, skin, &value, "Data"));
    }
    Ok(all)
}

/// The fitting of a skinned thing worn (`CustomItemSkin`), where its
/// dress's fullest skin is one: the skin `load` reads, whose mesh is the
/// fitting's `Base` as the prefab saves it. Its dress starts every skin it
/// lists (`SkinDress.Init`).
pub fn skin_fitting(assets: &mut Assets, bundle_key: &str) -> Result<Option<Fitting>> {
    let files = assets.load(bundle_key)?;
    let scripts: Vec<ObjRef> = files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect();
    let Some(dress) = scripts.iter().copied().find(|&obj| script_class(assets, obj).as_deref() == Some("SkinDress")) else { return Ok(None) };
    let listed = assets.read(dress)?;
    let Some(fullest) = listed.get("_lods").and_then(Value::as_array).and_then(|lods| lods.first()).and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(dress.file, p)) else {
        return Ok(None);
    };
    if script_class(assets, fullest).as_deref() != Some("CustomItemSkin") {
        return Ok(None);
    }
    let value = assets.read(fullest)?;
    Ok(Fitting::of(assets, fullest, &value, "Data"))
}

/// The bits a part of the body adds to every fitting's rule while it is
/// worn (`ClippingRuleChanger` on the part's own object, beside its
/// `LoddedSkin`: `PlayerBody.SetSkin`): 0 for a part without one.
pub fn part_clipping(assets: &mut Assets, bundle_key: &str) -> Result<u8> {
    let files = assets.load(bundle_key)?;
    let scripts: Vec<ObjRef> = files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect();
    let of_class = |name: &str| scripts.iter().copied().filter(|&obj| script_class(assets, obj).as_deref() == Some(name)).collect::<Vec<_>>();
    let Some(skin) = of_class("LoddedSkin").first().and_then(|&skin| game_object_of(assets, skin)) else { return Ok(0) };
    let mut bits = 0;
    for changer in of_class("ClippingRuleChanger").into_iter().filter(|&changer| game_object_of(assets, changer) == Some(skin)) {
        bits |= assets.read(changer)?.get("_clippingCustoms").and_then(Value::as_i64).unwrap_or(0) as u8;
    }
    Ok(bits)
}
