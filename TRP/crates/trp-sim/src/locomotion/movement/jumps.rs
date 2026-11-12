//! Jumps, vaults and the obstacle checks.

use super::*;

impl Movement {
    pub(super) fn enter_jump(&mut self, from: State) {
        // The jump is paid for as its state is entered.
        self.stamina.jump(self.frame_clock);
        self.blocked = false;
        let s = &self.settings;
        let before = self.velocity_before_limit;
        self.landing_flag = false;
        let heading = if matches!(from, State::Move | State::Sprint) {
            let d = normalize(self.to_world(self.movement_direction));
            [d[0], 0.0, d[1]]
        } else {
            [0.0; 3]
        };
        // Per axis, whichever of the two has the larger magnitude.
        let pick = |a: f32, b: f32| if a.abs() > b.abs() { a } else { b };
        let v = [pick(before[0], heading[0]), pick(before[2], heading[2])];
        let speed = length(v);
        let direction = if speed > 0.1 {
            [v[0] / speed, 0.0, v[1] / speed]
        } else {
            let yaw = self.body_yaw.to_radians();
            [yaw.sin(), 0.0, yaw.cos()]
        };
        self.jump = JumpData {
            direction,
            initial_speed: speed,
            speed,
            // The push off the ground is weaker the heavier the load, and
            // as weak as at full overweight on a damaged leg without
            // painkillers (`JumpPlayerState.Enter`).
            lift: s.lift_velocity_by_speed.evaluate(speed) * {
                let c = self.conditions;
                if c.leg_damaged() && !c.on_painkillers { 0.66 } else { lerp(1.0, 0.66, self.stamina.settings.overweight) }
            },
            lift_delay: s.jump_delay_by_speed.evaluate(speed),
            from_sprint: from == State::Sprint,
            potential_height: self.position[1],
            move_dir: self.direction,
            // The jump moves the character in the frame it begins.
            lag_at_start: self.clock_lag_before,
            ..JumpData::default()
        };
        self.tilt = 0.0;
    }

    pub(super) fn update_jump(&mut self, dt: f32, world: &dyn World) {
        let s = &self.settings;
        let j = &mut self.jump;
        // The jump ends by telling the animator it has landed: either a
        // touchdown, where a held direction carries on into a walk or a
        // sprint, or a halt when the jump never got going or stuck.
        let (mut touchdown, mut halt) = (false, false);
        self.inert = length(j.move_dir) > 0.1;

        if j.halted {
        } else if !j.airborne {
            if j.time > 2.0 + j.lift_delay {
                halt = true;
            } else if !self.grounded && j.time > j.lift_delay {
                j.airborne = true;
            }
        } else {
            if j.time_descending > s.jump_descending_exit_time && self.grounded {
                touchdown = true;
            }
            if let Some(last) = j.last_height {
                if (self.position[1] - last).abs() < 0.0001 {
                    j.frames_without_motion += 1;
                    if j.frames_without_motion > 3 {
                        halt = true;
                    }
                } else {
                    j.frames_without_motion = 0;
                }
            }
        }
        if self.grounded {
            self.freefall_time = dt;
            j.last_height = None;
        } else {
            self.freefall_time += dt;
            j.last_height = Some(self.position[1]);
        }
        j.time += dt;

        let turned_from = self.body_yaw;
        self.body_yaw = lerp_turn(self.body_yaw, self.yaw, s.rotation_lerp_speed * dt);
        j.turn_rate = delta_angle(turned_from, self.body_yaw) / dt;
        (self.hands_to_body, self.aim_angle) = (body_to_view(self.body_yaw, self.yaw), self.pitch);

        // Vertical: nothing until the push-off delay has passed, then a
        // launch velocity that gravity eats into.
        let since_lift = j.time - j.lift_delay;
        j.lift_velocity = if j.time < j.lift_delay { 0.0 } else { j.lift * s.jump_boost + s.gravity * since_lift };
        if self.position[1] < j.potential_height {
            j.lift_velocity += (j.potential_height - self.position[1]) / dt * 0.9;
        }

        // Horizontal: the direction is fixed at take-off. Input only decides
        // how much of the take-off speed is kept.
        let wish = if is_zero(j.move_dir) {
            [0.0; 2]
        } else {
            let unit = normalize(j.move_dir);
            let yaw = self.body_yaw.to_radians();
            [unit[0] * yaw.cos() + unit[1] * yaw.sin(), -unit[0] * yaw.sin() + unit[1] * yaw.cos()]
        };
        let along = j.direction[0] * wish[0] + j.direction[2] * wish[1];
        let target = if along < -0.966 {
            j.initial_speed * s.air_control_back
        } else if along > 0.966 {
            s.air_min_speed.max(j.initial_speed * s.air_control_same)
        } else {
            j.initial_speed * s.air_control_other
        };
        if j.airborne {
            j.speed = lerp(j.speed, target, dt * s.air_lerp);
        }

        let motion = [j.direction[0] * j.speed * dt, j.lift_velocity * dt, j.direction[2] * j.speed * dt];
        let height_before = self.position[1];
        j.potential_height = self.position[1] + j.lift_velocity * dt;
        let descending_by_velocity = j.lift_velocity < 0.0;
        // In the air the head is kept out of walls too (`JumpPlayerState`).
        let motion = self.limit_motion_xz(motion, false, world);
        self.move_by(motion, dt, world);
        if self.position[1] - height_before < 0.001 || descending_by_velocity {
            self.jump.time_descending += dt;
        }

        if touchdown {
            self.jump_flag = false;
            self.landing_flag = true;
            let move_dir = self.jump.move_dir;
            if dot(move_dir, move_dir) > 0.1 && self.conditions.can_walk() {
                self.enable_sprint(self.jump.from_sprint && move_dir[1] > 0.1);
                self.sprint_flag = self.sprinting;
                self.movement_direction = move_dir;
                self.inert = true;
            } else {
                self.inert = false;
            }
        }
        if halt {
            self.jump_flag = false;
            self.landing_flag = true;
            self.jump.halted = true;
        }
    }

