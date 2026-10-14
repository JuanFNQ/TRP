//! Weapons whose rounds lie in chambers of their own and not in a
//! magazine under a bolt: a revolver's or a launcher's cylinder, and
//! barrels loaded one by one (`docs/spec/more-weapons.md`). The game has an
//! operation of its own for the firing and for the reloading of each
//! (`FireCylinderMagOperation`, `ReloadCylinderMagOperation`,
//! `FireOnlyBarrelFireOperation`, `ReloadMultiBarrelOperation`); these are
//! those, on the weapon's own animator.

use super::{Event, FireMode, Firearm, Operation, FIRE_STATE_START};

/// What a chamber of a cylinder, or a barrel, holds: nothing, a round, or
/// the case of one that was fired (which stays until a reload takes it
/// out).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chamber {
    Empty,
    Round,
    Shell,
}

/// The state a cylinder fires double action from, with the hammer down:
/// the game plays it outright (`FireOperation.StartFireAnimation`).
const DOUBLE_ACTION_FIRE: &str = "Hands.DOUBLE_ACTION_FIRE";

impl Firearm {
    /// Whether the weapon's rounds lie in chambers of their own.
    pub(super) fn has_chambers(&self) -> bool {
        self.settings.cylinder.is_some() || self.settings.barrels > 0
    }

    fn count(&self, what: Chamber) -> u32 {
        self.in_chambers.iter().filter(|chamber| **chamber == what).count() as u32
    }

    /// The rounds as the rest of the logic and a host count them: all of
    /// them "in the magazine", none chambered apart.
    pub(super) fn count_chambers(&mut self) {
        self.magazine = Some(self.count(Chamber::Round));
        self.chamber = false;
    }

    /// What the idling operation tells the animator of the chambers.
    pub(super) fn animate_chambers(&mut self) {
        let (live, shells) = (self.count(Chamber::Round), self.count(Chamber::Shell));
        self.animator.set("AmmoInMag", live as f32);
        // PROVISIONAL: the game's count of rounds "in the chamber" is
        // taken to be the rounds in all of them (its chambers are the
        // cylinder's own, or the barrels').
        self.animator.set("AmmoInChamber", live as f32);
        self.animator.set("ShellsInWeapon", shells as f32);
        if self.settings.cylinder.is_some() {
            self.animator.set("CamoraIndex", self.camora as f32);
            self.animator.set("DoubleActionFireModeFloat", self.hammer_closed as u8 as f32);
        }
    }

    /// Every chamber emptied or filled, for a weapon made or found.
    pub(super) fn fill_chambers(&mut self, loaded: bool) {
        let chambers = self.settings.cylinder.map_or(self.settings.barrels, |(chambers, _)| chambers) as usize;
        self.in_chambers = vec![if loaded { Chamber::Round } else { Chamber::Empty }; chambers];
        (self.camora, self.hammer_closed) = (0, false);
        self.count_chambers();
    }

    // ------------------------------------------------------- a cylinder

    fn spring_driven(&self) -> bool {
        self.settings.cylinder.is_some_and(|(_, spring)| spring)
    }

