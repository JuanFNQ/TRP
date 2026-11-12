//! The handful of Unity math functions the game's formulas are written in.
//! They are reproduced as Unity defines them, clamping included, because
//! the formulas depend on those details.

/// `Mathf.Clamp`, which unlike Rust's accepts a lower bound above the upper
/// one (Rust's stops the program): the lower then wins below it and the
/// upper above it. The game relies on what it returns then (a sprint's
/// limit of nought after a hard landing: `SprintAcceleration`).
pub fn unity_clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// `Mathf.Lerp`: `t` is clamped to 0..1.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// `Mathf.InverseLerp`: clamped to 0..1, zero for an empty range.
pub fn inverse_lerp(a: f32, b: f32, value: f32) -> f32 {
    if a == b {
        0.0
    } else {
        ((value - a) / (b - a)).clamp(0.0, 1.0)
    }
}

/// `Mathf.DeltaAngle`: shortest signed difference in degrees, -180..180.
pub fn delta_angle(current: f32, target: f32) -> f32 {
    let mut d = (target - current).rem_euclid(360.0);
    if d > 180.0 {
        d -= 360.0;
    }
    d
}

/// The angle from the body's bearing round to the view's as the game
/// takes it: `Mathf.DeltaAngle` of the two with 360 added to each first.
/// The body's bearing is read off its rotation, 0 to 360, and the view's
/// is kept within a turn either way of nothing. The sums are single
/// precision past 256 or 512, so the angle comes in steps of 1/32768 or
/// 1/16384 of a degree, and a body held at the limit behind the view
/// reads the limit exactly (movement spec, "Looking").
pub fn body_to_view(body_yaw: f32, yaw: f32) -> f32 {
    delta_angle(360.0 + body_yaw.rem_euclid(360.0), 360.0 + yaw % 360.0)
}

/// `Mathf.LerpAngle`.
pub fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    a + delta_angle(a, b) * t.clamp(0.0, 1.0)
}

/// `Quaternion.Lerp` between two turns about the same axis, given and
/// returned as angles in degrees. The engine mixes the two quaternions'
/// components and makes the result a unit again, which is a little short
/// of `t` of the angle between them.
pub fn lerp_turn(a: f32, b: f32, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let half = (delta_angle(a, b) / 2.0).to_radians();
    a + 2.0 * (t * half.sin()).atan2(1.0 - t + t * half.cos()).to_degrees()
}

pub fn length(v: [f32; 2]) -> f32 {
    (v[0] * v[0] + v[1] * v[1]).sqrt()
}

pub fn normalize(v: [f32; 2]) -> [f32; 2] {
    let len = length(v);
    if len > 1e-5 {
        [v[0] / len, v[1] / len]
    } else {
        [0.0; 2]
    }
}

pub fn dot(a: [f32; 2], b: [f32; 2]) -> f32 {
    a[0] * b[0] + a[1] * b[1]
}

/// `Vector2.SmoothDamp` and `Vector3.SmoothDamp`: a critically damped
/// spring toward `target`.
pub fn smooth_damp<const N: usize>(
    current: [f32; N],
    target: [f32; N],
    velocity: &mut [f32; N],
    smooth_time: f32,
    max_speed: f32,
    dt: f32,
) -> [f32; N] {
    let smooth_time = smooth_time.max(0.0001);
    let omega = 2.0 / smooth_time;
    let x = omega * dt;
    let exp = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);

    let mut change: [f32; N] = std::array::from_fn(|i| current[i] - target[i]);
    let max_change = max_speed * smooth_time;
    let change_len = change.iter().map(|c| c * c).sum::<f32>().sqrt();
    if change_len > max_change && change_len > 0.0 {
        change = change.map(|c| c / change_len * max_change);
    }

    let mut out = [0.0f32; N];
    for i in 0..N {
        let clamped_target = current[i] - change[i];
        let temp = (velocity[i] + omega * change[i]) * dt;
        velocity[i] = (velocity[i] - omega * temp) * exp;
        out[i] = clamped_target + (change[i] + temp) * exp;
    }
    // Do not overshoot the target.
    let overshoot: f32 = (0..N).map(|i| (target[i] - current[i]) * (out[i] - target[i])).sum();
    if overshoot > 0.0 {
        out = target;
        *velocity = [0.0; N];
    }
    out
}

