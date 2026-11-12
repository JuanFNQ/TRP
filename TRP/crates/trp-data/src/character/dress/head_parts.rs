//! Head models, worn parts, hinges and their figures, and fitting kinds.

use super::*;

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
            let Some(at) = hang(assets, db, hierarchy, bone, taken) else { continue };
            let mut below = path.to_vec();
            below.push((holder.name.clone(), taken.clone()));
            parts.push(WornPart { path: below.clone(), root: at });
            fit_parts(assets, db, taken, hierarchy, at, &below, wanted, parts);
        }
    }
}

/// One part's model hung, hidden, on its slot's object, as `AttachMods`
/// hangs it: none for an armour plate, a thing with no model, or a model
/// that cannot be read. The index of its root.
fn hang(assets: &mut Assets, db: &Database, hierarchy: &mut Hierarchy, bone: usize, taken: &str) -> Option<usize> {
    let part = db.items.get(taken).filter(|part| !db.is_a(part, "ArmorPlate"))?;
    let key = part.prefab_key()?.to_owned();
    let at = crate::weapons::weapon::attach_at(assets, hierarchy, bone, &key).ok()?;
    hierarchy.nodes[at].active = false;
    Some(at)
}

/// A part fitted to a model on the head after it was made (`head_model`
/// read only the parts the run then had; the armor bench offers every
/// part of the game): hung by the same rule on the object of its slot's
/// name below `parent`, the root of what it is fitted to (the worn thing's
/// model, 0, or a part's), hidden, with the hinge its own model carries
/// (a face shield's own) if it has one. `path` is the way down to it, its
/// last step its slot and template. None where the game would hang
/// nothing there: no such object, an armour plate, no model.
pub fn fit_part(assets: &mut Assets, db: &Database, hierarchy: &mut Hierarchy, parent: usize, path: &[(String, String)]) -> Option<(WornPart, Option<Hinge>)> {
    let (slot, taken) = path.last()?;
    let bone = find_ignoring_case(hierarchy, parent, slot)?;
    let at = hang(assets, db, hierarchy, bone, taken)?;
    let hinge = db.items.get(taken).and_then(|item| item.prefab_key()).and_then(|key| hinge_in(assets, key, hierarchy, at, path.to_vec()).ok().flatten());
    Some((WornPart { path: path.to_vec(), root: at }, hinge))
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
    let Some(figures) = hinge_figures(assets, key)? else { return Ok(None) };
    let Some(node) = figures.object.as_deref().and_then(|name| hierarchy.find_below(root, name)) else { return Ok(None) };
    Ok(Some(Hinge { owner, node, off: figures.off, on: figures.on, forward: figures.forward, backward: figures.backward }))
}

/// A hinge's figures as a model's `HelmetDress` holds them, without the
/// model: its two turns, its filter's two curves and the name of the
/// object it turns (none where that cannot be read).
#[derive(Debug, Clone)]
pub struct HingeFigures {
    pub off: [f32; 4],
    pub on: [f32; 4],
    pub forward: Curve,
    pub backward: Curve,
    pub object: Option<String>,
}

/// The hinge an item's own model carries, if it has one (a helmet's for
/// its shield, a face shield's own): for setting the filter and the turn
/// beside a recording (`replay`).
pub fn own_hinge(assets: &mut Assets, db: &Database, template: &str) -> Option<HingeFigures> {
    let key = db.items.get(template)?.prefab_key()?.to_owned();
    hinge_figures(assets, &key).ok().flatten()
}

fn hinge_figures(assets: &mut Assets, key: &str) -> Result<Option<HingeFigures>> {
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
    let object = pointed(&value, "HingeTransform")
        .and_then(|t| assets.read(t).ok())
        .and_then(|t| pointed(&t, "m_GameObject"))
        .and_then(|go| assets.read(go).ok())
        .and_then(|go| go.get("m_Name").and_then(Value::as_str).map(str::to_owned));
    let filter = pointed(&value, "Filter").and_then(|f| assets.read(f).ok());
    let curve = |name: &str| filter.as_ref().and_then(|f| f.get(name)).and_then(|c| crate::character::player::curve(c).ok()).unwrap_or_default();
    Ok(Some(HingeFigures { off, on, forward: curve("Forward"), backward: curve("Backward"), object }))
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

/// The kinds of thing worn on the head that one thing's mesh may be fitted
/// to the others by (`EClippingCustoms`; body animator spec, "Worn items",
/// "The head's things fitted to one another"): a bit each.
pub mod fit {
    pub const FACE_COVER: u8 = 1;
    pub const VISOR: u8 = 2;
    pub const HAT: u8 = 4;
    pub const HEADPHONES: u8 = 8;
    pub const HELMET: u8 = 16;
    /// The kinds a fitting may be of: those the game lists what to listen
    /// to for (`CustomItem.InterestSlots`). One of any other kind is never
    /// started, and its mesh stays as the prefab saves it.
    pub const KINDS: [u8; 5] = [FACE_COVER, VISOR, HAT, HEADPHONES, HELMET];
    /// Not the game's: the app carries a hood's rule (`super::hood_up`) in
    /// the same byte as a wearer's bits, so that a part's hooded form is a
    /// second form like a fitting's. The game's flags end at 16 and no
    /// part's changer in the install gives this one (`probe fittings`).
    pub const HOOD_UP: u8 = 0x80;
    /// Not the game's either: the head's own rule (`HeadSkin.Init`: its
    /// mesh for under a face cover while the face cover's slot holds a face
    /// cover that is not a half mask), carried in the same byte so that a
    /// head's other mesh is a second form like the rest.
    pub const FACE_COVERED: u8 = 0x40;
}
