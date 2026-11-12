//! Loading and unloading a chamber of its own from the inventory.

use super::*;

impl Firearm {
    // ------------------------------- a round moved from the inventory

    /// Whether the hands would take a round the inventory puts into one of
    /// the weapon's chambers of its own now (`Idling.Execute`,
    /// `ExecuteCylinderMagOperations`; more-weapons spec, "A round moved
    /// into a chamber of its own, or out of one, from the inventory"): the
    /// logic idles and the chambers are loaded by hand (`isChamberLoad`).
    /// Neither a cylinder nor barrels have a magazine apart that refuses it.
    /// Not a rocket launcher's (its animator has no state its `LoadOne`
    /// leads to: not built, `PROVISIONAL` that the game would refuse it).
    pub fn can_load_own_chamber(&self) -> bool {
        self.has_chambers() && self.can_move_magazine() && self.settings.chamber_load && !self.settings.rocket_launcher && self.animator.has("LoadOne")
    }

    /// Whether the hands would let a round the inventory takes out of one
    /// of the weapon's chambers of its own go now: the logic idles (the
    /// game asks nothing else of a cylinder or of barrels). Not a rocket
    /// launcher's, as for a load.
    pub fn can_unload_own_chamber(&self) -> bool {
        self.has_chambers() && self.can_move_magazine() && !self.settings.rocket_launcher && self.animator.has("Discharge")
    }

    /// The animator told which barrel holds a case
    /// (`FirearmOperation.method_3`, as a discharge or a load begins): the
    /// last of them, or with every barrel holding one their count, or with
    /// none -1.
    fn tell_barrel_with_case(&mut self) {
        let with: Vec<usize> = (0..self.in_chambers.len()).filter(|index| self.in_chambers[*index].case).collect();
        let value = match with.last() {
            None => -1.0,
            Some(_) if with.len() == self.in_chambers.len() => with.len() as f32,
            Some(last) => *last as f32,
        };
        self.animator.set("ChamberIndexWithShell", value);
    }

    /// A round the inventory has just put into chamber `index` of the
    /// weapon's own, which the logic does not have yet: the hands put it
    /// in. A cylinder's (`LoadCartridgeToCamora.Start`): `LoadOne`, the
    /// chamber and whether its case is to come out first
    /// (`CamoraIndexForLoadAmmo`, `CamoraIndexWithShellForRemove`), the
    /// rounds without it (the game's cylinder holds it already and tells
    /// one fewer), `CanReload`; it ends with the cylinder shut (`MagHide`).
    /// A barrel's (`LoadCartridgeToChamber.Start`, by the barrel's number):
    /// `LoadOne`, `ChamberIndex`, which barrel holds a case; it ends with
    /// the round in (`AddAmmoInChamber`). Both: the sights down, the trigger
    /// let go, blind fire over, the body told the hands reload. False, and
    /// nothing done, where the hands would not take it
    /// (`can_load_own_chamber`) or the chamber holds a round.
    pub fn load_own_chamber(&mut self, index: usize) -> bool {
        if !self.can_load_own_chamber() || self.in_chambers.get(index).is_none_or(|chamber| chamber.round) {
            return false;
        }
        self.aiming = false;
        self.trigger_to(false);
        self.animator.set_flag("Fire", false);
        self.ends_blind_fire();
        // (`LoadOneTrigger` and the chamber's setter let the left hand go:
        // `ResetLeftHand`.)
        self.animator.set_flag("UseLeftHand", false);
        self.animator.set_flag("LoadOne", true);
        if self.settings.cylinder.is_some() {
            let case = self.in_chambers[index].case;
            self.animator.set("CamoraIndexWithShellForRemove", if case { index as f32 } else { -1.0 });
            self.animator.set("ShellsInWeapon", self.cases() as f32);
            self.animator.set("CamoraIndexForLoadAmmo", index as f32);
            self.animator.set("AmmoInMag", self.rounds() as f32);
            self.animator.set_flag("CanReload", true);
        } else {
            self.animator.set("ChamberIndex", index as f32);
            self.tell_barrel_with_case();
        }
        (self.hands_busy, self.reload_flag) = (true, true);
        self.operation = Operation::OwnChamberIn { chamber: index, loaded: false };
        // The class's chamber action as the operation begins (`Idling.
        // ExecuteCylinderMagOperations`, `Idling.Execute`), but for a round
        // moved from another of its chambers, whose taking out raised it.
        if !std::mem::take(&mut self.moving_round) {
            self.events.push(Event::Earn(Earn::Chamber));
        }
        true
    }

