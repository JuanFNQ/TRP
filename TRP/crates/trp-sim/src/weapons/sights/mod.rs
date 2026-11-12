//! The sights a weapon aims by, as the game picks and works them.
//! After the optics spec, "The sights a weapon has", "Zoom and modes", "Aiming through it",
//! and the weapon-handling spec, "The sights": every aim bone of a sight part a
//! way to aim, the current one (`AimIndex`), the key to the next
//! (`ChangeAimScope`), a scope's modes and the key through them
//! (`ChangeAimScopeMagnification`), a variable scope's zoom
//! (`ScopeZoomHandler`), what aiming through an optic does to the main
//! camera and to the eye's place, and the sensitivity the sight gives.
//! And zeroing ("Zeroing"): the keys through a scope's distances, the
//! points on the bullet's path, the turn of the optics' cameras, the
//! collimators and a part turned by distance toward them, a launcher's
//! line of sight, the shot's squeeze, and what the battle screen is told.
//! Nothing here is drawn: the turns are kept on the weapon's tree at rest
//! for a host to put on its own.

use trp_data::formats::curve::Curve;
use trp_data::weapons::sight::{AdjustableOptic, FoldingSight, SightPart};
use trp_data::unity::extract::Hierarchy;

use crate::combat::ballistics::{Round, Trajectory};
use crate::weapons::hands::SightLine;
use crate::base::math::{quat_euler, quat_look, quat_mul, quat_rotate, quat_to_euler};
use crate::base::rig::WeaponRig;

mod zoom_handler;
mod making;
mod optics;
mod switching;
mod zeroing;
mod turning;
mod kept;

pub use zoom_handler::*;
pub use zeroing::*;
pub use turning::*;
pub use kept::*;
use zoom_handler::{Edge};
use turning::{inverse_lerp, lerp, move_towards};

type V3 = [f32; 3];
const NO_TURN: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// How often a zeroing point's flight is sampled, seconds
/// (`CreateOpticCalibrationData`'s `deltaTime`).
const POINT_STEP: f32 = 0.001;
/// The samples a zeroing point is looked for in at most: a round that never
/// gets so far (none does with gravity) would hold the game; this stops.
const POINT_SAMPLES: usize = 1_000_000;
/// A collimator's object, turned to look at the point, then about its x
/// (`CollimatorSight.TransformLocalRotation`).
const COLLIMATOR_TURN: [f32; 3] = [-90.0, 0.0, 0.0];

/// What the sights tell the battle screen (`BattleUIScreen`; optics spec,
/// "Zeroing", the HUD): a zeroing distance for the ammunition's panel
/// (`ShowAmmoCountZeroingPanel`); a magnification for the optic's panel
/// (`_opticCratePanel.Show`, "x" and the figure), or that panel hidden.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SightHud {
    Zeroing(i32),
    Magnification(f32),
    HideMagnification,
}

/// A sight looking within this many degrees of straight ahead does
/// (the hard settings' `SCOPE_ROTATION_THRESHOLD`).
pub const SCOPE_ROTATION_THRESHOLD: f32 = 1.0;
/// How far from straight back an aim bone may be turned and still be used,
/// in degrees (`ScopeRotationWithinLimits`).
const SIGHT_TURN_LIMIT: f32 = 55.0;
/// The main camera's field of view through an optic, degrees
/// (`ProceduralWeaponAnimation.OnAimOrPoseChanged`).
pub const OPTIC_FIELD_OF_VIEW: f32 = 35.0;
/// How far behind an iron sight's aim plane the eye is drawn at a wide
/// field of view, at most (`FindCameraDepth`): `(1 - s) * 0.075`.
const IRON_EYE: f32 = 0.075;
const SIGHT_AIM_BONE: &str = "mod_aim_camera";
const WEAPON_AIM_BONE: &str = "aim_camera";
/// A slot whose part's bones are its own sight's, not the one it hangs on.
const SCOPE_SLOT: &str = "mod_scope";
const WEAPON_BONE: &str = "Weapon_root/Weapon_root_anim/weapon";
const MOVED_BONE: &str = "Weapon_root/Weapon_root_anim";

/// One way to aim (`ProceduralWeaponAnimation.SightNBone`): its aim bone;
/// the sight part it is (the item's `SightComponent`), and the part whose
/// prefab's modes it goes by (`ScopePrefabCache`), where it has them; how
/// many bones below its slot it lies; how far it is turned from straight
/// ahead, degrees; whether the iron sights' line is taken for it; whether
/// it is a mode's optic bone; an eyecup's shift; whether its bone is the
/// weapon's own `aim_camera`, whose line the eye is put on by its -y where
/// another's is its -z (`crate::rig::SightNodes::line_in_moved`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sight {
    pub bone: usize,
    pub part: Option<usize>,
    pub cache: Option<usize>,
    pub depth: usize,
    pub rotation: f32,
    pub align: bool,
    pub optic_bone: bool,
    pub eye_guard_shift: f32,
    pub own: bool,
}

/// Where the eye goes along the barrel while aiming (`FindCameraDepth`):
/// how deep ahead of the moved bone the sight's plane is, metres, and the
/// figure that, times `s - 1` (the squeeze of a wide field of view), moves
/// it along the view: an optic's `DistanceToCamera` with what its bone
/// lies past the far plane, an iron sight's `-0.075`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SightEye {
    pub depth: f32,
    pub distance: f32,
}

impl SightEye {
    /// An iron sight's or a collimator's at that depth.
    pub fn iron(depth: f32) -> Self {
        Self { depth, distance: -IRON_EYE }
    }

