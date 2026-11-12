//! Examining, checking and clearing a malfunction, and the repair marks.

use super::*;

impl Firearm {
    /// The trigger with a malfunction (`Idling.SetTriggerPressed`): no shot;
    /// the animator's dry fire, and the reminder. Whether it was taken here.
    pub(crate) fn malfunction_trigger(&mut self, pressed: bool) -> bool {
        if self.malfunction() == Malf::None {
            return false;
        }
        self.animator.set_flag("Fire", pressed);
        self.trigger_to(pressed);
        if pressed {
            self.animator.set_flag("MisfireSlideUnknown", false);
            self.remind();
            self.events.push(Event::DryFire);
        }
        true
    }

    /// The reminder (`ExamineMalfunction` when known): the battle screen's
    /// glow and words.
    pub(crate) fn remind(&mut self) {
        let Some(w) = self.wear.as_deref() else { return };
        self.events.push(Event::MalfunctionReminder { malf: w.state.malf, type_known: w.state.type_known });
    }

    /// The look at a malfunction (`ExamineMalfunctionTypeOperation`), from
    /// the look key at rest: the sights down, the trigger off, blind fire
    /// ended; the animator's look (`Look`: the AKM's `JAM_LOOK`). Whether
    /// it began.
    pub(crate) fn examine_malfunction(&mut self) -> bool {
        if self.malfunction() == Malf::None || !self.is_idling() || self.blind_fire {
            return false;
        }
        self.events.push(Event::LeaveMount { at_once: false });
        self.aiming = false;
        self.animator.set_flag("Fire", false);
        self.ends_blind_fire();
        (self.hands_busy, self.reload_flag) = (true, true);
        self.animator.trigger("Look");
        self.operation = Operation::ExamineMalfunction { hide: None };
        true
    }

    /// The look's mark: the malfunction's kind is known (`ExamineMalfunctionType`).
    pub(crate) fn malfunction_examined(&mut self) {
        let Operation::ExamineMalfunction { hide } = self.operation else { return };
        if let Some(w) = self.wear.as_deref_mut() {
            let first = !w.state.type_known;
            w.state.type_known = true;
            self.events.push(Event::MalfunctionExamined { malf: w.state.malf, first });
        }
        self.switch_to_idle(hide);
    }

    /// The check of the chamber with a malfunction (`Idling.CheckChamber`):
    /// the sights down; its repair where its kind is known, else the
    /// reminder. Whether it was taken here.
    pub(crate) fn malfunction_check_chamber(&mut self) -> bool {
        let malf = self.malfunction();
        if malf == Malf::None {
            return false;
        }
        if !self.is_idling() || self.trigger || self.blind_fire {
            return true;
        }
        self.aiming = false;
        if !self.wear.as_deref().is_some_and(|w| w.state.type_known) {
            self.animator.set_flag("MisfireSlideUnknown", false);
            self.remind();
            return true;
        }
        // `RepairMalfunction.Start`: the state put right at once, the
        // hands' repair played at the repair's speed.
        self.ends_blind_fire();
        let speed = self.wear.as_deref().map_or(1.0, |w| malfunction::repair_speed(&w.settings, &w.skills, w.arms_broken, malf));
        self.animator.set_flag("MalfunctionRepair", true);
        self.animator.set_layer_weight("Malfunction", 0.0);
        self.malfunction_ints(malf.number(), malf.number());
        self.animator.set("SpeedFix", speed);
        (self.hands_busy, self.reload_flag) = (true, true);
        if let Some(w) = self.wear.as_deref_mut() {
            w.state.malf = Malf::None;
            w.state.known = false;
            w.state.type_known = false;
        }
        self.events.push(Event::MalfunctionRepairBegun(malf));
        self.operation = Operation::RepairMalfunction { malf, ejected: false, hide: None };
        true
    }

    /// The repair's marks (`RepairMalfunction`): the round shown spent, the
    /// one thrown out (a failure to feed's onto the ground as a thing), the
    /// next chambered, the end. Whether the mark was the repair's.
    pub(crate) fn repair_mark(&mut self, name: &str) -> bool {
        let Operation::RepairMalfunction { malf, ejected, hide } = self.operation else { return false };
        match name {
            "DelAmmoChamber" => self.animator.set("AmmoInChamber", 0.0),
            "ShellEject" if !ejected => {
                self.operation = Operation::RepairMalfunction { malf, ejected: true, hide };
                self.events.push(match malf {
                    Malf::Jam => Event::ShellEject,
                    Malf::Feed => Event::FedRoundThrown,
                    _ => Event::MisfiredRoundThrown,
                });
            }
            "AddAmmoInChamber" => {
                if !self.chamber && self.magazine.unwrap_or(0) > 0 && !self.settings.no_chamber {
                    self.take_round();
                }
                self.animator_rounds();
            }
            "MalfunctionOff" => {
                self.animator.set_flag("MalfunctionRepair", false);
                self.malfunction_ints(0, 0);
                self.animator.set_flag("MisfireSlideUnknown", false);
                self.events.push(Event::MalfunctionRepaired(malf));
                self.switch_to_idle(hide);
            }
            _ => return false,
        }
        true
    }

    /// A misfire put right without its repair (`FirearmOperation.method_2`:
    /// a reload's or an inserted magazine's chambering, a draw, the chamber
    /// unloaded): the state cleared, the animator put back.
    pub(crate) fn clear_misfire(&mut self) {
        let Some(w) = self.wear.as_deref_mut() else { return };
        if w.state.malf != Malf::Misfire {
            return;
        }
        w.state.malf = Malf::None;
        w.state.known = false;
        w.state.type_known = false;
        self.malfunction_ints(0, 0);
        self.animator.set_flag("MalfunctionRepair", false);
        self.animator.set_layer_weight("Malfunction", 0.0);
        self.events.push(Event::MalfunctionCleared(Malf::Misfire));
    }

    /// The heat's stray shot (`Idling.ProcessAutoshot`), each frame at rest:
    /// cancelled with a malfunction; when its time has come, one single shot.
    pub(crate) fn autoshot(&mut self) {
        let ready = matches!(self.operation, Operation::Idle) && self.loaded();
        let Some(w) = self.wear.as_deref_mut() else { return };
        let Some(at) = w.state.autoshot_at else { return };
        if w.state.malf != Malf::None {
            w.state.autoshot_at = None;
            return;
        }
        if w.now < at || !ready {
            return;
        }
        w.state.autoshot_at = None;
        self.trigger_to(true);
        self.start_single();
        self.events.push(Event::Autoshot);
    }
}
