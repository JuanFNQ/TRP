//! The sights a weapon aims by, as the game picks and works them (optics
//! spec, "The sights a weapon has", "Zoom and modes", "Aiming through it";
//! weapon-handling spec, "The sights"): every aim bone of a sight part a
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

use trp_data::curve::Curve;
use trp_data::sight::{AdjustableOptic, FoldingSight, SightPart};
use trp_data::unity::extract::Hierarchy;

use crate::ballistics::{Round, Trajectory};
use crate::hands::SightLine;
use crate::math::{quat_euler, quat_look, quat_mul, quat_rotate, quat_to_euler};
use crate::rig::WeaponRig;

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
/// (`EFTHardSettings.SCOPE_ROTATION_THRESHOLD`).
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
    pub fn of(weapon: &trp_data::weapon::Weapon) -> Self {
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

/// Which end of its range a variable scope is at (`ESmoothScopeState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edge {
    Widest,
    Narrowest,
    Between,
}

/// A variable scope's zoom (`ScopeZoomHandler`): its field of view, the
/// target it moves to, the velocity the wheel builds up and the place the
/// wheel has asked for, the last input, and whether it has arrived. Its
/// range and speed are the item's (`MinMaxFov`), not its prefab's.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoomHandler {
    data: AdjustableOptic,
    curve: Curve,
    fov: f32,
    target: f32,
    velocity: f32,
    wanted: f32,
    input: f32,
    last_input: f32,
    arrived: bool,
    edge: Edge,
}

impl ZoomHandler {
    /// Set up on the item's kept zoom (`Init`): none kept is the widest.
    fn new(data: AdjustableOptic, curve: Curve, kept: &mut f32) -> Self {
        let [widest, narrowest] = data.fov;
        if *kept == 0.0 {
            *kept = widest;
        }
        let fov = kept.clamp(narrowest, widest);
        let mut handler = Self { data, curve, fov, target: fov, velocity: 0.0, wanted: fov, input: 0.0, last_input: 0.0, arrived: false, edge: Edge::Between };
        // (The game's `Init` tells the screen of an end the zoom starts
        // away from its widest at; the app's start at the widest.)
        handler.note_edge();
        handler
    }

    /// The scope's field of view now, degrees.
    pub fn field_of_view(&self) -> f32 {
        self.fov
    }

    /// How far toward the narrowest it is, 0 to 1 (`ZoomFactor`).
    fn factor(&self) -> f32 {
        inverse_lerp(self.data.fov[0], self.data.fov[1], self.fov)
    }

    /// The aiming sensitivity it gives (`method_2`): from
    /// `AdjustableOpticSensitivity` at the widest to its `Max` at the
    /// narrowest.
    pub fn sensitivity(&self) -> f32 {
        lerp(self.data.sensitivity[0], self.data.sensitivity[1], self.factor())
    }

    /// One frame of the wheel (`method_4`): `input` is the wheel's turn
    /// times the options' `OpticSensitivity` (up positive), `dt` the
    /// frame's time. The zoom kept with the item follows. Whether the zoom
    /// came to an end of its range or left one.
    fn update(&mut self, input: f32, dt: f32, kept: &mut f32) -> bool {
        let [widest, narrowest] = self.data.fov;
        self.input = input;
        if input != 0.0 {
            // The wheel turned back while the zoom was on its way stops it
            // where it is; else the velocity grows by the turn and the
            // wanted place moves by the velocity (`method_5`).
            let reversed = (input > 0.0 && self.last_input < 0.0) || (input < 0.0 && self.last_input > 0.0);
            if reversed && !self.arrived && self.edge == Edge::Between {
                self.velocity = 0.0;
                self.wanted = self.fov;
            } else {
                self.velocity += input * self.data.zoom_sensitivity;
                self.wanted -= self.velocity;
            }
            self.wanted = self.wanted.clamp(narrowest, widest);
            let along = inverse_lerp(narrowest, widest, self.wanted);
            let shaped = self.curve.evaluate(along);
            self.target = ((widest - narrowest).abs() * shaped + narrowest).clamp(narrowest, widest);
            self.last_input = input;
        }
        self.arrived = (self.fov - self.target).abs() < 0.0001;
        if self.arrived {
            self.velocity = 0.0;
            false
        } else {
            let step = self.data.speed * self.velocity.abs() * dt;
            self.fov = move_towards(self.fov, self.target, step).clamp(narrowest, widest);
            *kept = self.fov;
            self.note_edge()
        }
    }

    /// The magnification key's snap (`ForceChangeScopeState`): to the end
    /// of the range farther from where it is. Whether its end changed.
    fn snap(&mut self, kept: &mut f32) -> bool {
        let [widest, narrowest] = self.data.fov;
        self.fov = if (self.fov - widest).abs() > (self.fov - narrowest).abs() { widest } else { narrowest };
        self.target = self.fov;
        self.wanted = self.fov;
        *kept = self.fov;
        self.note_edge()
    }

    /// At an end the wheel's run is forgotten (`method_9`, `method_8`).
    /// Whether it came to an end or left one (`OnSmoothScopeStateChanged`).
    fn note_edge(&mut self) -> bool {
        let [widest, narrowest] = self.data.fov;
        let was = self.edge;
        self.edge = if self.fov == widest {
            Edge::Widest
        } else if self.fov == narrowest {
            Edge::Narrowest
        } else {
            Edge::Between
        };
        if self.edge != Edge::Between {
            (self.velocity, self.input, self.last_input) = (0.0, 0.0, 0.0);
        }
        self.edge != was
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

/// A weapon's sights. `PROVISIONAL`: what the game keeps with the items
/// (the weapon's `AimIndex`, each sight's scope, modes and zoom) is kept
/// here, with the rig of the build, from the first draw (where the game
/// would have them as the items last stood); a weapon's rig is kept while
/// it is carried, so they last as long as it does.
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
}

impl Sights {
    /// A weapon's, as built.
    pub fn of_weapon(weapon: &trp_data::weapon::Weapon) -> Option<Self> {
        Self::new(&weapon.hierarchy, weapon.sights.clone(), &weapon.folding_sights, AimPlanes::of(weapon))
    }

