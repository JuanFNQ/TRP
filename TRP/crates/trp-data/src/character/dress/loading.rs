//! Surveying the dress bundles and loading a dress with its levels.

use super::*;

/// One equipment bundle as the survey finds it: how many scripts of each
/// kind of fitting it holds (every level of detail's), and what is read of
/// them as the thing is worn: a model's fittings on its own (`fittings`)
/// or a skin's (`skin_fitting`), each with its kind, its object's name and
/// its two meshes' counts of submeshes and of bind poses.
#[derive(Debug, Clone, Default)]
pub struct SurveyedBundle {
    pub key: String,
    pub plain_scripts: usize,
    pub skin_scripts: usize,
    pub read: Vec<SurveyedFitting>,
}

#[derive(Debug, Clone)]
pub struct SurveyedFitting {
    pub kind: u8,
    pub object: String,
    /// Whether the object draws (a level of detail's other objects carry a
    /// fitting of their own and draw nothing near).
    pub drawn: bool,
    pub submeshes: [usize; 2],
    pub bind_poses: [usize; 2],
}

/// A body part as the survey finds it: the bits its own changer adds
/// (`part_clipping`) and whether the bundle holds a changer anywhere.
#[derive(Debug, Clone)]
pub struct SurveyedPart {
    pub key: String,
    pub bits: u8,
    pub any_changer: bool,
}

/// The install's fittings, for the probe (`probe fittings`) and the tests:
/// every bundle under `assets/content/items/equipment/` in the manifest,
/// and every bundle the customization list names for a body, feet or a
/// head (`templates/customization.json`) that the manifest has, each read
/// as the app reads it; and how many the list names that it has not. The
/// bundles are read a few dozen to a reader, which is let go between.
pub fn survey(install: &crate::Install) -> Result<(Vec<SurveyedBundle>, Vec<SurveyedPart>, usize)> {
    const EQUIPMENT: &str = "assets/content/items/equipment/";
    const BATCH: usize = 40;
    let manifest = install.manifest()?;
    let mut keys: Vec<String> = manifest.entries.keys().filter(|key| key.starts_with(EQUIPMENT) && key.ends_with(".bundle")).cloned().collect();
    keys.sort();
    let mut bundles = Vec::new();
    for chunk in keys.chunks(BATCH) {
        let mut assets = Assets::new(install.clone())?;
        for key in chunk {
            bundles.push(survey_bundle(&mut assets, key)?);
        }
    }
    let path = install.database_dir().join("templates/customization.json");
    let list: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
    let mut part_keys: Vec<String> = list
        .as_object()
        .into_iter()
        .flat_map(|all| all.values())
        .filter(|entry| matches!(entry.pointer("/_props/BodyPart").and_then(|part| part.as_str()), Some("Body" | "Feet" | "Head")))
        .filter_map(|entry| entry.pointer("/_props/Prefab/path").and_then(|path| path.as_str()).filter(|path| !path.is_empty()).map(str::to_owned))
        .collect();
    part_keys.sort();
    part_keys.dedup();
    let named = part_keys.len();
    part_keys.retain(|key| manifest.entries.contains_key(key));
    let missing = named - part_keys.len();
    let mut parts = Vec::new();
    for chunk in part_keys.chunks(BATCH) {
        let mut assets = Assets::new(install.clone())?;
        for key in chunk {
            let bits = part_clipping(&mut assets, key)?;
            let files = assets.load(key)?;
            let any_changer = files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).any(|obj| script_class(&assets, obj).as_deref() == Some("ClippingRuleChanger"));
            parts.push(SurveyedPart { key: key.clone(), bits, any_changer });
        }
    }
    Ok((bundles, parts, missing))
}

