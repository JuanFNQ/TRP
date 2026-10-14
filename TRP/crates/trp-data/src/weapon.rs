//! Building a weapon the way the game does: a base prefab plus one prefab
//! per installed part, each hung on the bone named after its slot.

use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};

use crate::database::Database;
use crate::unity::extract::{exported_game_objects, read_hierarchy, script_class, Hierarchy};
use crate::unity::{class, Assets, ObjRef, Value};

pub struct Weapon {
    pub template: String,
    pub name: String,
    /// The base prefab with every part attached. Node 0 is the animated root.
    pub hierarchy: Hierarchy,
    /// Serialized fields of the weapon's `WeaponPrefab` script.
    pub prefab: Value,
    /// File the `WeaponPrefab` lives in, for resolving its references.
    pub prefab_file: usize,
    /// Parts that could not be placed, with the reason.
    pub skipped: Vec<String>,
    /// Sum of the `Weight` of the weapon, its parts and a full load of its
    /// default ammunition, in kilograms.
    pub total_weight: f32,
    /// Sum of the `Ergonomics` of the weapon and its parts.
    pub total_ergonomics: f32,
    /// What the parts add to the weapon's recoil, as a fraction: the sum of
    /// their `Recoil` percentages. Negative for a build that kicks less.
    pub recoil_delta: f32,
    /// Rounds the fitted magazine holds, if the build has one.
    pub magazine_capacity: Option<u32>,
    /// How many inventory cells wide the build is. The body's animator
    /// picks how the weapon is carried by it.
    pub cell_width: u32,
    /// Whether the game holds the build closer to the body (a weapon of
    /// `CompactHandling` with nothing on it that reaches the shoulder).
    pub held_closer: bool,
    /// What the build is with its stock folded, if it has one that folds.
    pub fold: Option<Fold>,
    /// For a weapon of one's own (`crate::custom`): the model's parts on
    /// the donor's rig and the edits to the donor's animation.
    pub custom: Option<std::sync::Arc<crate::custom::Build>>,
}

/// A build with its stock folded (`FoldableComponent`, `Weapon.RecoilDelta`,
/// `CompoundItem.CalculateExtraSize`, `ProceduralWeaponAnimation.
/// CheckShouldMoveWeaponCloser`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fold {
    /// The parts' share of the recoil without the stock's.
    pub recoil_delta: f32,
    /// Cells wide.
    pub cell_width: u32,
    /// Whether it is then held closer to the body.
    pub held_closer: bool,
}

/// The game's empty hands: a prefab like a weapon's, with arms, hand
/// markers, an animator and clips of its own, that is no item of the
/// database. The game makes its item and template in code and names this
/// bundle there (`EmptyHandsTemplate`).
pub const EMPTY_HANDS: &str = "assets/content/weapons/empty hands/weapon_empty_hands_container.bundle";

/// A bundle's `WeaponPrefab` and the tree of the object it names.
fn open_prefab(assets: &mut Assets, key: &str) -> Result<(ObjRef, Value, Hierarchy)> {
    let files = assets.load(key)?;
    let (prefab_obj, prefab) = files
        .iter()
        .flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR))
        .find(|&obj| script_class(assets, obj).as_deref() == Some("WeaponPrefab"))
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

/// Reads a weapon-like prefab that is no item of the database, by its
/// bundle: nothing is fitted to it, and it weighs nothing. For the body's
/// animator it counts as one cell wide, which is what the game tells that
/// animator when empty hands appear.
pub fn assemble_prefab(assets: &mut Assets, key: &str) -> Result<Weapon> {
    let (prefab_obj, prefab, hierarchy) = open_prefab(assets, key)?;
    let name = hierarchy.nodes.first().map(|node| node.name.clone()).unwrap_or_default();
    Ok(Weapon {
        template: String::new(),
        name,
        hierarchy,
        prefab,
        prefab_file: prefab_obj.file,
        skipped: Vec::new(),
        total_weight: 0.0,
        total_ergonomics: 0.0,
        recoil_delta: 0.0,
        magazine_capacity: None,
        cell_width: 1,
        held_closer: false,
        fold: None,
        custom: None,
    })
}

/// A weapon by its item template, or a weapon-like prefab by its bundle
/// (anything with a `/` in it).
pub fn assemble_named(assets: &mut Assets, db: &Database, what: &str) -> Result<Weapon> {
    if what.contains('/') { assemble_prefab(assets, what) } else { assemble(assets, db, what) }
}

