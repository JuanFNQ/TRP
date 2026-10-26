//! Building a weapon the way the game does: a base prefab plus one prefab
//! per installed part, each hung on the bone named after its slot.

use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};

use crate::database::{Database, Preset, PresetItem};
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
    /// The build's `SpeedFactor`: one plus the `Velocity` of the weapon and
    /// its parts over 100 (a round leaves at its initial speed times it).
    pub speed_factor: f32,
    /// Rounds the fitted magazine holds, if the build has one.
    pub magazine_capacity: Option<u32>,
    /// The fitted magazine's `magAnimationIndex` (which of the reload's
    /// and the look's clips play), if the build has one.
    pub magazine_animation: Option<i32>,
    /// How many inventory cells wide the build is. The body's animator
    /// picks how the weapon is carried by it.
    pub cell_width: u32,
    /// Whether the game holds the build closer to the body (a weapon of
    /// `CompactHandling` with nothing on it that reaches the shoulder).
    pub held_closer: bool,
    /// What the build is with its stock folded, if it has one that folds.
    pub fold: Option<Fold>,
    /// The bones the fold turns on the model of the thing that folds, as
    /// any of its models not animated shows it (`FoldableStockView`: an
    /// inventory picture, a thing on the ground).
    pub fold_bones: Vec<FoldBone>,
    /// For a weapon of one's own (`crate::custom`): the model's parts on
    /// the donor's rig and the edits to the donor's animation.
    pub custom: Option<std::sync::Arc<crate::custom::Build>>,
    /// Whether a part of the build silences it (`SilencerComponent`: a
    /// `Silencer`, or a barrel whose template `IsSilencer`): its shots
    /// sound from the silenced banks (`WeaponSoundPlayer.IsSilenced`).
    pub silenced: bool,
    /// Its kind of sound (`crate::sound::sound_weapon_kind`), for an item's
    /// weapon; none for a rig that is no item.
    pub sound_kind: Option<&'static str>,
    /// How it throws a spent case at a shot (`ShellExtractionData.
    /// ShotSettings`), and its shell port (`Firearms.
    /// SetupShellPortTransform`: the first object named `shellport`, on, in
    /// the weapon with its parts), where it has them.
    pub extraction: Option<Extraction>,
    pub shell_port: Option<usize>,
    /// Its muzzle flash, where it has one (`crate::muzzle`).
    pub muzzle: Option<crate::muzzle::MuzzleData>,
    /// The parts of the build that are sights, in the order the game walks
    /// the items (`crate::sight`), and the sights that fold out of the way.
    pub sights: Vec<crate::sight::SightPart>,
    pub folding_sights: Vec<crate::sight::FoldingSight>,
    /// The parts of the build that carry a light or a laser, in the order
    /// the game walks the items (`crate::tactical`).
    pub devices: Vec<crate::tactical::Device>,
    /// The build's bipod, where it has one (`FirearmController.Bipod`: the
    /// first part that is a `Bipod`; mounting spec, "The bipod").
    pub bipod: Option<BipodPart>,
    /// The parts that glow with its heat (`crate::heat`).
    pub hot: Vec<crate::heat::HotPart>,
}

/// A bipod in a build: its template, the node its model hangs from, the
/// `bipod_ground` bone below it if its model has one (the pivot of a
/// mount with it out, `ProceduralWeaponAnimation.FindBipodMountingPoint`),
/// and whether it folds forward (`BipodViewController.BipodToggleDirection`:
/// `Forward`, the Harris and Atlas kinds; `Backward` the rest).
#[derive(Debug, Clone, PartialEq)]
pub struct BipodPart {
    pub template: String,
    pub node: usize,
    pub ground: Option<usize>,
    pub forward: bool,
}

