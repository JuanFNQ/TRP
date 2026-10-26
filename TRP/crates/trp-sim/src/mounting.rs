//! Mounting and the bipod (`docs/spec/mounting.md`): the search for a
//! place to rest the weapon, the mount's data worked out from what was
//! found, the figures the game reads for both, the weapon's own figures,
//! and what a mount tells its host. The mount's own state, which overrides
//! the movement state while it lasts (`IdleWeaponMountingState`), is the
//! movement's (`crate::movement`); the hand-over to the weapon and its
//! motion is here (`tell`).

use anyhow::{Context, Result};
use serde_json::Value as Json;

use crate::math::{delta_angle, inverse_lerp, lerp, quat_axis, quat_rotate};
use crate::movement::World;

/// Which way a mount meets its cover (`EMountSideDirection`): on a top
/// ahead, or at a wall's free edge to the left or the right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Forward,
    Left,
    Right,
}

impl Side {
    /// The game's number for it: 0 forward, -1 left, 1 right.
    pub fn number(self) -> i32 {
        match self {
            Side::Forward => 0,
            Side::Left => -1,
            Side::Right => 1,
        }
    }
}

/// A place found to rest the weapon on (`MountPointData`): the point, the
/// way the cover faces turned round, and the side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub at: [f32; 3],
    pub direction: [f32; 3],
    pub side: Side,
}

/// The world's layers a search casts on (`LayersMaskController`): the
/// detailed ones and the terrain (`HighPolyWithTerrainMask` and the
/// transparent layer), the terrain's coarse one and doors (`TerrainLowPoly`
/// and the door layer: the side search's second cast and the prone search),
/// and the character's static collision (`PlayerStaticCollisionsMask`: the
/// target place's ground and the way there). A host with one world answers
/// the same for all three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Detailed,
    Coarse,
    Static,
}

/// What a ray met first (`RaycastHit`): how far along it, where, and the
/// face's normal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub distance: f32,
    pub point: [f32; 3],
    pub normal: [f32; 3],
}

/// `TILT_POSITION_OFFSET`: how far the body is shifted sideways at the
/// yaw span's end, in metres, for a mount's yaw limit and its turn's check.
pub const TILT_POSITION_OFFSET: f32 = 0.25;
/// `PITCH_DEGREE_TO_POSE_LEVEL`: degrees of pitch to a whole unit of stance.
const PITCH_PER_POSE: f32 = 50.0;
/// The target place's offset from the pivot, crouched and standing
/// (`_localSitRsOffset`, `_localRsOffset`).
const SIT_OFFSET: [f32; 3] = [0.1866, 0.0, 0.2853];
const STAND_OFFSET: [f32; 3] = [0.1849, 0.0, 0.074];
/// `MAX_MOUNTING_POINT_OFFSET`: a side mount's drop at the span's end.
pub const SIDE_DROP: f32 = -0.1;
/// `POSSIBLE_POSITON_DELTASQR`: the target place's reach, squared.
const TARGET_REACH_SQUARED: f32 = 0.008;
/// The share of the way the body's `Inert` stays up past: 2.2 cm, squared.
const INERT_REACH_SQUARED: f32 = 0.0005;
/// `MAX_GROUND_SEARCH_DISTANCE`: the prone search's ray at most.
const PRONE_RAY_MOST: f32 = 0.4;
/// The prone search's free sphere's radius.
const PRONE_SPHERE: f32 = 0.2;
/// How far down the target place's ground is looked for.
const TARGET_GROUND_RAY: f32 = 3.0;
/// The weapon sticking this far into something puts a forward mount's
/// lower pitch limit at the current pitch (`CalculateDynamicDownLimit`).
const DOWN_LIMIT_OVERLAP: f32 = 0.01;
/// The fallback pivot's offset from the left hand's marker
/// (`ProceduralWeaponAnimation.FindMountingPoint`).
const MARKER_TO_PIVOT: [f32; 3] = [0.05, -0.18, 0.025];

pub(crate) const fn reach_squared() -> (f32, f32) {
    (TARGET_REACH_SQUARED, INERT_REACH_SQUARED)
}

pub(crate) const fn down_limit_overlap() -> f32 {
    DOWN_LIMIT_OVERLAP
}

/// The figures (globals.json `MountingSettings` and two of `Stamina`'s;
/// mounting spec, "The search", "The mount's data", "The weapon's motion
/// while mounted").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    /// `MaxVerticalMountAngle`: a side mount's yaw span.
    pub side_span: f32,
    /// `PitchLimitVertical`, `PitchLimitHorizontal`, `PitchLimitHorizontalBipod`.
    pub pitch_side: [f32; 2],
    pub pitch_forward: [f32; 2],
    pub pitch_bipod: [f32; 2],
    /// `MinApproachTime`, `MaxApproachTime`, `ApproachTimeDeltaAngleModifier`.
    pub approach: [f32; 2],
    pub approach_per_degree: f32,
    /// `ExitTime`.
    pub exit_time: f32,
    /// `RotationSpeedClamp`, `SensitivityMultiplier`.
    pub turn_clamp: f32,
    pub sensitivity: f32,
    /// `MaxPitchLimitExcess`, `MaxYawLimitExcess`.
    pub pitch_excess: f32,
    pub yaw_excess: f32,
    /// `MountingCameraSpeed`.
    pub camera_speed: f32,
    /// `MountingSwayFactorModifier`.
    pub sway_with_bipod: f32,
    /// `EdgeDetectionDistance`, `RaycastDistance`.
    pub edge: f32,
    pub ray: f32,
    /// `MaxHorizontalMountAngleDotDelta`, `MaxProneMountAngleDotDelta`,
    /// `MaxVerticalMountAngleDotDelta`.
    pub top_dot: f32,
    pub prone_dot: f32,
    pub side_dot: f32,
    /// `GridMinHeight`, `GridMaxHeight`, `HorizontalGridFromTopOffset`.
    pub grid: [f32; 2],
    pub grid_from_top: f32,
    /// `VerticalGridSize` and its steps, `HorizontalGridSize` and its.
    pub column: f32,
    pub column_steps: usize,
    pub row: f32,
    pub row_steps: usize,
    /// `SecondCheckVerticalDistance`, `SecondCheckVerticalGridSize` and its
    /// steps, `SecondCheckVerticalGridOffset`.
    pub top_ray: f32,
    pub top_row: f32,
    pub top_steps: usize,
    pub top_offset: f32,
    /// `CheckWallOffset`, `CheckHorizontalSecondaryOffset`.
    pub wall_check: f32,
    pub wall_in: f32,
    /// `PointVerticalMountOffset`, `PointHorizontalMountOffset`.
    pub side_point: f32,
    pub top_point: f32,
    /// `MaxFramesForRaycast`: what a batch of rays may be waited for.
    pub batch_frames: u32,
    /// The arms' drain aiming mounted: at a side, forward, forward with the
    /// bipod out (`MountingVerticalAimDrainRateMultiplier`,
    /// `MountingHorizontalAimDrainRateMultiplier`, `BipodAimDrainRateMultiplier`).
    pub drain: [f32; 3],
    /// The character's capsule's radius (the prefab's `_radius`: the
    /// game's own controller adds no skin; `trp_data::player::CapsuleData`)
    /// and its step height standing (movement spec, "The character's own
    /// collision": 0.25): the way to the target place goes by them.
    pub capsule_radius: f32,
    pub step_offset: f32,
}

