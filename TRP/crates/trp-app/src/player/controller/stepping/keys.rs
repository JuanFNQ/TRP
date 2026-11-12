//! The frame's keys as a script or a played run gives them, and the weapon's keys heard before the body moves.

use super::*;

/// The frame's keys as its phases read them: a script's or a scripted run's presses, and what may be heard.
#[derive(Clone, Copy)]
pub(super) struct Keys<'a> {
    pub(super) scenario: Option<&'a trp_sim::recordings::script::Scenario>,
    pub(super) scripted: Option<&'a str>,
    pub(super) before: f32,
    pub(super) played: bool,
    pub(super) heard: bool,
    pub(super) handled: bool,
    pub(super) armed: bool,
    pub(super) can_aim: bool,
    pub(super) can_fire: bool,
    pub(super) aim_key: bool,
    pub(super) since: &'a dyn Fn(f32) -> bool,
    pub(super) passing: &'a dyn Fn(f32) -> bool,
    pub(super) script_held: &'a dyn Fn(&str) -> bool,
    pub(super) script_pressed: &'a dyn Fn(&str) -> bool,
    pub(super) held: &'a dyn Fn(&str) -> bool,
    pub(super) has: &'a dyn Fn(&str) -> bool,
    pub(super) axis: &'a dyn Fn(bool, bool) -> f32,
}

