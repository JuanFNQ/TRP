//! The mount's settings read from the install.

use super::*;

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
    /// The install's figures (globals.json), for a host
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
