//! Laser figures, the laser's light and the head pose.


/// What a laser draws (`LaserBeam.LateUpdate`; tactical-devices spec, "The
/// look"), for a hit `distance` along its ray (cast from `ray_start` ahead
/// of its object, `max_distance` long; k the distance's share of it) or for
/// none: the beam's length and brightness, the dot's size, and its small
/// light's cone and brightness.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaserFigures {
    pub length: f32,
    pub brightness: f32,
    pub dot_size: f32,
    pub light_angle: f32,
    pub light_intensity: f32,
}

pub fn laser_figures(laser: &trp_data::weapons::tactical::DeviceLaser, distance: Option<f32>) -> LaserFigures {
    let k = distance.map_or(1.0, |d| (d / laser.max_distance).clamp(0.0, 1.0));
    let (length, brightness) = match distance {
        Some(d) => (d + laser.ray_start, (1.0 - k) * laser.intensity_factor),
        None => (laser.max_distance, laser.intensity_factor),
    };
    LaserFigures {
        length,
        brightness,
        dot_size: laser.point_size[0] + (laser.point_size[1] - laser.point_size[0]) * k,
        // (The engine takes no cone past 179 degrees.)
        light_angle: (laser.angle_close_far[0] + (laser.angle_close_far[1] - laser.angle_close_far[0]) * k).min(179.0),
        light_intensity: distance.map_or(0.0, |_| (1.0 - k) * laser.light_intensity),
    }
}

/// Where a laser's dot's light stands: `offset` off the hit point along the
/// surface's normal less the beam's way (along the normal where the two
/// cancel).
pub fn laser_light_at(point: [f32; 3], normal: [f32; 3], way: [f32; 3], offset: f32) -> [f32; 3] {
    let away: [f32; 3] = std::array::from_fn(|i| normal[i] - way[i]);
    let length = away.iter().map(|v| v * v).sum::<f32>().sqrt();
    let away = if length > 1e-6 { away.map(|v| v / length) } else { normal };
    std::array::from_fn(|i| point[i] + away[i] * offset)
}

/// A Unity rotation (`xyzw`) from its Euler angles in degrees
/// (`Quaternion.Euler`: about z, then x, then y).
pub fn quat_of_euler([x, y, z]: [f32; 3]) -> [f32; 4] {
    let half = |a: f32| (a.to_radians() / 2.0).sin_cos();
    let ((sx, cx), (sy, cy), (sz, cz)) = (half(x), half(y), half(z));
    // q = qy · qx · qz
    let [ax, ay, az, aw] = [cy * sx, sy * cx, -sy * sx, cy * cx];
    [ax * cz + ay * sz, ay * cz - ax * sz, aw * sz + az * cz, aw * cz - az * sz]
}

/// A Unity rotation's Euler angles in degrees, each in [0, 360)
/// (`Quaternion.eulerAngles`).
pub fn euler_of([x, y, z, w]: [f32; 4]) -> [f32; 3] {
    let pitch = (2.0 * (w * x - y * z)).clamp(-1.0, 1.0).asin();
    let yaw = (2.0 * (w * y + x * z)).atan2(1.0 - 2.0 * (x * x + y * y));
    let roll = (2.0 * (w * z + x * y)).atan2(1.0 - 2.0 * (x * x + z * z));
    [pitch, yaw, roll].map(|a| a.to_degrees().rem_euclid(360.0))
}

/// A vector turned by a rotation (`xyzw`).
pub fn turn([x, y, z, w]: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    // v + 2w(q×v) + 2q×(q×v)
    let q = [x, y, z];
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let t = cross(q, v).map(|c| 2.0 * c);
    let u = cross(q, t);
    [v[0] + w * t[0] + u[0], v[1] + w * t[1] + u[1], v[2] + w * t[2] + u[2]]
}

/// A head device's light or laser placed by the camera, in the game's axes
/// (`HeadLightsValidator.Update`, each frame for one's own player): at the
/// camera, moved 0.2 m to its side, up and ahead along its own axes as
/// turned the frame before, then turned as the camera, 10° down and 3°
/// toward the middle, and by the field of view below 50 (`fov`, its whole
/// degrees taken). Its place and its new turn.
pub fn head_pose(camera_at: [f32; 3], camera_turn: [f32; 4], fov: f32, right: bool, turned_before: [f32; 4]) -> ([f32; 3], [f32; 4]) {
    let below = (50.0 - fov.floor()).clamp(0.0, 50.0);
    let euler = euler_of(camera_turn);
    let offset = if right { [0.2, 0.2, 0.2] } else { [-0.2, 0.2, 0.2] };
    let moved = turn(turned_before, offset);
    let place = [camera_at[0] + moved[0], camera_at[1] + moved[1], camera_at[2] + moved[2]];
    let added = if right { [10.0, -3.0, 0.0] } else { [10.0, 3.0, 0.0] };
    let new = [euler[0] + added[0] - 0.2 * below, euler[1] + added[1] - 0.45 * below, euler[2] + added[2]];
    (place, quat_of_euler(new))
}
