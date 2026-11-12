//! The sights fitted to an assembled weapon.
//! After the optics spec, "The sights a weapon has", "A scope's parts": each part whose item is a sight
//! (`SightComponent`), with its item's figures and what its prefab says of
//! its modes (`ScopePrefabCache`, `OpticSight`, `ScopeCameraData`,
//! `ScopeSmoothCameraData`), and the backup sights that fold
//! (`AutoFoldableSight`).

use std::collections::HashMap;

use anyhow::Result;
use serde_json::Value as Json;

use crate::formats::curve::Curve;
use crate::setup::database::{Database, Preset, PresetItem};
use crate::unity::extract::Hierarchy;
use crate::unity::{class, Assets, ObjRef, Value};

mod sight_read;
mod lenses;

pub use sight_read::*;
pub use lenses::*;
use lenses::{angle_by_range, lens_nodes, lod_bias_factor, own_nodes, part_nodes, read_reticle, zoom_ring};

/// One scope of a sight item (the template's lists, a scope each): how
/// many modes it has, and for each mode its magnification (`Zooms`, which
/// only the HUD shows; a variable scope's one mode lists its range), its
/// aiming sensitivity (`AimSensitivity`) and its zeroing distances
/// (`CalibrationDistances`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScopeFigures {
    pub modes: usize,
    pub zooms: Vec<f32>,
    pub sensitivity: Vec<f32>,
    pub calibration: Vec<i32>,
}

/// A variable scope's figures (`IsAdjustableOptic`): its fields of view
/// widest and narrowest and the zoom's speed (`MinMaxFov`, which the game
/// takes over its prefab's), how much a turn of the wheel adds to the
/// zoom's velocity (`ZoomSensitivity`), and the aiming sensitivity at the
/// widest and at the narrowest (`AdjustableOpticSensitivity`, `..Max`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdjustableOptic {
    pub fov: [f32; 2],
    pub speed: f32,
    pub zoom_sensitivity: f32,
    pub sensitivity: [f32; 2],
}

/// One mode of a sight's prefab (`ScopePrefabCache.ScopeModeInfo`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PrefabMode {
    /// The mode's own object, on while the mode is.
    pub node: Option<usize>,
    pub optic: Option<OpticMode>,
    /// The object of its `CollimatorSight`, a red dot's or a holographic's.
    pub collimator: Option<usize>,
    pub ignore_optics_for_camera_plane: bool,
}

/// A mode's `OpticSight`: its object; the aim bone it names
/// (`ScopeTransform`); how far behind that bone the eye sits
/// (`DistanceToCamera`); a fixed scope's field of view for the mode (the
/// `ScopeCameraData` below it), and the object that carries that data,
/// which is where the optic camera stands.
#[derive(Debug, Clone, PartialEq)]
pub struct OpticMode {
    pub node: usize,
    pub bone: Option<usize>,
    pub distance_to_camera: f32,
    pub field_of_view: Option<f32>,
    pub camera: Option<usize>,
    /// The optic camera's near plane for the mode (its camera data's;
    /// a variable scope's, its root's), where it gives one.
    pub near: Option<f32>,
    /// The factor its camera puts on the levels of detail's bias, where its
    /// camera data turns the camera's own `CameraLodBiasController` on
    /// (`CameraLodBiasController`, `LodBiasFactor`; a variable scope's, its
    /// root's: `OpticComponentUpdater.CopyComponentFromOptic`). None: the
    /// bias as it stands (third-person spec, "Levels of detail").
    pub lod_bias_factor: Option<f32>,
    /// The object of its lens (`LensRenderer`), whose material is dimmed
    /// while the mode is not live.
    pub lens: Option<usize>,
    /// Its picture's chromatic aberration (`ScopeEffectsData`:
    /// `ChromaticAberrationShift` where `ChromaticAberration` is on).
    pub aberration: f32,
    /// A variable scope's reticle mesh, drawn into the picture
    /// (`ScopeData.Reticle`, a `ScopeReticle`).
    pub reticle: Option<std::sync::Arc<ScopeReticle>>,
    /// The thermal scope whose camera loses its roll as it is turned to
    /// the zeroing point (`IsThermalSightAvailableAt45Degrees`: the T12's).
    pub thermal_at_45: bool,
}

/// A part its prefab turns by the zeroing distance (`ScopePrefabCache.
/// CanChangeAngleByDistance`): the object turned (`WeaponScopeAxis`) and its
/// table of distances and angles about x, degrees (`AngleByRange`).
#[derive(Debug, Clone, PartialEq)]
pub struct AngleByRange {
    pub axis: usize,
    pub table: Vec<(f32, f32)>,
}

/// A variable scope's ring turned by its zoom (`ScopeZoomHandler.method_11`):
/// the ring (`ScopeSwitcher`), its own turns at the widest and the narrowest
/// view (`StartPivot`, `EndPivot`, its siblings, so their local turns stand
/// for the world ones the game blends), and whether it turns the long way
/// where that is the way the pivots stand (`InverseRotation`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoomRing {
    pub node: usize,
    pub start: [f32; 4],
    pub end: [f32; 4],
    pub inverse: bool,
}

