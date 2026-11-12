//! Loading, discharging, unloading and rechambering by hand.

use super::*;

impl Firearm {
    /// A round put into the empty chamber from the inventory
    /// (`LoadCartridgeToChamber`; firing spec, "A round into the chamber
    /// from the inventory"): aiming off, the trigger let go, the animator's
    /// load of one (`LoadOne`, chamber 0) from its inventory pose or its
    /// rest; at the state's `AddAmmoInChamber` mark the round is in
    /// (`Event::Chambered`) and the logic idles. False, and nothing done,
    /// where the hands would not take it (`can_load_chamber`).
    pub fn load_chamber(&mut self) -> bool {
        if self.has_chambers() || !self.can_load_chamber() {
            return false;
        }
        self.aiming = false;
        self.trigger_to(false);
        self.animator.set_flag("Fire", false);
        self.animator.set_flag("UseLeftHand", false);
        self.animator.set_flag("LoadOne", true);
        self.animator.set("ChamberIndex", 0.0);
        (self.hands_busy, self.reload_flag) = (true, true);
        self.round_in_hand = false;
        self.operation = Operation::InventoryChamber;
        // The class's chamber action as the operation begins (`Idling.
        // Execute`, after `LoadCartridgeToChamber.Start`).
        self.events.push(Event::Earn(Earn::Chamber));
        true
    }

    /// Whether the hands would let the chambered round be taken out from
    /// the inventory now (`Idling.Execute`, a round moved out of a chamber):
    /// the logic idles, the weapon has one chamber of its own with a round
    /// in it, and a weapon of a detachable magazine has none in, one of a
    /// magazine of its own has it empty. For a weapon of chambers of their
    /// own, whether they would let a round out of any of them
    /// (`Firearm::can_unload_own_chamber`).
    pub fn can_discharge(&self) -> bool {
        if self.has_chambers() {
            return self.can_unload_own_chamber();
        }
        let emptied = if self.magazine_comes_out() { self.magazine.is_none() } else { self.magazine.unwrap_or(0) == 0 };
        // (An animator without the state would never give its mark: the
        // round stays, `PROVISIONAL`, the game's weapons all having it.)
        let played = self.animator.has("Discharge") && self.animator.has_state("Hands.DISCHARGE");
        self.can_move_magazine() && !self.has_chambers() && !self.settings.no_chamber && self.chamber && emptied && played
    }

    /// The chambered round taken out from the inventory (`DischargeOperation`;
    /// firing spec, "The chambered round taken out from the inventory"):
    /// aiming off, the trigger let go, blind fire ended, the animator's
    /// `Discharge` (the AKM's `DISCHARGE`, from its inventory pose or its
    /// rest); at the state's `MagHide` mark the chamber is empty
    /// (`Event::Discharged`) and the logic idles. False, and nothing done,
    /// where the hands would not let it out (`can_discharge`).
    pub fn discharge(&mut self) -> bool {
        if self.has_chambers() || !self.can_discharge() {
            return false;
        }
        // It puts a misfire right (`DischargeOperation.Start`).
        self.clear_misfire();
        self.aiming = false;
        self.animator.set_flag("Discharge", true);
        self.trigger_to(false);
        self.animator.set_flag("Fire", false);
        self.ends_blind_fire();
        self.animator.set("ChamberIndex", 0.0);
        // (`SetLoadOneForNextOperation`: a load after it only for a round
        // moved from one chamber to another.)
        self.animator.set_flag("LoadOne", false);
        (self.hands_busy, self.reload_flag) = (true, true);
        self.operation = Operation::InventoryDischarge;
        // The class's chamber action as the operation begins (`Idling.
        // Execute`, after `DischargeOperation.Start`): the chamber key's
        // round handed to the rig too, which the inventory moves this way.
        self.moving_round = false;
        self.events.push(Event::Earn(Earn::Chamber));
        true
    }

    /// The magazine's unload key (`FirearmHandsInputTranslator.UnloadMag`;
    /// firing spec, "The magazine's and the chamber's keys"): only for a
    /// magazine that comes out, and it leaves a mount; with a magazine in
    /// and no failure to feed, true: the inventory then moves it to the vest
    /// or the pockets, the smallest grid with room first, and the hands play
    /// the move (`pull_out_magazine`). A failure to feed reminds instead.
    pub fn unload_magazine_key(&mut self) -> bool {
        if !self.magazine_comes_out() {
            return false;
        }
        self.events.push(Event::LeaveMount { at_once: false });
        if self.magazine.is_none() {
            return false;
        }
        if self.malfunction() == Malf::Feed {
            self.animator.set_flag("MisfireSlideUnknown", false);
            self.remind();
            return false;
        }
        true
    }

    /// The chamber's unload key (`FirearmHandsInputTranslator.UnloadChamber`):
    /// nothing for a weapon whose rounds the player may not take out, nor
    /// for one with neither a chamber nor a cylinder; else it leaves a mount.
    /// A malfunction is looked into (a misfire of a known kind repaired,
    /// `check_chamber`) or reminded of; a cylinder is turned on by a chamber
    /// (`roll_cylinder`); with a round in the first loaded chamber, a
    /// magazine in has the action worked (`rechamber`), and with none the
    /// round is the inventory's to move (`ChamberKey::ToRig`). An
    /// underbarrel launcher in use has its round unloaded instead, after the
    /// mount is left (`UnloadLauncher`: `ChamberKey::Launcher`).
    pub fn unload_chamber_key(&mut self) -> ChamberKey {
        let cylinder = self.settings.cylinder.is_some();
        if !self.settings.can_unload || (self.settings.no_chamber && !cylinder) {
            return ChamberKey::Done;
        }
        self.events.push(Event::LeaveMount { at_once: false });
        if self.in_launcher_mode() {
            return ChamberKey::Launcher;
        }
        let malf = self.malfunction();
        if malf != Malf::None {
            if malf == Malf::Misfire && self.wear.as_deref().is_some_and(|w| w.state.type_known) {
                self.check_chamber();
            } else {
                self.animator.set_flag("MisfireSlideUnknown", false);
                self.remind();
            }
            return ChamberKey::Done;
        }
        if cylinder {
            self.roll_cylinder();
            return ChamberKey::Done;
        }
        let loaded = if self.has_chambers() { self.in_chambers.iter().any(|chamber| chamber.round) } else { self.chamber };
        if !loaded {
            return ChamberKey::Done;
        }
        // Barrels have no magazine (`GetCurrentMagazine`), though their
        // rounds are counted as one's.
        if self.magazine.is_some() && self.settings.barrels == 0 {
            self.rechamber();
            return ChamberKey::Done;
        }
        ChamberKey::ToRig
    }

