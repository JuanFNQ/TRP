//! The hands' moves followed and the goggles, launcher, unload and chamber keys.

use super::*;

/// What the equipment makes of the character once something has moved.
///
/// The weapon in hand is one of those in the slots a weapon is held from
/// (the sling, the back, the holster), if it is of the kind the app has
/// built (inventory spec, "The weapon in hand"):
///
/// - A character made with one has the first of them in hand from the
///   start (`Player.SetFirstAvailableItem`).
/// - A slot's key takes what hangs there in hand (`Player.SetSlotItem`):
///   what is in hand is put away first, and the key of the thing already
///   in hand does nothing. The keys are the install's own (`keys`: the
///   sling's is `2`, the back's `3`, the holster's `1`). So does a
///   quick-use key bound to a weapon.
/// - A weapon put into the slot the hands last drew from is taken in hand
///   at once; into any other it hangs until its key is pressed.
/// - The weapon in hand moved out of its slot, to another slot or away,
///   is put away, and the hands take the next weapon in the slots' order
///   or are empty (`SetControllerInsteadRemovedOne`).
///
/// The app builds one rifle, with one logic for it: a second rifle of the
/// same kind would be drawn with the first one's rounds. PROVISIONAL.
///
/// Being drawn and being put away are the weapon's own operations
/// (`Firearm::draw`, `hide`), which its animator plays out by its own
/// states from whatever it was doing: from rest, from the hold of an open
/// inventory, after a shot or a reload in hand. The weapon says when it
/// is out of the hands, and only then is the next thing made.
///
/// Empty hands are a rig of their own (inventory spec, "Empty hands"),
/// with a controller and an animator of their own, which stands by while
/// the weapon is in hand and is changed over with it (`Stowed`): they
/// appear when the weapon has gone, and go at once when the weapon is
/// drawn, as the game's do. If the install's empty hands could not be
/// read, the weapon and the arms are hidden instead.
///
/// The inventory opening and shutting is told to whatever is in the
/// hands at that moment, and what comes of it is the business of the
/// operation it then has in hand (`Firearm::set_inventory`): a weapon at
/// rest goes down and comes back by its animator's own states; one being
/// drawn does not hear of it. What has just appeared in the hands is told
/// of the screen afresh, as the game's owner tells it when the hands have
/// changed (`GamePlayerOwner.SetHandsController`): so a weapon drawn
/// under an open screen goes down as soon as it is up.
///
/// The carried weight is the equipment's sum, which the controller goes
/// by from the next frame (inventory spec, "Weight").
/// What follows a switch flipped on what is worn on the head (inventory
/// spec, "A switch on the head"; `PlayerToggleOperation.Dispose`): its
/// gesture asked of the hands where the movement state lets them act
/// (`trp_sim::visor::switch_gesture`: refused while the thing in hand is
/// not at rest, so never under the open screen nor in a reload).
pub(super) fn switch_followed(controller: &mut Controller, carried: &Carried, part: Id, on: bool) {
    let Some(item) = carried.equipment.worn("Headwear").and_then(|head| head.find(part)) else { return };
    let face_shield = item.kind.face_shield;
    // (Its sound is heard with the rest of what is worn: `step_sound`.)
    info!("a switch on the head {}", if on { "on" } else { "off" });
    match trp_sim::effects::visor::switch_gesture(&mut controller.movement, face_shield, on) {
        Some(plays) => info!("its gesture {:?}: {}", trp_sim::Interaction::of_switch(face_shield, on), if plays { "plays" } else { "refused (the thing in hand not at rest, or a sprint)" }),
        None => info!("its gesture not asked: the movement state lets the hands do nothing"),
    }
}

