//! Barrels loaded one by one, the rocket's trigger and their marks.

use super::*;

impl Firearm {
    // ---------------------------------------------------------- barrels

    /// The trigger of a weapon whose barrels are loaded one by one, at
    /// rest (`FireOnlyBarrelFireOperation.PrepareShot`): one barrel a
    /// press, the first that is loaded, or all of them together in the
    /// mode for that.
    pub(crate) fn barrels_trigger(&mut self, pressed: bool) {
        self.trigger_to(pressed);
        let loaded: Vec<usize> = (0..self.in_chambers.len()).filter(|index| self.in_chambers[*index].round).collect();
        if !pressed || loaded.is_empty() {
            // The animator has the trigger, and a press only clicks.
            self.animator.set_flag("Fire", pressed);
            if pressed {
                self.events.push(Event::DryFire);
            }
            return;
        }
        // Each round fired out of its barrel (`SingleShot`: the slot
        // emptied); its case is the weapon's at the shot's mark.
        let fired = if self.mode() == FireMode::Doublet { loaded.len() } else { 1 };
        self.fired_chambers = loaded.into_iter().take(fired).collect();
        for index in self.fired_chambers.clone() {
            self.in_chambers[index].round = false;
        }
        self.count_chambers();
        self.animator.set_flag("Fire", self.trigger);
        self.animator.play(&self.settings.fire_state.clone(), FIRE_STATE_START);
        self.operation = Operation::BarrelsFire { fired: false, shots: fired as u32 };
    }

    /// The rounds the game's weapon holds while its barrels are reloaded:
    /// those to come count from the reload's start, the game's inventory
    /// having put them into the barrels before the hands begin
    /// (`ReloadMultiBarrelResult.Run`). It is this that takes an empty
    /// weapon out of its resting state: the animator's own way out for
    /// one with neither round nor case can never be taken.
    fn rounds_once_loaded(&self) -> f32 {
        (self.rounds() + self.reload_free.len() as u32) as f32
    }

    /// The barrels' reload begins (`ReloadMultiBarrelOperation.Start`):
    /// every barrel that holds no round is to have one, as far as the
    /// rounds in reach go.
    pub(crate) fn begin_barrels_reload(&mut self, within_reach: u32) {
        if self.in_chambers.len() == 1 {
            return self.begin_single_barrel_reload(within_reach);
        }
        self.reload_free = (0..self.in_chambers.len()).filter(|index| !self.in_chambers[*index].round).take(within_reach as usize).collect();
        if self.reload_free.is_empty() {
            return;
        }
        // The animator is told which barrel is loaded, or with all of
        // them how many there are.
        let which = if self.reload_free.len() == self.in_chambers.len() { self.in_chambers.len() } else { self.reload_free[self.reload_free.len() - 1] };
        self.animator.set("ChamberIndex", which as f32);
        self.animator.set_flag("Inventory", false);
        self.animator.set_flag("CanReload", true);
        self.animator.trigger("Reload");
        self.animator.set("ShellsInWeapon", self.cases() as f32);
        self.animator.set("AmmoInChamber", self.rounds_once_loaded());
        self.hands_busy = true;
        self.operation = Operation::ChambersReload { can_abort: false, aborted: false, added: false, hide: None };
        // The class's reload action as the operation begins
        // (`BaseReloadOperation.Start`), its only one.
        self.events.push(Event::Earn(Earn::Reload));
    }

    /// A single barrel's reload (`ReloadSingleBarrelOperation.Start`;
    /// more-weapons spec, "One barrel loaded alone, and the rocket
    /// launcher"): taken whatever the barrel holds, as far as a round is in
    /// reach. The game's inventory makes the exchange now: a live round in
    /// the barrel is taken out and one of those in reach goes in.
    fn begin_single_barrel_reload(&mut self, within_reach: u32) {
        if within_reach == 0 {
            return;
        }
        self.reload_unloads = self.in_chambers[0].round;
        self.reload_free = vec![0];
        self.animator.set_flag("Inventory", false);
        self.animator.set("ShellsInWeapon", self.cases() as f32);
        // With a live round in the barrel the weapon's case goes now; the
        // model's at `RemoveShell`, unthrown.
        if self.reload_unloads {
            self.in_chambers[0].case = false;
        }
        self.animator.set("AmmoInChamber", if self.reload_unloads { 1.0 } else { 0.0 });
        self.animator.set_flag("CanReload", true);
        self.animator.trigger("Reload");
        self.events.push(Event::RoundsTaken(1));
        self.hands_busy = true;
        self.operation = Operation::ChambersReload { can_abort: false, aborted: false, added: false, hide: None };
        // The class's reload action as the operation begins
        // (`BaseReloadOperation.Start`), its only one.
        self.events.push(Event::Earn(Earn::Reload));
    }

    /// A rocket launcher's trigger at rest (`Idling.SetTriggerPressed`,
    /// `RocketLauncherFire.Start`): refused just after the sights have come
    /// down, until the resting state begins again; otherwise, with its
    /// rocket in, the firing begins: the animator's `Fire` follows the
    /// trigger and the sights are put down. The animator fires from the
    /// sights only (its resting state's way to firing waits for
    /// `IsAimingFloat`), and until it does the firing waits.
    pub(crate) fn rocket_trigger(&mut self, pressed: bool) {
        if pressed && !self.aiming && self.aim_flags {
            return;
        }
        self.trigger_to(pressed);
        self.animator.set_flag("Fire", pressed);
        if pressed && self.in_chambers.first().is_some_and(|chamber| chamber.round) {
            self.aiming = false;
            self.operation = Operation::BarrelsFire { fired: false, shots: 1 };
            // At the fire mark its rocket's model goes to the port, the
            // whole of it (`MoveAmmoFromChamberToShellPort`: `SetUsed` finds
            // no `shell` nor `full` objects on a rocket), and stays there.
            self.fired_chambers = vec![0];
        }
    }

