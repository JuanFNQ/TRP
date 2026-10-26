//! The ground's slope under the character (movement spec, "Slopes and
//! sliding"): which way the ground faces, averaged over the last frames;
//! the pull down ground too steep to stand on; and motion down a slope laid
//! along it (`MovementContext.DetectSurfaceNormal`, `UpdateGroundCollision`,
//! `ProjectMotionToSurface`, `ApplyGravity`).

/// Ground whose upward part, averaged, is under this is too steep to
/// stand on: the character slides (`MovementContext._slopeTreshold`, about
/// 49 degrees). A ray's hit flatter than it is taken as the surface as it
/// is; a steeper one only if the capsule's own contact is steeper still.
pub const SLOPE_THRESHOLD: f32 = 0.65;

/// The slide's pull goes by how far the ground's upward part is under this
/// (`UpdateGroundCollision`: `0.9 - SurfaceNormal.y`).
const SLIDE_LEVEL: f32 = 0.9;

/// Motion is laid along the ground only where the averaged upward part is
/// inside this range, open at both ends (`ProjectMotionToSurface`): from
/// about 6 to about 60 degrees.
const LAID_RANGE: [f32; 2] = [0.5, 0.995];

/// The share of a motion's length that has to lie along the slope's way
/// down for it to be laid there, at least (and at most all of it).
const LAID_SHARE: f32 = 0.5;

/// How many frames the average runs over (`NormalDotHistory`, a
/// `RollingMedian` of five).
const HISTORY: usize = 5;

/// How much further than the ground check the ray under the axis looks for
/// the ground whose normal it reports (`UpdateGroundCollision`'s
/// `additionSearch`).
pub const GROUND_SEARCH: f32 = 0.25;

/// The ground's upward part over the last frames (`RollingMedian`'s
/// `Avarage`): the sum of what was added over how many were, up to five.
/// The game writes the first value at the buffer's second place, which
/// changes nothing in a sum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalHistory {
    values: [f32; HISTORY],
    count: usize,
    at: usize,
}

impl Default for NormalHistory {
    fn default() -> Self {
        Self { values: [0.0; HISTORY], count: 0, at: 0 }
    }
}

impl NormalHistory {
    pub fn add(&mut self, value: f32) {
        self.count = (self.count + 1).min(HISTORY);
        self.at = (self.at + 1) % HISTORY;
        self.values[self.at] = value;
    }

    /// Nothing until a value is added.
    pub fn average(&self) -> f32 {
        if self.count == 0 {
            return 0.0;
        }
        self.values.iter().sum::<f32>() / self.count as f32
    }
}

/// Which way the ground under the character faces this frame
/// (`DetectSurfaceNormal`): the ray's hit where it found ground and that
/// ground is not too steep; where it is, the flatter of the hit and the
/// capsule's contact (`capsule`, the way out of the ground the capsule was
/// last pushed along that points most upward); where the ray found
/// nothing, the capsule's contact.
pub fn surface_normal(hit: Option<[f32; 3]>, capsule: [f32; 3]) -> [f32; 3] {
    match hit {
        None => capsule,
        Some(hit) if hit[1] < SLOPE_THRESHOLD => {
            if hit[1] > capsule[1] {
                hit
            } else {
                capsule
            }
        }
        Some(hit) => hit,
    }
}

/// The slope's pull (`MovementContext.Slope`) for a surface facing
/// `normal` with `average` its upward part over the last frames: nothing on
/// ground that is not too steep; on steeper, the way up the slope, square
/// to the ground, as long as `0.9` less the upward part. The motion has it
/// added times the gravity's downward part and the frame's time, so the
/// character is carried down the slope at that many times `g` metres a
/// second, steeper ground faster (3.9 m/s at 60 degrees).
pub fn slide(normal: [f32; 3], average: f32) -> [f32; 3] {
    if average >= SLOPE_THRESHOLD {
        return [0.0; 3];
    }
    let across = cross(normal, [0.0, 1.0, 0.0]);
    let up_slope = normalized(cross(across, normal));
    up_slope.map(|k| k * (SLIDE_LEVEL - normal[1]))
}

