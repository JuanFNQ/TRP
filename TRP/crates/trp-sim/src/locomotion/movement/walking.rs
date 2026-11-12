//! Walking and sprinting: speed selection, move direction, sprint and landing.

use super::*;

impl Movement {
    /// A walk or a sprint goes on (`MovementContext.CanWalk`): not locked
    /// against an obstacle, and not with medicine at work that cannot be
    /// used walking.
    pub(super) fn walk_refused(&self) -> bool {
        self.blocked || !self.conditions.can_walk()
    }

    /// A change of the speed asked of the movement state in hand
    /// (`MovementState.ChangeSpeed`): from the speed as now limited. A
    /// sprint takes none.
    pub(super) fn change_speed(&mut self, delta: f32) {
        if self.state != State::Sprint {
            self.select_speed(self.clamped_speed() + delta);
        }
    }

    /// Sets the speed wheel.
    pub fn select_speed(&mut self, speed: f32) {
        let top = self.settings.max_speed;
        self.character_speed = speed.clamp(0.0, top);
        self.relative_speed = if top > 0.0 { self.character_speed / top } else { 0.0 };
        self.speed_set = true;
    }

    pub(super) fn hard_speed_cap(&self, state: State) -> f32 {
        let range = match state {
            State::Idle | State::ProneIdle | State::Roll => STANDING_SPEED_CAP,
            // A landing, going down and getting up have no cap of their
            // own and take the general one. The sidestep is given it.
            State::Move | State::Landing | State::ToProne | State::FromProne | State::Sidestep => WALKING_SPEED_CAP,
            // A vault, a climb and what follows them have none of their
            // own either; nor have the approach to a door and the kick.
            State::Vault | State::Climb | State::VaultFall | State::VaultLanding => WALKING_SPEED_CAP,
            State::Approach | State::Breach => WALKING_SPEED_CAP,
            // Blind fire has no cap of its own (`SpeedLimiter.
            // DefaultSpeedLimits` does not list it): the general one.
            State::BlindFire => WALKING_SPEED_CAP,
            State::Sprint | State::Jump | State::SprintStop => SPRINTING_SPEED_CAP,
            State::ProneMove => CRAWLING_SPEED_CAP,
        };
        // The carried weight's say on the walking cap is left out of a
        // sprint's, and of a vault's and a climb's (vaulting spec).
        self.settings.hard_speed_cap(range, matches!(state, State::Sprint | State::Vault | State::Climb))
    }

    /// Notes the sprint switching on. The weapon's logic lowers the sights
    /// at that moment (firing spec, "Aiming, and what takes it away"), and
    /// with them goes the cap aiming puts on the speed, before the frame's
    /// look at its limits; a host whose weapon kept them up says so with
    /// the next frame's input.
    /// How fast going down and getting up play (the animator's
    /// `TransitionMultiplier`): by the carried weight, and as slowly as
    /// fully overweight on spent legs.
    pub(super) fn transition_speed(&self) -> f32 {
        let s = &self.settings;
        let weight = s.overweight.max(if self.stamina.legs_spent() { 1.0 } else { 0.0 });
        lerp(s.transition_speed_range[0], s.transition_speed_range[1], weight)
    }

    /// Asks for the sprint or lets it go (`MovementContext.EnableSprint`).
    /// The flag is the stamina's: a sprint is one being paid for, and one
    /// asked for on spent legs does not start (stamina spec, "Who calls
    /// it").
    pub(super) fn enable_sprint(&mut self, on: bool) {
        self.sprinting = self.stamina.sprint(on, self.prone, self.frame_clock);
        // Asked where the health lets it, it stops the hands' gestures but
        // the operations (`EnableSprint`: `ForceStopInteractions`), whether
        // or not the legs then run.
        if on && self.conditions.can_sprint() {
            self.force_stop_interactions();
        }
    }

    /// Stops every gesture of the hands but the operations, the one playing
    /// and those waiting (`AnimatedInteractionsModel.ForceStopInteractions`):
    /// the inventory opening, a sprint asked for, and, asked by the host
    /// before it tells the weapon, a magazine's reload or check.
    pub fn force_stop_interactions(&mut self) {
        let shows = self.interactions.force_stop();
        self.show(shows);
    }

    pub(super) fn watch_sprint(&mut self) {
        if self.sprinting && !self.was_sprinting {
            self.sprint_started = true;
            self.aiming = false;
        }
        self.was_sprinting = self.sprinting;
    }

