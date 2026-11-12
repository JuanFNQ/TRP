//! A revolver's cylinder: turning, the trigger, reloading and its marks.

use super::*;

impl Firearm {
    // ------------------------------------------------------- a cylinder

    pub(super) fn spring_driven(&self) -> bool {
        self.settings.cylinder.is_some_and(|(_, spring)| spring)
    }

    /// The chamber that fires next, or that loading starts from
    /// (`GetCamoraFireOrLoadStartIndex`): the one under the hammer while
    /// it is cocked, and the next round the cylinder while it is down (a
    /// cylinder a spring turns has always the one under it).
    pub fn camora_in_turn(&self) -> usize {
        if self.hammer_closed && !self.spring_driven() {
            (self.camora + 1) % self.in_chambers.len().max(1)
        } else {
            self.camora
        }
    }

    /// The cylinder turned on by one (`IncrementCamoraIndex`): round and
    /// round for a revolver's, and no further than its last chamber for
    /// one a spring turns.
    fn turn_cylinder(&mut self) {
        let last = self.in_chambers.len().saturating_sub(1);
        self.camora = match (self.camora == last, self.spring_driven()) {
            (true, true) => last,
            (true, false) => 0,
            (false, _) => self.camora + 1,
        };
    }

    /// The cylinder turned on by a chamber by the chamber's key
    /// (`RollCylinderOperation.OnMagPuttedToRig`, after the logic idles:
    /// `IncrementCamoraIndex` not to the first chamber, the animator told
    /// the chamber, the turn's two flags off; firing spec, "The magazine's
    /// and the chamber's keys").
    pub(crate) fn cylinder_rolled(&mut self) {
        self.turn_cylinder();
        self.animator.set("CamoraIndex", self.camora as f32);
        self.animator.set_flag("RollCylinder", false);
        self.animator.set_flag("RollToZeroCamora", false);
    }

    /// The trigger of a weapon with a cylinder, at rest
    /// (`Idling.SetTriggerPressed`, `FireCylinderMagOperation.Start`): a
    /// press fires the chamber in turn, or falls on an empty one.
    pub(crate) fn cylinder_trigger(&mut self, pressed: bool) {
        self.trigger_to(pressed);
        if !pressed {
            self.animator.set_flag("Fire", false);
            return;
        }
        let double = self.mode() == FireMode::DoubleAction;
        let index = self.camora_in_turn();
        self.animator.set("CamoraFireIndex", self.camora as f32);
        if !self.in_chambers.get(index).is_some_and(|chamber| chamber.round) {
            // The hammer falls on nothing (`DryFire`): the cylinder turns
            // all the same where the action turns it, except one a
            // spring turns.
            let turns = (self.hammer_closed && double) || (!self.hammer_closed && self.mode() == FireMode::Single);
            if turns && !self.spring_driven() {
                self.turn_cylinder();
            }
            self.animator.set("DoubleActionFireModeFloat", self.hammer_closed as u8 as f32);
            self.animator.set("CamoraIndex", self.camora as f32);
            self.hammer_closed = double;
            self.animator.set_flag("Fire", true);
            self.events.push(Event::DryFire);
            self.operation = Operation::CylinderFire { fired: true, dry: true };
            return;
        }
        // The round is taken out of its chamber and its case left there,
        // in place of any (`PrepareShot`: `RemoveAmmoInCamora`,
        // `ShellsInChambers`); the cylinder turns on now unless the shot is
        // the first of double action from a cocked hammer.
        self.in_chambers[index] = Chamber { round: false, case: true };
        self.fired_chambers = vec![index];
        let from_closed = self.hammer_closed;
        if self.hammer_closed || !double {
            self.turn_cylinder();
        }
        self.animator.set("CamoraIndex", self.camora as f32);
        self.animator.set_flag("Fire", self.trigger);
        // The shot's state is played from its fifth, as any single shot's
        // (`FireOperation.StartFireAnimation`): the double action's with the
        // hammer down, the plain one's with it cocked (recorded: the Rhino's
        // cocked shot 0.3044 into `FIRE` on its first frame after a frame
        // of 13 ms).
        let state = if from_closed { DOUBLE_ACTION_FIRE.to_owned() } else { self.settings.fire_state.clone() };
        self.animator.play(&state, FIRE_STATE_START);
        self.count_chambers();
        self.animator.set("AmmoInMag", self.rounds() as f32);
        self.operation = Operation::CylinderFire { fired: false, dry: false };
    }