impl Default for Settings {
    /// The install's figures (globals.json, read 2026-10-25), for a host
    /// without an install and the tests.
    fn default() -> Self {
        Self {
            side_span: 23.0,
            pitch_side: [-15.0, 15.0],
            pitch_forward: [-15.0, 15.0],
            pitch_bipod: [-20.0, 20.0],
            approach: [0.2, 1.0],
            approach_per_degree: 0.01,
            exit_time: 0.2,
            turn_clamp: 1.0,
            sensitivity: 1.0,
            pitch_excess: 10.0,
            yaw_excess: 25.0,
            camera_speed: 250.0,
            sway_with_bipod: 0.15,
            edge: 1.2,
            ray: 1.6,
            top_dot: 0.707,
            prone_dot: 0.939,
            side_dot: 0.707,
            grid: [0.81, 1.4],
            grid_from_top: 0.15,
            column: 1.0,
            column_steps: 20,
            row: 0.5,
            row_steps: 15,
            top_ray: 0.07,
            top_row: 0.05,
            top_steps: 5,
            top_offset: 0.05,
            wall_check: 0.1,
            wall_in: 0.02,
            side_point: 0.05,
            top_point: 0.02,
            batch_frames: 3,
            drain: [0.85, 0.6, 0.2],
            capsule_radius: 0.366,
            step_offset: 0.25,
        }
    }
}

impl Settings {
    /// From globals.json (`db.config`).
    pub fn from_config(config: &Json) -> Result<Self> {
        let mounting = config.get("MountingSettings").context("globals have no MountingSettings")?;
        let movement = mounting.get("MovementSettings").context("no MountingSettings.MovementSettings")?;
        let detection = mounting.get("PointDetectionSettings").context("no MountingSettings.PointDetectionSettings")?;
        let stamina = config.get("Stamina").context("globals have no Stamina")?;
        let number = |v: &Json, name: &str| -> Result<f32> { v.get(name).and_then(Json::as_f64).map(|n| n as f32).with_context(|| format!("no number {name}")) };
        let pair = |v: &Json, name: &str| -> Result<[f32; 2]> {
            let p = v.get(name).with_context(|| format!("no {name}"))?;
            Ok([number(p, "x")?, number(p, "y")?])
        };
        let steps = |v: &Json, name: &str| -> Result<usize> { Ok(number(v, name)?.max(0.0) as usize) };
        Ok(Self {
            side_span: number(movement, "MaxVerticalMountAngle")?,
            pitch_side: pair(movement, "PitchLimitVertical")?,
            pitch_forward: pair(movement, "PitchLimitHorizontal")?,
            pitch_bipod: pair(movement, "PitchLimitHorizontalBipod")?,
            approach: [number(movement, "MinApproachTime")?, number(movement, "MaxApproachTime")?],
            approach_per_degree: number(movement, "ApproachTimeDeltaAngleModifier")?,
            exit_time: number(movement, "ExitTime")?,
            turn_clamp: number(movement, "RotationSpeedClamp")?,
            sensitivity: number(movement, "SensitivityMultiplier")?,
            pitch_excess: number(movement, "MaxPitchLimitExcess")?,
            yaw_excess: number(movement, "MaxYawLimitExcess")?,
            camera_speed: number(movement, "MountingCameraSpeed")?,
            sway_with_bipod: number(movement, "MountingSwayFactorModifier")?,
            edge: number(detection, "EdgeDetectionDistance")?,
            ray: number(detection, "RaycastDistance")?,
            top_dot: number(detection, "MaxHorizontalMountAngleDotDelta")?,
            prone_dot: number(detection, "MaxProneMountAngleDotDelta")?,
            side_dot: number(detection, "MaxVerticalMountAngleDotDelta")?,
            grid: [number(detection, "GridMinHeight")?, number(detection, "GridMaxHeight")?],
            grid_from_top: number(detection, "HorizontalGridFromTopOffset")?,
            column: number(detection, "VerticalGridSize")?,
            column_steps: steps(detection, "VerticalGridStepsAmount")?,
            row: number(detection, "HorizontalGridSize")?,
            row_steps: steps(detection, "HorizontalGridStepsAmount")?,
            top_ray: number(detection, "SecondCheckVerticalDistance")?,
            top_row: number(detection, "SecondCheckVerticalGridSize")?,
            top_steps: steps(detection, "SecondCheckVerticalGridSizeStepsAmount")?,
            top_offset: number(detection, "SecondCheckVerticalGridOffset")?,
            wall_check: number(detection, "CheckWallOffset")?,
            wall_in: number(detection, "CheckHorizontalSecondaryOffset")?,
            side_point: number(detection, "PointVerticalMountOffset")?,
            top_point: number(detection, "PointHorizontalMountOffset")?,
            batch_frames: number(detection, "MaxFramesForRaycast")?.max(0.0) as u32,
            drain: [
                number(stamina, "MountingVerticalAimDrainRateMultiplier")?,
                number(stamina, "MountingHorizontalAimDrainRateMultiplier")?,
                number(stamina, "BipodAimDrainRateMultiplier")?,
            ],
            ..Self::default()
        })
    }
}

