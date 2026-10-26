//! What a character wears over its clothing: armour, vests, backpacks,
//! helmets, headsets. An item's bundle is one of two kinds
//! (`docs/spec/body-animator.md`, "Worn items"): a skin for the player's
//! skeleton, built like a piece of clothing, or a plain model that is hung
//! on the head.

use anyhow::{anyhow, Result};

use crate::arms::{self, Arms};
use crate::curve::Curve;
use crate::database::Database;
use crate::unity::extract::{exported_game_objects, read_hierarchy, script_class, Hierarchy};
use crate::unity::{class, Assets, ObjRef, Value};

/// What else is worn, which some skins choose their mesh by: a backpack
/// sits further out over a vest, a vest over armour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Under {
    pub armour: bool,
    pub vest: bool,
}

pub enum Worn {
    /// Skinned to the bones it names by path, like clothing.
    Skinned(Arms),
    /// A model of its own, for the head bone.
    Rigid(Hierarchy),
}

/// A hinge (`HelmetDress`): the thing whose model carries it, as the
/// slots and templates from the worn thing down to it (none: the worn
/// thing itself; a face shield that carries its own, the Ops-Core FAST's
/// visor say, its way down), the object it turns, that object's own turn
/// with the shield up (off) and down (on), as `xyzw` in the engine's axes,
/// and its filter's two curves, which take it from one to the other
/// (`HysteresisFilter`: `Forward` going down, `Backward` coming up).
#[derive(Debug, Clone)]
pub struct Hinge {
    pub owner: Vec<(String, String)>,
    pub node: usize,
    pub off: [f32; 4],
    pub on: [f32; 4],
    pub forward: Curve,
    pub backward: Curve,
}

/// A part a worn thing's model may have fitted (`ObjectsFactory.
/// AttachMods`): the slots and templates from the worn thing down to it,
/// and the object of the model its own model hangs from.
#[derive(Debug, Clone, PartialEq)]
pub struct WornPart {
    pub path: Vec<(String, String)>,
    pub root: usize,
}

/// A model worn on the head with what may be fitted to it, and its hinges.
/// Every part its slots take, and the parts' slots in turn, that `wanted`
/// asks for and that has a model is hung on the object of its slot's name
/// as the game hangs a part (`AttachMods`: the object found by name below
/// the model, whatever its letters' case; an armour plate never), each
/// left hidden: whichever is fitted is to be shown. A hinge is read from
/// each model so made, the worn thing's and each part's, as the game sets
/// up the dresses of every model it makes with that model's own item
/// (`ObjectsFactory.CreateItemAsync`, `IDress.Init`): six helmets carry
/// their shield's (the ZSh-1-2M's), six shields their own.
pub fn head_model(assets: &mut Assets, db: &Database, template: &str, mut hierarchy: Hierarchy, wanted: &dyn Fn(&str) -> bool) -> (Hierarchy, Vec<WornPart>, Vec<Hinge>) {
    let mut parts = Vec::new();
    fit_parts(assets, db, template, &mut hierarchy, 0, &[], wanted, &mut parts);
    let mut hinges = Vec::new();
    let models = std::iter::once((Vec::new(), template.to_owned(), 0)).chain(parts.iter().map(|part| (part.path.clone(), part.path.last().map_or(String::new(), |(_, of)| of.clone()), part.root)));
    for (owner, of, root) in models.collect::<Vec<_>>() {
        let Some(key) = db.items.get(&of).and_then(|item| item.prefab_key()) else { continue };
        if let Ok(Some(hinge)) = hinge_in(assets, key, &hierarchy, root, owner) {
            hinges.push(hinge);
        }
    }
    (hierarchy, parts, hinges)
}

#[allow(clippy::too_many_arguments)]
fn fit_parts(assets: &mut Assets, db: &Database, template: &str, hierarchy: &mut Hierarchy, root: usize, path: &[(String, String)], wanted: &dyn Fn(&str) -> bool, parts: &mut Vec<WornPart>) {
    let Some(item) = db.items.get(template) else { return };
    for holder in item.slots() {
        let Some(bone) = find_ignoring_case(hierarchy, root, &holder.name) else { continue };
        for taken in holder.takes.iter().filter(|id| wanted(id)) {
            let Some(part) = db.items.get(taken).filter(|part| !db.is_a(part, "ArmorPlate")) else { continue };
            let Some(key) = part.prefab_key().map(str::to_owned) else { continue };
            let Ok(at) = crate::weapon::attach_at(assets, hierarchy, bone, &key) else { continue };
            hierarchy.nodes[at].active = false;
            let mut below = path.to_vec();
            below.push((holder.name.clone(), taken.clone()));
            parts.push(WornPart { path: below.clone(), root: at });
            fit_parts(assets, db, taken, hierarchy, at, &below, wanted, parts);
        }
    }
}

