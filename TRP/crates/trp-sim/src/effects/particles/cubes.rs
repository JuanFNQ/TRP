//! A particle system's meeting with the world at medium and low quality.
//! After the particles spec, "Meeting the world at medium and low quality": the planes its
//! rays have met, kept by the cube of the world the particle set out from
//! and the coarse way it went; the next particle to get a ray; the rays
//! left this frame; how many frames a low one waits before it takes part
//! again.

use std::collections::HashMap;

/// A plane met: its normal and a point on it, in the system's space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Plane {
    pub normal: [f32; 3],
    pub point: [f32; 3],
}

/// What a system keeps from frame to frame for meeting the world at
/// medium or low quality. A system at high quality keeps none of it.
#[derive(Debug, Clone, Default)]
pub struct Cubes {
    /// The planes met, by their key (`key`): the newest kept, none ever
    /// let go while the system lives.
    pub(super) planes: HashMap<u64, Plane>,
    /// The particle (its place in the list) the next ray goes to.
    pub(super) cursor: usize,
    /// The rays left this frame.
    pub(super) budget: i32,
    /// Frames before a low system takes part again.
    pub(super) countdown: u32,
    /// The quality it last took part at.
    pub(super) seen: Option<i64>,
    /// This frame's share of the rays (`assign_ray_budgets`), taken up by
    /// the frame's first step.
    pub(super) assigned: u32,
}

impl Cubes {
    /// How many planes it holds.
    pub fn planes(&self) -> usize {
        self.planes.len()
    }

    /// This frame's share of the rays, as given.
    pub fn assigned(&self) -> u32 {
        self.assigned
    }

    /// This frame's share given from outside (a tool setting a recording's
    /// systems' shares beside the game's), in place of
    /// `assign_ray_budgets`.
    pub fn assign(&mut self, rays: u32) {
        self.assigned = rays;
    }

    /// The particle the next ray goes to.
    pub fn cursor(&self) -> usize {
        self.cursor
    }
}

/// A figure raised to a multiple of four (the engine's rays go four at a
/// time).
pub(super) fn four_up(n: i32) -> i32 {
    (n.max(0) + 3) & !3
}

/// The class of a way: its largest part (x before y before z where two are
/// as large), and that part's sign: x 0 above nought and 1 otherwise, y 2
/// or 3, z 4 or 5. A way of nought is 1; one not a number in x is 0 (the
/// engine's comparisons, as made).
pub(super) fn way_class(way: [f32; 3]) -> u64 {
    let [x, y, z] = way.map(f32::abs);
    if x < y || x < z {
        if y < z {
            4 + u64::from(way[2] <= 0.0)
        } else {
            2 + u64::from(way[1] <= 0.0)
        }
    } else {
        u64::from(way[0] <= 0.0)
    }
}

/// A place's cube along one axis: the place over the cube's size, down to
/// the whole number below, as a 32-bit integer (one out of its range, or
/// not a number, the least: the engine's truncation).
fn cell(place: f32, size: f32) -> i32 {
    let f = (place / size).floor();
    if (-2_147_483_648.0..2_147_483_648.0).contains(&f) { f as i32 } else { i32::MIN }
}

/// The key of a point and a way: the cube the point is in (`voxel` across
/// in x and z, four times that in y, counted from the world's nought down
/// to the whole number below), hashed (CityHash64 of the three numbers as
/// 32-bit little-endian integers, x, y, z), with the way's class put in by
/// exclusive or.
pub(super) fn key(point: [f32; 3], way: [f32; 3], voxel: f32) -> u64 {
    let cell = [cell(point[0], voxel), cell(point[1], 4.0 * voxel), cell(point[2], voxel)];
    let mut bytes = [0u8; 12];
    for (k, c) in cell.iter().enumerate() {
        bytes[k * 4..k * 4 + 4].copy_from_slice(&c.to_le_bytes());
    }
    city_hash_8_to_16(&bytes) ^ way_class(way)
}

const K2: u64 = 0x9ae1_6a3b_2f90_404f;

