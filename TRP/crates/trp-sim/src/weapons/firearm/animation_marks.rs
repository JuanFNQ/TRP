//! What each animation mark does to the firearm.

use super::*;

impl Firearm {
    /// One of the animator's marks reaches the operation in hand.
    pub(super) fn on_mark(&mut self, mark: &Mark) {
        // A malfunction's repair answers its own marks (`RepairMalfunction`).
        if self.repair_mark(&mark.name) {
            return;
        }
        // A round moved into a chamber of its own or out of one from the
        // inventory answers its own marks, the model's with the logic's.
        if matches!(self.operation, Operation::OwnChamberIn { .. } | Operation::OwnChamberOut { .. }) && self.own_chamber_mark(&mark.name) {
            return;
        }
        // Chambers of their own answer their operations' marks first, the
        // model's rounds and cases moved before the logic's step.
        if matches!(self.operation, Operation::CylinderFire { .. } | Operation::BarrelsFire { .. } | Operation::ChambersReload { .. }) {
            self.show_mark(&mark.name);
            let answered = if self.settings.cylinder.is_some() { self.cylinder_mark(&mark.name) } else { self.barrels_mark(&mark.name) };
            if answered {
                return;
            }
        }
        let misfired = self.malfunction() == Malf::Misfire;
        match (mark.name.as_str(), &mut self.operation) {
            ("Arm", _) => {
                self.armed = true;
                self.animator.set_flag("Armed", true);
                self.animator.set_layer_weight("Hammer", 0.0);
            }
            ("Disarm", _) => {
                self.armed = false;
                self.animator.set_flag("Armed", false);
                self.animator.set_layer_weight("Hammer", 1.0);
            }
            // A rocket launcher's sights up and down as its own clips have
            // them (`FirearmController.OnAimReady`, `OnIdleReady`): what
            // its resting state's way to firing waits for.
            ("AimReady", _) if self.settings.rocket_launcher => self.animator.set("IsAimingFloat", 1.0),
            ("IdleReady", _) if self.settings.rocket_launcher => self.animator.set("IsAimingFloat", 0.0),
            ("Sound", _) => self.events.push(Event::Sound(mark.text.clone())),
            ("BipodOpen", _) => self.events.push(Event::BipodSound(true)),
            ("BipodClose", _) => self.events.push(Event::BipodSound(false)),
            ("ThirdAction", _) => self.events.push(Event::BodyAction(mark.int)),
            ("IdleStart", _) => self.idle_started(),
            ("WeapIn", Operation::Spawn { .. }) => self.appeared(),
            ("WeapOut", Operation::Remove) => {
                self.operation = Operation::Away;
                self.events.push(Event::Hidden);
            }
            // The shot itself, a frame after its round was taken.
            ("FiringBullet", Operation::Single { fired, malf, .. }) => {
                *fired = true;
                let malf = *malf;
                // Its sound with the trigger still held, then let go
                // (`FireOperation.OnFireEvent`).
                self.events.push(Event::FireSound { trigger: self.trigger, doublet: false });
                self.shot_made = true;
                self.trigger_to(false);
                self.animator.set_flag("Fire", false);
                self.shot_worn();
                if malf != Malf::None {
                    self.switch_to_malfunction(malf, false);
                }
            }
            // A malfunction's state: its case out (a failure to feed's), the
            // chamber shown empty, and at its mark the weapon at rest with it.
            ("ShellEject", Operation::Malfunction { .. }) => self.events.push(Event::ShellEject),
            ("DelAmmoChamber", Operation::Malfunction { .. }) => self.animator.set("AmmoInChamber", 0.0),
            ("MalfunctionOff", Operation::Malfunction { .. }) => self.malfunction_held(),
            ("MalfunctionOff", Operation::ExamineMalfunction { .. }) => self.malfunction_examined(),
            ("FiringBullet", Operation::Automatic { running, .. }) => *running = true,
            ("ShellEject", Operation::Single { .. }) => self.events.push(Event::ShellEject),
            ("DelAmmoChamber", Operation::Single { .. }) => self.animator.set("AmmoInChamber", 0.0),
            ("AddAmmoInChamber", Operation::Single { .. }) => {
                self.round_out = false;
                self.animator_rounds();
                self.animator.set_flag("Fire", self.trigger);
            }
            ("FireEnd", Operation::Single { .. }) => self.trigger_to(false),
            // A draw that racked the bolt.
            ("AddAmmoInChamber", Operation::Spawn { arms: true, .. }) => self.animator_rounds(),
            ("MagOut", Operation::Reload { mag_out, .. }) if !*mag_out => {
                *mag_out = true;
                self.magazine = None;
                self.events.push(Event::MagazineOut);
                self.animator.set_flag("Inventory", self.inventory);
                self.animator.set_flag("MagInWeapon", false);
                self.animator.set("MagInWeaponFloat", 0.0);
            }
            ("MagShow", Operation::Reload { mag_shown, .. }) if !*mag_shown => {
                *mag_shown = true;
                self.events.push(Event::MagazineShown);
                self.animator.set_flag("Inventory", self.inventory);
            }
            ("MagHide", Operation::Reload { mag_hidden, .. }) if !*mag_hidden => {
                *mag_hidden = true;
                self.events.push(Event::MagazineHidden);
            }
            // The bolt's catch in a magazine's reload: caught whenever the
            // mark says so, let go only the first time
            // (`ReloadExternalMagOperation.OnOnOffBoltCatchEvent`).
            ("OnBoltCatch", Operation::Reload { .. }) => self.set_bolt_catch(true),
            ("OffBoltCatch", Operation::Reload { catch_off, .. }) => {
                if !std::mem::replace(catch_off, true) {
                    self.set_bolt_catch(false);
                }
            }
            ("FoldOn" | "FoldOff", Operation::Fold { .. }) => self.folded_now(),
            // A magazine taken out from the inventory: out of the weapon at
            // its mark, put away at the hand's (`PullOutMagOperation.
            // OnMagPulledOutFromWeapon`, `OnMagPuttedToRig`).
            ("MagOut", Operation::InventoryMagOut { pulled }) if !*pulled => {
                *pulled = true;
                self.magazine = None;
                self.events.push(Event::MagazineOut);
                self.animator.set("AmmoInMag", 0.0);
                self.animator.set_flag("MagInWeapon", false);
                self.animator.set("MagInWeaponFloat", 0.0);
            }
            ("MagHide", Operation::InventoryMagOut { pulled: true }) => {
                self.events.push(Event::MagazineHidden);
                self.inventory_move_over();
            }
            // The rack (`RechamberOperation`): the round off the chamber's
            // point, thrown out as a thing, and the magazine's next fed
            // (`RemoveAmmoFromChamber`, `OnShellEjectEvent`,
            // `OnAddAmmoInChamber`; each answered once). The chamber is
            // empty from the throw, where the inventory takes the round
            // away (`WeaponRechamberOperation`: its discard, then the
            // magazine's top round moved into the chamber, which a chamber
            // that refuses it does not take).
            ("DelAmmoChamber", Operation::Rechamber { removed, .. }) if !*removed => {
                *removed = true;
                self.round_out = true;
                self.animator.set("AmmoInChamber", 0.0);
            }
            ("ShellEject", Operation::Rechamber { ejected, feeds, .. }) if !*ejected => {
                *ejected = true;
                let feeds = *feeds;
                self.chamber = false;
                self.events.push(Event::RoundRacked);
                if !feeds {
                    self.rechamber_over();
                }
            }
            ("AddAmmoInChamber", Operation::Rechamber { added, feeds: true, .. }) if !*added => {
                *added = true;
                self.take_round();
                self.round_out = false;
                self.rechamber_over();
            }
            ("OnBoltCatch" | "OffBoltCatch", Operation::Rechamber { .. }) => self.set_bolt_catch(mark.name == "OnBoltCatch"),
            // A cylinder turned (`RollCylinderOperation.OnMagPuttedToRig`):
            // the logic idles, then the cylinder turns on and the animator
            // is told.
            ("MagHide", Operation::RollCylinder) => {
                self.to_idle();
                self.cylinder_rolled();
            }
            // The chambered round taken out from the inventory, put away
            // (`DischargeOperation.OnMagPuttedToRig`).
            ("MagHide", Operation::InventoryDischarge) => {
                self.animator.set_flag("Discharge", false);
                self.chamber = false;
                self.animator_rounds();
                self.animator.set_flag("CanReload", false);
                self.events.push(Event::Discharged);
                self.to_idle();
            }
            // One put in from the inventory (`InsertMagOperation.
            // OnMagAppeared`, `OnMagInsertedToWeapon`, `OnAddAmmoInChamber`).
            ("MagShow", Operation::InventoryMagIn { shown, .. }) if !*shown => {
                *shown = true;
                self.events.push(Event::MagazineShown);
            }
            ("MagIn", Operation::InventoryMagIn { inserted, .. }) if !*inserted => {
                *inserted = true;
                self.magazine = Some(self.incoming);
                self.mag_variant = self.incoming_variant;
                self.mag_external = self.incoming_external;
                self.unfit = std::mem::take(&mut self.incoming_unfit);
                self.events.push(Event::MagazineIn);
                self.animator.set("AmmoInMag", self.incoming as f32);
                self.animator.set_flag("MagInWeapon", true);
                self.animator.set("MagInWeaponFloat", 1.0);
                // (`OnMagInsertedToWeapon`: the magazine as it went in.)
                let compatible = self.ammo_compatible();
                self.animator_ammo_compatible(compatible);
                if !self.chambers {
                    self.inventory_move_over();
                }
            }
            // A round put into the chamber from the inventory: in the hand
            // from the clip's `MagShow` (`LoadCartridgeToChamber.
            // OnMagAppeared` puts its model on the chamber's point), in the
            // chamber at `AddAmmoInChamber` (`OnAddAmmoInChamber`).
            ("MagShow", Operation::InventoryChamber) => self.round_in_hand = true,
            ("AddAmmoInChamber", Operation::InventoryChamber) => {
                self.chamber = true;
                self.animator_rounds();
                self.animator.set_flag("CanReload", false);
                self.animator.set_flag("LoadOne", false);
                self.events.push(Event::Chambered);
                // Mag Drills' round loaded as it goes in (`LoadCartridgeToChamber.
                // OnAddAmmoInChamber`).
                self.events.push(Event::Earn(Earn::RoundLoaded));
                self.to_idle();
            }
            ("AddAmmoInChamber", Operation::InventoryMagIn { inserted: true, .. }) if self.chambers => {
                self.chambers = false;
                self.take_round();
                self.animator_rounds();
                self.inventory_move_over();
            }
            ("MagIn", Operation::Reload { mag_in, .. }) if !*mag_in => {
                *mag_in = true;
                self.magazine = Some(self.incoming);
                self.unfit = std::mem::take(&mut self.incoming_unfit);
                self.events.push(Event::MagazineIn);
                self.animator.set_flag("MagInWeapon", true);
                self.animator.set("MagInWeaponFloat", 1.0);
                self.animator.set("AmmoInMag", self.incoming as f32);
                // With nothing to chamber the reload is over here; a weapon
                // with no chamber's goes on to the animation's rest
                // (`ReloadExternalMagResult`: only with chambers).
                if !self.chambers && !self.settings.no_chamber {
                    self.reload_over();
                }
                // (`OnMagInsertedToWeapon`: the new one's.)
                self.mag_variant = self.incoming_variant;
                self.mag_external = self.incoming_external;
                self.animator.set("RelTypeOld", self.mag_variant as f32);
            }
            ("AddAmmoInChamber", Operation::Reload { .. }) if self.chambers && misfired => {
                // A reload's chambering puts a misfire right (`ReloadExternalMagOperation.OnAddAmmoInChamber`).
                self.clear_misfire();
                self.chambers = false;
                self.take_round();
                self.animator_rounds();
            }
            ("AddAmmoInChamber", Operation::Reload { catch_off, .. }) if self.chambers => {
                let let_go = !std::mem::replace(catch_off, true);
                self.chambers = false;
                self.take_round();
                self.animator_rounds();
                // The bolt goes home on the round: its catch is let go
                // here if no mark has done it.
                if let_go {
                    self.set_bolt_catch(false);
                }
                self.reload_over();
            }
            ("StartUtilityOperation", Operation::Utility { ready, .. }) => *ready = true,
            // A tube being filled. (With the bolt opened for it the round
            // shown does not yet let it be cut short, and the chamber's
            // mark throws the old round out.)
            ("MagShow", Operation::Shells { can_abort, .. }) if !self.settings.open_bolt_reload => *can_abort = true,
            ("DelAmmoChamber", Operation::Shells { .. }) if self.settings.open_bolt_reload => self.shell_thrown_out(),
            ("AddAmmoInChamber", Operation::Shells { chambered: false, .. }) => self.shell_chambered(),
            ("AddAmmoInMag", Operation::Shells { .. }) => self.shell_in(),
            // The bolt caught open on the last round and let go again: a
            // shot answers only on a weapon that has a catch, a weapon at
            // rest, a look at it and a tube being filled always.
            ("OnBoltCatch" | "OffBoltCatch", Operation::Single { .. } | Operation::Automatic { .. }) if self.settings.bolt_catch => {
                self.set_bolt_catch(mark.name == "OnBoltCatch")
            }
            ("OnBoltCatch" | "OffBoltCatch", Operation::Idle | Operation::Utility { .. } | Operation::Shells { .. }) => {
                self.set_bolt_catch(mark.name == "OnBoltCatch")
            }
            _ => {}
        }
    }
}
