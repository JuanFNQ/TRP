//! The step on the clock, left stance, idling and appearing.

use super::*;

impl Firearm {
    /// One frame of `dt` seconds, in the game's order: the operation's own
    /// update, the engine's update of the animator, and at the frame's end
    /// the marks that update passed, each to the operation then in hand.
    /// Returns what happened.
    pub fn step(&mut self, dt: f32) -> Vec<Event> {
        self.step_on_clock(dt, dt)
    }

    /// `step`, the operation's own update on the player's clock
    /// (`player_dt`: the game's arms are updated with the player's
    /// whole-millisecond frame time, `Player.ArmsUpdate`) and the animator
    /// on the engine's (`dt`). The single shot's cooldown, the automatic
    /// fire's cycle and a look's wait go by the first (recorded: the
    /// Five-seveN's cooldown of 0.133 s ran out after 16 frames or 17 as
    /// the frames' whole milliseconds came to 133.3 or not;
    /// with it the scripted recordings' automatic fire has the game's round
    /// count on every frame, where a shot of each had come a frame off).
    pub fn step_on_clock(&mut self, dt: f32, player_dt: f32) -> Vec<Event> {
        // An operation the host began since the last frame.
        self.note_operation();
        // The animator hears of the sights whenever they have changed.
        if self.aiming != self.aim_told {
            self.tell_aim();
        }
        // A reload made with the sights up stiffens the weapon, and lets
        // it go a moment after either ends.
        self.clock += dt;
        if (self.aiming && self.reloading()) || self.mount_tactical {
            self.tactical.aim_for(1.0, 4.0, 0.0, self.clock);
        } else {
            self.tactical.aim_for(0.0, 2.0, 0.2, self.clock);
        }
        let heat_rate = self.heat_rate().filter(|mult| (mult - self.burst_rate).abs() > f32::EPSILON);
        // The heat's stray shot, at rest.
        if matches!(self.operation, Operation::Idle) {
            self.autoshot();
        }
        match &mut self.operation {
            Operation::Single { cooldown, ready, .. } => {
                *cooldown -= player_dt;
                if *cooldown <= 0.0 && *ready {
                    self.end_single();
                }
            }
            Operation::Automatic { running: false, .. } => {}
            Operation::Automatic { clock, .. } => {
                // The heat's factor, where a new one is drawn: the interval
                // changed with the phase kept (`AutomaticFireOperation.Update`).
                if let Some(mult) = heat_rate {
                    let phase = *clock % self.auto_cycle / self.auto_cycle;
                    self.auto_cycle = 60.0 / (self.settings.fire_rate * mult);
                    *clock = self.auto_cycle * (phase % 1.0);
                    self.burst_rate = mult;
                }
                let cycle = self.auto_cycle;
                let start = *clock % cycle / cycle;
                let end = start + player_dt / cycle;
                *clock += player_dt;
                let mut spans = vec![(start, end.min(1.0))];
                let mut rest = end - 1.0;
                while rest > 0.0 {
                    spans.push((0.0, rest.min(1.0)));
                    rest -= 1.0;
                }
                for (from, to) in spans {
                    self.automatic_span(from, to);
                }
            }
            // A look whose state never gives its first mark is given up.
            Operation::Utility { ready: false, time, in_inventory: false, examine } => {
                if *time > MAX_UTILITY_TIME {
                    self.operation = Operation::Utility { ready: true, time: 0.0, in_inventory: false, examine: *examine };
                    self.idle_started();
                } else {
                    *time += player_dt;
                }
            }
            _ => {}
        }
        // The launcher's own part: the recoil of its last shot, its looks' wait,
        // its range.
        self.launcher_update(dt, player_dt);
        // What is drawn is on curves without corners where the host asks
        // (`set_drawn_smooth`), every frame: an animator made afresh for a
        // draw starts as the engine has it.
        self.animator.set_drawn_smooth(self.drawn_smooth);
        self.animator.set_deferred_posing(self.deferred_posing);
        self.animator.update(dt);
        let launcher_speaks = self.launcher_step_animator(dt);
        // The frame's end: the recoil is given a shot the last frame's
        // marks made, and then this frame's marks are delivered.
        if std::mem::take(&mut self.shot_made) {
            let hammer_down = std::mem::take(&mut self.shot_hammer_down);
            self.events.push(Event::Shot { strength: self.settings.shot_strength, hammer_down });
        }
        if launcher_speaks {
            // The launcher's marks, the rifle's heard by nobody.
            self.launcher_deliver();
            self.marks = Vec::new();
        } else {
            // One at a time, each tested against the parameters as the one
            // before it left them (`AnimationEventsEmitter.EmitEvents`).
            let mut marks = Vec::new();
            while let Some(mark) = self.animator.deliver_next() {
                self.on_mark(&mark);
                marks.push(mark);
            }
            self.marks = marks;
        }
        self.note_operation();
        std::mem::take(&mut self.events)
    }

    /// What the operation in hand does to the left shoulder as it begins
    /// (`SetLeftStanceAnimOnStartOperation`): the rest and the draw put its
    /// animation back, the fire (`FireOperationBase`'s) leaves it, every
    /// other takes it off (`FirearmOperation`'s). No weapon of the install
    /// refuses it (`BlockLeftStance`), and none is stationary.
    fn left_stance_start(&self) -> Option<crate::weapons::left_stance::HandsStart> {
        use crate::weapons::left_stance::HandsStart;
        match self.operation {
            Operation::Idle => Some(HandsStart::Idle),
            Operation::Spawn { .. } => Some(HandsStart::Draw { blocked: false }),
            Operation::Single { .. } | Operation::Automatic { .. } | Operation::CylinderFire { .. } | Operation::BarrelsFire { .. } => None,
            // `MalfunctionOperation`'s own start leaves it too.
            Operation::Malfunction { .. } => None,
            // The launcher's switch leaves it (its start empty), its rest and its
            // looks put it back (`LauncherIdling`), its shot leaves it.
            Operation::ToggleLauncher { .. } | Operation::LauncherFire { .. } => None,
            Operation::LauncherIdle | Operation::LauncherUtility { .. } => Some(HandsStart::Idle),
            _ => Some(HandsStart::Other),
        }
    }

