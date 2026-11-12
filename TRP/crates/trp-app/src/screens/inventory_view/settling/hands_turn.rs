//! The hands' turn: the first weapon taken as the character is made, an item in use, and a weapon in hand.

use super::*;

/// What the hands' turn works with: the hands and the rig, what hangs where, the takes, and what was asked.
pub(super) struct Turn<'a> {
    pub(super) hands: &'a mut Hands,
    pub(super) rig: &'a mut WeaponRig,
    pub(super) weapon: &'a mut HandsRig,
    pub(super) controller: &'a mut Controller,
    pub(super) stowed: Option<Mut<'a, Stowed>>,
    pub(super) show: &'a mut dyn FnMut(Option<Entity>, bool),
    pub(super) hangs: &'a dyn Fn(&str) -> Option<Id>,
    pub(super) kind_of: &'a dyn Fn(Id) -> Option<String>,
    pub(super) folded_of: &'a dyn Fn(Id) -> bool,
    pub(super) combo_of: &'a dyn Fn(Id) -> bool,
    pub(super) chambers_of: &'a dyn Fn(Id) -> ChambersHeld,
    pub(super) magazine_of: &'a dyn Fn(Id) -> MagazineHeld,
    pub(super) slot_of: &'a dyn Fn(Id) -> Option<&'static str>,
    pub(super) first_but: &'a dyn Fn(Option<Id>) -> Option<(Id, &'static str)>,
    pub(super) level_of: &'a dyn Fn(&HandsRig) -> u32,
    pub(super) pistol: &'a dyn Fn(Option<Id>) -> bool,
    pub(super) empty_handed: &'a dyn Fn(&mut dyn FnMut(Option<Entity>, bool), &mut WeaponRig, &mut HandsRig, &mut Controller, Option<&mut Stowed>),
    #[allow(clippy::type_complexity)]
    pub(super) take_item: &'a dyn Fn(&mut dyn FnMut(Option<Entity>, bool), &mut WeaponRig, &mut HandsRig, &mut Controller, Option<&mut Stowed>, Id, (Option<trp_sim::condition::health::Part>, Option<f32>)) -> bool,
    #[allow(clippy::type_complexity)]
    pub(super) take: &'a dyn Fn(&mut dyn FnMut(Option<Entity>, bool), &mut WeaponRig, &mut HandsRig, &mut Controller, Option<&mut Stowed>, (Option<String>, bool), (bool, MagazineHeld, ChambersHeld)),
    pub(super) quick_draw: Option<trp_sim::weapons::quick_draw::QuickDraw>,
    pub(super) asked_for: Option<(Id, &'static str)>,
    pub(super) use_ready: Option<Id>,
    pub(super) use_part: Option<trp_sim::condition::health::Part>,
    pub(super) use_amount: Option<f32>,
    pub(super) gone: bool,
    pub(super) open: bool,
    pub(super) bench_put_away: bool,
    pub(super) quickdraw_begun: &'a mut bool,
}

/// As the character is made: the first weapon it has in hand, at rest, or none.
pub(super) fn take_first(turn: Turn) {
    let Turn { hands, rig, weapon, controller, mut stowed, mut show, kind_of, folded_of, combo_of, magazine_of, first_but, empty_handed, .. } = turn;
    let change = change_hands;
    // As the character is made: with the first weapon it has in hand
    // and at rest, or with none.
    match first_but(None) {
        Some((id, slot)) => {
            (hands.item, hands.active, hands.last_weapon) = (Some(id), Some(slot), Some(id));
            controller.armed = true;
            // The rig of the weapon it has, if that is not the one
            // the run was begun with.
            if let (Some(stowed), Some(kind)) = (stowed.as_deref_mut(), kind_of(id)) {
                if stowed.held() != Some(kind.as_str()) {
                    show(rig.weapon_root, false);
                    change(stowed, Some(&kind), rig, weapon, controller, combo_of(id));
                    show(rig.weapon_root, true);
                }
            }
            fold_as_item(weapon, rig, controller, folded_of(id), magazine_of(id));
            if let Some(firearm) = weapon.firearm.as_mut() {
                // One it began with in hand was drawn before: a
                // round is chambered (its item's from its magazine's
                // top, `settle` at rest).
                firearm.carried();
                firearm.start_drawn();
            }
            if let Some(knife) = weapon.knife.as_mut() {
                knife.start_drawn();
            }
            first_pose(weapon, controller);
        }
        None => {
            // A weapon the character does not begin with lies in the
            // crate as one found: with no round in its chamber (no
            // stock preset in the install's
            // database has one). The draw that first takes it in hand
            // works the action (firing spec, "Being drawn").
            if let Some(firearm) = weapon.firearm.as_mut() {
                firearm.found();
            }
            match stowed.as_deref_mut() {
                // Begun with the empty hands' rig in hand, every weapon's
                // made when first wanted (`weapon::spawn_weapon`): they
                // come up as they do when a weapon is put away.
                Some(stowed) if stowed.held().is_none() => {
                    controller.movement.body.set_weapon_width(rig.width);
                    controller.movement.body.set_hands_kind(rig.kind);
                    if let Some(empty) = weapon.empty.as_mut() {
                        empty.draw();
                    }
                    first_pose(weapon, controller);
                }
                stowed => empty_handed(&mut show, rig, weapon, controller, stowed),
            }
        }
    }
}

