//! The weapon's malfunctions, wear and heat in the hands
//! (`docs/spec/malfunctions.md`): the roll at each shot, the malfunction's
//! own state (`StartMalfunction`), the look at it
//! (`ExamineMalfunctionTypeOperation`), its repair (`RepairMalfunction`),
//! the trigger and the keys while it lasts, the heat's stray shot and the
//! automatic fire's rate by the heat. The rules themselves are
//! `crate::malfunction`'s; the host gives the weapon its wear and its state
//! as it is drawn (`set_wear`) and takes them back (`wear`).

use super::{Event, Firearm, Operation};
use crate::malfunction::{self, Malf, MalfRandom, RoundFigures, Settings, Skills, State, WeaponFigures};
use crate::unity_random::UnityRandom;

/// Which of the three systems are on (the options' `DURABILITY`,
/// `MALFUNCTIONS`, `OVERHEATING`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WearSwitches {
    pub durability: bool,
    pub malfunctions: bool,
    pub overheating: bool,
}

impl Default for WearSwitches {
    fn default() -> Self {
        Self { durability: true, malfunctions: true, overheating: true }
    }
}

/// What the weapon's logic keeps of its wear while in hand.
#[derive(Clone, Debug)]
pub struct Wear {
    pub figures: WeaponFigures,
    pub settings: Settings,
    /// The weapon's durability and its greatest durability now
    /// (`Repairable`), and its malfunction state.
    pub durability: f32,
    pub max_durability: f32,
    pub state: State,
    /// The roll's own generator (`MalfunctionRandom`) and Unity's global one
    /// for the other draws (the heat's wear, the fire rate's factor, the
    /// stray shot).
    pub random: MalfRandom,
    pub engine: UnityRandom,
    /// The round to fire next and the magazine's chance (none with no
    /// magazine in), as the host's inventory has them.
    pub round: RoundFigures,
    pub magazine_chance: Option<f32>,
    pub skills: Skills,
    /// How many arms are broken (`LeftArmDamaged`, `RightArmDamaged`).
    pub arms_broken: u32,
    /// The raid's clock (`GameHelper.PastTime`), as the host last said.
    pub now: f64,
    pub switches: WearSwitches,
    /// The next shot's malfunction, given outright (the game's console's,
    /// `EMalfunctionSource.ConsoleCommand`): for a test, or a recording's
    /// own malfunction set on its shot.
    pub force_next: Option<Malf>,
}

impl Wear {
    pub fn new(figures: WeaponFigures, settings: Settings, durability: f32, max_durability: f32, state: State, seed: i32) -> Self {
        Self {
            figures,
            settings,
            durability,
            max_durability,
            state,
            random: MalfRandom::new(seed),
            engine: UnityRandom::new(seed.wrapping_mul(31).wrapping_add(7)),
            round: RoundFigures::default(),
            magazine_chance: None,
            skills: Skills::default(),
            arms_broken: 0,
            now: 0.0,
            switches: WearSwitches::default(),
            force_next: None,
        }
    }

    /// The figures with the switches applied: heat only where overheating
    /// is on.
    fn figures(&self) -> WeaponFigures {
        WeaponFigures { allow_overheat: self.figures.allow_overheat && self.switches.overheating, ..self.figures }
    }

    /// The heat now.
    pub fn heat(&self) -> f32 {
        malfunction::current_heat(&self.state, self.now, &self.figures(), &self.settings)
    }
}