/// The goggles' key (`Player.ToggleGoggles`; inventory spec, "A switch on
/// the head"), heard before the controller's step: the switch of what is
/// worn on the head flipped at once, its sound, and its gesture asked of
/// the hands for this frame's animator, as the game's input comes before
/// its animator (`courses8`'s visors: the gesture's layer put to rest on
/// the key's frame and its state entered on the next). Not under a screen,
/// nor with the options' `INVENTORY SYSTEM` off, as the quick keys.
pub fn goggles_key(bindings: Res<Bindings>, options: Res<Options>, features: Res<crate::screens::options_view::Features>, mut controller: ResMut<Controller>, mut carried: ResMut<Carried>) {
    if !features.inventory || controller.edit_build || controller.inventory_open || !options.keys_live() || !bindings.fired("ToggleGoggles") {
        return;
    }
    if let Some((part, on)) = trp_sim::effects::visor::toggle_goggles(&mut carried.equipment) {
        switch_followed(&mut controller, &carried, part, on);
    }
}

/// The magazine's and the chamber's unload keys (`UnloadMagazine`,
/// `ChamberUnload`: Alt and R, Ctrl and R as the install binds them;
/// firing spec, "The magazine's and the chamber's keys"), heard before the
/// controller's step, as the game's input comes before its hands and
/// animator: the hands asked first (`Firearm::unload_magazine_key`,
/// `unload_chamber_key`), then what they leave to the inventory made at
/// once, the magazine, or the round of a chamber with no magazine in, moved
/// to the vest or the pockets by the moving's rules, and its hands' move
/// begun on the same frame (`pull_out_magazine`, `discharge`,
/// `unload_own_chamber`). Not under a screen, nor with the weapon's
/// handling or the inventory switched off.
/// The weapon's slot key pressed again with that weapon in hand
/// (`FirearmHandsInputTranslator.ChangeWeaponMode`; underbarrel-launcher
/// spec, "The switch"): the first or second primary weapon's key, the
/// holster's or the quick holster's, its slot's weapon the one in hand,
/// switches to the weapon's underbarrel launcher or back where its hands
/// take it (`Firearm::toggle_launcher`). The same press goes on to the
/// player's slot handling, which ends blind fire (and would put away a thing
/// in the left hand, none being built) before it finds the weapon already in
/// hand, as `settle` finds it and does nothing more. Heard before the
/// simulation's step, as the game's input comes before its hands.
pub fn launcher_keys(
    bindings: Res<Bindings>,
    options: Res<Options>,
    features: Res<crate::screens::options_view::Features>,
    mut controller: ResMut<Controller>,
    carried: Res<Carried>,
    held: Res<HandsHeld>,
    mut weapon: Option<ResMut<HandsRig>>,
) {
    if !features.weapon_handling || controller.edit_build || controller.inventory_open || !options.keys_live() || !controller.armed {
        return;
    }
    let quick = bindings.fired("QuickSecondaryWeapon").then_some("Holster");
    let Some(slot) = quick.or_else(|| SLOT_KEYS.iter().filter(|(_, slot)| *slot != "Scabbard").find(|(key, _)| bindings.fired(key)).map(|(_, slot)| *slot)) else { return };
    let Some(gun) = held.item.filter(|&gun| carried.equipment.worn(slot).is_some_and(|item| item.id == gun)) else { return };
    let Some(rig) = weapon.as_deref_mut() else { return };
    if controller.movement.blind_fire() != 0 {
        trp_sim::weapons::blind_fire::ended(&mut controller.movement, Some(&mut rig.hands));
    }
    if rig.firearm.as_mut().is_some_and(trp_sim::Firearm::toggle_launcher) {
        info!("the slot key again: the switch of {gun:?}'s underbarrel launcher begun");
    }
}

