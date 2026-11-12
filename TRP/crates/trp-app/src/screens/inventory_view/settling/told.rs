//! What the hands hold now, told to the workbench, the blinking boxes, the hands and the battle bar.

use super::*;

/// What the telling reads and changes.
pub(super) struct Told<'a> {
    pub(super) hands: &'a mut Hands,
    pub(super) held: Mut<'a, HandsHeld>,
    pub(super) asked: Mut<'a, AskedFor>,
    pub(super) blinking: Mut<'a, Blinking>,
    pub(super) carried: &'a Carried,
    pub(super) features: &'a crate::screens::options_view::Features,
    pub(super) controller: &'a Controller,
    pub(super) weapon: &'a mut HandsRig,
    pub(super) stowed: Option<&'a Stowed>,
    pub(super) frame_count: &'a bevy::diagnostic::FrameCount,
    pub(super) first: bool,
    pub(super) drained: Option<Id>,
    pub(super) open: bool,
    pub(super) appeared: bool,
    pub(super) quickdraw_begun: bool,
}

/// After the hands' turn: what was asked is in the hands, what blinks, the screen told to what is in them, and
/// what they hold for the battle screen's quick bar.
pub(super) fn told(told: Told) {
    let Told { hands, mut held, mut asked, mut blinking, carried, features, controller, weapon, stowed, frame_count, first, drained, open, appeared, quickdraw_begun } = told;
    // What was asked is in the hands (for a run that writes each change).
    if controller.hands_change.as_ref().is_some_and(|change| change.made == frame_count.0) {
        hands.asked_at = None;
    }
    // The workbench's weapon in hand again (or gone from the character):
    // its ask is answered.
    if let Some(id) = asked.3.take {
        if (hands.item == Some(id) && !hands.going) || carried.equipment.item.find(id).is_none() || !features.weapon_handling {
            (asked.3.take, asked.3.rack) = (None, false);
        }
    }
    // The hands are done with a move when what they took has appeared in
    // them (`SpawnOperation.WeaponAppeared`, at the draw's last mark).
    if appeared {
        (hands.moved_in, hands.moved_out) = (None, None);
    }
    // What blinks: those, and a weapon put into the slot the hands last
    // drew from while its rig is still being made (the hands' taking it
    // waits for that here).
    let rig_coming = (!controller.armed && !hands.going)
        .then(|| hands.active.and_then(|slot| carried.equipment.worn(slot)))
        .flatten()
        .filter(|weapon| Some(weapon.id) != hands.item && stowed.as_deref().is_some_and(|stowed| stowed.awaited(&trp_sim::items::inventory::rig_key(weapon))))
        .map(|weapon| weapon.id);
    let now: Vec<Id> = hands.moved_in.into_iter().chain(hands.moved_out).chain(rig_coming).chain(hands.magazine_moving).chain(drained).collect();
    if blinking.0 != now {
        blinking.0 = now;
    }
    // The screen opening and shutting, told to what is in the hands now;
    // and the screen as it stands, told to what has just appeared there
    // (a firearm is told in its own step, before its sights are turned:
    // `Firearm::set_appearing`).
    let changed = open != hands.told_open;
    if changed || appeared {
        hands.told_open = open;
        if let Some(firearm) = weapon.firearm.as_mut().filter(|_| changed) {
            firearm.set_inventory(open);
        }
        if let Some(empty) = weapon.empty.as_mut() {
            empty.set_inventory(open);
        }
        if let Some(grenade) = weapon.grenade.as_mut() {
            grenade.set_inventory(open);
        }
        if let Some(rangefinder) = weapon.rangefinder.as_mut() {
            rangefinder.set_inventory(open);
        }
        if let Some(knife) = weapon.knife.as_mut() {
            knife.set_inventory(open);
        }
    }
    // What the hands hold, and a change of them begun, for the battle
    // screen's quick bar.
    let (item, using, going) = hands.was;
    // (Not the first weapon taken as the character is made: it is in the
    // hands before the battle screen shows.)
    let began = !first && ((hands.going && !going) || (!going && item.is_none() && hands.item.is_some()) || (using.is_none() && hands.using.is_some()));
    if began {
        held.changes = held.changes.wrapping_add(1);
    }
    if quickdraw_begun {
        held.quickdraws = held.quickdraws.wrapping_add(1);
    }
    hands.was = (hands.item, hands.using, hands.going);
    let now = hands.using.or(hands.item);
    if held.item != now {
        held.item = now;
    }
    let takes_from = hands.active.filter(|_| hands.item.is_none() && hands.using.is_none() && !hands.going && !hands.holstered);
    if held.takes_from != takes_from {
        held.takes_from = takes_from;
    }
}