    /// Entering a vault or a climb (vaulting spec, "The moves as
    /// movement states").
    pub(super) fn enter_vault(&mut self, kind: Move, from: State) {
        /// The capsule's standing height, which both moves start from.
        const STANDING: f32 = 1.6;
        let s = self.settings.vaulting;
        let clear = s.of(kind).max_without_hand;
        let height = self.vault_figures[0];
        self.is_vaulting = true;
        self.blocked = false;
        // A vault takes the hand over that height; a climb at it.
        let hands = if kind == Move::Vault { height > clear } else { height >= clear };
        let capsule = match kind {
            // Shrunk from below at once, to pass over the top.
            Move::Vault => [STANDING - height * 1.1, height + (STANDING - height) / 1.5],
            // A climb's shrinks as it rises, from the standing one, whose
            // centre is half its height up: the player's controller is the
            // game's own, which has no skin (recorded: 1.6 and 0.8).
            Move::Climb => [STANDING, STANDING / 2.0],
        };
        self.vault = VaultData { height, from_sprint: from == State::Sprint, hands, capsule };
        if height >= s.vault.max_without_hand.min(s.climb.max_without_hand) {
            self.tilt = 0.0;
        }
        if kind == Move::Vault && self.vault.from_sprint {
            // The sprint was switched off on the way out of its state; the
            // animator is told as much, and that the player is moving.
            self.sprint_flag = self.sprinting;
            self.inert = true;
        }
        // The cost, and the pace of the clips: standing by the walk's
        // overweight, crouched by the general one.
        self.stamina.set_pose(self.prone, self.pose_level);
        let (now, st) = (self.frame_clock, &self.settings.stamina);
        let overweight = if self.pose_level < 0.11 { st.overweight } else { st.walk_overweight };
        // A damaged limb takes the move at the slow end of its range, as
        // full overweight does (`SetVaultingSpeed`).
        let slowed = self.conditions.vault_slowing(kind == Move::Climb, height, clear, s.climb.max_one_hand);
        match kind {
            Move::Vault => self.stamina.vault(height > clear, now),
            Move::Climb => self.stamina.climb(height > clear, height > s.climb.max_one_hand, now),
        }
        self.vaulting_speed = s.pace(kind, overweight.max(slowed));
        if kind == Move::Vault {
            self.moved.push(Moved::Vaulted(height));
        }
        if hands {
            // The sights go down and the view is the move's own.
            self.aim_taken = true;
            self.turn_limit = 0.0;
        }
    }