/// A reticle drawn into the optic camera's picture (`ScopeReticle`,
/// `OpticRetrice`; optics spec, "The lens"): its mesh's first part (the
/// vertices, their colours, the triangles, the first blend shape's full
/// move), its place and turn (Euler degrees) about the optic camera, its
/// stored scale, and its material's `_Color`.
#[derive(Debug, Clone, PartialEq)]
pub struct ScopeReticle {
    pub positions: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub triangles: Vec<u32>,
    pub shape: Option<Vec<[f32; 3]>>,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: f32,
    pub color: [f32; 4],
}

/// A piece of a sight part drawn by one of the sights' own shaders, by its
/// material's shader (optics spec, "The lens"): its object, the material
/// slot (the submesh: the Tango's glass is its body's second), its
/// material, and what kind it is.
#[derive(Debug, Clone)]
pub struct LensNode {
    pub node: usize,
    pub slot: usize,
    pub material: ObjRef,
    pub kind: LensKind,
    /// Its material's pictures that its shader reads (`_MarkTex`,
    /// `_MaskTex`, `_MaskTex2`; a collimator's `_MarkTex`, `_NoiseTex`,
    /// `_FadeTex`), read with the build: a host draws from them.
    pub pictures: Vec<(String, std::sync::Arc<crate::unity::extract::TextureData>)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LensKind {
    /// A scope's lens (`CW FX/OpticSight`).
    Optic(OpticLens),
    /// A red dot's or a holographic sight's (`CW FX/Collimator`).
    Collimator(CollimatorLens),
    /// The front glass: a scope's (`Custom/OpticGlass`), or a red dot's
    /// window (`CollimatorGlass`, transparent, its queue 3000: the
    /// PK-06's).
    Glass,
    /// The back of the lens (`CW FX/BackLens`).
    Back,
}

/// A scope's lens: its material's figures (`_Scales`, `_Shifts`,
/// `_ShiftDirection`, `_NormalHideness`, `_MarkLightness`, read as the
/// game's Y-up left-handed vectors) and its mesh's two rings' radii (the
/// inner ring's, whose uv radius is 0.4725, and the rim's), metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpticLens {
    pub scales: [f32; 2],
    pub shifts: [f32; 2],
    pub shift_direction: [f32; 3],
    pub normal_hideness: f32,
    pub mark_lightness: f32,
    pub inner_radius: f32,
    pub outer_radius: f32,
}

/// A collimator's lens: its mark's colour, `_HDR`, `_MarkScale` and
/// `_MarkShift` (the game's axes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollimatorLens {
    pub color: [f32; 4],
    pub hdr: f32,
    pub mark_scale: f32,
    pub mark_shift: [f32; 3],
}

/// One part of the build that is a sight.
#[derive(Debug, Clone)]
pub struct SightPart {
    /// The preset's id of the item, and its template's.
    pub item: String,
    pub template: String,
    /// The part's own root, and the bone of the slot it hangs in.
    pub root: usize,
    pub slot_bone: usize,
    /// An iron sight (`IronSight`), and whether it hangs on a part that is
    /// itself a sight (a backup sight on a scope's rail).
    pub iron: bool,
    pub on_sight: bool,
    pub scopes: Vec<ScopeFigures>,
    pub adjustable: Option<AdjustableOptic>,
    /// The aim plane of the weapon's prefab the sight names, if any (the
    /// T12 thermal's).
    pub custom_aim_plane: String,
    /// Its prefab's modes; none where its root has no `ScopePrefabCache`.
    pub modes: Vec<PrefabMode>,
    pub has_prefab_cache: bool,
    /// A variable scope's `ScopeZoomHandler`, on its root: its curve of the
    /// wheel's target (`ScopeSmoothCameraData.FieldOfViewCurve`). (Every
    /// sight's root is given the `SightModVisualControllers` that switch
    /// its modes when it is made: `WeaponModPoolObject`.)
    pub zoom_curve: Option<Curve>,
    /// And its reticle's blend by the zoom (`ReticleBlendCurve`).
    pub reticle_curve: Option<Curve>,
    /// How far an eyecup on it moves the eye back (`EyeGuardComponent.
    /// ShiftsAimCamera` of a part below it), in metres.
    pub eye_guard_shift: f32,
    /// Its pieces drawn by the sights' own shaders: lenses, collimators,
    /// the glass and the back of the lens (not those of a part hung on it).
    pub lenses: Vec<LensNode>,
    /// The part its prefab turns by the zeroing distance, where it does.
    pub angle_by_range: Option<AngleByRange>,
    /// A variable scope's ring its zoom turns, where its prefab names one.
    pub zoom_ring: Option<ZoomRing>,
}

/// A sight that folds out of the way (`AutoFoldableSight`): its object,
/// and whether it is shown when no sight part looks straight ahead (its
/// `Mode` `On`; `Off` shows it only beside one that does).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FoldingSight {
    pub node: usize,
    pub mode_on: bool,
}
