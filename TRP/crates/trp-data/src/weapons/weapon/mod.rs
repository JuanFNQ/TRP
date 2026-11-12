//! A weapon assembled as the game does it: a base prefab and one prefab for each part.
//!
//! Building a weapon the way the game does: a base prefab plus one prefab
//! per installed part, each hung on the bone named after its slot.

use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};

use crate::setup::database::{Database, Preset, PresetItem};
use crate::unity::extract::{exported_game_objects, read_hierarchy, script_class, Hierarchy};
use crate::unity::{class, Assets, ObjRef, Value};

mod moving_parts;
mod assembly;
mod presets;
mod folding;
mod attaching;

pub use moving_parts::*;
pub use assembly::*;
pub use presets::*;
pub use folding::*;
pub use attaching::*;
use moving_parts::{launcher_controller};
use assembly::{KEY_CLOSE, KEY_NEXT, KEY_OPEN, KEY_SLOT, is_weapon_prefab_script, open_prefab, vest_linked};
use attaching::{attach_part};

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
    /// Whether the fitted magazine is one that comes out (its
    /// `ReloadMagType` `ExternalMagazine`), if the build has one: the
    /// Model 700's are, on a weapon whose own `ReloadMode` is
    /// `InternalMagazine` (`Weapon.ReloadMode`).
    pub magazine_external: Option<bool>,
    /// The largest `UniqueAnimationModID` of the build's parts
    /// (`FirearmController.SetMaxUniqueAnimationModId`): a part with its
    /// own animations (the PP-19's left-side charging handle, the BAD
    /// lever, a side-folding stock); 0 for none.
    pub unique_animation: i32,
    /// How many inventory cells wide the build is. The body's animator
    /// picks how the weapon is carried by it.
    pub cell_width: u32,
    /// Whether the rig names its vest's marker among the bones the player
    /// takes from it (`vest_linked`): only then does a character seen from
    /// outside reach for the vest.
    pub vest_linked: bool,
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
    pub custom: Option<std::sync::Arc<crate::weapons::custom::Build>>,
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
    /// How it throws a live round out of its shell port (`ShellExtractionData.
    /// PatronExtractionSettings`, `Firearms.ThrowPatronAsLoot`: a round the
    /// action, a reload or a cylinder's reload takes out), beside the case's.
    pub patron_extraction: Option<Extraction>,
    pub shell_port: Option<usize>,
    /// The shell ports of a weapon of several chambers, by the chambers'
    /// numbers (`shell_ports`); none for any other.
    pub shell_ports: Vec<usize>,
    /// Its muzzle flash, where it has one (`crate::muzzle`).
    pub muzzle: Option<crate::weapons::muzzle::MuzzleData>,
    /// The parts of the build that are sights, in the order the game walks
    /// the items (`crate::sight`), and the sights that fold out of the way.
    pub sights: Vec<crate::weapons::sight::SightPart>,
    pub folding_sights: Vec<crate::weapons::sight::FoldingSight>,
    /// The parts of the build that carry a light or a laser, in the order
    /// the game walks the items (`crate::tactical`).
    pub devices: Vec<crate::weapons::tactical::Device>,
    /// The build's bipod, where it has one (`FirearmController.Bipod`: the
    /// first part that is a `Bipod`; mounting spec, "The bipod").
    pub bipod: Option<BipodPart>,
    /// The build's underbarrel launcher, where it has one
    /// (`Weapon.GetUnderbarrelWeapon`; underbarrel-launcher spec).
    pub launcher: Option<LauncherPart>,
    /// That launcher's own muzzle flash, read from its bundle over its own
    /// tree (`FirearmsEffects.Init` on its `WeaponPrefab`; `crate::muzzle::
    /// read_under`), where it has one.
    pub launcher_muzzle: Option<crate::weapons::muzzle::MuzzleData>,
    /// The parts that glow with its heat (`crate::heat`).
    pub hot: Vec<crate::weapons::heat::HotPart>,
}