    /// Whether the hands would work the action now to throw the chambered
    /// round out (`Idling.Execute`, a `WeaponRechamberOperation`): the logic
    /// idles, the weapon has one chamber of its own with a round in it, and
    /// its animator has the rack (`Rechamber`: else nothing is done,
    /// `PROVISIONAL`; of the install's weapons only the double barrels lack
    /// it, which the key refuses first).
    pub fn can_rechamber(&self) -> bool {
        self.can_move_magazine() && !self.has_chambers() && !self.settings.no_chamber && self.chamber && self.animator.has("Rechamber")
    }

    /// The chambered round thrown out by working the action, and the
    /// magazine's top round fed in its place (`RechamberOperation.Start`):
    /// a misfire put right, the trigger let go, the sights down, blind fire
    /// ended, the animator's `Rechamber` (the AKM's `RECHAMBER`,
    /// `akm_misfire_0`, from its rest). At `DelAmmoChamber` the round leaves
    /// the chamber's point; at `ShellEject` it is thrown out
    /// (`Event::RoundRacked`) and, with none to feed, the rack is over; at
    /// `AddAmmoInChamber` the magazine's top round is chambered where the
    /// chamber takes it, and the rack is over. False, and nothing done,
    /// where the hands would not (`can_rechamber`).
    pub fn rechamber(&mut self) -> bool {
        if !self.can_rechamber() {
            return false;
        }
        // (`Idling.Execute`: the trigger let go for any move of the
        // weapon's parts.)
        self.trigger_to(false);
        self.clear_misfire();
        let feeds = self.magazine.unwrap_or(0) > 0;
        self.events.push(Event::RackBegun { feeds });
        self.aiming = false;
        self.ends_blind_fire();
        self.animator.set("AmmoInChamber", 1.0);
        self.animator.set_flag("Rechamber", true);
        self.animator.set_flag("Inventory", false);
        self.animator.set_flag("Fire", false);
        (self.hands_busy, self.reload_flag) = (true, true);
        self.operation = Operation::Rechamber { feeds, removed: false, ejected: false, added: false, hide: None };
        true
    }

    /// The rack over (`RechamberOperation.SwitchToIdle`): the animator told
    /// the rounds, the rack off, the inventory as the controller last noted
    /// it; the logic idles, or the put-away asked meanwhile is made.
    pub(super) fn rechamber_over(&mut self) {
        let Operation::Rechamber { hide, .. } = self.operation else { return };
        self.animator_rounds();
        self.animator.set_flag("Rechamber", false);
        self.animator.set_flag("Inventory", self.inventory);
        self.to_idle();
        // Its plan goes straight to the put-away (`RechamberOperation.HideWeapon`).
        if let Some(fast) = hide {
            self.hide_from(fast, false);
        }
    }

    /// A cylinder turned on by a chamber (`FirearmController.RollCylinder`,
    /// `Idling.RollCylinder`, `RollCylinderOperation.Start`): not in blind
    /// fire, and only at rest; the sights down, the animator's
    /// `RollCylinder` (the Rhino's `RELOAD START`, then `ROLL CYLINDER`),
    /// and at its `MagHide` mark the cylinder turned and the logic idling.
    /// Whether it began (an animator without the parameter: not,
    /// `PROVISIONAL`, the install's four revolvers all having it).
    pub fn roll_cylinder(&mut self) -> bool {
        if self.settings.cylinder.is_none() || self.blind_fire || !matches!(self.operation, Operation::Idle) || !self.animator.has("RollCylinder") {
            return false;
        }
        self.aiming = false;
        self.animator.set_flag("RollCylinder", true);
        (self.hands_busy, self.reload_flag) = (true, true);
        self.operation = Operation::RollCylinder;
        true
    }

    /// A magazine's move from the inventory done: the inventory makes it,
    /// and the logic idles.
    pub(super) fn inventory_move_over(&mut self) {
        self.events.push(Event::MagazineMoved);
        self.to_idle();
    }

    /// Whether the animator is in its resting state, on a way out of it or
    /// not (`FirearmsAnimator.IsIdling`): what the game asks before it
    /// begins a gesture of the hands on the world.
    pub fn animator_idling(&self) -> bool {
        self.in_hand() && self.animator.current_is(&self.settings.rest_state)
    }

    /// Whether the hands are down for the open inventory, or on their way
    /// down: the game then tells the body's animator, which is the pose
    /// others see (`Player.WaitWeaponInventory`).
    pub fn in_inventory(&self) -> bool {
        IN_INVENTORY.iter().any(|state| self.animator.current_is(state))
    }

    /// Notes where the finger is in a frame in which a screen has the
    /// keys: the weapon is not told, so a press made under the screen
    /// fires nothing when it shuts with the trigger still held.
    pub fn note_trigger(&mut self, pressed: bool) {
        self.held = pressed;
    }
}
