//! Counting, filling and showing the rounds and cases in a firearm's chambers.

use super::*;

impl Firearm {
    /// Whether the weapon's rounds lie in chambers of their own.
    pub(crate) fn has_chambers(&self) -> bool {
        self.settings.cylinder.is_some() || self.settings.barrels > 0
    }

    /// The chambers holding a live round (`CylinderMagazine.Count`,
    /// `Weapon.ChamberAmmoCount`).
    pub(super) fn rounds(&self) -> u32 {
        self.in_chambers.iter().filter(|chamber| chamber.round).count() as u32
    }

    /// The chambers holding a case (`Weapon.ShellsInWeaponCount`).
    pub(super) fn cases(&self) -> u32 {
        self.in_chambers.iter().filter(|chamber| chamber.case).count() as u32
    }

    /// The rounds as the rest of the logic and a host count them: all of
    /// them "in the magazine", none chambered apart.
    pub(crate) fn count_chambers(&mut self) {
        self.magazine = Some(self.rounds());
        self.chamber = false;
    }

    /// What the idling operation tells the animator of the chambers.
    pub(crate) fn animate_chambers(&mut self) {
        let (live, shells) = (self.rounds(), self.cases());
        self.animator.set("AmmoInMag", live as f32);
        // The game's count of rounds in the weapon's own chambers
        // (`Weapon.ChamberAmmoCount`): a cylinder's chambers are its
        // magazine's, not the weapon's, so a revolver's is 0 whatever the
        // cylinder holds (recorded: the Rhino's stays 0 from five rounds
        // to none, `local\traces\rhino-s160`; told the rounds, a pull on an
        // empty chamber with rounds left in others was played as a shot,
        // the animator's way to its dry fire going by `AmmoInChamber < 1`).
        // Barrels are the weapon's own: all of their rounds (`PROVISIONAL`).
        let own = if self.settings.cylinder.is_some() { 0 } else { live };
        self.animator.set("AmmoInChamber", own as f32);
        self.animator.set("ShellsInWeapon", shells as f32);
        if self.settings.cylinder.is_some() {
            self.animator.set("CamoraIndex", self.camora as f32);
            self.animator.set("DoubleActionFireModeFloat", self.hammer_closed as u8 as f32);
        }
    }

    /// Every chamber emptied or filled, for a weapon made or found.
    pub(crate) fn fill_chambers(&mut self, loaded: bool) {
        let chambers = self.settings.cylinder.map_or(self.settings.barrels, |(chambers, _)| chambers) as usize;
        self.in_chambers = vec![Chamber { round: loaded, case: false }; chambers];
        // As the hands are made, a round at each loaded chamber's point (a
        // rocket launcher's rocket too: `FirearmController.Setup`).
        self.shown = vec![Shown { round: loaded, case: false }; chambers];
        (self.camora, self.hammer_closed) = (0, false);
        self.count_chambers();
    }

    /// What the weapon's model holds at each of its chambers of its own,
    /// by the chambers' numbers; none for any other weapon.
    pub fn chambers_shown(&self) -> &[Shown] {
        &self.shown
    }

    /// The rounds and the cases the host's item holds in the chambers, by
    /// their numbers (the game's `Chambers` or `Camoras`, and `Weapon.
    /// ShellsInChambers`), taken by the logic. `made`: the hands are made
    /// from the item, the weapon newly in hand (`FirearmController.Setup`):
    /// a round at the point of each chamber with one, a case at the port of
    /// each with one, and then the cases the draw takes away
    /// (`cases_drawn_away`). Otherwise the item and the logic are set
    /// together without the hands: the logic takes the item's rounds at
    /// once, and a chamber given a round loses its case, unthrown, as the
    /// hands' load of one would (`LoadCartridgeToCamora.OnRemoveShellEvent`).
    /// The host plays a move of the inventory's by the hands instead
    /// (`load_own_chamber`, `unload_own_chamber`), and takes it so only
    /// where they cannot. Whether the count of chambers was the item's.
    pub fn hold_rounds(&mut self, rounds: &[bool], cases: &[bool], made: bool) -> bool {
        if rounds.len() != self.in_chambers.len() {
            return false;
        }
        for (index, &round) in rounds.iter().enumerate() {
            let had = self.in_chambers[index];
            let case = match made {
                true => cases.get(index).copied().unwrap_or(false),
                false => had.case && (had.round || !round),
            };
            self.in_chambers[index] = Chamber { round, case };
            // A rocket launcher's spent rocket stays at its port, which the
            // weapon keeps no case for (`RocketLauncherFire` sets none),
            // until the hands are made again.
            let kept = self.settings.rocket_launcher && !made;
            if let Some(shown) = self.shown.get_mut(index) {
                *shown = Shown { round, case: if kept { shown.case } else { case } };
            }
        }
        if made {
            self.cases_drawn_away();
        }
        self.count_chambers();
        true
    }

    /// The cases the draw takes away from the weapon and its model, unthrown
    /// (`SpawnOperation.Start`, `RemoveShellInWeapon`): those of a cylinder's
    /// chambers that hold a round, of the chambers numbered below its count
    /// of rounds alone (the game's loop runs to `CylinderMagazine.Count`,
    /// not to its chambers' number), and of every barrel that holds one
    /// where there are two or more (`Weapon.IsMultiBarrel`); a single
    /// barrel keeps its case.
    pub(crate) fn cases_drawn_away(&mut self) {
        let (cylinder, count) = (self.settings.cylinder.is_some(), self.rounds() as usize);
        let several = !cylinder && self.in_chambers.len() > 1;
        for index in 0..self.in_chambers.len() {
            let reached = if cylinder { index < count } else { several };
            if reached && self.in_chambers[index].round {
                self.in_chambers[index].case = false;
                if let Some(shown) = self.shown.get_mut(index) {
                    shown.case = false;
                }
            }
        }
    }

