//! Fire mode changes, chamber and magazine checks, examining and the backpack.

use super::*;

impl Firearm {
    /// The next fire mode. The idling operation does it itself: the mode
    /// changes at once and the animator plays the switch when it is next
    /// at rest, so the trigger and the reload key work through it.
    /// The fire mode key (`ChangeWeaponMode`: `GetNextFireMode`, which on a
    /// weapon of one way of firing is that one): the screen names the mode
    /// whether it changed or not.
    pub fn next_fire_mode(&mut self) {
        let count = self.settings.modes.len();
        if count > 0 {
            let next = self.settings.modes[(self.mode + 1) % count];
            self.change_fire_mode_shown(next);
        }
    }

    /// `Idling.ChangeFireMode` from a key: refused as `set_fire_mode` refuses
    /// it, the bolt held open among it; else the screen names the mode
    /// (`ShowFireMode`), changed or not, and it is set. Whether it changed.
    fn change_fire_mode_shown(&mut self, mode: FireMode) -> bool {
        // Refused in blind fire before anything is shown
        // (`FirearmController.ChangeFireMode`).
        if self.blind_fire {
            return false;
        }
        let idling = matches!(self.operation, Operation::Idle | Operation::Utility { .. });
        if !idling || !self.settings.modes.contains(&mode) || (self.settings.bolt_catch && self.settings.no_fire_mode_on_bolt_catch && self.bolt_caught()) {
            return false;
        }
        self.events.push(Event::FireModeShown(mode));
        self.set_fire_mode(mode)
    }

    /// Changes to one of the weapon's fire modes (`Idling.ChangeFireMode`).
    /// Only the idling operation answers (and a look at the weapon, which
    /// is one), and **not while the bolt is held open** on a weapon that
    /// says so (`NoFiremodeOnBoltcatch`). Whether it changed.
    pub fn set_fire_mode(&mut self, mode: FireMode) -> bool {
        if self.blind_fire {
            return false;
        }
        let idling = matches!(self.operation, Operation::Idle | Operation::Utility { .. });
        let Some(index) = self.settings.modes.iter().position(|has| *has == mode).filter(|_| idling) else { return false };
        if self.settings.bolt_catch && self.settings.no_fire_mode_on_bolt_catch && self.bolt_caught() {
            return false;
        }
        if index == self.mode {
            return false;
        }
        self.mode = index;
        self.events.push(Event::FireMode(self.mode()));
        self.animator.set("FireMode", mode_number(self.mode()));
        self.animator.trigger("FiremodeSwitch");
        self.cylinder_mode_changed();
        // A weapon whose animator has a sprinting variant of the switch
        // has the body told the hands are busy.
        if self.animator.has("FiremodeSprint") {
            self.hands_busy = true;
        }
        true
    }

    /// The fire mode the weapon's item keeps (`FireModeComponent.FireMode`),
    /// as a controller made on the item finds it: nothing is shown and no
    /// switch is played. None is the weapon's first, and a mode the weapon
    /// has not leaves it as it is. Whether it changed.
    pub fn start_fire_mode(&mut self, mode: Option<FireMode>) -> bool {
        let Some(index) = mode.map_or(Some(0), |mode| self.settings.modes.iter().position(|has| *has == mode)) else { return false };
        if index == self.mode || index >= self.settings.modes.len() {
            return false;
        }
        self.mode = index;
        self.animator.set("FireMode", mode_number(self.mode()));
        true
    }

    /// Looks into the chamber (`Idling.CheckChamber`): not with the
    /// trigger pressed. The sights go down for it and the aim key does
    /// nothing until it is over; which state plays is the animator's
    /// choice by the fire mode and the bolt.
    pub fn check_chamber(&mut self) {
        self.events.push(Event::LeaveMount { at_once: false });
        // The launcher's rest looks into the launcher's chamber.
        if self.operation == Operation::LauncherIdle {
            self.launcher_check_chamber();
            return;
        }
        // With a malfunction, its repair where its kind is known, else the
        // reminder (`Idling.CheckChamber`).
        if self.malfunction_check_chamber() {
            return;
        }
        if self.is_idling() && !self.trigger && !self.blind_fire {
            self.aiming = false;
            self.animator.trigger("CheckChamber");
            self.hands_busy = true;
            self.operation = Operation::Utility { ready: false, time: 0.0, in_inventory: false, examine: false };
        }
    }