    /// Finds them (`FindAimTransforms`) on a weapon's tree with its sight
    /// parts. None for a tree with no aim bone at all.
    pub fn new(hierarchy: &Hierarchy, parts: Vec<SightPart>, folding: &[FoldingSight], planes: AimPlanes) -> Option<Self> {
        let nodes = &hierarchy.nodes;
        let weapon = (0..nodes.len()).find(|&i| hierarchy.path(i) == WEAPON_BONE)?;
        let moved = (0..nodes.len()).find(|&i| hierarchy.path(i) == MOVED_BONE);
        let rig = WeaponRig::new(hierarchy);
        let children = |of: usize| (0..nodes.len()).filter(move |&i| nodes[i].parent == Some(of));
        let below = |mut index: usize, top: usize| {
            let mut depth = 0;
            while index != top {
                index = nodes[index].parent?;
                depth += 1;
            }
            Some(depth)
        };
        let active = |mut index: usize| loop {
            if !nodes[index].active {
                return false;
            }
            match nodes[index].parent {
                Some(parent) => index = parent,
                None => return true,
            }
        };
        // How a bone is turned about the weapon's upright axis, from
        // straight back (`GetRotationRelativeToParent(...).eulerAngles.y`).
        let turned = |index: usize| {
            let (_, weapon_turn) = rig.placed(weapon);
            let (_, turn) = rig.placed(index);
            let back = [-weapon_turn[0], -weapon_turn[1], -weapon_turn[2], weapon_turn[3]];
            quat_to_euler(quat_mul(back, turn))[1].rem_euclid(360.0)
        };
        let within = |index: usize| (180.0 - turned(index)).abs() < SIGHT_TURN_LIMIT;
        // The first bone of that name below a bone, depth first, the bone
        // itself left out (`TransformTools.FindTransformRecursive`).
        let first_named = |top: usize, name: &str| -> Option<usize> {
            fn walk(nodes: &[trp_data::unity::extract::Node], at: usize, name: &str) -> Option<usize> {
                for kid in (0..nodes.len()).filter(|&i| nodes[i].parent == Some(at)) {
                    if nodes[kid].name == name {
                        return Some(kid);
                    }
                    if let Some(found) = walk(nodes, kid, name) {
                        return Some(found);
                    }
                }
                None
            }
            walk(nodes, top, name)
        };
        // The sight parts by their items' names (the template's id: `Item.
        // Name` is its localization key), the game's walk kept for alike.
        let mut by_name: Vec<usize> = (0..parts.len()).collect();
        by_name.sort_by(|&a, &b| parts[a].template.cmp(&parts[b].template));
        let mut list: Vec<Sight> = Vec::new();
        let mut straight = false;
        for &p in &by_name {
            let part = &parts[p];
            if part.iron && !part.on_sight {
                continue;
            }
            // Every bone whose name has `mod_aim_camera`, breadth first
            // from the slot's bone, on only, not into a `mod_scope` below.
            let mut found = Vec::new();
            let mut queue = std::collections::VecDeque::from([part.slot_bone]);
            while let Some(at) = queue.pop_front() {
                if nodes[at].name.contains(SIGHT_AIM_BONE) && active(at) {
                    found.push(at);
                }
                queue.extend(children(at).filter(|&kid| nodes[kid].name != SCOPE_SLOT));
            }
            // The prefab's modes are the slot's first child's.
            let cache = children(part.slot_bone).next().and_then(|first| parts.iter().position(|q| q.root == first && q.has_prefab_cache));
            for bone in found {
                let depth = below(bone, part.slot_bone).unwrap_or(0);
                if let Some(known) = list.iter_mut().find(|sight| sight.bone == bone) {
                    if depth < known.depth {
                        known.part = Some(p);
                    }
                    continue;
                }
                if !within(bone) {
                    continue;
                }
                let rotation = 180.0 - turned(bone);
                if rotation.abs() < SCOPE_ROTATION_THRESHOLD {
                    straight = true;
                }
                let optic_bone = cache.is_some_and(|c| parts[c].modes.iter().any(|mode| mode.optic.as_ref().is_some_and(|optic| optic.bone == Some(bone))));
                let own = nodes[bone].name == WEAPON_AIM_BONE;
                list.push(Sight { bone, part: Some(p), cache, depth, rotation, align: false, optic_bone, eye_guard_shift: if optic_bone { part.eye_guard_shift } else { 0.0 }, own });
            }
        }
        let folding = folding.iter().map(|fold| (fold.node, fold.mode_on ^ straight)).collect();
        let deepest = |part: &str| {
            let mut best: Option<(usize, usize)> = None;
            for index in (0..nodes.len()).filter(|&i| nodes[i].name.contains(part)) {
                let Some(depth) = below(index, weapon) else { continue };
                if best.is_none_or(|(_, d)| depth > d) {
                    best = Some((index, depth));
                }
            }
            best.map(|(index, _)| index)
        };
        let mut line = None;
        // The iron sights and the weapon's own bones are listed with their
        // line taken and no turn noted.
        let plain = |bone: usize| Sight { bone, part: None, cache: None, depth: 0, rotation: 0.0, align: true, optic_bone: false, eye_guard_shift: 0.0, own: nodes[bone].name == WEAPON_AIM_BONE };
        if !straight {
            // The iron sights: the first, by name, with an aim bone in
            // bounds; and the line through the rear and front points.
            for &p in by_name.iter().filter(|&&p| parts[p].iron) {
                if let Some(bone) = first_named(parts[p].slot_bone, SIGHT_AIM_BONE).filter(|&bone| within(bone)) {
                    list.push(Sight { part: Some(p), ..plain(bone) });
                    break;
                }
            }
            line = deepest("mod_align_rear").zip(deepest("mod_align_front")).map(|(rear, front)| [rear, front]);
        }
        // Else the first sight's bone anywhere on the weapon, else the
        // weapon's own.
        if list.is_empty() {
            if let Some(bone) = moved.and_then(|moved| first_named(moved, SIGHT_AIM_BONE)).filter(|&bone| within(bone)) {
                list.push(plain(bone));
            }
        }
        if list.is_empty() {
            if let Some(bone) = moved.and_then(|moved| first_named(moved, WEAPON_AIM_BONE)) {
                list.push(plain(bone));
            }
        }
        if list.is_empty() {
            return None;
        }
        let depths = list
            .iter()
            .map(|sight| {
                let (at, turn) = rig.placed(weapon);
                let back = [-turn[0], -turn[1], -turn[2], turn[3]];
                let there = rig.placed(sight.bone).0;
                -quat_rotate(back, std::array::from_fn(|k| there[k] - at[k]))[1]
            })
            .collect();
        // Each sight item's state as made, and its prefab's mode
        // (`SightModVisualControllers.OnEnable`: the first scope whose
        // modes are as many as the prefab's, else the first mode).
        let states = parts
            .iter()
            .map(|part| {
                let mut state = SightState { scope: 0, modes: vec![0; part.scopes.len()], zeroing: vec![0; part.scopes.len()], zoom_value: 0.0, prefab_mode: 0, zoom: None };
                if let (Some(data), Some(curve)) = (part.adjustable, part.zoom_curve.clone()) {
                    state.zoom = Some(ZoomHandler::new(data, curve, &mut state.zoom_value));
                }
                state
            })
            .collect();
        let muzzle = (0..nodes.len()).find(|&i| nodes[i].name == "fireport");
        let fireport = moved.and_then(|moved| first_named(moved, "fireport"));
        let points = parts.iter().map(|part| vec![Vec::new(); part.scopes.len()]).collect();
        let mut sights = Self {
            parts,
            states,
            list,
            aim_index: 0,
            straight,
            line,
            weapon,
            muzzle,
            folding,
            planes,
            depths,
            scale: 1.0,
            eye: SightEye::iron(0.0),
            rig,
            turned: Vec::new(),
            fireport,
            forced_optic: None,
            points,
            launcher: false,
            calibration_asked: false,
            local_point: [0.0; 3],
            shot_squeeze: true,
            line_of_sight: SightLine::default(),
            first_person: true,
            hud: Vec::new(),
        };
        for part in 0..sights.parts.len() {
            sights.show_mode(part);
        }
        sights.work_out_eye();
        Some(sights)
    }

