//! Reloading shells one at a time.

use super::*;

impl Firearm {
    /// Fills a magazine that stays in the weapon, a round at a time
    /// (`FirearmHandsInputTranslator.ReloadWithAmmo`, `Idling.
    /// ReloadWithAmmo`; shotgun spec, "Filling the tube"). `within_reach`
    /// is how many rounds the character has to load from; with none the
    /// key does nothing. A weapon at rest lowers its sights (below
    /// mastering 2) and lets the trigger go, and then **begins only if the
    /// tube has room**: a full tube under an empty chamber is refused.
    /// Asked for during a shot, it follows the shot if nothing else is to.
    /// `Event::RoundsTaken` says what has left the character's stock.
    pub fn reload_shells(&mut self, within_reach: u32) {
        self.reload_shells_as(within_reach, false);
    }

    /// The quick reload's key for a weapon filled a round at a time
    /// (`FirearmHandsInputTranslator.QuickReload`): heard only with the
    /// animator at rest, and not by a weapon used once; a mount is left
    /// first. A revolver's cylinder is reloaded the quick way
    /// (`ReloadRevolverDrum(quickReload: true)`; more-weapons spec, "A
    /// cylinder's quick reload, and the fistful"), begun even when full;
    /// two barrels or more are reloaded, and a tube filled, as by the
    /// plain key; a single barrel takes nothing.
    pub fn quick_reload_shells(&mut self, within_reach: u32) {
        if self.settings.one_off || !self.animator_idling() {
            return;
        }
        if self.settings.cylinder.is_some() {
            self.reload_shells_as(within_reach, true);
        } else if self.settings.barrels == 1 {
            self.reload_key();
        } else {
            self.reload_shells(within_reach);
        }
    }

    fn reload_shells_as(&mut self, within_reach: u32, quick: bool) {
        self.reload_key();
        if self.magazine.is_none() {
            return;
        }
        // **Any malfunction refuses it and reminds**, before the rounds are
        // looked for, but a misfire on a weapon whose bolt need not be
        // opened to reload, which the filling puts right
        // (`FirearmHandsInputTranslator.ReloadWithAmmo`; else a Mosin with
        // a malfunction would reload). Barrels and
        // a cylinder are loaded by other ways, which never meet one: their
        // shots roll none (`FireOnlyBarrelFireOperation`,
        // `FireCylinderMagOperation`).
        let malf = self.malfunction();
        if malf != Malf::None && !(malf == Malf::Misfire && !self.settings.must_open_bolt) {
            self.animator.set_flag("MisfireSlideUnknown", false);
            return self.remind();
        }
        if within_reach == 0 || self.blind_fire {
            return;
        }
        match &mut self.operation {
            Operation::Idle => {
                if self.reload_lowers_sights() {
                    self.aiming = false;
                }
                self.idle_trigger(false);
                // A single barrel is reloaded whatever it holds
                // (`ReloadSingleBarrelResult.Run`), and a cylinder the
                // quick way (`Idling.ReloadCylinderMagazine`).
                let room = self.magazine.is_some_and(|rounds| rounds < self.settings.magazine_capacity) || self.settings.barrels == 1 || quick;
                if room {
                    if self.settings.cylinder.is_some() {
                        self.begin_cylinder_reload(within_reach, quick);
                    } else if self.settings.barrels > 0 {
                        self.begin_barrels_reload(within_reach);
                    } else {
                        self.begin_shells(within_reach);
                    }
                }
            }
            Operation::Single { planned, .. } | Operation::Automatic { planned, .. } if planned.is_none() => {
                *planned = Some(Planned::Shells(within_reach));
                self.trigger_to(false);
                self.work_bolt_for_reload();
            }
            Operation::Single { .. } => self.work_bolt_for_reload(),
            _ => {}
        }
    }

    /// The operation's start (`BaseReloadOperation.Start`,
    /// `ReloadInternalMagBase.Start`, `ReloadInternalMagOperation.
    /// Prepare`): the animator is told, and **an empty chamber is given
    /// its round at once**; the animator hears of that at the mark of the
    /// round going in.
    pub(super) fn begin_shells(&mut self, mut within_reach: u32) {
        self.animator.set_flag("Inventory", false);
        self.animator.set_flag("CanReload", true);
        self.animator.trigger("Reload");
        self.animator.set_flag("IsExternalMag", false);
        self.hands_busy = true;
        // A misfire's layer is off while the filling plays
        // (`ReloadInternalMagOperation.Prepare`); the empty chamber's round
        // given below puts it right at its mark.
        if self.malfunction() == Malf::Misfire {
            self.animator.set_layer_weight("Malfunction", 0.0);
        }
        // The class's reload action as the operation begins
        // (`BaseReloadOperation.Start`).
        self.events.push(Event::Earn(Earn::Reload));
        let mut fed = false;
        if self.settings.open_bolt_reload {
            // The bolt is opened for it: a chambered round leaves the
            // weapon at once (`ReloadInternalMagWithOpenBoltOperation.
            // Prepare`), and is thrown out at the chamber's mark.
            self.chamber = false;
        } else if !self.chamber && within_reach > 0 {
            self.chamber = true;
            within_reach -= 1;
            fed = true;
            self.events.push(Event::RoundsTaken(1));
        }
        self.operation = Operation::Shells {
            within_reach,
            loaded: 0,
            can_abort: false,
            aborted: false,
            chambered: false,
            emptied: false,
            fed,
            hide: None,
        };
    }