    /// A vault or a climb: the animation's own motion as it comes. No
    /// gravity and no limit; only, on the ground, nothing downward.
    pub(super) fn update_vault(&mut self, dt: f32, world: &dyn World) {
        self.turn_body(dt);
        let mut motion = self.animated_motion;
        if self.grounded {
            motion[1] = motion[1].max(0.0);
        }
        self.travel(motion, dt, world, false);
        if self.state == State::Climb && motion[1] > 0.0 {
            // The capsule's underside rises as fast again as the feet.
            self.vault.capsule[0] -= motion[1];
            self.vault.capsule[1] += motion[1] / 2.0;
        }
        self.body_yaw += self.body.turn();
        self.note_rotation();
    }

    /// Coming down behind a vault: any state's motion, with gravity.
    pub(super) fn update_vault_fall(&mut self, dt: f32, world: &dyn World) {
        self.turn_body(dt);
        let motion = self.animated_motion;
        self.apply_motion(motion, dt, world);
        self.body_yaw += self.body.turn();
        self.note_rotation();
    }

    /// Once a frame, before the animator: how fast the character really
    /// went over the frame before, and whether the lock on moving into an
    /// obstacle comes off (movement spec, "Stopped by what stands in the
    /// way"). The game measures from one frame's place to the next over
    /// the engine's frame time.
    pub(super) fn watch_obstacle(&mut self, dt: f32) {
        /// The lock comes off past this many degrees, of the movement
        /// direction from the one it was put on with or of the view from
        /// the body.
        const TURNED: f32 = 5.0;
        let gone: [f32; 3] = std::array::from_fn(|k| self.position[k] - self.place_before[k]);
        self.place_before = self.position;
        if dt > 0.0 {
            self.real_speed = (gone[0] * gone[0] + gone[1] * gone[1] + gone[2] * gone[2]).sqrt() / dt;
        }
        if self.blocked {
            let (was, now) = (self.blocked_direction, self.movement_direction);
            let lengths = (length(was) * length(now)).max(1e-15);
            // Against no direction at all the engine's angle is nothing.
            let between = if length(was) * length(now) < 1e-15 {
                0.0
            } else {
                ((was[0] * now[0] + was[1] * now[1]) / lengths).clamp(-1.0, 1.0).acos().to_degrees()
            };
            if between > TURNED || self.hands_to_body.abs() > TURNED {
                self.blocked = false;
            }
        }
    }

    /// The walking and the sprinting state's look at whether the character
    /// is getting anywhere: slower than `threshold` with the movement
    /// direction all but full, for more than a tenth of a second of such
    /// looks, puts the lock on. Not looked at while the body's animator is
    /// between two states.
    pub(super) fn check_obstacle(&mut self) {
        /// How long the character has to have been that slow, in seconds,
        /// and how long its movement direction has to be.
        const WAIT: f32 = 0.1;
        const PUSHING: f32 = 0.9;
        if self.body.in_transition() {
            return;
        }
        let threshold = if self.state == State::Sprint { 0.5 } else { 0.2 };
        let slow = self.real_speed < threshold;
        let waited = self.slow_time > WAIT;
        self.slow_time = if slow { self.slow_time + self.frame_dt } else { 0.0 };
        if !self.blocked && slow && waited && length(self.movement_direction) > PUSHING {
            self.blocked = true;
            self.blocked_direction = self.movement_direction;
        }
    }

    /// What the choice of a move goes by besides the obstacle.
    fn standing(&self) -> Standing {
        let s = &self.settings.vaulting;
        let height = self.vaulting.obstacle.height;
        Standing {
            vaulting: self.is_vaulting,
            backing: self.movement_direction[1] < 0.0,
            grounded: self.grounded,
            in_transition: self.body.in_transition(),
            jumping: self.jump_flag,
            sprinting: self.sprinting,
            can_vault: self.stamina.can_vault(height > s.vault.max_without_hand),
            can_climb: self.stamina.can_vault(height > s.climb.max_without_hand),
        }
    }

