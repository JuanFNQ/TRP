//! Treads and the course of ground a capsule settles on.

use super::*;

/// A piece of ground between two faces: from `near` to `far` along the
/// line, with its top running through `tops` (how far along, how high;
/// nearest first). It is solid all the way down.
#[derive(Debug, Clone)]
pub struct Tread {
    pub near: f32,
    pub far: f32,
    pub tops: Vec<[f32; 2]>,
}

impl Tread {
    /// A level piece.
    pub fn level(near: f32, far: f32, height: f32) -> Self {
        Self { near, far, tops: vec![[near, height], [far, height]] }
    }

    /// The top's height at a place along the line, held at the ends.
    pub fn height(&self, along: f32) -> f32 {
        let after = self.tops.partition_point(|p| p[0] < along);
        match (after.checked_sub(1).map(|i| self.tops[i]), self.tops.get(after)) {
            (Some(before), Some(next)) => {
                let share = (along - before[0]) / (next[0] - before[0]).max(1e-6);
                before[1] + (next[1] - before[1]) * share.clamp(0.0, 1.0)
            }
            (Some(only), None) | (None, Some(&only)) => only[1],
            (None, None) => 0.0,
        }
    }

    /// How the capsule stands to this piece: which way is the shortest
    /// way out of it (along the line, and up) and how far the capsule is
    /// in it, or clear of it if that is negative. `at` is the feet: how
    /// far along, how high.
    pub(super) fn reach(&self, at: [f32; 2], capsule: Capsule, radius: f32) -> ([f32; 2], f32) {
        let [along, feet] = at;
        // The lowest the capsule's rounded foot can be, and the middle of
        // that round: a capsule shorter than it is wide is a ball.
        let underside = feet + capsule.underside;
        let round = underside + radius;
        if along >= self.near && along <= self.far {
            // Over the piece: out by its top, or, from deep inside, by the
            // nearer of its faces if that is shorter.
            let up = self.height(along) - underside;
            let sideways = (along - self.near).min(self.far - along) + radius;
            if up > 0.0 && sideways < up {
                let out = if along - self.near < self.far - along { -1.0 } else { 1.0 };
                return ([out, 0.0], sideways);
            }
            return ([0.0, 1.0], up);
        }
        // Beside it: against its face, or over its corner.
        let (edge, out) = if along < self.near { (self.near, -1.0) } else { (self.far, 1.0) };
        let (aside, top) = ((along - edge).abs(), self.height(edge));
        if round <= top {
            return ([out, 0.0], radius - aside);
        }
        let apart = (aside * aside + (round - top) * (round - top)).sqrt();
        ([out * aside / apart, (round - top) / apart], radius - apart)
    }
}

/// A line of ground: pieces one after another, nearest first, each
/// beginning where the one before ends.
#[derive(Debug, Clone, Default)]
pub struct Course {
    pub treads: Vec<Tread>,
}

impl Course {
    /// The course a path shows: places along a line with the height of
    /// what lay under each, nearest first. Where two neighbours differ by
    /// a step there is a face midway between them, and the ground runs on
    /// level beyond the first and the last place.
    pub fn from_path(points: &[[f32; 2]]) -> Self {
        let mut treads: Vec<Tread> = Vec::new();
        for point in points {
            match treads.last_mut() {
                Some(tread) => {
                    let last = *tread.tops.last().unwrap_or(point);
                    if (point[1] - last[1]).abs() > FACE_RISE && point[0] - last[0] < FACE_RUN {
                        let face = (point[0] + last[0]) / 2.0;
                        tread.far = face;
                        treads.push(Tread { near: face, far: f32::MAX, tops: vec![*point] });
                    } else {
                        tread.tops.push(*point);
                    }
                }
                None => treads.push(Tread { near: f32::MIN, far: f32::MAX, tops: vec![*point] }),
            }
        }
        Self { treads }
    }

    /// The ground's height under a place along the line.
    pub fn height(&self, along: f32) -> f32 {
        let index = self.treads.partition_point(|tread| tread.far < along);
        self.treads.get(index).or(self.treads.last()).map_or(0.0, |tread| tread.height(along))
    }

    /// One move of the game's controller: from the feet's place `from`
    /// toward `to` (how far along, how high), for a capsule of that
    /// radius; where it leaves the feet.
    pub fn settle(&self, from: [f32; 2], to: [f32; 2], capsule: Capsule, radius: f32) -> [f32; 2] {
        let mut at = to;
        for tread in &self.treads {
            let (mut way, mut depth) = tread.reach(at, capsule, radius);
            if depth <= 0.0 {
                continue;
            }
            if way[1] >= SNAP_DOT {
                depth /= way[1];
                way = [0.0, 1.0];
            }
            at[0] += way[0] * depth;
            if capsule.lift {
                at[1] += way[1] * depth;
            }
        }
        let moved = [at[0] - from[0], at[1] - from[1]];
        let far = (moved[0] * moved[0] + moved[1] * moved[1]).sqrt();
        if far >= DEEPEST {
            at = [from[0] + moved[0] * DEEP_STEP / far, from[1] + moved[1] * DEEP_STEP / far];
        }
        // Last, the controller looks straight down from the middle of the
        // capsule's foot *where the move began*: ground found nearer than
        // the foot's own reach less a millimetre means the capsule had sunk
        // into it there, and the feet are put on that ground and the move
        // dropped (`ResolveUnityDepenetrationBug`). A capsule that is
        // pushed up out of the ground every move never has; one that is not
        // (a climb begun on rising ground) has every few frames.
        let under = self.height(from[0]);
        if under > from[1] + capsule.underside + SUNK {
            at = [from[0], under];
        }
        at
    }

    /// Whether the capsule rests on something other than the ground under
    /// its axis: the corner of a piece its rounded foot overhangs.
    pub fn held(&self, at: [f32; 2], capsule: Capsule, radius: f32) -> bool {
        self.treads.iter().filter(|tread| at[0] < tread.near || at[0] > tread.far).any(|tread| {
            let (way, depth) = tread.reach(at, capsule, radius);
            depth >= -TOUCH && way[1] >= GROUND_DOT
        })
    }
}
