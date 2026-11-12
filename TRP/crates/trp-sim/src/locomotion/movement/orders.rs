//! The step, sprint, prone and pose commands, and approaching a spot.

use super::*;

impl Movement {
    /// The step-aside command as the state in hand takes it: a mounted
    /// state starts its exit and passes it on (`IdleWeaponMountingState.SetStep`).
    pub(super) fn step_command(&mut self, step: i32, world: &dyn World) {
        self.start_leaving_mount();
        self.set_step(step, world);
    }

    /// Asks for a step aside (1 right, -1 left) or takes it back (0), as
    /// the movement state in hand allows. Standing still takes it and the
    /// animator steps; stepped aside it is taken unless the step back is
    /// already under way. The walk, the sprint, the crawl and a jump do
    /// not listen. Lying still it is the roll: the lean is put to zero and
    /// the step waits for it to straighten, unless it already leaned that
    /// way; rolled over it is taken unless the roll back is under way. A
    /// step with something in the way or no steady ground to that side,
    /// and a roll with no room, ask for no step at all (movement spec,
    /// "Room to move").
    pub(super) fn set_step(&mut self, step: i32, world: &dyn World) {
        let stepped = |m: &Self| if step != 0 && !m.room_to_step(step, world) { 0 } else { step };
        match self.state {
            // Blind fire's state takes it as standing does; its animator
            // states lead to no step, which waits for standing.
            State::Idle | State::BlindFire => self.step = stepped(self),
            State::Sidestep if self.sidestep_data.part != part::OUT => self.step = stepped(self),
            State::ProneIdle if step != 0 && self.room_to_roll(step, world) => {
                if (step < 0 && self.tilt < 0.0) || (step > 0 && self.tilt > 0.0) {
                    self.step = step;
                } else {
                    self.pending_step = step;
                }
                self.tilt = 0.0;
            }
            // Taking it back does not take back a step still waiting.
            State::ProneIdle => self.step = 0,
            State::Roll if self.roll_data.part != part::OUT => {
                self.step = if step != 0 && self.room_to_roll(step, world) { step } else { 0 };
            }
            _ => {}
        }
    }

    /// Standing still, a movement key only tells the animator to start
    /// walking; the walk itself begins when the animator's walking state does.
    pub(super) fn idle_move(&mut self, d: [f32; 2]) {
        // Lying, the keys are not heard at all where the character may not
        // crawl (`PronePlayerState.Move`).
        if self.state == State::ProneIdle && !self.conditions.can_crawl() {
            return;
        }
        if d[0].abs() <= MIN_INPUT && d[1].abs() <= MIN_INPUT {
            self.movement_direction = [0.0; 2];
            return;
        }
        let signs = [if d[0] == 0.0 { 0.0 } else { d[0].signum() }, if d[1] == 0.0 { 0.0 } else { d[1].signum() }];
        self.movement_direction = [signs[0] * 0.01, signs[1] * 0.01];
        // Locked against an obstacle, nothing starts; nor with medicine at
        // work that cannot be used walking.
        if self.walk_refused() {
            return;
        }
        // The states built on standing each keep their own note of a held
        // sprint key, and only standing's is ever set.
        self.enable_sprint(self.state == State::Idle && self.idle_sprint_held && signs[1] > 0.1);
        if self.sprinting {
            self.pose_level = 1.0;
            if self.pose_level > SPRINT_POSE_THRESHOLD && self.smoothed_speed >= 1.0 {
                self.sprint_flag = true;
            }
        }
        self.inert = true;
    }

