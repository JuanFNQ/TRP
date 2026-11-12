//! Entering each movement state and turning toward the look.

use super::*;

impl Movement {
    /// Leaves the current movement state for another, as the game does
    /// when its animator enters a state that names a different one.
    /// `entered` is that animator state's own word on which part of the
    /// movement it is and for which side.
    pub(super) fn enter_state(&mut self, state: State, entered: Entered, world: &dyn World) {
        // Each of the animator's states says how freely the view turns in
        // it, and says so again every time it is entered.
        (self.turn_factor, self.turn_limit) = (entered.turn_factor, entered.turn_limit);
        self.can_interact = entered.can_interact;
        if state == self.state {
            // Another of the animator's states for the same movement
            // state: the game only tells its state object so. The
            // sidestep's starts its run over, for the new state's side.
            if state == State::Sidestep {
                self.sidestep_data.part = entered.part;
                self.sidestep_data.progress = 0.0;
                self.sidestep_data.far = if entered.side < 0.0 { -1.0 } else { 1.0 };
            }
            // The roll's only has to know which part it is in.
            if state == State::Roll {
                self.roll_data.part = entered.part;
            }
            // Blind fire's shared state takes the new state's part, side
            // and curve, and nothing else (`BlindeFireState.ReEnter`).
            if state == State::BlindFire {
                (self.blind_part, self.blind_side) = (entered.part, blind_side(entered.side));
            }
            return;
        }
        // Leaving. Standing and the states built on it let a held breath
        // go on the way out, if the sights are up to hold one by.
        if self.state.stands() && self.aiming {
            self.stamina.hold_breath(false);
        }
        // The head's capsule: the standing one again on leaving the crawl,
        // the roll or getting up, the lying one on entering going down,
        // lying still, the crawl or the roll (`SetPOMCollider`).
        if matches!(self.state, State::ProneMove | State::Roll | State::FromProne) {
            self.head_lying = false;
        }
        if matches!(state, State::ToProne | State::ProneIdle | State::ProneMove | State::Roll) {
            self.head_lying = true;
        }
        match self.state {
            State::Sidestep => {
                // The view turns freely again and the step is no longer
                // asked for. A way in or out leaves the body unstepped;
                // the hold leaves it as it stands.
                self.yaw_limit = None;
                self.step = 0;
                if self.sidestep_data.part != part::HOLD {
                    self.sidestep = 0.0;
                }
            }
            State::Roll => {
                // Rolled back: the view is as free as lying still leaves
                // it, at once, and the roll is no longer asked for.
                self.yaw_limit = None;
                self.pitch_limit = PRONE_PITCH_RANGE;
                self.pitch_target = PRONE_PITCH_RANGE;
                self.step = 0;
            }
            State::Sprint => {
                // A sprint's distance pushes the wounds (`DoWoundRelapse`),
                // from the ground.
                if self.grounded {
                    self.moved.push(Moved::SprintEnded(self.sprint_distance));
                }
                self.enable_sprint(false);
                self.sprint_flag = false;
                self.slide = false;
                self.reset_speed_after_sprint();
                // A jump out of a sprint keeps the sprint's pace for the
                // landing; anything else puts it back.
                if state != State::Jump {
                    self.sprint_speed = self.settings.starting_sprint_speed;
                }
            }
            State::Jump => {
                // Lying down was asked for in the air: only if the
                // character may lie (`JumpPlayerState.Exit`).
                if self.prone && !(self.conditions.can_prone() && self.room_to_lie(world)) {
                    self.prone = false;
                }
                self.moved.push(Moved::JumpEnded);
                self.tilt = 0.0;
                // A landing caps the selected speed for a while.
                let s = &self.settings;
                let limit = ((1.0 - s.jump_penalty) * s.max_speed).max(0.0);
                self.limit_after_landing(limit, s.jump_penalty_duration);
            }
            State::Vault => {
                self.do_vault = false;
                // Out of a sprint, the sprint is asked for again and the
                // move is over at once; otherwise it is over when the
                // landing is left.
                if self.vault.from_sprint && !self.walk_refused() {
                    self.enable_sprint(true);
                    self.is_vaulting = false;
                }
            }
            State::Climb => {
                self.do_climb = false;
                self.is_vaulting = false;
            }
            State::VaultLanding => self.is_vaulting = false,
            // The approach is over: its flag goes down with it
            // (`ApproachState.Exit`).
            State::Approach => {
                if let Some(kick) = &mut self.kick {
                    kick.approached = false;
                }
            }
            // The kick is over (`BreachDoorState.Exit`, `StopAnyInteractions`).
            State::Breach => {
                self.kick = None;
                self.acted.push(Acted::Done);
            }
            // Out of blind fire to another state: the float is put back to
            // the state's start, 0, whichever part was playing
            // (`BlindeFireState.Exit`).
            State::BlindFire => self.blind_float = 0.0,
            State::FromProne => {
                // Up again: the view and the body's turning are free as
                // they were, at once.
                self.pitch_limit = UPRIGHT_PITCH_RANGE;
                self.pitch_target = UPRIGHT_PITCH_RANGE;
                self.yaw_limit = None;
                self.trunk_limit = self.settings.trunk_rotation_limit;
                self.stand_upright();
            }
            _ => {}
        }
        let from = self.state;
        self.left_stance_exit(from);
        self.set_state(state);
        self.left_stance_enter(state, entered.part);
        self.aim_lowered |= self.settings.no_aim_states.contains(&state.number());
        // Entering.
        match state {
            State::Idle => {
                self.enable_sprint(false);
                self.idle_sprint_held = self.sprint_input;
            }
            State::ToProne => {
                self.enable_sprint(false);
                self.trunk_limit = PRONE_TRUNK_LIMIT;
                self.pitch_target = PRONE_PITCH_RANGE;
            }
            State::ProneIdle => {
                self.pose_level = 0.0;
                self.smoothed_pose_level = 0.0;
                self.pitch_target = PRONE_PITCH_RANGE;
                self.pending_step = 0;
                // Lying down ends blind fire (`PronePlayerState.Enter`).
                self.end_blind_fire();
            }
            State::Roll => {
                self.enable_sprint(false);
                self.pose_level = 0.0;
                self.smoothed_pose_level = 0.0;
                // The view is held around where it points now, and may
                // pitch much less far down; the body turns to that bearing.
                self.roll_bearing = self.yaw;
                self.yaw_limit = Some((self.yaw, self.trunk_limit - 1.0));
                self.pitch_target = ROLL_PITCH_RANGE;
                self.roll_yaw = self.body_yaw;
                self.inert = false;
                let s = &self.settings;
                let data = &mut self.roll_data;
                data.part = entered.part;
                data.speed = s.sideback_speed * if data.stepped_out { s.inertia_backward_coef } else { 1.0 };
                self.sideback_speed = data.speed;
            }
            State::ProneMove => {
                std::mem::swap(&mut self.mv, &mut self.mv_prone);
                self.enter_move(from);
                std::mem::swap(&mut self.mv, &mut self.mv_prone);
                self.yaw_limit = None;
                self.pose_level = 0.0;
                self.smoothed_pose_level = 0.0;
            }
            State::FromProne => self.enable_sprint(false),
            State::Sidestep => {
                self.enable_sprint(false);
                // The view may turn only so far from the way the body
                // faces, as the body's angle to it was last noted.
                let reach = self.trunk_limit - 1.0;
                self.yaw_limit = Some((self.yaw - self.hands_to_body, reach));
                self.pitch_limit = UPRIGHT_PITCH_RANGE;
                self.pitch_target = UPRIGHT_PITCH_RANGE;
                self.tilt = 0.0;
                let far = if entered.side < 0.0 { -1.0 } else { 1.0 };
                let data = &mut self.sidestep_data;
                (data.part, data.progress, data.far) = (entered.part, 0.0, far);
                self.sidestep = if entered.part == part::IN { 0.0 } else { far };
                // The state's own playback speed goes to where it rests,
                // which is lower if it last worked a way out.
                let s = &self.settings;
                data.speed = s.sideback_speed * if data.stepped_out { s.inertia_backward_coef } else { 1.0 };
                self.sideback_speed = data.speed;
            }
            State::Move => self.enter_move(from),
            State::Sprint => {
                self.sprint_distance = 0.0;
                self.moved.push(Moved::SprintStarted);
                // The sprint is a state object of its own in the game, so
                // what the walking state holds stays as the walk left it.
                self.mv.smoothed_yaw = self.body_yaw;
                self.direction = self.movement_direction;
                self.pose_level = 1.0;
                self.tilt = 0.0;
                // The stop's blend by the load, as the sprint begins.
                self.sprint_inertia = self.settings.sprint_brake_inertia;
                // An overweight sprint turns slower still.
                self.turn_factor *= self.settings.sprint_sensitivity;
                self.turn_limit *= self.settings.sprint_sensitivity;
            }
            State::Jump => {
                self.moved.push(Moved::JumpStarted);
                self.enter_jump(from);
            }
            State::SprintStop => {
                self.sprint_flag = false;
                self.tilt = 0.0;
                self.mv.smoothed_yaw = self.body_yaw;
                self.turn_factor *= self.settings.sprint_sensitivity;
                self.turn_limit *= self.settings.sprint_sensitivity;
            }
            State::Vault => self.enter_vault(Move::Vault, from),
            State::Climb => self.enter_vault(Move::Climb, from),
            State::VaultFall => {}
            // The approach lets the lean go and takes the character out of
            // its lying pose (`ApproachState.Enter`), and ends blind fire.
            State::Approach => {
                self.tilt = 0.0;
                self.prone = false;
                self.enable_sprint(false);
                self.end_blind_fire();
                self.begin_approach();
            }
            // Blind fire's state from outside it: standing's entry (the
            // sprint off), then its own: the lean let go, and the float at
            // the state's start for a way in or at its side for a way out
            // (`BlindeFireState.Enter`).
            State::BlindFire => {
                self.enable_sprint(false);
                self.tilt = 0.0;
                (self.blind_part, self.blind_side) = (entered.part, blind_side(entered.side));
                self.blind_float = if entered.part == part::OUT { self.blind_side } else { 0.0 };
            }
            State::Breach => {}
            State::Landing | State::VaultLanding => {
                self.landing_idle_time = 0.0;
                self.landing_still = false;
                // The landing has been handed no input yet on the frame
                // it starts, so that frame counts as one with none.
                self.direction = [0.0; 2];
            }
        }
        // Every change of state sets the cap on actual speed for the new
        // state and puts the selected speed back to the wheel's setting,
        // which the wind-up before a sprint had raised.
        self.speed_cap = self.hard_speed_cap(state);
        self.select_speed(self.relative_speed * self.settings.max_speed);
    }

