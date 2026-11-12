//! An underbarrel grenade launcher in the hands.
//! After `docs/spec/underbarrel-launcher.md`: the switch to it and back by the weapon's slot key (`ToggleLauncherOperation`),
//! its rest (`LauncherIdling`), its shot (`LauncherFire`), its reload by the key
//! (`LauncherReload`), its round put in or taken out from the inventory or by the
//! chamber's key (`LoadAmmoToUnderbarrelWeapon`, `DischargeAmmoFromUnderbarrelWeapon`),
//! its looks (`LauncherUtilityOperation`) and its range.
//!
//! The launcher is a weapon of its own in the model, with its own animator. While it
//! is in use that animator is the hands' and the source of every mark; the rifle's
//! goes on running beside it, its marks heard by nobody.

use std::sync::Arc;

use anyhow::Result;
use trp_data::unity::Assets;
use trp_data::weapons::weapon::Weapon;

use super::{Event, Firearm, Malf, Operation, MAX_UTILITY_TIME};
use crate::base::marks::Mark;
use crate::weapons::weapon_animator::{WeaponAnimator, WeaponAnimatorData};

mod fitting;
mod operating;
mod update;


/// The ranges a launcher's sight is set to, in metres (`Launcher.SightingRange`: one
/// list in the game's code for every launcher; the templates' own figures are not
/// read).
pub const RANGES: [i32; 7] = [100, 150, 200, 250, 300, 350, 400];

/// How fast the animator's range follows the one set, metres a second
/// (`Launcher.RANGE_UP_SPEED`).
const RANGE_SPEED: f32 = 400.0;

/// The share of the player's velocity a spent case thrown in the launcher's reload
/// or a look is given (`LauncherReload.OnShellEjectEvent`, `LauncherUtilityOperation.
/// OnShellEjectEvent`: `StartSpawnShell(velocity * 0.33)`).
const RELOAD_CASE_SHARE: f32 = 0.33;

/// What happens to a launcher, for the host (`Event::Launcher`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LauncherEvent {
    /// A switch has begun, to the launcher or back. Going to it, the aim point is the
    /// launcher's from now (`IsGrenadeLauncher` set at the press) and the weapon's
    /// bones the launcher's (`UpdateLauncherBones`); blind fire is off.
    SwitchBegun(bool),
    /// The switch is over, on the launcher or back on the rifle: the weapon's handling
    /// and recoil are the side's it ended on (`InitWeaponData`), and coming back the
    /// aim point and the bones are the rifle's again (its folding sights redone).
    Switched(bool),
    /// The round has left the launcher at its `FiringBullet`: the grenade made from
    /// its `fireport`, its muzzle effects and its shot sound. `case_kept` that the
    /// spent case stays in it (a launcher whose rounds leave one).
    Shot { case_kept: bool },
    /// The recoil of the launcher's shot, a frame after it (`bool_6`, given to the
    /// weapon's motion at the next late update), as a rifle's (`Event::Shot`).
    Recoil,
    /// A round within reach has gone into the launcher's chamber: the reload's, as it
    /// begins (`LauncherReload.Prepare`).
    RoundTaken,
    /// The round's model shown in the launcher (`MagShow`).
    RoundShown,
    /// The spent case's model thrown out from the launcher's shell port, at that
    /// share of the player's velocity (a `ShellEject` of the reload or of a look:
    /// `UnderbarrelContainer.StartSpawnShell`).
    CaseThrown(f32),
    /// The spent case's model taken away (a round loaded from the inventory: `RemoveShell`).
    CaseRemoved,
    /// The launcher's round taken out, its model gone (`DelAmmoChamber`).
    RoundRemoved,
    /// A round put in or taken out from the inventory or by the chamber's key is over:
    /// the inventory's move completes now.
    MoveDone,
    /// The range's step has changed: the figure the zeroing panel shows
    /// (`ShowAmmoCountZeroingPanel`).
    Range(i32),
    /// The reload key found no round the launcher takes within reach (`ReloadMagNotFound`).
    NoRounds,
}

/// A launcher's animator, read once with the build: its controller (its
/// `WeaponPrefab`'s), the weapon's bones it poses, and whether its rounds leave no
/// case in it.
#[derive(Clone)]
pub struct LauncherData {
    animator: WeaponAnimatorData,
    nodes: Arc<Vec<usize>>,
    caseless: bool,
    bones: LauncherBones,
}

