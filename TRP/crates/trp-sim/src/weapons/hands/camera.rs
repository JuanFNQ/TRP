//! The frame on the clock, the aim sway and the camera's position and rotation.

use super::*;

impl Hands {
    /// The camera for one rendered frame of `frame_dt` seconds.
    ///
    /// `animated` is the local rotation of the weapon's animated camera
    /// bone; `aim` says where the sight is and is only used while aiming.
    ///
    /// The game gives its per-frame camera code the physics time that
    /// elapsed when a physics step ran in the frame, and the frame's own
    /// time otherwise. Above the physics rate the camera therefore moves
    /// faster than the clock; that is kept.
    pub fn frame(&mut self, frame_dt: f32, animated: [f32; 4], aim: Option<AimPoint>) -> CameraPose {
        self.frame_on_clock(frame_dt, frame_dt, animated, aim)
    }

    /// `frame` for a host that keeps the player's own clock apart from the
    /// engine's (the movement's `step_on_clock`): the camera's place and
    /// turn ease by `arms_dt`, the player's frame time read in whole
    /// milliseconds, on a frame with no physics step (the game's weapon
    /// motion is given the player's arms update's time there,
    /// `Player._armsTime`; the physics steps' time otherwise), and the field
    /// of view by `frame_dt`, the engine's (`CameraManager.SetFovCoroutine`
    /// goes by `Time.deltaTime`).
    pub fn frame_on_clock(&mut self, frame_dt: f32, arms_dt: f32, animated: [f32; 4], aim: Option<AimPoint>) -> CameraPose {
        let steps = std::mem::take(&mut self.steps_since_frame);
        let dt = if steps > 0 { steps as f32 * self.settings.fixed_timestep } else { arms_dt };
        self.aim_sway_frame(frame_dt);
        self.breath.pass_frame(frame_dt);
        self.blind_fire_rest(frame_dt);
        // The aiming weight (`UpdateAimWeight`), every frame.
        let rate = if self.aiming { self.camera_aim_rate() } else { self.settings.data.camera_smooth_out };
        self.aim_weight = lerp(self.aim_weight, if self.aiming { 1.0 } else { 0.0 }, rate * dt);
        // With the left shoulder's curve up, and blind fire's blender at 0,
        // the camera's pass runs only late in the frame, by the frame's own
        // time (`ProcessEffectors` leaves it to `LateTransformations`).
        let late_only = self.left_shoulder_curve > 0.0 && self.blind_value == 0.0;
        let camera_dt = if late_only { frame_dt } else { dt };
        let mut pose = CameraPose {
            position: if self.mount_strategy { self.mounted_camera_position(camera_dt, aim) } else { self.camera_position(camera_dt, aim) },
            rotation: self.camera_rotation(camera_dt, animated),
            field_of_view: self.field_of_view(frame_dt),
        };
        // While blind fire's blender is off 0 the camera's pass runs again
        // late in the frame, by the frame's own time
        // (`ProceduralWeaponAnimation.LateTransformations`): its place eases
        // a second time, the position spring added again, and its turn's
        // follow of the recoil is blended again. PROVISIONAL: read from code;
        // in the recordings the camera rests then and the pass is unseen.
        if self.blind_value != 0.0 {
            pose.position = self.camera_position(frame_dt, aim);
            pose.rotation = self.camera_rotation(frame_dt, animated);
        }
        // Then the player's late update turns the sights to the zeroing
        // point (`StartFovCoroutine`: `PreCalibrate`, `Calibrate`, in first
        // person), which a launcher's line follows.
        self.calibrate();
        // And the weapon's controller sets off the frame's shots.
        self.feel_shots();
        pose
    }

    /// A heavy weapon's sway (weapon-handling spec, "Aim sway"): each frame
    /// the sights are up, the sway spring's velocity given the direction
    /// times what is left of it, from whole as they came up to nothing at
    /// `SwayFalloff` over the aiming speed a second, by the engine's clock
    /// (`_aimSwayBlender`, `SwaySpring.ApplyVelocity`, in the camera's
    /// frame: a kick a frame, not a physics step).
    fn aim_sway_frame(&mut self, frame_dt: f32) {
        if self.aim_sway_strength > 0.0 && self.aiming {
            let speed = self.settings.data.aim_sway.falloff / self.aiming_speed;
            let left = (1.0 - self.aim_sway_since * speed).clamp(0.0, 1.0);
            if left > 0.0 {
                for i in 0..3 {
                    self.sway.kick(i, self.aim_sway_direction[i] * left);
                }
            }
        }
        self.aim_sway_since += frame_dt;
    }

