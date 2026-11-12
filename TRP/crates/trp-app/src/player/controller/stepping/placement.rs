//! The rig placed for the frame: its pose read, the weapon and the camera where it leaves them, and the overlap measured.

use super::*;

/// What the frame's rig placed leaves for what is drawn of the frame.
pub(super) struct Placed {
    pub(super) weapon: ([f32; 3], [f32; 4]),
    pub(super) camera: CameraPose,
    pub(super) elbows: [[f32; 3]; 2],
    pub(super) rig_now: Option<std::sync::Arc<Vec<([f32; 3], [f32; 4])>>>,
    pub(super) rig_straight: Option<std::sync::Arc<Vec<([f32; 3], [f32; 4])>>>,
    pub(super) rig_apart: [f32; 3],
    pub(super) mount_drawn: Option<MountDrawn>,
    pub(super) shift: Option<[f32; 3]>,
}

/// The rig as its animator has it now, the weapon's moved bone and the camera placed, a grenade's release and
/// the blade's place, and how deep the weapon would stick into something; from what the frame had before.
#[allow(clippy::too_many_arguments)]
pub(super) fn place_rig(hands: &mut HandsRig, controller: &mut Controller, range: &Range, view: &View, frame: trp_sim::base::pacing::Frame, inventory_open: bool, outside: bool, thrown_now: bool, start: Placed) -> Placed {
    let Placed { mut camera, mut rig_now, mut rig_straight, mut mount_drawn, mut shift, .. } = start;
    let (weapon, elbows, rig_apart);
    // The rig as its animator has it now, which is what the rest
    // of the frame goes by, as the game's late update does.
    // What is drawn of it is the animator's pose for drawing,
    // which is that pose taken on curves without corners while
    // the options' switch says so (`Animator::set_drawn_smooth`).
    // The simulation reads the pose itself.
    let posed = hands.animator().map(|animator| (animator.pose().to_vec(), animator.drawn_pose()));
    if let Some((mut posed, mut drawn)) = posed {
        // The weapon bone held toward its rest by the tactical
        // reload's blend, in what is read and what is drawn alike.
        let blend = hands.firearm.as_ref().map_or(0.0, |firearm| firearm.tactical_reload());
        hands.hands.set_tactical_reload(blend);
        hands.frame.stiffen(&hands.hands, &mut posed);
        hands.frame.stiffen(&hands.hands, &mut drawn);
        // An underbarrel launcher's bones as its own animator has
        // them, and while they are in use the rifle carried by its
        // prop bone (`UpdateWeaponBoneByLauncherWeaponBone`).
        if let Some(firearm) = &hands.firearm {
            hands.frame.launcher_pose(firearm, &mut posed, false);
            hands.frame.launcher_pose(firearm, &mut drawn, true);
        }
        controller.launcher_bones = hands.firearm.as_ref().is_some_and(|firearm| firearm.launcher_bones_in_use());
        // At the sight now aimed by.
        if let Some(nodes) = hands.hands.sight_nodes() {
            hands.frame.set_sights(nodes);
        }
        controller.rig_pose = hands.frame.read(&posed);
        rig_straight = (posed != drawn).then(|| std::sync::Arc::new(posed.clone()));
        rig_now = Some(std::sync::Arc::new(drawn));
    }
    let pose = controller.rig_pose;
    hands.hands.weapon_root = pose.parent;
    // Seen from outside, the body reaches for the vest as the left
    // hand's marker nears the weapon's marker for it, while a
    // reload is in hand or the inventory is open, and not in a
    // sprint (`Player.AdjustUtilityLayerWeight`).
    if outside {
        let reaching = (hands.firearm.as_ref().is_some_and(|f| f.reloading()) || inventory_open) && !controller.movement.sprinting;
        controller.movement.body.weigh_reach_for_vest(pose.vest_reach.is_some(), pose.vest_reach.filter(|_| reaching), frame.dt);
    }

    // Where that leaves the weapon's moved bone, and the camera:
    // the body's weapon bone takes the weapon over in a sprint and
    // its right hand in a crawl, and its camera bone lends its turn.
    let m = &controller.movement;
    // The iron sights' line and the muzzle as the rig has them now.
    hands.hands.note_sights(pose.sight_line, pose.muzzle);
    // On the player's clock, as the game's weapon motion runs.
    let arms_dt = m.player_dt();
    hands.hands.blend_poses(arms_dt);
    // What is drawn of the body's hold takes the body as drawn.
    let in_view = m.drawn_body_in_view();
    // With the left shoulder's part (`PlayerBones.ShiftWeaponRoot`),
    // for the character's own eyes; seen from outside, the body's
    // weapon bone by its authority alone.
    let left = trp_sim::weapons::hands::LeftShoulder {
        curve: if outside { 0.0 } else { m.body.left_stance_curve() },
        sprinting: m.state == trp_sim::State::Sprint,
        anim_value: m.left_stance().last_anim,
        aiming: hands.hands.is_aiming(),
        forward: m.forward_input(),
        dt: arms_dt,
        ribcage_local: m.body.ribcage_local(),
        interaction: !outside && m.interaction_playing(),
    };
    weapon = hands.hands.weapon_transform_shifted(pose.parent, m.body.weapon_root_third(in_view), m.body.held(in_view), pose.marker, left).held;
    // The camera takes the sight's place off the weapon where the
    // procedural motion put it, before the body takes the weapon.
    // With the left shoulder's curve up the camera's pass comes late
    // in the frame (`LateTransformations`), after the shoulder has
    // moved the weapon, and takes the sight from there.
    let placed = if hands.hands.left_shoulder_curve() > 0.0 { weapon } else { hands.hands.weapon_transform() };
    let aim = pose.aim_bone.map(|bone| {
        let hanging = quat_rotate(pose.parent.1, placed.0);
        let origin: [f32; 3] = std::array::from_fn(|i| pose.parent.0[i] + hanging[i]);
        let turn = quat_mul(pose.parent.1, placed.1);
        let reach = quat_rotate(turn, bone);
        AimPoint {
            bone: std::array::from_fn(|i| origin[i] + reach[i]),
            origin,
            // The barrel runs along the moved bone's -y.
            barrel: quat_rotate(turn, [0.0, -1.0, 0.0]),
            line: quat_rotate(turn, pose.aim_line),
        }
    });
    hands.hands.body_camera = Some(m.body.camera_bone());
    // On a mount the rig is moved onto the point (`MountingRotationFunction`,
    // the pivot as the weapon's `Weapon_root` holds it), and the mount's
    // camera follows it from the camera's own object.
    let mount_pivot = m.mount_view().map(|view| {
        let pivot = hands.hands.settings.mount.pivot_for(view.side != trp_sim::weapons::mounting::Side::Forward, view.on_bipod);
        let turned = quat_rotate(pose.parent.1, pivot);
        std::array::from_fn::<f32, 3, _>(|k| pose.parent.0[k] + turned[k])
    });
    rig_apart = mount_pivot.map_or([0.0; 3], |pivot| m.mounted_rig_apart(pivot));
    let camera_apart = {
        let (container, ribcage) = (m.camera_container(), m.ribcage());
        let look = quat_mul(quat_axis(1, m.yaw - m.drawn_body_yaw()), quat_axis(0, m.pitch));
        quat_rotate([-look[0], -look[1], -look[2], look[3]], std::array::from_fn(|k| container[k] - ribcage[k]))
    };
    let rig_shift: [f32; 3] = std::array::from_fn(|k| rig_apart[k] - camera_apart[k]);
    hands.hands.set_rig_shift(rig_shift);
    if let (Some(view), Some(pivot)) = (m.mount_view(), mount_pivot) {
        // The pivot in the rig's frame, as `mounted_rig_apart` takes it.
        mount_drawn = Some(MountDrawn { point: view.point, pivot, progress: view.progress });
        shift = Some(rig_shift);
    }
    // The camera is moved for the character's own eyes only: seen
    // from outside it is left where it last stood
    // (`ThirdPersonStrategy.ApplyCameraTransformations`).
    if !outside {
        camera = hands.hands.frame_on_clock(frame.dt, arms_dt, pose.animated, aim);
        // The camera's box is pushed out of whatever it has got into,
        // and the camera with it: the next frame's easing starts there.
        if let Some(shape) = &controller.camera_box {
            let push = m.camera_push(&*range, camera.position, camera.rotation, shape.size, shape.centre);
            camera.position = hands.hands.push_camera(push);
        }
    } else {
        hands.hands.frame_outside(frame.dt);
    }
    elbows = [hands.hands.elbow_shift(true), hands.hands.elbow_shift(false)];

    // Off the rig as this frame poses and hangs it: where a grenade
    // let go of on this frame leaves the hand, as the frame draws the
    // rig, the first person's squeeze on (grenades-and-knife spec,
    // "The throw"); and where the knife's blade is left for the next
    // frame's sweep, the squeeze off ("The knife's hands").
    if thrown_now {
        if let (Some(bone), Some((_, data))) = (hands.thrown_bone, hands.thrown.as_ref()) {
            let squeeze = if *view == View::FirstPerson { hands.hands.fov_scale() } else { 1.0 };
            let in_root = trp_sim::held::grenade_hands::release(hands.frame.in_root(bone, weapon), squeeze, data.offset);
            let (at, turn) = rig_in_world(m, rig_apart, in_root);
            // The eye this frame, for the log: the camera beside the
            // ribcage, not squeezed (`hands_view::apply_hands`).
            let eye = rig_in_world(m, [0.0; 3], (std::array::from_fn(|k| camera.position[k] + camera_apart[k]), camera.rotation)).0;
            controller.grenade_release = Some(GrenadeRelease { at, turn, yaw: m.yaw, pitch: m.pitch, eye });
        }
    }
    controller.blade_left = match (hands.knife.as_ref(), hands.blade.as_ref()) {
        (Some(_), Some(blade)) => Some(rig_in_world(m, rig_apart, hands.frame.in_root(blade.bone, weapon))),
        _ => None,
    };

    // Last in the frame, as in the game: how deep the weapon would
    // stick into something (in blind fire from its end point first:
    // `Hands::measure_overlap`). The next physics steps turn it away
    // by that, and past a limit the sights come down until it is
    // clear.
    // With the left shoulder wanted, from where it puts the weapon;
    // and its way back's line, 0.2 m further left, tried as well.
    let left_wanted = m.left_stance().wanted;
    // From the weapon as placed: on a mount, moved onto its point.
    // Whether what it met is another character: the last line that
    // hit says (`OverlapLn` writes it only on a hit).
    let mut with_character = false;
    let (depth, measured) = hands.hands.measure_overlap_shoulder(left_wanted, |line| {
        let (depth, character) = m.placed_weapon_overlap_met(&*range, line, rig_apart);
        if depth > 0.0 {
            with_character = character;
        }
        depth
    });
    let clear_left = !left_wanted || m.placed_weapon_overlap(&*range, hands.hands.shoulder_line(0.2), rig_apart) <= 0.0;
    hands.hands.set_overlap_met(depth, with_character);
    controller.overlap_with_character = with_character && depth > 0.0;
    let run_down = hands.hands.turn_away_value();
    if let Some(firearm) = &mut hands.firearm {
        firearm.overlap(depth, run_down, m.can_aim());
        // Sights the obstacle put down or gave back are so from
        // here on, not from the next physics step.
        let aiming = firearm.aiming();
        hands.hands.aim_changed_after_frame(aiming, frame.dt);
    }
    controller.overlap = depth;
    controller.movement.left_stance_overlap(depth, measured, || clear_left, frame.dt);
    Placed { weapon, camera, elbows, rig_now, rig_straight, rig_apart, mount_drawn, shift }
}