    pub(super) fn enter_move(&mut self, from: State) {
        let s = &self.settings;
        let clamped = self.clamped_speed();
        self.mv = MoveData {
            inertia_direction: self.to_world(self.movement_direction),
            previous_target: clamped,
            smoothed_yaw: self.body_yaw,
            speed: if from == State::Sprint { [0.0, 1.0] } else { [0.0; 2] },
            lower_range: (clamped - s.movement_acceleration_range.evaluate(s.walk_inertia)).max(0.0),
            // The game does not clear these on entering the state: a walk
            // begins with the rates the last walk's smoothing ended on
            // ("What entering does not clear" in the movement spec).
            speed_velocity: self.mv.speed_velocity,
            direction_velocity: self.mv.direction_velocity,
            transition_coef: self.mv.transition_coef,
            last_direction_input: self.mv.last_direction_input,
            sprint_postponed: self.mv.sprint_postponed,
            unheard_time: self.mv.unheard_time,
            // Lying down the game reads two factors that are 1 on foot.
            speed_acceleration: if self.prone { s.prone_speed_acceleration } else { 1.0 },
            direction_acceleration: if self.prone { s.prone_direction_acceleration } else { 1.0 },
            ..MoveData::default()
        };
        // The state takes the direction it was entered with as its input
        // until the next command arrives, a frame later.
        self.direction = self.movement_direction;
    }

    /// What the walking state still holds from the last walk: the rate its
    /// direction smoothing was moving at when that walk ended, per second
    /// in the body's axes. The next walk starts with it.
    pub fn walk_leftover(&self) -> [f32; 2] {
        self.to_local(self.mv.direction_velocity)
    }

    pub fn set_walk_leftover(&mut self, rate: [f32; 2]) {
        self.mv.direction_velocity = self.to_world(rate);
    }

    /// The leftover that carries the direction `shift` away from where a
    /// walk begun from standing with `keys` held starts it, on a first
    /// frame `dt` long. For starting a replay as a recording started.
    pub fn walk_leftover_for(&self, shift: [f32; 2], keys: [f32; 2], dt: f32) -> [f32; 2] {
        let s = &self.settings;
        let target = self.clamped_speed().clamp(0.01, 1.0);
        let mut smooth_time = target * s.walk_inertia * s.direction_inertia(keys);
        if target < self.smoothed_speed {
            smooth_time /= s.deceleration_speed;
        }
        let blend_time = s.min_direction_blend_time.max(smooth_time);
        // On that frame the smoothing aims at where it stands, so all that
        // moves the direction is the leftover: this far for a rate of one.
        let reach = smooth_damp([0.0], [0.0], &mut [1.0], blend_time, s.inertia_input_max_speed, dt)[0];
        [shift[0] / reach, shift[1] / reach]
    }

    pub(super) fn update_move(&mut self, dt: f32, world: &dyn World) {
        self.update_movement_direction(dt);

        // With no input for a while and the speed run down, the animator
        // is told the player no longer wants to move; it blends to standing.
        let s = &self.settings;
        let no_input_for_long = self.mv.time_without_input > s.max_time_without_input
            && length(self.mv.speed) < s.exit_speed_threshold
            && self.mv.reversal_pause <= 0.0;
        // So is it when the character is locked against an obstacle, or
        // may not walk.
        if no_input_for_long || self.walk_refused() {
            self.inert = false;
        }

        self.follow_look_yaw(dt);
        let motion = self.animated_motion;
        self.apply_motion(motion, dt, world);
        self.check_obstacle();

        if self.movement_direction[1] > 0.1 {
            if self.mv.sprint_postponed {
                self.enable_sprint(true);
                if self.sprinting {
                    self.pose_level = 1.0;
                }
                self.mv.sprint_postponed = false;
            }
            // The sprint itself starts when the animator's sprint state does.
            if self.sprinting && self.pose_level > SPRINT_POSE_THRESHOLD && self.smoothed_speed >= 1.0 {
                self.sprint_flag = true;
            }
        }
    }