    /// The round of chamber `index` of the weapon's own, which the
    /// inventory has just taken out, taken out by the hands; `onward`, it
    /// goes into another of its chambers, which the hands load next (the
    /// inventory's move, `MoveOperation`, asks the hands to let it out,
    /// makes it, then asks them to put it in). A cylinder's
    /// (`DischargeAmmoFromCamoraOperation.Start`): `Discharge`, the chamber
    /// (`CamoraIndexForRemoveAmmo`); out at `MagShow`, over then where it
    /// goes on, else at `MagHide`. A barrel's (`DischargeOperation.Start`):
    /// a misfire put right, `Discharge`, `ChamberIndex`, `LoadOne` where it
    /// goes on, which barrel holds a case; out and over at `MagHide`, or of
    /// two barrels or more out at `DelAmmoChamber`. Both: the sights down,
    /// the trigger let go, blind fire over, the body told the hands reload.
    /// False, and nothing done, where the hands would not let it go
    /// (`can_unload_own_chamber`) or the chamber holds no round.
    pub fn unload_own_chamber(&mut self, index: usize, onward: bool) -> bool {
        if !self.can_unload_own_chamber() || !self.in_chambers.get(index).is_some_and(|chamber| chamber.round) {
            return false;
        }
        let cylinder = self.settings.cylinder.is_some();
        if !cylinder {
            self.clear_misfire();
        }
        self.aiming = false;
        self.animator.set_flag("Discharge", true);
        self.trigger_to(false);
        self.animator.set_flag("Fire", false);
        self.ends_blind_fire();
        if cylinder {
            self.animator.set("CamoraIndexForRemoveAmmo", index as f32);
        } else {
            self.animator.set_flag("UseLeftHand", false);
            self.animator.set("ChamberIndex", index as f32);
            self.animator.set_flag("LoadOne", onward);
            self.tell_barrel_with_case();
        }
        (self.hands_busy, self.reload_flag) = (true, true);
        self.operation = Operation::OwnChamberOut { chamber: index, onward, out: false };
        // The class's chamber action as the operation begins (`Idling.
        // ExecuteCylinderMagOperations`: `DischargeAmmoFromCamoraOperation`;
        // `Idling.Execute`: `DischargeOperation`), once for a round moved on
        // into another chamber.
        self.moving_round = onward;
        self.events.push(Event::Earn(Earn::Chamber));
        true
    }

    /// The round the hands load is in its chamber, and shown at its point
    /// (`SetRoundIntoWeapon`).
    pub(crate) fn own_chamber_loaded(&mut self, index: usize) {
        if let Some(chamber) = self.in_chambers.get_mut(index) {
            chamber.round = true;
        }
        if let Some(shown) = self.shown.get_mut(index) {
            shown.round = true;
        }
        self.count_chambers();
        self.events.push(Event::Chambered);
    }

    /// The round the hands take out has left its chamber, and its point
    /// (`DestroyPatronInWeapon`).
    fn own_chamber_emptied(&mut self, index: usize) {
        if let Some(chamber) = self.in_chambers.get_mut(index) {
            chamber.round = false;
        }
        if let Some(shown) = self.shown.get_mut(index) {
            shown.round = false;
        }
        self.count_chambers();
        self.events.push(Event::Discharged);
    }

    /// A cylinder's operation from the inventory over
    /// (`LoadCartridgeToCamora.SwitchToIdle`, `DischargeAmmoFromCamoraOperation.
    /// DischargeEnd`): a drum a spring turns back to its first chamber
    /// (`ResetCamoraIndex`, which a revolver's cylinder leaves be), the
    /// animator told the chamber in turn, and the logic idles.
    pub(crate) fn own_cylinder_at_rest(&mut self) {
        if self.spring_driven() {
            self.camora = 0;
        }
        self.animator.set("CamoraIndex", self.camora as f32);
        self.to_idle();
    }