fn survey_bundle(assets: &mut Assets, key: &str) -> Result<SurveyedBundle> {
    let files = assets.load(key)?;
    let scripts: Vec<ObjRef> = files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect();
    let count = |name: &str| scripts.iter().filter(|&&obj| script_class(assets, obj).as_deref() == Some(name)).count();
    let mut bundle = SurveyedBundle { key: key.to_owned(), plain_scripts: count("CustomItem"), skin_scripts: count("CustomItemSkin"), read: Vec::new() };
    let counts = |assets: &Assets, mesh: ObjRef| -> (usize, usize) {
        let value = assets.read(mesh).ok();
        let length = |name: &str| value.as_ref().and_then(|v| v.get(name)).and_then(Value::len).unwrap_or(0);
        (length("m_SubMeshes"), length("m_BindPose"))
    };
    let surveyed = |assets: &Assets, fitting: Fitting, object: String, drawn: bool| {
        let (base, custom) = (counts(assets, fitting.base), counts(assets, fitting.custom));
        SurveyedFitting { kind: fitting.kind, object, drawn, submeshes: [base.0, custom.0], bind_poses: [base.1, custom.1] }
    };
    if let Some(fitting) = skin_fitting(assets, key)? {
        bundle.read.push(surveyed(assets, fitting, "the fullest skin".to_owned(), true));
    } else if let Ok(Worn::Rigid(hierarchy)) = load(assets, key, Under::default()) {
        for (node, fitting) in fittings(assets, &hierarchy) {
            let n = &hierarchy.nodes[node];
            bundle.read.push(surveyed(assets, fitting, n.name.clone(), n.renderer.is_some()));
        }
    }
    Ok(bundle)
}

/// Reads the worn form of the item in a bundle.
pub fn load(assets: &mut Assets, bundle_key: &str, under: Under) -> Result<Worn> {
    let files = assets.load(bundle_key)?;
    let scripts: Vec<ObjRef> = files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect();
    let of_class = |name: &str| scripts.iter().copied().find(|&obj| script_class(assets, obj).as_deref() == Some(name));
    // An armband's worn half: skinned to the skeleton under the body's
    // mesh object, as any `DressItem` (its slot's bone, the right
    // shoulder, is not used: `SlotView.CreateAndParent`).
    if let Some(view) = of_class("ArmBandView") {
        return Ok(Worn::Band(arm_band(assets, view)?));
    }
    let Some(dress) = of_class("SkinDress") else {
        // No skin: the bundle's own object is the model.
        let root = files
            .iter()
            .find_map(|&file| exported_game_objects(assets, file).ok()?.into_iter().next())
            .ok_or_else(|| anyhow!("{bundle_key} exports no object"))?;
        return Ok(Worn::Rigid(read_hierarchy(assets, root.1)?));
    };
    // The skins are listed by level of detail, the fullest first.
    let listed = assets.read(dress)?;
    let fullest = listed
        .req("_lods")?
        .as_array()
        .and_then(|lods| lods.first())
        .and_then(Value::as_pptr)
        .and_then(|p| assets.resolve_loaded(dress.file, p))
        .ok_or_else(|| anyhow!("{bundle_key}: the dress lists no skin"))?;
    Ok(Worn::Skinned(dress_skin(assets, bundle_key, fullest, under)?))
}