// ==== Vectors ====

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn magnitude(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

/// `Vector3.normalized`: nothing for a vector shorter than 1e-5.
fn normalized(a: [f32; 3]) -> [f32; 3] {
    let m = magnitude(a);
    if m > 1e-5 { scale(a, 1.0 / m) } else { [0.0; 3] }
}

const UP: [f32; 3] = [0.0, 1.0, 0.0];

/// The way a yaw faces, and its right (Unity's `Quaternion.Euler(0, yaw, 0)`).
fn forward_of(yaw: f32) -> [f32; 3] {
    let (s, c) = yaw.to_radians().sin_cos();
    [s, 0.0, c]
}

fn right_of(yaw: f32) -> [f32; 3] {
    let (s, c) = yaw.to_radians().sin_cos();
    [c, 0.0, -s]
}

/// `Quaternion.LookRotation(direction).eulerAngles.y`: the bearing of a
/// direction, 0 to 360.
pub fn bearing(direction: [f32; 3]) -> f32 {
    direction[0].atan2(direction[2]).to_degrees().rem_euclid(360.0)
}

/// A missed ray's hit as the engine leaves it: nothing at all, its point
/// and normal nought and its distance 0.
fn missed() -> Hit {
    Hit { distance: 0.0, point: [0.0; 3], normal: [0.0; 3] }
}

/// A batch of rays (`SetRaycasts`): from `start`, stepped along `along`
/// over `size` in `steps`, each cast along `direction` for `length`. The
/// game steps by adding the step in single precision; only the first
/// `steps` are read.
fn cast_batch(world: &dyn World, start: [f32; 3], along: [f32; 3], size: f32, steps: usize, direction: [f32; 3], length: f32, layer: Layer) -> Vec<Hit> {
    let step = size / steps as f32;
    let mut at = 0.0f32;
    let mut hits = Vec::with_capacity(steps);
    for _ in 0..steps {
        let from = add(start, scale(along, at));
        hits.push(if at < size { world.ray(from, direction, length, layer).unwrap_or_else(missed) } else { missed() });
        at += step;
    }
    hits
}

fn hit_any(hit: &Hit) -> bool {
    hit.point != [0.0; 3] || hit.distance != 0.0 || hit.normal != [0.0; 3]
}

// ==== The search ====

/// A search under way (`MountingPointDetectionSystem`): what it will find
/// and how many frames from the key it answers on (a batch of rays is
/// answered the frame after it is cast: mounting spec, "Frames").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Search {
    answer: Option<Point>,
    frames: u32,
}

/// What a standing search starts from (`FindMountingPoint`): the grid's
/// root (the body's `Weapon_root` bone) in the world, the look's yaw, the
/// feet's height, and the pivot heights the grid is raised by: the
/// mount's (the bipod's with it out) for the column, the mount's for the
/// row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    pub root: [f32; 3],
    pub yaw: f32,
    pub feet: f32,
    pub column_raise: f32,
    pub row_raise: f32,
}

impl Search {
    /// A search from standing (or crouched), worked out at the key against
    /// the world as it stands then (PROVISIONAL: the game casts each batch
    /// as the last is answered; a character still for the two or three
    /// frames sees the same).
    pub fn standing(world: &dyn World, s: &Settings, grid: Grid) -> Self {
        let (answer, batches) = standing_search(world, s, grid);
        Self { answer, frames: batches }
    }

    /// A frame has passed: the answer, if this is the frame it comes on
    /// (found or not).
    pub fn frame(&mut self) -> Option<Option<Point>> {
        self.frames = self.frames.saturating_sub(1);
        (self.frames == 0).then_some(self.answer)
    }
}