    /// The fire mode of a weapon with a cylinder has been changed
    /// (`SetFireModeForCylinderMagazine`): to single action, the hammer is
    /// cocked, which turns the cylinder on if it was down.
    pub(crate) fn cylinder_mode_changed(&mut self) {
        if self.settings.cylinder.is_none() || self.mode() != FireMode::Single {
            return;
        }
        if self.hammer_closed {
            self.turn_cylinder();
            self.animator.set("CamoraIndex", self.camora as f32);
            self.animator.trigger("HammerArmed");
        }
        self.animator.set("DoubleActionFireModeFloat", 0.0);
        self.hammer_closed = false;
    }

    /// The cylinder's reload begins (`ReloadCylinderMagOperation.Prepare`;
    /// more-weapons spec, "A cylinder", "A cylinder's quick reload, and the
    /// fistful"): which chambers are to be loaded and which hold cases; the
    /// animator is told how many of each and which comes first. `quick`:
    /// the quick reload, which throws the live rounds out with the cases
    /// and loads every chamber. A character who knows the weapon (mastering
    /// above 0) loads a fistful into every chamber where the reload is the
    /// quick one or the cylinder holds no live round; the animator is told
    /// so (`WeaponLevel` 1).
    pub(crate) fn begin_cylinder_reload(&mut self, within_reach: u32, quick: bool) {
        let chambers = self.in_chambers.len();
        let live = self.rounds();
        let fistful = (quick || live == 0) && self.settings.mastering > 0;
        (self.reload_quick, self.reload_fistful) = (quick, fistful);
        self.animator.set("WeaponLevel", fistful as u8 as f32);
        // The chambers to load: for a fistful every one from the first
        // (`GetCamorasIndexesList`); else from the one in turn round (a
        // drum a spring turns from its first, turned back to it:
        // `GetFreeCamorasIndexesFromCurrentActiveIndex`), those without a
        // round, or with the quick reload all of them. The cases are those
        // the weapon holds, whether or not a round lies beside them
        // (`GetShellsIndexes`).
        self.reload_free = if fistful {
            (0..chambers).collect()
        } else {
            if self.spring_driven() {
                self.camora = 0;
            }
            let start = self.camora_in_turn();
            let order: Vec<usize> = if self.spring_driven() { (0..chambers).collect() } else { (start..chambers).chain(0..start).collect() };
            order.into_iter().filter(|index| quick || !self.in_chambers[*index].round).collect()
        };
        self.reload_shells = (0..chambers).filter(|index| self.in_chambers[*index].case).collect();
        // The rounds the cylinder keeps: none of them where the quick
        // reload throws them out.
        let kept = if quick { 0 } else { live };
        (self.reload_loading, self.reload_reach, self.reload_cached, self.reload_remove) = (0, within_reach, kept, -1);
        self.animator.set("ShellsInWeapon", self.reload_shells.len() as f32);
        self.animator.set("AmmoInMag", live as f32);
        if live != 0 {
            // The quick reload names no case: all of them go at once
            // (`SetCamoraIndexForRemoveShell`).
            if quick {
                self.animator.set("ShellsInWeapon", self.reload_shells.len() as f32);
            } else {
                self.next_shell_to_remove();
            }
            self.animator.set("CamoraIndexWithShellForRemove", self.reload_remove as f32);
        }
        // Everything the quick reload throws out, counted for the sounds.
        let out = self.reload_shells.len() as u32 + if quick { live } else { 0 };
        self.animator.set("AmmoCountForRemove", out as f32);
        self.next_chamber_to_load();
        // The base operation's start, as for any weapon filled a round at a
        // time; the quick reload's own trigger in place of the reload's
        // (`ReloadFast`, then `ResetReload`).
        self.animator.set_flag("Inventory", false);
        self.animator.set_flag("CanReload", true);
        self.animator.set_flag("UseLeftHand", false);
        self.animator.trigger(if quick { "ReloadFast" } else { "Reload" });
        self.hands_busy = true;
        self.operation = Operation::ChambersReload { can_abort: false, aborted: false, added: false, hide: None };
        // The class's reload action as the operation begins
        // (`BaseReloadOperation.Start`).
        self.events.push(Event::Earn(Earn::Reload));
    }