    /// The ways to aim, in the order the key goes through them.
    pub fn list(&self) -> &[Sight] {
        &self.list
    }

    /// Whether a sight part looks straight ahead (the iron sights are then
    /// left out, and the backup sights that fold do).
    pub fn straight(&self) -> bool {
        self.straight
    }

    /// The sight parts, in the order the game walks the items.
    pub fn parts(&self) -> &[SightPart] {
        &self.parts
    }

    /// The weapon's `AimIndex`.
    pub fn aim_index(&self) -> usize {
        self.aim_index
    }

    /// Whether the game's camera code takes the sight's figures
    /// (`AimIndex < ScopeAimTransforms.Count`).
    pub fn in_range(&self) -> bool {
        self.aim_index < self.list.len()
    }

    /// The sight aimed by (`CurrentScope`).
    pub fn current(&self) -> &Sight {
        &self.list[self.aim_index % self.list.len()]
    }

    /// How far the weapon is turned about its barrel while aiming by the
    /// sight in use, degrees (`UpdateCurrentScopeRotation`): the sight's own
    /// turn from straight ahead, where it is at least the threshold (a
    /// canted sight's: the PK-06 on a 45 degree mount), else none. The game
    /// works it out as the sights are found and as they change, which is
    /// when the sight in use can change.
    pub fn scope_rotation(&self) -> f32 {
        if self.list.is_empty() {
            return 0.0;
        }
        let rotation = self.current().rotation;
        if rotation.abs() >= SCOPE_ROTATION_THRESHOLD {
            rotation
        } else {
            0.0
        }
    }

    /// A sight item's state.
    pub fn state(&self, part: usize) -> &SightState {
        &self.states[part]
    }

    /// Whether a sight is aimed through as an optic (`SightNBone.IsOptic`):
    /// its prefab's mode now has an `OpticSight`, the bone is that optic's,
    /// and its item names no aim plane of its own.
    pub fn is_optic(&self, sight: &Sight) -> bool {
        let Some(cache) = sight.cache else { return false };
        let has = self.parts[cache].modes.get(self.states[cache].prefab_mode).is_some_and(|mode| mode.optic.is_some());
        has && sight.optic_bone && sight.part.is_none_or(|p| self.parts[p].custom_aim_plane.is_empty())
    }

    /// The current sight is an optic.
    pub fn optic(&self) -> bool {
        self.is_optic(self.current())
    }

    /// The main camera's field of view while aiming: 35 degrees through an
    /// optic, the setting less 15 over anything else.
    pub fn aimed_field_of_view(&self, setting: f32) -> f32 {
        if self.optic() {
            OPTIC_FIELD_OF_VIEW
        } else {
            setting - crate::hands::AIM_FOV_DROP
        }
    }

    /// The current sight's scope and mode, its item's (where it has one).
    pub fn scope_and_mode(&self) -> Option<(usize, usize)> {
        let p = self.current().part?;
        let state = &self.states[p];
        Some((state.scope, state.modes.get(state.scope).copied().unwrap_or(0)))
    }

    /// Its magnification for the HUD (`GetCurrentOpticZoom`).
    pub fn zoom(&self) -> Option<f32> {
        let (scope, mode) = self.scope_and_mode()?;
        self.parts[self.current().part?].scopes.get(scope)?.zooms.get(mode).copied()
    }

    /// The zoom its item keeps (`ScopeZoomValue`): a variable scope's field
    /// of view, 0 on anything else.
    pub fn zoom_value(&self) -> f32 {
        self.current().part.map_or(0.0, |p| self.states[p].zoom_value)
    }

    /// The figure its item has for its scope and mode (`GetCurrentSensitivity`;
    /// 1 where it has none).
    pub fn item_sensitivity(&self) -> Option<f32> {
        let p = self.current().part?;
        let (scope, mode) = self.scope_and_mode()?;
        Some(self.parts[p].scopes.get(scope).and_then(|s| s.sensitivity.get(mode)).copied().unwrap_or(1.0))
    }

    /// What the mouse's aiming sensitivity is multiplied by while aiming
    /// (`FirearmController.UpdateSensitivity`): the weapon's own figure with
    /// no sight part; a variable scope's by its zoom while aimed through as
    /// an optic; else the item's figure for its scope and mode.
    /// `PROVISIONAL`: a variable scope's is its zoom's now, where the game
    /// keeps the last its optic camera reported (`AimingSmoothSensitivity`,
    /// each frame the camera is on): a frame apart at most as the sights
    /// come up.
    pub fn aim_sensitivity(&self, weapon: f32) -> f32 {
        let sight = self.current();
        let Some(p) = sight.part else { return weapon };
        if self.parts[p].adjustable.is_some() && self.is_optic(sight) {
            let handler = sight.cache.and_then(|c| self.states[c].zoom.as_ref()).or(self.states[p].zoom.as_ref());
            if let Some(handler) = handler {
                return handler.sensitivity();
            }
        }
        self.item_sensitivity().unwrap_or(1.0)
    }

    /// The optic whose camera is on while aiming (`OpticComponentUpdater`:
    /// the current sight's current mode's `OpticSight`, where the sight is
    /// an optic): the part whose prefab's modes it goes by, and that mode.
    pub fn live_optic(&self) -> Option<(usize, usize)> {
        if let Some(part) = self.forced_optic.filter(|&part| part < self.parts.len()) {
            return Some((part, self.states[part].prefab_mode));
        }
        let sight = self.current();
        if !self.is_optic(sight) {
            return None;
        }
        let cache = sight.cache?;
        Some((cache, self.states[cache].prefab_mode))
    }

    /// A part's optic made live, or none (`forced_optic`).
    pub fn force_optic(&mut self, part: Option<usize>) {
        self.forced_optic = part;
    }

