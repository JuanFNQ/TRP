//! What a mount asks for and the data worked out for it.

use super::*;

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