    fn update_movement_direction(&mut self, dt: f32) {
        let clamped = self.clamped_speed();
        let mut input = self.direction;
        let direction_factor = self.settings.direction_inertia(input);

        // A reversal is answered with a short pause before the new direction takes over.
        let m = &mut self.mv;
        if !is_zero(m.prev_direction)
            && !is_zero(input)
            && m.reversal_pause <= 0.0
            && dot(normalize(m.prev_direction), normalize(input)) < 0.0
        {
            m.next_direction = input;
            m.reversal_pause = self.settings.move_time;
        }
        if m.reversal_pause > 0.0 {
            input = [0.0; 2];
            self.direction = input;
            m.reversal_pause -= dt;
            if m.reversal_pause < 0.0 {
                m.reversal_pause = 0.0;
                self.direction = m.next_direction;
                // The pause ends by telling the animator the player has
                // stopped, so a reversal goes through standing: the walk
                // blends out and starts again the other way.
                self.inert = false;
                return;
            }
        }
        if !is_zero(input) {
            m.prev_direction = input;
        }

        let world_input = self.to_world(input);
        let s = &self.settings;
        let m = &mut self.mv;
        let target = clamped.clamp(0.01, 1.0);
        let crouch = lerp(
            1.0,
            lerp(s.crouch_speed_acceleration[0], s.crouch_speed_acceleration[1], clamped),
            s.crouch_acceleration_weight.evaluate(1.0 - self.pose_level),
        );
        let unit = normalize(world_input);
        let wanted = target * m.speed_acceleration * crouch;
        let desired = [unit[0] * wanted, unit[1] * wanted];

        let just_released = !is_zero(m.last_direction_input) && is_zero(world_input);
        let just_pressed = !is_zero(world_input) && is_zero(m.last_nonzero_direction);
        if just_released {
            m.transition_coef = self.smoothed_speed;
        } else if just_pressed {
            m.transition_coef = target;
        } else if m.previous_target != target {
            m.lower_range = (clamped - s.movement_acceleration_range.evaluate(s.walk_inertia)).max(0.0);
            m.transition_coef = (target - length(m.speed)).abs();
        }
        m.previous_target = target;

        let mut smooth_time = m.transition_coef * s.walk_inertia * direction_factor;
        if target < self.smoothed_speed || is_zero(world_input) {
            smooth_time /= s.deceleration_speed;
        }
        // The game gives this smoothing no time of its own, so it runs on
        // the engine's; the direction's, below, is given the player's.
        m.speed =
            smooth_damp(m.speed, desired, &mut m.speed_velocity, smooth_time, s.inertia_input_max_speed, self.frame_dt);
        let mut magnitude = length(m.speed);
        if magnitude < m.lower_range {
            m.speed = [desired[0] / target * m.lower_range, desired[1] / target * m.lower_range];
            magnitude = m.lower_range;
        }
        if !self.sprinting {
            // Within a thousandth the two count as equal (step 10 of
            // "Moving" in the movement spec).
            self.smoothed_speed = if (magnitude - self.smoothed_speed).abs() < 0.001 {
                magnitude
            } else {
                lerp(self.smoothed_speed, magnitude, s.sudden_changes_smoothness * dt)
            };
        }

        m.last_direction_input = world_input;
        if !is_zero(world_input) {
            m.last_nonzero_direction = world_input;
        }
        let blend_time = s.min_direction_blend_time.max(smooth_time) / m.direction_acceleration;
        m.inertia_direction =
            smooth_damp(m.inertia_direction, world_input, &mut m.direction_velocity, blend_time, s.inertia_input_max_speed, dt);
        if is_zero(world_input) {
            m.time_without_input += dt;
        } else {
            m.time_without_input = 0.0;
        }
        // The update ends by dropping the input it was handed, once the
        // state's time without a call has run out, which it has for good
        // a fifth of a second into a character's first walk: the next
        // frame's input has to be handed over anew, and a second update in
        // the same frame, or a frame in which a screen has the keys, runs
        // with none.
        m.unheard_time += dt;
        if m.unheard_time > MAX_WITHOUT_MOVE_CALL {
            self.direction = [0.0; 2];
        }

        let inertia_direction = m.inertia_direction;
        self.movement_direction = self.to_local(inertia_direction);
        let facing = if is_zero(input) { self.movement_direction } else { input };
        if let Some(discrete) = discrete_direction(facing) {
            self.discrete_direction = discrete;
        }
    }

    fn follow_look_yaw(&mut self, dt: f32) {
        if self.mount.is_some() {
            return;
        }
        let behind = delta_angle(self.yaw, self.mv.smoothed_yaw).abs();
        let hurry = inverse_lerp(10.0, 45.0, behind) + 1.0;
        self.mv.smoothed_yaw =
            lerp_angle(self.mv.smoothed_yaw, self.yaw, self.settings.rotation_lerp_speed * dt * hurry);
        self.body_yaw = self.mv.smoothed_yaw;
        self.note_rotation();
    }

