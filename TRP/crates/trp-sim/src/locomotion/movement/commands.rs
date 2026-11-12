//! Reading the commands each step: hands, tilt and what was heard.

use super::*;

impl Movement {
    /// New hands are ready, their draw over: the player's owner tells them
    /// the screen's state again (`GamePlayerOwner.SetHandsController`,
    /// `Player.SetInventoryOpened`), and hands that pass the screen on
    /// (`Input::inventory_unheard`) set the body's flag for a reload by it
    /// (`OnInventory`): empty hands made with the screen shut clear what an
    /// item used left up.
    pub fn hands_ready(&mut self, passes_screen_on: bool) {
        if passes_screen_on {
            self.prone_reload = self.inventory;
        }
    }

    /// A reload begun or over, told as the game's reload operation tells
    /// the movement state, in the hands' update (`BaseReloadOperation`'s
    /// state, `OnReload`): the body's flag set now rather than at the next
    /// step, which sets it the same (the body's animator reads it at its
    /// next update either way).
    pub fn hands_told(&mut self, reloading: bool) {
        if reloading != self.reloading {
            self.prone_reload = reloading;
        }
    }

    /// The body's flag for a reload as it stands (`ProneReload`).
    pub fn prone_reload(&self) -> bool {
        self.prone_reload
    }

    pub(super) fn commands(&mut self, input: &Input, world: &dyn World) {
        // The sights coming up set the cap with the weapon's mastering
        // as it stands.
        if input.aiming && !self.aiming {
            self.aiming_cap = AIMING_SPEED_LIMIT + self.aim_walk_bonus;
        }
        self.aiming = input.aiming;
        self.aim_kept_from_body = input.aim_kept_from_body;
        // The body's flag for a reload is set at a reload's start and end
        // and at the inventory's opening and shutting, whichever came last.
        if input.reloading != self.reloading {
            self.prone_reload = input.reloading;
        }
        self.reloading = input.reloading;
        self.hands_busy = input.hands_busy;
        if input.inventory != self.inventory {
            self.inventory = input.inventory;
            if !input.inventory_unheard {
                self.prone_reload = input.inventory;
            }
            // Opening and shutting both switch a held vault key off.
            self.vault_on = false;
            self.vault_time = 0.0;
            // Opening stops the hands' gestures where they are, all but
            // those that are operations, and ends blind fire
            // (`Player.SetInventoryOpened`).
            if input.inventory {
                self.end_blind_fire();
                // And leaves a mount at once.
                self.leave_mount_now();
                self.force_stop_interactions();
            }
        }
        // The body's own pose for an open inventory: taken when the hands
        // have gone down for it, and given up for as long as they are
        // doing something else meanwhile (`Player.WaitWeaponInventory`).
        if !input.inventory {
            (self.body_inventory, self.inventory_reached) = ([false; 2], false);
        } else {
            if input.hands_in_inventory && !self.inventory_reached {
                self.inventory_reached = true;
                self.body_inventory[0] = true;
            }
            if self.inventory_reached {
                self.body_inventory[1] = !input.hands_in_inventory;
            }
        }
        // The game's states for unlocking a thing and for taking one up
        // take no walk and no jump. PROVISIONAL: they are not movement
        // states of their own here; the keys are noted and not heard, as
        // under a screen, so a walk runs out as with the keys let go
        // (the game's body stops for the approach), and a stance or a
        // lean asked for meanwhile, which the game's state for a door
        // takes, is dropped. (The pick-up's is compared with a recording:
        // 0.61 s, the view held, a third of the stance; interactions
        // spec.)
        if input.screen || self.acting.is_some() {
            self.unheard(input);
        } else {
            self.heard(input, world);
        }
        // Neither of these is a key's doing. The sights coming down let a
        // held breath go whatever the state; and the vault key's try is
        // made on every frame it is switched on, timed by the engine's
        // clock. Standing and the states built on it try (lying still and
        // rolled over do not), and so do the walk, the crawl and the sprint.
        if !self.aiming {
            self.stamina.hold_breath(false);
        }
        if self.vault_on {
            // A mounted state's vault leaves the mount at once and passes it on.
            self.leave_mount_now();
            let asks = (self.state.stands() && !matches!(self.state, State::ProneIdle | State::Roll))
                || matches!(self.state, State::Move | State::ProneMove | State::Sprint);
            if asks {
                self.try_vaulting(input, world);
            }
            self.vault_time += self.frame_dt;
        }
    }