/// Every level of detail of a skinned thing worn, over what `under` says
/// (third-person spec, "Levels of detail"): each skin its dress lists
/// (`SkinDress._lods`), read as `load` reads the fullest, with the levels
/// the group that lists the fullest's renderer (its `Skin` group; the loot
/// model's is another) draws it at; the fullest first. A skin no level
/// lists is left out. None for a thing that is not skinned; the fullest
/// alone, with no levels, where no group governs it.
pub fn load_levels(assets: &mut Assets, bundle_key: &str, under: Under) -> Result<Option<Vec<Arms>>> {
    let Worn::Skinned(fullest) = load(assets, bundle_key, under)? else { return Ok(None) };
    let files = assets.load(bundle_key)?;
    let scripts: Vec<ObjRef> = files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect();
    let Some(dress) = scripts.iter().copied().find(|&obj| script_class(assets, obj).as_deref() == Some("SkinDress")) else { return Ok(Some(vec![fullest])) };
    let Some(group) = crate::drawing::lod::of_files(assets, &files).into_iter().find(|group| group.lists(fullest.renderer)) else { return Ok(Some(vec![fullest])) };
    let skins: Vec<ObjRef> = assets.read(dress)?.req("_lods")?.as_array().unwrap_or(&[]).iter().filter_map(|v| v.as_pptr().and_then(|p| assets.resolve_loaded(dress.file, p))).collect();
    let mut all = Vec::new();
    for (at, &skin) in skins.iter().enumerate() {
        let mut level = if at == 0 { fullest.clone() } else { dress_skin(assets, bundle_key, skin, under)? };
        let bits = group.levels_of(level.renderer);
        if bits == 0 {
            continue;
        }
        level.lod = Some(crate::drawing::lod::SkinLod { group: group.clone(), levels: bits });
        all.push(level);
    }
    if all.is_empty() {
        return Ok(Some(vec![fullest]));
    }
    // TRP's measure of its levels (`lod::measured`), by the meshes each
    // draws in their own axes, as the point below is had.
    let meshes: Vec<(ObjRef, u8)> = all.iter().filter_map(|level| Some((level.mesh, level.lod.as_ref()?.levels))).collect();
    let group = std::sync::Arc::new(crate::drawing::lod::measured(Some(&*assets), &group, || crate::drawing::lod::skin_shapes(assets, group.levels.len(), &meshes)));
    for level in &mut all {
        if let Some(lod) = &mut level.lod {
            lod.group = group.clone();
        }
    }
    // A worn thing's group is placed anew as it is dressed (`Dress.Skin`:
    // `LODGroup.RecalculateBounds`, its saved size put back), its point the
    // middle of its renderers' bounds as they then stand: taken here as the
    // middle of its levels' meshes' own bounds, as at their bind pose, the
    // skeleton's pose at the dressing not taken (0.07 to 0.23 m from the
    // game's in the recordings, `local\lod_compare_s271.md`). Only
    // the game's rule reads it (the app's log of its count): TRP's own
    // picks by each piece's own place.
    if let Some(centre) = bounds_centre(assets, all.iter().map(|level| level.mesh)) {
        let placed = std::sync::Arc::new(crate::drawing::lod::LodGroup { reference: centre, ..(*group).clone() });
        for level in &mut all {
            if let Some(lod) = &mut level.lod {
                lod.group = placed.clone();
            }
        }
    }
    Ok(Some(all))
}

/// The middle of the meshes' own bounds together (`m_LocalAABB`), in their
/// own axes; none where none can be read.
pub fn bounds_centre(assets: &Assets, meshes: impl Iterator<Item = ObjRef>) -> Option<[f32; 3]> {
    let (mut low, mut high) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for mesh in meshes {
        let Ok(value) = assets.read(mesh) else { continue };
        let (Some(centre), Some(extent)) = (value.at("m_LocalAABB.m_Center").and_then(Value::as_vec3), value.at("m_LocalAABB.m_Extent").and_then(Value::as_vec3)) else { continue };
        for k in 0..3 {
            low[k] = low[k].min(centre[k] - extent[k]);
            high[k] = high[k].max(centre[k] + extent[k]);
        }
    }
    low[0].is_finite().then(|| std::array::from_fn(|k| (low[k] + high[k]) * 0.5))
}

/// One skin a dress lists, over what `under` says: a plain skin, or one of
/// the kinds that hold a skin and a mesh for each thing it may be worn
/// over.
fn dress_skin(assets: &Assets, bundle_key: &str, fullest: ObjRef, under: Under) -> Result<Arms> {
    let entry = assets.read(fullest)?;
    let pointed = |v: Option<&Value>| v.and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(fullest.file, p));
    // A plain skin, or one of the kinds that hold a skin and a mesh for
    // each thing it may be worn over.
    let (skin, mesh) = match script_class(assets, fullest).as_deref() {
        Some("VestSkin") => {
            let data = entry.req("Data")?;
            (pointed(entry.get("Skin")), pointed(data.get(if under.armour { "Armor" } else { "Body" })))
        }
        Some("BackpackSkin") => {
            let data = entry.req("Data")?;
            let name = match (under.armour, under.vest) {
                (true, true) => "ArmorVest",
                (true, false) => "Armor",
                (false, true) => "Vest",
                (false, false) => "Body",
            };
            (pointed(entry.get("Skin")), pointed(data.get(name)))
        }
        Some("TorsoSkin") => {
            let name = if under.armour { "_armor" } else if under.vest { "_vest" } else { "_base" };
            (pointed(entry.get("_skin")), pointed(entry.get(name)))
        }
        _ => (Some(fullest), None),
    };
    let skin = skin.ok_or_else(|| anyhow!("{bundle_key}: the dress's skin is missing"))?;
    let mut part = arms::read_skin(assets, skin)?;
    // A mesh left empty for one case falls back on the renderer's own.
    if let Some(mesh) = mesh {
        part.mesh = mesh;
    }
    Ok(part)
}
