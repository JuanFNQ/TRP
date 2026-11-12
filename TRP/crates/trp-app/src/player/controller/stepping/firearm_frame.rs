//! The firearm's frame: its trigger and keys, then its logic stepped and what it did told on.

use super::*;

/// The trigger, the fire mode, the checks, the sights' and zeroing keys, the zoom and the fold.
#[allow(clippy::too_many_arguments)]
pub(super) fn firearm_keys(keys: &Keys, hands: &mut HandsRig, controller: &mut Controller, pending: &Pending, view: &View, frame: trp_sim::base::pacing::Frame, outside: bool) {
    let Keys { scenario, scripted, before, armed, handled, can_fire, since, passing, script_pressed, script_held, has, .. } = *keys;
    if let Some(firearm) = &mut hands.firearm {
        let trigger = match (scenario, scripted) {
            // A script names the fire mode it wants.
            (Some(_), _) => {
                let wanted = [("auto", FireMode::FullAuto), ("single", FireMode::Single), ("semiauto", FireMode::SemiAuto)];
                for (_, mode) in wanted.into_iter().filter(|(name, _)| script_pressed(name)) {
                    firearm.set_fire_mode(mode);
                }
                if script_pressed("check_chamber") && armed && handled && look_into_chamber(firearm) {
                    controller.chamber_checked = true;
                }
                script_held("fire")
            }
            // A scripted run with `auto` first switches the fire
            // mode, then presses the trigger two seconds in; with
            // `fire<seconds>` it presses it from then on (a
            // launcher's shot after its switch, say).
            (None, Some(script)) => {
                if has("auto") && before == 0.0 {
                    firearm.next_fire_mode();
                }
                let fire_from = script.split('+').find_map(|key| key.strip_prefix("fire")?.parse::<f32>().ok());
                (has("fire") && (!has("auto") || since(2.0))) || fire_from.is_some_and(since)
            }
            (None, None) => view.played() && controller.trigger && !controller.edit_build,
        };
        if handled {
            firearm.set_trigger(trigger && armed, can_fire);
        } else {
            firearm.note_trigger(trigger && armed);
        }
        if armed && handled && pending.fire_mode {
            firearm.next_fire_mode();
        }
        // The magazine is known, and its count named in the
        // ammunition's panel, only where the look began and in first
        // person (`Idling.CheckAmmo`).
        // The check stops the hands' gestures first, out of blind
        // fire, and is refused while an operation still plays
        // (`FirearmController.CheckAmmo`).
        let check = armed && handled && (pending.check_magazine || script_pressed("check_magazine"));
        if check && controller.movement.blind_fire() == 0 {
            controller.movement.force_stop_interactions();
        }
        if check && !controller.movement.interaction_playing() && firearm.check_magazine() {
            controller.magazine_checked |= !outside;
            controller.magazine_looked = true;
        }
        if armed && handled && pending.check_chamber && look_into_chamber(firearm) {
            controller.chamber_checked = true;
        }
        // The fullest way of firing, the look at the selector and
        // the look all over, each the idling operation's to take.
        if armed && handled && pending.force_auto {
            firearm.force_auto_fire_mode();
        }
        if armed && handled && pending.check_fire_mode {
            firearm.check_fire_mode();
        }
        if armed && handled && pending.examine {
            firearm.examine();
        }
        // The sights' keys: the next sight, and the next mode of
        // the scope aimed by, which the hands' animator is told of
        // (`ChangeScope`, `ChangeScopeMagnification`). Blind fire
        // holds back the first (`ChangeAimingMode`); the left hand's
        // items and mounting, which do too, are not built.
        if armed && handled && pending.change_scope && !firearm.blind_fire() {
            hands.hands.change_aim_scope(controller.movement.mounted());
        }
        if armed && handled && pending.magnification && hands.hands.change_magnification() {
            firearm.mod_switch();
        }
        // The zeroing keys, while aiming; an underbarrel launcher in
        // use steps its range instead, aimed or not (its panel told
        // by its event: `LauncherEvent::Range`).
        for (asked, up) in [(pending.zeroing_up, true), (pending.zeroing_down, false)] {
            if !(armed && handled && asked) {
                continue;
            }
            if firearm.in_launcher_mode() {
                firearm.launcher_range_step(up);
            } else {
                hands.hands.step_zeroing(up);
            }
        }
        // What the sights' keys (and a mount's straight sight) told
        // the battle screen, heard with the keys, before the frame's
        // fades step (`SetScopeMode`, `ChangeAimingMode`, a variable
        // scope's snap); the zoom's edges and the zeroing come later
        // (`sight_panels::follow_sights`).
        controller.panel_calls.extend(hands.hands.take_sight_hud().into_iter().map(crate::panels::sight_panels::PanelCall::Sight));
        // A variable scope's zoom, once a frame by the frame's time
        // (`ScopeZoomHandler`), with the wheel's turns since the last.
        hands.hands.zoom_frame(if handled { pending.zoom } else { 0.0 }, frame.dt);
        // A scripted run with `fold` folds the stock a second and a
        // half in.
        if armed && handled && (pending.fold_stock || (has("fold") && passing(1.5))) && firearm.fold_stock() {
            controller.folds_begun += 1;
        }
    }
}