    /// How far a variable scope's reticle mesh takes its blend shape, 0 to
    /// 1 (`ScopeZoomHandler.method_1`: clamp((1 − `ReticleBlendCurve` of the
    /// zoom) × 100, 0, 100) of 100, the zoom 0 at the widest): the live
    /// optic's, nought where nothing sets it.
    pub fn reticle_weight(&self) -> f32 {
        let Some((cache, _)) = self.live_optic() else { return 0.0 };
        let (Some(handler), Some(curve)) = (self.states[cache].zoom.as_ref(), self.parts[cache].reticle_curve.as_ref()) else { return 0.0 };
        ((1.0 - curve.evaluate(handler.factor())) * 100.0).clamp(0.0, 100.0) / 100.0
    }

    /// How much the sight in use magnifies, where it is an optic, for
    /// drawing the world without the scope's own picture (optics spec,
    /// "With `PIP` off"): a fixed scope's mode's `Zooms` figure; a variable
    /// one's lowest zoom times its widest field of view over its zoom's now
    /// (the Tango's 1 × 24.5 / 4.08 at the narrowest: 6).
    pub fn magnification(&self) -> Option<f32> {
        let sight = self.current();
        if !self.is_optic(sight) {
            return None;
        }
        let cache = sight.cache?;
        match &self.states[cache].zoom {
            Some(handler) => {
                let (scope, _) = self.scope_and_mode()?;
                let lowest = self.parts[sight.part?].scopes.get(scope)?.zooms.iter().copied().filter(|z| *z > 0.0).fold(f32::INFINITY, f32::min);
                (lowest.is_finite() && handler.field_of_view() > 0.0).then(|| lowest * handler.data.fov[0] / handler.field_of_view())
            }
            None => self.zoom(),
        }
    }

    /// The optic camera's field of view while it is on: aiming, through an
    /// optic, a variable scope's zoom or a fixed one's mode's figure.
    pub fn optic_field_of_view(&self) -> Option<f32> {
        if let Some((part, mode)) = self.forced_optic.and_then(|_| self.live_optic()) {
            return self.parts[part].modes.get(mode)?.optic.as_ref()?.field_of_view;
        }
        let sight = self.current();
        if !self.is_optic(sight) {
            return None;
        }
        let cache = sight.cache?;
        match &self.states[cache].zoom {
            Some(handler) => Some(handler.field_of_view()),
            None => self.parts[cache].modes.get(self.states[cache].prefab_mode)?.optic.as_ref()?.field_of_view,
        }
    }

    /// The key to the next sight (`FirearmController.ChangeAimingMode`):
    /// A mount found with a canted sight in use (`WeaponMountingComponent.
    /// OnFoundPoint`: `ChangeAimingMode` to the first sight not turned aside):
    /// whether the sight changed. None straight, it stays.
    pub fn first_straight(&mut self) -> bool {
        if self.list.is_empty() || self.current().rotation.abs() < SCOPE_ROTATION_THRESHOLD {
            return false;
        }
        let Some(straight) = self.list.iter().position(|sight| sight.rotation.abs() < SCOPE_ROTATION_THRESHOLD) else { return false };
        self.aim_index = straight;
        let changed = self.cycle_scopes();
        self.tell_sight_changed();
        changed
    }

    /// the next, wrapping; mounted, any turned from straight ahead passed
    /// over. Whether the sight changed: the eye then goes to the new one
    /// at the aiming rate, and the main camera to its field of view
    /// (`CycleScopes`). The caller refuses it in blind fire and while the
    /// left hand is busy.
    pub fn change_aim_scope(&mut self, mounted: bool) -> bool {
        let count = self.list.len();
        let mut next = self.aim_index + 1;
        if next >= count {
            next = 0;
        }
        if mounted {
            while next != self.aim_index && self.list[next].rotation.abs() >= SCOPE_ROTATION_THRESHOLD {
                next += 1;
                if next >= count {
                    next = 0;
                }
            }
            if next == self.aim_index || self.list[next].rotation.abs() >= SCOPE_ROTATION_THRESHOLD {
                return false;
            }
        }
        if next == self.aim_index {
            return false;
        }
        self.aim_index = next;
        let changed = self.cycle_scopes();
        // `RaiseSightChangedEvent`, the index set whatever `CycleScopes` did.
        self.tell_sight_changed();
        changed
    }

    /// The weapon's sight index set (`CycleScopes`): the scope of the item
    /// it falls in is chosen by counting every sight item's scopes in the
    /// game's walk of the items, and set on the sight listed at that count,
    /// as the game does (on the G36 the carry handle's backup is its scope
    /// 1; on the M60E6 the Specter's backup leaves it on scope 0). Where the
    /// count runs past the list the game stops there, with the eye and the
    /// field of view as they were: false.
    fn cycle_scopes(&mut self) -> bool {
        let index = self.aim_index;
        if index >= self.list.len() {
            return false;
        }
        let mut count = 0;
        for item in 0..self.parts.len() {
            let scopes = self.parts[item].scopes.len();
            let set = if index == count {
                Some(0)
            } else if index > count && index < count + scopes {
                Some(index - count)
            } else {
                count += scopes;
                continue;
            };
            let Some(listed) = self.list.get(count) else { return false };
            let target = listed.part.unwrap_or(item);
            let scopes_count = self.parts[target].scopes.len();
            if let Some(scope) = set.filter(|_| scopes_count > 0) {
                self.states[target].scope = scope % scopes_count;
            }
            break;
        }
        // A launcher's line, the iron sights' (the shot's squeeze back on)
        // and the calibration asked for (`method_13`, `method_14`,
        // `method_2`).
        self.measure_launcher_line();
        self.iron_line_measured();
        self.calibration_asked = true;
        self.work_out_eye();
        true
    }

    /// The magnification key (`GetSightStateAfterChangeMagnification`,
    /// `SetScopeMode`, `Firearms.UpdateScopesMode`): the scope the sight
    /// index counts to, among the sight items listed sorted by their names,
    /// goes to its next mode; every item's prefab shows its mode, and every
    /// variable scope snaps to the far end of its range. Whether it was
    /// done (the hands' animator then takes `ModSwitch`, and the main
    /// camera and the eye go by the new mode: `OnScopesModeUpdated`).
    pub fn change_magnification(&mut self) -> bool {
        let mut by_name: Vec<usize> = (0..self.parts.len()).filter(|&p| self.list.iter().any(|sight| sight.part == Some(p))).collect();
        if by_name.is_empty() {
            return false;
        }
        by_name.sort_by(|&a, &b| self.parts[a].template.cmp(&self.parts[b].template));
        let mut count = 0;
        for p in by_name {
            for scope in 0..self.parts[p].scopes.len() {
                if count == self.aim_index {
                    let modes = self.parts[p].scopes[scope].modes;
                    let next = self.states[p].modes[scope] + 1;
                    self.states[p].modes[scope] = if next >= modes { 0 } else { next };
                }
                count += 1;
            }
        }
        // The screen hears of the new modes (`SetScopeMode`), then of each
        // variable scope's snap to an end (`ForceChangeScopeState`).
        self.tell_sight_changed();
        for p in 0..self.parts.len() {
            self.show_mode(p);
            let state = &mut self.states[p];
            if let Some(handler) = state.zoom.as_mut() {
                if handler.snap(&mut state.zoom_value) {
                    let edge = handler.edge;
                    self.tell_edge(edge);
                }
            }
        }
        // `OnScopesModeUpdated`: the calibration asked for, the eye worked
        // out, the shot's way and squeeze as the sights going down leave
        // them, then a launcher's line and the iron sights' measured.
        self.calibration_asked = true;
        self.work_out_eye();
        self.sights_down();
        self.measure_launcher_line();
        self.iron_line_measured();
        true
    }