    /// The aim sway's strength, and the direction it was last drawn in.
    pub fn aim_sway(&self) -> (f32, [f32; 3]) {
        (self.aim_sway_strength, self.aim_sway_direction)
    }

    /// The zeroing's calibration (`crate::sights::Sights::calibrate`), in
    /// first person, and a launcher's line as it leaves it.
    fn calibrate(&mut self) {
        if self.outside() {
            return;
        }
        let aiming = self.aiming;
        let Some(sights) = self.settings.sights.as_mut() else { return };
        sights.calibrate(aiming);
        if aiming && sights.launcher() {
            self.sight = self.line_in_force(self.sight_noted);
        }
    }

    /// Moves the camera toward its rest beside the weapon, or toward the
    /// sight line while aiming.
    pub(super) fn camera_position(&mut self, dt: f32, aim: Option<AimPoint>) -> [f32; 3] {
        let d = &self.settings.data;
        // The sight is the target only with blind fire's blender at 0
        // (`CalculateCameraPosition`): sights brought up on blind fire's
        // way out wait for it.
        let target = match aim.filter(|_| self.aiming && self.blind_value.abs() < f32::EPSILON) {
            Some(aim) => {
                // Sideways and up: the sight's aim bone. Forward: the
                // weapon's aim plane, drawn in with the weapon at wide
                // fields of view and then pushed out again a little.
                // The plane is the sight's (`crate::sights::SightEye`): an
                // optic's by the far plane and its own distance.
                let scale = self.fov_scale();
                let eye = self.sights().map_or(crate::weapons::sights::SightEye::iron(self.settings.aim_plane_depth), crate::weapons::sights::Sights::eye);
                let plane = aim.origin[2] + aim.barrel[2] * eye.depth * scale;
                // A weapon drawn back from an obstacle takes the eye back
                // with it. And the eye is put on the iron sights' line:
                // up from the sight's aim bone, and along the view.
                let [along, up] = self.sight.shift;
                // With the left shoulder's curve up the eye comes a little
                // nearer at a wide field of view (`_aimLeftStanceAdditionalOffset`,
                // -0.06, times one less the squeeze).
                let shoulder = if self.left_shoulder_curve > 0.0 { LEFT_SHOULDER_AIM_NEARER * (1.0 - scale) } else { 0.0 };
                let depth = plane + eye.compensation(scale) - self.turn_away.position[1] + along + shoulder;
                if self.recoil.auto_fire() {
                    // In a run of automatic fire the target is slid along
                    // the sight's own line to that depth, not straight
                    // along the view, so that the eye stays on the line as
                    // the recoil turns the weapon (`CheckAutoFire`; a way
                    // of no length leaves the bone where it is, as the
                    // engine's `normalized` does).
                    let size = (aim.line[0] * aim.line[0] + aim.line[1] * aim.line[1] + aim.line[2] * aim.line[2]).sqrt();
                    let way = if size > 1e-5 { aim.line.map(|v| v / size) } else { [0.0; 3] };
                    let reach = depth - aim.bone[2];
                    [aim.bone[0] + way[0] * reach, aim.bone[1] + way[1] * reach + up, aim.bone[2] + way[2] * reach]
                } else {
                    [aim.bone[0], aim.bone[1] + up, depth]
                }
            }
            None => [d.camera_offset[0], d.camera_offset[1], d.camera_offset[2] + self.turn_away.camera],
        };
        let (sideways, forward) = if self.aiming {
            (
                self.camera_aim_rate(),
                1.0 + self.hands_position.current[1] * 100.0 + self.turn_away.position[1] * 10.0,
            )
        } else {
            (d.camera_smooth_out, d.camera_smooth_out)
        };
        let c = self.camera_local;
        let spring = self.camera_position.current;
        // The game blends from where the camera was drawn, spring and all.
        self.camera_local = [
            lerp(c[0], target[0], dt * sideways) + spring[0],
            lerp(c[1], target[1], dt * sideways) + spring[1],
            lerp(c[2], target[2], d.camera_smooth_time * dt * forward) + spring[2],
        ];
        self.camera_local
    }