    /// Looks at the selector (`Idling.CheckFireMode`): only a weapon with
    /// more than one way of firing, and only at rest. The sights go down
    /// for it. Whether the look began.
    pub fn check_fire_mode(&mut self) -> bool {
        self.events.push(Event::LeaveMount { at_once: false });
        if !self.is_idling() || self.blind_fire {
            return false;
        }
        // The screen names the mode first, a weapon of one way of firing too
        // (`Idling.CheckFireMode`'s first line).
        self.events.push(Event::FireModeShown(self.mode()));
        if self.settings.modes.len() <= 1 {
            return false;
        }
        self.aiming = false;
        self.animator.trigger("FiremodeCheck");
        self.hands_busy = true;
        self.operation = Operation::Utility { ready: false, time: 0.0, in_inventory: false, examine: false };
        true
    }

    /// Looks the weapon over (`Idling.ExamineWeapon`): only at rest, and
    /// **not with the sights up**, which it does not put down. The
    /// trigger ends it (`trigger_changed`). Whether the look began.
    pub fn examine(&mut self) -> bool {
        // The launcher's rest looks the launcher over (`LauncherIdling.ExamineWeapon`),
        // and the look leaves a mount.
        if self.operation == Operation::LauncherIdle {
            let begun = !self.blind_fire && self.launcher_examine();
            if begun {
                self.events.push(Event::LeaveMount { at_once: false });
            }
            return begun;
        }
        // With a malfunction, the look at it (`Idling.ExamineWeapon`: the
        // shooter knows of it from the shot), which puts the sights down.
        if self.malfunction() != Malf::None {
            return self.examine_malfunction();
        }
        if !self.is_idling() || self.aiming || self.blind_fire {
            return false;
        }
        // Begun, the look leaves a mount (`FirearmController.ExamineWeapon`).
        self.events.push(Event::LeaveMount { at_once: false });
        self.hands_busy = true;
        self.operation = Operation::Utility { ready: false, time: 0.0, in_inventory: false, examine: true };
        self.animator.trigger("Look");
        true
    }

    /// The magnification key's sign to the hands (`FirearmsAnimator.
    /// ModToggleTrigger`, which every operation gives with a change of a
    /// scope's mode: `SetScopeMode`): the animator's `ModSwitch`, where it
    /// has one (optics spec, "Zoom and modes").
    pub fn mod_switch(&mut self) {
        self.animator.trigger("ModSwitch");
    }

    /// Whether the operation in hand lets a tactical device be switched
    /// (`CanChangeLightState`, overridden to refuse only by the draw, the
    /// put-away, a part fitted or taken off, a magazine put in or pulled
    /// out through the inventory, the backpack dropped and the bipod;
    /// tactical-devices spec, "The keys"): at rest, aimed, in any fire, in
    /// a reload, a look or a fold it may. A part changes here only at
    /// rest, by a new build (the app), and there is no bipod.
    pub fn can_change_lights(&self) -> bool {
        !matches!(
            self.operation,
            Operation::Away | Operation::Spawn { .. } | Operation::Remove | Operation::InventoryMagOut { .. } | Operation::InventoryMagIn { .. } | Operation::DropBackpack
        )
    }

    /// Puts the weapon on its fullest way of firing
    /// (`FireModeComponent.GetForceAutoFireMode`): automatic if it has it,
    /// else bursts; a weapon with one way of firing, or with neither of
    /// those, stays as it is. The change itself is `set_fire_mode`'s.
    pub fn force_auto_fire_mode(&mut self) -> bool {
        if self.settings.modes.len() <= 1 {
            return false;
        }
        // (`GetForceAutoFireMode`: automatic, bursts, both barrels, double
        // action, in that order; only where it is not the mode already.)
        let fullest = [FireMode::FullAuto, FireMode::Burst, FireMode::Doublet, FireMode::DoubleAction].into_iter().find(|mode| self.settings.modes.contains(mode));
        fullest.filter(|mode| *mode != self.mode()).is_some_and(|mode| self.change_fire_mode_shown(mode))
    }

    /// The backpack is to be dropped with this in hand (the idling
    /// operation's `Execute` of a thing leaving a slot that is animated,
    /// `DropBackpackOperation.Start`): **only at rest**; in anything else
    /// the game refuses the drop. The trigger's state is taken off the
    /// animator, the hands are told the inventory is shut, the sights go
    /// down. `backpack_dropped` ends it. Whether it began.
    pub fn drop_backpack(&mut self) -> bool {
        if !self.is_idling() {
            return false;
        }
        // It ends blind fire (`DropBackpackOperation.Start`).
        self.ends_blind_fire();
        self.animator.set_flag("Fire", false);
        self.animator.set_flag("Inventory", false);
        self.aiming = false;
        self.operation = Operation::DropBackpack;
        true
    }