/// The direction a bipod's model folds, from its prefab's view
/// (`BipodViewController`, its serialised `BipodToggleDirection`: 0
/// forward, 1 backward). A model with no view folds backward, the enum's
/// second value being what the most bipods have (PROVISIONAL: a model
/// without the view was not met).
pub fn bipod_forward(assets: &Assets, files: &[usize]) -> Result<bool> {
    let Some(view) = files.iter().flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR)).find(|&obj| script_class(assets, obj).as_deref() == Some("BipodViewController")) else {
        return Ok(false);
    };
    let value = assets.read(view)?;
    let direction = value.get("<BipodToggleDirection>k__BackingField").or_else(|| value.get("BipodToggleDirection")).and_then(Value::as_i64).unwrap_or(1);
    Ok(direction == 0)
}

/// A weapon's throw of a spent case (`ShellExtractionData.
/// ExtractionSettings`): a range for each axis of the force added, one for
/// each axis of the turn, and the multiplier of the shell port's move.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Extraction {
    pub force: [[f32; 2]; 3],
    pub turn: [[f32; 2]; 3],
    pub multiplier: f32,
}

/// A bundle's shot settings for a spent case (`ShellExtractionData.
/// ShotSettings`); none where it has no such data.
pub fn extraction(assets: &Assets, files: &[usize]) -> Result<Option<Extraction>> {
    let Some(data) = files.iter().flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR)).find(|&obj| script_class(assets, obj).as_deref() == Some("ShellExtractionData")) else {
        return Ok(None);
    };
    let value = assets.read(data)?;
    let shot = value.req("ShotSettings")?;
    let range = |name: &str| -> [f32; 2] {
        let v = shot.get(name);
        let f = |axis: &str| v.and_then(|v| v.get(axis)).and_then(Value::as_f32).unwrap_or(0.0);
        [f("x"), f("y")]
    };
    Ok(Some(Extraction {
        force: [range("ShellsForceXRange"), range("ShellsForceYRange"), range("ShellsForceZRange")],
        turn: [range("ShellsRotationRangeX"), range("ShellsRotationRangeY"), range("ShellsRotationRangeZ")],
        multiplier: shot.get("ShellForceMultiplier").and_then(Value::as_f32).unwrap_or(0.0),
    }))
}

/// A round's calibre's class for its case's sounds (`AmmoPoolObject.
/// _caliber`: 0 for 9 mm, 1 for 12 gauge, 2 for 5.56 mm, 3 heavy, 4 none),
/// from its bundle's files.
pub fn round_caliber(assets: &Assets, files: &[usize]) -> Option<u8> {
    let script = files.iter().flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR)).find(|&obj| script_class(assets, obj).as_deref() == Some("AmmoPoolObject"))?;
    assets.read(script).ok()?.get("_caliber").and_then(Value::as_i64).and_then(|caliber| u8::try_from(caliber).ok())
}

/// The first object named `shellport` that is on, with all above it
/// (`TransformTools.FindActiveTransformRecursive`).
pub fn shell_port(hierarchy: &Hierarchy) -> Option<usize> {
    let on = |mut index: usize| loop {
        if !hierarchy.nodes[index].active {
            return false;
        }
        match hierarchy.nodes[index].parent {
            Some(parent) => index = parent,
            None => return true,
        }
    };
    (0..hierarchy.nodes.len()).find(|&index| hierarchy.nodes[index].name == "shellport" && on(index))
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

/// A bundle's `WeaponPrefab` and the tree of the object it names: a
/// weapon's, the empty hands', an item's hands for using it (medicine,
/// food: `UsableHandsPrefab`, a `WeaponPrefab` with the bone the item hangs
/// from besides), or a grenade's (`GrenadePrefab`, a `WeaponPrefab` with
/// the grenade thrown and the parts let go of in the throw besides).
fn open_prefab(assets: &mut Assets, key: &str) -> Result<(ObjRef, Value, Hierarchy)> {
    let files = assets.load(key)?;
    let (prefab_obj, prefab) = files
        .iter()
        .flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR))
        .find(|&obj| matches!(script_class(assets, obj).as_deref(), Some("WeaponPrefab" | "UsableHandsPrefab" | "GrenadePrefab")))
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
    let files = assets.load(key)?;
    let extraction = extraction(assets, &files)?;
    let shell_port = shell_port(&hierarchy);
    let muzzle = crate::muzzle::read(assets, &files, &hierarchy)?;
    Ok(Weapon {
        extraction,
        shell_port,
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
        hot: Vec::new(),
    })
}