/// The hands' state of each malfunction, played outright by an automatic
/// weapon's (`SwitchToMalfunction`).
fn state_name(malf: Malf) -> Option<&'static str> {
    match malf {
        Malf::None => None,
        Malf::Misfire => Some("Hands.MISFIRE"),
        Malf::Jam => Some("Hands.JAM"),
        Malf::HardSlide => Some("Hands.HARD_SLIDE"),
        Malf::SoftSlide => Some("Hands.SOFT_SLIDE"),
        Malf::Feed => Some("Hands.FEED"),
    }
}

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
    pub(super) fn malfunction_ints(&mut self, malfunction: i32, kind: i32) {
        self.animator.set("Malfunction", malfunction as f32);
        self.animator.set("MalfType", kind as f32);
    }

    /// The roll for the shot about to be made (`ProcessInventoryOperationsForSingleshotWithMalf`):
    /// none without wear or with malfunctions off. A malfunction is kept on
    /// the state, known to the shooter at once.
    pub(super) fn roll_shot(&mut self) -> Malf {
        let rounds_in = self.magazine.unwrap_or(0) > 0;
        let magazine_in = self.magazine.is_some();
        let Some(w) = self.wear.as_deref_mut() else { return Malf::None };
        if !w.switches.malfunctions {
            return Malf::None;
        }
        let figures = w.figures();
        let (malf, source) = match w.force_next.take() {
            Some(malf) => (malf, malfunction::Source::Durability),
            None => {
                let heat = malfunction::current_heat(&w.state, w.now, &figures, &w.settings);
                let chances = malfunction::chances(&figures, &w.settings, w.durability, Some(&w.round), w.magazine_chance, heat, &w.skills, w.state.reduce_flags);
                malfunction::roll(&mut w.random, &figures, &w.settings, &chances, &w.state, heat, rounds_in, magazine_in)
            }
        };
        if malf != Malf::None {
            w.state.malf = malf;
            w.state.source = source;
            w.state.known = true;
            // Elite at Troubleshooting, its kind is known at once
            // (`InventoryController.ExamineMalfunction`, `TroubleFixingExamineMalfElite`).
            w.state.type_known = w.skills.troubleshooting_elite;
            if w.skills.troubleshooting_elite {
                w.state.reduce_flags |= match source {
                    malfunction::Source::Durability => 0x8,
                    malfunction::Source::Ammo => 1,
                    malfunction::Source::Magazine => 2,
                    malfunction::Source::Overheat => 4,
                };
            }
            self.events.push(Event::Malfunctioned(malf));
        }
        malf
    }

    /// A shot that fires (`Weapon.OnShot`): the heat added, the wear taken,
    /// as the switches allow; the item is told (`Event::Worn`).
    pub(super) fn shot_worn(&mut self) {
        let Some(w) = self.wear.as_deref_mut() else { return };
        let figures = w.figures();
        let round = w.round;
        let skills = if w.switches.durability { w.skills } else { Skills { maintenance: 0.0, ..w.skills } };
        let (mut durability, mut most) = (w.durability, w.max_durability);
        malfunction::on_shot(&mut w.state, w.now, &figures, &w.settings, &round, &skills, &mut durability, &mut most, &mut w.engine);
        if w.switches.durability {
            (w.durability, w.max_durability) = (durability, most);
        }
        self.events.push(Event::Worn { durability: w.durability, max_durability: w.max_durability });
    }

    /// The round taken for a shot that malfunctioned (`ProcessInventoryOperationsForSingleShot`):
    /// the chamber's always leaves; the magazine's top goes up only with no
    /// malfunction (the plain `take_round`), or out of the weapon with a
    /// failure to feed.
    pub(super) fn take_round_malfunctioned(&mut self, malf: Malf) {
        if malf == Malf::None {
            return self.take_round();
        }
        if self.settings.no_chamber {
            if let Some(rounds) = self.magazine.as_mut() {
                *rounds = rounds.saturating_sub(1);
            }
        }
        self.chamber = false;
        if malf == Malf::Feed {
            if let Some(rounds) = self.magazine.as_mut() {
                *rounds = rounds.saturating_sub(1);
            }
        }
    }

    /// Into the malfunction (`SwitchToMalfunction`): the trigger let go,
    /// the animator told, blind fire ended; an automatic weapon plays the
    /// malfunction's state outright and its run is over.
    pub(super) fn switch_to_malfunction(&mut self, malf: Malf, automatic: bool) {
        self.trigger_to(false);
        self.animator.set_flag("Fire", false);
        self.animator.set_flag("MisfireSlideUnknown", false);
        self.malfunction_ints(malf.number(), malf.number());
        if automatic {
            self.events.push(Event::AutoFire(false));
            if let Some(state) = state_name(malf) {
                self.animator.play(state, 0.0);
            }
        }
        self.ends_blind_fire();
        self.round_out = false;
        self.operation = Operation::Malfunction { malf };
    }

    /// A misfire at the trigger of a single shot (`FireOperation.PrepareShot`):
    /// no shot; the trigger as it is, the animator's own transition taking it
    /// to its state.
    pub(super) fn misfire_single(&mut self) {
        self.take_round_malfunctioned(Malf::Misfire);
        self.animator.set_flag("Fire", self.trigger);
        self.animator.set_flag("MisfireSlideUnknown", false);
        self.malfunction_ints(Malf::Misfire.number(), Malf::Misfire.number());
        self.ends_blind_fire();
        self.operation = Operation::Malfunction { malf: Malf::Misfire };
    }

    /// The malfunction's state reached its mark (`StartMalfunction.OnMalfunctionOffEvent`):
    /// the chamber's count, the malfunction's layer on, its kind held, the
    /// weapon at rest with it.
    pub(super) fn malfunction_held(&mut self) {
        self.animator_rounds();
        self.animator.set_layer_weight("Malfunction", 1.0);
        let kind = self.malfunction().number();
        self.malfunction_ints(-1, kind);
        self.to_idle();
    }

    /// The trigger with a malfunction (`Idling.SetTriggerPressed`): no shot;
    /// the animator's dry fire, and the reminder. Whether it was taken here.
    pub(super) fn malfunction_trigger(&mut self, pressed: bool) -> bool {
        if self.malfunction() == Malf::None {
            return false;
        }
        self.animator.set_flag("Fire", pressed);
        self.trigger_to(pressed);
        if pressed {
            self.animator.set_flag("MisfireSlideUnknown", false);
            self.remind();
            self.events.push(Event::DryFire);
        }
        true
    }

    /// The reminder (`ExamineMalfunction` when known): the battle screen's
    /// glow and words.
    pub(super) fn remind(&mut self) {
        let Some(w) = self.wear.as_deref() else { return };
        self.events.push(Event::MalfunctionReminder { malf: w.state.malf, type_known: w.state.type_known });
    }

    /// The look at a malfunction (`ExamineMalfunctionTypeOperation`), from
    /// the look key at rest: the sights down, the trigger off, blind fire
    /// ended; the animator's look (`Look`: the AKM's `JAM_LOOK`). Whether
    /// it began.
    pub(super) fn examine_malfunction(&mut self) -> bool {
        if self.malfunction() == Malf::None || !self.is_idling() || self.blind_fire {
            return false;
        }
        self.events.push(Event::LeaveMount { at_once: false });
        self.aiming = false;
        self.animator.set_flag("Fire", false);
        self.ends_blind_fire();
        (self.hands_busy, self.reload_flag) = (true, true);
        self.animator.trigger("Look");
        self.operation = Operation::ExamineMalfunction;
        true
    }

    /// The look's mark: the malfunction's kind is known (`ExamineMalfunctionType`).
    pub(super) fn malfunction_examined(&mut self) {
        if let Some(w) = self.wear.as_deref_mut() {
            let first = !w.state.type_known;
            w.state.type_known = true;
            self.events.push(Event::MalfunctionExamined { malf: w.state.malf, first });
        }
        self.to_idle();
    }

    /// The check of the chamber with a malfunction (`Idling.CheckChamber`):
    /// the sights down; its repair where its kind is known, else the
    /// reminder. Whether it was taken here.
    pub(super) fn malfunction_check_chamber(&mut self) -> bool {
        let malf = self.malfunction();
        if malf == Malf::None {
            return false;
        }
        if !self.is_idling() || self.trigger || self.blind_fire {
            return true;
        }
        self.aiming = false;
        if !self.wear.as_deref().is_some_and(|w| w.state.type_known) {
            self.animator.set_flag("MisfireSlideUnknown", false);
            self.remind();
            return true;
        }
        // `RepairMalfunction.Start`: the state put right at once, the
        // hands' repair played at the repair's speed.
        self.ends_blind_fire();
        let speed = self.wear.as_deref().map_or(1.0, |w| malfunction::repair_speed(&w.settings, &w.skills, w.arms_broken, malf));
        self.animator.set_flag("MalfunctionRepair", true);
        self.animator.set_layer_weight("Malfunction", 0.0);
        self.malfunction_ints(malf.number(), malf.number());
        self.animator.set("SpeedFix", speed);
        (self.hands_busy, self.reload_flag) = (true, true);
        if let Some(w) = self.wear.as_deref_mut() {
            w.state.malf = Malf::None;
            w.state.known = false;
            w.state.type_known = false;
        }
        self.events.push(Event::MalfunctionRepairBegun(malf));
        self.operation = Operation::RepairMalfunction { malf, ejected: false };
        true
    }

    /// The repair's marks (`RepairMalfunction`): the round shown spent, the
    /// one thrown out (a failure to feed's onto the ground as a thing), the
    /// next chambered, the end. Whether the mark was the repair's.
    pub(super) fn repair_mark(&mut self, name: &str) -> bool {
        let Operation::RepairMalfunction { malf, ejected } = self.operation else { return false };
        match name {
            "DelAmmoChamber" => self.animator.set("AmmoInChamber", 0.0),
            "ShellEject" if !ejected => {
                self.operation = Operation::RepairMalfunction { malf, ejected: true };
                self.events.push(match malf {
                    Malf::Jam => Event::ShellEject,
                    Malf::Feed => Event::FedRoundThrown,
                    _ => Event::MisfiredRoundThrown,
                });
            }
            "AddAmmoInChamber" => {
                if !self.chamber && self.magazine.unwrap_or(0) > 0 && !self.settings.no_chamber {
                    self.take_round();
                }
                self.animator_rounds();
            }
            "MalfunctionOff" => {
                self.animator.set_flag("MalfunctionRepair", false);
                self.malfunction_ints(0, 0);
                self.animator.set_flag("MisfireSlideUnknown", false);
                self.events.push(Event::MalfunctionRepaired(malf));
                self.to_idle();
            }
            _ => return false,
        }
        true
    }

    /// A misfire put right without its repair (`FirearmOperation.method_2`:
    /// a reload's or an inserted magazine's chambering, a draw, the chamber
    /// unloaded): the state cleared, the animator put back.
    pub(super) fn clear_misfire(&mut self) {
        let Some(w) = self.wear.as_deref_mut() else { return };
        if w.state.malf != Malf::Misfire {
            return;
        }
        w.state.malf = Malf::None;
        w.state.known = false;
        w.state.type_known = false;
        self.malfunction_ints(0, 0);
        self.animator.set_flag("MalfunctionRepair", false);
        self.animator.set_layer_weight("Malfunction", 0.0);
        self.events.push(Event::MalfunctionCleared(Malf::Misfire));
    }

    /// The heat's stray shot (`Idling.ProcessAutoshot`), each frame at rest:
    /// cancelled with a malfunction; when its time has come, one single shot.
    pub(super) fn autoshot(&mut self) {
        let ready = matches!(self.operation, Operation::Idle) && self.loaded();
        let Some(w) = self.wear.as_deref_mut() else { return };
        let Some(at) = w.state.autoshot_at else { return };
        if w.state.malf != Malf::None {
            w.state.autoshot_at = None;
            return;
        }
        if w.now < at || !ready {
            return;
        }
        w.state.autoshot_at = None;
        self.trigger_to(true);
        self.start_single();
        self.events.push(Event::Autoshot);
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

    /// The next shot's malfunction, given outright (`Wear::force_next`).
    pub fn force_malfunction(&mut self, malf: Malf) {
        if let Some(w) = self.wear.as_deref_mut() {
            w.force_next = Some(malf);
        }
    }

    /// The automatic fire's rate factor by the heat, to apply (`AutomaticFireOperation.Update`):
    /// some where drawn and not yet the burst's.
    pub(super) fn heat_rate(&self) -> Option<f32> {
        self.wear.as_deref().map(|w| w.state.firerate_mult).filter(|mult| *mult > 0.0)
    }
}