/// Quaternion `xyzw` for a rotation of `degrees` about one axis (0 x, 1 y, 2 z).
pub fn quat_axis(axis: usize, degrees: f32) -> [f32; 4] {
    let (sin, cos) = (degrees.to_radians() * 0.5).sin_cos();
    let mut q = [0.0, 0.0, 0.0, cos];
    q[axis] = sin;
    q
}

/// Hamilton product: `b` applied first, then `a`.
pub fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let ([ax, ay, az, aw], [bx, by, bz, bw]) = (a, b);
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

pub fn quat_rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let u = [q[0], q[1], q[2]];
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let t = cross(u, v).map(|c| c * 2.0);
    let c = cross(u, t);
    [v[0] + q[3] * t[0] + c[0], v[1] + q[3] * t[1] + c[1], v[2] + q[3] * t[2] + c[2]]
}

/// `Quaternion.Euler`: about z first, then x, then y. Degrees.
pub fn quat_euler(degrees: [f32; 3]) -> [f32; 4] {
    quat_mul(quat_axis(1, degrees[1]), quat_mul(quat_axis(0, degrees[0]), quat_axis(2, degrees[2])))
}

/// `Quaternion.Lerp`: straight blend along the shorter way, renormalised.
/// `t` is clamped to 0..1.
pub fn quat_lerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    let dot: f32 = (0..4).map(|i| a[i] * b[i]).sum();
    let sign = if dot < 0.0 { -1.0 } else { 1.0 };
    quat_normalize(std::array::from_fn(|i| a[i] + (sign * b[i] - a[i]) * t))
}

/// `Quaternion.LookRotation`: the rotation whose forward (z) is along
/// `forward` and whose up (y) leans toward `up`. `None` where the two give
/// no frame: no direction, or a direction along `up`.
pub fn quat_look(forward: [f32; 3], up: [f32; 3]) -> Option<[f32; 4]> {
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let unit = |v: [f32; 3]| {
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        (len > 1e-6).then(|| v.map(|c| c / len))
    };
    let z = unit(forward)?;
    let x = unit(cross(up, z))?;
    let y = cross(z, x);
    // The matrix with those three as its columns, as a quaternion.
    let trace = x[0] + y[1] + z[2];
    let q = if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        [(y[2] - z[1]) / s, (z[0] - x[2]) / s, (x[1] - y[0]) / s, 0.25 * s]
    } else if x[0] > y[1] && x[0] > z[2] {
        let s = (1.0 + x[0] - y[1] - z[2]).sqrt() * 2.0;
        [0.25 * s, (y[0] + x[1]) / s, (z[0] + x[2]) / s, (y[2] - z[1]) / s]
    } else if y[1] > z[2] {
        let s = (1.0 + y[1] - x[0] - z[2]).sqrt() * 2.0;
        [(y[0] + x[1]) / s, 0.25 * s, (z[1] + y[2]) / s, (z[0] - x[2]) / s]
    } else {
        let s = (1.0 + z[2] - x[0] - y[1]).sqrt() * 2.0;
        [(z[0] + x[2]) / s, (z[1] + y[2]) / s, 0.25 * s, (x[1] - y[0]) / s]
    };
    Some(quat_normalize(q))
}

/// `Quaternion.Slerp`: along the arc, the shorter way. `t` is clamped to 0..1.
pub fn quat_slerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    let dot: f32 = (0..4).map(|i| a[i] * b[i]).sum();
    let (b, dot) = if dot < 0.0 { (b.map(|c| -c), -dot) } else { (b, dot) };
    // Close together the arc is as good as straight.
    if dot > 0.9995 {
        return quat_normalize(std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t));
    }
    let angle = dot.min(1.0).acos();
    let (from, to) = (((1.0 - t) * angle).sin(), (t * angle).sin());
    quat_normalize(std::array::from_fn(|i| a[i] * from + b[i] * to))
}