/// The firearm's logic stepped on the player's clock, and each of its events told to the hands, the movement,
/// the inventory, the sounds and the panels.
#[allow(clippy::too_many_arguments)]
pub(super) fn firearm_events(hands: &mut HandsRig, controller: &mut Controller, features: &crate::screens::options_view::Features, frame: trp_sim::base::pacing::Frame, now: f32, outside: bool, aiming: bool) {
    if let Some(firearm) = &mut hands.firearm {
        firearm.set_aim_angle(controller.movement.pitch);
        // How its animations are drawn: the options' switch.
        firearm.set_drawn_smooth(features.smooth_animations);
        // Its operations on the player's clock, its animator on the
        // engine's (`Firearm::step_on_clock`).
        let player_dt = controller.movement.player_dt();
        // (A cylinder's or barrels' cases stay at their ports until
        // a reload throws them: `FirearmEvent::CaseThrown`.)
        let one_chamber = firearm.settings.cylinder.is_none() && firearm.settings.barrels == 0;
        let cylinder = firearm.settings.cylinder.is_some();
        let automatic = matches!(firearm.mode(), FireMode::FullAuto | FireMode::Burst);
        // Its heat by the raid's clock, this frame's.
        firearm.set_wear_clock(f64::from(now));
        // A draw that ends in this step is told the screen, then
        // comes up aimed where the player's two flags say so, as far
        // as the movement lets it (the inventory's `settle` leaves
        // the firearm's appearing to this).
        firearm.set_appearing(controller.inventory_open, controller.fast_slot && controller.aim_kept, controller.movement.can_aim());
        let launcher_sight = firearm.launcher_bones().and_then(|bones| bones.sight);
        for event in firearm.step_on_clock(frame.dt, player_dt) {
            match event {
                FirearmEvent::Shot { strength, hammer_down } => {
                    hands.hands.shoot(strength);
                    controller.shots_made += 1;
                    controller.shots_fired = controller.shots_fired.wrapping_add(1);
                    controller.bullet_calls.push(crate::weapons::bullets::BulletCall { automatic, hammer_down });
                    if one_chamber {
                        controller.case_calls.push(crate::weapons::cases::CaseCall::Shot);
                    }
                }
                FirearmEvent::ShellEject if one_chamber => controller.case_calls.push(crate::weapons::cases::CaseCall::Eject),
                // A cylinder's or barrels' rounds and cases as the
                // model holds them (more-weapons spec, "The rounds
                // and cases seen in chambers of their own").
                FirearmEvent::CaseThrown { chamber, share } => controller.case_calls.push(crate::weapons::cases::CaseCall::Thrown { chamber, share }),
                FirearmEvent::RoundShown { chamber, nth } => controller.rounds_shown.push((chamber, nth)),
                FirearmEvent::CaseLeft { chamber } => controller.cases_left.push(chamber),
                FirearmEvent::AutoFire(on) => hands.hands.set_auto_fire(on),
                // A change of fire mode updates the weapon's
                // attributes, which puts the recoil's rotation
                // spring back to nought (`Hands::weapon_parameters_changed`).
                FirearmEvent::FireMode(_) => hands.hands.weapon_parameters_changed(),
                // A firearm ready is told the screen's state again
                // (`Movement::hands_ready`).
                // Drawn with a malfunction, the quick prompt's
                // jammed or broken weapon for 5 s (`SpawnOperation.
                // WeaponAppeared`).
                FirearmEvent::Appeared => {
                    controller.movement.hands_ready(true);
                    if let Some(trigger) = crate::player::gestures::phrase::of_hands_event(&event, firearm.wear().map(|wear| &wear.state)) {
                        controller.phrase_situations.push(trigger);
                    }
                    controller.hands_events.push(event);
                }
                FirearmEvent::Hidden => controller.hands_events.push(event),
                FirearmEvent::AimKept(up) => controller.aim_kept = up,
                // The body's own part in a weapon drawn or put
                // away: its animator is told whoever is looking.
                FirearmEvent::BodyAction(action) => controller.movement.body.set_weapon_action(action),
                // And how fast that part plays: the weapon skill's draw speed.
                FirearmEvent::BodyDrawSpeed(speed) => controller.movement.body.set_draw_speed(speed),
                // An operation that ends blind fire has begun.
                FirearmEvent::BlindFireEnded => trp_sim::weapons::blind_fire::ended(&mut controller.movement, Some(&mut hands.hands)),
                // And one that ends a mount (mounting spec, "What ends it").
                FirearmEvent::LeaveMount { at_once } => controller.movement.leave_mount(at_once),
                // And what the left shoulder is told of the hands.
                FirearmEvent::LeftStance(call) => controller.movement.left_stance_call(call),
                // The rounds a tube took: the inventory takes
                // them out of the pockets and the vest.
                FirearmEvent::RoundsTaken(count) => {
                    controller.rounds_taken += count;
                    // (The reload's skill actions are the hands' own
                    // `FirearmEvent::Earn`, each where the game
                    // raises it.)
                    // A cylinder's reload shut: its chambers known
                    // (`ReloadCylinderMagazine`: `CheckChamber` after
                    // its commit).
                    controller.chamber_checked |= cylinder;
                }
                // A chambered round thrown out by an opened bolt:
                // the inventory puts it on the ground.
                FirearmEvent::RoundEjected => controller.rounds_ejected += 1,
                // The chamber's key working the action: a round of
                // the magazine to take the old one's place is the
                // class's chamber action (`RechamberOperation.Start`:
                // `WeaponChamberAction`), and the old one is thrown
                // out.
                FirearmEvent::RackBegun { feeds } => {
                    if feeds {
                        controller.happened.push(crate::player::experience::Happened::Chamber);
                    }
                }
                FirearmEvent::RoundRacked => controller.rounds_racked += 1,
                FirearmEvent::RoundUnloaded => controller.rounds_unloaded += 1,
                FirearmEvent::RoundDropped { chamber } => controller.rounds_dropped.push(chamber),
                // A magazine's exchange begun, and the old one out
                // of the hand: the inventory makes the one and
                // lets the magazine fall at the other.
                FirearmEvent::MagazineSwap { left, quick } => {
                    controller.swaps.push((left, quick));
                    controller.happened.push(crate::player::experience::Happened::Reload);
                    // The reload's own phrase, unasked: the quick
                    // prompt's for 5 s (`Player.Say(OnWeaponReload)`).
                    controller.phrase_situations.push(crate::player::gestures::phrase::ON_WEAPON_RELOAD);
                }
                // A round chambered by the hands from the inventory:
                // the class's chamber action (`WeaponChamberAction`).
                // A skill's action the hands raised: a reload's, a
                // chamber's (a round in or out for the inventory, a
                // tube's empty chamber), Mag Drills' round loaded
                // (`FirearmEvent::Earn`; skills spec, "How a skill
                // is earned"). The rack's is `RackBegun`'s, a
                // magazine's reload `MagazineSwap`'s.
                FirearmEvent::Earn(earn) => controller.happened.push(crate::player::experience::Happened::of_earn(earn)),
                FirearmEvent::MagazineHidden => {
                    controller.magazines_hidden += 1;
                    controller.magazine_drawn = Some(false);
                }
                FirearmEvent::MagazineShown | FirearmEvent::MagazineIn => controller.magazine_drawn = Some(true),
                FirearmEvent::MagazineMoved => controller.magazines_moved += 1,
                FirearmEvent::ChamberGivenUp => controller.chamber_given_up = true,
                // Its malfunctions, for the item, the words on the
                // screen and the round thrown out (`crate::wear`).
                FirearmEvent::Malfunctioned(_)
                | FirearmEvent::MalfunctionReminder { .. }
                | FirearmEvent::MalfunctionExamined { .. }
                | FirearmEvent::MalfunctionRepairBegun(_)
                | FirearmEvent::MalfunctionRepaired(_)
                | FirearmEvent::MalfunctionCleared(_)
                | FirearmEvent::FedRoundThrown
                | FirearmEvent::MisfiredRoundThrown
                | FirearmEvent::Autoshot => {
                    // A malfunction's repair begun, or a misfire put
                    // right at once: Troubleshooting's
                    // (`RepairMalfunction.Start`, `FirearmOperation`).
                    if matches!(event, FirearmEvent::MalfunctionRepairBegun(_) | FirearmEvent::MalfunctionCleared(_)) {
                        controller.happened.push(crate::player::experience::Happened::Act(trp_sim::condition::earning::Act::Malfunction));
                    }
                    // A malfunction made known at the shot or
                    // reminded of (not looked at): the quick
                    // prompt's jammed (a slide's, its kind known) or
                    // broken weapon for 5 s (`NeedRepairMalfPhraseSituation`).
                    if let Some(trigger) = crate::player::gestures::phrase::of_hands_event(&event, firearm.wear().map(|wear| &wear.state)) {
                        controller.phrase_situations.push(trigger);
                    }
                    controller.wear_events.push(event);
                }
                // A magazine whose top round the chamber refuses,
                // at a press: the notice (`crate::wear`).
                FirearmEvent::IncompatibleAmmo => controller.wear_events.push(event),
                // A way of firing named in the ammunition's panel,
                // to the character's own eyes (`ShowFireMode`).
                FirearmEvent::FireModeShown(mode) if !outside => controller.panel_calls.push(crate::panels::sight_panels::PanelCall::FireMode(mode)),
                // And a look at the magazine's count (`ShowAmmoDetails`).
                FirearmEvent::MagazineChecked if !outside => controller.panel_calls.push(crate::panels::sight_panels::PanelCall::Magazine),
                // The stock folded or unfolded: the recoil and
                // the hold go by the build as it now stands, and
                // the body's animator by its new width.
                FirearmEvent::Folded(folded) => {
                    hands.hands.set_folded(folded, aiming);
                    hands.hands.weapon_parameters_changed();
                    let width = if folded { hands.folded_width.unwrap_or(hands.width) } else { hands.width };
                    controller.movement.body.set_weapon_width(width);
                }
                // A mark of a sound: its rig's sound player plays it.
                FirearmEvent::Sound(name) => controller.hands_sounds.push(name),
                // And one of an underbarrel launcher's animator: the
                // launcher's own player plays it.
                FirearmEvent::LauncherSound(name) => controller.launcher_sounds.push(name),
                // The bipod opened or folded by the hands' clip:
                // the bipod's own sound player plays it.
                FirearmEvent::BipodSound(open) => controller.bipod_marks.push(open),
                // A shot's sound, and the trigger let go, which
                // ends an automatic weapon's loop.
                FirearmEvent::FireSound { trigger, doublet } => controller.shot_calls.push(crate::audio::shot_sound::ShotCall::Fire { trigger, doublet }),
                FirearmEvent::TriggerReleased => controller.shot_calls.push(crate::audio::shot_sound::ShotCall::Release),
                // The sights brought up or down by the aim's call:
                // the kit rattles at 0.2, alive (`SetAim`,
                // `PlayGearSound`), and coming up the weapon's
                // aiming sound plays.
                FirearmEvent::Aimed(up) => {
                    if controller.health.as_ref().is_none_or(|health| health.alive) {
                        let overweight = controller.movement.stamina.settings.overweight;
                        controller.movement.sounds.kit(AIM_KIT, overweight);
                    }
                    if up {
                        controller.aims_up += 1;
                    }
                    controller.happened.push(crate::player::experience::Happened::Aimed(up));
                }
                // The underbarrel launcher (underbarrel-launcher spec).
                FirearmEvent::Launcher(event) => {
                    use trp_sim::weapons::firearm::LauncherEvent as L;
                    match event {
                        // Its sight the only one from the press going
                        // in, the rifle's again at the end coming back
                        // (`IsGrenadeLauncher`).
                        L::SwitchBegun(true) => hands.hands.set_grenade_sight(launcher_sight),
                        L::SwitchBegun(false) => {}
                        // The handling of the side it ended on
                        // (`InitWeaponData`).
                        L::Switched(on) => {
                            if let Some(handling) = hands.launcher_handling.clone() {
                                hands.hands.take_handling(&handling[usize::from(on)]);
                            }
                            if !on {
                                hands.hands.set_grenade_sight(None);
                            }
                        }
                        // The round gone from its item; its shot's
                        // sound.
                        L::Shot { .. } => {
                            controller.launcher_fired = true;
                            controller.launcher_shots_fired = controller.launcher_shots_fired.wrapping_add(1);
                            controller.launcher_shot_calls.push(crate::audio::shot_sound::ShotCall::Fire { trigger: true, doublet: false });
                        }
                        // The recoil a frame on, of the strength its
                        // round gives (1 and its `ammoRec` a hundredth).
                        L::Recoil => hands.hands.shoot(controller.launcher_recoil),
                        L::RoundTaken => controller.launcher_round_taken = true,
                        // Its spent case thrown from its own port
                        // (`cases`).
                        L::CaseThrown(share) => controller.case_calls.push(crate::weapons::cases::CaseCall::LauncherThrown(share)),
                        // The zeroing panel shows the range set, to the
                        // character's own eyes.
                        L::Range(range) if !outside => controller.panel_calls.push(crate::panels::sight_panels::PanelCall::Sight(trp_sim::weapons::sights::SightHud::Zeroing(range))),
                        L::NoRounds => controller.phrase_situations.push(crate::player::gestures::phrase::NEED_AMMO),
                        _ => {}
                    }
                }
                // Shells and the magazine's model are not in yet.
                _ => {}
            }
        }
    }
}
