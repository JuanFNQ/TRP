//! Launchers, bipods, parts' clip binds, extraction and shell ports.

use super::*;

/// An underbarrel launcher in a build (underbarrel-launcher spec, "The
/// launchers and where they fit"): the first part, in the game's walk of
/// the slots, that is a `Launcher`, whatever its slot is called
/// (`Weapon.GetUnderbarrelWeapon`). Its template; the weapon template its
/// `LinkedWeapon` names, which holds its figures as a weapon (recoil,
/// spread, calibre: `Launcher.WeaponTemplate`); the node its model hangs
/// from; its bundle; the controller of its own animator, its
/// `WeaponPrefab`'s (`_originalAnimatorController`), which takes the
/// rifle's place while the launcher is in use; and whether its rounds
/// leave no case in it (`UseAmmoWithoutShell`: the GP-25's, not the
/// M203's).
#[derive(Debug, Clone, PartialEq)]
pub struct LauncherPart {
    pub template: String,
    pub linked: Option<String>,
    pub node: usize,
    pub bundle: Option<String>,
    pub controller: Option<crate::unity::ObjRef>,
    pub caseless: bool,
    /// Its own fireport and shell port below its node (`FindFireport`,
    /// `InitTransformsForAmmo`: the first of each name, depth first): where
    /// its grenade leaves, and where its spent case is kept and thrown from.
    pub fireport: Option<usize>,
    pub shell_port: Option<usize>,
    /// Its chamber's point below its node (`InitTransformsForAmmo`: the first
    /// `patron_in_weapon`, depth first), where its round's model is put; and
    /// how its spent case is thrown from its shell port (its own bundle's
    /// `ShellExtractionData.ShotSettings`: `UnderbarrelContainer.Setup`), where
    /// it has them.
    pub chamber: Option<usize>,
    pub extraction: Option<Extraction>,
    /// The share of recoil the build's stock parts give (`Weapon.
    /// StockRecoilDelta`: the `Stock` parts' `Recoil` summed, over a hundred),
    /// which the launcher's recoil takes beside its own parts' (none:
    /// `ShotEffector.OnWeaponParametersChanged`).
    pub stock_recoil_delta: f32,
}

/// The controller of a launcher's own animator: its bundle's
/// `WeaponPrefab`'s `_originalAnimatorController`.
pub(super) fn launcher_controller(assets: &Assets, files: &[usize]) -> Option<crate::unity::ObjRef> {
    let prefab = files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).find(|&obj| is_weapon_prefab_script(assets, obj))?;
    let value = assets.read(prefab).ok()?;
    value.get("_originalAnimatorController").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(prefab.file, p))
}

/// A bipod in a build: its template, the node its model hangs from, the
/// `bipod_ground` bone below it if its model has one (the pivot of a
/// mount with it out, `ProceduralWeaponAnimation.FindBipodMountingPoint`),
/// and whether it folds forward (`BipodViewController.BipodToggleDirection`:
/// `Forward`, the Harris and Atlas kinds; `Backward` the rest); and the
/// bundle its prefab is in, loaded with the build (its `BipodAudioController`:
/// `trp_data::sound::bipod_sounds`).
#[derive(Debug, Clone, PartialEq)]
pub struct BipodPart {
    pub template: String,
    pub node: usize,
    pub ground: Option<usize>,
    pub forward: bool,
    pub bundle: Option<String>,
    /// The controller of its own animator (on its prefab's root: the legs
    /// folding and unfolding, fed `Bipod`, with the marks its sound player
    /// plays by), where it has one.
    pub animator: Option<crate::unity::ObjRef>,
}

/// The controller of a part's own animator: the first `Animator` of its
/// prefab's files that has one.
pub fn part_animator(assets: &Assets, files: &[usize]) -> Option<crate::unity::ObjRef> {
    files.iter().flat_map(|&file| assets.objects_of(file, class::ANIMATOR)).find_map(|animator| {
        let value = assets.read(animator).ok()?;
        value.get("m_Controller").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(animator.file, p))
    })
}

/// A curve of a part's own clip and what it moves: the hash of the path
/// it binds below the animator (`unity::anim::path_hash`), the node of
/// the part's prefab with that path, if any, and the property (a bone's
/// place, turn, scale or angles; `None` for a property of a component).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PartBind {
    pub path: u32,
    pub node: Option<usize>,
    pub property: Option<crate::unity::anim::Property>,
}

