//! The step on the clock, search waits, work stamina and body height lag.

use super::*;

impl Movement {
    /// One step of `dt` seconds on a single clock. For a host that runs
    /// the simulation on steps of its own; one that draws frames calls
    /// `frame`.
    pub fn step(&mut self, dt: f32, input: &Input, world: &dyn World) {
        self.last_player_dt = dt;
        self.step_on_clock(dt, dt, input, world);
    }

    /// The last frame's time by the player's own clock, in whole
    /// milliseconds (none where it did not see a millisecond pass): what
    /// the game's health is updated by as well (health spec, "Compared with
    /// what": the recordings' effects end on it, not on the frames' time).
    pub fn player_dt(&self) -> f32 {
        self.last_player_dt
    }

    /// One drawn frame of `dt` seconds, timed as the game times it: the
    /// animator by the frame's own length and the movement by a stopwatch
    /// read in whole milliseconds. This is the stepping the recordings
    /// were compared on. A frame the stopwatch does not see pass (under a
    /// millisecond) is added to the next.
    pub fn frame(&mut self, dt: f32, input: &Input, world: &dyn World) {
        self.unstepped += dt;
        let player_dt = self.stopwatch.tick(dt);
        self.last_player_dt = player_dt;
        if player_dt > 0.0 {
            let dt = std::mem::take(&mut self.unstepped);
            self.step_on_clock(dt, player_dt, input, world);
        }
    }

