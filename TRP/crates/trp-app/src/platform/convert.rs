//! Unity space to Bevy space.
//!
//! Unity is left-handed with +Z forward; Bevy is right-handed with -Z
//! forward. Negating Z maps one onto the other and keeps right, up and
//! forward meaning the same thing in both.

use bevy::math::{Mat4, Quat, Vec3};

pub fn position(p: [f32; 3]) -> Vec3 {
    Vec3::new(p[0], p[1], -p[2])
}

/// Mirroring reverses the sense of rotation about X and Y. A quaternion of
/// no length (an asset's never-set field: the 0.2 BTC's icon pose is
/// 0, 0, 0, 0) is no turn, as Unity's own normalising makes it
/// (`Quaternion.Normalize`, Unity's published C#: identity under
/// `Mathf.Epsilon`; the engine's transform setter, `PROVISIONAL`: not read);
/// normalised here, it was not a number, and the BTC's picture nothing.
pub fn rotation(q: [f32; 4]) -> Quat {
    let turn = Quat::from_xyzw(-q[0], -q[1], q[2], q[3]);
    let length = turn.length();
    if length.is_finite() && length > f32::MIN_POSITIVE { turn / length } else { Quat::IDENTITY }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quaternion_of_no_length_is_no_turn() {
        // The 0.2 BTC's icon pose, as its prefab saves it.
        assert_eq!(rotation([0.0, 0.0, 0.0, 0.0]), Quat::IDENTITY);
        let turned = rotation([0.707106, 0.0, 0.0, 0.7071076]);
        assert!(turned.is_finite() && (turned.length() - 1.0).abs() < 1e-6);
    }
}
