//! The capsule, the world trait, the stopwatch and flat ground.

use super::*;

/// The upright space the character's collision takes up over its feet, in
/// metres: from `underside` to `top`. Standing, crouched and lying the
/// underside is at the feet. In a vault or a climb the game shrinks its
/// capsule from below to get it over the obstacle (vaulting spec, "The
/// capsule"), and a host's own collision has to let the character past
/// whatever is lower than the underside.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Capsule {
    pub underside: f32,
    pub top: f32,
    /// Whether what the character is in may push it upward out of itself,
    /// which is how the game's own controller sets it on the ground and
    /// over an edge. In a vault or a climb the game switches that off
    /// (movement spec, "The character's own collision"): the character is
    /// then pushed out sideways only and rises by the move's motion alone.
    pub lift: bool,
}

/// The ground the character stands on, and whatever else of the host's
/// world the controller has to feel.
pub trait World {
    fn ground_height(&self, x: f32, z: f32) -> f32;

    /// How high the ground holds the feet of a character whose feet are at
    /// `at`. Left alone, the ground's height under its axis. A host whose
    /// character has a foot with some width to it (the game's capsule has
    /// a rounded one) answers with where that foot rests: over an edge it
    /// overhangs, on the edge. The ground check and the feet go by this.
    fn footing(&self, at: [f32; 3]) -> f32 {
        self.ground_height(at[0], at[2])
    }

    /// Which way the ground faces there. Level unless the host says
    /// otherwise; a lying body is laid to it.
    fn ground_normal(&self, _x: f32, _z: f32) -> [f32; 3] {
        [0.0, 1.0, 0.0]
    }

    /// Of the ways out of the ground that the character's capsule, its feet
    /// at `at`, was last pushed along, the one that points most upward, of
    /// those within 85 degrees of straight up
    /// (`SimpleCharacterController.surfaceNormalAccordingToCapsule`):
    /// straight up if the capsule was in nothing, nothing at all (all
    /// three zero) if it was in walls alone. The slope's rules take it
    /// where the ray under the axis finds no ground, or ground too steep
    /// to stand on (movement spec, "Slopes and sliding"). Left alone: the
    /// ground's normal under the axis where the feet are on the ground,
    /// straight up where they are not.
    fn contact_normal(&self, at: [f32; 3], capsule: Capsule) -> [f32; 3] {
        /// Within this of the ground the feet are on it: rounding only,
        /// the simulation puts the feet on the ground exactly.
        const ON: f32 = 1e-4;
        if at[1] + capsule.underside - self.footing(at) <= ON {
            self.ground_normal(at[0], at[2])
        } else {
            [0.0, 1.0, 0.0]
        }
    }

    /// How far along the line from `from` to `to` the first solid thing
    /// lies, in metres, if anything does: the game's line cast, which the
    /// weapon feels for obstacles with (`Movement::weapon_overlap`). A host
    /// with walls gives its own. Left alone, only the ground counts: where
    /// the line first goes under it, and nothing for a line that starts
    /// under it (the game's ground is hit from above only).
    fn line(&self, from: [f32; 3], to: [f32; 3]) -> Option<f32> {
        let length = (0..3).map(|k| (to[k] - from[k]).powi(2)).sum::<f32>().sqrt();
        let above = |along: f32| {
            let t = if length > 0.0 { along / length } else { 0.0 };
            let at: [f32; 3] = std::array::from_fn(|k| from[k] + (to[k] - from[k]) * t);
            at[1] - self.ground_height(at[0], at[2])
        };
        if length <= 0.0 || above(0.0) < 0.0 {
            return None;
        }
        // Walked in short steps, then closed in on where it goes under.
        let steps = (length / LINE_STEP).ceil().max(1.0) as usize;
        let under = (1..=steps).map(|step| length * step as f32 / steps as f32).find(|along| above(*along) < 0.0)?;
        let (mut over, mut under) = (under - length / steps as f32, under);
        for _ in 0..16 {
            let middle = (over + under) / 2.0;
            if above(middle) < 0.0 { under = middle } else { over = middle }
        }
        Some(under)
    }