    /// The case at a chamber's port thrown, where there is one.
    fn throw_case(&mut self, index: usize, share: Option<f32>) {
        if let Some(shown) = self.shown.get_mut(index).filter(|shown| shown.case) {
            shown.case = false;
            self.events.push(Event::CaseThrown { chamber: index, share });
        }
    }

    /// Every case at the ports thrown (`StartSpawnAllShells`).
    fn throw_cases(&mut self, share: Option<f32>) {
        for index in 0..self.shown.len() {
            self.throw_case(index, share);
        }
    }

    /// A round shown at the point of a chamber a reload loads, the `nth`
    /// of the reload's rounds.
    fn show_loaded(&mut self, index: usize, nth: usize) {
        if let Some(shown) = self.shown.get_mut(index) {
            shown.round = true;
            self.events.push(Event::RoundShown { chamber: index, nth });
        }
    }

    /// The model's rounds and cases at one of the animator's marks, before
    /// the logic answers it (more-weapons spec, "The rounds and cases seen
    /// in chambers of their own").
    pub(crate) fn show_mark(&mut self, name: &str) {
        if self.shown.len() != self.in_chambers.len() {
            return;
        }
        let cylinder = self.settings.cylinder.is_some();
        let single = !cylinder && self.in_chambers.len() == 1;
        let firing = matches!(self.operation, Operation::CylinderFire { fired: false, dry: false } | Operation::BarrelsFire { fired: false, .. });
        let reloading = matches!(self.operation, Operation::ChambersReload { .. });
        match name {
            // The shot: each round fired goes to its port as its case
            // (`OnFireEvent`, `MoveAmmoFromChamberToShellPort`), in place
            // of any case there (`SetPatronInShellPort`).
            "FiringBullet" if firing => {
                for index in self.fired_chambers.clone() {
                    if let Some(shown) = self.shown.get_mut(index) {
                        *shown = Shown { round: false, case: true };
                        self.events.push(Event::CaseLeft { chamber: index });
                    }
                }
            }
            // A cylinder's case out, the one named for it; with no live
            // round in the cylinder, every case at once, straight down
            // (`OnRemoveShellEvent`, `RemoveAllShells`).
            // The quick reload's the same, and its live rounds gone from
            // their points (`RemoveAllAmmo`: `DestroyAllPatronsInWeapon`).
            "RemoveShell" if reloading && cylinder => {
                if self.rounds() == 0 || self.reload_quick {
                    self.throw_cases(None);
                    for shown in &mut self.shown {
                        shown.round = false;
                    }
                } else if let Ok(index) = usize::try_from(self.reload_remove) {
                    self.throw_case(index, Some(RELOAD_SHARE));
                }
            }
            // A round for the next free chamber, if one is in reach
            // (`SetCamoraIndexForLoadAmmo`); a fistful's are all shown at
            // once.
            "AddAmmoInChamber" if reloading && cylinder && !self.reload_fistful => {
                if self.reload_loading < self.reload_free.len() && (self.reload_loading as u32) < self.reload_reach {
                    self.show_loaded(self.reload_free[self.reload_loading], self.reload_loading);
                }
            }
            // A fistful in the hand: a round at each chamber's point from
            // the first, as many as are in reach
            // (`SetAllAmmoToHandsWithMasteringReload`).
            "MagShow" if reloading && cylinder && self.reload_fistful => {
                let count = self.in_chambers.len().min(self.reload_reach as usize);
                for index in 0..count {
                    self.show_loaded(index, index);
                }
            }
            // A fistful cut short: the rounds shown that did not go in are
            // taken back (`RemoveExtraPatronsInHandsAfterMasteringReload`).
            "DelAmmoChamber" if reloading && cylinder && self.reload_fistful => {
                let count = self.reload_free.len().min(self.reload_reach as usize);
                for nth in self.reload_loading..count {
                    if let Some(shown) = self.reload_free.get(nth).and_then(|index| self.shown.get_mut(*index)) {
                        shown.round = false;
                    }
                }
            }
            // A single barrel's live round out of it, or else its case
            // thrown with the whole of the player's velocity
            // (`ReloadSingleBarrelOperation.OnShellEjectEvent`).
            "ShellEject" if reloading && single => {
                if self.reload_unloads {
                    self.shown[0].round = false;
                } else if self.in_chambers[0].case {
                    self.throw_case(0, Some(1.0));
                }
            }
            // Barrels' cases thrown together, where the weapon held any
            // as the reload began (`ReloadMultiBarrelOperation.
            // OnShellEjectEvent`).
            "ShellEject" if reloading && !cylinder => {
                if self.cases() > 0 {
                    self.throw_cases(Some(RELOAD_SHARE));
                }
            }
            // Any case left at the ports taken away (`RemoveAllShells`).
            "RemoveShell" if reloading && !cylinder => {
                for shown in &mut self.shown {
                    shown.case = false;
                }
            }
            // The new rounds at the points of the barrels being loaded
            // (`OnMagAppeared`).
            "MagShow" if reloading && !cylinder => {
                for (nth, index) in self.reload_free.clone().into_iter().enumerate() {
                    self.show_loaded(index, nth);
                }
            }
            _ => {}
        }
    }
}