    /// `SetCamoraIndexForRemoveShell`.
    fn next_shell_to_remove(&mut self) {
        self.animator.set("ShellsInWeapon", self.reload_shells.len() as f32);
        if !self.reload_shells.is_empty() {
            self.reload_remove = self.reload_shells.remove(0) as i32;
        }
    }

    /// `SetNextCamoraIndexForLoadAmmo`.
    fn next_chamber_to_load(&mut self) {
        if let Some(index) = self.reload_free.get(self.reload_loading) {
            self.animator.set("CamoraIndexForLoadAmmo", *index as f32);
        }
    }

    /// A round of the cylinder's reload in (`AddAmmoToMag`): the next
    /// chamber named, the reload may be cut short from now, and where no
    /// round follows or it has been cut short it ends (`EndReload`): the
    /// animator told the chamber in turn, `CanReload` off, and whether it
    /// was cut short (`MasteringReloadAborted`, by which a fistful's
    /// animator puts back the rounds still in the hand).
    fn add_ammo_to_mag(&mut self) {
        let Operation::ChambersReload { aborted, added, hide, .. } = self.operation else { return };
        self.next_chamber_to_load();
        self.operation = Operation::ChambersReload { can_abort: true, aborted, added, hide };
        self.animator.set("AmmoInMag", (self.reload_cached + self.reload_loading as u32) as f32);
        if !self.cylinder_goes_on() || aborted {
            self.animator.set_flag("MasteringReloadAborted", aborted);
            self.animator.set("CamoraIndex", self.camora as f32);
            self.animator.set_flag("CanReload", false);
        }
    }

    /// Whether another round is to follow (`CanReload`).
    fn cylinder_goes_on(&self) -> bool {
        self.reload_reach > self.reload_loading as u32 && self.reload_cached + self.reload_loading as u32 != self.in_chambers.len() as u32
    }

