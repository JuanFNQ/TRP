//! Firing single shots and automatic bursts.

use super::*;

impl Firearm {
    /// The fire state asked of the animator outright, part-way in, with
    /// the trigger as the controller has it: the self-loading mode has a
    /// state of its own (`FireOperation.StartFireAnimation`).
    fn animate_fire(&mut self) {
        self.animator.set_flag("Fire", self.trigger);
        let state = if self.mode() == FireMode::SemiAuto { &self.settings.semi_fire_state } else { &self.settings.fire_state };
        self.animator.play(state, FIRE_STATE_START);
    }

    /// A single shot begins: its rounds are settled now, and the shot
    /// itself is made when the fire state's mark reaches the operation.
    pub(super) fn start_single(&mut self) {
        // The roll, as the operation begins (`FireOperation.PrepareShot`): a
        // misfire is no shot at all; any other malfunction lets the fire
        // state come by the animator's own way in (`StartFireAnimation`
        // plays it only with none), its shot made at its mark.
        let malf = self.roll_shot();
        if malf == Malf::Misfire {
            return self.misfire_single();
        }
        self.take_round_malfunctioned(malf);
        self.round_out = true;
        if malf == Malf::None {
            self.animate_fire();
        } else {
            self.animator.set_flag("Fire", self.trigger);
        }
        // A bolt action holds its bolt for as long as the trigger is held
        // (`BoltActionFireOperation.Start`).
        if self.settings.bolt_action {
            self.animator.set_flag("BoltActionReload", !self.trigger);
        }
        self.operation = Operation::Single {
            cooldown: 60.0 / self.settings.single_fire_rate,
            fired: false,
            ready: false,
            second_shot: false,
            open_inventory: false,
            planned: None,
            malf,
        };
    }

    pub(super) fn start_automatic(&mut self) {
        self.events.push(Event::AutoFire(true));
        // The heat's factor is applied afresh each burst (`float_6`).
        self.burst_rate = 0.0;
        self.animate_fire();
        // The clock does not start at zero, so that the first step does
        // not count as the start of a second cycle.
        self.operation = Operation::Automatic { clock: 0.0001, shots: 0, running: false, open_inventory: false, planned: None };
        self.fire_round();
    }

    /// The trigger as the controller has it, set; let go, the weapon's
    /// sound player is told (`Event::TriggerReleased`).
    pub(super) fn trigger_to(&mut self, pressed: bool) {
        if self.trigger && !pressed {
            self.events.push(Event::TriggerReleased);
        }
        self.trigger = pressed;
    }

    /// Takes the chambered round and feeds the next from the magazine, if
    /// the chamber takes its top round (`Magazine.IsAmmoCompatible`); a
    /// weapon with no chamber takes the round off its magazine
    /// (`GetAmmoForSingleShot`: `PopToNowhere`).
    pub(super) fn take_round(&mut self) {
        if self.settings.no_chamber {
            if let Some(rounds) = self.magazine.as_mut() {
                *rounds = rounds.saturating_sub(1);
            }
            return;
        }
        let unfit = self.unfit;
        self.chamber = match &mut self.magazine {
            Some(rounds) if *rounds > 0 && *rounds > unfit => {
                *rounds -= 1;
                true
            }
            _ => false,
        };
    }

    /// Fires the chambered round and feeds the next from the magazine, in
    /// one: an automatic shot.
    fn fire_round(&mut self) {
        // The roll, each shot of the run (`AutomaticFireOperation.InternalOnFireEvent`):
        // a misfire makes no shot; any other malfunction's shot is made and
        // the run is over.
        let malf = self.roll_shot();
        if malf == Malf::Misfire {
            self.take_round_malfunctioned(malf);
            return self.switch_to_malfunction(malf, true);
        }
        self.take_round_malfunctioned(malf);
        self.round_out = true;
        self.events.push(Event::Shot { strength: self.settings.shot_strength, hammer_down: false });
        // A malfunctioned shot plays its tail at once (`FireBullet`'s
        // `malfunctioned`), as a trigger let go.
        let trigger = self.trigger && malf == Malf::None;
        self.events.push(Event::FireSound { trigger, doublet: false });
        if let Operation::Automatic { shots, .. } = &mut self.operation {
            *shots += 1;
        }
        self.shot_worn();
        if malf != Malf::None {
            return self.switch_to_malfunction(malf, true);
        }
        self.animator.set_flag("Fire", self.trigger);
    }