pub fn unload_keys(
    bindings: Res<Bindings>,
    options: Res<Options>,
    features: Res<crate::screens::options_view::Features>,
    controller: Res<Controller>,
    mut carried: ResMut<Carried>,
    held: Res<HandsHeld>,
    mut weapon: Option<ResMut<HandsRig>>,
) {
    let (magazine_key, chamber_key) = (bindings.fired("UnloadMagazine"), bindings.fired("ChamberUnload"));
    if !(magazine_key || chamber_key) || !features.inventory || !features.weapon_handling || controller.edit_build || controller.inventory_open || !options.keys_live() || !controller.armed {
        return;
    }
    let (Some(gun), Some(firearm)) = (held.item, weapon.as_deref_mut().and_then(|rig| rig.firearm.as_mut())) else { return };
    if carried.equipment.item.find(gun).is_none() {
        return;
    }
    // The hands' refusal is theirs now (`FirearmController.CanExecute`), not
    // the inventory's of the last frame.
    let rules = trp_sim::items::inventory::Rules { busy_weapon: None, chamber_held: None, ..carried.rules };
    // (With an underbarrel launcher in use the magazine's key finds nothing
    // to do: underbarrel-launcher spec, "While the launcher is in use".)
    if magazine_key && !firearm.in_launcher_mode() && firearm.unload_magazine_key() {
        let magazine = carried.find(gun).and_then(fitted_magazine);
        if let (Some(magazine), true) = (magazine, firearm.can_move_magazine()) {
            match trp_sim::items::inventory::unload_to_rig(&mut carried.equipment.item, magazine, &rules) {
                Ok(()) => {
                    let left = firearm.magazine.unwrap_or(0);
                    trp_sim::items::inventory::set_rounds_of(&mut carried.equipment.item, magazine, left);
                    firearm.pull_out_magazine();
                    info!("the magazine's unload key: the magazine to the rig, {left} rounds");
                }
                Err(refusal) => info!("the magazine's unload key: the magazine stays ({refusal:?})"),
            }
        }
    }
    // The chamber's key, only where the item holds what the logic does: the
    // item keeps a fired round, and the rounds fed since, until the weapon
    // is at rest and the inventory has caught up (`settle`), so a key on the
    // frame it comes to rest may not be taken (the app's). The chamber's
    // round with no magazine in is the inventory's to move.
    let caught_up = !firearm.single_chamber()
        || carried.find(gun).is_some_and(|weapon| {
            let rounds = weapon.in_slot(trp_sim::items::inventory::MAGAZINE_SLOT).map(trp_sim::items::inventory::magazine_rounds);
            trp_sim::items::inventory::chambered(weapon).is_some() == firearm.chamber && (!firearm.magazine_comes_out() || rounds == firearm.magazine)
        });
    // An underbarrel launcher in use has its round unloaded instead
    // (`UnloadLauncher`): the round to the vest or the pockets, the smallest
    // grid with room first, which the hands then play (`discharge_launcher`;
    // the move made at once, `PROVISIONAL` as the rifle's).
    let launcher = firearm.in_launcher_mode();
    let key = (chamber_key && (caught_up || launcher)).then(|| firearm.unload_chamber_key());
    if key == Some(trp_sim::ChamberKey::Launcher) {
        let round = carried.find(gun).and_then(trp_sim::items::inventory::launcher_part).and_then(trp_sim::items::inventory::chambered).map(|round| round.id);
        if let (Some(round), Some((true, _))) = (round, firearm.launcher_chamber()) {
            match trp_sim::items::inventory::unload_to_rig(&mut carried.equipment.item, round, &rules) {
                Ok(()) => {
                    firearm.discharge_launcher();
                    info!("the chamber's unload key: the launcher's round to the rig");
                }
                Err(refusal) => info!("the chamber's unload key: the launcher's round stays ({refusal:?})"),
            }
        }
    }
    if key == Some(trp_sim::ChamberKey::ToRig) {
        let single = firearm.single_chamber();
        let first = carried.find(gun).and_then(trp_sim::items::inventory::first_loaded_chamber);
        let takes = if single { firearm.chamber && firearm.can_discharge() } else { firearm.can_unload_own_chamber() };
        if let (Some((at, round)), true) = (first, takes) {
            let held_by_logic = single || firearm.in_chambers.get(at).is_some_and(|chamber| chamber.round);
            match (held_by_logic, trp_sim::items::inventory::unload_to_rig(&mut carried.equipment.item, round, &rules)) {
                (true, Ok(())) => {
                    if single {
                        firearm.discharge();
                    } else {
                        firearm.unload_own_chamber(at, false);
                    }
                    info!("the chamber's unload key: chamber {at}'s round to the rig");
                }
                (true, Err(refusal)) => info!("the chamber's unload key: the round stays ({refusal:?})"),
                (false, _) => {}
            }
        }
    }
}

