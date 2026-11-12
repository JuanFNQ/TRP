//! Settling the inventory's changes each frame.

use super::*;

mod before;
mod chambers;
mod hands_turn;
mod magazines;
mod rounds;
mod told;

/// What every phase of the settling works on: what is carried, the controller and the hands.
pub(super) struct Books<'a> {
    pub(super) carried: Mut<'a, Carried>,
    pub(super) controller: Mut<'a, Controller>,
    pub(super) hands: Mut<'a, Hands>,
}

pub(super) fn settle(
    mut carried: ResMut<Carried>,
    options: Res<Options>,
    game: Res<crate::GameData>,
    mut preparing: Option<ResMut<crate::weapons::weapon::Preparing>>,
    bindings: Res<Bindings>,
    mut asked: ResMut<AskedFor>,
    rig: Option<ResMut<WeaponRig>>,
    mut weapon: Option<ResMut<HandsRig>>,
    mut stowed: Option<ResMut<Stowed>>,
    mut controller: ResMut<Controller>,
    (features, skills, mut chamber_kinds): (Res<crate::screens::options_view::Features>, Option<Res<crate::player::skills::Skills>>, ResMut<crate::weapons::magazine_rounds::ChamberKinds>),
    mut blinking: ResMut<Blinking>,
    mut shown: Query<&mut Visibility, crate::platform::park::EvenParked>,
    mut hands: ResMut<Hands>,
    mut again: MessageReader<crate::player::death::BeginAgain>,
    (mut held, drag, frame_count): (ResMut<HandsHeld>, Res<Drag>, Res<bevy::diagnostic::FrameCount>),
) {
    // The run begun again after a death: the hands start over, as the
    // character is made (`death`).
    if again.read().count() > 0 {
        *hands = Hands::default();
    }
    before::checks(Books { carried: carried.reborrow(), controller: controller.reborrow(), hands: hands.reborrow() });
    let _clock = crate::platform::render_timing::Clock::new("inventory settle");
    // A weapon hung where the hands can take it with no rig yet has its
    // rig made ready before any other (`weapon::Preparing`); the hands
    // take it once it is in the world.
    // (The sheath's knife only with the options' switch for it on.)
    let waiting = before::ask_rigs(&carried, &features, stowed.as_deref(), preparing.as_deref_mut());
    let game = &*game;
    // The character is made with the first weapon it has in hand: if that
    // one's rig is still to come, the hands wait for it.
    if !hands.settled && waiting {
        return;
    }
    let first = !std::mem::replace(&mut hands.settled, true);
    let quick = asked.0.take();
    let quick_knife = std::mem::take(&mut asked.2);
    before::grenade_thrown(Books { carried: carried.reborrow(), controller: controller.reborrow(), hands: hands.reborrow() });
    let dropped = asked.1.take();
    // The workbench's asks (`BenchAsk`), with the weapon's handling on.
    let bench_put_away = std::mem::take(&mut asked.3.put_away) && features.weapon_handling;
    let bench_take = asked.3.take.filter(|_| features.weapon_handling);
    let bench_rack = asked.3.rack;
    before::items_said(Books { carried: carried.reborrow(), controller: controller.reborrow(), hands: hands.reborrow() });
    magazines::reloads_and_next(
        Books { carried: carried.reborrow(), controller: controller.reborrow(), hands: hands.reborrow() },
        held.reborrow(),
        weapon.as_deref(),
        stowed.as_mut().map(|stowed| stowed.reborrow()),
        preparing.as_deref_mut(),
        &drag,
    );
    rounds::thrown_rounds(Books { carried: carried.reborrow(), controller: controller.reborrow(), hands: hands.reborrow() }, game, &chamber_kinds);
    // The weapon in hand's magazine moved in the inventory (firing spec,
    // "A magazine taken out or put in from the inventory"): one thing moved
    // out or in, its hands do it, the magazine's box blinking until they
    // are done; two swapped, its logic simply has the new one. The one that
    // leaves takes the rounds the logic had in it. While the hands cannot
    // take such a move, the inventory refuses it (`Rules::busy_weapon`).
    let magazine_moved = std::mem::take(&mut controller.magazines_moved) > 0;
    if magazine_moved {
        hands.magazine_moving = None;
    }
    // The reload key's last resort, its moves made before the fitted
    // magazine is set beside the last frame's, so that the hands' move of
    // one is told below as any other.
    let gun_hands = weapon.as_deref_mut().and_then(|rig| rig.firearm.as_mut());
    chamber_by_hand(std::mem::take(&mut controller.chamber_by_hand), &mut carried, &mut hands, gun_hands, magazine_moved);
    let fitted = hands.item.and_then(|id| carried.equipment.item.find(id)).map(|gun| (gun.id, fitted_magazine(gun)));
    let mut firearm = weapon.as_deref_mut().and_then(|rig| rig.firearm.as_mut());
    magazines::fitted_changed(Books { carried: carried.reborrow(), controller: controller.reborrow(), hands: hands.reborrow() }, &mut firearm, fitted);
    rounds::folds_and_shots(Books { carried: carried.reborrow(), controller: controller.reborrow(), hands: hands.reborrow() }, &mut firearm);
    let quiet = chambers::chamber_agreed(Books { carried: carried.reborrow(), controller: controller.reborrow(), hands: hands.reborrow() }, game, &mut firearm, fitted);
    let taken = chambers::launcher(Books { carried: carried.reborrow(), controller: controller.reborrow(), hands: hands.reborrow() }, game, &mut firearm);
    chambers::own_chambers(Books { carried: carried.reborrow(), controller: controller.reborrow(), hands: hands.reborrow() }, game, &held, chamber_kinds.reborrow(), stowed.as_deref(), &mut firearm);
    Carried::unheard(&mut carried, quiet && !taken);
    if hands.fitted.map(|(gun, _)| gun) != fitted.map(|(gun, _)| gun) {
        hands.fired = false;
    }
    hands.fitted = fitted;
    // Nothing goes into the chamber of the weapon in hand whose hands would
    // not take it now.
    let closed = hands.item.filter(|_| !firearm.as_deref().is_some_and(|firearm| firearm.can_load_chamber()));
    if carried.rules.chamber_closed != closed {
        carried.bypass_change_detection().rules.chamber_closed = closed;
    }
    // And nothing leaves it where they would not let the round out.
    let round_kept = hands.item.filter(|_| !firearm.as_deref().is_some_and(|firearm| firearm.can_discharge()));
    if carried.rules.chamber_held != round_kept {
        carried.bypass_change_detection().rules.chamber_held = round_kept;
    }
    // The weapon in hand part of a move from the inventory the hands are
    // still playing (a magazine, a chamber's or a barrel's round, a
    // cylinder's): a slot's key or a quick-use key does nothing until it
    // is over (`PlayerInputTranslator.SetSlotItem`:
    // `CurrentWeaponIsUsedInRemoveOperation`, the weapon's `CheckAction`
    // failing while the operation runs). Seen in the game: the Rhino's
    // round put into its cylinder
    // and one taken out, the key pressed throughout, the hands finishing
    // and the next press putting it away.
    let in_move = firearm.as_deref().is_some_and(|firearm| firearm.in_inventory_move());
    let busy = hands.item.filter(|_| firearm.is_some_and(|firearm| !firearm.can_move_magazine()));
    if carried.rules.busy_weapon != busy {
        carried.bypass_change_detection().rules.busy_weapon = busy;
    }
    // The thing the health is at work with (`HealthController.ApplyItem`
    // to its effect's end): drained, and blinking (the app's).
    let drained = controller.health.as_ref().and_then(|health| health.item_at_work()).map(|work| work.item).filter(|id| carried.find(*id).is_some());
    if carried.drained != drained {
        carried.drained = drained;
    }
    let carried = &*carried;
    let (Some(mut rig), Some(mut weapon)) = (rig, weapon) else { return };
    let (rig, weapon, controller) = (&mut *rig, &mut *weapon, &mut *controller);
    let open = controller.inventory_open;
    // What hangs in each slot a weapon is held from, of the kinds built.
    let built: Vec<String> = stowed.as_deref().map_or_else(|| vec![options.weapon.clone()], |stowed| stowed.built.clone());
    // What hangs where the hands take it, and what a weapon brings from its
    // item (`HandSlots`).
    let in_hand = hands.item;
    let slots = HandSlots { carried, in_hand, built: &built, devices: features.tactical_devices };
    let hangs = |slot: &str| slots.hangs(slot);
    let kind_of = |id: Id| slots.kind_of(id);
    let folded_of = |id: Id| slots.folded_of(id);
    let combo_of = |id: Id| slots.combo_of(id);
    let chambers_of = |id: Id| slots.chambers_of(id);
    let magazine_of = |id: Id| slots.magazine_of(id);
    let slot_of = |id: Id| slots.slot_of(id);
    let first_but = |but: Option<Id>| slots.first_but(but);
    let quick_draw = trp_sim::weapons::quick_draw::QuickDraw::read(&game.db.config);
    let level_of = |weapon: &HandsRig| skills.as_deref().filter(|s| s.on()).zip(weapon.class).map_or(0, |(s, class)| s.levels.level(class));
    let pistol = |id: Option<Id>| {
        id.and_then(|id| carried.equipment.item.find(id)).and_then(|item| game.db.items.get(&item.kind.template)).and_then(|t| t.props.get("weapClass")).and_then(|c| c.as_str())
            == Some("pistol")
    };
    // What the keys and the screen ask (the frame's keys not heard again
    // where `hands_first` heard them before the step).
    let keys = !hands.heard;
    let (asked_for, use_ready) = hands_asked(&mut hands, &slots, controller, (&options, &features, &bindings), weapon, stowed.as_deref(), preparing.as_deref_mut(), (keys, in_move, frame_count.0), (quick, quick_knife, dropped, bench_take));
    let logged = crate::player::controller::hands_log().then_some((hands.asked_at, frame_count.0));
    let (use_part, use_amount) = (hands.use_part, hands.use_amount);
    let said = controller.take_hands_events();
    let (gone, appeared) = (said.contains(&FirearmEvent::Hidden), said.contains(&FirearmEvent::Appeared));
    // A quick put-away begun as what was under way let it
    // (`Remove.Start`): the arms pay now, by the skill of the weapon going.
    if said.contains(&FirearmEvent::FastSwitch) {
        let now = controller.movement.frame_clock();
        controller.movement.stamina.fast_switch(level_of(weapon), now);
        hands.quickdraw = Some(now);
        held.quickdraws = held.quickdraws.wrapping_add(1);
    }
    // (One begun below, for the battle bar's glow: `HandsHeld::quickdraws`.)
    let mut quickdraw_begun = false;

    let mut show = |entity: Option<Entity>, on: bool| {
        if let Some(mut shown) = entity.and_then(|entity| shown.get_mut(entity).ok()) {
            *shown = if on { Visibility::Inherited } else { Visibility::Hidden };
        }
    };
    // The takes (`change_hands`, `empty_hands_in`, `item_into_hands`,
    // `weapon_into_hands`), each change noted for a run that writes them.
    let empty_handed = |show: &mut dyn FnMut(Option<Entity>, bool), rig: &mut WeaponRig, weapon: &mut HandsRig, controller: &mut Controller, stowed: Option<&mut Stowed>| empty_hands_in(show, (rig, weapon, controller), stowed, logged);
    let quick_stroke = hands.use_quick;
    let take_item = |show: &mut dyn FnMut(Option<Entity>, bool), rig: &mut WeaponRig, weapon: &mut HandsRig, controller: &mut Controller, stowed: Option<&mut Stowed>, id: Id, asked: (Option<trp_sim::condition::health::Part>, Option<f32>)| item_into_hands(carried, quick_stroke, logged, show, (rig, weapon, controller), stowed, id, asked);
    let take = |show: &mut dyn FnMut(Option<Entity>, bool), rig: &mut WeaponRig, weapon: &mut HandsRig, controller: &mut Controller, stowed: Option<&mut Stowed>, kind: (Option<String>, bool), held: (bool, MagazineHeld, ChambersHeld)| weapon_into_hands(logged, show, (rig, weapon, controller), stowed, kind, held);
    let (using, armed) = (hands.using, controller.armed);
    let turn = hands_turn::Turn {
        hands: &mut hands,
        rig: &mut *rig,
        weapon: &mut *weapon,
        controller: &mut *controller,
        stowed: stowed.as_mut().map(|stowed| stowed.reborrow()),
        show: &mut show,
        hangs: &hangs,
        kind_of: &kind_of,
        folded_of: &folded_of,
        combo_of: &combo_of,
        chambers_of: &chambers_of,
        magazine_of: &magazine_of,
        slot_of: &slot_of,
        first_but: &first_but,
        level_of: &level_of,
        pistol: &pistol,
        empty_handed: &empty_handed,
        take_item: &take_item,
        take: &take,
        quick_draw,
        asked_for,
        use_ready,
        use_part,
        use_amount,
        gone,
        open,
        bench_put_away,
        quickdraw_begun: &mut quickdraw_begun,
    };
    if first {
        hands_turn::take_first(turn);
    } else if let Some(using) = using {
        hands_turn::item_in_use(turn, using);
    } else if armed {
        hands_turn::armed(turn);
    } else {
        // Empty hands: what was asked is taken at once (`take_into_empty`;
        // before the step as a rule, `hands_first`, here what was asked
        // after it).
        take_into_empty(&mut hands, &slots, &mut show, (&mut *rig, &mut *weapon, &mut *controller), stowed.as_deref_mut(), (asked_for, use_ready), (bench_take, bench_rack), logged);
    }
    told::told(told::Told {
        hands: &mut hands,
        held: held.reborrow(),
        asked: asked.reborrow(),
        blinking: blinking.reborrow(),
        carried,
        features: &features,
        controller,
        weapon,
        stowed: stowed.as_deref(),
        frame_count: &frame_count,
        first,
        drained,
        open,
        appeared,
        quickdraw_begun,
    });
}
