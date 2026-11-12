//! Making the hands and carrying them on from other hands.

use super::*;

impl Hands {
    /// The same weapon built anew with another part, taken over mid-way
    /// (a reload's change of magazine: `Firearm::carry_on_from`): the
    /// motion as the build it replaces left it, its springs' and sway's
    /// motion, the breath, its clocks, blends, poses, lean, recoil, view,
    /// the field of view and the rest of what runs, on this build's own
    /// figures (its settings, its springs' and sway's constants, its
    /// strengths, its sights' line and muzzle as read off its own rig, its
    /// hold). The app's: the game changes the part on the weapon's own
    /// model, and nothing of its motion starts again. So too for the motion
    /// of a thing taken in hand in another's place (`Stowed::change_to`):
    /// the game's motion is one object (`ProceduralWeaponAnimation`), told
    /// the new thing's figures, and what runs runs on (recorded, `courses8`'s
    /// grenade throws and eating: the steps' tempo and the lean's reach go
    /// on through every change of hands).
    pub fn carry_on_from(&mut self, from: &Hands) {
        for (mine, theirs) in [(&mut self.hands_position, &from.hands_position), (&mut self.hands_rotation, &from.hands_rotation), (&mut self.camera_rotation, &from.camera_rotation), (&mut self.camera_position, &from.camera_position)] {
            mine.take_state(theirs);
        }
        (self.sway.position, self.sway.velocity) = (from.sway.position, from.sway.velocity);
        self.force_react = from.force_react.clone();
        (self.position_velocity, self.position_velocity_rate, self.position_acceleration, self.position_acceleration_rate, self.last_position_velocity) =
            (from.position_velocity, from.position_velocity_rate, from.position_acceleration, from.position_acceleration_rate, from.last_position_velocity);
        (self.rotation_velocity, self.rotation_velocity_rate, self.rotation_acceleration, self.rotation_acceleration_rate, self.last_rotation_velocity) =
            (from.rotation_velocity, from.rotation_velocity_rate, from.rotation_acceleration, from.rotation_acceleration_rate, from.last_rotation_velocity);
        (self.rotation_velocity_sum, self.rotation_acceleration_sum, self.last_forward, self.rig_view) = (from.rotation_velocity_sum, from.rotation_acceleration_sum, from.last_forward, from.rig_view);
        (self.walk_clocks, self.walk_clocks_lame, self.lame, self.walking) = (from.walk_clocks.clone(), from.walk_clocks_lame.clone(), from.lame, from.walking);
        (self.walk_clocks_load, self.walk_clocks_load_lame) = (from.walk_clocks_load.clone(), from.walk_clocks_load_lame.clone());
        // The steps' tempo too (`WalkEffector.StepFrequency`), worked out
        // only as a footstep comes.
        self.step_frequency = from.step_frequency;
        // The character's load and the gear's share are the character's, not
        // the build's: this build's strengths and spring go by them at once.
        // So is the arms' damage: a controller made with an arm damaged has
        // its ergonomics at 0 from the first time they are read
        // (`TotalErgonomicsFunc`, deferred), with nothing reset.
        self.arms_sway = from.arms_sway;
        if self.settings.arms_damaged != from.settings.arms_damaged {
            self.settings.arms_damaged = from.settings.arms_damaged;
            self.weapon_variables();
        }
        self.set_load(from.settings.overweight, from.settings.gear_ergonomics);
        self.update_sway_spring();
        (self.aim_sway_direction, self.aim_sway_since, self.aim_sway_random) = (from.aim_sway_direction, from.aim_sway_since, from.aim_sway_random.clone());
        (self.aiming, self.body_camera, self.accumulator) = (from.aiming, from.body_camera, from.accumulator);
        let settings = self.recoil.settings.clone();
        self.recoil = from.recoil.clone();
        self.recoil.settings = settings;
        // The breath goes on as it was, its hip penalty with it: hands that
        // hold no firearm keep the last firearm's, and a firearm's
        // controller made works its own out (`firearm_made`). (A reload's
        // change of magazine works it out again too, `WeaponModified` from
        // the reload's operations; PROVISIONAL: the app's
        // build changed while a reload runs keeps the old build's figure, one
        // changed at rest takes its own at once; the moment within the
        // reload not read.)
        (self.breath, self.combo_on, self.pose, self.time) = (from.breath.clone(), from.combo_on, from.pose, from.time);
        // The hand shake is the weapon motion's, run by its own clock
        // (`ProceduralWeaponAnimation`'s coroutine): it goes on through a
        // change of hands.
        self.hand_shake = from.hand_shake.clone();
        (self.camera_smooth, self.tilt_blend, self.possible_tilt) = (from.camera_smooth, from.tilt_blend, from.possible_tilt);
        (self.camera_follow, self.camera_follow_target, self.camera_follow_speed) = (from.camera_follow, from.camera_follow_target, from.camera_follow_speed);
        (self.fov, self.fov_target, self.fov_blend, self.fov_time) = (from.fov, from.fov_target, from.fov_blend, from.fov_time);
        (self.steps_since_frame, self.camera_local) = (from.steps_since_frame, from.camera_local);
        self.shots_pending = from.shots_pending.clone();
        (self.poses, self.pose_position, self.pose_rotation, self.pose_camera, self.lean_rest) = (from.poses.clone(), from.pose_position, from.pose_rotation, from.pose_camera, from.lean_rest);
        (self.turn_away, self.view, self.weapon_root, self.aim_pose) = (from.turn_away.clone(), from.view, from.weapon_root, from.aim_pose);
        (self.head, self.head_control, self.head_control_seen) = (from.head, from.head_control, from.head_control_seen);
        (self.scope_rotation, self.sight_angle) = (from.scope_rotation, from.sight_angle);
        (self.blind, self.blind_clock, self.blind_asked, self.blind_value, self.blind_strength, self.blind_camera) =
            (from.blind, from.blind_clock, from.blind_asked, from.blind_value, from.blind_strength, from.blind_camera);
        (self.left_shoulder, self.left_shoulder_curve) = (from.left_shoulder, from.left_shoulder_curve);
    }

