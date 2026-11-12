//! The update: state changes, landings, idling and turns.

use super::*;

impl StepSounds {
    /// The rules for one step of the movement. `last_step` is the time of
    /// the last footstep (`_lastStepTime`), which a footstep played on
    /// leaving `Run` and a crawl's mark set too; `surface` casts the line
    /// down.
    pub fn update(&mut self, m: &Moment, last_step: &mut f32, surface: &mut dyn FnMut() -> Option<u8>) {
        // How quietly one walks, worked out again when the clamped speed has
        // moved a hundredth since (`UpdateCovertEfficiency`). Another's is
        // asked every frame (`ObservedPlayerAudioController.ManualUpdate`);
        // one's own only where the movement's speed is set
        // (`MovementContext.SetCharacterMovementSpeed`), so a state's limit
        // coming or going (aiming, the ground, a turn) leaves it as it was:
        // a slow walk's quiet carried on after its limit is gone
        // (`courses2`'s `7-sound-moving`).
        if self.efficiency_due || ((self.third_person || m.speed_set) && (m.clamped_speed - self.efficiency_speed).abs() >= 0.01) {
            let share = if m.max_speed > 0.0 { m.clamped_speed / m.max_speed } else { 0.0 };
            self.efficiency = self.covert.efficiency(share, m.leg_damaged, m.walk_overweight);
            (self.efficiency_speed, self.efficiency_due) = (m.clamped_speed, false);
        }
        // The view's turn on this step (`UpdateRotationSpeed`).
        if m.dt > 0.0 {
            if let Some(last) = self.last_yaw {
                self.turn_at = (self.turn_at + 1) % self.turns.len();
                self.turns[self.turn_at] = delta_angle(last, m.yaw) / m.dt;
                self.turn_count = (self.turn_count + 1).min(self.turns.len());
            }
            self.last_yaw = Some(m.yaw);
        }
        // The ground found again (`PlayGroundedSound`).
        if let Some((fall, rise)) = m.landed {
            self.landing(m, fall, rise, surface);
        }
        if m.state != self.state {
            self.state_changed(self.state, m.state, m, last_step, surface);
            self.previous = self.state;
            self.state = m.state;
        }
        if m.footstep {
            match m.state {
                State::Move => {
                    self.played = true;
                    if self.in_range(RUN) {
                        self.check_surface(surface);
                        self.footstep(m);
                    }
                }
                State::Sprint => {
                    self.played = true;
                    if self.in_range(SPRINT) {
                        self.check_surface(surface);
                        self.sprint_step(m);
                    }
                }
                _ => {}
            }
        }
        for _ in 0..m.crawl_marks {
            if m.clock - *last_step >= CRAWL_GAP && self.in_range(PRONE) {
                self.check_surface(surface);
                let base = self.figures(PRONE).base_volume;
                let volume = (self.covert_volume() * m.clamped_speed * base).max(0.4) * self.kept_or_worked(Kind::Prone, m.overweight);
                let reach = self.work_out(Kind::Prone, None, m.overweight);
                let at = self.surface;
                self.ask(BankOf::Surface(at, PRONE), false, volume, Some(reach));
                *last_step = m.clock;
            }
        }
        if m.state == State::Idle {
            self.idle(m);
        }
        let mut due = std::mem::take(&mut self.gear_due);
        due.retain(|&(when, volume)| {
            if m.clock + 1e-6 >= when {
                self.gear(volume, false, m.overweight);
                false
            } else {
                true
            }
        });
        self.gear_due.extend(due);
    }