/// The standing search's answer, and how many batches of rays it cast.
fn standing_search(world: &dyn World, s: &Settings, grid: Grid) -> (Option<Point>, u32) {
    let forward = forward_of(grid.yaw);
    let right = right_of(grid.yaw);
    let down = [0.0, -1.0, 0.0];
    // The column straight ahead (`FindHorizontalMountingPoint`).
    let height = grid.root[1] - grid.feet;
    let low = s.grid[0] + grid.column_raise;
    let high = s.grid[1] + grid.column_raise;
    let top = (height + s.column / 2.0).clamp(low, high);
    let bottom = (height - s.column / 2.0).clamp(low, high);
    let size = (top - bottom).abs() + (grid.column_raise / 2.0).abs();
    let start = [grid.root[0], grid.feet + top, grid.root[2]];
    let column = cast_batch(world, start, down, size, s.column_steps, forward, s.ray, Layer::Detailed);
    let mut batches = 1;
    let mut edge: Option<Hit> = None;
    let (mut nearest_height, mut taken) = (f32::MAX, 0usize);
    for (i, hit) in column.iter().enumerate() {
        let after_gap = i == 0 || !hit_any(&column[i - 1]) || column[i - 1].distance > s.edge;
        let off = if hit_any(hit) { (hit.point[1] - grid.root[1]).abs() } else { f32::MAX };
        let last = edge.map_or(0.0, |e| e.distance);
        let stepped_out = last - hit.distance > s.edge * 0.2 && taken + 1 == i;
        if hit_any(hit) && hit.distance <= s.edge && ((after_gap && nearest_height > off) || stepped_out) {
            edge = Some(*hit);
            nearest_height = off;
            taken = i;
        }
    }
    if let Some(edge) = edge {
        let above = add(edge.point, scale(UP, s.top_offset));
        if world.ray(above, forward, s.top_row, Layer::Detailed).is_none() {
            batches += 1;
            let tops = cast_batch(world, above, forward, s.top_row, s.top_steps, down, s.top_ray, Layer::Detailed);
            let mut found: Option<[f32; 3]> = None;
            for hit in &tops {
                if hit.point == [0.0; 3] {
                    continue;
                }
                if hit.distance == 0.0 {
                    found = None;
                    break;
                }
                let higher = found.map_or(f32::NEG_INFINITY, |f| f[1]);
                if hit.point[1] > edge.point[1] && hit.point[1] > higher && dot(hit.normal, UP) >= s.top_dot {
                    found = Some(hit.point);
                }
            }
            if let Some(top) = found {
                return (Some(Point { at: add(top, scale(UP, s.top_point)), direction: scale(edge.normal, -1.0), side: Side::Forward }), batches);
            }
        }
    }
    // The row at the sides (`FindVerticalMountingPoint`): on the detailed
    // layers, then on the coarse one.
    let raised = (grid.root[1] - grid.feet).clamp(s.grid[0] + grid.row_raise, s.grid[1] + grid.row_raise - s.grid_from_top);
    let centre = [grid.root[0], grid.feet + raised, grid.root[2]];
    let start = add(centre, scale(right, s.row / 2.0));
    for layer in [Layer::Detailed, Layer::Coarse] {
        batches += 1;
        if let Some(point) = row_search(world, s, start, forward, right, layer) {
            return (Some(point), batches);
        }
    }
    (None, batches)
}

/// One cast of the side row (`FindHorizontalGridPoint`).
fn row_search(world: &dyn World, s: &Settings, start: [f32; 3], forward: [f32; 3], right: [f32; 3], layer: Layer) -> Option<Point> {
    let n = s.row_steps;
    if n < 3 {
        return None;
    }
    let hits = cast_batch(world, start, scale(right, -1.0), s.row, n, forward, s.ray, layer);
    let mut both = vec![false; n + 1];
    let mut least = 1.0f32;
    for i in 1..n - 1 {
        both[i] = hits[i - 1].distance != 0.0 && hits[i + 1].distance != 0.0;
        if !(hits[i].distance > s.edge) {
            least = least.min(dot(hits[i].normal, forward));
        }
    }
    let blocked: Vec<bool> = (0..n).map(|j| j >= 1 && j < n - 1 && hits[j].normal != [0.0; 3] && (dot(hits[j].normal, forward) - least).abs() > 0.001).collect();
    let middle = n / 2;
    let (mut chosen, mut chosen_at, mut chosen_off, mut side) = (missed(), None::<usize>, 0usize, Side::Forward);
    for k in 1..n - 1 {
        let hit = hits[k];
        if blocked[k] || hit.point == [0.0; 3] || dot(hit.normal, UP).abs() > s.side_dot || hit.distance > s.edge {
            continue;
        }
        let off = middle.abs_diff(k);
        let free_after = hits[k + 1].point == [0.0; 3] && !both[k + 1];
        let free_before = hits[k - 1].point == [0.0; 3] && !both[k - 1];
        let better = chosen_at.is_none() || chosen_off > off || (chosen_off == off && chosen.distance > hit.distance);
        if (free_after || free_before) && better {
            side = if hits[k - 1].point == [0.0; 3] { Side::Right } else { Side::Left };
            chosen = hit;
            chosen_at = Some(k);
            chosen_off = off;
        }
    }
    // With nothing chosen the game goes on from the world's origin: its
    // point is the missed hit's nought (kept: the check below then finds
    // nothing on open ground).
    let point = chosen.point;
    let away = if side == Side::Right { right } else { scale(right, -1.0) };
    let facing = normalized([chosen.normal[0], 0.0, chosen.normal[2]]);
    let along = normalized(sub(away, scale(facing, dot(away, facing))));
    let round = add(point, scale(add(along, facing), s.wall_check));
    if world.ray(round, scale(facing, -1.0), s.wall_check, layer).is_some() {
        return None;
    }
    let origin = add(sub(point, scale(facing, s.wall_in)), scale(along, s.wall_check));
    let again = world.ray(origin, scale(along, -1.0), s.wall_check, layer).filter(|h| dot(h.normal, UP).abs() <= s.side_dot)?;
    let at = add(add(point, scale(along, s.wall_check - again.distance)), scale(normalized(add(along, facing)), s.side_point));
    Some(Point { at, direction: scale(facing, -1.0), side })
}

/// The search lying down (`FindMountingPointProne`), answered at once: the
/// grid's root turned by the look's yaw and the ground's pitch; at the
/// bipod's reach ahead a free sphere and a ray down onto level enough
/// ground. `reach` and `height` are the bipod pivot's, unsigned.
pub fn search_prone(world: &dyn World, s: &Settings, root: [f32; 3], yaw: f32, ground_pitch: f32, reach: f32, height: f32) -> Option<Point> {
    let turn = crate::math::quat_mul(quat_axis(1, yaw), quat_axis(0, ground_pitch));
    let forward = quat_rotate(turn, [0.0, 0.0, 1.0]);
    let up = quat_rotate(turn, UP);
    let length = PRONE_RAY_MOST.min(height * 2.0);
    let at = add(root, scale(forward, reach));
    if !world.sphere_clear(at, PRONE_SPHERE, Layer::Coarse) {
        return None;
    }
    let hit = world.ray(at, scale(up, -1.0), length, Layer::Coarse)?;
    (hit.distance != 0.0 && dot(hit.normal, up) >= s.prone_dot).then(|| Point { at: add(hit.point, scale(hit.normal, s.top_point)), direction: forward, side: Side::Forward })
}

