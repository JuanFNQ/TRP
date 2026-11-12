//! Opening a weapon's prefab and assembling it by name or by key.

use super::*;

/// The game's empty hands: a prefab like a weapon's, with arms, hand
/// markers, an animator and clips of its own, that is no item of the
/// database. The game makes its item and template in code and names this
/// bundle there (`EmptyHandsTemplate`).
pub const EMPTY_HANDS: &str = "assets/content/weapons/empty hands/weapon_empty_hands_container.bundle";

/// Whether a script is a `WeaponPrefab` or one of its two kinds
/// (`UsableHandsPrefab`, `GrenadePrefab`: the game's only classes made
/// from it).
pub(super) fn is_weapon_prefab_script(assets: &Assets, obj: ObjRef) -> bool {
    matches!(script_class(assets, obj).as_deref(), Some("WeaponPrefab" | "UsableHandsPrefab" | "GrenadePrefab"))
}

/// Whether a bundle's prefab is a weapon's container: a `WeaponPrefab`
/// (or one of its kinds) on it. The game's pool hangs a copy of the
/// object it names under any such container as it makes it, whatever item
/// it is made for (`AbstractAssetsPool.RunInitialStageBody` takes the
/// prefab's `AssetPoolObject`, the `WeaponPrefab`, whose
/// `OnCreatePoolRoleModel` and `CacheInternalObjects` call
/// `CreateObjectInstance`): a weapon's, a grenade's, a knife's, and a
/// launcher's, the rangefinder's and the radio transmitter's, whose
/// templates are none of those (icons spec, "The same model").
pub fn is_weapon_container(assets: &mut Assets, key: &str) -> Result<bool> {
    let files = assets.load(key)?;
    Ok(files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).any(|obj| is_weapon_prefab_script(assets, obj)))
}

/// A bundle's `WeaponPrefab` and the tree of the object it names: a
/// weapon's, the empty hands', an item's hands for using it (medicine,
/// food: `UsableHandsPrefab`, a `WeaponPrefab` with the bone the item hangs
/// from besides), or a grenade's (`GrenadePrefab`, a `WeaponPrefab` with
/// the grenade thrown and the parts let go of in the throw besides).
pub(super) fn open_prefab(assets: &mut Assets, key: &str) -> Result<(ObjRef, Value, Hierarchy)> {
    let files = assets.load(key)?;
    let (prefab_obj, prefab) = files
        .iter()
        .flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR))
        .find(|&obj| is_weapon_prefab_script(assets, obj))
        .map(|obj| assets.read(obj).map(|v| (obj, v)))
        .ok_or_else(|| anyhow!("{key} has no WeaponPrefab"))??;
    let weapon_object = prefab
        .req("_weaponObject")?
        .as_pptr()
        .and_then(|p| assets.resolve_loaded(prefab_obj.file, p))
        .ok_or_else(|| anyhow!("WeaponPrefab has no weapon object"))?;
    let hierarchy = read_hierarchy(assets, weapon_object)?;
    Ok((prefab_obj, prefab, hierarchy))
}

/// The bone `TransformLinks` lists the vest's marker at
/// (`ECharacterWeaponBones.weapon_vest_IK_marker`).
const LINKED_VEST_MARKER: usize = 18;

/// Whether a rig's bundle names its vest's marker among the bones the
/// player takes from it (its `TransformLinks` list, entry 18): the player
/// takes the marker only from a list that long, and reaches for the vest
/// only with one (`Player.UpdateBonesOnWeaponChange`,
/// `AdjustUtilityLayerWeight`: the layer's weight to nought at once
/// without it). An item's hands for using it have a shorter list
/// (`courses8`'s eating: the layer's weight 0 from the item's first frame).
pub(super) fn vest_linked(assets: &Assets, files: &[usize]) -> Result<bool> {
    let links = files
        .iter()
        .flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR))
        .find(|&obj| script_class(assets, obj).as_deref() == Some("TransformLinks"));
    let Some(links) = links else { return Ok(false) };
    let value = assets.read(links)?;
    let listed = value.get("Transforms").and_then(Value::as_array).and_then(|list| list.get(LINKED_VEST_MARKER)).and_then(Value::as_pptr);
    Ok(listed.is_some_and(|pointer| !pointer.is_null()))
}