    /// The weapon's line cast (`OverlapLn`, on `WEAPON_OCCLUSION_LAYERS`,
    /// whose `Player` layer holds every character's capsule): how far along
    /// it the first thing lies, and whether that is another character's
    /// capsule (`OverlapsWithPlayer`). A line never meets a capsule it
    /// starts inside, the character's own among them (the game leaves its
    /// own out, `IsOverlapHitIgnored`). Left alone, `line`, with no
    /// characters.
    fn line_meets(&self, from: [f32; 3], to: [f32; 3]) -> Option<(f32, bool)> {
        self.line(from, to).map(|along| (along, false))
    }

    /// Whether what is solid at a point a line hit is the terrain itself
    /// and not something standing on it: the game never vaults or climbs
    /// its terrain (vaulting spec, "The obstacle"). Left alone everything
    /// is, as for a world that is only ground; a host with walls and
    /// crates says no for those.
    fn terrain(&self, _at: [f32; 3]) -> bool {
        true
    }

    /// Where a move from `from` toward `to` leaves the character's feet,
    /// once whatever stands in the way has had its say. The controller
    /// has no collision of its own beyond the ground: a host with walls
    /// answers with the place its own collision allows, for a character
    /// that takes up `capsule` over its feet. `to` is where the motion alone would put the
    /// feet, which may be under the ground there: the ground lifts them
    /// afterwards (`ground_height` at the place answered), so a rise too
    /// high to step onto has to be refused here. Nothing stands in the way
    /// unless it says so.
    fn obstructed(&self, _from: [f32; 3], to: [f32; 3], _capsule: Capsule) -> [f32; 3] {
        to
    }

    /// Whether the capsule, with its feet where a frame's motion put them
    /// (`at`), is in something that lifts the hard cap on speed for the
    /// move: a door's leaf (`SimpleCharacterController`'s colliders with no
    /// speed limit, which the game fills with every door's). A door that
    /// swings into the character so pushes it out of its way at whatever
    /// pace that takes (interactions spec, "A door: which gesture, and how
    /// it swings"). Nothing does unless the host says so.
    fn lifts_cap(&self, _at: [f32; 3], _capsule: Capsule) -> bool {
        false
    }

    /// Whether `obstructed` also sets the character on the ground it has
    /// moved onto or into, as the game's own controller does by pushing
    /// its capsule up out of whatever it is in (movement spec, "The
    /// character's own collision"). Left alone it does not, and the
    /// simulation puts the feet up onto `footing` itself after every move:
    /// that lifts a character onto anything its axis is over, a wall it
    /// has just vaulted and not yet left as well.
    fn sets_feet(&self) -> bool {
        false
    }

    /// What a step's line down from `from` to `to` meets first, for the
    /// sounds of moving (`steps.rs`): `None` if nothing, else the surface of
    /// what it hit (`BaseBallistic.ESurfaceSound`'s number;
    /// `trp_data::sound::SURFACES`). Left alone, whatever `line` hits is
    /// concrete, which is the game's surface for a thing without one of its
    /// own.
    fn surface(&self, from: [f32; 3], to: [f32; 3]) -> Option<u8> {
        self.line(from, to).map(|_| crate::sound::steps::CONCRETE)
    }

    /// Whether a character whose feet are at `at` is held up by something
    /// it touches other than what lies under its axis: the edge of a
    /// ledge its foot overhangs. The game's controller counts a character
    /// it has had to push upward out of something as on the ground
    /// (movement spec, "The character's own collision"), which is what
    /// keeps one standing that has climbed to a top's edge and no
    /// further. Nothing does unless the host says so.
    fn held(&self, _at: [f32; 3], _capsule: Capsule) -> bool {
        false
    }

