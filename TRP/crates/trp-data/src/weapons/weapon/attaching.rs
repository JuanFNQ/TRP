//! Attaching parts and rounds to a weapon's tree.

use super::*;

impl Weapon {
    /// Clips of the full animation set, the one that moves both arms, every
    /// finger and the weapon's parts.
    ///
    /// The prefab also names two reduced sets (`_animatorSimple`,
    /// `_animatorSpirit`). Their clips only move the weapon root and muzzle.
    pub fn clips(&self, assets: &Assets) -> Result<Vec<(String, ObjRef)>> {
        let controller = self
            .prefab
            .req("_originalAnimatorController")?
            .as_pptr()
            .and_then(|p| assets.resolve_loaded(self.prefab_file, p))
            .ok_or_else(|| anyhow!("weapon has no animator controller"))?;
        crate::unity::anim::controller_clips(assets, controller)
    }
}

pub(super) fn attach_part(
    assets: &mut Assets,
    hierarchy: &mut Hierarchy,
    parent_root: usize,
    slot: &str,
    prefab_key: Option<&str>,
) -> Result<usize> {
    let bone = hierarchy
        .find_below(parent_root, slot)
        .ok_or_else(|| anyhow!("no bone named {slot}"))?;
    let key = prefab_key.ok_or_else(|| anyhow!("item has no prefab"))?;
    attach_at(assets, hierarchy, bone, key)
}

/// Hangs the part a bundle holds on that object of the hierarchy, as the
/// game hangs a part on its slot's object, and gives the index of its root.
pub(crate) fn attach_at(assets: &mut Assets, hierarchy: &mut Hierarchy, bone: usize, key: &str) -> Result<usize> {
    let mut part = prefab_tree(assets, key)?;
    // The game's own rule (`ContainerCollectionView.SlotView.InsertItem`):
    // a part goes on its bone at the bone's origin, a quarter turn about x
    // from the bone and at its own size; a part that carries a `ModPlacer`
    // goes where that says instead. (Most prefabs are stored upright with
    // their geometry turned a quarter turn underneath, and the bones live
    // inside that turned space: the quarter turn stands the part upright
    // again. A part made in its bone's own space says so with a
    // `ModPlacer`, as the M3's stock does.) What the weapon's clips then
    // make of a part's root is the animator's business.
    let (position, rotation, scale) = match part.nodes[0].placer {
        Some(placer) => (placer.position, quat_of_euler(placer.rotation), placer.scale),
        None => ([0.0; 3], quat_of_euler([90.0, 0.0, 0.0]), [1.0; 3]),
    };
    part.nodes[0].translation = position;
    part.nodes[0].rotation = rotation;
    part.nodes[0].scale = scale;
    Ok(hierarchy.attach(bone, part))
}

/// The tree of the part a bundle holds, its root first, as its prefab
/// saves it. A part whose bundle is a weapon's container (an underbarrel
/// launcher's: a `WeaponPrefab` of its own) is the object that names, as
/// the game's pool makes it (`is_weapon_container`); a launcher's bundle
/// exports three objects at its root, and the first is not always the
/// model (the M203's is the bare container: underbarrel-launcher spec,
/// "Found while reading").
pub fn prefab_tree(assets: &mut Assets, key: &str) -> Result<Hierarchy> {
    let files = assets.load(key).with_context(|| format!("loading {key}"))?;
    let named = files
        .iter()
        .flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR))
        .find(|&obj| is_weapon_prefab_script(assets, obj))
        .and_then(|prefab| {
            let value = assets.read(prefab).ok()?;
            value.get("_weaponObject").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(prefab.file, p))
        });
    if let Some(object) = named {
        return read_hierarchy(assets, object);
    }
    let root = files
        .iter()
        .find_map(|&f| part_root(assets, f).transpose())
        .transpose()?
        .ok_or_else(|| anyhow!("{key} exports no prefab"))?;
    read_hierarchy(assets, root)
}

/// Hangs a round's model on a magazine's point as the game fills one
/// (`SpringMagazineVisual.FillAmmoObjectsOnStart`): its prefab at the
/// point's origin, a quarter turn about x, at its own size, its objects
/// named `full` on and those named `shell` off (`AmmoPoolObject.SetUsed`).
/// The index of its root.
pub fn attach_round(assets: &mut Assets, hierarchy: &mut Hierarchy, point: usize, key: &str) -> Result<usize> {
    let files = assets.load(key).with_context(|| format!("loading {key}"))?;
    let root = files
        .iter()
        .find_map(|&f| part_root(assets, f).transpose())
        .transpose()?
        .ok_or_else(|| anyhow!("{key} exports no prefab"))?;
    let mut round = read_hierarchy(assets, root)?;
    round.nodes[0].translation = [0.0; 3];
    round.nodes[0].rotation = quat_of_euler([90.0, 0.0, 0.0]);
    for node in &mut round.nodes {
        if node.name.contains("shell") {
            node.active = false;
        } else if node.name.contains("full") {
            node.active = true;
        }
    }
    Ok(hierarchy.attach(point, round))
}

/// The engine's `Quaternion.Euler` as `xyzw`: degrees about z, then about
/// x, then about y.
pub(crate) fn quat_of_euler(degrees: [f32; 3]) -> [f32; 4] {
    let [(sx, cx), (sy, cy), (sz, cz)] = degrees.map(|d| (d.to_radians() * 0.5).sin_cos());
    [
        cy * sx * cz + sy * cx * sz,
        sy * cx * cz - cy * sx * sz,
        cy * cx * sz - sy * sx * cz,
        cy * cx * cz + sy * sx * sz,
    ]
}

/// The prefab a part bundle exports. Bundles can list several objects; the
/// one that is the root of its own tree is the part.
pub(crate) fn part_root(assets: &Assets, file: usize) -> Result<Option<ObjRef>> {
    let exported = exported_game_objects(assets, file)?;
    for (_, obj) in &exported {
        let go = assets.read(*obj)?;
        let is_root = go
            .req("m_Component")?
            .as_array()
            .unwrap_or(&[])
            .iter()
            .filter_map(|c| c.get("component")?.as_pptr())
            .filter_map(|p| assets.resolve_loaded(file, p))
            .filter(|c| assets.class_id(*c) == Some(class::TRANSFORM))
            .filter_map(|t| assets.read(t).ok())
            .any(|t| t.get("m_Father").and_then(Value::as_pptr).is_some_and(|p| p.is_null()));
        if is_root {
            return Ok(Some(*obj));
        }
    }
    Ok(exported.first().map(|(_, obj)| *obj))
}