    /// The shift along the view at the weapon's squeeze `scale`.
    pub fn compensation(&self, scale: f32) -> f32 {
        self.distance * (scale - 1.0)
    }
}

/// The weapon prefab's aim planes (`WeaponPrefab.DefaultAimPlane`,
/// `FarPlane`, `CustomAimPlanes`), and how far along y the weapon's own
/// bone sits in the moved one (`HandsContainer.Weapon.localPosition.y`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AimPlanes {
    pub default: f32,
    pub far: f32,
    pub custom: Vec<(String, f32)>,
    pub weapon_y: f32,
}

impl AimPlanes {
    /// A weapon's, from its prefab and its tree.
    pub fn of(weapon: &trp_data::weapons::weapon::Weapon) -> Self {
        let depth = |name: &str| weapon.prefab.get(name).and_then(|plane| plane.get("Depth")).and_then(trp_data::unity::Value::as_f32).unwrap_or(0.0);
        let custom = weapon
            .prefab
            .get("CustomAimPlanes")
            .and_then(trp_data::unity::Value::as_array)
            .unwrap_or(&[])
            .iter()
            .filter_map(|plane| Some((plane.get("Name")?.as_str()?.to_owned(), plane.get("Depth")?.as_f32()?)))
            .collect();
        let h = &weapon.hierarchy;
        let weapon_y = (0..h.nodes.len()).find(|&i| h.path(i) == WEAPON_BONE).map_or(0.0, |i| h.nodes[i].translation[1]);
        Self { default: depth("DefaultAimPlane"), far: depth("FarPlane"), custom, weapon_y }
    }
}

/// What a sight item keeps (`SightComponent`): its scope, each scope's
/// mode and zeroing index (`ScopesCurrentCalibPointIndexes`), its zoom
/// (`ScopeZoomValue`, the field of view; 0 until it is first set); and what
/// its prefab shows: its mode's object (`ScopePrefabCache.CurrentModeId`)
/// and its zoom handler.
#[derive(Debug, Clone, PartialEq)]
pub struct SightState {
    pub scope: usize,
    pub modes: Vec<usize>,
    pub zeroing: Vec<usize>,
    pub zoom_value: f32,
    pub prefab_mode: usize,
    pub zoom: Option<ZoomHandler>,
}

/// A weapon's sights. What the game keeps with the items (the weapon's
/// `AimIndex`, each sight's scope, modes, zeroing and zoom) is worked
/// here while the weapon is in hand, given by its items as it is taken
/// (`take_kept`) and read back for them as it changes (`kept`).
#[derive(Debug, Clone)]
pub struct Sights {
    parts: Vec<SightPart>,
    states: Vec<SightState>,
    list: Vec<Sight>,
    aim_index: usize,
    /// A sight part looks straight ahead: the iron sights are not used, and
    /// the backup sights that fold do.
    straight: bool,
    /// The two points of the iron sights' line, when they are used
    /// (`method_15`), the weapon's own bone and its muzzle.
    line: Option<[usize; 2]>,
    weapon: usize,
    muzzle: Option<usize>,
    folding: Vec<(usize, bool)>,
    planes: AimPlanes,
    /// Each sight's depth along the barrel (`AimTransformDepth`).
    depths: Vec<f32>,
    /// The squeeze of the weapon at the field of view (`_compensatoryScale`)
    /// and the eye as last worked out (`FindCameraDepth`).
    scale: f32,
    eye: SightEye,
    /// The weapon's tree at rest, and on it the turns zeroing has given
    /// its sights' objects, each kept as the game's transforms keep it
    /// until the next; the objects turned, in the order first turned.
    rig: WeaponRig,
    turned: Vec<(usize, [f32; 4])>,
    /// The fireport the zeroing point hangs from (`HandsContainer.
    /// Fireport`: the first below the moved bone, depth first).
    fireport: Option<usize>,
    /// A part whose optic is live whatever sight is in use, set by a
    /// controller that switches its optic itself (the rangefinder's
    /// `_opticSight.enabled`: rangefinder spec).
    forced_optic: Option<usize>,
    /// Each part's scopes' zeroing points (`OpticCalibrationPoints`), in the
    /// order of the scope's distances as written; none where it has none.
    points: Vec<Vec<Vec<V3>>>,
    /// A launcher (`AdjustCollimatorsToTrajectory`).
    launcher: bool,
    /// The weapon motion's own: the calibration asked for
    /// (`_updateCalibration`), the local point (`_localCalibratePoint`),
    /// the shot's squeeze (`ShotNeedsFovAdjustments`), a launcher's line of
    /// sight (`_lineOfSightDeltaAngle`, `_cameraShiftToLineOfSight`), and
    /// whether the eye is the character's own (`FirstPersonPointOfView`).
    /// `PROVISIONAL`: kept with the weapon's sights, where the game's weapon
    /// motion is the player's and keeps its local point from one weapon to
    /// the next (a launcher's first aimed frame measures its line with it).
    calibration_asked: bool,
    local_point: V3,
    shot_squeeze: bool,
    line_of_sight: SightLine,
    first_person: bool,
    /// What the battle screen is told, until a host takes it.
    hud: Vec<SightHud>,
    /// The weapon's own ways to aim, their depths and its iron sights' line,
    /// set aside while an underbarrel launcher's sight is the only one
    /// (`set_grenade`).
    grenade_kept: Option<(Vec<Sight>, Vec<f32>, Option<[usize; 2]>)>,
}

impl Sights {
}