/// An item in the hands: it gives way to what is asked, and the hands held before come back when it is done.
pub(super) fn item_in_use(turn: Turn, using: Id) {
    let Turn { hands, rig, weapon, controller, mut stowed, mut show, hangs, kind_of, folded_of, combo_of, chambers_of, magazine_of, slot_of, first_but, empty_handed, take_item, take, asked_for, use_ready, use_part, use_amount, gone, .. } = turn;
    // An item in the hands: a key for anything else has it give way
    // (`MedsController.Drop`: its work cut short 0.6 s on), and what
    // was asked for comes when it has been put away; a use over by
    // itself asks at once for the hands held before it (the owner's
    // used-callback, `PlayerOwner.ValidateOnUsedCallback`, through the
    // same drop: `MedsHands` hands over at the later of the put-away's
    // mark and 0.6 s from the finish), here the weapon last held; or,
    // where the item was taken from the empty hands (the owner's
    // `PreviousItem` their item, which lies nowhere: `TryProceed`'s
    // `IsAtReachablePlace` fails and `SetFirstAvailableItem` takes
    // over; an item's own controller leaves `PreviousItem` as it was),
    // or where the weapon last held hangs no more, the first in the
    // slots' order (the sling's, the back's, the holster's, the
    // sheath's: `_slotPriority`), or the empty hands.
    let other_use = use_ready.filter(|id| *id != using);
    if asked_for.is_some() || other_use.is_some() {
        (hands.next, hands.next_use) = (asked_for, other_use);
        weapon.give_way();
    }
    if gone || !weapon.using() {
        hands.using = None;
        match hands.next_use.take() {
            Some(id) => {
                if take_item(&mut show, rig, weapon, controller, stowed.as_deref_mut(), id, (use_part, use_amount)) {
                    hands.using = Some(id);
                }
            }
            None => {
                let back = hands.last_weapon.filter(|_| !hands.from_empty).and_then(|id| slot_of(id).map(|slot| (id, slot)));
                match hands.next.take().filter(|(id, slot)| hangs(slot) == Some(*id)).or(back).or_else(|| first_but(None)) {
                    Some((id, slot)) => {
                        (hands.item, hands.active, hands.last_weapon) = (Some(id), Some(slot), Some(id));
                        take(&mut show, rig, weapon, controller, stowed.as_deref_mut(), (kind_of(id), combo_of(id)), (folded_of(id), magazine_of(id), chambers_of(id)));
                    }
                    None => empty_handed(&mut show, rig, weapon, controller, stowed.as_deref_mut()),
                }
            }
        }
    }
}