// ==== The mount's data ====

/// What the mount's data is worked out from (`TrySetPlayerMountingPointData`,
/// `CheckIfTargetPosReachable`): the character as it stands when the
/// search answers, and its weapon.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Asking {
    pub position: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub pose_level: f32,
    pub prone: bool,
    pub grounded: bool,
    /// The weapon's mount figures, and whether its bipod is out.
    pub weapon: WeaponMount,
    pub bipod_out: bool,
    /// The capsule (`CharacterController`): radius and skin added, height,
    /// step height, and its centre's height over the feet.
    pub capsule: [f32; 4],
}

/// The mount's data (`PlayerMountingPointData.SetData`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Data {
    pub point: Point,
    /// Where the feet go, and the stance the data names (unused by the
    /// game's state, which works its own out at the start).
    pub target: [f32; 3],
    pub target_pose: f32,
    /// The look's target yaw, and the body's (`TargetHandsRotation`,
    /// `TargetBodyRotation`'s yaw).
    pub target_yaw: f32,
    pub body_yaw: f32,
    pub approach: f32,
    pub pose_limit: [f32; 2],
    pub pitch_limit: [f32; 2],
    pub yaw_limit: [f32; 2],
    /// Whether the bipod's pivot is the mount's (forward, with it out).
    pub on_bipod: bool,
}

/// `SetYawLimit`'s and `TrySetPlayerMountingPointData`'s fold of a pair of
/// limits back within a turn either way.
fn fold_limits(limit: [f32; 2]) -> [f32; 2] {
    if limit[0] > 360.0 || limit[1] > 360.0 {
        [limit[0] - 360.0, limit[1] - 360.0]
    } else if limit[0] < -360.0 || limit[1] < -360.0 {
        [limit[0] + 360.0, limit[1] + 360.0]
    } else {
        limit
    }
}

/// A yaw within a turn either way, and the same yaw the other way round
/// (`ClampAngle360Sensitive`'s first steps).
fn both_ways(yaw: f32) -> (f32, f32) {
    let mut a = yaw;
    if a < -360.0 {
        a += 360.0;
    }
    if a > 360.0 {
        a -= 360.0;
    }
    (a, if a > 0.0 { a - 360.0 } else { a + 360.0 })
}

/// The mount's data for a point found, or nothing where the game refuses
/// it (the pitch or yaw too far off, the target place not reached).
pub fn data_for(world: &dyn World, s: &Settings, point: Point, a: &Asking) -> Option<Data> {
    let forward = point.side == Side::Forward;
    let on_bipod = forward && a.bipod_out;
    let pivot = if on_bipod { a.weapon.bipod_pivot.unwrap_or([0.0; 3]) } else { a.weapon.pivot };
    let reach = pivot[1].abs();
    let half_span = (TILT_POSITION_OFFSET / reach).atan().to_degrees() - 1.0;
    let mut pitch_limit = if !forward { s.pitch_side } else if a.bipod_out { s.pitch_bipod } else { s.pitch_forward };
    if a.pitch < pitch_limit[0] - s.pitch_excess || a.pitch > pitch_limit[1] + s.pitch_excess {
        return None;
    }
    let (yaw_limit, target_yaw, body_yaw) = if !forward {
        let facing = bearing(point.direction);
        let end = facing + point.side.number() as f32 * s.side_span;
        let limit = fold_limits([facing.min(end), facing.max(end)]);
        let square = bearing(quat_rotate(quat_axis(1, if point.side == Side::Right { 90.0 } else { -90.0 }), point.direction));
        let (to_low, to_high) = (delta_angle(square, limit[0]).abs(), delta_angle(square, limit[1]).abs());
        let (yaw, other) = both_ways(a.yaw);
        let outside = |y: f32| y < limit[0] - s.yaw_excess || y > limit[1] + s.yaw_excess;
        if outside(yaw) && outside(other) {
            return None;
        }
        (limit, if to_low < to_high { limit[0] } else { limit[1] }, facing)
    } else {
        (fold_limits([a.yaw - half_span, a.yaw + half_span]), a.yaw, a.yaw)
    };
    if a.prone {
        let (pitch, yaw) = a.weapon.bipod.map_or(([0.0; 2], [0.0; 2]), |b| (b.pitch_prone, b.yaw_prone));
        return Some(Data {
            point,
            target: a.position,
            target_pose: a.pose_level,
            target_yaw,
            body_yaw,
            approach: s.approach[0],
            pose_limit: [0.0, 0.0],
            pitch_limit: pitch,
            yaw_limit: [target_yaw + yaw[0], target_yaw + yaw[1]],
            on_bipod,
        });
    }
    let offset: [f32; 3] = std::array::from_fn(|k| lerp(SIT_OFFSET[k], STAND_OFFSET[k], a.pose_level));
    let back = quat_rotate(quat_axis(1, target_yaw), [offset[0], 0.0, reach + offset[2]]);
    let mut target = [point.at[0] - back[0], point.at[1], point.at[2] - back[2]];
    target[1] = world.ray(target, [0.0, -1.0, 0.0], TARGET_GROUND_RAY, Layer::Static).map_or(a.position[1], |h| h.point[1]);
    if !reachable(world, a, target, body_yaw) {
        return None;
    }
    let share = inverse_lerp(s.grid[0], s.grid[1], point.at[1] - target[1] - pivot[2]);
    let top = (share - pitch_limit[0] / PITCH_PER_POSE).clamp(0.0, 1.0);
    let bottom = (share - pitch_limit[1] / PITCH_PER_POSE).clamp(0.0, 1.0);
    let pose_limit = [bottom, top];
    pitch_limit = [-(share - bottom) * PITCH_PER_POSE, (top - share) * PITCH_PER_POSE];
    let target_pose = lerp(pose_limit[0], pose_limit[1], inverse_lerp(pitch_limit[0], pitch_limit[1], a.pitch));
    let distance = magnitude(sub(a.position, target));
    let mut approach = lerp(s.approach[0], s.approach[1], inverse_lerp(0.0, s.edge, distance));
    if a.pitch < pitch_limit[0] || a.pitch > pitch_limit[1] {
        let nearer = if (a.pitch - pitch_limit[0]).abs() <= (a.pitch - pitch_limit[1]).abs() { pitch_limit[0] } else { pitch_limit[1] };
        let pitch_off = if forward { a.pitch.abs() } else { (a.pitch - nearer).abs() };
        let yaw_off = (a.yaw - target_yaw).abs();
        approach *= 1.0 + (pitch_off + yaw_off) * s.approach_per_degree;
    }
    approach = approach.clamp(s.approach[0], s.approach[1]);
    Some(Data { point, target, target_pose, target_yaw, body_yaw, approach, pose_limit, pitch_limit, yaw_limit, on_bipod })
}

