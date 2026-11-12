//! Upright capsules entering blocks, the sites settled on and the reach.

use super::*;

/// Another character's capsule, upright (its controller's, on the
/// `Player` layer, which the character's own collides with as with the
/// world: the install's collision matrix; a body that has died is on
/// `Deadbody`, which it does not meet): its feet, its height and how far
/// over the feet its middle is (`Movement::capsule_figures`), its radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Upright {
    pub feet: [f32; 3],
    pub height: f32,
    pub centre: f32,
    pub radius: f32,
}

/// How near a character's own capsule in the site's list stands to where
/// its move begins, at most (a frame's move or a few of a character's
/// steps): any other keeps two radii off.
const OWN: f32 = 0.25;

/// The line a capsule's round ends' middles lie on, from `low` to `high`
/// over its feet: shorter than its two ends, a sphere at its middle (the
/// engine holds a capsule at least as high as it is wide).
pub(super) fn ends(low: f32, high: f32, radius: f32) -> (f32, f32) {
    let (from, to) = (low + radius, high - radius);
    if from > to {
        let middle = (low + high) / 2.0;
        (middle, middle)
    } else {
        (from, to)
    }
}

impl Upright {
    /// The line ends' middles' heights.
    fn line_ends(&self) -> (f32, f32) {
        ends(self.feet[1] + self.centre - self.height / 2.0, self.feet[1] + self.centre + self.height / 2.0, self.radius)
    }

    /// Where a line from `from` to `to` first enters the capsule, as a
    /// share of the line; none for a line that starts inside it (the
    /// engine's casts do not meet what they start in).
    pub fn entered(&self, from: [f32; 3], to: [f32; 3]) -> Option<f32> {
        let (low, high) = self.line_ends();
        let r2 = self.radius * self.radius;
        let nearest = |p: [f32; 3]| [self.feet[0], p[1].clamp(low, high), self.feet[2]];
        let apart2 = |p: [f32; 3], q: [f32; 3]| (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f32>();
        if apart2(from, nearest(from)) < r2 {
            return None;
        }
        let d: [f32; 3] = std::array::from_fn(|k| to[k] - from[k]);
        let mut best: Option<f32> = None;
        let mut keep = |t: f32| {
            if (0.0..=1.0).contains(&t) && best.is_none_or(|b| t < b) {
                best = Some(t);
            }
        };
        // The upright side.
        let (ox, oz) = (from[0] - self.feet[0], from[2] - self.feet[2]);
        let a = d[0] * d[0] + d[2] * d[2];
        if a > 1e-12 {
            let b = 2.0 * (ox * d[0] + oz * d[2]);
            let c = ox * ox + oz * oz - r2;
            let disc = b * b - 4.0 * a * c;
            if disc >= 0.0 {
                let t = (-b - disc.sqrt()) / (2.0 * a);
                let y = from[1] + d[1] * t;
                if y >= low && y <= high {
                    keep(t);
                }
            }
        }
        // The round ends.
        for y in [low, high] {
            let o = [from[0] - self.feet[0], from[1] - y, from[2] - self.feet[2]];
            let a = d.iter().map(|x| x * x).sum::<f32>();
            if a <= 1e-12 {
                continue;
            }
            let b = 2.0 * (o[0] * d[0] + o[1] * d[1] + o[2] * d[2]);
            let c = o.iter().map(|x| x * x).sum::<f32>() - r2;
            let disc = b * b - 4.0 * a * c;
            if disc >= 0.0 {
                keep((-b - disc.sqrt()) / (2.0 * a));
            }
        }
        best
    }

    /// The way out of it, and how far in, of the character's capsule with
    /// its feet at `at` (as `Block::reach`): the shortest way between the
    /// two capsules' lines, which is level while they stand side by side
    /// and tips upward over its top.
    pub(super) fn reach(&self, at: [f32; 3], capsule: Capsule, radius: f32) -> ([f32; 3], f32) {
        let (low, high) = ends(at[1] + capsule.underside, at[1] + capsule.top, radius);
        let (other_low, other_high) = ends(self.feet[1] + self.centre - self.height / 2.0, self.feet[1] + self.centre + self.height / 2.0, self.radius);
        let rise = if low > other_high {
            low - other_high
        } else if other_low > high {
            high - other_low
        } else {
            0.0
        };
        let across = [at[0] - self.feet[0], at[2] - self.feet[2]];
        let apart = (across[0] * across[0] + across[1] * across[1] + rise * rise).sqrt();
        if apart < 1e-6 {
            return ([1.0, 0.0, 0.0], radius + self.radius);
        }
        ([across[0] / apart, rise / apart, across[1] / apart], radius + self.radius - apart)
    }
}

/// Level ground with boxes standing on it: the same rules as `Course`,
/// for a place that has width as well as length (the app's test range),
/// and the other characters standing on it.
#[derive(Debug, Clone, Default)]
pub struct Site {
    /// The ground's height.
    pub ground: f32,
    pub blocks: Vec<Block>,
    pub others: Vec<Upright>,
}

impl Site {
    /// The height of whatever is highest under a place: a box's top, or
    /// the ground.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        self.blocks.iter().filter(|block| block.covers(x, z)).map(|block| block.high[1]).fold(self.ground, f32::max)
    }

