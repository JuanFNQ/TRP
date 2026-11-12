//! Boxes: overlaps and pushes against spheres, capsules and upright boxes.

use super::*;

/// A solid box standing upright (x right, y up, z ahead), from `low` to
/// `high` in its own axes, turned by `turn` radians about the upright line
/// through its middle, the way the game's `Quaternion.AngleAxis` turns
/// about straight up (a quarter turn takes x to where -z was). Square to
/// the world's axes at a turn of nothing. To the character it is solid from
/// its top down to the ground: only a capsule wholly under it is clear of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Block {
    pub low: [f32; 3],
    pub high: [f32; 3],
    pub turn: f32,
}

impl Block {
    /// A box square to the world's axes.
    pub fn square(low: [f32; 3], high: [f32; 3]) -> Self {
        Self { low, high, turn: 0.0 }
    }

    /// A box of `size` (along its own x, up, along its own z) whose middle
    /// at the ground is `foot`, with its own x along `along` (level).
    pub fn turned(foot: [f32; 3], size: [f32; 3], along: [f32; 3]) -> Self {
        let half = [size[0] / 2.0, size[2] / 2.0];
        Self {
            low: [foot[0] - half[0], foot[1], foot[2] - half[1]],
            high: [foot[0] + half[0], foot[1] + size[1], foot[2] + half[1]],
            turn: (-along[2]).atan2(along[0]),
        }
    }

    /// Its middle across the ground.
    pub(super) fn middle(&self) -> [f32; 2] {
        [(self.low[0] + self.high[0]) / 2.0, (self.low[2] + self.high[2]) / 2.0]
    }

    /// A level way in the world turned into the box's own axes, or back
    /// with `back`.
    fn turn_way(&self, x: f32, z: f32, back: bool) -> [f32; 2] {
        let (sin, cos) = (if back { self.turn } else { -self.turn }).sin_cos();
        [x * cos + z * sin, z * cos - x * sin]
    }

    /// A place in the world in the box's own axes.
    pub(super) fn own(&self, at: [f32; 3]) -> [f32; 3] {
        if self.turn == 0.0 {
            return at;
        }
        let m = self.middle();
        let [x, z] = self.turn_way(at[0] - m[0], at[2] - m[1], false);
        [m[0] + x, at[1], m[1] + z]
    }

    /// Whether a sphere reaches into the box (`Physics.CheckSphere`).
    pub fn sphere_overlaps(&self, centre: [f32; 3], radius: f32) -> bool {
        let c = self.own(centre);
        let gap: f32 = (0..3).map(|k| (c[k].clamp(self.low[k], self.high[k]) - c[k]).powi(2)).sum();
        gap < radius * radius
    }

    /// The shortest way out of the box for a sphere, and how far, if it
    /// reaches in (`Physics.ComputePenetration` of a sphere and a box):
    /// from the box's nearest point where the middle is outside, through
    /// the nearest face where it is inside.
    pub fn sphere_push(&self, centre: [f32; 3], radius: f32) -> Option<([f32; 3], f32)> {
        let c = self.own(centre);
        let near: [f32; 3] = std::array::from_fn(|k| c[k].clamp(self.low[k], self.high[k]));
        let gap: [f32; 3] = std::array::from_fn(|k| c[k] - near[k]);
        let off = (gap[0] * gap[0] + gap[1] * gap[1] + gap[2] * gap[2]).sqrt();
        let (way, depth) = if off > 0.0 {
            if off >= radius {
                return None;
            }
            (gap.map(|g| g / off), radius - off)
        } else {
            // Inside: out through the nearest of the six faces.
            let mut best = ([0.0, 1.0, 0.0], f32::MAX);
            for k in 0..3 {
                for (to_face, sign) in [(c[k] - self.low[k], -1.0), (self.high[k] - c[k], 1.0)] {
                    if to_face < best.1 {
                        let mut way = [0.0; 3];
                        way[k] = sign;
                        best = (way, to_face);
                    }
                }
            }
            (best.0, best.1 + radius)
        };
        if self.turn == 0.0 {
            return Some((way, depth));
        }
        let [x, z] = self.turn_way(way[0], way[2], true);
        Some(([x, way[1], z], depth))
    }