    /// A mark of the animator's while a round is moved into a chamber of
    /// its own or out of one from the inventory. Whether it was this
    /// operation's to answer.
    pub(crate) fn own_chamber_mark(&mut self, name: &str) -> bool {
        let cylinder = self.settings.cylinder.is_some();
        let several = self.in_chambers.len() > 1;
        match (name, self.operation.clone()) {
            // A cylinder's load: the chamber's case out first, unthrown
            // (`OnRemoveShellEvent`: `RemoveShellInWeapon`); the round in at
            // `AddAmmoInChamber`; `CanReload` taken away at `AddAmmoInMag`;
            // over with the cylinder shut.
            ("RemoveShell", Operation::OwnChamberIn { chamber, .. }) if cylinder => {
                if let Some(held) = self.in_chambers.get_mut(chamber) {
                    held.case = false;
                }
                if let Some(shown) = self.shown.get_mut(chamber) {
                    shown.case = false;
                }
                self.animator.set("ShellsInWeapon", self.cases() as f32);
            }
            ("AddAmmoInChamber", Operation::OwnChamberIn { chamber, loaded: false }) if cylinder => {
                self.own_chamber_loaded(chamber);
                self.operation = Operation::OwnChamberIn { chamber, loaded: true };
                self.animator.set("AmmoInMag", self.rounds() as f32);
                self.animator.set("ShellsInWeapon", self.cases() as f32);
                self.animator.set_flag("CanReload", true);
                self.animator.set_flag("LoadOne", false);
            }
            ("AddAmmoInMag", Operation::OwnChamberIn { .. }) if cylinder => self.animator.set_flag("CanReload", false),
            ("MagHide", Operation::OwnChamberIn { chamber, loaded }) if cylinder => {
                if !loaded {
                    self.own_chamber_loaded(chamber);
                }
                self.own_cylinder_at_rest();
            }
            // A barrel's load: every case out, unthrown (`OnRemoveShellEvent`:
            // `RemoveAllShells`); the round shown as the hand brings it
            // (`OnMagAppeared`), in at `AddAmmoInChamber`, which ends it.
            ("RemoveShell", Operation::OwnChamberIn { .. }) => {
                for held in &mut self.in_chambers {
                    held.case = false;
                }
                for shown in &mut self.shown {
                    shown.case = false;
                }
                self.animator.set("ShellsInWeapon", 0.0);
            }
            ("MagShow", Operation::OwnChamberIn { chamber, .. }) => {
                if let Some(shown) = self.shown.get_mut(chamber) {
                    shown.round = true;
                }
            }
            ("AddAmmoInChamber", Operation::OwnChamberIn { chamber, loaded: false }) => {
                self.own_chamber_loaded(chamber);
                // A barrel's round in: Mag Drills' round loaded
                // (`LoadCartridgeToChamber.OnAddAmmoInChamber`).
                self.events.push(Event::Earn(Earn::RoundLoaded));
                self.animator.set("AmmoInChamber", self.rounds() as f32);
                self.animator.set("ShellsInWeapon", self.cases() as f32);
                self.animator.set_flag("CanReload", false);
                self.animator.set_flag("LoadOne", false);
                self.to_idle();
            }
            // A cylinder's round out as the hand shows it
            // (`OnMagAppeared`); over then where it goes into another
            // chamber, else as the hand puts it away (`OnMagPuttedToRig`).
            ("MagShow", Operation::OwnChamberOut { chamber, onward, out: false }) if cylinder => {
                self.animator.set_flag("Discharge", false);
                self.own_chamber_emptied(chamber);
                self.animator.set("ShellsInWeapon", self.cases() as f32);
                self.animator.set("AmmoInMag", self.rounds() as f32);
                self.animator.set_flag("CanReload", false);
                self.operation = Operation::OwnChamberOut { chamber, onward, out: true };
                if onward {
                    self.own_cylinder_unloaded(true);
                }
            }
            ("MagHide", Operation::OwnChamberOut { onward: false, out: true, .. }) if cylinder => self.own_cylinder_unloaded(false),
            // Barrels': of two or more, the round out at `DelAmmoChamber`
            // (`RemoveAmmoFromChamber`); of one, that mark is not its own.
            ("DelAmmoChamber", Operation::OwnChamberOut { chamber, onward, out: false }) if several && !cylinder => {
                self.own_barrel_emptied(chamber);
                self.operation = Operation::OwnChamberOut { chamber, onward, out: true };
            }
            ("DelAmmoChamber", Operation::OwnChamberOut { .. }) if !cylinder => {}
            // Over as the hand puts it away (`OnMagPuttedToRig`): of one
            // barrel, the round out now; of more, now where it goes into
            // another barrel and its own mark has not come (else that has
            // taken it out). PROVISIONAL: of more, going nowhere, with no
            // mark come, the game's round would stay (the inventory's move
            // never made); the host has moved it already, so it is out.
            ("MagHide", Operation::OwnChamberOut { chamber, out, .. }) if !cylinder => {
                self.animator.set_flag("Discharge", false);
                self.to_idle();
                if !out {
                    self.own_barrel_emptied(chamber);
                }
            }
            // Marks the game's operations do not answer (`ProhibitedCall`).
            ("ShellEject" | "AddAmmoInMag" | "DelAmmoChamber", Operation::OwnChamberIn { .. } | Operation::OwnChamberOut { .. }) => {}
            _ => return false,
        }
        true
    }

    /// A cylinder's round taken out, the operation over
    /// (`DischargeEnd`): the drum turned back as a load's end does, the
    /// animator told whether a load follows (`SetLoadOneForNextOperation`).
    fn own_cylinder_unloaded(&mut self, onward: bool) {
        self.own_cylinder_at_rest();
        self.animator.set_flag("LoadOne", onward);
    }

    /// A barrel's round out (`RemoveAmmoFromChamber`, `DischargeOperation.
    /// OnMagPuttedToRig`): the animator told the rounds and cases left,
    /// `CanReload` taken away.
    fn own_barrel_emptied(&mut self, index: usize) {
        self.own_chamber_emptied(index);
        self.animator.set("AmmoInChamber", self.rounds() as f32);
        self.animator.set("ShellsInWeapon", self.cases() as f32);
        self.animator.set_flag("CanReload", false);
    }
}