/// The first object of that name at or below `start`, nearest first, its
/// letters' case aside (`TransformTools.FindTransformRecursive` as
/// `AttachMods` asks it).
fn find_ignoring_case(hierarchy: &Hierarchy, start: usize, name: &str) -> Option<usize> {
    let mut depth = vec![usize::MAX; hierarchy.nodes.len()];
    depth[start] = 0;
    let mut best: Option<(usize, usize)> = None;
    for index in start..hierarchy.nodes.len() {
        if index != start {
            let Some(parent) = hierarchy.nodes[index].parent.filter(|parent| depth[*parent] != usize::MAX) else { continue };
            depth[index] = depth[parent] + 1;
        }
        if hierarchy.nodes[index].name.eq_ignore_ascii_case(name) && best.is_none_or(|(d, _)| depth[index] < d) {
            best = Some((depth[index], index));
        }
    }
    best.map(|(_, index)| index)
}

/// The hinge a bundle's `HelmetDress` names, its object found in the model
/// at or below `root` (where that bundle's model hangs) by the name of the
/// object its transform is on.
fn hinge_in(assets: &mut Assets, key: &str, hierarchy: &Hierarchy, root: usize, owner: Vec<(String, String)>) -> Result<Option<Hinge>> {
    let files = assets.load(key)?;
    let Some(dress) = files.iter().flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR)).find(|&obj| script_class(assets, obj).as_deref() == Some("HelmetDress")) else {
        return Ok(None);
    };
    let value = assets.read(dress)?;
    let quat = |name: &str| -> Option<[f32; 4]> {
        let q = value.get(name)?;
        let f = |axis: &str| q.get(axis).and_then(Value::as_f32).unwrap_or(0.0);
        Some([f("x"), f("y"), f("z"), f("w")])
    };
    let (Some(off), Some(on)) = (quat("OffRotation"), quat("OnRotation")) else { return Ok(None) };
    let pointed = |v: &Value, name: &str| v.get(name).and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(dress.file, p));
    let name = pointed(&value, "HingeTransform")
        .and_then(|t| assets.read(t).ok())
        .and_then(|t| pointed(&t, "m_GameObject"))
        .and_then(|go| assets.read(go).ok())
        .and_then(|go| go.get("m_Name").and_then(Value::as_str).map(str::to_owned));
    let Some(node) = name.and_then(|name| hierarchy.find_below(root, &name)) else { return Ok(None) };
    let filter = pointed(&value, "Filter").and_then(|f| assets.read(f).ok());
    let curve = |name: &str| filter.as_ref().and_then(|f| f.get(name)).and_then(|c| crate::player::curve(c).ok()).unwrap_or_default();
    Ok(Some(Hinge { owner, node, off, on, forward: curve("Forward"), backward: curve("Backward") }))
}

/// Whether an item's model marks it as armour worn like a vest
/// (`VestlikeArmor` on it), under which a torso of the newer kind takes its
/// mesh for a vest (`TorsoSkin.ApplySkin`, which asks the armour's own
/// dress).
pub fn vestlike(assets: &mut Assets, bundle_key: &str) -> Result<bool> {
    let files = assets.load(bundle_key)?;
    let scripts: Vec<ObjRef> = files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect();
    Ok(scripts.into_iter().any(|obj| script_class(assets, obj).as_deref() == Some("VestlikeArmor")))
}

/// Reads the worn form of the item in a bundle.
pub fn load(assets: &mut Assets, bundle_key: &str, under: Under) -> Result<Worn> {
    let files = assets.load(bundle_key)?;
    let scripts: Vec<ObjRef> = files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect();
    let of_class = |name: &str| scripts.iter().copied().find(|&obj| script_class(assets, obj).as_deref() == Some(name));
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
    Ok(Worn::Skinned(part))
}
