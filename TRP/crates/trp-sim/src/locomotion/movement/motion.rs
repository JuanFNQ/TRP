//! Setting the state, applying motion, falling and travelling through the world.

use super::*;

impl Movement {
    pub(super) fn set_state(&mut self, state: State) {
        self.previous_state = self.state;
        self.state = state;
        // Every change but into the walk stops the weapon's bob, and its
        // curves are rewound (`take_walk_stop`).
        if state != State::Move {
            self.walk_stop = true;
        }
        match state {
            State::Move | State::ProneMove => self.ground_check = GROUND_CHECK_MOVING,
            State::Sprint => self.ground_check = GROUND_CHECK_SPRINT,
            State::Jump => self.ground_check = GROUND_CHECK_JUMP,
            _ => {}
        }
    }

    // ------------------------------------------------------------- motion

    /// The common tail of the grounded states: remember the motion, add
    /// gravity, move.
    pub(super) fn apply_motion(&mut self, motion: [f32; 3], dt: f32, world: &dyn World) {
        self.last_blend_velocity = motion.map(|m| m / dt);
        // Down a slope the motion is laid along it (`ProjectMotionToSurface`).
        let motion = slope::along_surface(motion, self.surface, self.normal_history.average());
        self.fall_and_move(motion, dt, world);
    }

    /// Adds gravity and moves, for the states that do not remember their
    /// motion for standing's carry-over. On ground too steep to stand on
    /// the slope's pull takes gravity's place, unless the character has
    /// been falling for a while (`MovementContext.ApplyGravity`).
    pub(super) fn fall_and_move(&mut self, mut motion: [f32; 3], dt: f32, world: &dyn World) {
        let g = self.settings.gravity;
        if self.freefall_time >= FREEFALL_GRACE && !self.grounded {
            let damp = 1.0 / (1.0 + 2.0 * self.freefall_time);
            motion = motion.map(|m| m * damp);
            motion[1] += g * self.freefall_time * dt;
        } else if self.slope != [0.0; 3] {
            for k in 0..3 {
                motion[k] += self.slope[k] * g * dt;
            }
        } else {
            motion[1] += g * self.freefall_time * dt;
        }
        // The head is kept out of walls, but going down (movement spec,
        // "The head's push out of walls"); lying still and crawling, motion
        // toward ground that is not there is then turned back; going down,
        // something ahead or behind pushes the character off it ("Room to
        // move").
        let motion = match self.state {
            State::ToProne => motion,
            _ => self.limit_motion_xz(motion, self.state == State::ProneIdle, world),
        };
        let motion = match self.state {
            State::ProneIdle | State::ProneMove => self.limit_prone_motion(motion, world),
            State::ToProne => self.going_down_push(motion, dt, world).unwrap_or(motion),
            _ => motion,
        };
        if self.grounded {
            self.freefall_time = 0.0;
        } else {
            self.freefall_time += dt;
        }
        self.move_by(motion, dt, world);
    }

    pub(super) fn move_by(&mut self, motion: [f32; 3], dt: f32, world: &dyn World) {
        self.travel(motion, dt, world, true);
    }

    /// Moves the character. `on_ground` is whether the ground holds the
    /// feet up: not in a vault or a climb, where the game's capsule is
    /// clear of whatever the feet pass through.
    pub(super) fn travel(&mut self, motion: [f32; 3], dt: f32, world: &dyn World, on_ground: bool) {
        self.velocity_before_limit = motion.map(|m| m / dt);
        let before = self.position;
        for i in 0..3 {
            self.position[i] += motion[i];
        }
        // Whether the cap is lifted is asked where the motion put the
        // capsule, as the game's controller notes what it overlaps there.
        let uncapped = world.lifts_cap(self.position, self.capsule());
        // What stands in the way first, then the ground where that leaves
        // the character: ground too high to step onto is the host's to
        // refuse, before the feet are put on it.
        self.position = world.obstructed(before, self.position, self.capsule());
        if on_ground && !world.sets_feet() {
            let ground = world.footing(self.position);
            if self.position[1] < ground {
                self.position[1] = ground;
            }
        }
        // A move pushes the capsule out of the ground, and lying down that
        // lifts the feet by as much as the ball it is reaches into the floor
        // under it, a host's move or not; the game's controller does nothing
        // with no motion at all (movement spec, "The character's own
        // collision"). The floor is felt by a ray down from the ball's middle
        // as long as its radius, so that a character riding over the floor
        // (by up to its ground check) is not lifted. Recorded: a crawl's
        // first frames lift the feet 1.59 cm off a floor they stood on, and
        // they stay there, gravity being off on the ground; over a floor 2 cm
        // under the feet, nothing.
        let reach = self.reach_under_feet();
        if on_ground && reach > 0.0 && motion.iter().any(|m| *m != 0.0) {
            let [_, middle] = self.capsule_figures();
            let radius = middle + reach;
            let centre = [self.position[0], self.position[1] + middle, self.position[2]];
            if let Some(hit) = world.ray(centre, [0.0, -1.0, 0.0], radius, crate::weapons::mounting::Layer::Static) {
                self.position[1] += (radius - hit.distance).max(0.0);
            }
        }
        for i in 0..3 {
            self.velocity[i] = (self.position[i] - before[i]) / dt;
        }
        // The hard cap for the state: whatever asked for more, the
        // character goes no faster. Falling speed is not counted.
        let [x, y, z] = self.velocity;
        let speed = (x * x + y.max(0.0) * y.max(0.0) + z * z).sqrt();
        if speed > self.speed_cap && !uncapped {
            let whole = (x * x + y * y + z * z).sqrt();
            // A falling character keeps its direction and is slowed until
            // the level part is at the cap.
            let allowed = if y < 0.0 { self.speed_cap * whole / speed } else { self.speed_cap };
            for i in 0..3 {
                self.velocity[i] *= allowed / whole;
                self.position[i] = before[i] + self.velocity[i] * dt;
            }
        }
    }

    /// Body axes (x right, y forward) to world (x, z).
    pub(super) fn to_world(&self, v: [f32; 2]) -> [f32; 2] {
        let (sin, cos) = self.body_yaw.to_radians().sin_cos();
        [v[0] * cos + v[1] * sin, -v[0] * sin + v[1] * cos]
    }

    pub(super) fn to_local(&self, v: [f32; 2]) -> [f32; 2] {
        let (sin, cos) = self.body_yaw.to_radians().sin_cos();
        [v[0] * cos - v[1] * sin, v[0] * sin + v[1] * cos]
    }
}

/// The animator's eight-way direction value: left 0, front 1, right 2,
/// back 3, with the diagonals halfway between. `None` for no direction.
pub(super) fn discrete_direction(v: [f32; 2]) -> Option<f32> {
    let [x, y] = v;
    if x.abs() < f32::EPSILON {
        return if y >= f32::EPSILON {
            Some(1.0)
        } else if y < 0.0 {
            Some(3.0)
        } else {
            None
        };
    }
    let ratio = (y / x).abs();
    Some(if ratio > 2.0 {
        if y > 0.0 { 1.0 } else { 3.0 }
    } else if ratio < 0.5 {
        if x > 0.0 { 2.0 } else { 0.0 }
    } else {
        match (x > 0.0, y > 0.0) {
            (true, true) => 1.5,
            (false, true) => 0.5,
            (true, false) => 2.5,
            (false, false) => 3.5,
        }
    })
}
