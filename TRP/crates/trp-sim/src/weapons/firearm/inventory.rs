//! Moving magazines from the inventory and the chamber checks before loading.

use super::*;

impl Firearm {
    /// The character's inventory opens or shuts (`SetInventoryOpened`;
    /// inventory spec, "The inventory open"). A weapon at rest puts its
    /// sights down, lets the trigger go and tells its animator at once,
    /// whose own states take the hands down and back. A shot in hand only
    /// notes it and tells the animator as it hands back to idling; a
    /// reload tells it once the magazine is out or the new one has shown;
    /// a weapon being drawn or put away does not hear of it at all (one
    /// that has just appeared is told afresh: `draw`).
    pub fn set_inventory(&mut self, open: bool) {
        // The left shoulder first (`FirearmController.SetInventoryOpened`).
        self.events.push(Event::LeftStance(crate::weapons::left_stance::Call::Inventory(open)));
        // Opened, blind fire is over (`FirearmController.InventoryOpened`).
        if open {
            self.ends_blind_fire();
        }
        match &mut self.operation {
            // The malfunction's own operations leave the animator be (the
            // base operation's `ProhibitedCall`); the rest after them goes
            // by it.
            Operation::Malfunction { .. } | Operation::ExamineMalfunction { .. } | Operation::RepairMalfunction { .. } => self.inventory = open,
            // The rack hears nothing of it (the base operation's
            // `ProhibitedCall`; its end tells the animator what the
            // controller last noted, `SwitchToIdle`).
            Operation::Rechamber { .. } => {}
            // A cylinder's turn notes it and tells the animator
            // (`RollCylinderOperation.SetInventoryOpened`).
            Operation::RollCylinder => {
                self.inventory = open;
                self.animator.set_flag("Inventory", open);
            }
            Operation::Idle => {
                self.aiming = false;
                self.idle_trigger(false);
                self.inventory = open;
                self.animator.set_flag("Inventory", open);
            }
            Operation::Utility { in_inventory, time, .. } => {
                *in_inventory = open;
                if !open {
                    *time = 0.0;
                }
                self.inventory = open;
                self.animator.set_flag("Inventory", open);
            }
            Operation::Single { open_inventory, .. } | Operation::Automatic { open_inventory, .. } => {
                *open_inventory = open;
                // The screen does not wait for the trigger: the bolt is
                // worked now.
                if self.settings.bolt_action {
                    self.animator.set_flag("BoltActionReload", true);
                }
            }
            // A stock being folded only notes it, and tells the animator
            // when the fold is over; so does the bipod's toggle.
            Operation::Fold { .. } | Operation::Bipod { .. } => self.inventory = open,
            Operation::Reload { mag_out, mag_shown, .. } => {
                self.inventory = open;
                if *mag_out || *mag_shown {
                    self.animator.set_flag("Inventory", open);
                }
            }
            // A tube being filled is cut short by the screen, opening or
            // shutting; the animator hears of it only once a round has
            // shown (and one opened before that is never told to it by
            // this operation: the game's own setter cannot do what it is
            // written to do. Shotgun spec, "Filling the tube").
            Operation::Shells { can_abort, aborted, .. } => {
                self.inventory = open;
                *aborted = true;
                if *can_abort {
                    self.animator.set_flag("Inventory", open);
                }
            }
            // A backpack being dropped only notes it, and tells the
            // animator when the pack is off.
            Operation::DropBackpack => self.inventory = open,
            // A cylinder or barrels being reloaded is cut short as a
            // tube is; a shot from either only notes the screen
            // (`FireCylinderMagOperation.SetInventoryOpened`).
            Operation::ChambersReload { can_abort, aborted, .. } => {
                self.inventory = open;
                *aborted = true;
                if *can_abort {
                    self.animator.set_flag("Inventory", open);
                }
            }
            Operation::CylinderFire { .. } | Operation::BarrelsFire { .. } => self.inventory = open,
            Operation::ToggleLauncher { .. } | Operation::LauncherIdle | Operation::LauncherFire { .. } | Operation::LauncherReload { .. } | Operation::LauncherLoad | Operation::LauncherDischarge | Operation::LauncherUtility { .. } => self.launcher_inventory(open),
            // A magazine moved from the inventory tells the animator at
            // once (`PullOutMagOperation`, `InsertMagOperation`:
            // `SetInventoryOpened`).
            Operation::InventoryMagOut { .. }
            | Operation::InventoryMagIn { .. }
            | Operation::InventoryChamber
            | Operation::InventoryDischarge
            | Operation::OwnChamberIn { .. }
            | Operation::OwnChamberOut { .. } => {
                self.inventory = open;
                self.animator.set_flag("Inventory", open);
            }
            Operation::Spawn { .. } | Operation::Remove | Operation::Away => {}
        }
    }

    /// Whether the inventory may move the weapon's magazine now, which the
    /// hands then do (`FirearmController.CanExecute`): only while the
    /// logic idles.
    pub fn can_move_magazine(&self) -> bool {
        matches!(self.operation, Operation::Idle)
    }