    /// A part's prefab shows the mode of its first scope that has as many
    /// modes as the prefab (`SightModVisualControllers.UpdateSightMode`;
    /// a prefab of one mode stays as it is).
    fn show_mode(&mut self, part: usize) {
        let prefab_modes = self.parts[part].modes.len();
        if prefab_modes < 2 {
            return;
        }
        let state = &mut self.states[part];
        if let Some(scope) = self.parts[part].scopes.iter().position(|scope| scope.modes == prefab_modes) {
            let mode = state.modes.get(scope).copied().unwrap_or(0);
            if mode < prefab_modes {
                state.prefab_mode = mode;
            }
        }
    }

    /// One frame of the zoom's wheel (`ScopeZoomHandler`): aiming, only the
    /// variable scope aimed through as an optic takes it (its optic camera
    /// runs it); else every variable scope on the weapon does. `input` is
    /// the wheel's turn times `OpticSensitivity`, wheel up positive, `dt`
    /// the frame's time.
    pub fn zoom_frame(&mut self, input: f32, dt: f32, aiming: bool) {
        let live = if aiming {
            let sight = *self.current();
            match sight.cache.filter(|_| self.is_optic(&sight)) {
                Some(cache) => vec![cache],
                None => Vec::new(),
            }
        } else {
            (0..self.parts.len()).collect()
        };
        for p in live {
            let state = &mut self.states[p];
            if let Some(handler) = state.zoom.as_mut() {
                if handler.update(input, dt, &mut state.zoom_value) {
                    let edge = handler.edge;
                    self.tell_edge(edge);
                }
            }
        }
    }

    /// The squeeze of the weapon at the field of view has changed
    /// (`SetFovParams`): the eye goes by it.
    pub fn set_scale(&mut self, scale: f32) {
        if scale != self.scale {
            self.scale = scale;
            self.work_out_eye();
        }
    }

    /// The eye as last worked out (`FindCameraDepth`).
    pub fn eye(&self) -> SightEye {
        self.eye
    }

    /// Where the eye goes for the current sight (`UpdateFarPlane`,
    /// `FindCameraDepth`): the far plane is the nearest optic's bone, less
    /// its eyecup, or the prefab's far plane if that is nearer; the plane
    /// aimed at is the far one through an optic and the prefab's ordinary
    /// one (or the plane the sight names) else, no deeper than the far
    /// plane less the weapon bone's own place.
    fn work_out_eye(&mut self) {
        let mut nearest = f32::INFINITY;
        let mut past = 0.0f32;
        for (i, sight) in self.list.iter().enumerate() {
            if self.is_optic(sight) {
                nearest = nearest.min(self.depths[i] - sight.eye_guard_shift);
                past = (nearest - self.planes.far).max(0.0);
            }
        }
        let far = nearest.min(self.planes.far);
        let sight = *self.current();
        let custom = sight.part.map(|p| self.parts[p].custom_aim_plane.as_str()).filter(|name| !name.is_empty()).and_then(|name| self.planes.custom.iter().find(|(n, _)| n == name)).map(|(_, depth)| *depth);
        let optic = self.is_optic(&sight);
        let plane = custom.unwrap_or(if optic { self.planes.far } else { self.planes.default });
        let depth = plane.min(far - self.planes.weapon_y);
        self.eye = if optic {
            let cache = sight.cache.map(|c| &self.parts[c]);
            let mode = cache.and_then(|c| c.modes.get(self.states[sight.cache.unwrap_or(0)].prefab_mode));
            let distance = mode
                .and_then(|m| m.optic.as_ref())
                .or_else(|| cache.and_then(|c| c.modes.iter().find_map(|m| m.optic.as_ref())))
                .map_or(0.0, |optic| optic.distance_to_camera);
            SightEye { depth, distance: distance + past }
        } else {
            SightEye::iron(depth)
        };
    }

    /// The backup sights that fold, and whether each is shown
    /// (`AutoFoldableSight`).
    pub fn folding(&self) -> &[(usize, bool)] {
        &self.folding
    }

    /// The bones a rig reads for the current sight: its aim bone, and the
    /// iron sights' line where the game takes one for it.
    pub fn nodes(&self) -> crate::rig::SightNodes {
        let sight = self.current();
        crate::rig::SightNodes { weapon: self.weapon, bone: sight.bone, line: self.line.filter(|_| sight.align), muzzle: self.muzzle, own: sight.own }
    }

    // ---- Zeroing (optics spec, "Zeroing") ----

    /// The zeroing points of every sight item's scopes (`Weapon.
    /// RecalculateOpticCalibrationPoints`), from the weapon's default round
    /// (none: no points), its speed factor and the world's gravity; and
    /// whether the weapon is a launcher (`AdjustCollimatorsToTrajectory`).
    pub fn set_ballistics(&mut self, round: Option<&Round>, speed_factor: f32, gravity: V3, launcher: bool) {
        self.launcher = launcher;
        self.points = self
            .parts
            .iter()
            .map(|part| part.scopes.iter().map(|scope| round.map_or_else(Vec::new, |round| zeroing_points(&scope.calibration, round, speed_factor, gravity))).collect())
            .collect();
    }

    /// Whether the weapon is a launcher (`AdjustCollimatorsToTrajectory`).
    pub fn launcher(&self) -> bool {
        self.launcher
    }

    /// The current sight's item and its current scope, where that scope
    /// has zeroing points (`HasOpticCalibrationPoints`).
    fn zeroed_scope(&self) -> Option<(usize, usize)> {
        let p = self.current().part?;
        let scope = self.states[p].scope;
        self.points.get(p)?.get(scope).filter(|points| !points.is_empty()).map(|_| (p, scope))
    }

    /// The zeroing keys (`OpticCalibrationSwitch`): refused unless aiming;
    /// the current sight's item moves its current scope's index one place
    /// up or down its points, held at either end. Whether it moved: the
    /// calibration is then asked for.
    pub fn step_zeroing(&mut self, up: bool, aiming: bool) -> bool {
        if !aiming {
            return false;
        }
        let Some((p, scope)) = self.zeroed_scope() else { return false };
        let last = self.points[p][scope].len() - 1;
        let index = &mut self.states[p].zeroing[scope];
        let was = *index;
        *index = if up { (*index + 1).min(last) } else { index.saturating_sub(1) };
        let moved = *index != was;
        self.calibration_asked |= moved;
        moved
    }

