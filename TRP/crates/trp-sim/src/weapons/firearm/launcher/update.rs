//! The launcher's range, checks, aim, update and animation marks.

use super::*;

impl Firearm {
    /// The range keys with the launcher in use (`OpticCalibrationSwitchUp` and
    /// `Down`, `UnderbarrelSightingRangeUp` and `Down`): one step along `RANGES`,
    /// held at the ends, on the launcher's rest and its looks only. The figure set,
    /// where a step was taken.
    pub fn launcher_range_step(&mut self, up: bool) -> Option<i32> {
        if !matches!(self.operation, Operation::LauncherIdle | Operation::LauncherUtility { .. }) {
            return None;
        }
        let launcher = self.launcher.as_mut()?;
        launcher.step = if up { (launcher.step + 1).min(RANGES.len() - 1) } else { launcher.step.saturating_sub(1) };
        launcher.range = RANGES[launcher.step];
        let range = launcher.range;
        self.events.push(Event::Launcher(LauncherEvent::Range(range)));
        Some(range)
    }

    /// The launcher's chamber looked into (`LauncherIdling.CheckChamber`): refused
    /// with the trigger held, and for a launcher whose rounds leave no case with
    /// nothing in it. Whether it began.
    pub(crate) fn launcher_check_chamber(&mut self) -> bool {
        let Some(launcher) = self.launcher.as_mut() else { return false };
        if self.trigger || (launcher.caseless && !launcher.round) {
            return false;
        }
        self.aiming = false;
        launcher.animator.trigger("CheckChamber");
        self.hands_busy = true;
        self.operation = Operation::LauncherUtility { ready: false, time: 0.0, examine: false };
        true
    }

    /// The launcher looked over (`LauncherIdling.ExamineWeapon`): refused with the
    /// sights up.
    pub(crate) fn launcher_examine(&mut self) -> bool {
        if self.aiming {
            return false;
        }
        let Some(launcher) = self.launcher.as_mut() else { return false };
        launcher.animator.trigger("Look");
        self.operation = Operation::LauncherUtility { ready: false, time: 0.0, examine: true };
        true
    }

    /// The aim key as a launcher's operation takes it: its rest lets the sights up
    /// where the movement does and nothing is in the way (no mount's exception), its
    /// shot asks nothing, the rest refuse.
    pub(crate) fn launcher_aim(&mut self, want: bool, allowed: bool) {
        let blocked = self.overlap > self.settings.stop_aiming_at;
        match self.operation {
            Operation::LauncherIdle => {
                if !want || (allowed && !blocked) {
                    self.aiming = want;
                }
            }
            Operation::LauncherFire { .. } => self.aiming = want,
            _ => {}
        }
    }

    /// The launcher's part of a frame, before the animators move: the recoil of last
    /// frame's shot, the looks' wait, and the range followed on its rest.
    pub(crate) fn launcher_update(&mut self, dt: f32, player_dt: f32) {
        let Some(launcher) = self.launcher.as_mut() else { return };
        if std::mem::take(&mut launcher.recoil_due) {
            self.events.push(Event::Launcher(LauncherEvent::Recoil));
        }
        match &mut self.operation {
            // The animator's range follows at 400 m a second, whole metres a frame,
            // and is written only while it moves (`LauncherIdling.SetSightingRange`).
            Operation::LauncherIdle if launcher.shown_range != launcher.range => {
                let last = launcher.shown_range as f32;
                let target = launcher.range as f32;
                let next = if last > target { (last - RANGE_SPEED * player_dt).clamp(target, last) } else { (last + RANGE_SPEED * player_dt).clamp(last, target) };
                launcher.shown_range = next as i32;
                launcher.animator.set("SightingRange", launcher.shown_range as f32);
            }
            // A look whose state never gives its first mark is given up.
            Operation::LauncherUtility { ready: false, time, examine } => {
                if *time > MAX_UTILITY_TIME {
                    let examine = *examine;
                    self.operation = Operation::LauncherUtility { ready: true, time: 0.0, examine };
                    self.launcher_idle_start();
                } else {
                    *time += player_dt;
                }
            }
            _ => {}
        }
        let _ = dt;
    }

