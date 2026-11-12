//! Setting the wear inputs and heat, and reading the malfunction state.

use super::*;

impl Firearm {
    /// Gives the weapon its wear and its malfunction state, as its item
    /// keeps them (at a draw, or a build made anew). A malfunction already
    /// there is set on the animator as the game's spawn of such a weapon
    /// does: its kind held, its layer on. Without one, the animator is put
    /// back as made (the logic may be one another weapon of the same build
    /// had before, malfunctioned).
    pub fn set_wear(&mut self, wear: Wear) {
        let malf = wear.state.malf;
        self.wear = Some(Box::new(wear));
        if malf != Malf::None {
            self.malfunction_ints(-1, malf.number());
            self.animator.set_layer_weight("Malfunction", 1.0);
        } else {
            self.malfunction_ints(0, 0);
            self.animator.set_layer_weight("Malfunction", 0.0);
        }
    }

    /// The wear and the state as they stand now, for the item.
    pub fn wear(&self) -> Option<&Wear> {
        self.wear.as_deref()
    }

    /// Its durability and greatest durability as the item now has them,
    /// changed elsewhere (a repair at the bench).
    pub fn set_durability(&mut self, durability: f32, max_durability: f32) {
        if let Some(w) = self.wear.as_deref_mut() {
            (w.durability, w.max_durability) = (durability, max_durability);
        }
    }

    /// Its repair buff as the item now has it, changed elsewhere (a repair
    /// at the bench drew one).
    pub fn set_buff(&mut self, buff: Option<RepairBuff>) {
        if let Some(w) = self.wear.as_deref_mut() {
            w.buff = buff;
        }
    }

    /// The host's word each frame: the raid's clock, the round to fire next
    /// and the magazine's chance (none with no magazine in).
    pub fn set_wear_inputs(&mut self, now: f64, round: RoundFigures, magazine_chance: Option<f32>) {
        if let Some(w) = self.wear.as_deref_mut() {
            (w.now, w.round, w.magazine_chance) = (now, round, magazine_chance);
        }
    }

    /// The raid's clock alone, each frame before the weapon's logic runs
    /// (`GameHelper.PastTime`): the heat cools by it and a shot's time is
    /// kept on it.
    pub fn set_wear_clock(&mut self, now: f64) {
        if let Some(w) = self.wear.as_deref_mut() {
            w.now = now;
        }
    }

    /// The skills and the arms, and the switches.
    pub fn set_wear_skills(&mut self, skills: Skills, arms_broken: u32, switches: WearSwitches) {
        if let Some(w) = self.wear.as_deref_mut() {
            (w.skills, w.arms_broken, w.switches) = (skills, arms_broken, switches);
        }
    }

    /// The malfunction the weapon has, none at all without wear.
    pub fn malfunction(&self) -> Malf {
        self.wear.as_deref().map_or(Malf::None, |w| w.state.malf)
    }

    /// The heat now, 0 without wear.
    pub fn heat(&self) -> f32 {
        self.wear.as_deref().map_or(0.0, Wear::heat)
    }

    /// The spread's factor from the heat (`MaxCOIIncreaseMult`), by the last
    /// shot's (`FirearmController.Shot` reads it after the shot's own heat
    /// is added); 1 with overheating off.
    pub fn heat_spread(&self) -> f32 {
        self.wear.as_deref().filter(|w| w.switches.overheating).map_or(1.0, |w| malfunction::spread_factor(w.state.last_shot_heat, &w.settings))
    }

    /// The durability now, for the barrel's deviation (100 without wear).
    pub fn durability(&self) -> f32 {
        self.wear.as_deref().map_or(100.0, |w| w.durability)
    }

    /// The animator's `Malfunction` and `MalfType` (`FirearmsAnimator.Malfunction`).
    pub(crate) fn malfunction_ints(&mut self, malfunction: i32, kind: i32) {
        self.animator.set("Malfunction", malfunction as f32);
        self.animator.set("MalfType", kind as f32);
    }

    /// The heat now set to `heat`, as if the last shot had left it so this
    /// moment: the app's debug panel's (not the game's), to try the
    /// overheating out. Within the settings' bounds.
    pub fn set_heat(&mut self, heat: f32) {
        if let Some(w) = self.wear.as_deref_mut() {
            w.state.last_shot_heat = heat.clamp(w.settings.min_overheat, w.settings.max_overheat);
            w.state.last_shot_time = w.now;
        }
    }

    /// The malfunction taken off at once, the weapon as one with none is
    /// set up (`set_wear`): the app's debug panel's (not the game's, whose
    /// malfunctions are only put right by hand).
    pub fn clear_malfunction(&mut self) {
        let Some(w) = self.wear.as_deref_mut() else { return };
        if w.state.malf == Malf::None {
            return;
        }
        w.state = State { malf: Malf::None, source: Default::default(), known: false, type_known: false, ..w.state };
        w.force_next = None;
        self.malfunction_ints(0, 0);
        self.animator.set_layer_weight("Malfunction", 0.0);
    }

    /// The next shot's malfunction, given outright (`Wear::force_next`):
    /// only a kind the weapon allows (`WeaponFigures::allows`), as the
    /// game's own roll gives no other (`GetSpecificMalfunctionVariants`). A
    /// weapon's animator has no state for a kind it does not allow, and the
    /// hands would wait for that state's mark for good (the UZI, the PPSh,
    /// the M60 and the AA-12, which allow none). Whether it was given.
    pub fn force_malfunction(&mut self, malf: Malf) -> bool {
        if !self.malfunction_allowed(malf) {
            return false;
        }
        let Some(w) = self.wear.as_deref_mut() else { return false };
        w.force_next = Some(malf);
        true
    }

    /// Whether this weapon can have that malfunction in the game: its
    /// template allows the kind, and its shot rolls at all. A weapon loaded
    /// by its barrels or its cylinder never does, whatever its template
    /// says (`FireOnlyBarrelFireOperation` and `FireCylinderMagOperation`
    /// prepare their shots without the roll): the MP-18 and the MP-43s
    /// allow a misfire that never comes.
    pub fn malfunction_allowed(&self, malf: Malf) -> bool {
        let rolls = self.settings.barrels == 0 && self.settings.cylinder.is_none() && !self.settings.rocket_launcher && !self.settings.one_off;
        rolls && malf != Malf::None && self.wear.as_deref().is_some_and(|w| w.figures.allows(malf))
    }

    /// The automatic fire's rate factor by the heat, to apply (`AutomaticFireOperation.Update`):
    /// some where drawn and not yet the burst's.
    pub(crate) fn heat_rate(&self) -> Option<f32> {
        self.wear.as_deref().map(|w| w.state.firerate_mult).filter(|mult| *mult > 0.0)
    }
}
