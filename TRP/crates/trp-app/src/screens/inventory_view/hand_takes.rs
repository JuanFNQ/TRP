//! Taking things into the hands, from empty hands or another thing.

use super::*;

/// An item let go on a part of the HEALTH tab, or used from the screen
/// (`AskedFor`): the item, the part it is for, and food's or drink's share
/// asked (none: what is left).
pub(super) type UseAsked = (Id, Option<trp_sim::condition::health::Part>, Option<f32>);

/// What a weapon's item says of its magazine as it is taken in hand: the
/// fitted magazine's rounds (none where it is no weapon), whether a round
/// is in its chamber (a weapon with one), and the fitted magazine's clips'
/// index, whether it comes out and the rounds its chamber refuses.
pub(super) type MagazineHeld = (Option<Option<u32>>, Option<bool>, Option<(i32, bool, u32)>);

/// A change of the hands noted for a run that writes each
/// (`controller::hands_log`): when it was asked and the frame it is made.
pub(super) type Logged = Option<(Option<u32>, u32)>;

/// The weapons the hands may take (`settle`, `hands_first`): what hangs in
/// each slot a weapon is held from, of the kinds built. A weapon is by its
/// build (`trp_sim::inventory::rig_key`); the one in hand hangs where it
/// hangs while the rig of a build it has just been given is still being
/// made (it keeps its old one meanwhile). And what a weapon taken in hand
/// brings from its item.
pub(super) struct HandSlots<'a> {
    pub(super) carried: &'a Carried,
    pub(super) in_hand: Option<Id>,
    pub(super) built: &'a [String],
    /// The tactical devices switched on (`TACTICAL DEVICES`).
    pub(super) devices: bool,
}

impl HandSlots<'_> {
    pub(super) fn hangs(&self, slot: &str) -> Option<Id> {
        self.carried.equipment.worn(slot).filter(|weapon| Some(weapon.id) == self.in_hand || self.built.contains(&trp_sim::items::inventory::rig_key(weapon))).map(|weapon| weapon.id)
    }

    pub(super) fn slot_of(&self, id: Id) -> Option<&'static str> {
        HAND_SLOTS.into_iter().find(|slot| self.hangs(slot) == Some(id))
    }

    pub(super) fn first_but(&self, but: Option<Id>) -> Option<(Id, &'static str)> {
        HAND_SLOTS.into_iter().find_map(|slot| self.hangs(slot).filter(|id| Some(*id) != but).map(|id| (id, slot)))
    }

    pub(super) fn kind_of(&self, id: Id) -> Option<String> {
        self.carried.equipment.item.find(id).map(trp_sim::items::inventory::rig_key)
    }

    pub(super) fn folded_of(&self, id: Id) -> bool {
        self.carried.equipment.item.find(id).and_then(trp_sim::items::inventory::folding_part).is_some_and(|part| part.folded)
    }

    /// A combo unit on the weapon on, which its hip penalty goes by as its
    /// controller is made (`UpdateHipInaccuracy`); the devices switched off
    /// count for nothing.
    pub(super) fn combo_of(&self, id: Id) -> bool {
        self.devices && self.carried.equipment.item.find(id).is_some_and(trp_sim::weapons::tactical::combo_on)
    }

    pub(super) fn chambers_of(&self, id: Id) -> ChambersHeld {
        item_chambers(self.carried, id)
    }

    pub(super) fn magazine_of(&self, id: Id) -> MagazineHeld {
        let gun = self.carried.equipment.item.find(id);
        let fitted = gun.and_then(|gun| gun.in_slot(trp_sim::items::inventory::MAGAZINE_SLOT));
        let unfit = |magazine: &Item| gun.map_or(0, |gun| trp_sim::items::inventory::unfit_rounds(gun, magazine));
        (gun.map(|_| fitted.map(trp_sim::items::inventory::magazine_rounds)), gun.filter(|gun| trp_sim::items::inventory::chamber_slot(gun).is_some()).map(|gun| trp_sim::items::inventory::chambered(gun).is_some()), fitted.map(|magazine| (magazine.kind.magazine_animation, magazine.kind.magazine_external, unfit(magazine))))
    }
}