    /// The chamber that fires next, or that loading starts from
    /// (`GetCamoraFireOrLoadStartIndex`): the one under the hammer while
    /// it is cocked, and the next round the cylinder while it is down (a
    /// cylinder a spring turns has always the one under it).
    fn camora_in_turn(&self) -> usize {
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

    /// The trigger of a weapon with a cylinder, at rest
    /// (`Idling.SetTriggerPressed`, `FireCylinderMagOperation.Start`): a
    /// press fires the chamber in turn, or falls on an empty one.
    pub(super) fn cylinder_trigger(&mut self, pressed: bool) {
        self.trigger = pressed;
        if !pressed {
            self.animator.set_flag("Fire", false);
            return;
        }
        let double = self.mode() == FireMode::DoubleAction;
        let index = self.camora_in_turn();
        self.animator.set("CamoraFireIndex", self.camora as f32);
        if self.in_chambers.get(index) != Some(&Chamber::Round) {
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
        // The round is taken out of its chamber and its case left there
        // (`PrepareShot`); the cylinder turns on now unless the shot is
        // the first of double action from a cocked hammer.
        self.in_chambers[index] = Chamber::Shell;
        let from_closed = self.hammer_closed;
        if self.hammer_closed || !double {
            self.turn_cylinder();
        }
        self.animator.set("CamoraIndex", self.camora as f32);
        self.animator.set_flag("Fire", self.trigger);
        if from_closed {
            self.animator.play(DOUBLE_ACTION_FIRE, FIRE_STATE_START);
        }
        self.count_chambers();
        self.animator.set("AmmoInMag", self.count(Chamber::Round) as f32);
        self.operation = Operation::CylinderFire { fired: false, dry: false };
    }

    /// The fire mode of a weapon with a cylinder has been changed
    /// (`SetFireModeForCylinderMagazine`): to single action, the hammer is
    /// cocked, which turns the cylinder on if it was down.
    pub(super) fn cylinder_mode_changed(&mut self) {
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

    /// The cylinder's reload begins (`ReloadCylinderMagOperation.Prepare`):
    /// which chambers are free, from the one in turn round, and which
    /// hold cases; the animator is told how many of each and which comes
    /// first. The fistful a character who knows the weapon loads into an
    /// empty cylinder, and the quick reload that throws everything out,
    /// are not built.
    pub(super) fn begin_cylinder_reload(&mut self, within_reach: u32) {
        let chambers = self.in_chambers.len();
        let live = self.count(Chamber::Round);
        if self.spring_driven() {
            self.camora = 0;
        }
        let start = self.camora_in_turn();
        let order: Vec<usize> = if self.spring_driven() { (0..chambers).collect() } else { (start..chambers).chain(0..start).collect() };
        self.reload_free = order.into_iter().filter(|index| self.in_chambers[*index] != Chamber::Round).collect();
        self.reload_shells = (0..chambers).filter(|index| self.in_chambers[*index] == Chamber::Shell).collect();
        (self.reload_loading, self.reload_reach, self.reload_cached, self.reload_remove) = (0, within_reach, live, -1);
        self.animator.set("WeaponLevel", 0.0);
        self.animator.set("ShellsInWeapon", self.reload_shells.len() as f32);
        self.animator.set("AmmoInMag", live as f32);
        if live != 0 {
            self.next_shell_to_remove();
            self.animator.set("CamoraIndexWithShellForRemove", self.reload_remove as f32);
        }
        self.animator.set("AmmoCountForRemove", self.reload_shells.len() as f32);
        self.next_chamber_to_load();
        // The base operation's start, as for any weapon filled a round
        // at a time.
        self.animator.set_flag("Inventory", false);
        self.animator.set_flag("CanReload", true);
        self.animator.trigger("Reload");
        self.hands_busy = true;
        self.operation = Operation::ChambersReload { can_abort: false, aborted: false, added: false, hide: None };
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

    /// Whether another round is to follow (`CanReload`).
    fn cylinder_goes_on(&self) -> bool {
        self.reload_reach > self.reload_loading as u32 && self.reload_cached + self.reload_loading as u32 != self.in_chambers.len() as u32
    }

    /// A mark of the animator's while a cylinder fires or is reloaded.
    /// Whether it was this operation's to answer.
    pub(super) fn cylinder_mark(&mut self, name: &str) -> bool {
        let double = self.mode() == FireMode::DoubleAction;
        match (name, self.operation.clone()) {
            ("FiringBullet", Operation::CylinderFire { fired: false, dry }) => {
                self.operation = Operation::CylinderFire { fired: true, dry };
                self.shot_made = true;
            }
            ("AddAmmoInChamber", Operation::CylinderFire { .. }) => {
                self.animator.set("AmmoInMag", self.count(Chamber::Round) as f32);
                self.hammer_closed = double;
                self.animator.set("DoubleActionFireModeFloat", self.hammer_closed as u8 as f32);
            }
            ("FireEnd", Operation::CylinderFire { dry: true, .. }) => {
                self.animator.set("DoubleActionFireModeFloat", self.hammer_closed as u8 as f32);
            }
            ("IdleStart", Operation::CylinderFire { fired: true, .. }) => {
                self.trigger = false;
                self.animator.set_flag("Fire", false);
                self.to_idle();
            }
            // The case stays in its chamber: nothing is thrown out.
            ("ShellEject" | "DelAmmoChamber", Operation::CylinderFire { .. }) => {}
            ("RemoveShell", Operation::ChambersReload { .. }) => {
                if self.count(Chamber::Round) == 0 {
                    for chamber in self.in_chambers.iter_mut().filter(|chamber| **chamber == Chamber::Shell) {
                        *chamber = Chamber::Empty;
                    }
                    self.reload_shells.clear();
                    self.animator.set("ShellsInWeapon", 0.0);
                } else {
                    if let Some(chamber) = usize::try_from(self.reload_remove).ok().and_then(|index| self.in_chambers.get_mut(index)) {
                        *chamber = Chamber::Empty;
                    }
                    self.next_shell_to_remove();
                }
            }
            ("ShellEject", Operation::ChambersReload { .. }) => {
                self.animator.set("CamoraIndexWithShellForRemove", self.reload_remove as f32);
            }
            // A round shows in the hand for the next free chamber.
            ("AddAmmoInChamber", Operation::ChambersReload { .. }) => {
                if self.reload_loading < self.reload_free.len() && (self.reload_loading as u32) < self.reload_reach {
                    self.animator.set("CamoraIndex", self.camora as f32);
                    self.animator.set("CamoraIndexForLoadAmmo", self.reload_free[self.reload_loading] as f32);
                    self.reload_loading += 1;
                }
                self.animator.set("AmmoInMag", (self.reload_cached + self.reload_loading as u32) as f32);
            }
            // It is in: another follows, or the cylinder is shut.
            ("AddAmmoInMag", Operation::ChambersReload { aborted, added, hide, .. }) => {
                self.next_chamber_to_load();
                self.operation = Operation::ChambersReload { can_abort: true, aborted, added, hide };
                self.animator.set("AmmoInMag", (self.reload_cached + self.reload_loading as u32) as f32);
                if !self.cylinder_goes_on() || aborted {
                    self.animator.set("CamoraIndex", self.camora as f32);
                    self.animator.set_flag("CanReload", false);
                }
            }
            // The cylinder shut and the hand back: the rounds shown are
            // the weapon's from now (`OnMagPuttedToRig`, `Commit`).
            ("MagHide", Operation::ChambersReload { hide, .. }) => {
                self.animator.set("CamoraIndex", self.camora as f32);
                self.animator.set_flag("CanReload", false);
                let loaded = self.reload_loading;
                for index in self.reload_free.iter().take(loaded) {
                    self.in_chambers[*index] = Chamber::Round;
                }
                if loaded > 0 {
                    self.events.push(Event::RoundsTaken(loaded as u32));
                }
                // With the hammer cocked the first loaded chamber from
                // the one under it comes under the hammer.
                if !self.hammer_closed {
                    let chambers = self.in_chambers.len();
                    let next = if self.spring_driven() { Some(0) } else { (0..chambers).map(|step| (self.camora + step) % chambers).find(|index| self.in_chambers[*index] == Chamber::Round) };
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

    // ---------------------------------------------------------- barrels

    /// The trigger of a weapon whose barrels are loaded one by one, at
    /// rest (`FireOnlyBarrelFireOperation.PrepareShot`): one barrel a
    /// press, the first that is loaded, or all of them together in the
    /// mode for that.
    pub(super) fn barrels_trigger(&mut self, pressed: bool) {
        self.trigger = pressed;
        let loaded: Vec<usize> = (0..self.in_chambers.len()).filter(|index| self.in_chambers[*index] == Chamber::Round).collect();
        if !pressed || loaded.is_empty() {
            // The animator has the trigger, and a press only clicks.
            self.animator.set_flag("Fire", pressed);
            if pressed {
                self.events.push(Event::DryFire);
            }
            return;
        }
        let fired = if self.mode() == FireMode::Doublet { loaded.len() } else { 1 };
        for index in loaded.into_iter().take(fired) {
            self.in_chambers[index] = Chamber::Shell;
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
        (self.count(Chamber::Round) + self.reload_free.len() as u32) as f32
    }

    /// The barrels' reload begins (`ReloadMultiBarrelOperation.Start`):
    /// every barrel that holds no round is to have one, as far as the
    /// rounds in reach go.
    pub(super) fn begin_barrels_reload(&mut self, within_reach: u32) {
        self.reload_free = (0..self.in_chambers.len()).filter(|index| self.in_chambers[*index] != Chamber::Round).take(within_reach as usize).collect();
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
        self.animator.set("ShellsInWeapon", self.count(Chamber::Shell) as f32);
        self.animator.set("AmmoInChamber", self.rounds_once_loaded());
        self.hands_busy = true;
        self.operation = Operation::ChambersReload { can_abort: false, aborted: false, added: false, hide: None };
    }

    /// A mark of the animator's while barrels fire or are reloaded.
    pub(super) fn barrels_mark(&mut self, name: &str) -> bool {
        match (name, self.operation.clone()) {
            ("FiringBullet", Operation::BarrelsFire { fired: false, shots }) => {
                self.operation = Operation::BarrelsFire { fired: true, shots };
                for _ in 0..shots {
                    self.events.push(Event::Shot { strength: self.settings.shot_strength });
                }
                self.trigger = false;
                self.animator.set_flag("Fire", false);
                self.animator.set("AmmoInChamber", self.count(Chamber::Round) as f32);
                self.animator.set("ShellsInWeapon", self.count(Chamber::Shell) as f32);
            }
            ("IdleStart", Operation::BarrelsFire { fired: true, .. }) => self.to_idle(),
            ("ShellEject" | "DelAmmoChamber" | "AddAmmoInChamber", Operation::BarrelsFire { .. }) => {}
            // The cases are out.
            ("RemoveShell", Operation::ChambersReload { .. }) => {
                for chamber in self.in_chambers.iter_mut().filter(|chamber| **chamber == Chamber::Shell) {
                    *chamber = Chamber::Empty;
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
                    self.in_chambers[index] = Chamber::Round;
                }
                self.events.push(Event::RoundsTaken(loaded as u32));
                self.count_chambers();
                self.animator.set("AmmoInChamber", self.count(Chamber::Round) as f32);
                self.operation = Operation::ChambersReload { can_abort, aborted, added: true, hide };
            }
            // The weapon shut: the reload hands over to idling.
            ("AddAmmoInMag", Operation::ChambersReload { hide, .. }) => {
                self.animator.set_flag("CanReload", false);
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