    pub(super) fn update_sprint(&mut self, dt: f32, world: &dyn World) {
        let input = self.direction;
        // Letting go of forward, or of sprint, only lowers the animator's
        // flag; the sprint ends when the animator leaves its sprint state.
        // Nothing moves the character in such a frame. A character locked
        // against an obstacle is treated the same. Not in a frame in which
        // an update has already dropped its input (below): that one goes
        // on to sprint with none.
        let dropped = self.frame_count == self.sprint_dropped_in;
        if (input[1].abs() < f32::EPSILON || self.slide || self.walk_refused()) && !dropped {
            self.sprint_flag = false;
            if input[0].abs() < f32::EPSILON || self.slide {
                self.inert = false;
            }
            self.check_obstacle();
            return;
        }
        if !self.sprinting {
            self.sprint_flag = false;
            self.check_obstacle();
            return;
        }
        // The sprinting state looks at the speed limits again itself, so
        // one set since the frame's own look (the cap a jump leaves, on
        // the frame a sprint is landed in) holds the sprint's pace at once.
        self.work_in_limits();
        let t = (dt * self.settings.direction_lerp_speed).clamp(0.0, 1.0);
        self.movement_direction = [
            self.movement_direction[0] + (input[0] - self.movement_direction[0]) * t,
            self.movement_direction[1] + (input[1] - self.movement_direction[1]) * t,
        ];
        if let Some(discrete) = discrete_direction(input) {
            self.discrete_direction = discrete;
        }
        // A sprinting update drops the input it was handed, so that it
        // has to be handed over again every frame: a frame without, as
        // when a screen has the keys, ends the sprint like keys let go.
        self.direction = [0.0; 2];
        self.sprint_dropped_in = self.frame_count;
        self.sprint_acceleration(dt);
        self.follow_look_yaw(dt);
        let motion = self.animated_motion;
        self.apply_motion(motion, dt, world);
        self.check_obstacle();
    }

    /// A sprint coming to a stop: the animator plays its own clip for it.
    pub(super) fn update_sprint_stop(&mut self, dt: f32, world: &dyn World) {
        self.follow_look_yaw(dt);
        let keep = self.settings.sprint_stop_motion_preservation;
        let motion = [0, 1, 2].map(|i| lerp(self.animated_motion[i], self.last_blend_velocity[i] * dt, keep));
        self.apply_motion(motion, dt, world);
    }

    /// Landing from a jump made while moving. The landing clip carries the
    /// character on unless the player has let go for a while.
    pub(super) fn update_landing(&mut self, dt: f32, world: &dyn World) {
        /// The speed setting a landing with no input settles toward.
        const LANDING_SPEED: f32 = 0.35;
        if self.landing_idle_time > self.settings.max_time_without_input / 2.0 {
            self.landing_still = true;
        }
        if is_zero(self.direction) {
            self.landing_idle_time += dt;
            if self.smoothed_speed > LANDING_SPEED {
                self.smoothed_speed =
                    lerp(self.smoothed_speed, LANDING_SPEED, self.settings.sudden_changes_smoothness * dt);
            }
        }
        self.turn_body(dt);
        if !self.landing_still {
            let motion = self.animated_motion;
            self.apply_motion(motion, dt, world);
            self.note_rotation();
        }
    }

    fn sprint_acceleration(&mut self, dt: f32) {
        let s = &self.settings;
        let step = s.sprint_acceleration * dt;
        // What a landing left as a cap on the selected speed also scales
        // the sprint's top speed while it lasts.
        let after_landing = self.landing_limit_in_effect.unwrap_or(1.0);
        let limit = (s.physical_sprint_speed * s.sprinting_speed + 1.0) * after_landing;
        // Turning hard while sprinting costs speed. The measure is the
        // average yaw change per frame over the last frames.
        let turn_rate = if self.yaw_deltas.is_empty() {
            0.0
        } else {
            self.yaw_deltas.iter().sum::<f32>() / self.yaw_deltas.len() as f32
        };
        let floor = s.sprint_speed_inertia.evaluate(f32::MAX) * (2.0 - s.inertia);
        let turn = s.sprint_speed_inertia.evaluate(turn_rate.abs()).max(floor);
        // `Mathf.Clamp`: a landing's limit of nought leaves 0.1 (the lower
        // bound wins), and the sprint's speed between 0.01 and that.
        let limit = unity_clamp(limit * turn, 0.1, limit);
        self.sprint_speed = unity_clamp(self.sprint_speed + step * sign(limit - self.sprint_speed), 0.01, limit);
    }
}