    /// The launcher's animator moves with the rifle's while it is the hands'; the
    /// rifle's marks meanwhile are heard by nobody (`AnimationEventsEmitter` swapped).
    /// Its marks reach the operation in hand. Whether it spoke this frame.
    pub(crate) fn launcher_step_animator(&mut self, dt: f32) -> bool {
        let smooth = self.drawn_smooth;
        let Some(launcher) = self.launcher.as_mut() else { return false };
        if !launcher.speaks {
            return false;
        }
        launcher.animator.set_drawn_smooth(smooth);
        launcher.animator.update(dt);
        true
    }

    /// The launcher's marks, one at a time, each to the operation then in hand; the
    /// rifle's, dropped.
    pub(crate) fn launcher_deliver(&mut self) -> Vec<Mark> {
        let _ = self.animator.deliver();
        let mut marks = Vec::new();
        loop {
            let Some(launcher) = self.launcher.as_mut() else { break };
            if !launcher.speaks {
                break;
            }
            let Some(mark) = launcher.animator.deliver_next() else { break };
            self.on_launcher_mark(&mark);
            marks.push(mark);
        }
        if let Some(launcher) = self.launcher.as_mut() {
            launcher.marks_seen = marks.clone();
        }
        marks
    }

    fn on_launcher_mark(&mut self, mark: &Mark) {
        let name = mark.name.as_str();
        if name == "Sound" {
            self.events.push(Event::LauncherSound(mark.text.clone()));
            return;
        }
        match (name, &mut self.operation) {
            ("LauncherAppeared", Operation::ToggleLauncher { on: true, hide }) => {
                let hide = *hide;
                self.events.push(Event::Launcher(LauncherEvent::Switched(true)));
                self.launcher_rest();
                let then = self.launcher.as_mut().and_then(|launcher| launcher.then.take());
                if let Some(fast) = hide {
                    return self.launcher_hide(fast);
                }
                match then {
                    Some(true) => self.begin_load(),
                    Some(false) => self.begin_discharge(),
                    None => {}
                }
            }
            ("LauncherDisappeared", Operation::ToggleLauncher { on: false, hide }) => {
                let hide = *hide;
                if let Some(launcher) = self.launcher.as_mut() {
                    launcher.speaks = false;
                }
                self.events.push(Event::Launcher(LauncherEvent::Switched(false)));
                self.to_idle();
                if let Some(fast) = hide {
                    self.hide_from(fast, true);
                }
            }
            ("FiringBullet", Operation::LauncherFire { fired }) if !*fired => {
                *fired = true;
                // The rifle's left hand let go (`LActions` at nought).
                self.animator.set_layer_weight("LActions", 0.0);
                let Some(launcher) = self.launcher.as_mut() else { return };
                launcher.round = false;
                // The game asks the round (`AmmoTemplate.RemoveShellAfterFire`); its
                // launcher's `UseAmmoWithoutShell` agrees for every round each
                // launcher takes in the install (the VOG-25 none, the 40x46 rounds
                // theirs kept).
                launcher.case = !launcher.caseless;
                // The round's model gone, or moved to the shell port as the case
                // (`DestroyPatronInWeapon`, `MoveAmmoFromChamberToShellPort`).
                if std::mem::take(&mut launcher.round_drawn) && launcher.case {
                    launcher.case_drawn = true;
                }
                launcher.animator.set("ShellsInWeapon", launcher.case as u8 as f32);
                launcher.animator.set("AmmoInChamber", 0.0);
                launcher.recoil_due = true;
                let case_kept = launcher.case;
                self.events.push(Event::Launcher(LauncherEvent::Shot { case_kept }));
            }
            ("FireEnd", Operation::LauncherFire { .. }) => {
                self.trigger_to(false);
                self.launcher_rest();
            }
            // The reload's and a look's (`LauncherReload`, `LauncherUtilityOperation`:
            // the M203's `CHECK SHELL`): the case's model thrown where there is one
            // (`StartSpawnShell`), the cases cleared either way.
            ("ShellEject", Operation::LauncherReload { .. } | Operation::LauncherUtility { .. }) => {
                let Some(launcher) = self.launcher.as_mut() else { return };
                launcher.case = false;
                launcher.animator.set("ShellsInWeapon", 0.0);
                if std::mem::take(&mut launcher.case_drawn) {
                    self.events.push(Event::Launcher(LauncherEvent::CaseThrown(RELOAD_CASE_SHARE)));
                }
            }
            // The round's model into its chamber (`SetRoundIntoWeapon`).
            ("MagShow", Operation::LauncherReload { .. } | Operation::LauncherLoad) => {
                if let Some(launcher) = self.launcher.as_mut() {
                    launcher.round_drawn = true;
                }
                self.events.push(Event::Launcher(LauncherEvent::RoundShown));
            }
            ("AddAmmoInChamber", Operation::LauncherReload { hide }) => {
                let hide = *hide;
                self.launcher_rest();
                if let Some(fast) = hide {
                    self.launcher_hide(fast);
                }
            }
            ("RemoveShell", Operation::LauncherLoad) => {
                let Some(launcher) = self.launcher.as_mut() else { return };
                launcher.case_drawn = false;
                launcher.animator.set("ShellsInWeapon", 0.0);
                self.events.push(Event::Launcher(LauncherEvent::CaseRemoved));
            }
            ("AddAmmoInChamber", Operation::LauncherLoad) => {
                let Some(launcher) = self.launcher.as_mut() else { return };
                (launcher.round, launcher.case) = (true, false);
                for (name, value) in [("AmmoInChamber", 1.0), ("ShellsInWeapon", 0.0)] {
                    launcher.animator.set(name, value);
                }
                launcher.animator.set_flag("CanReload", false);
                launcher.animator.set_flag("LoadOne", false);
                self.events.push(Event::Launcher(LauncherEvent::MoveDone));
                self.launcher_rest();
            }
            ("DelAmmoChamber", Operation::LauncherDischarge) => {
                if let Some(launcher) = self.launcher.as_mut() {
                    launcher.round_drawn = false;
                }
                self.events.push(Event::Launcher(LauncherEvent::RoundRemoved));
            }
            ("MagShow", Operation::LauncherDischarge) => {
                let Some(launcher) = self.launcher.as_mut() else { return };
                launcher.animator.set_flag("Discharge", false);
                launcher.animator.set("ShellsInWeapon", launcher.case as u8 as f32);
                launcher.animator.set_flag("CanReload", false);
            }
            ("MagHide", Operation::LauncherDischarge) => {
                let Some(launcher) = self.launcher.as_mut() else { return };
                launcher.round = false;
                launcher.animator.set("AmmoInChamber", 0.0);
                self.events.push(Event::Launcher(LauncherEvent::MoveDone));
                self.launcher_rest();
            }
            ("StartUtilityOperation", Operation::LauncherUtility { ready, .. }) => *ready = true,
            ("IdleStart", _) => self.launcher_idle_start(),
            _ => {}
        }
    }

    /// The launcher's resting state begun (`IdleStart`): its rest's
    /// (`LauncherIdling.OnIdleStartEvent`: the left shoulder's animation put back,
    /// the body told the hands are free), or a look's end.
    fn launcher_idle_start(&mut self) {
        match self.operation {
            Operation::LauncherIdle => {
                self.hands_busy = false;
                self.events.push(Event::LeftStance(crate::weapons::left_stance::Call::Start(crate::weapons::left_stance::HandsStart::Idle)));
            }
            Operation::LauncherUtility { ready: true, .. } => {
                self.hands_busy = false;
                self.launcher_rest();
            }
            _ => {}
        }
    }
}