    /// What a ray from `from` along `direction` (a unit vector) meets first
    /// within `length`, on the world's `layer` (`Physics.Raycast`): the
    /// mount's search and its target place (`crate::mounting`). A ray that
    /// starts inside something does not meet it. Left alone, only the
    /// ground: where `line` first goes under it, with its normal there.
    fn ray(&self, from: [f32; 3], direction: [f32; 3], length: f32, _layer: crate::weapons::mounting::Layer) -> Option<crate::weapons::mounting::Hit> {
        let to: [f32; 3] = std::array::from_fn(|k| from[k] + direction[k] * length);
        let distance = self.line(from, to)?;
        let point: [f32; 3] = std::array::from_fn(|k| from[k] + direction[k] * distance);
        Some(crate::weapons::mounting::Hit { distance, point, normal: self.ground_normal(point[0], point[2]) })
    }

    /// Whether a sphere is clear of everything on `layer`
    /// (`Physics.CheckSphere`): the prone mount's room ahead. Left alone,
    /// only the ground under its middle counts.
    fn sphere_clear(&self, centre: [f32; 3], radius: f32, _layer: crate::weapons::mounting::Layer) -> bool {
        centre[1] - radius >= self.ground_height(centre[0], centre[2])
    }

    /// Whether a box swept from `from` along `direction` (a unit vector)
    /// for `length` meets anything on `layer` (`Physics.BoxCast`): `half` its
    /// half-width, half-height and half-depth, turned to `yaw` about the
    /// upright. The mount's way to its target place. Left alone nothing
    /// is met: the box rides above the step height, over the ground.
    fn box_sweep(&self, _from: [f32; 3], _half: [f32; 3], _yaw: f32, _direction: [f32; 3], _length: f32, _layer: crate::weapons::mounting::Layer) -> bool {
        false
    }

    /// Whether a box reaches into anything solid (`Physics.CheckBox`,
    /// `OverlapBox`): `centre` its middle, `half` its half-width,
    /// half-height and half-depth along its own axes, `turn` how those
    /// axes are turned. The refusals of a step aside, a roll, lying down
    /// and standing up ask it (movement spec, "Room to move"); the game's
    /// water and other characters count where a host has them. Left alone,
    /// only the ground: a corner of the box under it.
    fn box_overlaps(&self, centre: [f32; 3], half: [f32; 3], turn: [f32; 4]) -> bool {
        (0..8).any(|corner| {
            let reach = quat_rotate(turn, std::array::from_fn(|k| if corner >> k & 1 == 0 { half[k] } else { -half[k] }));
            let at: [f32; 3] = std::array::from_fn(|k| centre[k] + reach[k]);
            at[1] < self.ground_height(at[0], at[2])
        })
    }

    /// How far a sphere of `radius` swept from `from` along `direction` (a
    /// unit vector) goes before it meets something solid, within `length`
    /// (`Physics.SphereCast`); what it starts inside is not met. Lying
    /// down asks it for room ahead and behind. Left alone, only the ground
    /// under its middle, felt every centimetre.
    fn sphere_cast(&self, from: [f32; 3], radius: f32, direction: [f32; 3], length: f32) -> Option<f32> {
        /// How finely the way is felt, in metres.
        const STEP: f32 = 0.01;
        let touches = |along: f32| {
            let at: [f32; 3] = std::array::from_fn(|k| from[k] + direction[k] * along);
            at[1] - radius < self.ground_height(at[0], at[2])
        };
        if touches(0.0) {
            return None;
        }
        let steps = (length / STEP).ceil().max(1.0) as usize;
        (1..=steps).map(|step| length * step as f32 / steps as f32).find(|along| touches(*along))
    }

    /// Whether the character's capsule, its feet at `at` moved by `shift`
    /// and turned by `turn` degrees about its axis, would collide
    /// (`MovementContext.RotationOverlapPrediction`): a mounted turn outward
    /// is refused then. Left alone nothing is in the way.
    fn turn_blocked(&self, _at: [f32; 3], _shift: [f32; 3], _turn: f32, _capsule: Capsule) -> bool {
        false
    }