    /// `StateChangedHandler`: what leaving one state and entering the next
    /// plays.
    fn state_changed(&mut self, from: State, to: State, m: &Moment, last_step: &mut f32, surface: &mut dyn FnMut() -> Option<u8>) {
        match from {
            State::Idle => self.turn_wait = None,
            State::Move => {
                if !self.played && m.clock - *last_step > RUN_LEFT_QUIET {
                    if self.in_range(RUN) {
                        self.check_surface(surface);
                        self.footstep(m);
                    }
                    *last_step = m.clock;
                }
            }
            State::Sprint => {
                if !self.played && self.in_range(SPRINT) {
                    self.check_surface(surface);
                    self.default_play(SPRINT, 1.0, Kind::Sprint, m.overweight);
                }
                if matches!(to, State::SprintStop | State::Idle) {
                    let base = if self.third_person { 1.0 } else { self.figures(STOP).base_volume };
                    let covert = self.covert_volume();
                    self.default_play(STOP, base * covert, Kind::Stop, m.overweight);
                }
            }
            _ => {}
        }
        match to {
            State::FromProne => self.gear(0.7, true, m.overweight),
            State::Sprint | State::Move => self.played = false,
            State::Jump => {
                self.default_play(JUMP, 1.0, Kind::Jump, m.overweight);
                let noise = self.equipment_noise(m.overweight);
                self.gear(noise, true, m.overweight);
            }
            State::ToProne => {
                let volume = 0.7 * self.covert_volume();
                if from == State::Sprint {
                    self.default_play(PRONE_DROP, volume, Kind::Drop, m.overweight);
                } else {
                    self.gear(volume, true, m.overweight);
                }
            }
            _ => {}
        }
    }

    /// `PlayGroundedSound`: the larger of the drop and the rise (the drop
    /// alone after a climb up), heard over 0.3 m, not within 0.5 s of the
    /// last, nor out of a climb or a vault's landing. The ground is found
    /// before the animator changes the state on the step (`CheckFlying` in
    /// `ManualUpdate`), so the states are those seen before it.
    fn landing(&mut self, m: &Moment, fall: f32, rise: f32, surface: &mut dyn FnMut() -> Option<u8>) {
        if m.clock < self.next_landing {
            return;
        }
        let (now, before) = (self.state, self.previous);
        let quiet = !matches!(now, State::Move | State::Sprint) && (before == State::Climb || now == State::Climb || before == State::VaultLanding);
        if quiet {
            return;
        }
        let height = if before == State::Climb { fall } else { fall.max(rise) };
        if height > LANDING_THRESHOLD && self.in_range(LANDING) {
            self.check_surface(surface);
            let volume = inverse_lerp(0.1, LANDING_THRESHOLD * LANDING_WHOLE, height);
            self.default_play(LANDING, volume, Kind::Land, m.overweight);
            self.next_landing = m.clock + 0.5;
        }
    }

    /// `Idle`: a turn of the hands past the angle is heard, and the check
    /// waits.
    pub(super) fn idle(&mut self, m: &Moment) {
        if let Some((until, skip)) = self.turn_wait {
            if m.clock + 1e-6 < until {
                return;
            }
            if skip {
                self.turn_wait = Some((until, false));
                return;
            }
            self.turn_wait = None;
        }
        if m.hands_to_body.abs() > self.settings.turn_angle {
            self.turn(m);
            self.turn_wait = Some((m.clock + self.settings.turn_delay, true));
        }
    }

    /// `PlayTurnSound`.
    pub(super) fn turn(&mut self, m: &Moment) {
        if m.clock - self.last_turn < TURN_AGAIN {
            return;
        }
        // The average of the last turns, signed (`RollingMedian.Avarage`):
        // a turn to the left is below 0 and silent.
        let average = if self.turn_count == 0 { 0.0 } else { self.turns.iter().sum::<f32>() / self.turn_count as f32 };
        let share = inverse_lerp(1.0, TURN_FULL + (1.0 - m.pose_level) * TURN_FULL, average.max(0.0));
        let volume = share * self.covert_volume() * self.figures(TURN).base_volume;
        self.default_play(TURN, volume, Kind::Turn, m.overweight);
        self.last_turn = m.clock;
        if share > 0.4 {
            self.gear(volume, false, m.overweight);
        }
    }
}
