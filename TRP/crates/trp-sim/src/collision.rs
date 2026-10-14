//! The character's own collision, as the game's controller for the player
//! works it (`SimpleCharacterController`; movement spec, "The character's
//! own collision"), for a world that is one line of ground with steps in
//! it: what a replay rebuilds from a recorded path, and what a test sets
//! a wall or a box up in.
//!
//! The game moves the capsule by the frame's motion and then, for each
//! solid thing the capsule is in, moves it out by the shortest way. A way
//! out that points upward at all counts as ground under the character; one
//! that points mostly upward is taken straight up, so that the character
//! rides up onto low things and does not slide down gentle slopes. In a
//! vault or a climb the upward part of every way out is dropped. A move
//! that comes to more than a fifth of a metre in all is cut to a twentieth,
//! which is how a capsule that finds itself well inside something comes
//! out of it over several frames.
//!
//! A host with a collision engine of its own answers `World::obstructed`
//! from that; nothing here is asked of it.

use crate::movement::Capsule;

/// A way out counts as ground from this much of it pointing upward
/// (the cosine of 85 degrees: the controller's `_groundDotThreshold`).
const GROUND_DOT: f32 = 0.087_155_8;
/// From this much upward it is taken straight up
/// (`_snapGroundDenepentrationToUp`).
const SNAP_DOT: f32 = 0.64;
/// A step's move of this much or more in all is cut to `DEEP_STEP`
/// (`_maxDepenetrationDistance`, `_maxDepenetrationDistanceResolve`).
const DEEPEST: f32 = 0.2;
const DEEP_STEP: f32 = 0.05;
/// How near a surface the capsule counts as touching it, in metres.
const TOUCH: f32 = 0.0005;
/// How far into the ground under it the capsule's foot has to have been
/// for the move to be dropped (the controller's `_depenetrationBugRayAddition`).
const SUNK: f32 = 0.001;
/// Two neighbouring places of a path whose heights differ by more than
/// `FACE_RISE`, no further apart than `FACE_RUN`, have a face between them.
const FACE_RISE: f32 = 0.15;
const FACE_RUN: f32 = 0.12;

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
    fn reach(&self, at: [f32; 2], capsule: Capsule, radius: f32) -> ([f32; 2], f32) {
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
    fn middle(&self) -> [f32; 2] {
        [(self.low[0] + self.high[0]) / 2.0, (self.low[2] + self.high[2]) / 2.0]
    }

    /// A level way in the world turned into the box's own axes, or back
    /// with `back`.
    fn turn_way(&self, x: f32, z: f32, back: bool) -> [f32; 2] {
        let (sin, cos) = (if back { self.turn } else { -self.turn }).sin_cos();
        [x * cos + z * sin, z * cos - x * sin]
    }

    /// A place in the world in the box's own axes.
    fn own(&self, at: [f32; 3]) -> [f32; 3] {
        if self.turn == 0.0 {
            return at;
        }
        let m = self.middle();
        let [x, z] = self.turn_way(at[0] - m[0], at[2] - m[1], false);
        [m[0] + x, at[1], m[1] + z]
    }

    /// Whether a place is over the box (or in it, or under it).
    pub fn covers(&self, x: f32, z: f32) -> bool {
        let [x, _, z] = self.own([x, 0.0, z]);
        x >= self.low[0] && x <= self.high[0] && z >= self.low[2] && z <= self.high[2]
    }

    /// How the capsule stands to the box, as `Tread::reach` has it for a
    /// piece of a line: the shortest way out and how far in. `at` is the
    /// feet. Worked in the box's own axes and turned back.
    fn reach(&self, at: [f32; 3], capsule: Capsule, radius: f32) -> ([f32; 3], f32) {
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

/// Level ground with boxes standing on it: the same rules as `Course`,
/// for a place that has width as well as length (the app's test range).
#[derive(Debug, Clone, Default)]
pub struct Site {
    /// The ground's height.
    pub ground: f32,
    pub blocks: Vec<Block>,
}

impl Site {
    /// The height of whatever is highest under a place: a box's top, or
    /// the ground.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        self.blocks.iter().filter(|block| block.covers(x, z)).map(|block| block.high[1]).fold(self.ground, f32::max)
    }

    /// One move of the game's controller, as `Course::settle`: from the
    /// feet's place `from` toward `to`, for a capsule of that radius;
    /// where it leaves the feet.
    pub fn settle(&self, from: [f32; 3], to: [f32; 3], capsule: Capsule, radius: f32) -> [f32; 3] {
        let mut at = to;
        let sunk = self.ground - (at[1] + capsule.underside);
        if sunk > 0.0 && capsule.lift {
            at[1] += sunk;
        }
        for block in &self.blocks {
            let (mut way, mut depth) = block.reach(at, capsule, radius);
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
    /// its axis: the rim of a box its rounded foot overhangs.
    pub fn held(&self, at: [f32; 3], capsule: Capsule, radius: f32) -> bool {
        self.blocks.iter().filter(|block| !block.covers(at[0], at[2])).any(|block| {
            let (way, depth) = block.reach(at, capsule, radius);
            depth >= -TOUCH && way[1] >= GROUND_DOT
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RADIUS: f32 = 0.366;
    const STANDING: Capsule = Capsule { underside: 0.0, top: 1.6, lift: true };

    /// The yard of the tests below with width to it: ground at 0 and a
    /// box a metre high from 2 to 3 m ahead and from 1 m left to 1 m right.
    fn site() -> Site {
        Site { ground: 0.0, blocks: vec![Block::square([-1.0, 0.0, 2.0], [1.0, 1.0, 3.0])] }
    }

    /// Walked at square, a box stops the capsule its radius from the face
    /// and leaves it on the ground, as a line's wall does; walked at
    /// aslant, it lets the capsule along its face; and its upright edge
    /// is rounded by the capsule's radius.
    #[test]
    fn a_box_stops_the_capsule_and_lets_it_along_its_face() {
        let site = site();
        let mut at = [0.0, 0.0, 1.0];
        for _ in 0..60 {
            at = site.settle(at, [at[0], at[1], at[2] + 0.025], STANDING, RADIUS);
        }
        assert!((at[2] - (2.0 - RADIUS)).abs() < 1e-4 && at[0] == 0.0 && at[1] == 0.0, "{at:?}");
        assert!(!site.held(at, STANDING, RADIUS));
        for _ in 0..30 {
            at = site.settle(at, [at[0] + 0.02, at[1], at[2] + 0.02], STANDING, RADIUS);
        }
        assert!((at[0] - 0.6).abs() < 1e-4 && (at[2] - (2.0 - RADIUS)).abs() < 1e-4, "{at:?}");
        // On past the box's edge it comes round the corner, never nearer
        // the edge than its radius.
        let mut nearest = f32::MAX;
        for _ in 0..90 {
            at = site.settle(at, [at[0] + 0.02, at[1], at[2] + 0.02], STANDING, RADIUS);
            nearest = nearest.min(((at[0] - 1.0).powi(2) + (at[2] - 2.0).powi(2)).sqrt());
        }
        assert!(at[2] > 2.5 && at[0] > 1.0 + RADIUS - 1e-4 && nearest > RADIUS - 1e-4, "{at:?}, {nearest}");
    }

    /// The line's rules hold for a box: a low one is ridden up, a rim
    /// holds a foot that overhangs it, and a capsule deep in a box comes
    /// out a twentieth of a metre a move, by its nearest side.
    #[test]
    fn a_box_is_ridden_up_held_onto_and_come_out_of() {
        let low = Site { ground: 0.0, blocks: vec![Block::square([-1.0, 0.0, 2.0], [1.0, 0.1, 9.0])] };
        let mut at = [0.0, 0.0, 1.0];
        for _ in 0..80 {
            at = low.settle(at, [at[0], at[1], at[2] + 0.025], STANDING, RADIUS);
        }
        assert!(at[2] > 2.5 && (at[1] - 0.1).abs() < 2e-3, "{at:?}");

        let site = site();
        let rest = |out: f32| 1.0 - (RADIUS - (RADIUS * RADIUS - out * out).sqrt());
        assert!(site.held([0.0, rest(0.1), 3.1], STANDING, RADIUS));
        assert!(site.held([1.1, rest(0.1), 2.5], STANDING, RADIUS));
        assert!(!site.held([0.0, rest(0.1) + 0.01, 3.1], STANDING, RADIUS));
        assert!(!site.held([0.0, 0.9, 3.5], STANDING, RADIUS));

        let at = site.settle([0.0, 0.0, 3.05], [0.0, 0.0, 3.05], STANDING, RADIUS);
        assert!((at[2] - 3.10).abs() < 1e-5 && at[0] == 0.0 && at[1] == 0.0, "{at:?}");
        let at = site.settle([1.05, 0.0, 2.5], [1.05, 0.0, 2.5], STANDING, RADIUS);
        assert!((at[0] - 1.10).abs() < 1e-5 && at[2] == 2.5 && at[1] == 0.0, "{at:?}");
        // In a climb the upward part of every way out is dropped.
        let climbing = Capsule { underside: 0.7, top: 1.6, lift: false };
        assert_eq!(site.settle([0.0, 0.0, 1.95], [0.0, 0.0, 1.95], climbing, RADIUS), [0.0, 0.0, 1.95]);
        // A foot that had sunk into the top under it is put back on that
        // top and its move dropped; under a top over its head it is not.
        let at = site.settle([0.0, 0.99, 2.5], [0.0, 0.99, 2.6], Capsule { lift: false, ..STANDING }, RADIUS);
        assert_eq!(at, [0.0, 1.0, 2.5]);
    }

    /// Ground at 0 with a box a metre high from 2 to 3 m out.
    fn yard() -> Course {
        Course { treads: vec![Tread::level(f32::MIN, 2.0, 0.0), Tread::level(2.0, 3.0, 1.0), Tread::level(3.0, f32::MAX, 0.0)] }
    }

    /// A path with a step in it splits into pieces at the step.
    #[test]
    fn a_path_is_cut_at_its_steps() {
        let course = Course::from_path(&[[0.0, 0.0], [1.0, 0.02], [1.96, 0.02], [2.04, 1.0], [2.96, 1.0], [3.04, 0.0], [4.0, 0.0]]);
        assert_eq!(course.treads.len(), 3);
        assert!((course.treads[0].far - 2.0).abs() < 1e-5 && (course.treads[1].far - 3.0).abs() < 1e-5);
        assert!((course.height(0.5) - 0.01).abs() < 1e-5 && course.height(2.5) == 1.0 && course.height(3.5) == 0.0);
    }

    /// A walk into a wall stops with the axis the capsule's radius from
    /// its face, and stays on the ground.
    #[test]
    fn a_wall_stops_the_capsule_at_its_radius() {
        let course = yard();
        let mut at = [1.0, 0.0];
        for _ in 0..60 {
            at = course.settle(at, [at[0] + 0.025, at[1]], STANDING, RADIUS);
        }
        assert!((at[0] - (2.0 - RADIUS)).abs() < 1e-4 && at[1] == 0.0, "{at:?}");
        assert!(!course.held(at, STANDING, RADIUS));
    }

    /// A step lower than a third of the radius or so is ridden up as it
    /// is walked into: its corner pushes mostly upward.
    #[test]
    fn a_low_step_is_ridden_up() {
        let course = Course { treads: vec![Tread::level(f32::MIN, 2.0, 0.0), Tread::level(2.0, f32::MAX, 0.1)] };
        let mut at = [1.0, 0.0];
        for _ in 0..80 {
            at = course.settle(at, [at[0] + 0.025, at[1]], STANDING, RADIUS);
        }
        assert!(at[0] > 2.5 && (at[1] - 0.1).abs() < 2e-3, "{at:?}");
    }

    /// On a top, with the axis past the edge by less than the radius, the
    /// corner holds the capsule; further out it does not.
    #[test]
    fn an_edge_holds_a_foot_that_overhangs_it() {
        let course = yard();
        let rest = |out: f32| 1.0 - (RADIUS - (RADIUS * RADIUS - out * out).sqrt());
        assert!(course.held([3.1, rest(0.1)], STANDING, RADIUS));
        assert!(!course.held([3.1, rest(0.1) + 0.01], STANDING, RADIUS));
        // Sunk a little into the corner, it is put back on it: straight
        // up, since the corner is nearly under it.
        let at = course.settle([3.1, rest(0.1)], [3.1, rest(0.1) - 0.01], STANDING, RADIUS);
        assert!(at[0] == 3.1 && (at[1] - rest(0.1)).abs() < 2e-3, "{at:?}");
        assert!(!course.held([3.5, 0.9], STANDING, RADIUS));
    }

    /// A capsule put down well inside the box's reach comes out a
    /// twentieth of a metre a move, and with no lift only sideways.
    #[test]
    fn a_capsule_deep_in_something_comes_out_by_steps() {
        let course = yard();
        let at = course.settle([3.05, 0.0], [3.05, 0.0], STANDING, RADIUS);
        assert!((at[0] - 3.10).abs() < 1e-5 && at[1] == 0.0, "{at:?}");
        // In a climb the corner of the top, once the foot is over it,
        // pushes back by the level part of the way out alone, and not at
        // all once it is mostly under the foot.
        let climbing = Capsule { underside: 0.7, top: 1.6, lift: false };
        let beside = course.settle([1.75, 0.0], [1.75, 0.0], climbing, RADIUS);
        assert!(beside[0] < 1.75 && beside[1] == 0.0, "{beside:?}");
        let over = course.settle([1.95, 0.0], [1.95, 0.0], climbing, RADIUS);
        assert_eq!(over, [1.95, 0.0]);
    }

    /// A box turned about the upright: a door's leaf 0.9 m wide and 4 cm
    /// thick, lying along z, stops a capsule walked into it from the side
    /// its radius off its face; turned an eighth, it is met at its turned
    /// face and lets the capsule slide off it.
    #[test]
    fn a_turned_box_stops_the_capsule_at_its_face() {
        let leaf = Block::turned([0.0, 0.0, 3.0], [0.9, 2.0, 0.04], [0.0, 0.0, 1.0]);
        assert!(leaf.covers(0.0, 3.4) && !leaf.covers(0.0, 3.5) && !leaf.covers(0.1, 3.0));
        let site = Site { ground: 0.0, blocks: vec![leaf] };
        let mut at = [-2.0, 0.0, 3.0];
        for _ in 0..200 {
            at = site.settle(at, [at[0] + 0.0235, at[1], at[2]], STANDING, RADIUS);
        }
        assert!((at[0] - (-0.02 - RADIUS)).abs() < 1e-4 && (at[2] - 3.0).abs() < 1e-4, "{at:?}");
        // An eighth turned: the face's normal points back along x and on
        // along z, so a walk along x slides along it towards +z.
        let along = [std::f32::consts::FRAC_1_SQRT_2, 0.0, std::f32::consts::FRAC_1_SQRT_2];
        let site = Site { ground: 0.0, blocks: vec![Block::turned([0.0, 0.0, 3.0], [2.0, 2.0, 0.04], along)] };
        let mut at = [-2.0, 0.0, 3.0];
        for _ in 0..100 {
            at = site.settle(at, [at[0] + 0.0235, at[1], at[2]], STANDING, RADIUS);
        }
        let off_face = (at[0] * along[2] - (at[2] - 3.0) * along[0]).abs();
        assert!((off_face - (0.02 + RADIUS)).abs() < 2e-3 && at[2] > 3.3, "{at:?} is {off_face} off the face");
        assert!(site.blocks[0].overlaps([0.0, 0.0, 3.0], STANDING, RADIUS));
    }
}