    /// One move of the game's controller, as `Course::settle`: from the
    /// feet's place `from` toward `to`, for a capsule of that radius;
    /// where it leaves the feet. The site's characters may hold the
    /// mover's own: the one whose axis stands within `OWN` of where the
    /// move begins is taken for it and not met (no other stands so near:
    /// two capsules keep their two radii apart).
    pub fn settle(&self, from: [f32; 3], to: [f32; 3], capsule: Capsule, radius: f32) -> [f32; 3] {
        let own = |other: &&Upright| ((other.feet[0] - from[0]).powi(2) + (other.feet[2] - from[2]).powi(2)).sqrt() < OWN && (other.feet[1] - from[1]).abs() < OWN;
        let mut at = to;
        let sunk = self.ground - (at[1] + capsule.underside);
        if sunk > 0.0 && capsule.lift {
            at[1] += sunk;
        }
        // Each thing in turn, from where the last left the capsule: the
        // boxes, then the other characters.
        for reach in self.blocks.iter().map(Reach::Block).chain(self.others.iter().filter(|other| !own(other)).map(Reach::Other)) {
            let (mut way, mut depth) = reach.of(at, capsule, radius);
            if depth <= 0.0 {
                continue;
            }
            if way[1] >= SNAP_DOT {
                depth /= way[1];
                way = [0.0, 1.0, 0.0];
            }
            at[0] += way[0] * depth;
            at[2] += way[2] * depth;
            if capsule.lift {
                at[1] += way[1] * depth;
            }
        }
        let moved: [f32; 3] = std::array::from_fn(|k| at[k] - from[k]);
        let far = moved.iter().map(|m| m * m).sum::<f32>().sqrt();
        if far >= DEEPEST {
            at = std::array::from_fn(|k| from[k] + moved[k] * DEEP_STEP / far);
        }
        // The look straight down from the middle of the capsule's foot
        // where the move began (`Course::settle` has the whole of it). The
        // look starts at that middle, so it finds only what is no higher.
        let round = from[1] + capsule.underside + radius;
        let tops = self.blocks.iter().filter(|block| block.covers(from[0], from[2])).map(|block| block.high[1]);
        let under = tops.filter(|top| *top <= round).fold(self.ground, f32::max);
        if under > from[1] + capsule.underside + SUNK {
            at = [from[0], under, from[2]];
        }
        at
    }

    /// Whether the capsule rests on something other than what is under
    /// its axis: the rim of a box its rounded foot overhangs, or another
    /// character's top.
    pub fn held(&self, at: [f32; 3], capsule: Capsule, radius: f32) -> bool {
        let on = |(way, depth): ([f32; 3], f32)| depth >= -TOUCH && way[1] >= GROUND_DOT;
        let own = |other: &&Upright| ((other.feet[0] - at[0]).powi(2) + (other.feet[2] - at[2]).powi(2)).sqrt() < OWN && (other.feet[1] - at[1]).abs() < OWN;
        self.blocks.iter().filter(|block| !block.covers(at[0], at[2])).any(|block| on(block.reach(at, capsule, radius))) || self.others.iter().filter(|other| !own(other)).any(|other| on(other.reach(at, capsule, radius)))
    }
}

/// A thing of the site the capsule may be in.
enum Reach<'a> {
    Block(&'a Block),
    Other(&'a Upright),
}

impl Reach<'_> {
    fn of(&self, at: [f32; 3], capsule: Capsule, radius: f32) -> ([f32; 3], f32) {
        match self {
            Reach::Block(block) => block.reach(at, capsule, radius),
            Reach::Other(other) => other.reach(at, capsule, radius),
        }
    }
}