    /// An operation begun since the left shoulder was last told: its start
    /// told now. (The game calls it from each operation's `Start`; a change
    /// of kind is that here, the operations' own changes of their fields
    /// not being starts. `PROVISIONAL`: looked at as a frame begins and
    /// ends, so one begun and over between the two is not told.)
    fn note_operation(&mut self) {
        let kind = std::mem::discriminant(&self.operation);
        if self.left_stance_seen != Some(kind) {
            self.left_stance_seen = Some(kind);
            if let Some(start) = self.left_stance_start() {
                self.events.push(Event::LeftStance(crate::weapons::left_stance::Call::Start(start)));
            }
        }
    }

    /// Whether the operation in hand passes the left shoulder's key on
    /// (`ToggleLeftStance`: `Idling`'s, and so the looks', which are built
    /// on it; every other drops it). Lying down is the movement's to refuse.
    pub fn may_toggle_left_stance(&self) -> bool {
        matches!(self.operation, Operation::Idle | Operation::Utility { .. } | Operation::LauncherIdle | Operation::LauncherUtility { .. })
    }

    /// The mark at the very start of the animator's resting state.
    pub(super) fn idle_started(&mut self) {
        // The resting state begun: the sights' news is old
        // (`Idling.OnIdleStartEvent`).
        self.aim_flags = false;
        match self.operation {
            // The weapon is back at rest: the body is told the hands are
            // free, and the trigger is heard again after a reload; and the
            // left shoulder's animation put back (`Idling.OnIdleStartEvent`).
            Operation::Idle => {
                (self.hands_busy, self.reload_flag) = (false, false);
                self.events.push(Event::LeftStance(crate::weapons::left_stance::Call::Start(crate::weapons::left_stance::HandsStart::Idle)));
            }
            Operation::Single { fired: true, ref mut ready, .. } => *ready = true,
            Operation::Reload { mag_shown: true, .. } => {
                self.reload_flag = false;
                self.reload_over();
            }
            Operation::Utility { ready: true, .. } => {
                (self.hands_busy, self.reload_flag) = (false, false);
                self.to_idle();
            }
            Operation::Spawn { .. } => self.appeared(),
            _ => {}
        }
    }

    /// The draw is over: the weapon idles, and is put away again at once
    /// if that was asked for meanwhile.
    pub(super) fn appeared(&mut self) {
        let Operation::Spawn { hide, compatible, .. } = self.operation else { return };
        // A misfire is put right by the draw (`SpawnOperation.WeaponAppeared`):
        // with a magazine in, compatible as the draw began, and a chamber.
        if compatible && self.magazine.is_some() && !self.has_chambers() && !self.settings.no_chamber {
            self.clear_misfire();
        }
        self.to_idle();
        // A weapon whose item is still on its launcher (hands lost on it) goes
        // straight on into the switch (`Idling.Start`).
        if self.launcher.as_ref().is_some_and(|launcher| launcher.marked) {
            self.begin_switch(true, None);
        }
        self.events.push(Event::Appeared);
        // The draw's callback: the game's owner tells what has just
        // appeared how the screen stands (`GamePlayerOwner.SetHandsController`
        // through `Player.SpawnController`'s `RaiseHandsChanged`), which at
        // rest puts the sights down; then a put-away asked meanwhile; then
        // the sights turned.
        let told = self.appearing;
        if let Some((screen, ..)) = told {
            self.set_inventory(screen);
        }
        if let Some(fast) = hide {
            self.hide(fast);
        } else if let Some((_, true, allowed)) = told {
            // The sights turned over as the aim's key does (`ToggleAim`).
            // A put-away asked for in the draw went first, from rest, with
            // the sights down: the game's flag is then down, and nothing
            // comes up.
            self.set_aim(!self.aiming, allowed);
        }
        self.hands_busy = false;
    }

    /// What the host knows before each step of a draw that may end in it
    /// (`SpawnOperation.WeaponAppeared`): whether the inventory's screen is
    /// open, which the weapon that has appeared is told first, in place of
    /// the host's telling it after (`draw`); whether the sights then turn
    /// over, the last slot having been taken by the holster's key pressed
    /// twice and the last weapon put away from rest having gone aimed
    /// (`ToggleAim` with the player's `FastSlotSelection` and
    /// `PreviousWeaponAimState`; `Event::AimKept`); and whether the
    /// movement lets them up (`Movement::can_aim`). A host that never calls
    /// it tells the screen itself on `Event::Appeared`, and nothing turns.
    pub fn set_appearing(&mut self, screen: bool, turn_aim: bool, can_aim: bool) {
        self.appearing = Some((screen, turn_aim, can_aim));
    }

    /// The game's reload hands over to its idling: the animation plays on.
    pub(super) fn reload_over(&mut self) {
        let Operation::Reload { hide, .. } = self.operation else { return };
        self.to_idle();
        if let Some(fast) = hide {
            self.hide(fast);
        }
    }
}