pub fn quat_normalize(q: [f32; 4]) -> [f32; 4] {
    let len = q.iter().map(|c| c * c).sum::<f32>().sqrt();
    if len > 1e-12 {
        q.map(|c| c / len)
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

/// `Quaternion.eulerAngles`, the inverse of [`quat_euler`]: degrees, with
/// the angle about x kept within a quarter turn either way.
pub fn quat_to_euler(q: [f32; 4]) -> [f32; 3] {
    let [x, y, z, w] = quat_normalize(q);
    // The rotation matrix's third column and second row, which hold the angles.
    let m02 = 2.0 * (x * z + w * y);
    let m12 = 2.0 * (y * z - w * x);
    let m22 = 1.0 - 2.0 * (x * x + y * y);
    let m10 = 2.0 * (x * y + w * z);
    let m11 = 1.0 - 2.0 * (x * x + z * z);
    let sin_x = (-m12).clamp(-1.0, 1.0);
    if sin_x.abs() > 0.9999 {
        // Looking straight up or down: the other two angles merge into one.
        let m00 = 1.0 - 2.0 * (y * y + z * z);
        let m20 = 2.0 * (x * z - w * y);
        return [sin_x.asin().to_degrees(), (-m20).atan2(m00).to_degrees(), 0.0];
    }
    [sin_x.asin().to_degrees(), m02.atan2(m22).to_degrees(), m10.atan2(m11).to_degrees()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euler_angles_round_trip() {
        for e in [[10.0, 20.0, 30.0], [-35.0, 170.0, -5.0], [0.0, 0.0, 0.0], [60.0, -90.0, 45.0]] {
            let back = quat_to_euler(quat_euler(e));
            for i in 0..3 {
                assert!(delta_angle(back[i], e[i]).abs() < 1e-3, "{e:?} -> {back:?}");
            }
        }
        // A half turn about x is reported as a half turn about y and z.
        let flipped = quat_to_euler(quat_euler([180.0, 0.0, 0.0]));
        assert!(flipped[0].abs() < 1e-3);
        assert!(delta_angle(flipped[1], 180.0).abs() < 1e-3 && delta_angle(flipped[2], 180.0).abs() < 1e-3);
    }

    #[test]
    fn quaternion_lerp_takes_the_short_way() {
        let a = quat_euler([0.0, 10.0, 0.0]);
        let b = quat_euler([0.0, 30.0, 0.0]).map(|c| -c);
        let mid = quat_to_euler(quat_lerp(a, b, 0.5));
        assert!((mid[1] - 20.0).abs() < 0.05, "{mid:?}");
    }

    #[test]
    fn angles_wrap() {
        assert_eq!(delta_angle(350.0, 10.0), 20.0);
        assert_eq!(delta_angle(10.0, 350.0), -20.0);
        assert_eq!(lerp_angle(350.0, 10.0, 0.5), 360.0);
    }

    #[test]
    fn smooth_damp_converges_without_overshoot() {
        let mut v = [0.0; 2];
        let mut p = [0.0, 0.0];
        let mut max = 0.0f32;
        for _ in 0..600 {
            p = smooth_damp(p, [1.0, 0.0], &mut v, 0.2, 100.0, 1.0 / 60.0);
            max = max.max(p[0]);
        }
        assert!((p[0] - 1.0).abs() < 1e-3);
        assert!(max <= 1.0);
    }

    #[test]
    fn smooth_damp_is_about_ninety_percent_done_after_two_smooth_times() {
        let mut v = [0.0; 2];
        let mut p = [0.0, 0.0];
        for _ in 0..24 {
            p = smooth_damp(p, [1.0, 0.0], &mut v, 0.2, 100.0, 1.0 / 60.0);
        }
        assert!(p[0] > 0.85 && p[0] < 0.95, "{}", p[0]);
    }
}
