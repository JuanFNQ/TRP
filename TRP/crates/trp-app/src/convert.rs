//! Unity space to Bevy space.
//!
//! Unity is left-handed with +Z forward; Bevy is right-handed with -Z
//! forward. Negating Z maps one onto the other and keeps right, up and
//! forward meaning the same thing in both.

use bevy::math::{Mat4, Quat, Vec3};

pub fn position(p: [f32; 3]) -> Vec3 {
    Vec3::new(p[0], p[1], -p[2])
}

/// Mirroring reverses the sense of rotation about X and Y.
pub fn rotation(q: [f32; 4]) -> Quat {
    Quat::from_xyzw(-q[0], -q[1], q[2], q[3]).normalize()
}

/// The same mapping the other way.
pub fn rotation_back(q: Quat) -> [f32; 4] {
    [-q.x, -q.y, q.z, q.w]
}

pub fn transform(translation: [f32; 3], rot: [f32; 4], scale: [f32; 3]) -> bevy::prelude::Transform {
    bevy::prelude::Transform {
        translation: position(translation),
        rotation: rotation(rot),
        scale: Vec3::from(scale),
    }
}

/// `rows` holds `e00..e33` in reading order. The result is `S * M * S` with
/// `S = diag(1, 1, -1, 1)`.
pub fn matrix(rows: [f32; 16]) -> Mat4 {
    let mut cols = [0.0f32; 16];
    for r in 0..4 {
        for c in 0..4 {
            let v = rows[r * 4 + c];
            cols[c * 4 + r] = if (r == 2) != (c == 2) { -v } else { v };
        }
    }
    Mat4::from_cols_array(&cols)
}