    /// The current sight's zeroing distance (`GetCurrentOpticCalibration
    /// Distance`): its item's current scope's distance at its index (the
    /// index back to 0 where it runs past the points), 0 where the scope
    /// lists none; none without an item.
    pub fn zeroing_distance(&self) -> Option<i32> {
        let p = self.current().part?;
        let scope = self.states[p].scope;
        let distances = &self.parts[p].scopes.get(scope)?.calibration;
        if distances.is_empty() {
            return Some(0);
        }
        Some(distances.get(self.zeroing_index(p, scope)).copied().unwrap_or(0))
    }

    /// `GetCurrentOpticCalibrationPointIndex`: the index, 0 where it is past
    /// the scope's points.
    fn zeroing_index(&self, p: usize, scope: usize) -> usize {
        let index = self.states[p].zeroing.get(scope).copied().unwrap_or(0);
        let count = self.points.get(p).and_then(|s| s.get(scope)).map_or(0, Vec::len);
        if count > 0 && index >= count {
            0
        } else {
            index
        }
    }

    /// The local zeroing point in force (`_localCalibratePoint`): the game's
    /// axes, z along the flight, y up.
    pub fn local_point(&self) -> V3 {
        self.local_point
    }

    /// The sights have come up or gone down (`ProceduralWeaponAnimation.
    /// IsAiming`): up, a launcher's line measured with the point as it
    /// stands (`method_13`), the shot's squeeze back on where the sight
    /// lines up with the iron sights (`method_14`), and the calibration
    /// asked for (`method_2`); down, the squeeze on and a launcher's line
    /// gone (`method_12`).
    pub fn aim_changed(&mut self, aiming: bool) {
        if aiming {
            self.measure_launcher_line();
            self.iron_line_measured();
            self.calibration_asked = true;
        } else {
            self.sights_down();
        }
    }

    /// Whether the eye is the character's own: a launcher's eye is put on
    /// its line only then (`FirstPersonPointOfView`).
    pub fn set_first_person(&mut self, first_person: bool) {
        self.first_person = first_person;
    }

    /// `method_12`: the shot's squeeze on, a launcher's line gone.
    fn sights_down(&mut self) {
        self.shot_squeeze = true;
        self.line_of_sight = SightLine::default();
    }

    /// `method_14`, as far as the squeeze goes: a sight that lines up with
    /// the iron sights' pair puts it back on (the line itself is the
    /// host's, read off its rig: `Hands::note_sights`).
    fn iron_line_measured(&mut self) {
        if self.current().align && self.line.is_some() {
            self.shot_squeeze = true;
        }
    }

    /// Whether the shot and the recoil take the field of view's squeeze
    /// (`ShotNeedsFovAdjustments`): off once an optic has been turned.
    pub fn shot_squeeze(&self) -> bool {
        self.shot_squeeze
    }

    /// A launcher's line of sight in force (`_lineOfSightDeltaAngle`,
    /// `_cameraShiftToLineOfSight`, as `method_13` and `method_12` set
    /// them); none on any other weapon, whose line is its iron sights'.
    pub fn launcher_line(&self) -> Option<SightLine> {
        self.launcher.then_some(self.line_of_sight)
    }

    /// What the battle screen has been told since this was last asked.
    pub fn take_hud(&mut self) -> Vec<SightHud> {
        std::mem::take(&mut self.hud)
    }

    /// The objects zeroing has turned and each one's turn in its parent,
    /// the game's axes, for a host to put on its own tree.
    pub fn turns(&self) -> impl Iterator<Item = (usize, [f32; 4])> + '_ {
        self.turned.iter().map(|&(node, _)| (node, self.rig.local(node).1))
    }