    /// One frame that lasted `dt` seconds by the engine's time and
    /// `player_dt` by the player's own clock.
    ///
    /// The game keeps the two apart ("The player's clock" in the movement
    /// spec): its animator runs on the engine's time and the movement
    /// states on a stopwatch read in whole milliseconds, so the two differ
    /// by up to half a millisecond a frame. A host with one clock calls
    /// `step`.
    pub fn step_on_clock(&mut self, dt: f32, player_dt: f32, input: &Input, world: &dyn World) {
        // What health was to take of the last frame has been taken, or
        // nobody takes it.
        self.moved.clear();
        // The keys of a system switched off are not heard, and the
        // stamina is told which of its two parts run.
        let heard = self.systems.heard(input);
        let input = &heard;
        (self.stamina.drains, self.stamina.holds_back) = (self.systems.stamina, self.systems.exhaustion);
        if dt <= 0.0 || player_dt <= 0.0 {
            // The engine's timers still see the frame.
            if dt > 0.0 {
                self.wait_for_search(dt);
            }
            return;
        }
        self.frame_dt = dt;
        // The rig as the last frame left it turned, before this frame's view.
        self.rig_turn_before = self.rig_frame().1;
        // The engine's clock, which the landing's cap and the stamina's
        // pauses are timed by.
        self.frame_clock += dt;
        self.clock_lag_before = self.clock_lag;
        self.clock_lag += dt - player_dt;
        self.frame_count += 1;
        self.jump.turn_rate = 0.0;
        // With a screen open the view is handed no turn at all: not a
        // turn of nothing, which would count among the last frames' turns.
        // Nor while a thing is being taken up: that state does not turn.
        let picking_up = matches!(self.acting, Some(Acting::PickingUp { .. }));
        self.yaw_before_turn = self.yaw;
        // Mounted, the turn is the mount's own (`IdleWeaponMountingState.
        // Rotate`), whatever the movement state under it.
        if !input.screen && !picking_up {
            if self.mount.is_some() {
                self.mount_rotate(input.look_delta, input.overlap_depth, world);
            } else if input.free_look {
                self.look(input.look_delta);
            } else {
                let mut turn = self.limited_turn(input.look_delta);
                // Lying, a turn that would swing the body over an edge is
                // dropped (`PronePlayerState.Rotate`, `IsAbleToRotateProne`).
                if matches!(self.state, State::ProneIdle | State::ProneMove | State::Roll)
                    && !(self.head_turn_clear(turn[0], world) && self.able_to_turn_prone(turn[0], world))
                {
                    turn[0] = 0.0;
                }
                self.look(turn);
            }
        }
        // The head's free look is handed its axes on every frame the keys
        // reach the character, whatever the movement state makes of the
        // turn (`MoveInputTranslator.TranslateAxes`).
        if !input.screen {
            if input.head_reset {
                self.head_look.reset();
            }
            self.head_look.frame(&HeadLookInput {
                travel: input.head_look,
                dt,
                aiming: input.aiming,
                camera_pitch: input.camera_pitch,
                sprinting: self.sprinting,
            });
            self.body.set_head_turn(self.head_look.bones_turn());
        }
        self.hands_idle = input.hands_idle;
        self.act(player_dt, world);
        self.commands(input, world);
        self.watch_sprint();
        self.manual_update(player_dt, world);
        self.watch_obstacle(dt);
        // Stamina is worked here, before the animator: a movement state the
        // animator enters in this frame shows in the pools from the next.
        self.work_stamina(player_dt);
        // A mount's search answers after the frame's updates, before the
        // animator's (its batches of rays are awaited by `yield return null`).
        self.answer_mount_search(world);

        // The animator runs on what the movement set so far. Entering one
        // of its states enters the movement state it names, and this
        // happens when a blend into it starts, not when it ends.
        let entered = self.body.update(dt, &self.body_input());
        self.crawl_marks += self.body.take_crawl_marks();
        for entered in entered {
            if let Some(state) = State::from_number(entered.state) {
                self.enter_state(state, entered, world);
            }
        }
        for (kind, mark) in self.body.take_vault_marks() {
            self.vault_mark(kind, mark, world);
        }
        // A gesture whose state the animator has left in this update is
        // over with it: the state's own script says so as it is left, and
        // the flag is down in the same frame (recorded).
        for _ in self.body.take_finished_interactions() {
            let shows = self.interactions.finished();
            self.show(shows);
        }
        // The mark of a backpack's drop, passed by the gesture's state.
        // A drop whose gesture is gone without the mark is over too: the
        // game runs such an operation on to its end (`FastForward`).
        let passed = self.body.take_backpack_dropped();
        if self.dropping_backpack && (passed || !self.interactions.has(Interaction::DropBackpack)) {
            self.dropping_backpack = false;
            self.acted.push(Acted::BackpackDropped);
        }
        // So does the bipod's toggle, on its gesture's mark.
        let toggled = self.body.take_bipod_toggled();
        if let Some(gesture) = self.toggling_bipod {
            if toggled || !self.interactions.has(gesture) {
                self.toggling_bipod = None;
                self.acted.push(Acted::BipodToggled);
            }
        }
        // The animator sits on the part of the body that is laid to the
        // ground, so its motion comes out turned with it.
        let moved = quat_rotate(self.body_tilt, self.body.motion());
        let across = self.to_world([moved[0], moved[2]]);
        self.animated_motion = [across[0], moved[1], across[1]];
        // From a mount's approach until its state is off the animator's own
        // motion is not taken (`IgnoreDeltaMovement`).
        if self.ignore_root_motion {
            self.animated_motion = [0.0; 3];
        }

        // The current state's own handling of the frame. The animator calls
        // on a movement state once for each of its own states that names it
        // and had a part in the frame: not at all while none is playing,
        // and twice where one such state gives way to another.
        for _ in 0..self.body.calls(self.state.number()) {
            // Where the character stood before this update's motion: the game
            // schedules a state's motion and moves by the last one asked for
            // in the frame (`ApplyMotion`), so a mount's approach after the
            // state's update takes its place.
            let before = self.position;
            match self.state {
                State::Idle => self.update_idle(player_dt, world),
                State::Move => self.update_move(player_dt, world),
                State::Sprint => {
                    let from = self.position;
                    self.update_sprint(player_dt, world);
                    self.sprint_distance += ((self.position[0] - from[0]).powi(2) + (self.position[2] - from[2]).powi(2)).sqrt();
                    // Health may forbid it (`SprintPlayerState`, after the
                    // move: `CanSprint`).
                    if !self.conditions.can_sprint() {
                        self.sprint_flag = false;
                    }
                }
                State::Jump => self.update_jump(player_dt, world),
                State::SprintStop => self.update_sprint_stop(player_dt, world),
                State::Landing => self.update_landing(player_dt, world),
                State::ToProne => self.update_to_prone(player_dt, world),
                State::ProneIdle => self.update_prone_idle(player_dt, world),
                State::ProneMove => self.update_prone_move(player_dt, world),
                // Getting up is handled as standing is, once the body
                // has been let off the ground it was laid to.
                State::FromProne => {
                    self.rise_from_surface();
                    self.update_idle(player_dt, world);
                }
                State::Sidestep => self.update_sidestep(player_dt, world),
                State::Roll => self.update_roll(player_dt, world),
                State::Vault | State::Climb => self.update_vault(player_dt, world),
                State::VaultFall => self.update_vault_fall(player_dt, world),
                // The landing out of a vault is the jump's.
                State::VaultLanding => self.update_landing(player_dt, world),
                // Set before a door that puts the character nowhere, the
                // approach does nothing to the body at all.
                State::Approach => self.update_approach(player_dt, world),
                State::Breach => self.update_breach(player_dt, world),
                State::BlindFire => self.update_blind_fire(player_dt, world),
            }
            // The mount's state's frame comes after the movement state's.
            if self.mount.is_some() {
                self.update_mount(player_dt, world, before);
            }
        }
        self.watch_sprint();
        self.left_stance_frame();
        self.watch_footsteps(dt);
        self.hear(dt, world);
        self.lag_body_height(player_dt);
        self.wait_for_search(dt);
    }