    /// The mount's camera place (`MountedWeaponCamera`): the camera's offset
    /// from the weapon's root (which a mount moves) blended toward the
    /// sight's eye by the aiming weight, the turn-away's pull not taken off;
    /// reached across and up at the mount's camera speed (at once) and along
    /// at the usual easing times it.
    fn mounted_camera_position(&mut self, dt: f32, aim: Option<AimPoint>) -> [f32; 3] {
        let d = &self.settings.data;
        let shift = self.rig_shift;
        let resting: [f32; 3] = std::array::from_fn(|k| shift[k] + d.camera_offset[k]);
        let eye = match aim {
            Some(aim) => {
                let scale = self.fov_scale();
                let eye = self.sights().map_or(crate::weapons::sights::SightEye::iron(self.settings.aim_plane_depth), crate::weapons::sights::Sights::eye);
                let plane = aim.origin[2] + aim.barrel[2] * eye.depth * scale;
                let [along, up] = self.sight.shift;
                let depth = plane + eye.compensation(scale) + along;
                let at = if self.recoil.auto_fire() {
                    let size = (aim.line[0] * aim.line[0] + aim.line[1] * aim.line[1] + aim.line[2] * aim.line[2]).sqrt();
                    let way = if size > 1e-5 { aim.line.map(|v| v / size) } else { [0.0; 3] };
                    let reach = depth - aim.bone[2];
                    [aim.bone[0] + way[0] * reach, aim.bone[1] + way[1] * reach + up, aim.bone[2] + way[2] * reach]
                } else {
                    [aim.bone[0], aim.bone[1] + up, depth]
                };
                std::array::from_fn(|k| shift[k] + at[k])
            }
            None => resting,
        };
        let weight = self.aim_weight;
        let target: [f32; 3] = std::array::from_fn(|k| lerp(resting[k], eye[k], weight));
        let speed = self.settings.mount_settings.camera_speed;
        let along_rate = if self.aiming { 1.0 + self.hands_position.current[1] * 100.0 + self.turn_away.position[1] * 10.0 } else { d.camera_smooth_out };
        let c = self.camera_local;
        let spring = self.camera_position.current;
        self.camera_local = [
            lerp(c[0], target[0], dt * speed) + spring[0],
            lerp(c[1], target[1], dt * speed) + spring[1],
            lerp(c[2], target[2], d.camera_smooth_time * dt * along_rate * speed) + spring[2],
        ];
        self.camera_local
    }

    /// Moves the camera by `push`, in the frame it is placed in, and says
    /// where that leaves it. Last in each frame the game pushes the
    /// camera's box out of the world (`Movement::camera_push`), and what it
    /// moves is the camera itself: the next frame's easing starts from
    /// there, so a camera whose target is in the ground rests on it.
    pub fn push_camera(&mut self, push: [f32; 3]) -> [f32; 3] {
        for (at, by) in self.camera_local.iter_mut().zip(push) {
            *at += by;
        }
        self.camera_local
    }

    /// The camera's rotation beside the weapon root, advanced by one
    /// rendered frame.
    ///
    /// `animated` is the local rotation of the weapon's animated camera
    /// bone. With the body's camera bone it rests at a half turn about x,
    /// which the game undoes at the end.
    pub(super) fn camera_rotation(&mut self, frame_dt: f32, animated: [f32; 4]) -> [f32; 4] {
        let half_turn = quat_euler([180.0, 0.0, 0.0]);
        // The body's own camera bone moves with the body's clips; without a
        // body it stays at rest.
        let bones = quat_mul(animated, self.body_camera.unwrap_or(self.settings.data.body_camera_rotation));
        // The mount's camera takes the bones' turn whole (`MountedWeaponCamera`);
        // the first person's takes the head bobbing's share of it, less the
        // tactical reload's blend's (`LerpCamera`).
        let share = self.settings.head_bobbing * (1.0 - self.tactical_blend);
        let bone = if self.mount_strategy { bones } else { quat_lerp(quat_euler([-180.0, 0.0, 0.0]), bones, share) };
        // The head's free look is added to the spring's figure, and while
        // the head is turned at all the camera does not follow the
        // weapon's climb (`AddHandRecoilRotateToCamera`).
        let spring = self.camera_rotation.value();
        let looking = self.head != [0.0; 3];
        let turned: [f32; 3] = if looking { std::array::from_fn(|i| spring[i] + self.head[i]) } else { spring };
        let mut rotation = quat_mul(quat_mul(bone, quat_euler(turned)), half_turn);
        if !looking {
            rotation = self.follow_recoil(frame_dt, rotation);
        }
        // The camera's own kick is added to its angles, not composed.
        let kick = self.recoil.camera_angles();
        let angles = quat_to_euler(rotation);
        quat_euler(std::array::from_fn(|i| angles[i] + kick[i]))
    }