    /// Each variable scope's ring as its zoom has it (`ScopeZoomHandler.
    /// method_11`, run from `Init` on and after every move of the zoom, so
    /// always the zoom's): its turn in its parent, from its start pivot's at
    /// the widest view to its end pivot's at the narrowest (`game_slerp`).
    /// Only a part with a zoom handler has one, as only an adjustable
    /// optic's `Init` goes on.
    pub fn ring_turns(&self) -> impl Iterator<Item = (usize, [f32; 4])> + '_ {
        self.parts.iter().zip(&self.states).filter_map(|(part, state)| {
            let ring = part.zoom_ring?;
            let zoom = state.zoom.as_ref()?;
            Some((ring.node, game_slerp(ring.start, ring.end, zoom.factor(), !ring.inverse)))
        })
    }

    /// The calibration (`PreCalibrate`, `Calibrate`), in the player's late
    /// update, in first person: where asked and the current sight's item
    /// has points for its scope, the local point taken, the screen told the
    /// distance, a launcher's line measured and the part turned by distance;
    /// then, where asked or aiming while the field of view squeezes the
    /// weapon, the optics turned (not a launcher's) and the collimators.
    pub fn calibrate(&mut self, aiming: bool) {
        if self.calibration_asked {
            if let Some((p, scope)) = self.zeroed_scope() {
                self.local_point = self.points[p][scope][self.zeroing_index(p, scope)];
                if let Some(distance) = self.zeroing_distance() {
                    self.hud.push(SightHud::Zeroing(distance));
                    self.measure_launcher_line();
                    self.turn_by_distance(distance as f32);
                }
            }
        }
        if self.calibration_asked || (aiming && self.scale < 1.0) {
            if !self.launcher {
                self.turn_optics();
            }
            self.turn_collimators();
            self.calibration_asked = false;
        }
    }

    /// The zeroing point in the weapon's tree (`GetWorldCalibrationPoint`):
    /// the fireport, plus the local point turned by Euler (90, -cant, 0) in
    /// the fireport's axes; and the fireport's forward, the turns' up.
    fn world_point(&self) -> Option<(V3, V3)> {
        let (at, turn) = self.rig.placed(self.fireport?);
        let turn_point = quat_mul(turn, quat_euler([90.0, -self.scope_rotation(), 0.0]));
        let off = quat_rotate(turn_point, self.local_point);
        Some((std::array::from_fn(|k| at[k] + off[k]), quat_rotate(turn, [0.0, 0.0, 1.0])))
    }

    /// `Transform.LookAt`: an object turned in its parent so that its
    /// forward is toward `point` and its up as near `up` as that allows;
    /// `then` turns it further in its own axes. `PROVISIONAL`: on the tree
    /// at rest, where the game turns it in the world squeezed along the
    /// hands' root's forward at a wide field of view (the T1's at 25 m,
    /// 4e-5 radians apart at the install's 74 degrees: `tests/sim/zeroing.rs`).
    fn look_at(&mut self, node: usize, point: V3, up: V3, then: [f32; 4]) {
        let (at, _) = self.rig.placed(node);
        let Some(world) = quat_look(std::array::from_fn(|k| point[k] - at[k]), up) else { return };
        let parent = self.rig.parent(node).map_or(NO_TURN, |parent| self.rig.placed(parent).1);
        let back = [-parent[0], -parent[1], -parent[2], parent[3]];
        let local = quat_mul(quat_mul(back, world), then);
        self.note_turned(node);
        let place = self.rig.local(node).0;
        self.rig.set_local(node, (place, local));
    }

    /// `OpticLookAt`: where the current sight's prefab has an optic in any
    /// mode, each mode in turn its collimator and its optic's camera object
    /// turned to the point, up the fireport's forward (the T12's then losing
    /// its roll); the shot's squeeze off.
    fn turn_optics(&mut self) {
        let Some(cache) = self.current().cache else { return };
        if !self.parts[cache].modes.iter().any(|mode| mode.optic.is_some()) {
            return;
        }
        let Some((point, up)) = self.world_point() else { return };
        for m in 0..self.parts[cache].modes.len() {
            if let Some(node) = self.parts[cache].modes[m].collimator {
                self.look_at(node, point, up, quat_euler(COLLIMATOR_TURN));
            }
            let camera = self.parts[cache].modes[m].optic.as_ref().and_then(|optic| optic.camera.map(|node| (node, optic.thermal_at_45)));
            if let Some((node, thermal)) = camera {
                self.look_at(node, point, up, NO_TURN);
                // `IsThermalSightAvailableAt45Degrees`: turned about its z
                // by minus its local roll, where that less 1 is above 0.
                let (place, turn) = self.rig.local(node);
                let roll = quat_to_euler(turn)[2].rem_euclid(360.0);
                if thermal && roll - 1.0 > 0.0 {
                    self.rig.set_local(node, (place, quat_mul(turn, quat_euler([0.0, 0.0, -roll]))));
                }
            }
        }
        self.shot_squeeze = false;
    }

    /// `CollimatorLookAt`: where the prefab has a collimator in any mode,
    /// every mode's turned to the point, up the first one's forward as it
    /// stands, then by -90 degrees about its x.
    fn turn_collimators(&mut self) {
        let Some(cache) = self.current().cache else { return };
        let Some(first) = self.parts[cache].modes.iter().find_map(|mode| mode.collimator) else { return };
        let Some((point, _)) = self.world_point() else { return };
        let up = quat_rotate(self.rig.placed(first).1, [0.0, 0.0, 1.0]);
        for m in 0..self.parts[cache].modes.len() {
            if let Some(node) = self.parts[cache].modes[m].collimator {
                self.look_at(node, point, up, quat_euler(COLLIMATOR_TURN));
            }
        }
    }

    /// `method_13`: a launcher's line of sight, where its sight is not an
    /// optic and its prefab has a collimator: from the first collimator's
    /// object to the zeroing point, in the weapon bone's axes, measured as
    /// the iron sights' line is (in first person, the eye's shift too).
    fn measure_launcher_line(&mut self) {
        if !self.launcher {
            return;
        }
        let sight = *self.current();
        if self.is_optic(&sight) {
            return;
        }
        let Some(cache) = sight.cache else { return };
        let Some(lens) = self.parts[cache].modes.iter().find_map(|mode| mode.collimator) else { return };
        let Some((point, _)) = self.world_point() else { return };
        let (at, turn) = self.rig.placed(self.weapon);
        let back = [-turn[0], -turn[1], -turn[2], turn[3]];
        let flat = |there: V3| {
            let v = quat_rotate(back, std::array::from_fn(|k| there[k] - at[k]));
            [-v[1], v[2]]
        };
        let (lens, point, bone) = (flat(self.rig.placed(lens).0), flat(point), flat(self.rig.placed(sight.bone).0));
        self.line_of_sight.angle = ((point[1] - lens[1]) / (point[0] - lens[0])).atan().to_degrees();
        if self.first_person {
            // `Vector2.normalized`: nought below 1e-5.
            let way = [lens[0] - point[0], lens[1] - point[1]];
            let size = (way[0] * way[0] + way[1] * way[1]).sqrt();
            let way = if size > 1e-5 { [way[0] / size, way[1] / size] } else { [0.0; 2] };
            let behind = lens[0] - bone[0];
            self.line_of_sight.shift = [lens[0] + way[0] * behind - bone[0], lens[1] + way[1] * behind - bone[1]];
        }
    }

    /// `RotateCurrentScopeToCorrectDistanceAngle`: the current sight's
    /// prefab's part turned by distance, where its table has this one: its
    /// turn set to Euler (angle, y, z), y and z its turn's own quaternion's
    /// (the game's, kept).
    fn turn_by_distance(&mut self, distance: f32) {
        let Some(cache) = self.current().cache else { return };
        let Some(turned) = self.parts[cache].angle_by_range.as_ref() else { return };
        let Some(&(_, angle)) = turned.table.iter().find(|(d, _)| *d == distance) else { return };
        let axis = turned.axis;
        self.note_turned(axis);
        let (place, turn) = self.rig.local(axis);
        self.rig.set_local(axis, (place, quat_euler([angle, turn[1], turn[2]])));
    }

    /// An object about to be turned, kept with its turn as its prefab has it.
    fn note_turned(&mut self, node: usize) {
        if !self.turned.iter().any(|&(n, _)| n == node) {
            self.turned.push((node, self.rig.local(node).1));
        }
    }

    /// The live optic's camera object's turn from its prefab's, as the
    /// turned camera's forward in the picture of the camera as its prefab
    /// stands, at a field of view of `fov` degrees: right and up from the
    /// picture's middle, in the picture's widths (for the reticle on clear
    /// glass with `PIP` off: optics spec, "With `PIP` off").
    pub fn zeroing_offset(&self, fov: f32) -> Option<[f32; 2]> {
        let (part, mode) = self.live_optic()?;
        let node = self.parts[part].modes.get(mode)?.optic.as_ref()?.camera?;
        let before = self.turned.iter().find(|&&(n, _)| n == node)?.1;
        let now = self.rig.local(node).1;
        let back = [-before[0], -before[1], -before[2], before[3]];
        let a = quat_rotate(quat_mul(back, now), [0.0, 0.0, 1.0]);
        if a[2] <= 1e-6 {
            return None;
        }
        let t = (fov.to_radians() * 0.5).tan();
        Some([0.5 * a[0] / a[2] / t, 0.5 * a[1] / a[2] / t])
    }

    /// `BattleUIScreen.SightChanged`: a sight item that is not a variable
    /// scope names its zoom where that is above 1, or any of its scope's is.
    fn tell_sight_changed(&mut self) {
        let Some(p) = self.current().part else { return };
        if self.parts[p].adjustable.is_some() {
            return;
        }
        let Some((scope, mode)) = self.scope_and_mode() else { return };
        let zooms = self.parts[p].scopes.get(scope).map(|s| s.zooms.as_slice()).unwrap_or(&[]);
        let zoom = zooms.get(mode).copied().unwrap_or(0.0);
        if zoom > 1.0 || zooms.iter().any(|z| *z > 1.0) {
            self.hud.push(SightHud::Magnification(zoom));
        }
    }

    /// `BattleUIScreen.SmoothSightChanged`, a variable scope come to an end
    /// of its range or left one: at the widest the current sight's lowest
    /// zoom, at the narrowest its highest, between, the panel hidden.
    fn tell_edge(&mut self, edge: Edge) {
        let zooms: Vec<f32> = match self.current().part {
            Some(p) => self.parts[p].scopes.get(self.states[p].scope).map(|s| s.zooms.clone()).unwrap_or_default(),
            None => {
                self.hud.push(SightHud::HideMagnification);
                return;
            }
        };
        let (lowest, highest) = if zooms.is_empty() { (0.0, 0.0) } else { (zooms.iter().copied().fold(f32::INFINITY, f32::min), zooms.iter().copied().fold(f32::NEG_INFINITY, f32::max)) };
        self.hud.push(match edge {
            Edge::Widest => SightHud::Magnification(lowest),
            Edge::Narrowest => SightHud::Magnification(highest),
            Edge::Between => SightHud::HideMagnification,
        });
    }
}