/// `CheckIfTargetPosReachable`: on the ground, and a box from the feet
/// raised by the step and the capsule's centre, swept to the target place
/// turned to the body's target yaw, meets nothing static.
fn reachable(world: &dyn World, a: &Asking, target: [f32; 3], body_yaw: f32) -> bool {
    if !a.grounded {
        return false;
    }
    let [radius, height, step, centre] = a.capsule;
    let way = sub(target, a.position);
    let length = magnitude(way) + radius / 4.0;
    let from = [a.position[0], a.position[1] + step + centre, a.position[2]];
    let side = (radius / 2.0).sqrt();
    let half = [side / 2.0, (height - step / 2.0) / 2.0, side / 2.0];
    !world.box_sweep(from, half, body_yaw, normalized(way), length, Layer::Static)
}

// ==== The weapon's figures ====

/// A bipod's figures (`BipodTemplate`, the M60E6's: recoil 0.2, camera
/// snap 1, breath 0.1, return 1, prone pitch -4 to 6, yaw ±15), and
/// whether it folds forward.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BipodFigures {
    pub recoil: f32,
    pub camera_snap: f32,
    pub breath: f32,
    pub return_speed: f32,
    pub pitch_prone: [f32; 2],
    pub yaw_prone: [f32; 2],
    pub forward: bool,
}

/// A weapon's mount figures (its template's; mounting spec, "The mount's
/// data", "The weapon's motion while mounted"), with its pivots in the
/// axes of the weapon's `Weapon_root`, as the game uses them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponMount {
    /// `IsMountable`.
    pub mountable: bool,
    /// `MountingRotationCenter`: the template's `MountingPosition`, or the
    /// left hand's marker plus the moved bone's place plus (0.05, -0.18,
    /// 0.025) where that is nought.
    pub pivot: [f32; 3],
    /// `MountingRotationCenterBipods`: the bipod's ground bone (nought for a
    /// bipod without one; none without a bipod).
    pub bipod_pivot: Option<[f32; 3]>,
    /// `MountVerticalRecoilMultiplier`, `MountHorizontalRecoilMultiplier`.
    pub recoil_side: f32,
    pub recoil_forward: f32,
    /// `MountCameraSnapMultiplier`, `MountReturnSpeedHandMultiplier`.
    pub camera_snap: f32,
    pub return_speed: f32,
    /// `MountingVerticalOutOfBreathMultiplier`, `MountingHorizontalOutOfBreathMultiplier`.
    pub breath_side: f32,
    pub breath_forward: f32,
    pub bipod: Option<BipodFigures>,
}

impl Default for WeaponMount {
    fn default() -> Self {
        Self {
            mountable: false,
            pivot: [0.0; 3],
            bipod_pivot: None,
            recoil_side: 0.75,
            recoil_forward: 0.6,
            camera_snap: 1.0,
            return_speed: 1.0,
            breath_side: 0.8,
            breath_forward: 0.4,
            bipod: None,
        }
    }
}

/// The name of the left hand's marker on a weapon.
const LEFT_MARKER: &str = "weapon_L_hand_marker";

impl WeaponMount {
    /// A weapon's figures from its template and its assembled model.
    pub fn of_weapon(db: &trp_data::Database, weapon: &trp_data::weapon::Weapon) -> Result<Self> {
        let template = &db.item(&weapon.template)?.props;
        let number = |v: &Json, name: &str, or: f32| v.get(name).and_then(Json::as_f64).map_or(or, |n| n as f32);
        let triple = |v: &Json, name: &str| -> [f32; 3] {
            let p = v.get(name);
            let f = |k: &str| p.and_then(|p| p.get(k)).and_then(Json::as_f64).map_or(0.0, |n| n as f32);
            [f("x"), f("y"), f("z")]
        };
        let hierarchy = &weapon.hierarchy;
        let moved = crate::rig::WeaponRig::new(hierarchy).moved_bone();
        let moved_at = moved.map_or([0.0; 3], |m| hierarchy.nodes[m].translation);
        let mut pivot = triple(template, "MountingPosition");
        if pivot == [0.0; 3] {
            pivot = match hierarchy.find_below(0, LEFT_MARKER) {
                Some(marker) => add(add(hierarchy.nodes[marker].translation, moved_at), MARKER_TO_PIVOT),
                None => [0.0; 3],
            };
        }
        let bipod = weapon.bipod.as_ref();
        let bipod_pivot = bipod.map(|part| {
            // The ground bone in the axes of the moved bone's child it
            // hangs under (`FindBipodMountingPoint`: up to, not through, it).
            let Some(ground) = part.ground else { return [0.0; 3] };
            let node = &hierarchy.nodes[ground];
            let mut at = node.translation;
            let mut parent = node.parent;
            while let Some(p) = parent {
                let above = hierarchy.nodes[p].parent;
                if above.is_none() || above == moved {
                    break;
                }
                at = add(quat_rotate(hierarchy.nodes[p].rotation, at), hierarchy.nodes[p].translation);
                parent = above;
            }
            at
        });
        let bipod_figures = match bipod {
            Some(part) => {
                let props = &db.item(&part.template)?.props;
                let pair = |name: &str, or: [f32; 2]| -> [f32; 2] {
                    let p = props.get(name);
                    let f = |k: &str, o: f32| p.and_then(|p| p.get(k)).and_then(Json::as_f64).map_or(o, |n| n as f32);
                    [f("x", or[0]), f("y", or[1])]
                };
                Some(BipodFigures {
                    recoil: number(props, "BipodRecoilMultiplier", 0.2),
                    camera_snap: number(props, "BipodCameraSnapMultiplier", 1.0),
                    breath: number(props, "BipodOutOfStaminaBreathMultiplier", 0.1),
                    return_speed: number(props, "BipodReturnHandSpeedMultiplier", 1.0),
                    pitch_prone: pair("PitchLimitProneBipod", [-4.0, 6.0]),
                    yaw_prone: pair("YawLimitProneBipod", [-15.0, 15.0]),
                    forward: part.forward,
                })
            }
            None => None,
        };
        Ok(Self {
            mountable: template.get("IsMountable").and_then(Json::as_bool).unwrap_or(false),
            pivot,
            bipod_pivot,
            recoil_side: number(template, "MountVerticalRecoilMultiplier", 0.75),
            recoil_forward: number(template, "MountHorizontalRecoilMultiplier", 0.6),
            camera_snap: number(template, "MountCameraSnapMultiplier", 1.0),
            return_speed: number(template, "MountReturnSpeedHandMultiplier", 1.0),
            breath_side: number(template, "MountingVerticalOutOfBreathMultiplier", 0.8),
            breath_forward: number(template, "MountingHorizontalOutOfBreathMultiplier", 0.4),
            bipod: bipod_figures,
        })
    }
}