    /// A mark of the animator's while a cylinder fires or is reloaded.
    /// Whether it was this operation's to answer.
    pub(crate) fn cylinder_mark(&mut self, name: &str) -> bool {
        let double = self.mode() == FireMode::DoubleAction;
        match (name, self.operation.clone()) {
            ("FiringBullet", Operation::CylinderFire { fired: false, dry }) => {
                self.fired_chambers.clear();
                self.operation = Operation::CylinderFire { fired: true, dry };
                self.events.push(Event::FireSound { trigger: self.trigger, doublet: false });
                // The hammer as the shot is made: down for a double action
                // pull from a hammer left down, which `AddAmmoInChamber`
                // leaves down after each shot in double action.
                self.shot_made = true;
                self.shot_hammer_down = self.hammer_closed;
            }
            ("AddAmmoInChamber", Operation::CylinderFire { .. }) => {
                self.animator.set("AmmoInMag", self.rounds() as f32);
                self.hammer_closed = double;
                self.animator.set("DoubleActionFireModeFloat", self.hammer_closed as u8 as f32);
            }
            ("FireEnd", Operation::CylinderFire { dry: true, .. }) => {
                self.animator.set("DoubleActionFireModeFloat", self.hammer_closed as u8 as f32);
            }
            ("IdleStart", Operation::CylinderFire { fired: true, .. }) => {
                self.trigger_to(false);
                self.animator.set_flag("Fire", false);
                self.to_idle();
            }
            // The case stays in its chamber: nothing is thrown out.
            ("ShellEject" | "DelAmmoChamber", Operation::CylinderFire { .. }) => {}
            ("RemoveShell", Operation::ChambersReload { .. }) => {
                if self.rounds() == 0 || self.reload_quick {
                    for chamber in &mut self.in_chambers {
                        chamber.case = false;
                    }
                    self.reload_shells.clear();
                    self.animator.set("ShellsInWeapon", 0.0);
                    // The quick reload throws the live rounds out too, each
                    // to the world as a thing (`RemoveAllAmmo`).
                    for index in 0..self.in_chambers.len() {
                        if std::mem::take(&mut self.in_chambers[index].round) {
                            self.events.push(Event::RoundDropped { chamber: index });
                        }
                    }
                    self.count_chambers();
                } else {
                    if let Some(chamber) = usize::try_from(self.reload_remove).ok().and_then(|index| self.in_chambers.get_mut(index)) {
                        chamber.case = false;
                    }
                    self.next_shell_to_remove();
                }
            }
            ("ShellEject", Operation::ChambersReload { .. }) => {
                self.animator.set("CamoraIndexWithShellForRemove", self.reload_remove as f32);
            }
            // A round shows in the hand for the next free chamber; of a
            // fistful, the next of those shown goes in, and the last ends it
            // (`AddAmmoInHighMasteringReload`).
            ("AddAmmoInChamber", Operation::ChambersReload { .. }) => {
                let more = self.reload_loading < self.reload_free.len() && (self.reload_loading as u32) < self.reload_reach;
                if self.reload_fistful {
                    if more {
                        self.reload_loading += 1;
                        self.animator.set("AmmoInMag", self.reload_loading as f32);
                        if self.reload_loading >= self.reload_free.len() || self.reload_loading as u32 >= self.reload_reach {
                            self.add_ammo_to_mag();
                        }
                    }
                } else if more {
                    self.animator.set("CamoraIndex", self.camora as f32);
                    self.animator.set("CamoraIndexForLoadAmmo", self.reload_free[self.reload_loading] as f32);
                    self.reload_loading += 1;
                }
                self.animator.set("AmmoInMag", (self.reload_cached + self.reload_loading as u32) as f32);
            }
            // It is in: another follows, or the cylinder is shut.
            ("AddAmmoInMag", Operation::ChambersReload { .. }) => self.add_ammo_to_mag(),
            // A fistful shown: the cylinder turned to its first chamber, or
            // with the hammer down to its last (`SetAllAmmoToHandsWithMasteringReload`).
            ("MagShow", Operation::ChambersReload { .. }) if self.reload_fistful => {
                self.camora = if self.hammer_closed { self.in_chambers.len().saturating_sub(1) } else { 0 };
            }
            // The cylinder shut and the hand back: the rounds shown are
            // the weapon's from now (`OnMagPuttedToRig`, `Commit`).
            ("MagHide", Operation::ChambersReload { hide, .. }) => {
                self.animator.set("CamoraIndex", self.camora as f32);
                self.animator.set_flag("CanReload", false);
                self.animator.set_flag("MasteringReloadAborted", false);
                (self.reload_quick, self.reload_fistful) = (false, false);
                let loaded = self.reload_loading;
                for index in self.reload_free.iter().take(loaded) {
                    self.in_chambers[*index].round = true;
                }
                if loaded > 0 {
                    self.events.push(Event::RoundsTaken(loaded as u32));
                    // The reload's end with a round loaded: the class's
                    // reload action once more (`ReloadCylinderMagOperation.
                    // EndReload`, as the hand is put away or after the last
                    // round; here at the former).
                    self.events.push(Event::Earn(Earn::Reload));
                }
                // With the hammer cocked the first loaded chamber from
                // the one under it comes under the hammer.
                if !self.hammer_closed {
                    let chambers = self.in_chambers.len();
                    let next = if self.spring_driven() { Some(0) } else { (0..chambers).map(|step| (self.camora + step) % chambers).find(|index| self.in_chambers[*index].round) };
                    self.camora = next.unwrap_or(self.camora);
                }
                self.count_chambers();
                self.to_idle();
                if let Some(fast) = hide {
                    self.hide(fast);
                }
            }
            _ => return false,
        }
        true
    }
}
