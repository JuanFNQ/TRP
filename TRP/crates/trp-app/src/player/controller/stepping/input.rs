//! The frame's input to the movement: what the thing in hand says of itself, and the keys as the movement takes them.

use super::*;

/// What the thing in hand tells the movement, and the frame's input built from the keys; with whether it is aimed.
#[allow(clippy::too_many_arguments)]
pub(super) fn frame_input(keys: &Keys, controller: &mut Controller, hands: &Option<ResMut<HandsRig>>, pending: &Pending, view: &View, inventory_open: bool, [look, head]: [Vec2; 2], [delta_level, delta_speed]: [f32; 2]) -> (SimInput, bool) {
    let Keys { scenario, scripted, played, since, passing, script_pressed, script_held, held, has, axis, .. } = *keys;
    let firearm = hands.as_deref().and_then(|h| h.firearm.as_ref());
    // The rangefinder aimed slows the walk as the sights do
    // (`SetAimingSlowdown(IsAiming, 0.33)`) and costs the arms.
    let aiming_rangefinder = hands.as_deref().and_then(|h| h.rangefinder.as_ref()).is_some_and(|r| r.aiming());
    let (aiming, reloading) = (firearm.is_some_and(|f| f.aiming()) || aiming_rangefinder, firearm.is_some_and(|f| f.reloading()));
    let empty_hands = hands.as_deref().and_then(|h| h.empty.as_ref());
    // A grenade's draw and put-away tell the body the hands are busy, and
    // an item's hands leave the body's figure where the put-away before
    // them set it (1): they never lower it.
    let grenade_hands = hands.as_deref().and_then(|h| h.grenade.as_ref());
    let meds_hands = hands.as_deref().and_then(|h| h.meds.as_ref());
    // A knife's draw and put-away tell it too, so that a sprint brings the
    // knife into view as it does a firearm.
    let knife_hands = hands.as_deref().and_then(|h| h.knife.as_ref());
    let hands_busy = firearm.is_some_and(|f| f.hands_busy())
        || empty_hands.is_some_and(|empty| empty.hands_busy())
        || grenade_hands.is_some_and(|grenade| grenade.hands_busy())
        || knife_hands.is_some_and(|knife| knife.hands_busy())
        || meds_hands.is_some_and(|meds| meds.in_hand());
    // An item's hands and a grenade's do not pass the screen on to the
    // movement state (`OnInventory`).
    let inventory_unheard = meds_hands.is_some_and(|meds| meds.in_hand()) || grenade_hands.is_some_and(|grenade| grenade.in_hand());
    // Whether the animator of the thing in hand is at rest, which a
    // gesture of the hands waits for, and whether it is in its states
    // for an open inventory, which the body's own pose for one waits
    // for (empty hands have none, so their body never takes it).
    let hands_idle = firearm.is_some_and(|f| f.animator_idling()) || empty_hands.is_some_and(|empty| empty.animator_idling());
    let hands_in_inventory = firearm.is_some_and(|f| f.in_inventory());
    let weapon_occupied = firearm.is_some_and(|f| f.occupied());
    // Medicine or food in hand, or a grenade's throw held: a vault only
    // under the move's height without hands.
    let hands_limit_vault = hands.as_deref().is_some_and(|h| {
        h.meds.as_ref().is_some_and(|meds| meds.in_hand()) || h.grenade.as_ref().is_some_and(|grenade| grenade.holds_throw())
    });
    // Blind fire's commands, a played run's from its bindings and a
    // scripted run's as the recorder gives them (`cmd:`), and what the
    // thing in hand says to its gate.
    let blind_toggle = if script_pressed("cmd:ToggleBlindAbove") {
        1
    } else if script_pressed("cmd:ToggleBlindRight") {
        -1
    } else {
        pending.blind_toggle
    };
    let blind_end = pending.blind_end || script_pressed("cmd:BlindShootEnd");
    let (blind_firearm, blind_may_start) = (firearm.is_some(), firearm.is_some_and(|f| f.can_set_blind_fire()));
    let notch = |up: &str, down: &str| axis(script_pressed(up), script_pressed(down));
    // What a played run's keys push the movement axes to; none while
    // the keys fly the camera.
    let move_keys = if view.played() { controller.move_keys } else { [0.0; 2] };
    // The walk's cap with the sights up gains the weapon's mastering
    // (`AimMovementSpeed`); a rangefinder's is the plain one.
    controller.movement.aim_walk_bonus = firearm.map_or(0.0, |f| f.aim_walk_bonus());

    let input = SimInput {
        aiming,
        aim_kept_from_body: firearm.is_some_and(|f| !f.tells_body_of_aim()),
        reloading,
        hands_busy,
        hands_idle,
        hands_in_inventory,
        weapon_occupied,
        hands_limit_vault,
        free_look: false,
        // The inventory screen has the keys while it is open: the
        // character is handed no movement, no turn and no key.
        screen: inventory_open || controller.edit_build,
        inventory: inventory_open,
        inventory_unheard,
        // The head's free look: its share of the mouse's travel, its
        // key come up, and how far down the camera last pointed.
        head_look: [head.x, head.y],
        head_reset: pending.head_reset,
        camera_pitch: {
            let ahead = quat_rotate(quat_mul(quat_axis(0, controller.movement.shown_pitch()), controller.now.camera.rotation), [0.0, 0.0, 1.0]);
            (-ahead[1]).clamp(-1.0, 1.0).asin().to_degrees()
        },
        // The jump key held vaults or climbs what stands ahead.
        vault: if played { view.played() && controller.vault_key } else { held("vault") },
        move_dir: if played { move_keys } else { [axis(held("d"), held("a")), axis(held("w"), held("s"))] },
        look_delta: [look.x, look.y],
        // Sprint only starts from a walk, so a scripted run presses it
        // a second after the movement keys.
        sprint: held("shift") && (scenario.is_some() || since(1.0)),
        // A played run's sprint, lean, step aside, breath and crouch
        // come as the game's commands for their bindings.
        sprint_toggle: pending.sprint_toggle,
        sprint_end: pending.sprint_end,
        lean_toggle: pending.lean_toggle,
        lean_end: pending.lean_end,
        step_toggle: pending.step_toggle,
        step_end: pending.step_end,
        breath_end: pending.breath_end,
        restore_pose: pending.restore_pose,
        walk_toggle: pending.walk_toggle,
        walk_end: pending.walk_end,
        // The slow lean's axis, as its keys push it.
        slow_lean: if played && view.played() { controller.slow_lean } else { 0.0 },
        // A scripted run with `jump` jumps two seconds in.
        jump: pending.jump || (has("jump") && passing(2.0)) || script_pressed("jump"),
        crouch: pending.crouch || script_pressed("crouch"),
        // A scripted run with `prone` lies down a second in.
        prone: pending.prone || (has("prone") && passing(1.0)) || script_pressed("prone"),
        // A scripted run with `breath` holds it three seconds in.
        hold_breath: pending.breath || (has("breath") && passing(3.0)) || script_pressed("breath"),
        pose_delta: pending.stance_delta + notch("stance_up", "stance_down") * delta_level,
        speed_delta: pending.speed_delta + notch("wheel_up", "wheel_down") * delta_speed,
        // A scripted run with `stepr` or `stepl` steps a second in,
        // or three seconds in with `prone`, when the character is
        // down: lying down the same key rolls.
        lean: 0.0,
        lean_keys: [held("q"), held("e")],
        step: match (scenario, scripted) {
            (Some(_), _) => axis(script_held("step_right"), script_held("step_left")) as i32,
            (None, Some(_)) => {
                let at = if has("prone") { 3.0 } else { 1.0 };
                axis(has("stepr") && since(at), has("stepl") && since(at)) as i32
            }
            (None, None) => 0,
        },
        blind_toggle,
        blind_end,
        blind_firearm,
        blind_may_start,
        // The mount's key, a played run's binding or a script's command,
        // what the thing in hand says of it, and how deep the weapon last
        // stuck into something.
        mount: pending.mount || script_pressed("cmd:WeaponMounting"),
        mount_may: firearm.is_some_and(|f| f.may_mount()),
        // Nought where the weapon is in another character
        // (`MovementContext.OverlapDepth`).
        overlap_depth: if controller.overlap_with_character { 0.0 } else { controller.overlap },
        // The left shoulder's key, and whether the operation in hand
        // passes it on.
        left_stance: pending.left_stance || script_pressed("cmd:LeftStanceToggle"),
        left_stance_may: firearm.is_some_and(|f| f.may_toggle_left_stance()),
    };
    // Dead, no key is heard (the game takes the character's input away
    // with its life; health spec, "Death").
    let input = if controller.health.as_ref().is_some_and(|health| !health.alive) { SimInput::default() } else { input };
    (input, aiming)
}