    /// The shortest way out of the box for a capsule (the ends of its
    /// middle line and its radius), and how far, if it reaches in: the
    /// deepest of spheres along its line, a centimetre apart at most
    /// (`PROVISIONAL`: the engine works the capsule out whole).
    pub fn capsule_push(&self, ends: [[f32; 3]; 2], radius: f32) -> Option<([f32; 3], f32)> {
        /// The spheres' spacing along the line, at most, in metres.
        const SPACING: f32 = 0.01;
        let length = (0..3).map(|k| (ends[1][k] - ends[0][k]).powi(2)).sum::<f32>().sqrt();
        let count = (length / SPACING).ceil().max(1.0) as usize;
        (0..=count)
            .filter_map(|i| {
                let t = i as f32 / count as f32;
                self.sphere_push(std::array::from_fn(|k| ends[0][k] + (ends[1][k] - ends[0][k]) * t), radius)
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// Whether an upright box (`half` its half-width, half-height and
    /// half-depth, its middle at `centre`, turned `yaw` degrees about the
    /// upright as Unity turns: its own z along (sin, 0, cos)) reaches into
    /// this one: their heights meet, and no level axis of either parts them.
    pub fn upright_box_overlaps(&self, centre: [f32; 3], half: [f32; 3], yaw: f32) -> bool {
        if centre[1] + half[1] <= self.low[1] || centre[1] - half[1] >= self.high[1] {
            return false;
        }
        let m = self.middle();
        let mine = [(self.high[0] - self.low[0]) / 2.0, (self.high[2] - self.low[2]) / 2.0];
        let a = -self.turn;
        let my_axes = [[a.cos(), a.sin()], [-a.sin(), a.cos()]];
        let (s, c) = yaw.to_radians().sin_cos();
        let its_axes = [[c, -s], [s, c]];
        let apart = [centre[0] - m[0], centre[2] - m[1]];
        let dot = |u: [f32; 2], v: [f32; 2]| u[0] * v[0] + u[1] * v[1];
        for axis in my_axes.iter().chain(its_axes.iter()) {
            let reach_mine = mine[0] * dot(my_axes[0], *axis).abs() + mine[1] * dot(my_axes[1], *axis).abs();
            let reach_its = half[0] * dot(its_axes[0], *axis).abs() + half[2] * dot(its_axes[1], *axis).abs();
            if dot(apart, *axis).abs() >= reach_mine + reach_its {
                return false;
            }
        }
        true
    }

    /// Whether a place is over the box (or in it, or under it).
    pub fn covers(&self, x: f32, z: f32) -> bool {
        let [x, _, z] = self.own([x, 0.0, z]);
        x >= self.low[0] && x <= self.high[0] && z >= self.low[2] && z <= self.high[2]
    }

    /// How the capsule stands to the box, as `Tread::reach` has it for a
    /// piece of a line: the shortest way out and how far in. `at` is the
    /// feet. Worked in the box's own axes and turned back.
    pub(super) fn reach(&self, at: [f32; 3], capsule: Capsule, radius: f32) -> ([f32; 3], f32) {
        let (way, depth) = self.reach_square(self.own(at), capsule, radius);
        if self.turn == 0.0 {
            return (way, depth);
        }
        let [x, z] = self.turn_way(way[0], way[2], true);
        ([x, way[1], z], depth)
    }

    /// Whether a capsule with its feet at `at` is in the box.
    pub fn overlaps(&self, at: [f32; 3], capsule: Capsule, radius: f32) -> bool {
        self.reach(at, capsule, radius).1 > 0.0
    }

    /// How far along the line from `from` to `to` the box is entered, as
    /// a share of the line. A line that starts inside it does not enter it.
    pub fn entered(&self, from: [f32; 3], to: [f32; 3]) -> Option<f32> {
        let (from, to) = (self.own(from), self.own(to));
        let (mut enter, mut leave) = (0.0f32, 1.0f32);
        for k in 0..3 {
            let (start, travel) = (from[k], to[k] - from[k]);
            if travel.abs() < 1e-9 {
                if start <= self.low[k] || start >= self.high[k] {
                    return None;
                }
                continue;
            }
            let (a, b) = ((self.low[k] - start) / travel, (self.high[k] - start) / travel);
            enter = enter.max(a.min(b));
            leave = leave.min(a.max(b));
        }
        (enter <= leave && enter > 0.0).then_some(enter)
    }

    /// As `entered`, with the face entered's outward normal in the world.
    pub fn entered_face(&self, from: [f32; 3], to: [f32; 3]) -> Option<(f32, [f32; 3])> {
        let (from, to) = (self.own(from), self.own(to));
        let (mut enter, mut leave, mut face) = (0.0f32, 1.0f32, None);
        for k in 0..3 {
            let (start, travel) = (from[k], to[k] - from[k]);
            if travel.abs() < 1e-9 {
                if start <= self.low[k] || start >= self.high[k] {
                    return None;
                }
                continue;
            }
            let (a, b) = ((self.low[k] - start) / travel, (self.high[k] - start) / travel);
            if a.min(b) > enter {
                enter = a.min(b);
                face = Some((k, if travel > 0.0 { -1.0 } else { 1.0 }));
            }
            leave = leave.min(a.max(b));
        }
        let (k, sign) = face.filter(|_| enter <= leave && enter > 0.0)?;
        let mut normal = [0.0; 3];
        normal[k] = sign;
        if self.turn != 0.0 {
            let [x, z] = self.turn_way(normal[0], normal[2], true);
            normal = [x, normal[1], z];
        }
        Some((enter, normal))
    }

    fn reach_square(&self, at: [f32; 3], capsule: Capsule, radius: f32) -> ([f32; 3], f32) {
        if at[1] + capsule.top <= self.low[1] {
            return ([0.0, 1.0, 0.0], -1.0);
        }
        let underside = at[1] + capsule.underside;
        let (round, top) = (underside + radius, self.high[1]);
        // How far outside the box's outline the axis is along x and along
        // z (inside it by that much, if negative), and which way is out.
        let beyond = |k: usize| {
            if at[k] - self.low[k] < self.high[k] - at[k] { (self.low[k] - at[k], -1.0) } else { (at[k] - self.high[k], 1.0) }
        };
        let (across, ahead) = (beyond(0), beyond(2));
        if across.0 <= 0.0 && ahead.0 <= 0.0 {
            // Over the box: out by its top, or, from deep inside, by the
            // nearest of its sides if that is shorter.
            let up = top - underside;
            let (inside, way) = if across.0 > ahead.0 { (-across.0, [across.1, 0.0, 0.0]) } else { (-ahead.0, [0.0, 0.0, ahead.1]) };
            if up > 0.0 && inside + radius < up {
                return (way, inside + radius);
            }
            return ([0.0, 1.0, 0.0], up);
        }
        // Beside it: against a side or an upright edge, or over the rim.
        let aside = [across.0.max(0.0) * across.1, ahead.0.max(0.0) * ahead.1];
        let off = (aside[0] * aside[0] + aside[1] * aside[1]).sqrt();
        if round <= top {
            return ([aside[0] / off, 0.0, aside[1] / off], radius - off);
        }
        let apart = (off * off + (round - top) * (round - top)).sqrt();
        ([aside[0] / apart, (round - top) / apart, aside[1] / apart], radius - apart)
    }
}