    /// Whether another round is to follow (`ReloadInternalMagBase.
    /// CanReload`): there is one left to load from, and the tube is not
    /// full with those already on their way in.
    fn shells_go_on(&self) -> bool {
        let Operation::Shells { within_reach, loaded, .. } = self.operation else { return false };
        within_reach > loaded && self.magazine.unwrap_or(0) + loaded != self.settings.magazine_capacity
    }

    /// The mark of the round going into the chamber, answered once
    /// (`ReloadInternalMagOperation.OnAddAmmoInChamber`).
    pub(super) fn shell_chambered(&mut self) {
        let Operation::Shells { can_abort, chambered, fed, .. } = &mut self.operation else { return };
        (*can_abort, *chambered) = (true, true);
        // The round given to the empty chamber as the filling began goes in:
        // the class's chamber action (`ReloadInternalMagOperation.
        // OnAddAmmoInChamber`).
        if *fed {
            self.events.push(Event::Earn(Earn::Chamber));
        }
        // The round chambered puts a misfire right (`ReloadInternalMagOperation.
        // OnAddAmmoInChamber`, `method_2`).
        if self.chamber && !self.settings.open_bolt_reload {
            self.clear_misfire();
        }
        // With the bolt opened for the filling this is the bolt going
        // home, which ends it (`ReloadInternalMagWithOpenBoltOperation.
        // OnAddAmmoInChamber`).
        if self.settings.open_bolt_reload {
            self.shells_over();
            return;
        }
        self.animator.set("AmmoInChamber", self.chamber as u8 as f32);
        let goes_on = self.shells_go_on();
        self.animator.set_flag("CanReload", goes_on);
        if !goes_on || matches!(self.operation, Operation::Shells { aborted: true, .. }) {
            self.shells_over();
        }
    }

    /// The mark of a round going into the tube (`AddAmmoToMag`).
    pub(super) fn shell_in(&mut self) {
        let Operation::Shells { loaded, can_abort, .. } = &mut self.operation else { return };
        *can_abort = true;
        *loaded += 1;
        // Mag Drills' round loaded, at each round's mark
        // (`ReloadInternalMagBase`'s `AddAmmoToMag`, both fillings).
        self.events.push(Event::Earn(Earn::RoundLoaded));
        let in_tube = self.magazine.unwrap_or(0) + *loaded;
        self.animator.set("AmmoInMag", in_tube as f32);
        let aborted = matches!(self.operation, Operation::Shells { aborted: true, .. });
        // With the bolt open the filling does not end here: the animator
        // is told whether another round follows, and shuts the bolt when
        // none does.
        if self.settings.open_bolt_reload {
            let goes_on = self.shells_go_on() && !aborted;
            self.animator.set_flag("CanReload", goes_on);
            return;
        }
        if !self.shells_go_on() || aborted {
            self.shells_over();
        }
    }

    /// The chamber's mark in a filling with the bolt open, answered once:
    /// the round that was chambered is thrown out
    /// (`ReloadInternalMagWithOpenBoltOperation.RemoveAmmoFromChamber`).
    pub(super) fn shell_thrown_out(&mut self) {
        let Operation::Shells { emptied, .. } = &mut self.operation else { return };
        if std::mem::replace(emptied, true) {
            return;
        }
        self.animator.set("AmmoInChamber", 0.0);
        self.events.push(Event::RoundEjected);
    }

    /// The filling hands over to idling (`SwitchToIdle`): the animator is
    /// told whether more would have followed, the rounds loaded are the
    /// weapon's from now, and the animation plays out.
    fn shells_over(&mut self) {
        let Operation::Shells { loaded, aborted, hide, .. } = self.operation else { return };
        let goes_on = self.shells_go_on() && !aborted && !self.settings.open_bolt_reload;
        self.animator.set_flag("CanReload", goes_on);
        self.magazine = self.magazine.map(|rounds| rounds + loaded);
        if loaded > 0 {
            self.events.push(Event::RoundsTaken(loaded));
        }
        // The commit: the class's reload action for each round it puts in
        // (`CommitReloadWithAmmo`'s loop, both fillings).
        for _ in 0..loaded {
            self.events.push(Event::Earn(Earn::Reload));
        }
        // The bolt shut on a filled magazine takes its first round into
        // the empty chamber (`CommitReloadWithAmmo`): only if something
        // was loaded, with the class's chamber action.
        if self.settings.open_bolt_reload {
            if loaded > 0 && !self.chamber {
                self.events.push(Event::Earn(Earn::Chamber));
                self.take_round();
            }
            self.animator.set("AmmoInChamber", self.chamber as u8 as f32);
        }
        self.animator.set("AmmoInMag", self.magazine.unwrap_or(0) as f32);
        // A misfire still there has its layer back (`SwitchToIdle`).
        if self.malfunction() == Malf::Misfire {
            self.animator.set_layer_weight("Malfunction", 1.0);
        }
        self.to_idle();
        if let Some(fast) = hide {
            self.hide(fast);
        }
    }
}