    /// A mark of the animator's while barrels fire or are reloaded.
    pub(crate) fn barrels_mark(&mut self, name: &str) -> bool {
        let (single, rocket) = (self.in_chambers.len() == 1, self.settings.rocket_launcher);
        match (name, self.operation.clone()) {
            // A rocket launcher's shot (`RocketLauncherFire`): the rocket
            // leaves at the fire mark; at the shot's end the trigger is
            // let go and the weapon idles.
            ("FiringBullet", Operation::BarrelsFire { fired: false, shots }) if rocket => {
                self.operation = Operation::BarrelsFire { fired: true, shots };
                self.in_chambers[0].round = false;
                self.count_chambers();
                self.events.push(Event::Shot { strength: self.settings.shot_strength, hammer_down: false });
                self.events.push(Event::FireSound { trigger: self.trigger, doublet: false });
            }
            ("FireEnd", Operation::BarrelsFire { fired: true, .. }) if rocket => {
                self.trigger_to(false);
                self.animator.set_flag("Fire", false);
                self.animator.set("AmmoInChamber", 0.0);
                self.to_idle();
            }
            // A single barrel's reload: the live round counts as the barrel
            // stands, comes out at the case's mark (to the vest or the
            // ground, as the host finds room), and the new round in ends it.
            ("DelAmmoChamber", Operation::ChambersReload { .. }) if single => self.animator.set("AmmoInChamber", 1.0),
            ("ShellEject", Operation::ChambersReload { .. }) if single => {
                self.animator.set_flag("CanReload", true);
                if std::mem::take(&mut self.reload_unloads) {
                    self.in_chambers[0].round = false;
                    self.count_chambers();
                    self.events.push(Event::RoundUnloaded);
                }
                self.animator.set("AmmoInChamber", 0.0);
            }
            ("AddAmmoInChamber", Operation::ChambersReload { added: false, hide, .. }) if single => {
                self.reload_free.clear();
                self.in_chambers[0].round = true;
                self.count_chambers();
                self.animator.set("AmmoInChamber", 1.0);
                self.animator.set_flag("CanReload", false);
                self.to_idle();
                if let Some(fast) = hide {
                    self.hide(fast);
                }
            }
            ("FiringBullet", Operation::BarrelsFire { fired: false, shots }) => {
                self.operation = Operation::BarrelsFire { fired: true, shots };
                // Each barrel fired holds its round's case from now
                // (`OnFireEvent`: `ShellsInChambers`).
                for index in std::mem::take(&mut self.fired_chambers) {
                    if let Some(chamber) = self.in_chambers.get_mut(index) {
                        chamber.case = true;
                    }
                }
                for _ in 0..shots {
                    self.events.push(Event::Shot { strength: self.settings.shot_strength, hammer_down: false });
                }
                // Each chamber's shot sounds (`Shot`, `multiShot` with more
                // than one), the trigger still held.
                for _ in 0..shots {
                    self.events.push(Event::FireSound { trigger: self.trigger, doublet: shots > 1 });
                }
                self.trigger_to(false);
                self.animator.set_flag("Fire", false);
                self.animator.set("AmmoInChamber", self.rounds() as f32);
                self.animator.set("ShellsInWeapon", self.cases() as f32);
            }
            ("IdleStart", Operation::BarrelsFire { fired: true, .. }) => self.to_idle(),
            ("ShellEject" | "DelAmmoChamber" | "AddAmmoInChamber", Operation::BarrelsFire { .. }) => {}
            // The cases are out.
            ("RemoveShell", Operation::ChambersReload { .. }) => {
                for chamber in &mut self.in_chambers {
                    chamber.case = false;
                }
                self.animator.set("ShellsInWeapon", 0.0);
            }
            ("ShellEject", Operation::ChambersReload { .. }) => {
                self.animator.set_flag("CanReload", true);
                self.animator.set("AmmoInChamber", self.rounds_once_loaded());
            }
            // The rounds are in their barrels, answered once.
            ("AddAmmoInChamber", Operation::ChambersReload { added: false, can_abort, aborted, hide }) => {
                let loaded = self.reload_free.len();
                for index in std::mem::take(&mut self.reload_free) {
                    self.in_chambers[index].round = true;
                }
                self.events.push(Event::RoundsTaken(loaded as u32));
                self.count_chambers();
                self.animator.set("AmmoInChamber", self.rounds() as f32);
                self.operation = Operation::ChambersReload { can_abort, aborted, added: true, hide };
            }
            // The weapon shut: the reload hands over to idling. A put-away
            // asked for meanwhile goes straight to its own
            // (`ReloadMultiBarrelOperation.HideWeapon`), keeping no aim.
            ("AddAmmoInMag", Operation::ChambersReload { hide, .. }) => {
                self.animator.set_flag("CanReload", false);
                self.to_idle();
                if let Some(fast) = hide {
                    self.hide_from(fast, false);
                }
            }
            _ => return false,
        }
        true
    }
}