// ==== What a mount tells its host ====

/// What the mount did in a frame, for the host: the weapon's motion, the
/// weapon's animator, the sounds and the ergonomics go by it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MountEvent {
    /// Entered (`EnterMountedState`): the approach's length, the side,
    /// whether the bipod's pivot is the mount's, and lying down.
    Entered { approach: f32, side: Side, on_bipod: bool, prone: bool },
    /// The exit has begun (`StartExiting`): it lasts the exit time.
    Leaving { side: Side },
    /// The mount's state is taken off (`ExitMountedState`).
    Left { side: Side },
    /// A search found nothing, lying down there was nothing to search with,
    /// or what it found was refused (`TrySetPlayerMountingPointData` false,
    /// then `NoPointFound`): `OnNothingFound`, the battle screen's mount
    /// icon flashes red (`BattleUiBipodPanel.OnMountUnavailable`).
    Refused,
}

/// What the weapon's motion is told of a mount (`SetMountingData`): the
/// pivot it turns about, in the moved bone's axes; whether at a side; and
/// whether the bipod is out (`IsBipodUsed`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HandsMount {
    pub pivot: [f32; 3],
    pub side: bool,
    pub bipod: bool,
}

/// The weapon's motion's figures for a weapon on a mount (`recoil`, its
/// camera snap, its return, the breath out of stamina, the sway with the
/// bipod): what the motion reads while `HandsMount` is set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MountedMotion {
    pub recoil: f32,
    pub return_speed: f32,
    pub camera_snap: f32,
    pub breath: f32,
    pub sway: f32,
    pub tactical: bool,
}

impl WeaponMount {
    /// What the weapon's motion goes by on a mount at `side`, the bipod out
    /// or not.
    pub fn motion(&self, s: &Settings, side: bool, bipod_out: bool) -> MountedMotion {
        let on_bipod = !side && bipod_out;
        let bipod = self.bipod.filter(|_| bipod_out);
        MountedMotion {
            recoil: if side { self.recoil_side } else if let (true, Some(b)) = (on_bipod, bipod) { b.recoil } else { self.recoil_forward },
            return_speed: if let (true, Some(b)) = (on_bipod, bipod) { b.return_speed } else { self.return_speed },
            camera_snap: if let (true, Some(b)) = (on_bipod, bipod) { b.camera_snap } else { self.camera_snap },
            breath: if side { self.breath_side } else if let (true, Some(b)) = (on_bipod, bipod) { b.breath } else { self.breath_forward },
            sway: if on_bipod { s.sway_with_bipod } else { 1.0 },
            tactical: on_bipod,
        }
    }

    /// The pivot a mount turns the weapon about: the bipod's forward with it
    /// out, else the mount's.
    pub fn pivot_for(&self, side: bool, bipod_out: bool) -> [f32; 3] {
        if !side && bipod_out { self.bipod_pivot.unwrap_or([0.0; 3]) } else { self.pivot }
    }
}