/// A weapon by its item template, or a weapon-like prefab by its bundle
/// (anything with a `/` in it).
pub fn assemble_named(assets: &mut Assets, db: &Database, what: &str) -> Result<Weapon> {
    if what.contains('/') { assemble_prefab(assets, what) } else { assemble(assets, db, what) }
}

/// Assembles the stock build of a weapon from the user's install.
pub fn assemble(assets: &mut Assets, db: &Database, weapon_template: &str) -> Result<Weapon> {
    let preset = db.default_preset(weapon_template)?.clone();
    assemble_preset(assets, db, &preset)
}

/// A weapon by its build's key (`build_key`), or by its template alone
/// for its stock build.
pub fn assemble_key(assets: &mut Assets, db: &Database, key: &str) -> Result<Weapon> {
    match preset_of_key(key) {
        Some(preset) => assemble_preset(assets, db, &preset),
        None => assemble(assets, db, key),
    }
}

/// What opens a build's parts in its key, closes them, and parts them.
const KEY_OPEN: char = '[';
const KEY_CLOSE: char = ']';
const KEY_NEXT: char = ',';
const KEY_SLOT: char = '=';

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

/// A build out of its key (`build_key`): its items numbered in the order
/// they are named, the weapon first. None for a key with no parts' list
/// (a template alone) or one not well made.
pub fn preset_of_key(key: &str) -> Option<Preset> {
    fn item(text: &str, at: &mut usize, parent: Option<(String, String)>, items: &mut Vec<PresetItem>) -> Option<()> {
        let rest = &text[*at..];
        let end = rest.find(KEY_OPEN)?;
        let template = rest[..end].to_owned();
        if template.is_empty() || template.contains([KEY_CLOSE, KEY_NEXT, KEY_SLOT]) {
            return None;
        }
        *at += end + 1;
        let id = items.len().to_string();
        let (parent, slot) = parent.map_or((None, None), |(parent, slot)| (Some(parent), Some(slot)));
        items.push(PresetItem { id: id.clone(), template, parent, slot });
        loop {
            match text[*at..].chars().next()? {
                KEY_CLOSE => {
                    *at += 1;
                    return Some(());
                }
                KEY_NEXT => *at += 1,
                _ => {
                    let rest = &text[*at..];
                    let equals = rest.find(KEY_SLOT)?;
                    let slot = rest[..equals].to_owned();
                    *at += equals + 1;
                    item(text, at, Some((id.clone(), slot)), items)?;
                }
            }
        }
    }
    if !key.contains(KEY_OPEN) {
        return None;
    }
    let mut items = Vec::new();
    let mut at = 0;
    item(key, &mut at, None, &mut items)?;
    if at != key.len() {
        return None;
    }
    Some(Preset { id: key.to_owned(), name: String::new(), root: "0".to_owned(), items, encyclopedia: None })
}