/// The reload key's last resort for the weapon in hand
/// (`FirearmHandsInputTranslator.LoadAmmoToChamber`; firing spec, "A round
/// into the chamber by the reload key"), asked by the controller's step
/// where the key found no magazine to put in: the hands say what is left to
/// do (`Firearm::chamber_by_hand`), and the inventory does it. A round within
/// reach goes into the empty chamber as a round let go on the weapon does
/// (`apply_item`: one split off a stack, one not known refused), which the
/// hands then load (`settle`: `load_chamber`); an empty magazine goes to the
/// vest or the pockets first (`unload_to_rig`, the hands' `pull_out_magazine`)
/// and the round goes in as the hands are done with it (the move's callback,
/// after `PullOutMagOperation.OnMagPuttedToRig` has begun the idling).
///
/// What is carried is reached mutably only where it is written (through
/// its `ResMut`, not a `&mut Carried` taken from it, which marks it changed
/// on every frame: the inventory's page was then drawn afresh on every
/// frame, and nothing could be hovered, 0.211.0 to 0.247.0).
pub(super) fn chamber_by_hand(asked: bool, carried: &mut ResMut<Carried>, hands: &mut Hands, firearm: Option<&mut trp_sim::Firearm>, magazine_moved: bool) {
    let (Some(gun), Some(firearm)) = (hands.item, firearm) else {
        hands.chamber_after_magazine = None;
        return;
    };
    if hands.chamber_after_magazine.is_some_and(|waiting| waiting != gun) {
        hands.chamber_after_magazine = None;
    }
    // The hands' refusal is theirs (`Idling.Execute`), not the inventory's of
    // the last frame.
    let rules = trp_sim::items::inventory::Rules { busy_weapon: None, chamber_held: None, chamber_closed: None, ..carried.rules };
    let load = |carried: &mut Carried, firearm: &trp_sim::Firearm| {
        let Some(round) = trp_sim::items::inventory::round_for_chamber(&carried.equipment.item, gun) else { return };
        if !firearm.can_load_chamber() {
            info!("the reload key's last resort: the hands would not take a round into the chamber now");
            return;
        }
        match trp_sim::items::inventory::apply_item(&mut [&mut carried.equipment.item], round, gun, &rules) {
            Ok(_) => info!("the reload key's last resort: a round into the chamber by hand"),
            Err(refusal) => info!("the reload key's last resort: the round stays ({refusal:?})"),
        }
    };
    if magazine_moved && hands.chamber_after_magazine.take().is_some() {
        load(&mut **carried, firearm);
    }
    if !asked || carried.equipment.item.find(gun).is_none() {
        return;
    }
    let round = trp_sim::items::inventory::round_for_chamber(&carried.equipment.item, gun);
    match firearm.chamber_by_hand(round.is_some()) {
        trp_sim::ChamberByHand::Nothing => info!("the reload key's last resort: nothing ({})", if round.is_some() { "the hands said no" } else { "no round within reach" }),
        trp_sim::ChamberByHand::Load => load(&mut **carried, firearm),
        trp_sim::ChamberByHand::MagazineOutFirst => {
            let magazine = carried.find(gun).and_then(fitted_magazine);
            if let (Some(magazine), true) = (magazine, firearm.can_move_magazine()) {
                match trp_sim::items::inventory::unload_to_rig(&mut carried.equipment.item, magazine, &rules) {
                    Ok(()) => {
                        trp_sim::items::inventory::set_rounds_of(&mut carried.equipment.item, magazine, 0);
                        firearm.pull_out_magazine();
                        hands.chamber_after_magazine = Some(gun);
                        info!("the reload key's last resort: the empty magazine to the rig first");
                    }
                    Err(refusal) => info!("the reload key's last resort: the empty magazine stays ({refusal:?})"),
                }
            }
        }
    }
}