/// Assembles the stock build of a weapon from the user's install.
pub fn assemble(assets: &mut Assets, db: &Database, weapon_template: &str) -> Result<Weapon> {
    let preset = db.default_preset(weapon_template)?;
    let root_item = preset
        .items
        .iter()
        .find(|i| i.id == preset.root)
        .ok_or_else(|| anyhow!("preset {} has no root item", preset.id))?;
    let base = db.item(&root_item.template)?;
    let key = base.prefab_key().ok_or_else(|| anyhow!("{} has no prefab", base.name))?;
    let (prefab_obj, prefab, mut hierarchy) = open_prefab(assets, key)?;

    let mut root_of: HashMap<&str, usize> = HashMap::from([(root_item.id.as_str(), 0)]);
    let mut skipped = Vec::new();
    let mut remaining: Vec<_> = preset.items.iter().filter(|i| i.id != root_item.id).collect();
    // A weapon of one's own is the donor's bare prefab with the model's
    // own parts on its bones: none of its preset's items has a prefab to
    // hang.
    let custom = match db.custom.get(weapon_template) {
        Some(definition) => {
            remaining.clear();
            Some(std::sync::Arc::new(crate::custom::build(definition, &mut hierarchy).with_context(|| format!("building {}", definition.name))?))
        }
        None => None,
    };
    while !remaining.is_empty() {
        let before = remaining.len();
        let mut later = Vec::new();
        for part in remaining {
            let parent_root = part.parent.as_deref().and_then(|p| root_of.get(p).copied());
            let (Some(parent_root), Some(slot)) = (parent_root, part.slot.as_deref()) else {
                later.push(part);
                continue;
            };
            let item = db.item(&part.template)?;
            match attach_part(assets, &mut hierarchy, parent_root, slot, item.prefab_key()) {
                Ok(index) => {
                    root_of.insert(part.id.as_str(), index);
                }
                Err(e) => skipped.push(format!("{} in {slot}: {e:#}", item.name)),
            }
        }
        if later.len() == before {
            skipped.extend(later.iter().map(|p| format!("{}: parent was not placed", p.template)));
            break;
        }
        remaining = later;
    }

    let sum = |prop: &str| -> f32 {
        preset.items.iter().filter_map(|i| db.items.get(&i.template)?.prop_f32(prop)).sum()
    };
    let parts = || preset.items.iter().filter(|i| i.id != root_item.id).filter_map(|i| db.items.get(&i.template));
    let recoil_delta = parts().filter_map(|t| t.prop_f32("Recoil")).sum::<f32>() / 100.0;
    let magazine_capacity = preset
        .items
        .iter()
        .find(|i| i.slot.as_deref() == Some("mod_magazine"))
        .and_then(|i| db.items.get(&i.template))
        .and_then(|t| t.props.get("Cartridges")?.get(0)?.get("_max_count")?.as_u64())
        .map(|n| n as u32);
    // The game weighs a weapon with what is loaded in it. A recorded trace
    // gives the stock AKM as 3.867 kg: the parts, and 31 rounds.
    let round_weight = base
        .props
        .get("defAmmo")
        .and_then(|a| a.as_str())
        .and_then(|id| db.items.get(id))
        .and_then(|a| a.prop_f32("Weight"))
        .unwrap_or(0.0);
    let rounds = magazine_capacity.unwrap_or(0) as f32 + base.prop_f32("chamberAmmoCount").unwrap_or(0.0);
    // The weapon's own width, the longest reach of any part to each side,
    // and every part that adds its length whatever the others do.
    let (mut left, mut right, mut forced) = (0.0f32, 0.0f32, 0.0f32);
    for part in parts() {
        let (l, r) = (part.prop_f32("ExtraSizeLeft").unwrap_or(0.0), part.prop_f32("ExtraSizeRight").unwrap_or(0.0));
        if part.props.get("ExtraSizeForceAdd").and_then(|v| v.as_bool()).unwrap_or(false) {
            forced += l + r;
        } else {
            left = left.max(l);
            right = right.max(r);
        }
    }
    let cell_width = (base.prop_f32("Width").unwrap_or(1.0) + left + right + forced).max(1.0) as u32;

    // The stock that folds: the weapon's own fold if its template has one,
    // else the first part that has (a stock). It folds if what its folded
    // slot names is fitted and no part blocks folding.
    let flag = |props: &serde_json::Value, name: &str| props.get(name).and_then(|v| v.as_bool()).unwrap_or(false);
    let folds = |props: &serde_json::Value| flag(props, "Foldable") || flag(props, "Retractable");
    let text = |props: &serde_json::Value, name: &str| props.get(name).and_then(|v| v.as_str()).unwrap_or("").to_owned();
    let part_items: Vec<_> = preset.items.iter().filter(|i| i.id != root_item.id).collect();
    let template_of = |id: &str| preset.items.iter().find(|i| i.id == id).and_then(|i| db.items.get(&i.template));
    // Whether an item of the preset hangs below another, at any depth.
    let below = |item: &str, ancestor: &str| {
        let mut at = preset.items.iter().find(|i| i.id == item).and_then(|i| i.parent.clone());
        while let Some(parent) = at {
            if parent == ancestor {
                return true;
            }
            at = preset.items.iter().find(|i| i.id == parent).and_then(|i| i.parent.clone());
        }
        false
    };
    let folding = if folds(&base.props) {
        Some(root_item.id.clone())
    } else {
        part_items.iter().find(|i| template_of(&i.id).is_some_and(|t| folds(&t.props))).map(|i| i.id.clone())
    };
    let shoulder = |id: &str| template_of(id).is_some_and(|t| flag(&t.props, "HasShoulderContact"));
    let compact = flag(&base.props, "CompactHandling");
    let blocked = part_items.iter().any(|i| template_of(&i.id).is_some_and(|t| flag(&t.props, "BlocksFolding")));
    let fold = folding.as_deref().filter(|_| !blocked).and_then(|id| {
        let template = template_of(id)?;
        let slot = text(&template.props, "FoldedSlot");
        // What sits in the slot the fold names, if it names one.
        let folded_part = part_items.iter().find(|i| i.parent.as_deref() == Some(id) && i.slot.as_deref() == Some(slot.as_str()));
        if !slot.is_empty() && folded_part.is_none() {
            return None;
        }
        let reduce = template.prop_f32("SizeReduceRight").unwrap_or(0.0);
        let own = id == root_item.id;
        // Folded, the stock's own recoil is left out of the parts' share.
        let stock_recoil: f32 = part_items
            .iter()
            .filter_map(|i| template_of(&i.id))
            .filter(|t| db.is_a(t, "Stock"))
            .filter_map(|t| t.prop_f32("Recoil"))
            .sum();
        // And its reach to the right is shorter; what hangs in the folded
        // slot no longer counts for the size at all.
        let (mut left, mut right, mut forced) = (0.0f32, 0.0f32, if own { -reduce } else { 0.0 });
        for part in &part_items {
            if folded_part.is_some_and(|folded| part.id == folded.id || below(&part.id, &folded.id)) {
                continue;
            }
            let Some(t) = template_of(&part.id) else { continue };
            let (l, mut r) = (t.prop_f32("ExtraSizeLeft").unwrap_or(0.0), t.prop_f32("ExtraSizeRight").unwrap_or(0.0));
            if part.id == id {
                r -= reduce;
            }
            if flag(&t.props, "ExtraSizeForceAdd") {
                forced += l + r;
            } else {
                left = left.max(l);
                right = right.max(r);
            }
        }
        // Folded, neither the stock nor what its slot holds reaches the
        // shoulder.
        let reaches = part_items.iter().any(|part| {
            let folded_away = part.id == id || folded_part.is_some_and(|folded| part.id == folded.id || below(&part.id, &folded.id));
            !folded_away && shoulder(&part.id)
        });
        Some(Fold {
            recoil_delta: recoil_delta - stock_recoil / 100.0,
            cell_width: (base.prop_f32("Width").unwrap_or(1.0) + left + right + forced).max(1.0) as u32,
            held_closer: compact && !reaches,
        })
    });
    // Unfolded, a weapon whose own template folds with no slot named
    // always counts as reaching the shoulder.
    let own_fold_reaches = fold.is_some() && folds(&base.props) && text(&base.props, "FoldedSlot").is_empty();
    let held_closer = compact && !own_fold_reaches && !part_items.iter().any(|part| shoulder(&part.id));
    Ok(Weapon {
        template: weapon_template.to_owned(),
        name: base.name.clone(),
        hierarchy,
        prefab,
        prefab_file: prefab_obj.file,
        skipped,
        total_weight: sum("Weight") + rounds * round_weight,
        total_ergonomics: sum("Ergonomics"),
        recoil_delta,
        magazine_capacity,
        cell_width,
        held_closer,
        fold,
        custom,
    })
}

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

fn attach_part(
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
    let files = assets.load(key).with_context(|| format!("loading {key}"))?;
    let root = files
        .iter()
        .find_map(|&f| part_root(assets, f).transpose())
        .transpose()?
        .ok_or_else(|| anyhow!("{key} exports no prefab"))?;
    let mut part = read_hierarchy(assets, root)?;
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

/// The engine's `Quaternion.Euler` as `xyzw`: degrees about z, then about
/// x, then about y.
fn quat_of_euler(degrees: [f32; 3]) -> [f32; 4] {
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
fn part_root(assets: &Assets, file: usize) -> Result<Option<ObjRef>> {
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