    /// The vault key's work for one frame it is held
    /// (`MovementContext.TryVaulting`; vaulting spec, "What a try does").
    pub(super) fn try_vaulting(&mut self, input: &Input, world: &dyn World) {
        let s = self.settings.vaulting;
        // The game's restrictions go by the move the obstacle as last
        // measured would get: not lying down, and with the weapon busy
        // only what leaves the hands on it.
        let stale = self.vaulting.choose(&s, &self.standing());
        let high = stale.is_some_and(|kind| self.vaulting.obstacle.height >= s.of(kind).max_without_hand);
        // Nor with blind fire's figure set (`CheckBlindFireCondition`).
        let hands_held = input.hands_limit_vault && (stale.is_none() || high);
        if self.prone || self.blind_fire != 0 || (input.weapon_occupied && high) || hands_held || self.input_direction[1] <= 0.0 {
            return;
        }
        // How fast the character goes along the view's heading; nothing if
        // it goes against it.
        let (sin, cos) = self.yaw.to_radians().sin_cos();
        let speed = (self.velocity[0] * sin + self.velocity[2] * cos).max(0.0);
        match self.vault_given.take() {
            Some((obstacle, speed)) => self.vaulting.take(&s, obstacle, speed),
            None => self.vaulting.measure(&s, world, self.position, self.body_yaw, speed, self.sprinting),
        }
        let Some(kind) = self.vaulting.choose(&s, &self.standing()) else { return };
        let o = self.vaulting.obstacle;
        self.vault_figures = [o.height, o.length, o.behind, self.vaulting.forward_speed];
        match kind {
            Move::Vault => self.do_vault = true,
            Move::Climb => self.do_climb = true,
        }
    }

    /// A mark of a move over an obstacle (`FirstPersonVaultingAudioController`,
    /// `kind` the move by `body::VAULT_STATES`, `mark` 0 to 3: start, grab,
    /// slide, end): at the start the obstacle's surface is found (a cast
    /// 0.1 m down onto the heaviest point, 0.6 m at most: a line here for
    /// the game's sphere of 0.1 m, `PROVISIONAL`) and the terrain's (from
    /// the capsule's centre down its height and half a metre more); the
    /// start and the end play the terrain's surface set's element, the
    /// grab and the slide the obstacle's; the grab and the end play the
    /// move's kit bank first, and the end finds the terrain's surface
    /// again before it.
    pub(super) fn vault_mark(&mut self, kind: usize, mark: usize, world: &dyn World) {
        // The height as the animator holds it (`VaultingHeight`), and the
        // capsule as the game's controller stands just now.
        let height = self.vault_figures[0];
        let terrain = |m: &Self| {
            let [x, y, z] = m.position;
            let [capsule_height, centre_up] = m.capsule_figures();
            let centre = [x, y + centre_up, z];
            world.surface(centre, [x, centre[1] - (capsule_height + 0.5), z])
        };
        match mark {
            0 => {
                if let Some(found) = self.vaulting.heaviest_point().and_then(|[x, y, z]| world.surface([x, y + 0.1, z], [x, y - 0.5, z])) {
                    self.vault_surfaces[0] = found;
                }
                if let Some(found) = terrain(self) {
                    self.vault_surfaces[1] = found;
                }
                self.sounds.vault_element(kind, mark, self.vault_surfaces[1], height);
            }
            1 => {
                self.sounds.vault_gear(kind, height);
                self.sounds.vault_element(kind, mark, self.vault_surfaces[0], height);
            }
            2 => self.sounds.vault_element(kind, mark, self.vault_surfaces[0], height),
            _ => {
                if let Some(found) = terrain(self) {
                    self.vault_surfaces[1] = found;
                }
                self.sounds.vault_gear(kind, height);
                self.sounds.vault_element(kind, mark, self.vault_surfaces[1], height);
            }
        }
    }
}