/// Reads a weapon-like prefab that is no item of the database, by its
/// bundle: nothing is fitted to it, and it weighs nothing. For the body's
/// animator it counts as one cell wide, which is what the game tells that
/// animator when empty hands appear.
pub fn assemble_prefab(assets: &mut Assets, key: &str) -> Result<Weapon> {
    let (prefab_obj, prefab, hierarchy) = open_prefab(assets, key)?;
    let name = hierarchy.nodes.first().map(|node| node.name.clone()).unwrap_or_default();
    let files = assets.load(key)?;
    let (extraction, patron_extraction) = extraction(assets, &files)?.unzip();
    let shell_port = shell_port(&hierarchy);
    let shell_ports = shell_ports(&hierarchy);
    let muzzle = crate::weapons::muzzle::read(assets, &files, &hierarchy)?;
    let vest_linked = vest_linked(assets, &files)?;
    Ok(Weapon {
        vest_linked,
        extraction,
        patron_extraction,
        shell_port,
        shell_ports,
        muzzle,
        template: String::new(),
        name,
        hierarchy,
        prefab,
        prefab_file: prefab_obj.file,
        skipped: Vec::new(),
        total_weight: 0.0,
        total_ergonomics: 0.0,
        recoil_delta: 0.0,
        speed_factor: 1.0,
        magazine_capacity: None,
        magazine_animation: None,
        magazine_external: None,
        unique_animation: 0,
        cell_width: 1,
        held_closer: false,
        fold: None,
        fold_bones: Vec::new(),
        custom: None,
        silenced: false,
        sound_kind: None,
        sights: Vec::new(),
        folding_sights: Vec::new(),
        devices: Vec::new(),
        bipod: None,
        launcher: None,
        launcher_muzzle: None,
        hot: Vec::new(),
    })
}

/// A weapon by its item template, or a weapon-like prefab by its bundle
/// (anything with a `/` in it).
pub fn assemble_named(assets: &mut Assets, db: &Database, what: &str) -> Result<Weapon> {
    if what.contains('/') { assemble_prefab(assets, what) } else { assemble(assets, db, what) }
}

/// Assembles the stock build of a weapon from the player's install.
pub fn assemble(assets: &mut Assets, db: &Database, weapon_template: &str) -> Result<Weapon> {
    let preset = db.default_preset(weapon_template)?.clone();
    assemble_preset(assets, db, &preset)
}

/// A weapon by its build's key (`build_key`), or by its template alone
/// for its stock build.
pub fn assemble_key(assets: &mut Assets, db: &Database, key: &str) -> Result<Weapon> {
    match preset_of_key(key) {
        Some(preset) => assemble_preset(assets, db, &preset),
        // A weapon the database has no stock build of (the RSP-30s, the
        // SP-81, the launchers' own weapons) is, by its template, the
        // weapon with nothing in its slots, as the game makes any item's
        // model of what its slots hold (`ObjectsFactory.CreateItemAsync`,
        // `AssembleMods`).
        None if db.default_preset(key).is_err() && db.items.contains_key(key) => assemble_preset(assets, db, &alone(key)),
        None => assemble(assets, db, key),
    }
}

/// A build of a weapon alone, nothing in its slots: its stock build where
/// the database has none (`assemble_key`; `trp_sim::inventory::Kind::
/// stock_key`).
pub fn alone(template: &str) -> Preset {
    Preset {
        id: template.to_owned(),
        name: String::new(),
        root: template.to_owned(),
        items: vec![PresetItem { id: template.to_owned(), template: template.to_owned(), parent: None, slot: None }],
        encyclopedia: None,
    }
}

/// What opens a build's parts in its key, closes them, and parts them.
pub(super) const KEY_OPEN: char = '[';

pub(super) const KEY_CLOSE: char = ']';

pub(super) const KEY_NEXT: char = ',';

pub(super) const KEY_SLOT: char = '=';

/// A build's key: its weapon's template, then its parts in brackets, each
/// the slot of its parent it is in, `=`, its template and its own parts in
/// brackets, by their slots' names (`5644bd2b4bdc2d3b4c8b4572[mod_gas_block=
/// 59c6633186f7740cf0493bb9[mod_handguard=...[]],mod_magazine=...[]]`):
/// alike for alike builds whatever order a list gives them. The inventory
/// makes one of a weapon it holds (`trp_sim::inventory::rig_key`) and the
/// app asks for a rig, a picture or a model on the ground by it. The app's
/// own way of naming a build (the game names a build by its items).
pub fn build_key(preset: &Preset) -> String {
    fn of(preset: &Preset, id: &str, template: &str, out: &mut String) {
        out.push_str(template);
        out.push(KEY_OPEN);
        let mut children: Vec<&PresetItem> = preset.items.iter().filter(|item| item.parent.as_deref() == Some(id) && item.slot.is_some()).collect();
        children.sort_by(|a, b| a.slot.cmp(&b.slot));
        for (n, child) in children.into_iter().enumerate() {
            if n > 0 {
                out.push(KEY_NEXT);
            }
            out.push_str(child.slot.as_deref().unwrap_or(""));
            out.push(KEY_SLOT);
            of(preset, &child.id, &child.template, out);
        }
        out.push(KEY_CLOSE);
    }
    let mut out = String::new();
    if let Some(root) = preset.items.iter().find(|item| item.id == preset.root) {
        of(preset, &root.id, &root.template, &mut out);
    }
    out
}