    /// The pack is off the shoulders (`OnBackpackDropEvent`): the
    /// animator is told how the inventory stands, and the weapon idles.
    pub fn backpack_dropped(&mut self) {
        if self.operation == Operation::DropBackpack {
            self.animator.set_flag("Inventory", self.inventory);
            self.to_idle();
        }
    }

    /// Whether a backpack's drop is in hand.
    pub fn dropping_backpack(&self) -> bool {
        self.operation == Operation::DropBackpack
    }

    /// Looks at the magazine. The sights go down for it and the aim key
    /// does nothing until it is over. Whether it began (the battle screen
    /// then names the count: `Event::MagazineChecked`).
    pub fn check_magazine(&mut self) -> bool {
        self.events.push(Event::LeaveMount { at_once: false });
        // A failure to feed allows no look at the magazine (`Idling.CheckAmmo`).
        if self.malfunction() == Malf::Feed {
            self.remind();
            return false;
        }
        // The look asks for the weapon's magazine (`GetCurrentMagazine`):
        // barrels have none, though their rounds are counted as one's.
        if self.is_idling() && self.magazine.is_some() && self.settings.barrels == 0 && !self.blind_fire {
            self.events.push(Event::MagazineChecked);
            self.aiming = false;
            self.animator.trigger("CheckAmmo");
            // The animator is told whether the magazine comes out
            // (`Idling.CheckAmmo`: `ExternalMagazine` or
            // `ExternalMagazineWithInternalReloadSupport`): a weapon with
            // both looks takes the one that takes the magazine out (the
            // Model 700's `CHECK MAG`, where `CHECK` works the bolt).
            self.animator.set_flag("IsExternalMag", self.magazine_comes_out());
            self.hands_busy = true;
            self.operation = Operation::Utility { ready: false, time: 0.0, in_inventory: false, examine: false };
            true
        } else {
            false
        }
    }

    /// Whether the hands are busy with the weapon: through a reload, to
    /// the end of its animation, through a look at the weapon, and while
    /// it is drawn and put away. The body's animator is told
    /// (`movement::Input::hands_busy`, the game's `ReloadFloat`): in a
    /// sprint it brings the weapon up into view for it. The game sets the
    /// same for a fire-mode switch on weapons whose animator has a
    /// sprinting variant of it; the AKM's has none.
    pub fn hands_busy(&self) -> bool {
        self.hands_busy
    }

    /// Whether the hands are playing a move from the inventory the weapon
    /// is part of (a magazine out or in, a round into or out of the
    /// chamber, a barrel or a cylinder's chamber): the game's inventory
    /// operation still running, during which the weapon's own
    /// `CheckAction` fails and a slot's key is not taken
    /// (`PlayerInputTranslator.CurrentWeaponIsUsedInRemoveOperation`).
    pub fn in_inventory_move(&self) -> bool {
        // (The rack's own runs until its round is thrown, the hands' answer
        // it waits on: `WeaponRechamberOperation`'s `OutProcess`.)
        matches!(
            self.operation,
            Operation::InventoryMagOut { .. } | Operation::InventoryMagIn { .. } | Operation::InventoryChamber | Operation::InventoryDischarge | Operation::OwnChamberIn { .. } | Operation::OwnChamberOut { .. } | Operation::Rechamber { ejected: false, .. }
        )
    }

    /// Whether the weapon's logic has in hand something the game does not
    /// count as idling: a shot or automatic fire, a reload up to where it
    /// hands over, a look at the weapon, a draw, a put-away. Switching the
    /// fire mode is the idling operation's own work. The movement is told
    /// (`movement::Input::weapon_occupied`): with the weapon so taken up,
    /// only an obstacle low enough to leave the hands on it is vaulted or
    /// climbed (vaulting spec, "What a try does").
    pub fn occupied(&self) -> bool {
        !matches!(self.operation, Operation::Idle | Operation::Away)
    }

    /// Whether a look at the weapon is in hand.
    pub fn checking(&self) -> bool {
        matches!(self.operation, Operation::Utility { .. })
    }
}
