//! The other things the hands may hold, stepped: empty hands, an item used, a grenade, the rangefinder and the knife.

use super::*;

/// What the knife's sweep and the hands' log read of the world around.
pub(super) struct Around<'a> {
    pub(super) features: &'a crate::screens::options_view::Features,
    pub(super) ragdoll: &'a crate::player::death::Ragdoll,
    pub(super) range: &'a Range,
    pub(super) dummies: Option<&'a crate::world::dummy::Dummies>,
    pub(super) skills: Option<&'a crate::player::skills::Skills>,
    pub(super) frame_count: &'a bevy::diagnostic::FrameCount,
}

/// Empty hands, an item used, a grenade, the rangefinder and the knife stepped with their keys, and a change
/// of the hands written as they are first stepped. A grenade let go of on this frame sets `thrown_now`.
pub(super) fn held_items(hands: &mut HandsRig, controller: &mut Controller, pending: &mut Pending, frame: trp_sim::base::pacing::Frame, heard: bool, around: Around, thrown_now: &mut bool) {
    let Around { features, ragdoll, range, dummies, skills, frame_count } = around;
    // Empty hands have a controller and an animator of their own.
    if let Some(empty) = &mut hands.empty {
        empty.set_drawn_smooth(features.smooth_animations);
        let said = empty.step(frame.dt, controller.movement.pitch);
        for event in &said {
            match event {
                FirearmEvent::BodyAction(action) => controller.movement.body.set_weapon_action(*action),
                FirearmEvent::Sound(name) => controller.hands_sounds.push(name.clone()),
                _ => {}
            }
        }
        // Empty hands ready are told the screen's state again
        // (`Movement::hands_ready`).
        if said.contains(&FirearmEvent::Appeared) {
            controller.movement.hands_ready(true);
        }
        controller.hands_events.extend(said.into_iter().filter(|event| matches!(event, FirearmEvent::Appeared | FirearmEvent::Hidden)));
    }
    // So has an item used, its work done by health.
    if let (Some(meds), Some(health)) = (&mut hands.meds, controller.health.as_mut()) {
        meds.set_drawn_smooth(features.smooth_animations);
        if pending.use_cancel {
            meds.cancel();
        }
        for event in meds.step(health, frame.dt, controller.movement.pitch) {
            match event {
                MedsEvent::BodyAction(action) => controller.movement.body.set_weapon_action(action),
                MedsEvent::Appeared => controller.hands_events.push(FirearmEvent::Appeared),
                MedsEvent::Hidden => controller.hands_events.push(FirearmEvent::Hidden),
                MedsEvent::Sound(name) => controller.hands_sounds.push(name),
            }
        }
    }
    // A grenade (grenades-and-knife spec, "The grenade's hands"):
    // its keys, then its animator; let go of, it is thrown
    // (`grenade::throw_out`) and gone from the inventory.
    if let Some(grenade) = &mut hands.grenade {
        grenade.set_drawn_smooth(features.smooth_animations);
        for (low, down) in pending.throw_keys.drain(..).filter(|_| heard) {
            if down {
                grenade.press(low);
                // The throw's own phrase, unasked
                // (`Player.Say(OnGrenade)`).
                controller.phrase_situations.push(crate::player::gestures::phrase::ON_GRENADE);
            } else {
                grenade.release(low);
            }
        }
        for event in grenade.step(frame.dt) {
            match event {
                GrenadeEvent::Appeared => controller.hands_events.push(FirearmEvent::Appeared),
                GrenadeEvent::Hidden => controller.hands_events.push(FirearmEvent::Hidden),
                GrenadeEvent::Thrown { low } => {
                    controller.grenade_thrown = true;
                    controller.throws.push(low);
                    *thrown_now = true;
                    controller.happened.push(crate::player::experience::Happened::Act(trp_sim::condition::earning::Act::Thrown));
                }
                GrenadeEvent::Used => {}
                // The fuse's sound is the thrown grenade's, played
                // where it leaves the hand (`PROVISIONAL`: the game
                // plays it at the grenade, 0.05 s on).
                GrenadeEvent::Sound(name) | GrenadeEvent::SoundAtPoint(name) => controller.hands_sounds.push(name),
                GrenadeEvent::BodyAction(action) => controller.movement.body.set_weapon_action(action),
            }
        }
    }
    // A knife (grenades-and-knife spec, "The knife's hands"): its
    // strokes, with the stamina for them; its blade's sweep is
    // `knife::sweep`'s.
    // The rangefinder (rangefinder spec): the examine key's look
    // over it, its animator and its readout's clock moved on.
    if let Some(rangefinder) = &mut hands.rangefinder {
        rangefinder.set_drawn_smooth(features.smooth_animations);
        if pending.examine && heard {
            rangefinder.look();
        }
        for event in rangefinder.step(frame.dt) {
            match event {
                RangefinderEvent::Appeared => controller.hands_events.push(FirearmEvent::Appeared),
                RangefinderEvent::Hidden => controller.hands_events.push(FirearmEvent::Hidden),
                RangefinderEvent::Sound(name) => controller.hands_sounds.push(name),
                RangefinderEvent::BodyAction(action) => controller.movement.body.set_weapon_action(action),
            }
        }
    }
    if let Some(knife) = &mut hands.knife {
        knife.set_drawn_smooth(features.smooth_animations);
        knife.set_melee_speed(controller.movement.stamina.melee_speed());
        for stroke in pending.strokes.drain(..).filter(|_| heard) {
            // Not in a movement state that takes no hand input, a sprint or a
            // crawl (`Idling.MakeKnifeKick`: `StateIsSuitableForHandInput`
            // first): the press is dropped whole, no stamina is asked of it
            // and nothing of it is kept for after.
            if !controller.movement.state_suits_hands() {
                continue;
            }
            let (can, speed) = (controller.movement.stamina.can_melee(), controller.movement.stamina.melee_speed());
            if !knife.strike(stroke, can, speed) && knife.idling() {
                info!("a stroke: too tired for it");
            }
        }
        // The blade's sweep (grenades-and-knife spec, "The knife's
        // hands"): while the stroke is live, before its clips move
        // on, from where the last frame left the blade, along this
        // frame's look (`PROVISIONAL`: no recording's look moved
        // between the two), through the stroke's world: the range as
        // the engine has it and the living dummies' hit colliders
        // where they were last placed, met in the engine's order
        // (`knife::StrokeWorld::meet`).
        match (knife.live(), controller.blade_left, hands.blade.as_ref()) {
            (true, Some((at, turn)), Some(blade)) => {
                let m = &controller.movement;
                let reach = match knife.stroke() {
                    trp_sim::Stroke::Slash => knife.settings().slash_reach,
                    trp_sim::Stroke::Stab => knife.settings().stab_reach,
                };
                let sweep = trp_sim::BladeSweep::of(at, turn, blade.centre, blade.size, trp_sim::held::knife_hands::look_way(m.yaw, m.pitch), reach);
                if controller.stroke_world.0.is_none() {
                    controller.stroke_world.0 = ragdoll.physics.as_ref().map(|physics| crate::weapons::knife::StrokeWorld::of_range(physics, &range));
                }
                let met = controller.stroke_world.0.as_mut().and_then(|world| {
                    world.set_targets(dummies.as_deref().map(crate::world::dummy::Dummies::blade_targets).unwrap_or_default());
                    world.meet(&sweep).map(|meeting| (meeting, world.deflects(&meeting.struck)))
                });
                if let Some((meeting, deflects)) = met {
                    // The blow (`ProcessHit`): the stroke's damage, cut
                    // with the hands exhausted, times one and
                    // Strength's melee buff; its way the swing's.
                    let strength = skills.as_ref().map_or(0.0, |skills| skills.buffs().strength_melee);
                    let stroke = knife.stroke();
                    let cut = controller.movement.stamina.melee_damage_cut();
                    if let Some(damage) = knife.hit().map(|damage| trp_sim::held::knife_hands::blow_damage(damage, cut, strength)) {
                        let direction = knife.hit_direction(at);
                        // What it struck plays its material's hit
                        // (`EffectsCommutator.PlayKnifeHitEffect`).
                        controller.knife_hits.push((meeting, knife.settings().shown_on_model));
                        let crate::weapons::knife::Meeting { distance, point, normal, struck } = meeting;
                        match struck {
                            crate::weapons::knife::Struck::Dummy(target) => {
                                info!("the knife's {stroke:?} met dummy {}'s collider {} at {point:?}, {distance:.3} m along its sweep: {damage} of damage, glancing off", target.which, target.index);
                                controller.blows.push(crate::weapons::knife::Blow { target, stroke, damage, penetration: knife.penetration(), burn_rate: knife.settings().stamina_burn_rate, direction, point });
                                // A blow on a body's collider: Strength's
                                // fistfight action.
                                controller.happened.push(crate::player::experience::Happened::Act(trp_sim::condition::earning::Act::Fistfight));
                            }
                            crate::weapons::knife::Struck::World { .. } => {
                                info!("the knife's {stroke:?} met the range at {point:?} (normal {normal:?}), {distance:.3} m along its sweep: {damage} of damage, glancing off");
                            }
                        }
                        // Off the deflecting layer, the knife's
                        // deflection's stamina besides.
                        if deflects {
                            let now = controller.movement.frame_clock();
                            controller.movement.stamina.melee(knife.settings().deflection_cost, now);
                        }
                    }
                }
            }
            (false, ..) => controller.stroke_world.0 = None,
            _ => {}
        }
        // A stroke's frame after its sweep (`Kick.Update`): the
        // swing's way eased toward the blade's step.
        if let Some((at, _)) = controller.blade_left {
            knife.follow_blade(at);
        }
        for event in knife.step(frame.dt) {
            match event {
                KnifeEvent::Appeared => controller.hands_events.push(FirearmEvent::Appeared),
                KnifeEvent::Hidden => controller.hands_events.push(FirearmEvent::Hidden),
                KnifeEvent::Live { cost, .. } => {
                    let now = controller.movement.frame_clock();
                    controller.movement.stamina.melee(cost, now);
                }
                KnifeEvent::Dead | KnifeEvent::Used => {}
                KnifeEvent::Sound(name) => controller.hands_sounds.push(name),
                // What the body plays of the strokes, seen from
                // outside (grenades-and-knife spec, "The knife's
                // strokes seen from outside").
                KnifeEvent::BodyAction(action) => controller.movement.body.set_weapon_action(action),
                KnifeEvent::BodySwingSpeed(speed) => controller.movement.body.set_swing_speed(speed),
                KnifeEvent::BodyDeflected(deflected) => controller.movement.body.set_deflected(deflected),
            }
        }
    }
    // A change of the hands written as they are first stepped
    // (`hands_log`): the animator and an item's effect as this
    // frame's health and hands have left them.
    if let Some(change) = controller.hands_change.take() {
        let animator = hands.animator().map(|animator| animator.state()).map_or_else(|| "-".to_owned(), |(state, at, next)| format!("{} {at:.5}", next.unwrap_or(state).trim_start_matches("Hands.")));
        let effect = controller.health.as_ref().and_then(|health| health.effects.iter().find(|effect| effect.kind == trp_sim::condition::health::Kind::MedEffect && effect.existing())).map_or_else(|| "-".to_owned(), |effect| format!("{:.4} s", effect.state_left()));
        let asked = change.asked.map_or_else(|| "-".to_owned(), |frame| frame.to_string());
        info!("hands: {} asked on frame {asked}, made on frame {}, first stepped on frame {} (animator {animator}, medicine's effect left of its state {effect})", change.what, change.made, frame_count.0);
    }
}
