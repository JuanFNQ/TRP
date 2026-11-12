//! The hands made ready for the frame: their settings, health and pose, the aim, and the springs' physics steps.

use super::*;

/// The weapon's motion told the frame's settings, load, health and point of view, the aim taken, the springs
/// stepped and the rig turned to the view; whether it is aimed now.
pub(super) fn hands_before(hands: &mut HandsRig, controller: &mut Controller, point_of_view: PointOfView, aiming: bool, frame: trp_sim::base::pacing::Frame) -> bool {
    // The sprint switching on puts the sights down and lets go of
    // the trigger; so does entering a state that does not allow
    // aiming, for the sights.
    let (sprint_started, aim_lowered) =
        (controller.movement.take_sprint_started(), controller.movement.take_aim_lowered());
    // A state the body's animator entered may carry a pose for the
    // first-person weapon (the roll lying down does).
    hands.hands.weapon_flip_speed = controller.movement.settings.weapon_flip_speed;
    // The weapon's motion and its recoil as the options' switches
    // have them (the rig in hand may be new), and what health does
    // to it.
    hands.hands.set_systems(&controller.movement.systems);
    // The character's load and the gear's share of the ergonomics
    // (weapon handling spec, "The load"), before health's share of
    // the sway spring is added to the load's.
    hands.hands.set_load(controller.movement.stamina.settings.overweight, controller.gear_ergonomics);
    // An arm's damage coming or going has a firearm worked out again
    // at once (`Hands::set_health`) and aiming costs the arms by its
    // new ergonomic weight (`RecalculateErgonomic`: `Physical.Aim`);
    // its animator's skill speeds go to 1 or back
    // (`Firearm::set_arms_damaged`).
    let conditions = controller.movement.conditions;
    if hands.hands.set_health(conditions) {
        hands.aim_mass = hands.hands.settings.ergonomic_weight();
        controller.movement.stamina.aim_mass = hands.aim_mass;
    }
    if let Some(firearm) = &mut hands.firearm {
        firearm.set_arms_damaged(conditions.left_arm, conditions.right_arm);
    }
    // The weapon's motion is told whose eyes see it; the rig in
    // the hands may have been changed for another since.
    if hands.hands.point_of_view() != point_of_view {
        hands.hands.set_point_of_view(point_of_view);
    }
    for pose in controller.movement.take_poses() {
        hands.hands.assign_pose(pose.as_ref());
    }
    if let Some(firearm) = &mut hands.firearm {
        if sprint_started {
            firearm.sprint_started();
        }
        if aim_lowered {
            firearm.lower_aim();
        }
        // More than half a second off the ground puts the sights
        // down, on every frame of it.
        if controller.movement.long_fall() {
            firearm.set_aim(false, controller.movement.can_aim());
        }
        // So does a vault or a climb of something high enough to
        // take a hand off the weapon.
        if controller.movement.take_aim_taken() {
            firearm.set_aim(false, controller.movement.can_aim());
        }
    }
    // The camera takes the head's turn, and the aimed field of
    // view gives way while the mouse has the head.
    let head = &controller.movement.head_look;
    hands.hands.set_head_rotation(head.rotation(), head.control(), aiming);
    // The rangefinder's aim let go as a weapon's sights are put down:
    // a sprint begun, a state that cannot aim, a long fall, a vault
    // (`OnAimingDisabled`).
    if let Some(rangefinder) = &mut hands.rangefinder {
        let can = controller.movement.can_aim();
        if rangefinder.aiming() && (sprint_started || aim_lowered || !can || controller.movement.long_fall() || controller.movement.take_aim_taken()) {
            rangefinder.set_aim(false, can);
        }
    }
    let aiming = hands.firearm.as_ref().is_some_and(|f| f.aiming()) || hands.rangefinder.as_ref().is_some_and(|r| r.aiming());
    hands.aiming = aiming;
    // Its optic live while its controller has it on.
    let forced = hands.rangefinder.as_ref().filter(|r| r.optic_on()).map(|_| 0);
    if let Some(sights) = hands.hands.settings.sights.as_mut().filter(|_| hands.rangefinder.is_some()) {
        sights.force_optic(forced);
    }
    // The weapon's motion takes the aim on this frame, whether a
    // physics step comes in it or not (`Hands::aim_changed`).
    hands.hands.aim_changed(aiming);
    // The bone the weapon's moved bone hangs from, as its animator
    // left it on the frame before: the springs run ahead of the
    // frame's own update of the animator.
    hands.hands.weapon_root = controller.rig_pose.parent;

    // The springs, once for each physics step in the frame.
    for _ in 0..frame.physics_steps {
        let footstep = controller.movement.take_footstep();
        let walk_stop = controller.movement.take_walk_stop();
        let m = &controller.movement;
        hands.hands.advance(
            controller.pacer.physics_step(),
            &HandsInput {
                yaw: m.yaw,
                pitch: m.shown_pitch(),
                motion: m.input_velocity(),
                velocity: m.velocity,
                speed: m.smoothed_speed,
                walking: m.state == State::Move,
                walk_stop,
                sprinting: m.state == State::Sprint,
                idle: m.state == State::Idle,
                footstep,
                // Mounted, the weapon's motion reads the mount's lean
                // (`ProceduralWeaponAnimation.SmoothedTilt`).
                tilt: if m.mounted() { m.weapon_tilt() } else { m.smoothed_tilt },
                tilt_target: m.tilt,
                pose_level: m.pose_level,
                smoothed_pose_level: m.smoothed_pose_level,
                aiming,
                prone: m.prone,
                crawling: m.state == State::ProneMove,
                frame_dt: frame.dt,
                wind: m.wind(),
            },
        );
    }
    // The rig is turned to the view after the steps, as the game
    // does it: the sway sees this frame's turn a frame on.
    hands.hands.turn_rig(controller.movement.yaw, controller.movement.shown_pitch());
    aiming
}