/// After the movement's frame: what the mount did goes to the weapon and
/// its motion, as the game's `EnterMountedState`, `StartExitingMountedState`
/// and `ExitMountedState` tell them in the same call (the animator's
/// `Mounted`, `SetMountingData`, the mount's way of moving), and the
/// ergonomics take the class skill's mounted gain (`mount_ergonomics`: the
/// class's `WeaponBuffs::mount_ergonomics`, while mounted and forward on the
/// bipod). The events are handed back for the host's sounds and screen.
pub fn tell(
    movement: &mut crate::movement::Movement,
    mut firearm: Option<&mut crate::firearm::Firearm>,
    mut hands: Option<&mut crate::hands::Hands>,
    mount_ergonomics: [f32; 2],
) -> Vec<MountEvent> {
    let events = movement.take_mount_events();
    for event in &events {
        match *event {
            MountEvent::Entered { side, on_bipod, .. } => {
                let bipod_out = firearm.as_ref().and_then(|f| f.bipod()).unwrap_or(false);
                if let Some(f) = firearm.as_deref_mut() {
                    f.set_mounted(true, on_bipod);
                }
                // A canted sight gives way to a straight one; with none, the
                // sights go down (`OnFoundPoint`, `ToggleFirearmAimByBodyAction`).
                let canted = hands.as_deref_mut().is_some_and(|h| h.straighten_for_mount());
                if let (true, Some(f)) = (canted, firearm.as_deref_mut()) {
                    if f.aiming() {
                        f.set_aim(false, true);
                    }
                }
                if let Some(h) = hands.as_deref_mut() {
                    let at_side = side != Side::Forward;
                    let pivot = h.settings.mount.pivot_for(at_side, bipod_out);
                    h.set_mount_strategy(true);
                    h.set_mounting(Some(HandsMount { pivot, side: at_side, bipod: bipod_out }));
                    h.set_mount_ergonomics(if on_bipod { mount_ergonomics[1] } else { mount_ergonomics[0] });
                }
            }
            MountEvent::Leaving { .. } => {
                if let Some(f) = firearm.as_deref_mut() {
                    f.set_mounted(false, false);
                }
                if let Some(h) = hands.as_deref_mut() {
                    h.set_mounting(None);
                }
            }
            MountEvent::Left { .. } => {
                if let Some(f) = firearm.as_deref_mut() {
                    f.set_mounted(false, false);
                }
                if let Some(h) = hands.as_deref_mut() {
                    h.set_mounting(None);
                    h.set_mount_strategy(false);
                    h.set_mount_ergonomics(0.0);
                }
            }
            MountEvent::Refused => {}
        }
    }
    events
}

/// The arms' drain multiplier aiming on a mount: at a side, forward with
/// the bipod out, forward (`Physical`).
pub fn drain(s: &Settings, side: Side, bipod_out: bool) -> f32 {
    match side {
        Side::Left | Side::Right => s.drain[0],
        Side::Forward if bipod_out => s.drain[2],
        Side::Forward => s.drain[1],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movement::Flat;

    /// A wall ahead with a top at `height`, its face at z = `face`, and the
    /// ground at 0.
    struct Sill {
        face: f32,
        height: f32,
        depth: f32,
    }

    impl World for Sill {
        fn ground_height(&self, _x: f32, _z: f32) -> f32 {
            0.0
        }

        fn ray(&self, from: [f32; 3], direction: [f32; 3], length: f32, _layer: Layer) -> Option<Hit> {
            let block = crate::collision::Block::square([-5.0, 0.0, self.face], [5.0, self.height, self.face + self.depth]);
            let to = add(from, scale(direction, length));
            let solid = block.entered_face(from, to).map(|(t, normal)| (t * length, normal));
            let ground = Flat.ray(from, direction, length, Layer::Detailed).map(|h| (h.distance, h.normal));
            let (distance, normal) = solid.into_iter().chain(ground).min_by(|a, b| a.0.total_cmp(&b.0))?;
            Some(Hit { distance, point: add(from, scale(direction, distance)), normal })
        }
    }

    #[test]
    fn a_sill_ahead_is_found_on_the_second_frame() {
        let s = Settings::default();
        let world = Sill { face: 0.6, height: 1.0, depth: 0.3 };
        let grid = Grid { root: [0.0, 1.35, 0.0], yaw: 0.0, feet: 0.0, column_raise: 0.03, row_raise: 0.03 };
        let mut search = Search::standing(&world, &s, grid);
        assert_eq!(search.frame(), None);
        let point = search.frame().expect("answers on the second frame").expect("found");
        assert_eq!(point.side, Side::Forward);
        assert!((point.at[1] - 1.02).abs() < 1e-3, "{point:?}");
        assert!((point.at[2] - 0.6).abs() < 0.02, "{point:?}");
        assert!((point.direction[2] - 1.0).abs() < 1e-4, "faces the wall turned round: {point:?}");
    }

    #[test]
    fn open_ground_answers_nothing_on_the_third_frame() {
        let s = Settings::default();
        let grid = Grid { root: [0.0, 1.35, 0.0], yaw: 30.0, feet: 0.0, column_raise: 0.03, row_raise: 0.03 };
        let mut search = Search::standing(&Flat, &s, grid);
        assert_eq!(search.frame(), None);
        assert_eq!(search.frame(), None);
        assert_eq!(search.frame(), Some(None));
    }

    #[test]
    fn the_approach_is_two_tenths_and_more_by_the_distance() {
        // The M60E6's second `05-mount`: 0.243 m off, 0.36195 s.
        let s = Settings::default();
        let d = 0.2429f32;
        let approach = lerp(s.approach[0], s.approach[1], inverse_lerp(0.0, s.edge, d));
        assert!((approach - 0.3619).abs() < 2e-4, "{approach}");
    }

    #[test]
    fn a_forward_mount_s_data_cuts_the_pitch_to_the_stance() {
        let s = Settings::default();
        let world = Sill { face: 1.0, height: 1.0, depth: 0.3 };
        let weapon = WeaponMount { mountable: true, pivot: [0.0, -0.687, 0.03], ..WeaponMount::default() };
        let point = Point { at: [0.0, 1.02, 1.0], direction: [0.0, 0.0, 1.0], side: Side::Forward };
        let asking = Asking { position: [0.0, 0.0, 0.0], yaw: 0.0, pitch: 0.0, pose_level: 1.0, prone: false, grounded: true, weapon, bipod_out: false, capsule: [0.35, 1.8, 0.3, 0.9] };
        let data = data_for(&world, &s, point, &asking).expect("taken");
        // ±(atan(0.25 / 0.687) - 1): the M700's recorded ±18.98.
        assert!((data.yaw_limit[1] - 18.99).abs() < 0.02, "{data:?}");
        let share = inverse_lerp(0.81, 1.4, 1.02 - 0.03);
        assert!((data.pose_limit[0] - (share - 0.3)).abs() < 1e-4 && (data.pose_limit[1] - (share + 0.3)).abs() < 1e-4, "{data:?}");
        assert!((data.target[2] - (1.0 - 0.687 - 0.074)).abs() < 1e-4, "{data:?}");
    }
}