    pub(super) fn sprint_command(&mut self, enable: bool, is_toggle: bool) {
        match self.state {
            // Standing still, a toggle does nothing.
            State::Idle => {
                if !is_toggle {
                    self.idle_sprint_held = enable && self.conditions.can_sprint();
                }
            }
            // Health may forbid it (`MovementContext.CanSprint`).
            State::Move if !self.conditions.can_sprint() => {}
            State::Move => {
                if self.movement_direction[1] > 0.1 {
                    self.enable_sprint(enable);
                    // Only a sprint that started stands the character up.
                    if self.sprinting {
                        self.pose_level = 1.0;
                    } else {
                        self.reset_speed_after_sprint();
                    }
                } else if !is_toggle {
                    self.mv.sprint_postponed = enable;
                }
            }
            State::Sprint => self.enable_sprint(enable && self.conditions.can_sprint()),
            State::Jump => self.jump.from_sprint &= enable && self.conditions.can_sprint(),
            // Crawling, the key winds the speed setting up instead.
            State::ProneMove => self.prone_sprint = enable,
            State::SprintStop
            | State::Landing
            | State::ToProne
            | State::ProneIdle
            | State::FromProne
            | State::Sidestep
            | State::Roll
            | State::Vault
            | State::Climb
            | State::VaultFall
            | State::VaultLanding
            | State::Approach
            | State::Breach
            // Blind fire's own note of the key, as each state built on
            // standing keeps one, is never read.
            | State::BlindFire => {}
        }
    }

    /// The prone key: down from anything upright, up from lying. The
    /// player of an online raid refuses the change both ways with a
    /// reload in hand, and still sets the stance of a lying character to
    /// the one last chosen; the offline player takes it at any time.
    pub(super) fn prone_command(&mut self, world: &dyn World) {
        // The approach to a thing does not hear the key at all.
        if self.state == State::Approach {
            return;
        }
        let refused = self.reloading && self.settings.prone_key_refused_in_reload;
        if matches!(self.state, State::ProneIdle | State::ProneMove | State::Roll) {
            if !refused {
                // Getting up needs room to stand at the stance held lying,
                // or failing that room to crouch, which is then taken at
                // once; with neither the character lies on
                // (`IsInPronePose`'s setter, `CanStandAt`, `CanSit`).
                let room = self.can_stand_at(self.pose_level, world) || {
                    let sit = self.can_stand_at(0.0, world);
                    if sit {
                        self.pose_level = 0.0;
                        self.smoothed_pose_level = 0.0;
                    }
                    sit
                };
                if room {
                    // Getting up is paid for at the key.
                    if self.prone {
                        self.stamina.stand_up(self.frame_clock);
                    }
                    self.prone = false;
                }
            }
            // Then the stance last chosen, where there is room to stand
            // at it (`UpdatePoseAfterProne`).
            self.set_pose_level(self.pose_memo, world);
        } else if !refused && self.conditions.can_prone() && self.room_to_lie(world) {
            // The animator takes it from here, when it can. Health may
            // forbid lying down, and so may the room to lie in
            // (`MovementContext.CanProne`).
            self.prone = true;
        }
    }

    /// The stance set (`MovementContext.SetPoseLevel`): refused in a vault
    /// or a climb and where there is no room to stand at it, else clamped
    /// to the character's range. Whether it was taken.
    pub(super) fn set_pose_level(&mut self, level: f32, world: &dyn World) -> bool {
        if matches!(self.state, State::Vault | State::Climb) {
            return false;
        }
        if (self.pose_level - level).abs() < f32::EPSILON {
            return true;
        }
        if !self.can_stand_at(level, world) {
            return false;
        }
        self.pose_level = level.clamp(0.0, 1.0);
        true
    }

    /// The character turns to something to open it (`ApproachState`;
    /// inventory spec, "Opening a container"): a lean is let go, and a
    /// thing reached for more than 0.65 m under the ribcage is crouched
    /// to, the stance held before being kept for `let_go`. `height` is
    /// where the thing is reached for, in the world. Lying down nothing
    /// is approached. Not in this: the character's walk and turn to the
    /// place the thing is opened from, and the state of its own the game
    /// has for the while.
    pub fn approach(&mut self, height: f32) {
        /// How far under the ribcage a thing is crouched to.
        const LOWER: f32 = 0.65;
        if self.prone {
            return;
        }
        self.tilt = 0.0;
        if self.ribcage()[1] - height > LOWER && self.pose_level > 0.0 {
            self.approach_level = Some(self.pose_level);
            // Lower only, not lying: the room to stand is never asked.
            self.change_pose(-self.pose_level, &Flat);
        }
    }

    /// What was approached is let go: the stance held before it is taken
    /// again where there is room for it (`Player.TryInteractionCallback`).
    pub fn let_go(&mut self, world: &dyn World) {
        if let Some(level) = self.approach_level.take().filter(|_| !self.prone) {
            self.change_pose(level - self.pose_level, world);
        }
    }
}