/// A frame's motion laid along a slope it goes down
/// (`ProjectMotionToSurface`): where the averaged upward part is within
/// `LAID_RANGE`, the motion's heading along the surface is found; if that
/// goes down and at least half the motion lies along it, the motion is put
/// on it, lengthened so that its level part is kept. Up a slope, and on
/// ground nearly level or too steep, the motion is left as it is.
pub fn along_surface(motion: [f32; 3], normal: [f32; 3], average: f32) -> [f32; 3] {
    if !(average < LAID_RANGE[1] && average > LAID_RANGE[0]) {
        return motion;
    }
    let along = normalized(cross([motion[2], 0.0, -motion[0]], normal));
    if along[1] >= 0.0 {
        return motion;
    }
    let length = (motion[0] * motion[0] + motion[1] * motion[1] + motion[2] * motion[2]).sqrt();
    let share = dot3(along, normalized(motion));
    if (LAID_SHARE..=1.0).contains(&share) {
        along.map(|k| k * (length / share))
    } else {
        motion
    }
}

/// Unity's `Vector3.Cross`.
pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Unity's `Vector3.normalized`: nothing for a vector shorter than 1e-5.
pub fn normalized(v: [f32; 3]) -> [f32; 3] {
    let length = dot3(v, v).sqrt();
    if length > 1e-5 {
        v.map(|k| k / length)
    } else {
        [0.0; 3]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plane rising toward -x at `degrees`.
    fn plane(degrees: f32) -> [f32; 3] {
        let (sin, cos) = degrees.to_radians().sin_cos();
        [sin, cos, 0.0]
    }

    #[test]
    fn the_average_counts_only_what_was_added() {
        let mut history = NormalHistory::default();
        assert_eq!(history.average(), 0.0);
        history.add(1.0);
        history.add(0.5);
        assert!((history.average() - 0.75).abs() < 1e-6);
        for _ in 0..5 {
            history.add(0.4);
        }
        assert!((history.average() - 0.4).abs() < 1e-6);
    }

    #[test]
    fn steep_ground_pulls_down_the_slope_by_its_steepness() {
        // 60 degrees: the pull is 0.9 - 0.5 = 0.4 long, up the slope (toward -x, rising).
        let normal = plane(60.0);
        let pull = slide(normal, normal[1]);
        let length = (pull[0] * pull[0] + pull[1] * pull[1] + pull[2] * pull[2]).sqrt();
        assert!((length - 0.4).abs() < 1e-5);
        assert!(pull[0] < 0.0 && pull[1] > 0.0);
        // Square to the ground.
        assert!(dot3(pull, normal).abs() < 1e-5);
        // 40 degrees is stood on.
        assert_eq!(slide(plane(40.0), plane(40.0)[1]), [0.0; 3]);
        // The average decides, the frame's own normal sizes it.
        assert_eq!(slide(normal, 0.7), [0.0; 3]);
    }

    #[test]
    fn a_steep_hit_gives_way_to_a_flatter_contact() {
        let up = [0.0, 1.0, 0.0];
        assert_eq!(surface_normal(None, plane(70.0)), plane(70.0));
        assert_eq!(surface_normal(Some(plane(70.0)), plane(30.0)), plane(30.0));
        assert_eq!(surface_normal(Some(plane(30.0)), up), plane(30.0));
        assert_eq!(surface_normal(Some(plane(70.0)), plane(80.0)), plane(70.0));
    }

    #[test]
    fn downhill_motion_is_laid_on_the_slope_and_uphill_is_not() {
        // The plane falls toward +x at 20 degrees.
        let normal = plane(20.0);
        let down = along_surface([0.1, 0.0, 0.0], normal, normal[1]);
        assert!((down[0] - 0.1).abs() < 1e-6, "the level part is kept: {down:?}");
        assert!((down[1] + 0.1 * 20f32.to_radians().tan()).abs() < 1e-6);
        assert_eq!(along_surface([-0.1, 0.0, 0.0], normal, normal[1]), [-0.1, 0.0, 0.0]);
        // Nearly level or too steep: left.
        assert_eq!(along_surface([0.1, 0.0, 0.0], plane(3.0), plane(3.0)[1]), [0.1, 0.0, 0.0]);
        assert_eq!(along_surface([0.1, 0.0, 0.0], plane(65.0), plane(65.0)[1]), [0.1, 0.0, 0.0]);
        // Across the slope at more than 60 degrees from its fall: under half along it.
        let across = along_surface([0.0, 0.0, 0.1], normal, normal[1]);
        assert_eq!(across, [0.0, 0.0, 0.1]);
    }
}