/// The weapon in hand's mount figures, the bipod's key, the rangefinder's aim, and the firearm's aim and reload keys.
pub(super) fn weapon_keys(keys: &Keys, controller: &mut Controller, hands: &mut Option<ResMut<HandsRig>>, pending: &Pending, features: &crate::screens::options_view::Features) {
    let Keys { scenario, scripted, played, heard, handled, armed, can_aim, aim_key, since, passing, script_pressed, script_held, has, .. } = *keys;
    // The weapon in hand's mount figures, and its bipod, for the
    // movement's mount (`Movement::set_mount_weapon`).
    match hands.as_deref() {
        Some(rig) => controller.movement.set_mount_weapon(rig.firearm.as_ref().map(|_| rig.hands.settings.mount), rig.firearm.as_ref().and_then(|f| f.bipod()).unwrap_or(false)),
        None => controller.movement.set_mount_weapon(None, false),
    }
    // The bipod's key (`FirearmController.ToggleBipod`), with its
    // switch on: taken, the body's gesture is asked and the weapon's
    // motion and the movement are told at once (`ToggleBipodOperation.
    // Start`).
    if let Some(rig) = hands.as_deref_mut() {
        let asked = pending.bipod || script_pressed("cmd:ToggleBipods");
        if let Some(firearm) = rig.firearm.as_mut().filter(|_| asked && armed && handled && features.mounting) {
            let m = &controller.movement;
            if firearm.toggle_bipod(m.mounted(), m.sprinting, m.gesture_free()) {
                if let (Some(out), Some(forward)) = (firearm.bipod(), firearm.settings.bipod) {
                    controller.movement.bipod_gesture(out, forward);
                    rig.hands.set_bipod(out);
                    controller.movement.set_mount_weapon(Some(rig.hands.settings.mount), out);
                }
            }
        }
    }
    // The handheld rangefinder's aim, by the same key and the same rule
    // (`UsableItemInputTranslator`: `ToggleAim`, and let go where held;
    // rangefinder spec), whatever is armed.
    if let Some(rangefinder) = hands.as_deref_mut().and_then(|h| h.rangefinder.as_mut()) {
        if played {
            for given in pending.aim.iter().filter(|_| heard) {
                rangefinder.set_aim(*given && !rangefinder.aiming(), can_aim);
            }
        } else {
            let wanted = match (scenario, scripted) {
                (Some(_), _) => script_held("aim"),
                (None, Some(_)) => has("aim") && since(0.5),
                (None, None) => false,
            };
            if std::mem::replace(&mut controller.aim_key, wanted) != wanted && heard {
                rangefinder.set_aim(wanted, can_aim);
            }
        }
    }
    if let Some(firearm) = hands.as_deref_mut().and_then(|h| h.firearm.as_mut()) {
        if played {
            // The aim's binding given puts the sights the other way
            // (`ToggleAim`); taken back, where it is bound to be
            // held, it puts them down.
            for given in pending.aim.iter().filter(|_| armed && heard) {
                firearm.set_aim(*given && !firearm.aiming(), can_aim);
            }
        } else if std::mem::replace(&mut controller.aim_key, aim_key) != aim_key && heard {
            firearm.set_aim(aim_key, can_aim);
        }
        // The reload key goes to the weapon before the body moves:
        // the body's animator is told of a reload on the frame it
        // starts. A scripted run with `reload` starts one two seconds
        // in, or three with `prone`, when the character is down.
        let at = if has("prone") { 3.0 } else { 2.0 };
        // The magazine picker's choice is the plain reload with what it
        // chose (`SwitchMagazine`: `ReloadMag`, `ReloadWithAmmo`, ...).
        let picked = controller.picked.take();
        let plain = pending.reload || (has("reload") && passing(at)) || script_pressed("reload") || picked.is_some();
        // Stunned, a reload by the key is the quick one
        // (`ShouldForceQuickReload`: a stun on the head; grenade spec,
        // "A flash"), not the picker's, which calls the hands itself.
        let stunned = picked.is_none() && controller.health.as_ref().is_some_and(|health| health.active(trp_sim::condition::health::Kind::Stun, Some(trp_sim::condition::health::Part::Head)).is_some());
        let quick = (pending.quick_reload && !plain) || (plain && stunned);
        let reload_key = plain;
        let plain = plain && !stunned;
        // Where the magazine comes out (`Weapon.ReloadMode`
        // `ExternalMagazine`, or the Model 700's
        // `ExternalMagazineWithInternalReloadSupport`), or by the plain
        // key with none fitted, a magazine within reach is looked for
        // first; a weapon of a magazine of its own fills it a round at
        // a time where none is found (`FirearmHandsInputTranslator.
        // Reload`, `QuickReload`; bolt-action spec).
        let looks = if reload_key { firearm.reload_looks_for_magazine() } else { firearm.magazine_comes_out() };
        // A magazine the picker chose is the one; rounds it chose are a
        // reload of a magazine of its own from that stack alone.
        let found = match picked {
            Some(crate::screens::ammo_picker::Picked::Magazine(chosen)) => Some(chosen),
            Some(crate::screens::ammo_picker::Picked::Rounds(..)) => None,
            None => controller.next_magazine.filter(|_| looks && scenario.is_none()),
        };
        let in_reach = match picked {
            Some(crate::screens::ammo_picker::Picked::Rounds(_, count)) => count,
            _ => controller.rounds_in_reach,
        };
        if armed && handled && firearm.in_launcher_mode() {
            // An underbarrel launcher in use takes the plain key alone
            // (underbarrel-launcher spec, "Reloading and unloading"): the
            // rounds its chamber takes within reach, none the phrase for
            // no ammunition; then the rifle asked first, its magazine
            // known among the rest; the quick key finds nothing to do.
            if plain {
                let in_reach = controller.launcher_rounds_in_reach > 0;
                if in_reach && controller.magazine_unknown && scenario.is_none() {
                    controller.reload_refused_unknown = true;
                } else {
                    firearm.reload_launcher(in_reach);
                }
            }
        } else if armed && handled && (plain || quick) && controller.magazine_unknown && scenario.is_none() {
            // The weapon's own magazine not known: no reload of any
            // kind, the game's warning instead (`Player.CanStartReload`).
            controller.reload_refused_unknown = true;
        } else if armed && handled && (plain || quick) {
            if firearm.settings.internal_magazine && found.is_none() && !(looks && scenario.is_some()) {
                // A tube is filled from the rounds within reach, and
                // barrels are loaded, by either key; a cylinder is
                // reloaded the quick way by the quick key, at rest
                // (more-weapons spec, "A cylinder's quick reload, and
                // the fistful"). The plain key, stunned, is the plain
                // reload for all of them (`ReloadRevolverDrum`,
                // `ReloadWithAmmo` and `ReloadBarrels` take no quick
                // way from it).
                // (Heard as the magazine's search below: the quick key's
                // only at rest.)
                let heard = reload_key || firearm.animator_idling();
                if reload_key {
                    firearm.reload_shells(in_reach);
                } else {
                    firearm.quick_reload_shells(in_reach);
                }
                if looks && heard && scenario.is_none() && picked.is_none() {
                    // The magazine looked for first and none found (the
                    // Model 700's, or one of a magazine of its own with
                    // none fitted): `ReloadMagNotFound`'s NEED AMMO.
                    controller.phrase_situations.push(crate::player::gestures::phrase::NEED_AMMO);
                    // With no magazine to fill a round at a time
                    // (`ReloadWithAmmo` false), the plain key's last
                    // resort: a round into the chamber by hand
                    // (`LoadAmmoToChamber`).
                    if reload_key && firearm.magazine.is_none() {
                        controller.chamber_by_hand = true;
                    }
                }
            } else if scenario.is_some() {
                // A recorded scenario changes the magazine for a full
                // one, as its recording does.
                firearm.reload();
            } else if let Some((magazine, rounds, variant, external, unfit)) = found {
                // The fullest magazine within reach goes in; with
                // none, nothing is done (`ReloadMagNotFound`). The
                // inventory makes the exchange as the reload begins.
                // Its clips are its own (`magAnimationIndex`).
                // The plain reload stops the hands' gestures first, out
                // of blind fire, and is refused while an operation
                // still plays (`FirearmController.ReloadMag`: a take, a
                // backpack's drop); the quick one does neither
                // (`QuickReloadMag`).
                if !quick && controller.movement.blind_fire() == 0 {
                    controller.movement.force_stop_interactions();
                }
                if quick || !controller.movement.interaction_playing() {
                    controller.magazine_asked = Some(magazine);
                    firearm.next_magazine_variant(variant);
                    firearm.next_magazine_external(external);
                    firearm.next_magazine_unfit(unfit);
                    if quick {
                        firearm.quick_reload_with(rounds);
                    } else {
                        firearm.reload_with(rounds);
                    }
                }
            } else if reload_key || firearm.animator_idling() {
                // None found: the key still leaves a mount (the
                // quick one's only at rest), and a failure to feed
                // reminds.
                firearm.reload_none_found();
                // `ReloadMagNotFound`: the quick prompt's NEED AMMO.
                controller.phrase_situations.push(crate::player::gestures::phrase::NEED_AMMO);
                // The plain key's last resort (`Reload`, not
                // `QuickReload`): a round within reach into the empty
                // chamber, by the inventory (`LoadAmmoToChamber`).
                if reload_key && picked.is_none() {
                    controller.chamber_by_hand = true;
                }
            }
        }
    }
}