    /// The magazine moved out of the weapon from the inventory
    /// (`PullOutMagOperation`; firing spec, "A magazine taken out or put
    /// in from the inventory"): the hands take it out by the reload's first
    /// half from their inventory pose, and `Event::MagazineMoved` says when
    /// it has gone. False, and nothing done, if the logic is not idling or
    /// no magazine is in.
    pub fn pull_out_magazine(&mut self) -> bool {
        if !self.can_move_magazine() || self.magazine.is_none() {
            return false;
        }
        self.trigger_to(false);
        self.animator.set_flag("MagInFromInv", false);
        self.animator.trigger("MagOutFromInv");
        self.animator.set_flag("CanReload", false);
        self.animator.set_flag("Fire", false);
        self.animator.set_flag("IsExternalMag", true);
        // (`PullOutMagOperation.Start`: the fitted one's, nought with none.)
        self.animator.set("RelTypeOld", if self.magazine.is_some() { self.mag_variant as f32 } else { 0.0 });
        let s = &self.settings;
        if s.bolt_catch && self.chamber && !s.manual_bolt_catch && !s.must_open_bolt {
            self.set_bolt_catch(false);
        }
        (self.hands_busy, self.reload_flag) = (true, true);
        self.operation = Operation::InventoryMagOut { pulled: false };
        true
    }

    /// Whether the hands are taking the magazine out for the inventory
    /// (`PullOutMagOperation`), begun by the inventory's move or the
    /// magazine's unload key.
    pub fn pulling_magazine_out(&self) -> bool {
        matches!(self.operation, Operation::InventoryMagOut { .. })
    }

    /// A magazine of `rounds` put into the weapon from the inventory
    /// (`InsertMagOperation`): the hands put it in by the reload's second
    /// half, and chamber a round from it if the chamber is empty;
    /// `Event::MagazineMoved` says when it is done. False, and nothing
    /// done, if the logic is not idling or a magazine is in already.
    pub fn insert_magazine(&mut self, rounds: u32) -> bool {
        if !self.can_move_magazine() || self.magazine.is_some() {
            return false;
        }
        self.trigger_to(false);
        self.incoming = rounds;
        self.chambers = !self.chamber && !self.settings.no_chamber && rounds > self.incoming_unfit;
        let variant = self.incoming_variant as f32;
        self.animator.set_flag("Fire", false);
        self.animator.set_flag("IsExternalMag", true);
        self.animator.set_flag("CanReload", true);
        self.animator.set("RelTypeOld", variant);
        self.animator.set("RelTypeNew", variant);
        self.animator.trigger("MagInFromInv");
        let s = &self.settings;
        if s.bolt_catch && self.chamber && !self.chambers && !s.manual_bolt_catch && !s.must_open_bolt {
            self.set_bolt_catch(false);
        }
        (self.hands_busy, self.reload_flag) = (true, true);
        self.operation = Operation::InventoryMagIn { shown: false, inserted: false };
        true
    }


    /// Whether the hands would take a round put into the chamber from the
    /// inventory now (`Idling.Execute`, a round moved to a chamber): the
    /// logic idles, the weapon has one chamber of the weapon's own and it
    /// is empty, a weapon of a detachable magazine has none in, and its
    /// chamber is loaded by hand (`isChamberLoad`). For a weapon of
    /// chambers of their own (a cylinder's, barrels'), whether they would
    /// take one into any of them (`Firearm::can_load_own_chamber`).
    pub fn can_load_chamber(&self) -> bool {
        if self.has_chambers() {
            return self.can_load_own_chamber();
        }
        // (`Idling.Execute`: refused while a magazine that comes out is in,
        // the Model 700's too.)
        self.can_move_magazine() && !self.settings.no_chamber && !self.chamber && (!self.magazine_comes_out() || self.magazine.is_none()) && self.settings.chamber_load
    }

    /// Whether the weapon has one chamber of its own (not a cylinder's nor
    /// barrels'), which `chamber` says of.
    pub fn single_chamber(&self) -> bool {
        !self.has_chambers()
    }

    /// A round put into the empty chamber outright, at once and with no
    /// hands: for the debug panel's FILL, a test tool, never a rule of the
    /// game's (whose keys feed an empty chamber only by a reload: firing
    /// spec, "The magazine's and the chamber's keys").
    /// Only a weapon of one chamber of its own,
    /// its logic at rest. A bolt held open by its catch goes home, as a
    /// feed lets it go, and the animator is told the rounds. Whether it
    /// was put in.
    pub fn chamber_outright(&mut self) -> bool {
        if self.has_chambers() || self.settings.no_chamber || self.chamber || !self.is_idling() {
            return false;
        }
        self.chamber = true;
        let s = &self.settings;
        if s.bolt_catch && !s.manual_bolt_catch && !s.must_open_bolt && self.bolt_caught() {
            self.set_bolt_catch(false);
        }
        self.animator_rounds();
        true
    }
}
