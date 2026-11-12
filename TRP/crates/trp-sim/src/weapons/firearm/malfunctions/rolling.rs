//! Rolling a malfunction on a shot and switching the firearm into it.

use super::*;

impl Firearm {
    /// The roll for the shot about to be made (`ProcessInventoryOperationsForSingleshotWithMalf`):
    /// none without wear or with malfunctions off. A malfunction is kept on
    /// the state, known to the shooter at once.
    pub(crate) fn roll_shot(&mut self) -> Malf {
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
                let protection = RepairBuff::factor(w.buff.as_ref(), BuffKind::MalfunctionProtections);
                let chances = malfunction::chances(&figures, &w.settings, w.durability, Some(&w.round), w.magazine_chance, heat, &w.skills, w.state.reduce_flags, protection);
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
    /// as the switches allow; a repair buff gone where the wear took the
    /// durability under its threshold (`TryDisableComponent`, asked only of
    /// a weapon that had durability to wear); the item is told
    /// (`Event::Worn`).
    pub(crate) fn shot_worn(&mut self) {
        let Some(w) = self.wear.as_deref_mut() else { return };
        let figures = w.figures();
        let round = w.round;
        let skills = if w.switches.durability { w.skills } else { Skills { maintenance: 0.0, ..w.skills } };
        let (mut durability, mut most) = (w.durability, w.max_durability);
        malfunction::on_shot(&mut w.state, w.now, &figures, &w.settings, &round, &skills, &mut durability, &mut most, &mut w.engine);
        if w.switches.durability {
            if w.durability > 0.0 {
                w.buff = w.buff.filter(|buff| buff.holds(durability));
            }
            (w.durability, w.max_durability) = (durability, most);
        }
        self.events.push(Event::Worn { durability: w.durability, max_durability: w.max_durability });
    }

    /// The round taken for a shot that malfunctioned (`ProcessInventoryOperationsForSingleShot`):
    /// the chamber's always leaves; the magazine's top goes up only with no
    /// malfunction (the plain `take_round`), or out of the weapon with a
    /// failure to feed, where the chamber would take it (`IsAmmoCompatible`).
    pub(crate) fn take_round_malfunctioned(&mut self, malf: Malf) {
        if malf == Malf::None {
            return self.take_round();
        }
        if self.settings.no_chamber {
            if let Some(rounds) = self.magazine.as_mut() {
                *rounds = rounds.saturating_sub(1);
            }
        }
        self.chamber = false;
        if malf == Malf::Feed && self.ammo_compatible() {
            if let Some(rounds) = self.magazine.as_mut() {
                *rounds = rounds.saturating_sub(1);
            }
        }
    }

    /// Into the malfunction (`SwitchToMalfunction`): the trigger let go,
    /// the animator told, blind fire ended; an automatic weapon plays the
    /// malfunction's state outright and its run is over.
    pub(crate) fn switch_to_malfunction(&mut self, malf: Malf, automatic: bool) {
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
        self.operation = Operation::Malfunction { malf, hide: None };
    }

    /// A misfire at the trigger of a single shot (`FireOperation.PrepareShot`):
    /// no shot; the trigger as it is, the animator's own transition taking it
    /// to its state.
    pub(crate) fn misfire_single(&mut self) {
        self.take_round_malfunctioned(Malf::Misfire);
        self.animator.set_flag("Fire", self.trigger);
        self.animator.set_flag("MisfireSlideUnknown", false);
        self.malfunction_ints(Malf::Misfire.number(), Malf::Misfire.number());
        self.ends_blind_fire();
        self.operation = Operation::Malfunction { malf: Malf::Misfire, hide: None };
    }

    /// The malfunction's state reached its mark (`StartMalfunction.OnMalfunctionOffEvent`):
    /// the chamber's count, the malfunction's layer on, its kind held, the
    /// weapon at rest with it.
    pub(crate) fn malfunction_held(&mut self) {
        let Operation::Malfunction { hide, .. } = self.operation else { return };
        self.animator_rounds();
        self.animator.set_layer_weight("Malfunction", 1.0);
        let kind = self.malfunction().number();
        self.malfunction_ints(-1, kind);
        self.switch_to_idle(hide);
    }

    /// A malfunction's operation over (`MalfunctionOperation.SwitchToIdle`):
    /// the trigger let go, and the put-away asked for meanwhile made, else
    /// the weapon at rest.
    pub(super) fn switch_to_idle(&mut self, hide: Option<bool>) {
        self.trigger_to(false);
        self.animator.set_flag("Fire", false);
        self.to_idle();
        // Its plan goes straight to the put-away (`MalfunctionOperation.HideWeapon`),
        // keeping no aim.
        if let Some(fast) = hide {
            self.hide_from(fast, false);
        }
    }
}
