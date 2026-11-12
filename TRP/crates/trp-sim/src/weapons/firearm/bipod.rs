//! Folding the stock, mounting and the bipod.

use super::*;

impl Firearm {
    /// Folds the stock, or unfolds it (`FirearmHandsInputTranslator`, the
    /// fold key; `FoldStockOperation`; shotgun spec, "The stock"). Only a
    /// weapon whose logic idles takes it, and not in a look at it; the
    /// trigger is let go. The animator plays the fold from its resting
    /// state, and at the state's own mark the stock stands the new way
    /// (`Event::Folded`) and the logic idles again while the animation
    /// plays out. Whether it began.
    pub fn fold_stock(&mut self) -> bool {
        if !self.settings.foldable {
            return false;
        }
        // A stock that folds leaves a mount at once (`FoldStock`'s command).
        self.events.push(Event::LeaveMount { at_once: true });
        if !self.is_idling() {
            return false;
        }
        // The fold is not refused in blind fire, and ends it
        // (`FoldStockOperation.Start`).
        self.ends_blind_fire();
        self.trigger_to(false);
        self.animator.set_flag("Inventory", false);
        self.animator.trigger("StockSwitch");
        self.hands_busy = true;
        self.operation = Operation::Fold { to: !self.folded, hide: None };
        true
    }

    /// Whether the thing in hand may mount (`Player.TryMountWeapon`'s part of
    /// the weapon's): a mountable firearm in hand, in no reload, draw,
    /// put-away or interaction with the inventory.
    pub fn may_mount(&self) -> bool {
        self.settings.mountable
            && !self.reloading()
            && !matches!(
                self.operation,
                Operation::Away
                    | Operation::Spawn { .. }
                    | Operation::Remove
                    | Operation::DropBackpack
                    | Operation::InventoryMagOut { .. }
                    | Operation::InventoryMagIn { .. }
                    | Operation::InventoryChamber
                    | Operation::InventoryDischarge
                    | Operation::OwnChamberIn { .. }
                    | Operation::OwnChamberOut { .. }
            )
    }

    /// Mounted or not, as the mount told the weapon (`FirearmsAnimator.
    /// SetMounted`: the animator's `Mounted`, where it has one); and whether
    /// mounted forward on the bipod (the tactical reload's blend runs then).
    pub fn set_mounted(&mut self, mounted: bool, on_bipod: bool) {
        if self.animator.has("Mounted") {
            self.animator.set_flag("Mounted", mounted);
        }
        self.mount_tactical = mounted && on_bipod;
    }

    /// Whether the weapon has a bipod, and whether it is out.
    pub fn bipod(&self) -> Option<bool> {
        self.settings.bipod.map(|_| self.bipod_out)
    }

    /// The bipod's key (`FirearmController.ToggleBipod`): refused without a
    /// bipod, mounted, with the weapon stuck into something, in a sprint,
    /// with the body unable to make a gesture (`gesture_free`: its queue
    /// takes one and it may interact), with the animator not at rest, and in
    /// any operation but the idling one. Taken, the bipod is out (or in) at
    /// once, the animator's inventory flag down and its `Bipod` set where it
    /// has one, blind fire ended, and `Event::Bipod` given. Whether taken.
    pub fn toggle_bipod(&mut self, mounted: bool, sprinting: bool, gesture_free: bool) -> bool {
        let Some(forward) = self.settings.bipod else { return false };
        if mounted || self.overlap > 0.0 || sprinting || !gesture_free || !self.animator_idling() || !self.is_idling() {
            return false;
        }
        self.bipod_out = !self.bipod_out;
        self.animator.set_flag("Inventory", false);
        // The hands' `Bipod`, where they have one, and the bipod's own
        // animator's: one animator in the game (`FirearmsAnimator.SetBipod`
        // on the weapon's, its parts' merged into it), so its legs fold and
        // their marks (`BipodOpen`, `BipodClose`) play the bipod's sounds.
        self.animator.set_flag("Bipod", self.bipod_out);
        self.ends_blind_fire();
        self.operation = Operation::Bipod { hide: None };
        self.events.push(Event::Bipod { out: self.bipod_out, forward });
        true
    }

    /// Whether the bipod's toggle is the operation in hand.
    pub fn toggling_bipod(&self) -> bool {
        matches!(self.operation, Operation::Bipod { .. })
    }

    /// The body's gesture's mark (`OnBipodToggle`; `ToggleBipodOperation.
    /// SwitchToIdle`): the animator is told how the inventory stands, the
    /// weapon idles, and a put-away asked for meanwhile goes.
    pub fn bipod_toggled(&mut self) {
        let Operation::Bipod { hide } = self.operation else { return };
        self.animator.set_flag("Inventory", self.inventory);
        self.to_idle();
        if let Some(fast) = hide {
            self.hide(fast);
        }
    }

    /// The fold's mark (`FoldStockOperation.OnFold`, `SwitchToIdle`).
    pub(super) fn folded_now(&mut self) {
        let Operation::Fold { to, hide } = self.operation else { return };
        self.folded = to;
        self.animator_fold();
        self.animator.set_flag("Inventory", self.inventory);
        self.to_idle();
        self.events.push(Event::Folded(to));
        if let Some(fast) = hide {
            self.hide(fast);
        }
    }
}