    /// The game's wait after a search (a timer of the player's): it runs
    /// after the frame's update, so it goes by where the character is once
    /// the frame has moved it, and the frame of the key does not count. It
    /// is over on the first frame at least `thing_answers_after` after the
    /// key's (recorded: the screen open 0.550 to 0.553 s after the key).
    fn wait_for_search(&mut self, dt: f32) {
        let Some((at, from, time, fresh)) = self.searching else { return };
        let time = if fresh { time } else { time + dt };
        if time + 1e-6 < self.settings.thing_answers_after {
            self.searching = Some((at, from, time, false));
            return;
        }
        self.searching = None;
        let there = self.state != State::Jump && self.distance_to(at) - from <= SEARCH_REACH;
        self.acted.push(if there { Acted::Searched } else { Acted::LeftUnsearched });
    }

    /// The stamina's frame (stamina spec, "A pool, each frame"): what is
    /// running is paid for and the pools refill. It comes after the keys
    /// and the stance have been seen to and before the animator, so it
    /// goes by the movement state the last frame ended in and by the
    /// sights as the keys and a sprint begun by them leave them. Legs
    /// that run dry end a sprint, which the sprinting state then sees in
    /// this same frame.
    fn work_stamina(&mut self, player_dt: f32) {
        self.stamina.set_pose(self.prone, self.pose_level);
        self.stamina.moving(self.state == State::Move, self.state == State::ProneMove);
        self.stamina.aim(self.aiming);
        self.stamina.set_aim_drain_by(self.mount_drain());
        self.stamina.update(player_dt, self.frame_clock);
        if self.sprinting && !self.stamina.sprinting() {
            self.sprinting = false;
        }
    }

    /// The body does not follow the feet up and down at once: it is held
    /// back by what the feet just moved and eased home, never more than
    /// 0.2 m away. Going up it trails below, coming down it rides above.
    /// Lying down nothing is held back, and what is left eases home.
    ///
    /// `dt` is the player's own frame time, the whole milliseconds the
    /// movement states go by, not the engine's (body animator spec, "A
    /// lag in the body's height"; recorded on a frame where the two were
    /// 75 and 8 ms).
    fn lag_body_height(&mut self, dt: f32) {
        /// The share of the catching-up time used while the gap is still
        /// opening.
        const OPENING_SHARE: f32 = 0.3;
        const LIMIT: f32 = 0.2;
        let moved = if self.prone { 0.0 } else { self.position[1] - self.height_before };
        self.height_before = self.position[1];
        let eased = smooth_damp(
            [self.body_height_offset - moved],
            [0.0],
            &mut self.height_lag_velocity,
            self.height_lag_time,
            1_000_000.0,
            dt,
        )[0];
        self.body_height_offset = eased.clamp(-LIMIT, LIMIT);
        let wanted = if eased.abs() > self.height_lag_before { HEIGHT_SMOOTH_TIME * OPENING_SHARE } else { HEIGHT_SMOOTH_TIME };
        self.height_lag_time = lerp(self.height_lag_time, wanted, dt);
        self.height_lag_before = eased.abs();
    }
}