    // ------------------------------------------------------------ input

    /// What the movement state lets through of a frame's turn of the view
    /// ("Looking" in the movement spec): scaled, then cut to a length.
    pub(super) fn limited_turn(&self, delta: [f32; 2]) -> [f32; 2] {
        if self.turn_limit <= 0.0 {
            return [0.0; 2];
        }
        let turn = delta.map(|d| d * self.turn_factor);
        let size = length(turn);
        if size > self.turn_limit { turn.map(|d| d * self.turn_limit / size) } else { turn }
    }

    /// Where a turn of `delta` leaves the view: yaw, then pitch.
    pub(super) fn turned(&self, delta: [f32; 2]) -> [f32; 2] {
        let mut yaw = self.yaw + delta[0];
        // Lying still, the view turns only so far from where the body lay
        // a frame ago; stepped aside, only so far from where the body
        // faced when the step began.
        if let Some((centre, reach)) = self.yaw_limit {
            yaw = centre + delta_angle(centre, yaw).clamp(-reach, reach);
        }
        // The range is measured from the ground the body is laid to.
        let range = self.pitch_limit.map(|limit| limit + self.surface_pitch);
        [yaw, (self.pitch + delta[1]).clamp(range[0], range[1])]
    }

    /// Where the view will point once the next frame has turned it by
    /// `delta`, yaw then pitch, if the movement state stays as it is.
    ///
    /// A host that draws more often than the simulation's frames come
    /// shows this for the mouse's travel no frame has had yet, so that the
    /// view turns at once. It has to go through the state's factor and
    /// limits as the frame's turn will: shown whole, the travel would be
    /// taken back when a frame in a sprint, a jump or lying down lets only
    /// part of it through, and the view would shake. Mounted, the frame's
    /// turn is the mount's (`mount_look_ahead`): its lower limit goes by how
    /// deep the weapon last stood in what it rests on (`overlap_depth`, as
    /// the next frame's input will give it), and without it the view shook
    /// against the sill while the stance followed the pitch.
    pub fn look_ahead(&self, delta: [f32; 2], overlap_depth: f32) -> [f32; 2] {
        let [yaw, pitch] = self.mount_look_ahead(delta, overlap_depth).unwrap_or_else(|| self.turned(self.limited_turn(delta)));
        [yaw, self.pitch_as_shown(pitch)]
    }

    pub(super) fn look(&mut self, delta: [f32; 2]) {
        [self.yaw, self.pitch] = self.turned(delta);
        self.yaw_deltas.push_back(delta[0]);
        while self.yaw_deltas.len() > self.settings.rotation_frame_span {
            self.yaw_deltas.pop_front();
        }
    }
}
