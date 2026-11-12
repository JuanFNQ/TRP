//! Switching to the launcher, its trigger, and reloading, loading and discharging it.

use super::*;

impl Firearm {
    /// `ToggleLauncherOperation.Start`.
    pub(crate) fn begin_switch(&mut self, on: bool, hide: Option<bool>) {
        // The rifle's animator, which the switch keeps (`FirearmsAnimator.SetLauncher`:
        // `LauncherID` is always -1).
        self.animator.set_flag("LauncherReady", on);
        self.animator.set("LauncherID", -1.0);
        let Some(launcher) = self.launcher.as_mut() else { return };
        if on {
            // Enabled, it starts as its controller has it (the engine's animator
            // enabled afresh); then it is the hands'.
            launcher.animator.reset();
            launcher.speaks = true;
        }
        launcher.animator.set_flag("Active", on);
        launcher.marked = on;
        self.events.push(Event::Launcher(LauncherEvent::SwitchBegun(on)));
        if on {
            self.ends_blind_fire();
        }
        self.operation = Operation::ToggleLauncher { on, hide };
    }

    /// The launcher's rest begins (`LauncherIdling.Start`).
    pub(super) fn launcher_rest(&mut self) {
        self.operation = Operation::LauncherIdle;
        let inventory = self.inventory;
        let Some(launcher) = self.launcher.as_mut() else { return };
        let animator = &mut launcher.animator;
        animator.set_flag("UseLeftHand", false);
        animator.set("AmmoInChamber", launcher.round as u8 as f32);
        animator.set_flag("Inventory", inventory);
        animator.set_flag("CheckChamber", false);
        animator.set_flag("Fire", false);
    }

    /// A put-away or a change of weapon with the launcher in hand
    /// (`LauncherIdling.HideWeapon`): the sights down, the trigger let go, and the
    /// switch back, whose end makes the put-away. Asked in the switch or the
    /// reload, it waits for their end; in the shot it is planned, and the shot's end
    /// does not look at the plan (as read: lost); in a round moved from the
    /// inventory it is not taken.
    pub(crate) fn launcher_hide(&mut self, fast: bool) {
        match &mut self.operation {
            Operation::LauncherIdle | Operation::LauncherUtility { .. } => {
                self.aiming = false;
                self.trigger_to(false);
                self.begin_switch(false, Some(fast));
            }
            Operation::ToggleLauncher { hide, .. } | Operation::LauncherReload { hide } => *hide = Some(fast),
            _ => {}
        }
    }

    /// The inventory's screen opened or shut with a launcher's operation in hand. Its
    /// rest and its looks put the sights down, let the trigger go and tell its
    /// animator (`LauncherIdling.SetInventoryOpened`); its shot and its reload only
    /// note it for themselves, and the rest that follows them reads the hands' own
    /// note, which they did not change (as read); the switch and a round moved from
    /// the inventory do not take it.
    pub(crate) fn launcher_inventory(&mut self, open: bool) {
        if !matches!(self.operation, Operation::LauncherIdle | Operation::LauncherUtility { .. }) {
            return;
        }
        self.aiming = false;
        self.trigger_to(false);
        self.inventory = open;
        if let Some(launcher) = self.launcher.as_mut() {
            launcher.animator.set_flag("Fire", false);
            launcher.animator.set_flag("Inventory", open);
        }
    }

    /// The trigger on the launcher's rest or in its shot.
    pub(crate) fn launcher_trigger(&mut self, pressed: bool) {
        match self.operation {
            Operation::LauncherIdle => {
                // Dropped where the movement gives the hands no say.
                if pressed && !self.hands_free {
                    return;
                }
                self.trigger_to(pressed);
                let loaded = self.launcher.as_ref().is_some_and(|launcher| launcher.round);
                if pressed && loaded {
                    return self.launcher_fire();
                }
                // Nothing in it: its animator has the trigger.
                if let Some(launcher) = self.launcher.as_mut() {
                    launcher.animator.set_flag("Fire", pressed);
                }
                if pressed {
                    self.events.push(Event::DryFire);
                }
            }
            Operation::LauncherFire { .. } => {
                self.trigger_to(self.trigger && pressed);
                let trigger = self.trigger;
                if let Some(launcher) = self.launcher.as_mut() {
                    launcher.animator.set_flag("Fire", trigger);
                }
            }
            // The look all over is ended by the trigger, which the rest takes on.
            Operation::LauncherUtility { examine: true, .. } => {
                self.hands_busy = false;
                self.launcher_rest();
                self.launcher_trigger(pressed);
            }
            _ => {}
        }
    }

    /// `LauncherFire.Start`: the fire state at a fifth of its clip at once, its `Fire`
    /// with the trigger.
    fn launcher_fire(&mut self) {
        let trigger = self.trigger;
        if let Some(launcher) = self.launcher.as_mut() {
            launcher.animator.set_flag("BoltActionReload", true);
            launcher.animator.set_flag("Fire", trigger);
            launcher.animator.play("Hands.FIRE", super::super::FIRE_STATE_START);
        }
        self.operation = Operation::LauncherFire { fired: false };
    }

