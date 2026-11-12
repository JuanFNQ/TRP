//! The magazine picker: with R held, the wheel chooses what the weapon is reloaded with.
//!
//! The magazine picker (`docs/spec/firing.md`, "The magazine picker";
//! `InventoryScreenQuickAccessPanel.TryShowAmmoSelector`, `AmmoSelector`,
//! `GamePlayerOwner.SwitchMagazine`): with R held, a notch of the wheel
//! shows beside the weapon's quick slot what the weapon can be reloaded
//! with (`trp_sim::inventory::picker_choices`), and a cell to cancel last;
//! each notch after moves the choice round, and the reload's key reloads
//! with the one chosen. Drawn on the battle screen's quick bar
//! (`battle_view`); under the options' `WEAPON HANDLING` and `INVENTORY
//! SYSTEM`.

use bevy::prelude::*;
use trp_sim::items::inventory::Id;

use crate::player::controller::Controller;
use crate::screens::inventory_view::{Carried, HandsHeld};
use crate::platform::keys::Bindings;

/// What a choice asks of the hands (`SwitchMagazine`), taken up by the
/// controller's step on the frame it is made.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Picked {
    /// A magazine to reload with (`ReloadMag`), as `Controller::
    /// next_magazine` has one: its number, its rounds, its clips' variant,
    /// whether it comes out, and how many of its rounds the chamber refuses.
    Magazine((Id, u32, i32, bool, u32)),
    /// A stack of rounds to load from alone (`ReloadWithAmmo`,
    /// `ReloadCylinderMagazine` the plain way, `ReloadBarrels`, each with
    /// an `AmmoPack` of it): its number and how many it holds.
    Rounds(Id, u32),
}

/// The picker as shown: the weapon it was shown for, what it offers in its
/// order, and the choice (its last place, past them, the cancel).
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    pub weapon: Id,
    pub choices: Vec<Id>,
    pub selected: usize,
}

#[derive(Resource, Default)]
pub struct Picker {
    pub shown: Option<Shown>,
}

impl Shown {
    /// `NextMagazine` (the wheel up): the choice one back, round past the
    /// first to the cancel (`AmmoSelector.SelectNext`).
    fn next(&mut self) {
        let places = self.choices.len() + 1;
        self.selected = (self.selected + places - 1) % places;
    }

    /// `PreviousMagazine` (the wheel down): one on (`SelectPrev`).
    fn previous(&mut self) {
        let places = self.choices.len() + 1;
        self.selected = (self.selected + 1) % places;
    }

    /// The thing chosen; none for the cancel (`GetSelectedMagazine`).
    fn chosen(&self) -> Option<Id> {
        self.choices.get(self.selected).copied()
    }
}

/// Whether the hands would start a reload now (`GamePlayerOwner.
/// CanSwitchMagazine`, `FirearmController.CanStartReload`): a firearm in
/// them, the character alive and no screen over the game; not in blind
/// fire, nor with the left hand's gesture playing, the fitted magazine
/// known and no failure to feed; the hands idling or firing (`Idling`, not
/// a look or a check, and `FireOperationBase`). `PROVISIONAL`: a put-away
/// asked during a shot (`_plannedAction`) is not told apart; the
/// warning a magazine not known gives and the reminder of a failure to
/// feed (the game's, on every notch) are not built here.
fn can_switch(controller: &Controller, hands: Option<&crate::weapons::weapon::HandsRig>, dead: bool) -> bool {
    let Some(firearm) = hands.and_then(|hands| hands.firearm.as_ref()) else { return false };
    !dead
        && !controller.inventory_open
        && !controller.edit_build
        && !firearm.blind_fire()
        && !controller.movement.interaction_playing()
        && !controller.magazine_unknown
        && firearm.malfunction() != trp_sim::weapons::malfunction::Malf::Feed
        && firearm.can_interact()
}