    /// How far a box has to move to be clear of everything solid: for each
    /// thing it is in, the shortest move that takes it out of that thing,
    /// lengthened by `margin`, all added up. This is the game's push of
    /// the camera's box (`Movement::camera_push`). `centre` is the box's
    /// middle, `half` its half-width, half-height and half-depth along its
    /// own axes, and `turn` how those axes are turned. A host with walls
    /// gives its own. Left alone, only the ground counts: the box is
    /// lifted along the ground's normal by as far as its lowest corner is
    /// under it.
    /// How far a capsule (the two ends of its middle line, and its radius)
    /// has to move to be clear of everything solid: for each thing it is
    /// in, the shortest move that takes it out, lengthened by a millimetre
    /// (`pom::MARGIN`), all added up (`ColliderExtrusion.
    /// CalculateDepenetration`). The head's push out of walls asks it (movement
    /// spec, "The head's push out of walls"); the low-poly world, doors, the
    /// terrain and other characters count. Left alone, only the ground:
    /// along its normal by as far as the capsule's lowest point is under it.
    fn capsule_push(&self, ends: [[f32; 3]; 2], radius: f32) -> [f32; 3] {
        let deepest = ends
            .iter()
            .map(|end| {
                let normal = self.ground_normal(end[0], end[2]);
                ((self.ground_height(end[0], end[2]) - end[1]) * normal[1] + radius, normal)
            })
            .max_by(|a, b| a.0.total_cmp(&b.0));
        match deepest {
            Some((under, normal)) if under > 0.0 => normal.map(|n| n * (under + pom::MARGIN)),
            _ => [0.0; 3],
        }
    }

    fn push_out(&self, centre: [f32; 3], half: [f32; 3], turn: [f32; 4], margin: f32) -> [f32; 3] {
        let mut deepest: Option<(f32, [f32; 3])> = None;
        for corner in 0..8 {
            let reach = quat_rotate(turn, std::array::from_fn(|k| if corner >> k & 1 == 0 { half[k] } else { -half[k] }));
            let at: [f32; 3] = std::array::from_fn(|k| centre[k] + reach[k]);
            let normal = self.ground_normal(at[0], at[2]);
            // Under the ground by this much, measured square to it.
            let under = (self.ground_height(at[0], at[2]) - at[1]) * normal[1];
            if under > 0.0 && deepest.is_none_or(|(most, _)| under > most) {
                deepest = Some((under, normal));
            }
        }
        deepest.map_or([0.0; 3], |(under, normal)| normal.map(|n| n * (under + margin)))
    }
}

/// How finely the ground is felt for along a line, in metres.
const LINE_STEP: f32 = 0.05;

/// The clock the game times the player by: a stopwatch that is read once
/// a frame in whole milliseconds ("The player's clock" in the movement
/// spec). At 120 frames a second it gives 8, 8 and 9 ms in turn.
#[derive(Debug, Clone, Copy, Default)]
pub struct Stopwatch {
    seconds: f64,
    read: u64,
}

impl Stopwatch {
    /// A frame of `dt` seconds has passed: the time since the last
    /// reading, as the stopwatch shows it.
    pub fn tick(&mut self, dt: f32) -> f32 {
        self.seconds += f64::from(dt.max(0.0));
        let now = (self.seconds * 1000.0) as u64;
        let passed = now - self.read;
        self.read = now;
        passed as f32 / 1000.0
    }
}

/// Level ground at height zero.
pub struct Flat;

impl World for Flat {
    fn ground_height(&self, _x: f32, _z: f32) -> f32 {
        0.0
    }
}

/// Unity's `Mathf.Sign`: zero counts as positive.
pub(super) fn sign(v: f32) -> f32 {
    if v >= 0.0 {
        1.0
    } else {
        -1.0
    }
}

pub(super) fn is_zero(v: [f32; 2]) -> bool {
    v[0] == 0.0 && v[1] == 0.0
}
