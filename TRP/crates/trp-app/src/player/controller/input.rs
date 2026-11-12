//! Gathering the frame's input, and the hands' log.

use super::*;

/// A change of the hands, as a run told to write each has it written
/// (`TRP_LOG_HANDS` in the environment): the frame it was asked on (none
/// where nothing asked, as the way back after a use), the frame the new
/// hands were made on and what they hold. Written on the frame the
/// simulation first steps them, with what that step left of the hands'
/// animator and of an item's effect on health: the game makes the hands
/// asked from the empty hands within the key's or the click's own call,
/// before that frame's health and hands (health spec, "The hands").
#[derive(Clone)]
pub struct HandsChange {
    pub asked: Option<u32>,
    pub made: u32,
    pub what: String,
}

/// Whether the run writes each change of the hands (`TRP_LOG_HANDS`).
pub fn hands_log() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("TRP_LOG_HANDS").is_some())
}

/// Collects mouse look and what the keys came to every frame: the game's
/// bindings as its input manager gives them out (`crate::keys::hear`),
/// each by the game's name for it.
pub fn gather_input(
    view: Res<View>,
    options: Res<Options>,
    hands: Option<Res<HandsRig>>,
    frame_time: Res<Time<Virtual>>,
    bindings: Res<crate::platform::keys::Bindings>,
    mut controller: ResMut<Controller>,
) {
    // A capture's turns of the wheel with the zoom's keys held (`--zoom`),
    // as `GetZoomInput` takes them (the turn times `OpticSensitivity`),
    // whatever else it holds.
    let scripted_zoom = std::mem::take(&mut controller.zoom_notches) as f32 * options.optic_sensitivity;
    controller.pending.zoom += scripted_zoom;
    // An unattended capture must not pick up the mouse happening to move
    // over its window. It can turn the view at a steady rate instead
    // (`--turn`), from a second in: what a mouse moved evenly would add in
    // the time each drawn frame took.
    // A run that plays a script (`--scenario`) takes its turning from that,
    // in `step`, and leaves the mouse alone likewise.
    if options.screenshot.is_some() || options.scenario.is_some() {
        if options.turn != 0.0 && frame_time.elapsed_secs() > 1.0 {
            controller.pending.look.x += options.turn * frame_time.delta_secs();
        }
        // A capture given presses of its own (`--tap`) and no keys to
        // hold hears them as a played run hears a hand's, through the
        // bindings: how the bindings themselves are tried unattended.
        if (options.taps.is_empty() && options.mouse.is_empty()) || options.hold.is_some() || options.scenario.is_some() {
            return;
        }
    }
    // How the trigger, the vault key and the movement keys stand is kept
    // whatever has the keys, as a key's own state is: let go under an
    // open screen, they are let go.
    // (Not a press under the edit build screen, whose left button turns
    // the preview and clicks its buttons: the screen is the mouse's until
    // the button that shut it is let go, `edit_build::release`.)
    if bindings.fired("Shoot") && !controller.edit_build && !controller.on_debug_button {
        controller.trigger = !controller.trigger;
    }
    if bindings.ended("Shoot") {
        controller.trigger = false;
    }
    if bindings.fired("Vaulting") || bindings.ended("Vaulting") {
        controller.vault_key = !controller.vault_key;
    }
    controller.move_keys = [bindings.axis("MoveX"), bindings.axis("MoveY")];
    controller.slow_lean = bindings.axis("LeanX");
    // With the inventory open the mouse and every key are the screen's
    // (`R` turns what the mouse holds): nothing is gathered, and a key
    // pressed meanwhile is lost, as in the game (inventory spec, "What an
    // open screen does to the keys").
    if !view.played() || controller.inventory_open || controller.edit_build {
        return;
    }
    let (delta_level, delta_speed) = {
        let m = &controller.movement;
        (m.settings.delta_level, m.settings.delta_speed)
    };
    // While aiming, the weapon's factor and the aiming setting replace the
    // ordinary sensitivity.
    let sensitivity = match hands.as_deref() {
        Some(rig) if rig.aiming => rig.hands.aim_sensitivity(rig.aim_sensitivity) * options.aiming_sensitivity,
        _ => options.sensitivity,
    };
    let menu_shown = controller.menu_shown;
    let mut menu_notches = 0;
    let picker_took_reload = std::mem::take(&mut controller.picker_took_reload);
    let pending = &mut controller.pending;
    // The mouse's travel as the manager gave it out: to the turn, or with
    // the free look's key held to the head (`Preset::mouse`). A count is
    // so many degrees before the sensitivity (settings spec, "The
    // mouse"), which the head's look does not have, and either axis may
    // be turned about for both (the game's two switches).
    let mouse = bindings.mouse();
    let way = Vec2::new(if options.invert[0] { -1.0 } else { 1.0 }, if options.invert[1] { -1.0 } else { 1.0 });
    pending.look += Vec2::from(mouse.turn) * way * options.degrees_a_count * sensitivity;
    pending.head += Vec2::from(mouse.look) * way * options.look_degrees_a_count;
    // The look's key is bound to its release: coming up, it asks for the
    // head's way back.
    pending.head_reset |= bindings.fired(trp_sim::base::bindings::LOOK_BINDING);
    // Each binding by the game's name for it, pressed as the install has
    // it: the jump when its key is let go, where the same key held vaults
    // (vaulting spec, "Two moves, one key"); the reload when its key is
    // let go, and late by the double click's time where a double click
    // is bound to the same key (settings spec).
    pending.fold_stock |= bindings.fired("FoldStock");
    pending.jump |= bindings.fired("Jump");
    // (Not where the magazine picker took the key: `ammo_picker::hear`.)
    pending.reload |= bindings.fired("ReloadWeapon") && !picker_took_reload;
    pending.quick_reload |= bindings.fired("QuickReloadWeapon");
    pending.fire_mode |= bindings.fired("ShootingMode");
    pending.check_magazine |= bindings.fired("CheckAmmo");
    pending.check_chamber |= bindings.fired("CheckChamber");
    pending.force_auto |= bindings.fired("ForceAutoWeaponMode");
    pending.check_fire_mode |= bindings.fired("CheckFireMode");
    pending.examine |= bindings.fired("ExamineWeapon");
    pending.change_scope |= bindings.fired("ChangeAimScope");
    pending.magnification |= bindings.fired("ChangeAimScopeMagnification");
    pending.zoom += bindings.zoom();
    pending.zeroing_up |= bindings.fired("OpticCalibrationSwitchUp");
    pending.zeroing_down |= bindings.fired("OpticCalibrationSwitchDown");
    // An item in use stops for the shoot key, the aim key or the scope's
    // (`ToggleShooting`, `ToggleAlternativeShooting`, `ChangeScope`).
    pending.use_cancel |= bindings.fired("Shoot") || bindings.fired("Aim") || bindings.fired("ChangeAimScope");
    pending.walk_toggle |= bindings.fired("Walk");
    pending.walk_end |= bindings.ended("Walk");
    pending.breath |= bindings.fired("Breath");
    pending.breath_end |= bindings.ended("Breath");
    pending.crouch |= bindings.fired("Duck");
    pending.restore_pose |= bindings.ended("Duck");
    pending.prone |= bindings.fired("Prone");
    pending.sprint_toggle |= bindings.fired("Sprint");
    pending.sprint_end |= bindings.ended("Sprint");
    for (side, (lean, step)) in [("LeanLockLeft", "StepLeft"), ("LeanLockRight", "StepRight")].into_iter().enumerate() {
        pending.lean_toggle[side] |= bindings.fired(lean);
        pending.lean_end[side] |= bindings.ended(lean);
        pending.step_toggle[side] |= bindings.fired(step);
        pending.step_end[side] |= bindings.ended(step);
    }
    // Blind fire's two bindings, each held: given, its side; coming up,
    // the end (`ToggleBlindAbove`, `ToggleBlindRight`, `BlindShootEnd`).
    for (binding, side) in [("BlindShootAbove", 1), ("BlindShootRight", -1)] {
        if bindings.fired(binding) {
            pending.blind_toggle = side;
        }
        pending.blind_end |= bindings.ended(binding);
    }
    pending.mount |= bindings.fired("WeaponMounting");
    pending.left_stance |= bindings.fired("LeftStance");
    pending.bipod |= bindings.fired("ToggleBipods");
    if bindings.fired("Aim") {
        pending.aim.push(true);
    }
    if bindings.ended("Aim") {
        pending.aim.push(false);
    }
    for (binding, stroke) in [("Shoot", trp_sim::Stroke::Slash), ("Aim", trp_sim::Stroke::Stab)] {
        if bindings.fired(binding) {
            pending.strokes.push(stroke);
        }
    }
    // A grenade's throws by their own bindings, held whatever the trigger's
    // and the aim's are (`HighThrow`, `LowThrow`: `trp_sim::bindings`).
    for (binding, low) in [("HighThrow", false), ("LowThrow", true)] {
        if bindings.fired(binding) {
            pending.throw_keys.push((low, true));
        }
        if bindings.ended(binding) {
            pending.throw_keys.push((low, false));
        }
    }
    // The wheel's two bindings set the speed, and with the stance's key
    // held the stance's two set that. While a menu is shown for what is
    // looked at, the speed's two are the menu's instead: up the list for
    // a notch up.
    let notches = |up: &str, down: &str| bindings.fired(up) as i32 - bindings.fired(down) as i32;
    pending.stance_delta += notches("NextWalkPose", "PreviousWalkPose") as f32 * delta_level;
    if menu_shown {
        menu_notches = notches("Next", "Previous");
    } else {
        pending.speed_delta += notches("Next", "Previous") as f32 * delta_speed;
    }
    controller.menu_notches += menu_notches;
}