/// The picker's keys, heard before the controller's step: a notch with R
/// held shows it (the bar's own input node, before the hands hear it:
/// `InventoryScreenQuickAccessPanel.TranslateCommand`), the choice still
/// the first; each notch after moves the choice (`AmmoSelector.
/// TranslateCommand`); the reload's key takes the choice and shuts it, and
/// the hands do not hear that key (`ETranslateResult.Block`). The bar going
/// (the inventory opening, a death) shuts it with nothing chosen
/// (`InventoryScreenQuickAccessPanel.Hide`: `ammoSelector.Close`).
#[allow(clippy::too_many_arguments)]
pub fn hear(
    bindings: Res<Bindings>,
    options: Res<crate::Options>,
    features: Res<crate::screens::options_view::Features>,
    mut controller: ResMut<Controller>,
    carried: Res<Carried>,
    mut held: ResMut<HandsHeld>,
    hands: Option<Res<crate::weapons::weapon::HandsRig>>,
    dying: Res<crate::player::death::Dying>,
    mut picker: ResMut<Picker>,
    mut icons: ResMut<crate::screens::icons::Icons>,
) {
    if !features.weapon_handling || !features.inventory {
        return;
    }
    let dead = dying.since().is_some();
    if picker.shown.is_some() && (dead || controller.inventory_open || controller.edit_build) {
        picker.shown = None;
        return;
    }
    if !options.keys_live() {
        return;
    }
    let (next, previous) = (bindings.fired("NextMagazine"), bindings.fired("PreviousMagazine"));
    match picker.shown.as_mut() {
        Some(shown) => {
            if next {
                shown.next();
            }
            if previous {
                shown.previous();
            }
            if bindings.fired("ReloadWeapon") {
                controller.picker_took_reload = true;
                let (weapon, chosen) = (shown.weapon, shown.chosen());
                picker.shown = None;
                if let Some(item) = chosen {
                    switch_magazine(&mut controller, &carried, &mut held, hands.as_deref(), dead, weapon, item);
                }
            }
        }
        None if next || previous => {
            let able = can_switch(&controller, hands.as_deref(), dead);
            let choices = held.item.map(|weapon| trp_sim::items::inventory::picker_choices(&carried.equipment.item, weapon)).unwrap_or_default();
            debug!("the magazine picker's key: the hands {} a reload, {} to offer", if able { "would start" } else { "would not start" }, choices.len());
            let Some(weapon) = held.item.filter(|_| able) else { return };
            if !choices.is_empty() {
                info!("the magazine picker shown: {} to choose from", choices.len());
                // Each thing's picture, where it has none yet (the game's
                // views are made with theirs).
                for item in choices.iter().filter_map(|id| carried.find(*id)) {
                    let key = crate::screens::icons::picture_key(item);
                    if !carried.pictures.contains_key(&key) {
                        icons.want(key, item.cells());
                    }
                }
                picker.shown = Some(Shown { weapon, choices, selected: 0 });
            }
        }
        None => {}
    }
}

/// The reload with the thing chosen (`GamePlayerOwner.SwitchMagazine`):
/// nothing where it is no longer the character's, the hands would not start
/// a reload, or the weapon is no longer in them. A magazine, for a weapon
/// whose magazine comes out (or one of a magazine of its own with none
/// fitted), that its magazine slot takes: the plain reload with it
/// (`ReloadMag`), the old one put where a reload puts it. Rounds: the plain
/// reload from that stack alone (a tube's `ReloadWithAmmo`, a cylinder's
/// `ReloadCylinderMagazine` not the quick way, barrels' `ReloadBarrels`).
fn switch_magazine(controller: &mut Controller, carried: &Carried, held: &mut HandsHeld, hands: Option<&crate::weapons::weapon::HandsRig>, dead: bool, weapon: Id, item: Id) {
    let equipment = &carried.equipment.item;
    let (Some(gun), Some(thing)) = (equipment.find(weapon), equipment.find(item)) else { return };
    if !can_switch(controller, hands, dead) || held.item != Some(weapon) {
        return;
    }
    if trp_sim::items::inventory::is(thing, "Magazine") {
        let mode = trp_sim::items::inventory::reload_mode(gun);
        let fitted = gun.in_slot(trp_sim::items::inventory::MAGAZINE_SLOT).is_some();
        let comes_out = mode == "ExternalMagazine" || mode == trp_sim::items::inventory::MIXED_RELOAD || (mode == "InternalMagazine" && !fitted);
        let takes = gun.slot_index(trp_sim::items::inventory::MAGAZINE_SLOT).and_then(|slot| gun.kind.slots.get(slot)).is_some_and(|slot| thing.kind.among(&slot.takes));
        if comes_out && takes {
            let unfit = trp_sim::items::inventory::unfit_rounds(gun, thing);
            info!("the magazine picker: reload with {} ({} rounds)", thing.kind.template, trp_sim::items::inventory::magazine_rounds(thing));
            controller.picked = Some(Picked::Magazine((item, trp_sim::items::inventory::magazine_rounds(thing), thing.kind.magazine_animation, thing.kind.magazine_external, unfit)));
        }
    } else if trp_sim::items::inventory::is(thing, "Ammo") {
        info!("the magazine picker: load from {} ({} rounds)", thing.kind.template, thing.count);
        held.pack = Some(item);
        controller.picked = Some(Picked::Rounds(item, thing.count));
    }
}

#[cfg(test)]
mod tests {
    use super::Shown;

    /// The wheel up goes back and round to the cancel, the wheel down on and
    /// round to the first; the cancel chooses nothing.
    #[test]
    fn the_choice_goes_round_through_the_cancel() {
        let mut shown = Shown { weapon: trp_sim::items::inventory::Id::default(), choices: vec![trp_sim::items::inventory::Id::default(); 2], selected: 0 };
        shown.next();
        assert_eq!(shown.selected, 2);
        assert!(shown.chosen().is_none());
        shown.previous();
        assert_eq!(shown.selected, 0);
        shown.previous();
        shown.previous();
        assert_eq!(shown.selected, 2);
        shown.previous();
        assert_eq!((shown.selected, shown.chosen().is_some()), (0, true));
    }
}