/// A scope's zeroing points (`Weapon.CreateOpticCalibrationData`; optics
/// spec, "Zeroing"): its distances sorted, repeats dropped; the round flown
/// from nought along z at its initial speed times the weapon's speed
/// factor by the game's trajectory, sampled from 0 s every millisecond and
/// carried on from one distance to the next, a distance's point the first
/// sample at least that far from the start; each distance as written then
/// given its point. The game's axes: z along the flight, y up.
pub fn zeroing_points(distances: &[i32], round: &Round, speed_factor: f32, gravity: V3) -> Vec<V3> {
    let mut sorted: Vec<i32> = Vec::new();
    for &distance in distances {
        if !sorted.contains(&distance) {
            sorted.push(distance);
        }
    }
    sorted.sort_unstable();
    let speed = round.initial_speed * speed_factor;
    let mut flight = Trajectory::new([0.0; 3], [0.0, 0.0, speed], round.mass_g, round.diameter_mm, round.ballistic_coefficient, gravity);
    let (mut position, mut reach, mut time) = ([0.0f32; 3], 0.0f32, 0.0f32);
    let mut found = Vec::with_capacity(sorted.len());
    for &distance in &sorted {
        // The game squares the whole number, then takes it for a float.
        let want = distance.wrapping_mul(distance) as f32;
        let mut samples = 0;
        while reach < want && samples < POINT_SAMPLES {
            position = flight.at(time).0;
            reach = position[0] * position[0] + position[1] * position[1] + position[2] * position[2];
            time += POINT_STEP;
            samples += 1;
        }
        found.push(position);
    }
    distances.iter().map(|distance| found[sorted.binary_search(distance).unwrap_or(0)]).collect()
}

/// Shows and hides a build's backup sights that fold, on the tree a rig is
/// drawn from, as its sights have them (`AutoFoldableSight`, set as the
/// sights are found): a folded one shown beside a sight part that looks
/// straight ahead, an unfolded one without. A model of the build not in
/// hand (a picture, one on the ground) keeps its prefab's.
pub fn fold_sights(weapon: &mut trp_data::weapon::Weapon) {
    let Some(sights) = Sights::of_weapon(weapon) else { return };
    for &(node, shown) in sights.folding() {
        weapon.hierarchy.nodes[node].active = shown;
    }
}

/// The wheel's input to a variable scope's zoom on a frame
/// (`ScopeZoomHandler.GetZoomInput`), read straight off the keys and the
/// wheel, not through the bindings' commands: the wheel's turn on the
/// frame times `OpticSensitivity` where a variant of `ScopeZoomIn` or
/// `ScopeZoomOut` that ends in the wheel has its keys held (the install's
/// two: `LeftAlt` and the wheel either way), whichever way the wheel went,
/// turned about with the inverted up and down; a variant of keys alone
/// gives the sensitivity, out for `ScopeZoomIn`'s and in for the other's.
/// Wheel up positive, which narrows.
pub fn zoom_input(controls: &crate::bindings::Controls, held: &dyn Fn(crate::bindings::Code) -> bool, wheel: f32, optic_sensitivity: f32, inverted: bool) -> f32 {
    let scroll = wheel * optic_sensitivity;
    for (name, mut positive) in [("ScopeZoomIn", true), ("ScopeZoomOut", false)] {
        let Some(group) = controls.group(name) else { continue };
        for variant in &controls.keys[group].variants {
            let keys_held = variant.keys.iter().all(|&key| held(key));
            if let Some(up) = variant.wheel {
                // The game's own reading of a wheel's variant: its way is
                // taken against the inverted switch, and the turn must be
                // the other way to it (`Positive`: above nothing), which
                // lets either variant take either way.
                positive = if inverted { up } else { !up };
                if scroll == 0.0 || (scroll > 0.0) != positive {
                    continue;
                }
                if keys_held {
                    return if inverted { -scroll } else { scroll };
                }
            }
            if !variant.keys.is_empty() && keys_held {
                return if positive { -optic_sensitivity } else { optic_sensitivity };
            }
        }
    }
    0.0
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// `Mathf.InverseLerp`.
/// `ScopeZoomHandler.Slerp`: the sine formula, not normalised; on the short
/// way `p` is turned over where the two stand more than half a turn apart,
/// else the arc goes as they stand (past half a turn). (Where they are one
/// turn the game divides by nought; here `p` comes back.)
pub fn game_slerp(p: [f32; 4], q: [f32; 4], t: f32, short_way: bool) -> [f32; 4] {
    let dot = p[0] * q[0] + p[1] * q[1] + p[2] * q[2] + p[3] * q[3];
    if short_way && dot < 0.0 {
        return game_slerp(p.map(|v| -v), q, t, true);
    }
    let angle = dot.clamp(-1.0, 1.0).acos();
    let sine = angle.sin();
    if sine.abs() < 1e-7 {
        return p;
    }
    let (a, b) = (((1.0 - t) * angle).sin(), (t * angle).sin());
    let scalar = 1.0 / sine;
    std::array::from_fn(|k| (p[k] * a + q[k] * b) * scalar)
}

fn inverse_lerp(a: f32, b: f32, value: f32) -> f32 {
    if a == b {
        0.0
    } else {
        ((value - a) / (b - a)).clamp(0.0, 1.0)
    }
}

/// `Mathf.MoveTowards`.
fn move_towards(current: f32, target: f32, step: f32) -> f32 {
    if (target - current).abs() <= step {
        target
    } else {
        current + (target - current).signum() * step
    }
}