/// CityHash64 (Geoff Pike and Jyrki Alakuijala's published algorithm,
/// version 1.1) of 8 to 16 bytes.
fn city_hash_8_to_16(s: &[u8]) -> u64 {
    let len = s.len();
    debug_assert!((8..=16).contains(&len));
    let fetch = |at: usize| u64::from_le_bytes(s[at..at + 8].try_into().unwrap_or([0; 8]));
    let mul = K2.wrapping_add(len as u64 * 2);
    let a = fetch(0).wrapping_add(K2);
    let b = fetch(len - 8);
    let c = b.rotate_right(37).wrapping_mul(mul).wrapping_add(a);
    let d = a.rotate_right(25).wrapping_add(b).wrapping_mul(mul);
    hash_len_16(c, d, mul)
}

fn hash_len_16(u: u64, v: u64, mul: u64) -> u64 {
    let mut a = (u ^ v).wrapping_mul(mul);
    a ^= a >> 47;
    let mut b = (v ^ a).wrapping_mul(mul);
    b ^= b >> 47;
    b.wrapping_mul(mul)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_way_classes_go_by_the_largest_part() {
        assert_eq!(way_class([1.0, 0.5, -0.5]), 0);
        assert_eq!(way_class([-1.0, 0.5, -0.5]), 1);
        assert_eq!(way_class([0.0, 0.0, 0.0]), 1);
        assert_eq!(way_class([0.2, -0.9, 0.1]), 3);
        assert_eq!(way_class([0.2, 0.9, 0.1]), 2);
        assert_eq!(way_class([0.2, 0.1, 0.9]), 4);
        assert_eq!(way_class([0.2, 0.1, -0.9]), 5);
        // A tie: x before y, y before z.
        assert_eq!(way_class([0.5, -0.5, 0.0]), 0);
        assert_eq!(way_class([0.0, 0.5, -0.5]), 2);
        assert_eq!(way_class([f32::NAN, 1.0, 0.0]), 0);
    }

    /// The published algorithm worked by hand for the cube (1, 2, 3): its
    /// two words, x and y and y and z, each little-endian.
    #[test]
    fn the_key_is_city_hash_of_the_cube() {
        let (a, b) = ((2u64 << 32 | 1).wrapping_add(K2), 3u64 << 32 | 2);
        let mul = K2.wrapping_add(24);
        assert_eq!(mul, 0x9ae1_6a3b_2f90_4067);
        let c = b.rotate_right(37).wrapping_mul(mul).wrapping_add(a);
        let d = a.rotate_right(25).wrapping_add(b).wrapping_mul(mul);
        let mut x = (c ^ d).wrapping_mul(mul);
        x ^= x >> 47;
        let mut y = (d ^ x).wrapping_mul(mul);
        y ^= y >> 47;
        assert_eq!(key([1.5, 8.5, 3.5], [0.0, 0.0, -1.0], 1.0), y.wrapping_mul(mul) ^ 5);
        // Out of an integer's range: the least.
        assert_eq!(cell(1e12, 0.2), i32::MIN);
        assert_eq!(cell(f32::NAN, 0.2), i32::MIN);
        assert_eq!(cell(-0.01, 0.2), -1);
    }

    #[test]
    fn a_cube_is_four_times_taller_than_wide() {
        let v = 0.2;
        let k = |p: [f32; 3]| key(p, [1.0, 0.0, 0.0], v);
        assert_eq!(k([0.05, 0.05, 0.05]), k([0.15, 0.75, 0.15]));
        assert_ne!(k([0.05, 0.05, 0.05]), k([0.25, 0.05, 0.05]));
        assert_ne!(k([0.05, 0.05, 0.05]), k([0.05, 0.85, 0.05]));
        // Below nought is the cube below, not the one at nought.
        assert_ne!(k([-0.05, 0.05, 0.05]), k([0.05, 0.05, 0.05]));
        // The way's class is part of it.
        assert_ne!(key([0.05; 3], [1.0, 0.0, 0.0], v), key([0.05; 3], [-1.0, 0.0, 0.0], v));
    }

    #[test]
    fn rays_go_four_at_a_time() {
        assert_eq!(four_up(0), 0);
        assert_eq!(four_up(1), 4);
        assert_eq!(four_up(4), 4);
        assert_eq!(four_up(85), 88);
        assert_eq!(four_up(-3), 0);
    }
}