/// What the keys and the screen ask of the hands on a frame, as the hands
/// hear it: a slot's key (the holster's pressed twice taking its weapon
/// quick), a quick-use key's weapon or item, the quick stroke's knife, an
/// item let go on the HEALTH tab or used from the screen, the workbench's
/// weapon. Returned: the weapon asked for and the slot it hangs in, and the
/// item to be used whose rig is ready; what waits for its rig is kept in
/// `hands`. `keys`: whether the frame's slot keys are still to be heard
/// (`hands_first` hears them first, with the hands empty).
#[allow(clippy::too_many_arguments)]
pub(super) fn hands_asked(
    hands: &mut Hands,
    slots: &HandSlots,
    controller: &mut Controller,
    (options, features, bindings): (&Options, &crate::screens::options_view::Features, &Bindings),
    weapon: &HandsRig,
    stowed: Option<&Stowed>,
    preparing: Option<&mut crate::weapons::weapon::Preparing>,
    (keys, in_move, frame): (bool, bool, u32),
    (quick, quick_knife, dropped, bench_take): (Option<Id>, bool, Option<UseAsked>, Option<Id>),
) -> (Option<(Id, &'static str)>, Option<Id>) {
    let carried = slots.carried;
    // A slot's key, or a quick-use key bound to a weapon.
    // With the weapon's handling switched off nothing is drawn or put
    // away by a key (settings spec, "Features"); under the edit build
    // screen, as under the inventory, no key is heard.
    let live = !controller.inventory_open && !controller.edit_build && options.keys_live() && features.weapon_handling;
    // The holster's key pressed twice takes the holster's weapon quick
    // (`QuickSelectSecondaryWeapon`: `SetSlotItem(1, fast)`); every slot
    // taken says whether it was so (`FastSlotSelection`).
    let keyed = live && !in_move && keys;
    let quick = quick.filter(|_| !in_move);
    let quick_pistol = keyed && bindings.fired("QuickSecondaryWeapon");
    let pressed = if quick_pistol { Some("Holster") } else { SLOT_KEYS.iter().find(|(key, _)| keyed && bindings.fired(key)).map(|(_, slot)| *slot) };
    if pressed.is_some() || quick.is_some() {
        controller.fast_slot = quick_pistol;
    }
    // A key for a weapon whose rig is still to come waits for it.
    let worn_in = |slot: &str| carried.equipment.worn(slot);
    let coming = |slot: &str| worn_in(slot).filter(|weapon| slots.hangs(slot).is_none() && stowed.is_some_and(|stowed| stowed.awaited(&trp_sim::items::inventory::rig_key(weapon))));
    if let Some(slot) = pressed {
        hands.waiting = coming(slot).map(|weapon| (weapon.id, slot));
    }
    let waited = hands.waiting.filter(|(id, slot)| live && slots.hangs(slot) == Some(*id));
    if waited.is_some() || hands.waiting.is_some_and(|(id, slot)| worn_in(slot).is_none_or(|weapon| weapon.id != id) || coming(slot).is_none() && slots.hangs(slot).is_none()) {
        hands.waiting = None;
    }
    // A thing not known is not taken in hand (`IsAtReachablePlace`).
    let known = |id: &Id| carried.equipment.item.find(*id).is_some_and(trp_sim::items::examine::known);
    let key = pressed.and_then(|slot| slots.hangs(slot).filter(known).map(|id| (id, slot))).or(waited);
    let asked_for = key.or_else(|| quick.filter(|_| live).and_then(|id| slots.slot_of(id).map(|slot| (id, slot)))).or_else(|| bench_take.and_then(|id| slots.slot_of(id).map(|slot| (id, slot))));
    // A quick-use key's item to be used: its rig made first if it has none
    // yet (as a weapon's key waits for its rig), and a key for anything
    // else meanwhile forgets it.
    let item_of = |id: Id| carried.equipment.item.find(id).filter(|item| !is(item, "Weapon"));
    if let Some(id) = quick.filter(|id| live && item_of(*id).is_some()) {
        (hands.use_waiting, hands.use_part, hands.use_amount, hands.use_quick) = (Some(id), None, None, false);
    } else if asked_for.is_some() {
        hands.use_waiting = None;
    }
    // The quick stroke (`PlayerInputTranslator.method_4`): the sheath's
    // knife, with no knife in hand already, with stamina for a stroke, and
    // not in a movement state that takes no hand input (a sprint, a crawl).
    if quick_knife && live && controller.movement.state_suits_hands() {
        let knife = carried.equipment.worn("Scabbard").filter(|knife| is(knife, "Knife")).map(|knife| knife.id);
        let knife_in_hand = hands.item.is_some_and(|id| Some(id) == knife) || weapon.knife.as_ref().is_some_and(|knife| knife.in_hand());
        match knife {
            Some(id) if !knife_in_hand && controller.movement.stamina.can_melee() => {
                (hands.use_waiting, hands.use_part, hands.use_amount, hands.use_quick) = (Some(id), None, None, true);
            }
            // Too tired (`Physical.InvokeInsufficient`).
            Some(_) if !knife_in_hand => info!("the quick stroke: too tired for it"),
            _ => {}
        }
    }
    // An item let go on a part of the HEALTH tab, or used from the screen,
    // with the screen open.
    if let Some((id, part, amount)) = dropped.filter(|(id, ..)| features.weapon_handling && item_of(*id).is_some()) {
        (hands.use_waiting, hands.use_part, hands.use_amount) = (Some(id), part, amount);
    }
    // (When the hands were asked, for a run that writes each change.)
    if pressed.is_some() || quick.is_some() || (quick_knife && live) || dropped.is_some() {
        hands.asked_at = Some(frame);
    }
    let mut use_ready = None;
    if let Some(id) = hands.use_waiting {
        match (item_of(id).map(|item| item.kind.template.clone()), stowed) {
            (Some(template), Some(stowed)) if stowed.built.contains(&template) => use_ready = Some(id),
            (Some(template), Some(stowed)) if stowed.awaited(&template) => {
                if let Some(preparing) = preparing {
                    preparing.ask(&template);
                }
            }
            _ => hands.use_waiting = None,
        }
        if use_ready.is_some() {
            hands.use_waiting = None;
        }
    }
    (asked_for, use_ready)
}

/// A change of the hands noted for a run that writes each.
fn note_change(controller: &mut Controller, logged: Logged, what: &str) {
    if let Some((asked, made)) = logged {
        controller.hands_change = Some(crate::player::controller::HandsChange { asked, made, what: what.to_owned() });
    }
}

/// What is in the hands is changed for another rig that stands by (a
/// weapon's, by its template, or the empty hands'), and the body's
/// animator is told what is now there. Whether there was such a rig.
pub(super) fn change_hands(stowed: &mut Stowed, to: Option<&str>, rig: &mut WeaponRig, weapon: &mut HandsRig, controller: &mut Controller, combo: bool) -> bool {
    if !stowed.change_to(to, rig, weapon, combo) {
        return false;
    }
    let folded = weapon.firearm.as_ref().is_some_and(|firearm| firearm.stock_folded());
    controller.movement.body.set_weapon_width(if folded { weapon.folded_width.unwrap_or(rig.width) } else { rig.width });
    // With no controller between the two the body is told the empty
    // hands at once (`DestroyController`), and the new kind eases in
    // from there (`PlayerAnimatorSetWeaponIdSmooth`).
    controller.movement.body.start_hands_kind(trp_sim::locomotion::body::hands_kind::EMPTY_HANDS);
    controller.movement.body.set_hands_kind(rig.kind);
    // Aiming costs the arms by the weapon's own ergonomic weight.
    if to.is_some() {
        controller.movement.stamina.aim_mass = weapon.aim_mass;
    }
    // The firearm drawn is told the arms before its draw tells its
    // animator the skill's speeds (`Firearm::set_arms_damaged`).
    if let Some(firearm) = weapon.firearm.as_mut() {
        let conditions = controller.movement.conditions;
        firearm.set_arms_damaged(conditions.left_arm, conditions.right_arm);
    }
    true
}

/// The weapon is out of sight and the empty hands appear.
pub(super) fn empty_hands_in(show: &mut dyn FnMut(Option<Entity>, bool), (rig, weapon, controller): (&mut WeaponRig, &mut HandsRig, &mut Controller), stowed: Option<&mut Stowed>, logged: Logged) {
    match stowed {
        Some(stowed) if stowed.held().is_some() => {
            show(rig.weapon_root, false);
            if change_hands(stowed, None, rig, weapon, controller, false) {
                if let Some(empty) = weapon.empty.as_mut() {
                    empty.draw();
                }
                first_pose(weapon, controller);
                note_change(controller, logged, "the empty hands");
            } else {
                // No rig for empty hands could be read: nothing is drawn.
                show(Some(rig.root), false);
            }
        }
        Some(_) => {}
        None => show(Some(rig.root), false),
    }
}

/// An item is taken in hand to be used (health spec, "The hands"): its rig
/// is changed for whatever was there, and its controller puts it to work
/// as it spawns, on the common body as a quick-use key names it
/// (`TargetBodyParts`), food and drink all that is left. Refused (it can do
/// nothing after all), it gives way as a use cut short does. A grenade is
/// drawn, the rangefinder too; the knife strikes, its quick stroke where
/// that was asked. Whether it was taken.
#[allow(clippy::too_many_arguments)]
pub(super) fn item_into_hands(carried: &Carried, quick_stroke: bool, logged: Logged, show: &mut dyn FnMut(Option<Entity>, bool), (rig, weapon, controller): (&mut WeaponRig, &mut HandsRig, &mut Controller), stowed: Option<&mut Stowed>, id: Id, (part, amount): (Option<trp_sim::condition::health::Part>, Option<f32>)) -> bool {
    let (Some(stowed), Some(item)) = (stowed, carried.equipment.item.find(id)) else { return false };
    if let Some(empty) = weapon.empty.as_mut() {
        empty.hide();
    }
    if stowed.held().is_some() {
        show(rig.weapon_root, false);
    }
    if !change_hands(stowed, Some(&item.kind.template), rig, weapon, controller, false) {
        return false;
    }
    show(rig.weapon_root, true);
    show(Some(rig.root), true);
    controller.armed = false;
    if let (Some(meds), Some(health)) = (weapon.meds.as_mut(), controller.health.as_mut()) {
        let resource = item.resource.unwrap_or_else(|| meds.medicine().full());
        // Food and drink go to the body whole, whatever part they
        // were let go on (`PlayerHealthController.ApplyItem`).
        let parts = if meds.medicine().food() { [None] } else { [part] };
        // Told whose eyes see it as it spawns (`SetPointOfViewOnSpawn`).
        meds.set_point_of_view(controller.movement.body.point_of_view());
        if meds.take(health, id, resource, &parts, amount, None).is_none() {
            meds.drop();
        }
    }
    // A grenade is drawn, the rangefinder too; the knife strikes its
    // quick stroke.
    if let Some(grenade) = weapon.grenade.as_mut() {
        grenade.draw();
    }
    if let Some(rangefinder) = weapon.rangefinder.as_mut() {
        rangefinder.draw();
    }
    if let Some(knife) = weapon.knife.as_mut() {
        let speed = controller.movement.stamina.melee_speed();
        if quick_stroke {
            knife.draw_quick(speed);
        } else {
            knife.draw();
        }
    }
    first_pose(weapon, controller);
    note_change(controller, logged, &item.kind.template);
    true
}

/// A weapon is taken in hand: its own rig is changed for whatever was
/// there, the empty hands going at once with no state of their own waited
/// for, and it is drawn as its item has it (its magazine, fold and
/// chambers of its own).
pub(super) fn weapon_into_hands(logged: Logged, show: &mut dyn FnMut(Option<Entity>, bool), (rig, weapon, controller): (&mut WeaponRig, &mut HandsRig, &mut Controller), stowed: Option<&mut Stowed>, (kind, combo): (Option<String>, bool), (folded, magazine, own): (bool, MagazineHeld, ChambersHeld)) {
    let mut ours = true;
    if let Some(stowed) = stowed {
        if stowed.held() != kind.as_deref() {
            if let Some(empty) = weapon.empty.as_mut() {
                empty.hide();
            }
            // Another weapon's own tree goes out of sight.
            if stowed.held().is_some() {
                show(rig.weapon_root, false);
            }
            change_hands(&mut *stowed, kind.as_deref(), rig, weapon, controller, combo);
        }
        ours = stowed.held() == kind.as_deref();
    }
    show(rig.weapon_root, true);
    show(Some(rig.root), true);
    controller.armed = true;
    // Its magazine in sight (a rig's model has one only where its build
    // does): a rig left with its magazine hidden (taken out in hand, a
    // reload's old one let go) shows it again as it is drawn.
    controller.magazine_drawn = Some(true);
    fold_as_item(weapon, rig, controller, folded, magazine);
    // The hands made from the item: the rounds and cases in its
    // chambers of their own, before the draw (`settle` keeps them so).
    if let Some(firearm) = weapon.firearm.as_mut().filter(|firearm| ours && !firearm.single_chamber()) {
        let held = |list: &[Option<String>]| list.iter().map(Option::is_some).collect::<Vec<bool>>();
        firearm.hold_rounds(&held(&own.0), &held(&own.1), true);
    }
    weapon.draw_held();
    first_pose(weapon, controller);
    note_change(controller, logged, kind.as_deref().unwrap_or("-"));
}

/// The empty hands take what was asked at once, the empty hands going with
/// no state of their own waited for: an item to be used; else a key's
/// weapon, or one put into the slot the hands last drew from, which waits
/// on the hands to take it (`Player.TrySetInHands`); the workbench's weapon
/// built anew comes up racking.
#[allow(clippy::too_many_arguments)]
pub(super) fn take_into_empty(
    hands: &mut Hands,
    slots: &HandSlots,
    show: &mut dyn FnMut(Option<Entity>, bool),
    (rig, weapon, controller): (&mut WeaponRig, &mut HandsRig, &mut Controller),
    mut stowed: Option<&mut Stowed>,
    (asked_for, use_ready): (Option<(Id, &'static str)>, Option<Id>),
    (bench_take, bench_rack): (Option<Id>, bool),
    logged: Logged,
) {
    let (quick_stroke, use_asked) = (hands.use_quick, (hands.use_part, hands.use_amount));
    if let Some(id) = use_ready.filter(|id| item_into_hands(slots.carried, quick_stroke, logged, show, (&mut *rig, &mut *weapon, &mut *controller), stowed.as_deref_mut(), *id, use_asked)) {
        (hands.using, hands.from_empty) = (Some(id), true);
        return;
    }
    let back = hands.active.filter(|_| !hands.holstered).and_then(|slot| slots.hangs(slot).map(|id| (id, slot)));
    let Some((id, slot)) = asked_for.or(back) else { return };
    (hands.item, hands.active, hands.last_weapon, hands.holstered) = (Some(id), Some(slot), Some(id), false);
    weapon_into_hands(logged, show, (&mut *rig, &mut *weapon, &mut *controller), stowed, (slots.kind_of(id), slots.combo_of(id)), (slots.folded_of(id), slots.magazine_of(id), slots.chambers_of(id)));
    if bench_take == Some(id) && bench_rack {
        if let Some(firearm) = weapon.firearm.as_mut() {
            firearm.rack_drawn();
        }
    }
    if asked_for.is_none() {
        hands.moved_in = Some(id);
    }
}

/// From the empty hands, what the keys and the screen ask is taken in hand
/// before the controller's step (health spec, "The hands"): the game makes
/// those hands within the key's or the click's own call (`Proceed`: the
/// empty hands' `Drop` calls back at once, and the new controller is made
/// and spawned there), before that frame's health and hands update, so an
/// item's effect and its animator are stepped on the key's frame, and a
/// weapon's draw too. The same take as `settle`'s from the empty hands,
/// which then has the frame's asks and keys done. From anything else in
/// hand the hands go by `settle`, after the step, as the game's put-away
/// ends in its animator's update.
#[allow(clippy::too_many_arguments)]
pub(super) fn hands_first(
    carried: Res<Carried>,
    (options, bindings, features): (Res<Options>, Res<Bindings>, Res<crate::screens::options_view::Features>),
    mut asked: ResMut<AskedFor>,
    rig: Option<ResMut<WeaponRig>>,
    weapon: Option<ResMut<HandsRig>>,
    mut stowed: Option<ResMut<Stowed>>,
    mut preparing: Option<ResMut<crate::weapons::weapon::Preparing>>,
    mut controller: ResMut<Controller>,
    mut hands: ResMut<Hands>,
    mut shown: Query<&mut Visibility, crate::platform::park::EvenParked>,
    mut again: MessageReader<crate::player::death::BeginAgain>,
    frame_count: Res<bevy::diagnostic::FrameCount>,
) {
    hands.heard = false;
    // The run begun again after a death is `settle`'s to start over.
    if again.read().count() > 0 || !hands.settled || controller.armed || hands.using.is_some() {
        return;
    }
    let (Some(mut rig), Some(mut weapon)) = (rig, weapon) else { return };
    let (rig, weapon, controller) = (&mut *rig, &mut *weapon, &mut *controller);
    let built: Vec<String> = stowed.as_deref().map_or_else(|| vec![options.weapon.clone()], |stowed| stowed.built.clone());
    let slots = HandSlots { carried: &*carried, in_hand: hands.item, built: &built, devices: features.tactical_devices };
    let in_move = weapon.firearm.as_ref().is_some_and(|firearm| firearm.in_inventory_move());
    let (quick, quick_knife, dropped) = (asked.0.take(), std::mem::take(&mut asked.2), asked.1.take());
    let (bench_take, bench_rack) = (asked.3.take.filter(|_| features.weapon_handling), asked.3.rack);
    let asks = hands_asked(&mut hands, &slots, controller, (&options, &features, &bindings), weapon, stowed.as_deref(), preparing.as_deref_mut(), (true, in_move, frame_count.0), (quick, quick_knife, dropped, bench_take));
    hands.heard = true;
    let logged = crate::player::controller::hands_log().then_some((hands.asked_at, frame_count.0));
    let mut show = |entity: Option<Entity>, on: bool| {
        if let Some(mut shown) = entity.and_then(|entity| shown.get_mut(entity).ok()) {
            *shown = if on { Visibility::Inherited } else { Visibility::Hidden };
        }
    };
    take_into_empty(&mut hands, &slots, &mut show, (&mut *rig, &mut *weapon, &mut *controller), stowed.as_deref_mut(), asks, (bench_take, bench_rack), logged);
    if controller.hands_change.as_ref().is_some_and(|change| change.made == frame_count.0) {
        hands.asked_at = None;
    }
}