    /// A frame in which a screen has the keys: nothing is handed over.
    /// Each key is noted as it stands, so that what it did meanwhile is
    /// lost and not made up for when the screen shuts. The walking, the
    /// crawling and the sprinting state are left with the input their
    /// last update left them, which is none; a jump keeps its direction;
    /// standing, nothing is asked at all.
    fn unheard(&mut self, input: &Input) {
        self.step_key = input.step;
        self.lean_keys = lean_keys(input);
        self.sprint_input = input.sprint;
        self.vault_key = input.vault;
    }

    /// A lean asked of the movement state in hand (`MovementState.SetTilt`
    /// and what overrides it). A jump, a sprint and its stop, a step aside
    /// and a roll do not listen: the lean they put to zero as they began
    /// stays, key or no key. A crawl takes any request as one for none.
    fn ask_tilt(&mut self, tilt: f32) {
        match self.state {
            State::Jump | State::Sprint | State::SprintStop | State::Sidestep | State::Roll => {}
            State::ProneMove => self.tilt = 0.0,
            _ => self.tilt = tilt.clamp(-MAX_TILT, MAX_TILT),
        }
    }

    /// A frame's keys, in the order the game takes them.
    pub(super) fn heard(&mut self, input: &Input, world: &dyn World) {
        // Blind fire's commands, each passed by the state in hand to the
        // gate (`Player.ToggleBlindFire`, `StopBlindFire`): the end, then
        // a start.
        // Mounted, the mount's state has every command first (`Player.
        // CurrentManagedState`): blind fire, the stance, the step aside and the
        // jump start its exit and are passed on; lying down and a vault leave
        // at once and are passed on; the lean, the walk keys and the slow lean
        // start the exit and are dropped; the sprint and the speed are dropped.
        let mounted = self.mount.is_some();
        self.left_stance_firearm = input.blind_firearm;
        if input.blind_end {
            if mounted {
                self.start_leaving_mount();
            }
            self.ask_blind_fire(0, input);
        }
        // A start with the left shoulder wanted is dropped by the gate
        // (`Player.ToggleBlindFire`), before the state in hand hears it.
        if input.blind_toggle != 0 && !self.left_stance.wanted {
            if mounted {
                self.start_leaving_mount();
            }
            self.ask_blind_fire(input.blind_toggle.signum(), input);
        }
        self.ask_left_stance(input);
        if input.speed_delta != 0.0 && self.state != State::Sprint && !mounted {
            self.select_speed(self.clamped_speed() + input.speed_delta);
        }
        // The walk key (`Player.ToggleMinSpeed`): a speed above none is
        // put aside and the speed put to none, which walks at the floor
        // under a walk's pace; at none already, what was put aside is
        // taken up again. The key coming up, where it is bound to be held,
        // only takes it up again (`RestoreSpeed`).
        if mounted {
        } else if input.walk_toggle && self.clamped_speed() >= f32::EPSILON {
            self.speed_memo = self.character_speed;
            self.change_speed(-self.speed_memo);
        } else if input.walk_toggle || input.walk_end {
            if self.clamped_speed() < f32::EPSILON {
                self.change_speed(self.speed_memo - self.character_speed);
            }
        }
        if input.prone {
            self.leave_mount_now();
            self.prone_command(world);
        }
        if (input.crouch || (input.restore_pose && !self.prone && self.pose_level < 0.5) || input.pose_delta != 0.0) && self.mount.is_some() {
            self.start_leaving_mount();
        }
        if input.crouch {
            // Back to the stance last chosen with the stance keys, if the
            // stance is not there now (the key's own crouch, or a sprint
            // that stood the character up); from there, between crouched
            // and standing. The key does not itself count as choosing.
            // Lying down it asks for no change at all, and what that does
            // is the state's.
            let target = if self.prone {
                self.pose_level
            } else if (self.pose_memo - self.pose_level).abs() < 1e-6 {
                if self.pose_level >= 0.5 { 0.0 } else { 1.0 }
            } else {
                self.pose_memo
            };
            self.change_pose(target - self.pose_level, world);
        }
        // The crouch key coming up, where it is bound to be held
        // (`Player.RestorePose`): a character lower than half way is put
        // back to the stance last chosen, or upright if that was as low.
        if input.restore_pose && !self.prone && self.pose_level < 0.5 {
            let to = if self.pose_memo < 0.5 { 1.0 } else { self.pose_memo };
            self.change_pose(to - self.pose_level, world);
        }
        if input.pose_delta != 0.0 {
            self.change_pose(input.pose_delta, world);
            self.pose_memo = self.pose_level;
        }
        // The lean keys are presses and releases (`Player.ToggleLean`,
        // `Straighten`). A key let go straightens the character if it
        // still leans to that key's side; a key pressed leans it to its
        // side, or straightens it if it leans that way already. Nothing
        // holds the lean to a key in between.
        // Neither does anything on a frame after one in which the lean
        // was made slowly (`_lastSlowLean`).
        let keys = lean_keys(input);
        let sides = [(0, -1.0f32), (1, 1.0)];
        for (key, side) in sides {
            if ((self.lean_keys[key] && !keys[key]) || input.lean_end[key]) && !self.last_slow_lean && self.tilt * side > 0.0 {
                if self.mount.is_some() {
                    self.mount_tilt(0.0);
                } else {
                    self.ask_tilt(0.0);
                }
            }
        }
        for (key, side) in sides {
            if ((!self.lean_keys[key] && keys[key]) || input.lean_toggle[key]) && !self.last_slow_lean {
                self.slow_leaning = false;
                // A lean pressed tells the body's animator blind fire is
                // over, and nothing else of it (blind-fire spec, "The
                // lean's quirk": `MovementContext.SetBlindFire(float)`).
                // PROVISIONAL: read from code, not recorded.
                self.blind_shown = 0.0;
                let tilt = if self.tilt * side > 0.0 { 0.0 } else { side * MAX_TILT };
                if self.mount.is_some() {
                    self.mount_tilt(tilt);
                } else {
                    self.ask_tilt(tilt);
                }
            }
        }
        self.lean_keys = keys;
        // The step-aside key: pressed, it asks for the step to its side,
        // or takes it back if that is what is already asked for; let go,
        // it takes its own side's step back.
        if input.step != self.step_key {
            let before = std::mem::replace(&mut self.step_key, input.step);
            if before != 0 && self.step == before {
                self.step_command(0, world);
            }
            if input.step != 0 {
                self.step_command(if self.step == input.step { 0 } else { input.step }, world);
            }
        }
        // The same as commands (`Player.ToggleStep`, `ReturnFromStep`).
        for (at, side) in [(0, -1), (1, 1)] {
            if input.step_end[at] && self.step == side {
                self.step_command(0, world);
            }
            if input.step_toggle[at] {
                self.step_command(if self.step == side { 0 } else { side }, world);
            }
        }
        // The key press is a toggle command and the release an explicit stop.
        if input.sprint != self.sprint_input {
            self.sprint_input = input.sprint;
            if mounted {
            } else if input.sprint {
                self.sprint_command(!self.sprinting, true);
            } else {
                self.sprint_command(false, false);
            }
        }
        // The two as commands (`Player.ToggleSprint`, `EnableSprint`).
        if input.sprint_toggle && !mounted {
            self.sprint_command(!self.sprinting, true);
        }
        if input.sprint_end && !mounted {
            self.sprint_command(false, false);
        }
        // A toggle pressed before the character is moving forward is ignored
        // by the movement states, and nothing repeats it: in a recorded run
        // with sprint held first and forward pressed after, the game walks.
        // With blind fire's figure set the movement keys are not handed on
        // at all, and the last direction given stands
        // (`MoveInputTranslator.TranslateAxes`).
        if self.blind_fire == 0 {
            self.input_direction = input.move_dir;
            if self.mount.is_some() {
                // A mounted state's walk: any key starts the exit, and the
                // movement state is handed nothing (`IdleWeaponMountingState.Move`).
                if !is_zero(input.move_dir) {
                    self.start_leaving_mount();
                }
            } else {
                self.direction = input.move_dir;
                match self.state {
                    State::Jump => self.jump.move_dir = input.move_dir,
                    state if state.stands() => self.idle_move(input.move_dir),
                    _ => {}
                }
            }
        }
        // The slow lean's axis (`Player.SlowLean`), which the game hands
        // out with the walk's, after the frame's commands: pushed, and
        // with the body's lean within a degree of the one asked for, it
        // asks the state in hand for a tenth of a degree more that way.
        // A frame, not a time: the lean goes as fast as frames come.
        self.last_slow_lean = false;
        if (self.smoothed_tilt - self.tilt).abs() < 1.0 && input.slow_lean != 0.0 {
            self.slow_leaning = true;
            if self.mount.is_some() {
                self.mount_tilt(self.tilt + input.slow_lean * SLOW_LEAN_STEP);
            } else {
                self.ask_tilt(self.tilt + input.slow_lean * SLOW_LEAN_STEP);
            }
            self.last_slow_lean = true;
        }
        // The jump is refused once the vault key has been held past its
        // time, and while a vault is under way (`Player.Jump`). In the
        // game's default binding the two are one key, and the jump comes
        // when it is let go.
        let jump = input.jump && self.vault_time <= self.settings.vaulting.input_time && !self.is_vaulting;
        // A mounted state's jump starts the exit and is passed on.
        if jump {
            self.start_leaving_mount();
        }
        if jump && matches!(self.state, State::ProneIdle | State::ProneMove) {
            // Lying down, jump gets up.
            self.prone_command(world);
        } else if jump && matches!(self.state, State::Idle | State::Move | State::Sprint | State::BlindFire) {
            // (Blind fire's state jumps as standing does; its animator states
            // lead to no jump, which waits for standing: blind-fire spec, "The
            // jump". PROVISIONAL: read from code, not recorded.)
            if self.pose_level > JUMP_POSE_THRESHOLD && self.grounded {
                // The animator takes it from here: its jump state starts
                // the jump proper. Legs at or under the threshold do not
                // jump, and nothing else happens for the key.
                if self.stamina.can_jump() && self.conditions.can_jump() {
                    self.jump_flag = true;
                }
            } else {
                // Too low to jump: stand up instead.
                self.change_pose(1.0 - self.pose_level, world);
            }
        } else if jump && self.state == State::Sidestep {
            // Stepped aside, jump only takes the step back.
            self.set_step(0, world);
            self.sidestep = 0.0;
        } else if jump && self.state == State::Roll {
            // Rolled over, the same.
            self.set_step(0, world);
        }
        // The hold-breath key: a press holds the breath or lets it go.
        // Only the standing state and those built on it listen
        // (`IdlePlayerState.EnableBreath`), and only with the sights up
        // (`MovementContext.HoldBreath`).
        // (A mounted state holds the breath whatever the movement state under
        // it: `IdleWeaponMountingState.EnableBreath`.)
        if input.hold_breath && (self.state.stands() || self.mount.is_some()) && self.aiming {
            let held = self.stamina.holding_breath();
            self.stamina.hold_breath(!held);
        }
        // The key coming up, where it is bound to be held, lets a held
        // breath go (`Player.StopHoldingBreath`).
        if input.breath_end && self.state.stands() && self.aiming && self.stamina.holding_breath() {
            self.stamina.hold_breath(false);
        }
        self.hear_vault_key(input.vault);
        // The mount's key (`ECommand.WeaponMounting`), from the hands' input.
        if input.mount {
            self.ask_mount(input, world);
        }
    }

    /// The vault key, pressed or let go, switches its try over
    /// (`Player.Vaulting`); switched off, the hold's time is cleared.
    pub(super) fn hear_vault_key(&mut self, key: bool) {
        if key != self.vault_key {
            self.vault_key = key;
            self.vault_on = !self.vault_on;
            if !self.vault_on {
                self.vault_time = 0.0;
            }
        }
    }
}
