//! Stepping the controller one frame.

use super::*;

mod firearm_frame;
mod hands_ready;
mod held_items;
mod input;
mod keys;
mod movement_told;
mod placement;

use keys::Keys;

/// Runs the frames of the simulation that have come due.
///
/// Each is one frame of the game: the player is moved and the weapon's
/// logic run once, by the frame's own length, and the weapon's springs
/// stepped once for each physics step that falls in it, in the game's
/// order: movement, the springs, the weapon's logic (whose shots the
/// springs feel on their next step), then the camera.
#[allow(clippy::too_many_arguments)]
pub fn step(
    view: Res<View>,
    options: Res<Options>,
    mut controller: ResMut<Controller>,
    hands: Option<ResMut<HandsRig>>,
    frame_time: Res<Time<Virtual>>,
    mut range: ResMut<Range>,
    features: Res<crate::screens::options_view::Features>,
    ragdoll: Res<crate::player::death::Ragdoll>,
    skills: Option<Res<crate::player::skills::Skills>>,
    frame_count: Res<bevy::diagnostic::FrameCount>,
    dummies: Option<Res<crate::world::dummy::Dummies>>,
) {
    let controller = &mut *controller;
    controller.feet_before_move = controller.movement.position;
    // Dead, the character is left as it stood: the game switches its
    // animators and its controller off with its life (`Player.OnDead`;
    // health spec, "Death"), and what moves after is the corpse's
    // (`death`).
    // (Dead, what is drawn is cut to the newest frame, straight: `cut`.)
    let dead = controller.health.as_ref().is_some_and(|health| !health.alive);
    if dead {
        if !controller.cut {
            controller.cut = true;
            controller.settle_shown();
        }
        controller.paced = Paced::default();
        return;
    }
    controller.cut = false;
    let inventory_open = controller.inventory_open;
    // Whose eyes the character is seen with: its own while the picture is
    // its own, and anyone else's from a camera outside it, as the game has
    // every character but the one playing (third-person spec).
    let point_of_view = options.point_of_view(*view);
    let outside = point_of_view == PointOfView::ThirdPerson;
    if controller.movement.body.point_of_view() != point_of_view {
        controller.movement.body.set_point_of_view(point_of_view);
    }
    let began = std::time::Instant::now();
    let host_before = controller.pacer.host_time();
    let due = controller.pacer.pass(frame_time.delta_secs());
    controller.paced =
        Paced { host_dt: frame_time.delta_secs(), blend: controller.pacer.blend(), ..Paced::default() };
    // One of the recorder's scripts as the input. Its turns come as a
    // mouse's would: what the script asks over each drawn frame, through
    // the sensitivity in force (the game takes the recorder's turns
    // through its mouse sensitivity too), gathered for the simulation's
    // frames to share out.
    let scenario = options.scenario.as_ref();
    if let Some(scenario) = scenario {
        let at = options.scenario_at;
        let [yaw, pitch] = scenario.turned(host_before - at, controller.pacer.host_time() - at);
        let sensitivity = match hands.as_deref() {
            Some(rig) if rig.aiming => rig.hands.aim_sensitivity(rig.aim_sensitivity) * options.aiming_sensitivity,
            _ => options.sensitivity,
        };
        controller.pending.look += Vec2::new(yaw, pitch) * sensitivity;
    }
    if due == 0 {
        controller.settle_shown();
        controller.paced.cost = began.elapsed().as_secs_f32();
        return;
    }
    let mut hands = hands;

    let scripted = options.hold.as_deref();
    let has = |name: &str| scripted.is_some_and(|script| script.split('+').any(|k| k == name));
    let axis = |pos: bool, neg: bool| pos as i32 as f32 - neg as i32 as f32;
    let (delta_level, delta_speed) = (controller.movement.settings.delta_level, controller.movement.settings.delta_speed);

    // The mouse's travel goes to the frames by their own time, as a game
    // running at their rate would have had it: each frame that is due
    // takes the share that falls in it, and what is left belongs to a
    // frame still to come and is shown ahead of it (`Controller::look`).
    // Given whole to whichever frame came due, the view would turn by
    // uneven amounts wherever the two rates do not divide, and the body
    // that follows it in a jump, with the weapon it carries, would shake.
    let mut travel = controller.pending.look;
    let mut head_travel = controller.pending.head;
    for _ in 0..due {
        let before = controller.pacer.simulated();
        let frame = controller.pacer.frame();
        let now = controller.pacer.simulated();
        let look = travel * frame.input_share;
        travel -= look;
        let head = head_travel * frame.input_share;
        head_travel -= head;
        // A scripted run's presses: held from a moment on, or made on the
        // frame that passes it.
        let since = |seconds: f32| now > seconds;
        let passing = |seconds: f32| before < seconds && now >= seconds;
        // A script's keys and presses go by the simulation's own time.
        let (script_before, script_now) = (before - options.scenario_at, now - options.scenario_at);
        let script_held = |name: &str| scenario.is_some_and(|s| s.held(name, script_now));
        let script_pressed =
            |name: &str| scenario.is_some_and(|s| s.pressed(script_before, script_now).any(|key| key == name));
        // A key of a run that plays a script, by the script's name for
        // it. A played run's keys come as the game's bindings instead.
        let played = scenario.is_none() && scripted.is_none();
        let held = |name: &str| match (scenario, scripted) {
            (Some(_), _) => script_held(name),
            (None, Some(_)) => has(name),
            (None, None) => false,
        };
        // Presses go to the first frame that comes.
        let mut pending = std::mem::take(&mut controller.pending);

        // The aim key asks; the weapon's logic and the movement state decide.
        // The same goes for a press of the trigger: a sprint and a crawl
        // take none.
        let (can_aim, can_fire) = (controller.movement.can_aim(), controller.movement.can_press_trigger());
        let armed = controller.armed;
        // With the inventory screen open the weapon hears none of its
        // keys either. Where each stands is still noted, so that a key
        // pressed under the screen does nothing when it shuts.
        let heard = !inventory_open;
        // With the weapon's handling switched off none of its keys but the
        // aim's is heard (settings spec, "Features").
        let handled = heard && features.weapon_handling;
        let aim_key = armed
            && match (scenario, scripted) {
            (Some(_), _) => script_held("aim"),
            // A scripted run with `aim` presses it half a second in.
            (None, Some(_)) => has("aim") && since(0.5),
            (None, None) => false,
        };
        let keys = keys::Keys {
            scenario,
            scripted,
            before,
            played,
            heard,
            handled,
            armed,
            can_aim,
            can_fire,
            aim_key,
            since: &since,
            passing: &passing,
            script_held: &script_held,
            script_pressed: &script_pressed,
            held: &held,
            has: &has,
            axis: &axis,
        };
        keys::weapon_keys(&keys, controller, &mut hands, &pending, &features);
        let (input, aiming) = input::frame_input(&keys, controller, &hands, &pending, &view, inventory_open, [look, head], [delta_level, delta_speed]);
        movement_told::after_movement(controller, &mut hands, &mut range, frame, &input);

        let (mut weapon, mut camera) = (controller.now.weapon, controller.now.camera);
        let mut elbows = controller.now.elbows;
        let mut rig_now = None;
        let mut rig_straight = None;
        let mut rig_apart = [0.0f32; 3];
        // A grenade let go of on this frame.
        let mut thrown_now = false;
        let (mut mount_drawn, mut shift) = (None, None);
        if let Some(hands) = hands.as_deref_mut() {
            let aiming = hands_ready::hands_before(hands, controller, point_of_view, aiming, frame);
            // The weapon's own logic.
            firearm_frame::firearm_keys(&keys, hands, controller, &pending, &view, frame, outside);
            firearm_frame::firearm_events(hands, controller, &features, frame, now, outside, aiming);
            let around = held_items::Around { features: &features, ragdoll: &ragdoll, range: &range, dummies: dummies.as_deref(), skills: skills.as_deref(), frame_count: &frame_count };
            held_items::held_items(hands, controller, &mut pending, frame, heard, around, &mut thrown_now);
            let start = placement::Placed { weapon, camera, elbows, rig_now, rig_straight, rig_apart, mount_drawn, shift };
            placement::Placed { weapon, camera, elbows, rig_now, rig_straight, rig_apart, mount_drawn, shift } = placement::place_rig(hands, controller, &range, &view, frame, inventory_open, outside, thrown_now, start);
        }

        // What the springs moved is drawn from the frames with a physics
        // step; what the movement and the body moved, from every frame.
        if frame.physics_steps > 0 {
            controller.before = controller.now;
            controller.now = Sprung { weapon, camera, elbows, shift };
        }
        controller.stepped.push_back(Stepped { rig: rig_now, rig_straight, rig_apart, mount: mount_drawn, ..Stepped::of(&controller.movement) });
        let kept = (controller.pacer.physics_step() / controller.pacer.frame_length()).ceil() as usize + 2;
        while controller.stepped.len() > kept {
            controller.stepped.pop_front();
        }
        controller.paced.frames += 1;
        controller.paced.physics_steps += frame.physics_steps;
    }
    // The travel no frame has had yet.
    controller.pending.look = travel;
    controller.pending.head = head_travel;
    // A death on one of this step's frames: what is drawn cut to the
    // newest, straight, from now on (`cut`).
    controller.cut = controller.health.as_ref().is_some_and(|health| !health.alive);
    controller.settle_shown();
    controller.paced.cost = began.elapsed().as_secs_f32();
    controller.paced.blend = controller.pacer.blend();
}