/// Assembles a build of a weapon: its root item and each part with the
/// slot of its parent it is in (a preset's list: the stock one, another of
/// the install's, or a build out of its key).
pub fn assemble_preset(assets: &mut Assets, db: &Database, preset: &Preset) -> Result<Weapon> {
    let root_item = preset
        .items
        .iter()
        .find(|i| i.id == preset.root)
        .ok_or_else(|| anyhow!("preset {} has no root item", preset.id))?;
    let weapon_template = root_item.template.as_str();
    let base = db.item(&root_item.template)?;
    let key = base.prefab_key().ok_or_else(|| anyhow!("{} has no prefab", base.name))?;
    let (prefab_obj, prefab, mut hierarchy) = open_prefab(assets, key)?;

    let mut root_of: HashMap<&str, usize> = HashMap::from([(root_item.id.as_str(), 0)]);
    let mut skipped = Vec::new();
    let mut remaining: Vec<_> = preset.items.iter().filter(|i| i.id != root_item.id).collect();
    // A weapon of one's own is the donor's bare prefab with the model's
    // own parts on its bones: none of its preset's items is drawn. Its
    // magazine's own prefab is hung as the game hangs it, the model's
    // magazine below it, for what the game goes by on a magazine's object
    // (where its rounds are drawn), and another weapon's muzzle effects
    // are laid at its fireport where its file names one (`crate::custom`).
    let mut muzzle_key = None;
    let custom = match db.custom.get(weapon_template) {
        Some(definition) => {
            let magazine = remaining.iter().find(|item| item.slot.as_deref() == Some("mod_magazine")).map(|item| item.template.clone());
            remaining.clear();
            let mut hung = Vec::new();
            if let Some(magazine) = magazine {
                let item = db.item(&magazine)?;
                match attach_part(assets, &mut hierarchy, 0, "mod_magazine", item.prefab_key()) {
                    Ok(root) => hung.push(root),
                    Err(e) => skipped.push(format!("{} in mod_magazine: {e:#}", item.name)),
                }
            }
            let build = crate::custom::build(definition, &mut hierarchy, &hung).with_context(|| format!("building {}", definition.name))?;
            if let Some(from) = &definition.muzzle_from {
                let other = assemble(assets, db, from).with_context(|| format!("{}'s muzzle", definition.name))?;
                crate::custom::take_muzzle(&other.hierarchy, &mut hierarchy).with_context(|| format!("{}'s muzzle from {}", definition.name, other.name))?;
                let preset = db.default_preset(from)?;
                let root = preset.items.iter().find(|item| item.id == preset.root).ok_or_else(|| anyhow!("preset {} has no root item", preset.id))?;
                muzzle_key = db.item(&root.template)?.prefab_key().map(str::to_owned);
            }
            Some(std::sync::Arc::new(build))
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
    let magazine_animation = preset
        .items
        .iter()
        .find(|i| i.slot.as_deref() == Some("mod_magazine"))
        .and_then(|i| db.items.get(&i.template))
        .and_then(|t| t.props.get("magAnimationIndex")?.as_i64())
        .map(|n| n as i32);
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
    // The bones a fold turns: the fold's view on the model of the thing
    // that folds, the weapon's own or a part's (`FoldableStockView` is set
    // up with its own model's item, and does nothing where that item has
    // no fold: the M3's weapon has one that names its stock, which is the
    // stock's to fold), found below that model's root by name.
    let mut fold_bones = Vec::new();
    if let Some((id, key)) = folding.as_deref().and_then(|id| Some((id, template_of(id)?.prefab_key()?.to_owned()))) {
        if let Some(&root) = root_of.get(id) {
            let files = assets.load(&key)?;
            // A bone not on the model is passed over, as the game's view
            // passes it (the SCAR-L's names its stocks of both colours).
            for (name, folded, unfolded) in fold_view(assets, &files)? {
                if let Some(node) = hierarchy.find_below(root, &name) {
                    fold_bones.push(FoldBone { node, folded, unfolded });
                }
            }
        }
    }
    let own_fold_reaches = fold.is_some() && folds(&base.props) && text(&base.props, "FoldedSlot").is_empty();
    let held_closer = compact && !own_fold_reaches && !part_items.iter().any(|part| shoulder(&part.id));
    let silenced = parts().any(|t| db.is_a(t, "Silencer") || flag(&t.props, "IsSilencer"));
    let files = assets.load(key)?;
    let extraction = extraction(assets, &files)?;
    let shell_port = shell_port(&hierarchy);
    let muzzle = match &muzzle_key {
        Some(other) => {
            let theirs = assets.load(other)?;
            crate::muzzle::read(assets, &theirs, &hierarchy)?
        }
        None => crate::muzzle::read(assets, &files, &hierarchy)?,
    };
    let (sights, folding_sights) = crate::sight::read(assets, db, preset, &root_of, &hierarchy)?;
    let devices = crate::tactical::read(assets, db, preset, &root_of, &hierarchy)?;
    // The bipod: the first part of the build, in the game's walk of the
    // slots, that is one (`WeaponModified`: `Item.AllSlots` where the
    // contained item is a `Bipod`).
    let mut bipod = None;
    for item in crate::sight::all_slots_order(db, preset) {
        let Some(template) = db.items.get(&item.template) else { continue };
        if !db.is_a(template, "Bipod") {
            continue;
        }
        let Some(&node) = root_of.get(item.id.as_str()) else { continue };
        let forward = match template.prefab_key() {
            Some(key) => {
                let files = assets.load(key)?;
                bipod_forward(assets, &files)?
            }
            None => false,
        };
        bipod = Some(BipodPart { template: item.template.clone(), node, ground: hierarchy.find_below(node, "bipod_ground"), forward });
        break;
    }
    let hot = crate::heat::read(assets, &hierarchy);
    Ok(Weapon {
        bipod,
        hot,
        sights,
        folding_sights,
        devices,
        extraction,
        shell_port,
        muzzle,
        template: weapon_template.to_owned(),
        name: base.name.clone(),
        hierarchy,
        prefab,
        prefab_file: prefab_obj.file,
        skipped,
        total_weight: sum("Weight") + rounds * round_weight,
        total_ergonomics: sum("Ergonomics"),
        recoil_delta,
        speed_factor: 1.0 + sum("Velocity") / 100.0,
        magazine_capacity,
        magazine_animation,
        cell_width,
        held_closer,
        fold,
        fold_bones,
        custom,
        silenced,
        sound_kind: Some(crate::sound::sound_weapon_kind(db, base)),
    })
}

/// A bone a fold turns (`FoldableStockView.BonePosition`): the node, and
/// its own turn folded and unfolded (`xyzw`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FoldBone {
    pub node: usize,
    pub folded: [f32; 4],
    pub unfolded: [f32; 4],
}

impl Weapon {
    /// The tree as a model not animated shows it with its fold folded or
    /// not: each bone the fold turns given that turn
    /// (`FoldableStockView.Init`, at once as it is set up).
    pub fn fold_tree(&mut self, folded: bool) {
        for bone in &self.fold_bones {
            if let Some(node) = self.hierarchy.nodes.get_mut(bone.node) {
                node.rotation = if folded { bone.folded } else { bone.unfolded };
            }
        }
    }
}

/// A bundle's fold view (`FoldableStockView._bonePositions`): each bone's
/// name and its turn folded and unfolded. None where it has no such view.
pub fn fold_view(assets: &Assets, files: &[usize]) -> Result<Vec<(String, [f32; 4], [f32; 4])>> {
    let Some(view) = files.iter().flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR)).find(|&obj| script_class(assets, obj).as_deref() == Some("FoldableStockView")) else {
        return Ok(Vec::new());
    };
    let value = assets.read(view)?;
    // A turn of all noughts, as the 9A-91's view has for its unfolded
    // stock, is taken as no turn at all (`PROVISIONAL`: what the engine
    // makes of it is not read; the model lies so as made).
    let quat = |v: Option<&Value>| -> [f32; 4] {
        let f = |name: &str| v.and_then(|v| v.get(name)).and_then(Value::as_f32).unwrap_or(0.0);
        let q = [f("x"), f("y"), f("z"), f("w")];
        let length = q.iter().map(|k| k * k).sum::<f32>().sqrt();
        if length < 1e-6 { [0.0, 0.0, 0.0, 1.0] } else { q.map(|k| k / length) }
    };
    Ok(value
        .req("_bonePositions")?
        .as_array()
        .unwrap_or(&[])
        .iter()
        .filter_map(|bone| Some((bone.get("BoneName")?.as_str()?.to_owned(), quat(bone.get("FoldedRotation")), quat(bone.get("UnfoldedRotation")))))
        .collect())
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
    attach_at(assets, hierarchy, bone, key)
}

/// Hangs the part a bundle holds on that object of the hierarchy, as the
/// game hangs a part on its slot's object, and gives the index of its root.
pub(crate) fn attach_at(assets: &mut Assets, hierarchy: &mut Hierarchy, bone: usize, key: &str) -> Result<usize> {
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