    /// The idling operation takes over (`Idling.Start`). Its start gives
    /// the animator the skill's two speeds again
    /// (`SetAnimatorAndProceduralValues`): a quick draw's last through the
    /// draw only (`pistol-quickdraw`: the pistol's `SpeedDraw` 1.5 until it
    /// idles, 1 from then).
    pub(super) fn to_idle(&mut self) {
        self.operation = Operation::Idle;
        self.round_out = false;
        self.animator_values();
        self.animator_skill_speeds();
    }

    /// A single shot's wait is out and the weapon back at rest.
    pub(super) fn end_single(&mut self) {
        let Operation::Single { second_shot, open_inventory, planned, .. } = self.operation else { return };
        self.animator.set_flag("Fire", self.trigger);
        self.animator_rounds();
        match planned {
            Some(planned) => {
                self.aiming = false;
                self.run(planned);
            }
            None => {
                self.animator.set_flag("Inventory", open_inventory);
                self.to_idle();
                // A queued shot is a press and a release.
                if second_shot && !open_inventory {
                    self.idle_trigger(true);
                    self.trigger_changed(false);
                }
            }
        }
    }

    /// What a shot was asked to hand over to.
    pub(super) fn run(&mut self, planned: Planned) {
        match planned {
            Planned::Reload(rounds, quick) => self.begin_reload(rounds, quick),
            Planned::Shells(within_reach) => self.begin_shells(within_reach),
            Planned::Hide(fast) => self.start_remove(fast),
        }
    }

    /// The part of one automatic cycle between two fractions of it.
    pub(super) fn automatic_span(&mut self, from: f32, to: f32) {
        let crosses = |mark: f32| from <= mark && mark < to;
        if !self.auto_fire() {
            return;
        }
        if crosses(0.0) {
            self.fire_round();
        }
        if crosses(AUTO_SHELL_EJECT) {
            self.events.push(Event::ShellEject);
        }
        // The round fed at the shot shows in the chamber at AUTO_CHAMBER;
        // nothing in the logic waits for it, and the animator is told. A
        // weapon with a catch that has fired its last round has its bolt
        // caught there.
        if crosses(AUTO_CHAMBER) {
            let s = &self.settings;
            if s.bolt_catch && !s.manual_bolt_catch && !self.chamber && self.magazine == Some(0) {
                self.set_bolt_catch(true);
            }
            self.round_out = false;
            self.animator_rounds();
            self.animator.set_flag("Fire", self.trigger);
        }
        if crosses(AUTO_CYCLE_END) {
            self.automatic_cycle_end();
        }
    }

    fn automatic_cycle_end(&mut self) {
        let Operation::Automatic { shots, open_inventory, planned, .. } = self.operation else { return };
        self.animator_rounds();
        if self.mode() == FireMode::Burst {
            if shots < self.settings.burst_count && self.chamber {
                self.trigger_to(true);
                return;
            }
            self.trigger_to(false);
        }
        if let Some(planned) = planned {
            self.trigger_to(false);
            self.aiming = false;
            self.animator.set_flag("Fire", false);
            self.animator.play(&self.settings.rest_state, REST_AFTER_AUTO);
            self.events.push(Event::AutoFire(false));
            self.run(planned);
            return;
        }
        // An open inventory ends the run at the cycle's end, trigger or no.
        if self.trigger && !open_inventory {
            // Held on while a round is left: in the chamber, or for a
            // weapon with no chamber in its magazine.
            if self.loaded() {
                return;
            }
            self.automatic_to_idle(open_inventory);
            // Still holding on an empty weapon with no bolt catch: the
            // hammer falls once more.
            if !self.settings.bolt_catch {
                self.trigger_changed(true);
            }
            return;
        }
        self.automatic_to_idle(open_inventory);
    }

    /// Handing back to idling, the animator is asked for its resting
    /// state outright, a tenth in.
    fn automatic_to_idle(&mut self, open_inventory: bool) {
        self.trigger_to(false);
        self.animator.set_flag("Inventory", open_inventory);
        self.animator.set_flag("Fire", false);
        self.animator.play(&self.settings.rest_state, REST_AFTER_AUTO);
        self.events.push(Event::AutoFire(false));
        self.to_idle();
    }
}
