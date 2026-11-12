//! The movement's frame, and what it did told on: blind fire, the mount and its sounds, health, the range's things.

use super::*;

/// The movement stepped by the frame's input, then blind fire's and the mount's outcome told to the weapon,
/// health followed, and the range's doors and containers moved.
pub(super) fn after_movement(controller: &mut Controller, hands: &mut Option<ResMut<HandsRig>>, range: &mut Range, frame: trp_sim::base::pacing::Frame, input: &SimInput) {
    // The pacer's frames are the game's drawn frames: the player's
    // own whole-millisecond clock runs on them.
    controller.movement.frame(frame.dt, &input, &*range);
    // What blind fire's gate took goes to the weapon and its motion, on
    // this frame (`trp_sim::blind_fire::tell`).
    match hands.as_deref_mut() {
        Some(rig) => trp_sim::weapons::blind_fire::tell(&mut controller.movement, rig.firearm.as_mut(), Some(&mut rig.hands)),
        None => _ = controller.movement.take_blind_fire(),
    }
    // The weapon's mount sound comes on the first frame its timer has run
    // out on, the frame of the mount not counted (`WaitForSeconds`, from
    // the mount's frame: recorded 0.4584 s after a mount of 0.4506 s).
    if let Some(left) = controller.mount_sound_in.as_mut() {
        *left -= frame.dt;
        if *left <= 0.0 {
            controller.mount_sound_in = None;
            controller.mount_sounds += 1;
            // The bipod's on the ground on the same frame (recorded:
            // `bipod_stand_on_1` and the weapon's beside it), after a
            // forward mount only.
            if controller.mount_forward {
                controller.bipod_grounds += 1;
            }
        }
    }
    // What the mount did goes to the weapon and its motion
    // (`trp_sim::mounting::tell`), and sounds: the kit's medium rattle at
    // the mount, the exit's start and its end; the weapon's mount sound
    // the approach's length after the mount, unless an exit comes first.
    let mounted = match hands.as_deref_mut() {
        Some(rig) => trp_sim::weapons::mounting::tell(&mut controller.movement, rig.firearm.as_mut(), Some(&mut rig.hands), rig.mount_ergonomics),
        None => trp_sim::weapons::mounting::tell(&mut controller.movement, None, None, [0.0; 2]),
    };
    for event in mounted {
        use trp_sim::weapons::mounting::MountEvent;
        info!("the mount: {event:?} (the search's root {:?}, the feet {:?})", controller.movement.grid_root(), controller.movement.position);
        let alive = controller.health.as_ref().is_none_or(|health| health.alive);
        match event {
            MountEvent::Entered { approach, side, .. } => {
                controller.mount_forward = side == trp_sim::weapons::mounting::Side::Forward;
                if alive {
                    let overweight = controller.movement.stamina.settings.overweight;
                    controller.movement.sounds.kit_medium(overweight);
                }
                controller.mount_sound_in = Some(approach);
            }
            MountEvent::Leaving { .. } | MountEvent::Left { .. } => {
                if alive {
                    let overweight = controller.movement.stamina.settings.overweight;
                    controller.movement.sounds.kit_medium(overweight);
                }
                controller.mount_sound_in = None;
            }
            MountEvent::Refused => controller.mount_refusals = controller.mount_refusals.wrapping_add(1),
        }
    }
    // Then health, which takes what the movement did and tells it what
    // the character may do, on the player's whole-millisecond clock as
    // the game's is (health spec, "Compared with what").
    if let Some(health) = controller.health.as_mut() {
        // The weight's two notices as the weight now stands
        // (`Physical.EncumberedChanged`, `OverEncumberedChanged`): told on
        // a change alone.
        let (encumbered, over) = controller.movement.encumbered();
        health.set_encumbered(encumbered, over);
        let player_dt = controller.movement.player_dt();
        health.follow(player_dt, &mut controller.movement);
        // What it did goes to an item at work in the hands (a part
        // done: the next, or finished), and what an item holds now, or
        // that it is used up, to the inventory.
        for event in health.take_events() {
            if let Some(meds) = hands.as_deref_mut().and_then(|hands| hands.meds.as_mut()) {
                meds.on_health(health, &event);
            }
            if let Some(heard) = controller.voice_heard.as_mut() {
                heard.push(event.clone());
            }
            if !matches!(event, trp_sim::condition::health::Event::Resource(..) | trp_sim::condition::health::Event::UsedUp(..)) {
                controller.happened.push(crate::player::experience::Happened::Health(event.clone()));
            }
            {
                use trp_sim::condition::health::{Event, Kind};
                match event {
                    Event::Damaged(..) | Event::Started(Kind::ChronicFatigue | Kind::Encumbered | Kind::OverEncumbered | Kind::ImmunityPrevented, _) => controller.health_pulls[0] = controller.health_pulls[0].wrapping_add(1),
                    Event::Removed(..) => controller.health_pulls[1] = controller.health_pulls[1].wrapping_add(1),
                    Event::Started(Kind::MedEffect, _) => controller.meds_started = controller.meds_started.wrapping_add(1),
                    _ => {}
                }
            }
            if let trp_sim::condition::health::Event::Damaged(part, _, kind @ (trp_sim::condition::health::DamageKind::LightBleeding | trp_sim::condition::health::DamageKind::HeavyBleeding)) = event {
                controller.bled.push((part, kind == trp_sim::condition::health::DamageKind::HeavyBleeding));
            }
            if let Some(on) = trp_sim::combat::impacts::bleeding_change(&event) {
                controller.bleeding_drops.push(on);
            }
            if matches!(event, trp_sim::condition::health::Event::Resource(..) | trp_sim::condition::health::Event::UsedUp(..)) {
                controller.items_said.push(event);
            }
        }
    }
    // The doors, containers and trunk move after the character, as the
    // game runs their routines after its update.
    range.step_things(frame.dt);
}