    /// The reload key with the launcher in use (`FirearmHandsInputTranslator.Reload`,
    /// `LauncherIdling.ReloadGrenadeLauncher`): `in_reach` that a round the launcher
    /// takes is within reach. Only an empty chamber is reloaded (a spent case alone is
    /// empty); with a live round in it nothing happens. Whether a reload began.
    pub fn reload_launcher(&mut self, in_reach: bool) -> bool {
        if !self.in_launcher_mode() {
            return false;
        }
        // A mount is left first.
        self.events.push(Event::LeaveMount { at_once: false });
        if !in_reach {
            self.events.push(Event::Launcher(LauncherEvent::NoRounds));
            return false;
        }
        // The rifle is asked first (`CanStartReload`), then the operation in hand.
        if self.blind_fire || self.rifle_refuses_reload() || !matches!(self.operation, Operation::LauncherIdle) {
            return false;
        }
        let Some(launcher) = self.launcher.as_mut() else { return false };
        if launcher.round {
            return false;
        }
        self.aiming = false;
        launcher.animator.set_flag("Fire", false);
        launcher.animator.trigger("Reload");
        // The round goes in now (`Prepare`: the inventory's move made at once).
        launcher.round = true;
        self.events.push(Event::Launcher(LauncherEvent::RoundTaken));
        self.hands_busy = true;
        self.operation = Operation::LauncherReload { hide: None };
        true
    }

    /// The rifle's own refusal of any reload (`FirearmController.CanStartReload`): a
    /// failure to feed, or a weapon whose bolt has to be opened for a reload with any
    /// malfunction but a misfire, is reminded of (`MisfireSlideUnknown` off on the
    /// hands' animator, `ExamineMalfunction`).
    fn rifle_refuses_reload(&mut self) -> bool {
        let malf = self.malfunction();
        if malf == Malf::Feed || (self.settings.must_open_bolt && malf != Malf::None && malf != Malf::Misfire) {
            if let Some(launcher) = self.launcher.as_mut() {
                launcher.animator.set_flag("MisfireSlideUnknown", false);
            }
            self.remind();
            return true;
        }
        false
    }

    /// A round put into the launcher from the inventory. On the launcher's rest it
    /// is loaded (`LoadAmmoToUnderbarrelWeapon`: the inventory's move completes at
    /// its end, `LauncherEvent::MoveDone`); on the rifle's the switch is made first,
    /// and the hands stay on the launcher. Whether it began.
    pub fn load_launcher(&mut self) -> bool {
        match self.operation {
            Operation::LauncherIdle => {
                self.begin_load();
                true
            }
            Operation::Idle if self.launcher.is_some() => {
                self.animator.set_flag("Inventory", false);
                self.begin_switch(true, None);
                if let Some(launcher) = self.launcher.as_mut() {
                    launcher.then = Some(true);
                }
                true
            }
            _ => false,
        }
    }

    /// The launcher's round taken out, by the chamber's key once the host has found
    /// it a place (the vest's or the pockets' grids, the smallest first), or from the
    /// inventory (`DischargeAmmoFromUnderbarrelWeapon`; the switch first from the
    /// rifle's rest, as `load_launcher`). Whether it began.
    pub fn discharge_launcher(&mut self) -> bool {
        let loaded = self.launcher.as_ref().is_some_and(|launcher| launcher.round);
        if !loaded {
            return false;
        }
        match self.operation {
            Operation::LauncherIdle => {
                self.begin_discharge();
                true
            }
            Operation::Idle => {
                self.animator.set_flag("Inventory", false);
                self.begin_switch(true, None);
                if let Some(launcher) = self.launcher.as_mut() {
                    launcher.then = Some(false);
                }
                true
            }
            _ => false,
        }
    }

    pub(super) fn begin_load(&mut self) {
        self.aiming = false;
        self.hands_busy = true;
        // The rifle's skill counts the chamber's action as the move begins
        // (`LauncherIdling.ExecuteLauncherOperation`).
        self.events.push(Event::Earn(super::super::Earn::Chamber));
        if let Some(launcher) = self.launcher.as_mut() {
            launcher.animator.set_flag("Fire", false);
            launcher.animator.set_flag("LoadOne", true);
        }
        self.operation = Operation::LauncherLoad;
    }

    pub(super) fn begin_discharge(&mut self) {
        self.aiming = false;
        self.hands_busy = true;
        if let Some(launcher) = self.launcher.as_mut() {
            launcher.animator.set_flag("Discharge", true);
            launcher.animator.set_flag("Fire", false);
        }
        self.events.push(Event::Earn(super::super::Earn::Chamber));
        self.operation = Operation::LauncherDischarge;
    }
}