    pub fn new(settings: HandsSettings) -> Self {
        let d = &settings.data;
        let (sway_strength, displacement_strength, aiming_speed) = settings.by_weight();
        let mut hands_position = Spring::new(d.hands_position);
        hands_position.damping = settings.hand_damping;
        let mut hands = Self {
            recoil: Recoil::new(settings.recoil.clone(), settings.seed),
            breath: Breath::new(d.breath.clone(), settings.breath_gain, settings.seed.wrapping_add(1)),
            hand_shake: HandShake::new(d.hand_shake.clone(), settings.seed.wrapping_add(4)),
            combo_on: false,
            pose: 2,
            time: 0.0,
            camera_smooth: Blender::new(d.camera_smooth_steady, d.camera_smooth_blend_speed),
            // Starts standing still, where the lean reaches the weapon fully.
            tilt_blend: Blender::new(1.0, TILT_BLEND_SPEED),
            possible_tilt: 1.0,
            camera_follow: None,
            camera_follow_target: [0.0, 0.0, 0.0, 1.0],
            camera_follow_speed: settings.camera_follow_range[0],
            fov: settings.field_of_view,
            fov_target: settings.field_of_view,
            fov_blend: 0.0,
            fov_time: 1.0,
            steps_since_frame: 0,
            left_shoulder: LeftShoulderState::default(),
            left_shoulder_curve: 0.0,
            shoulder_root: ([0.0; 3], [0.0, 0.0, 0.0, 1.0]),
            shots_pending: Vec::new(),
            camera_local: d.camera_offset,
            poses: Vec::new(),
            pose_position: [0.0; 3],
            pose_rotation: [0.0; 3],
            pose_camera: [0.0; 3],
            lean_rest: 0.0,
            weapon_flip_speed: 1.0,
            closer: settings.held_closer,
            folded: false,
            sight_noted: None,
            sight: SightLine::default(),
            sight_angle: 0.0,
            scope_rotation: [0.0, 0.0, 0.0, 1.0],
            retake_line: false,
            muzzle: None,
            turn_away: TurnAway::default(),
            view: [0.0; 2],
            weapon_root: settings.weapon_root,
            point_of_view: PointOfView::FirstPerson,
            mask: d.mask,
            motion: true,
            recoil_on: true,
            aim_pose: Blender::new(0.0, d.aim_pose_speed),
            head: [0.0; 3],
            head_control: false,
            head_control_seen: false,
            blind: Blender::new(0.0, d.blind_fire.speed),
            blind_clock: 0.0,
            blind_asked: None,
            blind_value: 0.0,
            blind_strength: 1.0,
            blind_camera: [0.0; 3],
            mounted: None,
            mount_strategy: false,
            bipod_used: false,
            aim_weight: 0.0,
            rig_shift: [0.0; 3],
            tactical_blend: 0.0,
            body_camera: None,
            hands_position,
            hands_rotation: Spring::new(d.hands_rotation),
            camera_rotation: Spring::new(d.camera_rotation),
            camera_position: Spring::new(d.camera_position),
            // The spring as the load its settings carry softens it (sound
            // arms; `update_sway_spring` once health is heard).
            sway: {
                let share = settings.overweight.clamp(0.0, 1.0);
                Sway::new(lerp(settings.sway_frequency, settings.sway_damaged[0], share), lerp(settings.sway_damping, settings.sway_damaged[1], share))
            },
            force_react: ForceReact::default(),
            position_velocity: [0.0; 3],
            position_velocity_rate: [0.0; 3],
            position_acceleration: [0.0; 3],
            position_acceleration_rate: [0.0; 3],
            last_position_velocity: [0.0; 3],
            rotation_velocity: [0.0; 2],
            rotation_velocity_rate: [0.0; 2],
            rotation_acceleration: [0.0; 2],
            rotation_acceleration_rate: [0.0; 2],
            last_rotation_velocity: [0.0; 2],
            rotation_velocity_sum: [0.0; 2],
            rotation_acceleration_sum: [0.0; 2],
            last_forward: None,
            rig_view: None,
            walk_clocks: d.walk_normal.iter().map(|e| e.time_shift).collect(),
            walk_clocks_lame: d.walk_lame.iter().map(|e| e.time_shift).collect(),
            lame: false,
            walk_clocks_load: d.walk_overweight.iter().map(|e| e.time_shift).collect(),
            walk_clocks_load_lame: d.walk_overweight_lame.iter().map(|e| e.time_shift).collect(),
            arms_sway: 0.0,
            walking: false,
            step_frequency: d.walk_step_frequency,
            sway_strength,
            displacement_strength,
            aiming_speed,
            aim_sway_strength: 0.0,
            aim_sway_direction: [0.0; 3],
            aim_sway_since: f32::INFINITY,
            aim_sway_random: crate::base::unity_random::UnityRandom::new(settings.seed.wrapping_add(3) as i32),
            aim_sway_override: None,
            manual: None,
            fov_kept_aimed: false,
            aiming: false,
            accumulator: 0.0,
            settings,
        };
        hands.aim_sway_strength = hands.settings.aim_sway_strength();
        // The sights' eye goes by the weapon's squeeze at the field of view.
        let scale = hands.fov_scale();
        if let Some(sights) = hands.settings.sights.as_mut() {
            sights.set_scale(scale);
        }
        // The hip penalty as the controller works it out when it is made
        // (`UpdateHipInaccuracy`), no combo unit on until told.
        hands.update_hip_inaccuracy(false);
        hands
    }
}
