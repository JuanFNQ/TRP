//! Shot groups, curve recoil, the recoil settings and a recoil process.

use super::*;

/// One group of shots within a burst that adds to the weapon's base values.
#[derive(Debug, Clone, Copy)]
pub struct ShotGroup {
    pub start: i32,
    pub end: i32,
    pub rotation_strength: [f32; 2],
    pub position_strength: [f32; 2],
    /// Added to the two ends of the recoil angle range, in degrees.
    pub angle_range: [f32; 2],
}

/// The weapon's own curve played from each shot.
#[derive(Debug, Clone)]
pub struct CurveRecoil {
    /// 0 camera rotation, 1 hands position, 2 hands rotation.
    pub target: u8,
    pub axis: usize,
    pub time_multiply: f32,
    pub value_multiply: f32,
    pub aiming_value_multiply: f32,
    pub curve: Curve,
}

/// What the weapon's template sets on the rotation spring.
#[derive(Debug, Clone)]
pub struct RotationSettings {
    pub return_speed: f32,
    pub damping: f32,
    pub category_multiplier: f32,
    pub return_path_offset: f32,
    pub return_path_damping: f32,
    pub stable_angle_increase_step: f32,
    pub progress_angle_on_stable: [f32; 2],
    pub after_recoil_vertical_range: [f32; 2],
    pub after_recoil_horizontal_range: [f32; 2],
    pub reset_after_shot: bool,
    pub to_next_stable_point_distance_limit: f32,
    pub next_stable_point_distance_range: [f32; 2],
    pub to_next_current_speed: f32,
    pub return_speed_curve: Curve,
}

/// Everything about recoil for one weapon build, resolved.
#[derive(Debug, Clone)]
pub struct RecoilSettings {
    pub progress_power_on: bool,
    pub progress_power_shot: i32,
    /// The shot of a burst from which the weapon holds around one point.
    pub stable_shot_index: i32,
    pub shot_vals: Vec<RecoilShotVal>,
    /// Range the rotation and position strength of a shot is drawn from.
    pub rotation_strength: [f32; 2],
    pub position_strength: [f32; 2],
    /// Range of the recoil's direction, in radians; a quarter turn is straight up.
    pub angle_range: [f32; 2],
    pub groups: Vec<ShotGroup>,
    /// Scale of the three axes of a shot, by stance: prone, crouched, standing.
    pub intensity_by_pose: [[f32; 3]; 3],
    /// Scale of the rotation spring's kick while aiming.
    pub aim_intensity: f32,
    pub rotation: RotationSettings,
    pub hand_position: RecoilProcessData,
    pub camera_rotation: RecoilProcessData,
    pub curves: Vec<CurveRecoil>,
}

impl RecoilSettings {
    /// The shot effector as the player prefab has it, with nothing a
    /// weapon's template would set: for hands that hold no weapon, which
    /// never make a shot.
    pub fn of_prefab(base: &trp_data::character::player::RecoilData) -> Self {
        Self {
            progress_power_on: base.progress_power_on,
            progress_power_shot: base.progress_power_shot,
            stable_shot_index: 0,
            shot_vals: base.shot_vals.clone(),
            rotation_strength: [0.0; 2],
            position_strength: [0.0; 2],
            angle_range: [0.0; 2],
            groups: Vec::new(),
            intensity_by_pose: [[1.0; 3]; 3],
            aim_intensity: 1.0,
            rotation: RotationSettings {
                return_speed: base.hand_rotation.return_speed,
                damping: base.hand_rotation.damping,
                category_multiplier: 1.0,
                return_path_offset: 0.0,
                return_path_damping: 0.0,
                stable_angle_increase_step: 0.0,
                progress_angle_on_stable: [0.0; 2],
                after_recoil_vertical_range: [0.0; 2],
                after_recoil_horizontal_range: [0.0; 2],
                reset_after_shot: false,
                to_next_stable_point_distance_limit: base.to_next_stable_point_distance_limit,
                next_stable_point_distance_range: base.next_stable_point_distance_range,
                to_next_current_speed: base.to_next_current_speed,
                return_speed_curve: base.return_speed_curve.clone(),
            },
            hand_position: base.hand_position,
            camera_rotation: base.camera_rotation,
            curves: Vec::new(),
        }
    }
}

/// The position and camera springs.
#[derive(Debug, Clone)]
pub struct Process {
    pub(super) data: RecoilProcessData,
    /// Where the spring rests; other effects move it.
    pub offset: [f32; 3],
    pub(super) velocity: [f32; 3],
    pub current: [f32; 3],
    pub(super) force: [f32; 3],
    pub(super) need_update: bool,
}

impl Process {
    pub(super) fn new(data: RecoilProcessData) -> Self {
        Self { data, offset: [0.0; 3], velocity: [0.0; 3], current: [0.0; 3], force: [0.0; 3], need_update: false }
    }

    pub(super) fn push(&mut self, axis: usize, value: f32) {
        self.force[axis] += value;
        self.need_update = true;
    }

    pub(super) fn step(&mut self) {
        if self.need_update {
            self.velocity = add(self.velocity, scale(self.force, self.data.intensity));
            self.force = [0.0; 3];
            self.need_update = false;
        }
        self.velocity = sub(self.velocity, scale(sub(self.current, self.offset), self.data.return_speed));
        self.velocity = scale(self.velocity, self.data.damping);
        self.current = add(self.current, self.velocity);
    }

    pub fn value(&self) -> [f32; 3] {
        add(self.offset, self.current)
    }
}