    /// Blends the camera toward the weapon's climb while aiming, so that
    /// the sight picture rises with the muzzle. From the hip the same blend
    /// only smooths the camera.
    fn follow_recoil(&mut self, frame_dt: f32, rotation: [f32; 4]) -> [f32; 4] {
        let [slowest, fastest] = self.settings.camera_follow_range;
        let step = self.settings.camera_follow_step * frame_dt;
        let climb = self.recoil.hand_rotation.current;
        let auto_fire = self.recoil.hand_rotation.auto_fire_on;
        // `Mathf.Clamp` with the bounds as the weapon gives them.
        let limit = |speed: f32| if speed < slowest { slowest } else { speed.min(fastest) };

        let (target, blend) = if !self.aiming {
            self.camera_follow_speed = limit(self.camera_follow_speed - step);
            self.camera_follow_target = rotation;
            (rotation, fastest)
        } else if auto_fire && climb == [0.0; 3] {
            // Nothing to follow yet; the game leaves the camera alone.
            return rotation;
        } else if auto_fire && self.recoil.hand_rotation.stable_on {
            // Held automatic fire: keep the last target, taking only one
            // component from the camera.
            self.camera_follow_speed = limit(self.camera_follow_speed + step);
            let mut target = self.camera_follow_target;
            target[2] = rotation[2];
            (target, self.camera_follow_speed)
        } else {
            let mut base = rotation;
            base[1] = 0.0;
            let target = quat_mul(base, quat_euler(climb));
            let change = if auto_fire { step } else { -step };
            self.camera_follow_speed = limit(self.camera_follow_speed + change);
            self.camera_follow_target = target;
            (target, self.camera_follow_speed)
        };
        // The first frame has nothing to blend from and lands on the target.
        let result = self.camera_follow.map_or(quat_lerp(target, target, 1.0), |from| quat_lerp(from, target, blend));
        self.camera_follow = Some(result);
        result
    }

    /// Offset the camera spring adds to the camera's position.
    pub fn camera_offset(&self) -> [f32; 3] {
        self.camera_position.current
    }

    /// The four springs' values, for comparing against the running game.
    pub fn spring_values(&self) -> [[f32; 3]; 4] {
        [
            self.hands_position.value(),
            self.hands_rotation.value(),
            self.camera_rotation.value(),
            self.camera_position.current,
        ]
    }
}

/// World vector into the view's axes: undo the yaw, then the pitch.
pub(super) fn to_view(v: [f32; 3], yaw: f32, pitch: f32) -> [f32; 3] {
    let (sy, cy) = yaw.to_radians().sin_cos();
    let (sp, cp) = pitch.to_radians().sin_cos();
    let x = v[0] * cy - v[2] * sy;
    let z = v[0] * sy + v[2] * cy;
    [x, v[1] * cp + z * sp, -v[1] * sp + z * cp]
}

/// View vector into world axes: the pitch, then the yaw.
pub(super) fn from_view(v: [f32; 3], yaw: f32, pitch: f32) -> [f32; 3] {
    let (sy, cy) = yaw.to_radians().sin_cos();
    let (sp, cp) = pitch.to_radians().sin_cos();
    let y = v[1] * cp - v[2] * sp;
    let z = v[1] * sp + v[2] * cp;
    [v[0] * cy + z * sy, y, -v[0] * sy + z * cy]
}