/// What a part's own animator moves (`probe partbinds`): the tree of the
/// part's prefab, and each clip of its controller by name with what each
/// of its curves binds. The animator stands on the prefab's root (every
/// bipod's does), so a binding's path is a node's path in the tree. A
/// part with no animator of its own has no clips.
pub fn part_clip_binds(assets: &mut Assets, key: &str) -> Result<(Hierarchy, Vec<(String, Vec<PartBind>)>)> {
    let tree = prefab_tree(assets, key)?;
    let files = assets.load(key)?;
    let Some(controller) = part_animator(assets, &files) else {
        return Ok((tree, Vec::new()));
    };
    let by_hash: HashMap<u32, usize> = (0..tree.nodes.len()).map(|index| (crate::unity::anim::path_hash(&tree.path(index)), index)).collect();
    let mut clips = Vec::new();
    for (name, obj) in crate::unity::anim::controller_clips(assets, controller)? {
        let clip = crate::unity::anim::Clip::read(assets, obj)?;
        let tracks = clip.tracks.iter().map(|t| (t.path, Some(t.property)));
        let others = clip.others.iter().map(|b| (b.path, None));
        let binds = tracks.chain(others).map(|(path, property)| PartBind { path, node: by_hash.get(&path).copied(), property }).collect();
        clips.push((name, binds));
    }
    Ok((tree, clips))
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

/// A bundle's settings for what leaves its shell port (`ShellExtractionData`):
/// a spent case's at a shot (`ShotSettings`) and a live round's thrown out
/// (`PatronExtractionSettings`); none where it has no such data.
pub fn extraction(assets: &Assets, files: &[usize]) -> Result<Option<(Extraction, Extraction)>> {
    let Some(data) = files.iter().flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR)).find(|&obj| script_class(assets, obj).as_deref() == Some("ShellExtractionData")) else {
        return Ok(None);
    };
    let value = assets.read(data)?;
    let settings = |name: &str| -> Result<Extraction> {
        let settings = value.req(name)?;
        let range = |name: &str| -> [f32; 2] {
            let v = settings.get(name);
            let f = |axis: &str| v.and_then(|v| v.get(axis)).and_then(Value::as_f32).unwrap_or(0.0);
            [f("x"), f("y")]
        };
        Ok(Extraction {
            force: [range("ShellsForceXRange"), range("ShellsForceYRange"), range("ShellsForceZRange")],
            turn: [range("ShellsRotationRangeX"), range("ShellsRotationRangeY"), range("ShellsRotationRangeZ")],
            multiplier: settings.get("ShellForceMultiplier").and_then(Value::as_f32).unwrap_or(0.0),
        })
    };
    // A bundle with no round's settings throws a round with nothing added
    // (Unity's default for a field its file lacks).
    let patron = settings("PatronExtractionSettings").unwrap_or_default();
    Ok(Some((settings("ShotSettings")?, patron)))
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

/// The shell ports of a weapon of several chambers (`Firearms.
/// SetupShellPortTransform`, `TransformTools.FindChildsByNameRecNoneAlloc`;
/// more-weapons spec, "The rounds and cases seen in chambers of their
/// own"): where more than one object's name has `patron_in_weapon`, every
/// object whose name has `shellport`, depth first in file order, none
/// looked for inside one found, on or off. None for any other weapon, whose
/// port is `shell_port`.
pub fn shell_ports(hierarchy: &Hierarchy) -> Vec<usize> {
    let nodes = &hierarchy.nodes;
    if nodes.iter().filter(|node| node.name.contains("patron_in_weapon")).count() < 2 {
        return Vec::new();
    }
    let mut children = vec![Vec::new(); nodes.len()];
    for (index, node) in nodes.iter().enumerate() {
        if let Some(parent) = node.parent {
            children[parent].push(index);
        }
    }
    let mut found = Vec::new();
    let mut stack: Vec<usize> = (0..nodes.len()).filter(|&index| nodes[index].parent.is_none()).rev().collect();
    while let Some(index) = stack.pop() {
        if nodes[index].name.contains("shellport") {
            found.push(index);
            continue;
        }
        stack.extend(children[index].iter().rev());
    }
    found
}