/// The launcher's bones that the weapon's motion, the arms and the shot go by
/// while it is in use, as the build's nodes (underbarrel-launcher spec, "While
/// the launcher is in use"): the prop bone the rifle is carried by (its
/// `AlternativePropBone`, `Weapon_anim_override`: `UpdateLauncherBones`,
/// `UpdateWeaponBoneByLauncherWeaponBone`), its sight's aim bone
/// (`FindUnderbarrelWeaponSight`: the first `mod_aim_camera` below its root), the
/// left elbow's goal (`GatherUnderbarrelWeaponIK`: its `Bend_Goal_Left`) and its
/// muzzle (`UnderbarrelContainer.Init`: its `fireport`), each the first of its name
/// below the launcher's root, depth first.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LauncherBones {
    pub prop: Option<usize>,
    pub sight: Option<usize>,
    pub goal: Option<usize>,
    pub fireport: Option<usize>,
}

impl LauncherBones {
    fn find(hierarchy: &trp_data::unity::extract::Hierarchy, root: usize) -> Self {
        let nodes = &hierarchy.nodes;
        // Depth first, a node's children in the tree's order (`FindTransformRecursive`).
        fn walk(nodes: &[trp_data::unity::extract::Node], at: usize, name: &str) -> Option<usize> {
            for kid in (at + 1..nodes.len()).filter(|&i| nodes[i].parent == Some(at)) {
                if nodes[kid].name == name {
                    return Some(kid);
                }
                if let Some(found) = walk(nodes, kid, name) {
                    return Some(found);
                }
            }
            None
        }
        let first = |name: &str| walk(nodes, root, name);
        Self { prop: first("Weapon_anim_override"), sight: first("mod_aim_camera"), goal: first("Bend_Goal_Left"), fireport: first("fireport") }
    }
}

impl LauncherData {
    /// The build's launcher, where it has one with a controller.
    pub fn load(assets: &Assets, weapon: &Weapon) -> Result<Option<Self>> {
        let Some(part) = &weapon.launcher else { return Ok(None) };
        let Some(controller) = part.controller else { return Ok(None) };
        let (animator, nodes) = WeaponAnimatorData::load_part(assets, weapon, part.node, controller)?;
        let bones = LauncherBones::find(&weapon.hierarchy, part.node);
        Ok(Some(Self { animator, nodes: Arc::new(nodes), caseless: part.caseless, bones }))
    }

    /// Its bones in the build.
    pub fn bones(&self) -> LauncherBones {
        self.bones
    }
}

/// The launcher on the weapon, as the hands have it.
#[derive(Clone)]
pub(crate) struct Launcher {
    pub(super) animator: WeaponAnimator,
    nodes: Arc<Vec<usize>>,
    caseless: bool,
    bones: LauncherBones,
    /// A live round in its chamber, and a spent case (`ShellsInChambers`).
    round: bool,
    case: bool,
    /// The round's model in its chamber and the spent case's at its shell port
    /// (`UnderbarrelContainer._ammoObjectInUnderbarrel`, `_shellObjectInUnderbarrel`),
    /// which its marks move on their own: the round's from the reload's `MagShow`,
    /// the case's from the shot to the `ShellEject` that throws it.
    round_drawn: bool,
    case_drawn: bool,
    /// The weapon item's mark of being on its launcher (`IsUnderBarrelDeviceActive`,
    /// kept with the item).
    pub(super) marked: bool,
    /// Its animator is the hands' (`SetUnderBarrelDeviceActive`): from the switch's
    /// start going in to its end coming back. It runs only then.
    pub(super) speaks: bool,
    /// The range's step, the range set and the animator's (`RangeIndex`,
    /// `RangeValue`, `LastRangeValue`).
    step: usize,
    range: i32,
    shown_range: i32,
    /// A shot was made whose recoil the next frame gives.
    recoil_due: bool,
    /// What a switch to the launcher does at its end, for a round moved from the
    /// inventory with the rifle in use: load it (true) or take it out (false).
    then: Option<bool>,
    /// The marks its animator delivered last frame.
    marks_seen: Vec<Mark>,
}

impl Firearm {
}