/// A weapon in hand: built anew where its build changed, put away where it left its slot or another is asked,
/// and what comes next taken once it is gone.
pub(super) fn armed(turn: Turn) {
    let Turn {
        hands, rig, weapon, controller, mut stowed, mut show, hangs, kind_of, folded_of, combo_of, chambers_of, magazine_of, slot_of, first_but, level_of, pistol, empty_handed, take_item, take, quick_draw, asked_for, use_ready, use_part, use_amount, gone, open, bench_put_away, quickdraw_begun,
    } = turn;
    let change = change_hands;
    // The weapon in hand built anew (a part put on or taken off, in the
    // inventory or at the workbench, or a magazine of another kind put
    // in): once its new build's rig is in the world, the hands take it
    // in the old one's place. At rest (the logic idle and its animation
    // over), with what that one had in it (`Firearm::take_over`) and its
    // item's magazine and chamber; through a reload or a magazine put
    // in, while no magazine is in sight, going on with it where the old
    // one was (`Firearm::carry_on_from`: its operation, its animator's
    // states and times), its magazine hidden until the new one shows.
    // The screen as it stands is told to it afresh. The app's: the game
    // changes the part on the weapon's own model.
    let at_rest = weapon.firearm.as_ref().is_some_and(|firearm| firearm.can_move_magazine() && !firearm.hands_busy());
    let unseen = weapon.firearm.as_ref().is_some_and(|firearm| firearm.magazine_out_of_sight());
    // A magazine going in already shown: the game's model shows the new
    // one from `MagShow` on. Where the new build's rig was not there by
    // then, the old one's magazine (another kind) is not shown, and the
    // new rig takes over the moment it is there, the reload going on.
    let shown_in = weapon.firearm.as_ref().is_some_and(|firearm| firearm.magazine_shown_going_in());
    if shown_in && !hands.going {
        let other = hands.item.and_then(kind_of).zip(stowed.as_deref()).is_some_and(|(key, stowed)| stowed.held().is_some_and(|held| held != key.as_str()));
        if other {
            controller.magazine_drawn = Some(false);
        }
    }
    if !hands.going && (at_rest || unseen || shown_in) {
        if let (Some(id), Some(stowed)) = (hands.item, stowed.as_deref_mut()) {
            if let Some(key) = kind_of(id).filter(|key| stowed.held().is_some_and(|held| held != key.as_str()) && stowed.built.contains(key)) {
                let before = stowed.held().map(str::to_owned);
                show(rig.weapon_root, false);
                if change(stowed, Some(&key), rig, weapon, controller, combo_of(id)) {
                    let old_hands = before.as_deref().and_then(|before| stowed.hands_of(before));
                    let old = old_hands.and_then(|old| old.firearm.as_ref());
                    let mut carried_on = false;
                    if let (Some(firearm), Some(old)) = (weapon.firearm.as_mut(), old) {
                        carried_on = !at_rest && firearm.carry_on_from(old);
                        if !carried_on {
                            firearm.take_over(old);
                        }
                    }
                    // The weapon's motion goes on as it was, and the
                    // frames still to be drawn keep their poses on the
                    // new rig's bones.
                    let map = before.as_deref().and_then(|before| stowed.rig_of(before)).filter(|_| carried_on).map(|old_rig| crate::weapons::weapon::bone_map(old_rig, rig));
                    if let (true, Some(old_hands)) = (carried_on, old_hands) {
                        weapon.hands.carry_on_from(&old_hands.hands);
                    }
                    // Its sights as they were: the same items, their
                    // state theirs (`Sights::take_state`). Only here: a
                    // weapon taken in another's place starts with its
                    // own.
                    if let (Some(new), Some(old)) = (weapon.hands.settings.sights.as_mut(), old_hands.and_then(|old| old.hands.sights())) {
                        new.take_state(old);
                    }
                    show(rig.weapon_root, true);
                    show(Some(rig.root), true);
                    if carried_on {
                        // (The logic has its magazine as the reload
                        // goes; the new build's is out of sight, or
                        // already shown.)
                        fold_as_item(weapon, rig, controller, folded_of(id), (None, None, None));
                        controller.magazine_drawn = Some(shown_in && !unseen);
                    } else {
                        fold_as_item(weapon, rig, controller, folded_of(id), magazine_of(id));
                        if let Some(firearm) = weapon.firearm.as_mut() {
                            firearm.start_drawn();
                        }
                    }
                    match map {
                        Some(map) => {
                            if let Some(pose) = weapon.animator().map(|animator| animator.pose_now()) {
                                let read = weapon.frame.read(&pose);
                                controller.rig_carried_over(&map, pose, read);
                            }
                        }
                        None => first_pose(weapon, controller),
                    }
                    // The old build's rig, read from above, is put away
                    // where it stands by: built back to it later, the
                    // weapon is drawn on it as on any other (otherwise the
                    // M4A1 built, its
                    // magazine taken out in hand, built again with one,
                    // came up with none shown and its hands busy).
                    if let Some(before) = before.as_deref() {
                        stowed.put_away(before);
                    }
                    hands.told_open = !open;
                    info!("the weapon in hand built anew: its rig {key}{}", if carried_on { ", the reload going on" } else { "" });
                }
            }
        }
    }
    // The weapon in hand has left the slot it was drawn from: it is
    // put away, and the next in the slots' order is taken. Or a key
    // asks for another, or for an item to be used: it is put away for
    // that one.
    let held = hands.item;
    let left = held.is_none_or(|id| slot_of(id) != hands.active);
    if !hands.going {
        let other = asked_for.filter(|(id, _)| Some(*id) != held);
        if bench_put_away && !left && other.is_none() && use_ready.is_none() {
            // Put away for the workbench: the hands are left empty,
            // and the slot is not drawn from again until it asks.
            (hands.going, hands.next, hands.next_use, hands.holstered) = (true, None, None, true);
            weapon.hide_held();
        } else if left || other.is_some() || use_ready.is_some() {
            hands.going = true;
            hands.next = if left && use_ready.is_none() { first_but(held) } else { other.filter(|_| use_ready.is_none()) };
            hands.next_use = use_ready;
            // Taken out of its slot, it waits on the hands to put it
            // away (`Player.TryRemoveFromHands`).
            if left {
                hands.moved_out = held;
            }
            // A firearm put away for the holster's weapon taken quick,
            // not lying (`Player.Proceed(Weapon)`,
            // `CheckForFastWeaponSwitch`): put away quick (`FastHide`),
            // at its quick speed, the arms paying as the put-away begins
            // (`Remove.Start`; `pistol-quickdraw`: the game's state, its
            // frames and every charge; not with an item's use, where it
            // would never run).
            let quick_away = !left && use_ready.is_none() && controller.fast_slot && !controller.movement.prone && hands.next.is_some_and(|(_, slot)| slot == "Holster");
            let level = level_of(weapon);
            let speed = quick_draw.map_or(1.0, |q| q.speed(pistol(held), level, true));
            match weapon.firearm.as_mut().filter(|_| quick_away) {
                Some(firearm) => {
                    firearm.hide_quick(speed);
                    if firearm.take_fast_switch() {
                        let now = controller.movement.frame_clock();
                        controller.movement.stamina.fast_switch(level, now);
                        hands.quickdraw = Some(now);
                        *quickdraw_begun = true;
                    }
                }
                None => weapon.hide_held(),
            }
        }
    } else if let Some(id) = use_ready {
        // A key pressed while it is going takes the place of what
        // was to come next.
        (hands.next, hands.next_use) = (None, Some(id));
    } else if let Some(other) = asked_for.filter(|(id, _)| Some(*id) != held || left) {
        (hands.next, hands.next_use) = (Some(other), None);
    }
    if gone || (hands.going && !weapon.held_in_hand()) {
        (hands.going, hands.item) = (false, None);
        controller.armed = false;
        // An item to be used; else what was to come next, if it still
        // hangs where it did.
        let next_use = hands.next_use.take();
        if let Some(id) = next_use.filter(|id| take_item(&mut show, rig, weapon, controller, stowed.as_deref_mut(), *id, (use_part, use_amount))) {
            (hands.using, hands.from_empty) = (Some(id), false);
            hands.moved_out = None;
        } else {
            match hands.next.take().filter(|(id, slot)| hangs(slot) == Some(*id)) {
                Some((id, slot)) => {
                    (hands.item, hands.active, hands.last_weapon) = (Some(id), Some(slot), Some(id));
                    take(&mut show, rig, weapon, controller, stowed.as_deref_mut(), (kind_of(id), combo_of(id)), (folded_of(id), magazine_of(id), chambers_of(id)));
                    // The holster's weapon drawn within a second of the
                    // quick put-away (`SetAnimatorAndProceduralValues`):
                    // the arms pay again, by its own skill, and it is
                    // drawn quick, the reload's speed 1, until it idles
                    // (`pistol-quickdraw`: 15 off the arms, or the legs
                    // once the arms are out, and `SpeedDraw` 1.5, the
                    // game's at every draw). Where that leaves the arms
                    // empty the weapon's motion shakes the hands, by what
                    // the arms lacked at the put-away and now
                    // (`StartHandShake`; `pistol-quickdraw`'s fourth: 10 s,
                    // its reach the game's), the drawn rig tied first as
                    // the game's is.
                    let now = controller.movement.frame_clock();
                    if slot == "Holster" && hands.quickdraw.take().is_some_and(|at| now - at < trp_sim::weapons::quick_draw::QUICK_WITHIN) {
                        let level = level_of(weapon);
                        let stamina = &mut controller.movement.stamina;
                        let before = stamina.switch_lack();
                        let drawn = stamina.fast_switch(level, now);
                        if let Some(quick) = quick_draw.filter(|_| stamina.arms.current <= 0.0) {
                            let full = 2.0 * stamina.settings.fast_switch_cost;
                            weapon.hands.start_hand_shake(quick.shake_for(before + drawn, full), quick.shake_strength);
                        }
                        let speed = quick_draw.map(|q| q.speed(pistol(Some(id)), level, false));
                        if let (Some(firearm), Some(speed)) = (weapon.firearm.as_mut(), speed) {
                            firearm.draw_quick(speed);
                        }
                    }
                }
                None => {
                    empty_handed(&mut show, rig, weapon, controller, stowed.as_deref_mut());
                    hands.moved_out = None;
                }
            }
        }
    }
}
