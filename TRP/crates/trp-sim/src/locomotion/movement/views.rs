//! What the movement shows the rest: capsule, poses, camera frames, overlaps.

use super::*;

impl Movement {
    /// Height of the collision capsule for the current stance.
    pub fn capsule_height(&self) -> f32 {
        if matches!(self.state, State::ToProne | State::ProneIdle | State::ProneMove | State::Roll) {
            0.3
        } else {
            lerp(1.2, 1.6, self.pose_level)
        }
    }

    /// The space the character's collision takes up over its feet just
    /// now (`Capsule`): from the feet up to the stance's height, but for
    /// a vault or a climb, whose capsule the game shrinks from below.
    pub fn capsule(&self) -> Capsule {
        if matches!(self.state, State::Vault | State::Climb) {
            let [height, centre] = self.vault.capsule;
            Capsule { underside: (centre - height / 2.0).max(0.0), top: centre + height / 2.0, lift: false }
        } else {
            Capsule { underside: 0.0, top: self.capsule_height(), lift: true }
        }
    }

    /// The game's capsule as its code sets it: the height, and how far
    /// over the feet the centre is. By the stance (`SetPoseLevel`: 1.2 to
    /// 1.6 m high with the centre 0.6 to 0.8 up, and 0.3 with the centre
    /// 0.35 up lying down; the game's own controller for the player has no
    /// skin to add), but for a vault or a climb under way.
    pub fn capsule_figures(&self) -> [f32; 2] {
        if matches!(self.state, State::Vault | State::Climb) {
            self.vault.capsule
        } else if matches!(self.state, State::ToProne | State::ProneIdle | State::ProneMove | State::Roll) {
            [0.3, 0.35]
        } else {
            [lerp(1.2, 1.6, self.pose_level), lerp(0.6, 0.8, self.pose_level)]
        }
    }

    /// How far the game's capsule reaches under the feet: a capsule shorter
    /// than it is wide is a ball of its radius about its middle, so lying
    /// down (0.3 m high, the middle 0.35 up, 0.366 round) it reaches 1.6 cm
    /// under them; standing and crouched, nothing.
    pub(super) fn reach_under_feet(&self) -> f32 {
        if matches!(self.state, State::Vault | State::Climb) {
            return 0.0;
        }
        let [height, middle] = self.capsule_figures();
        ((height / 2.0).max(room::RADIUS) - middle).max(0.0)
    }

    /// What the animator has been told of a vault or a climb: the
    /// obstacle's height, its length, the drop behind it and the forward
    /// speed it was measured at; the pace of the move's clips; and the two
    /// flags that ask for a vault and for a climb.
    pub fn vault_parameters(&self) -> ([f32; 4], f32, [bool; 2]) {
        (self.vault_figures, self.vaulting_speed, [self.do_vault, self.do_climb])
    }

    /// Puts the figures the animator holds of the last vault or climb where
    /// a character met some way into a raid has them: they stay as the
    /// last move left them.
    pub fn set_vault_parameters(&mut self, figures: [f32; 4], pace: f32) {
        (self.vault_figures, self.vaulting_speed) = (figures, pace);
    }

    /// The view's pitch as it is to be shown. In a vault or a climb that
    /// takes a hand the view is the move's own (vaulting spec, "The
    /// view"): the pitch it began with, brought up to at most 50 degrees
    /// down by the middle of the move and back by its end, by the clips'
    /// `Mouse_look` curve. The mouse does not move it meanwhile.
    pub fn shown_pitch(&self) -> f32 {
        self.pitch_as_shown(self.pitch)
    }

    pub(super) fn pitch_as_shown(&self, pitch: f32) -> f32 {
        /// The furthest down the view is left in such a move, degrees.
        const DEEPEST: f32 = 50.0;
        if self.vault.hands && matches!(self.state, State::Vault | State::Climb) {
            lerp(pitch, pitch.min(DEEPEST), self.body.mouse_look())
        } else {
            pitch
        }
    }

    /// How far the view may pitch just now: up, then down.
    pub fn pitch_range(&self) -> [f32; 2] {
        self.pitch_limit
    }

    /// Horizontal speed in metres per second.
    pub fn ground_speed(&self) -> f32 {
        length([self.velocity[0], self.velocity[2]])
    }

    /// The velocity the controller asked for this step, before the world
    /// had its say.
    pub fn input_velocity(&self) -> [f32; 3] {
        self.velocity_before_limit
    }

    /// Turns the body's axes into those of the frame the first-person rig
    /// hangs in: at the ribcage, turned to where the player looks.
    pub fn body_in_view(&self) -> [f32; 4] {
        quat_mul(quat_mul(quat_axis(0, -self.pitch), quat_axis(1, self.body_yaw - self.yaw)), self.body_tilt)
    }

    /// Where the ribcage is from the feet, in the axes of the way the
    /// body faces (x right, y up, z ahead): what the body's animation
    /// gives, laid to the ground if the body is, and held back by the lag
    /// in the body's height. The first-person rig and the camera hang there.
    /// Where to draw the character's feet. It differs from `position` only
    /// in a jump. The game moves a jumping character by the player's own
    /// clock, which counts whole milliseconds and so runs up to a
    /// millisecond behind the frames and catches up in steps (8, 8 and 9
    /// ms at 120 frames a second): the travel through the air is uneven
    /// from frame to frame, where travel on the ground, which is the
    /// animator's, is even. This is the place the same jump has reached by
    /// the frames' own time, which is that jump without the ripple. Level
    /// travel only: the eye's height already trails the feet's.
    pub fn drawn_position(&self) -> [f32; 3] {
        if self.state != State::Jump {
            return self.position;
        }
        let behind = self.clock_lag - self.jump.lag_at_start;
        let [x, y, z] = self.position;
        [x + self.velocity[0] * behind, y, z + self.velocity[2] * behind]
    }

    /// Which way to draw the body facing. It differs from `body_yaw` only
    /// in a jump, and for the reason `drawn_position` does: there the game
    /// turns the body after the view by the player's whole-millisecond
    /// clock, so on frames of even length the body turns by uneven amounts
    /// (an eighth more on every third frame at 120 a second), and the
    /// weapon it carries in the air shakes against the view. This is where
    /// the same turn has got to by the frames' own time.
    pub fn drawn_body_yaw(&self) -> f32 {
        if self.state != State::Jump {
            return self.body_yaw;
        }
        self.body_yaw + self.jump.turn_rate * (self.clock_lag - self.jump.lag_at_start)
    }

    /// `body_in_view` for a host that draws: with the body facing as it
    /// is to be drawn (`drawn_body_yaw`).
    pub fn drawn_body_in_view(&self) -> [f32; 4] {
        quat_mul(quat_mul(quat_axis(0, -self.pitch), quat_axis(1, self.drawn_body_yaw() - self.yaw)), self.body_tilt)
    }

    pub fn ribcage(&self) -> [f32; 3] {
        let mut at = quat_rotate(self.body_tilt, self.body.ribcage());
        at[1] += self.body_height_offset;
        at
    }

    /// Where the first-person camera's own object is, as `ribcage` gives the
    /// ribcage (`Body::camera_container`): the camera hangs here, turned to
    /// the view. It is the ribcage's place but where a clip shifts the
    /// ribcage on its spine, as blind fire's do.
    pub fn camera_container(&self) -> [f32; 3] {
        let mut at = quat_rotate(self.body_tilt, self.body.camera_container());
        at[1] += self.body_height_offset;
        at
    }

    /// The angle from the body round to the view and the view's pitch
    /// over the ground's, as last noted: what the animator's
    /// `DeltaRotation` and `Aim_angle` stand at when the frame is over.
    pub fn noted_angles(&self) -> [f32; 2] {
        [self.hands_to_body, self.aim_angle]
    }

    /// The view the last frame's pose of the body was made for, as its
    /// yaw and pitch. The body's animation bends and turns the upper body
    /// by the angles the game noted when it last turned the body, which is
    /// before the frame's own turn of the view: the ribcage, and the eye
    /// on it, trail the view by a frame's turn (movement spec, "Looking").
    pub fn posed_for(&self) -> [f32; 2] {
        let [delta_rotation, aim_angle] = self.body.posed_for();
        [self.drawn_body_yaw() + delta_rotation, self.pitch + aim_angle - self.aim_angle]
    }

    /// `ribcage` as it would be now had the body's pose been made for the
    /// view at another yaw and pitch, nothing else different. For a host
    /// that draws between the simulation's frames and shows the view's
    /// turning ahead of them: with this it can put the ribcage where the
    /// view it shows would have it, where the frames it draws the rest of
    /// the body from were posed for the view as it stood some frames ago.
    /// `ribcage_posed_for(posed_for())` is `ribcage()`.
    pub fn ribcage_posed_for(&self, look: [f32; 2]) -> [f32; 3] {
        let delta_rotation = delta_angle(self.drawn_body_yaw(), look[0]);
        let aim_angle = self.aim_angle + look[1] - self.pitch;
        let mut at = quat_rotate(self.body_tilt, self.body.ribcage_posed_for(delta_rotation, aim_angle));
        at[1] += self.body_height_offset;
        at
    }

    /// Puts a bone the body gives in its own axes (`Body::arm_bone`,
    /// `Body::weapon_bone`) where it is from the feet, as `ribcage` does
    /// the ribcage: laid to the ground with the body and held back by the
    /// lag in its height.
    pub fn in_player(&self, bone: ([f32; 3], [f32; 4])) -> ([f32; 3], [f32; 4]) {
        let mut at = quat_rotate(self.body_tilt, bone.0);
        at[1] += self.body_height_offset;
        (at, quat_mul(self.body_tilt, bone.1))
    }

    /// The clock stamina's charges go by: the frames' own lengths summed.
    pub fn frame_clock(&self) -> f32 {
        self.frame_clock
    }

    /// Whether the movement state lets the sights be up: not in a crawl,
    /// a sprint or a jump. For `Firearm::set_aim`.
    pub fn can_aim(&self) -> bool {
        !self.settings.no_aim_states.contains(&self.state.number())
    }

    /// Whether the character has been off the ground long enough for the
    /// sights to be put down (movement spec, "Falling"). The game does
    /// that on every such frame, after the movement's update, through the
    /// aim key's own call: a host calls `Firearm::set_aim(false, ..)` for
    /// as long as this holds.
    pub fn long_fall(&self) -> bool {
        self.freefall_time > LONG_FALL
    }

    /// The frame the first-person rig hangs in, in the world's axes: where
    /// its origin is (the body's ribcage) and how it is turned (to where
    /// the player looks).
    pub fn rig_frame(&self) -> ([f32; 3], [f32; 4]) {
        let ribcage = quat_rotate(quat_axis(1, self.body_yaw), self.ribcage());
        (
            std::array::from_fn(|k| self.position[k] + ribcage[k]),
            quat_mul(quat_axis(1, self.yaw), quat_axis(0, self.shown_pitch())),
        )
    }

    /// The frame the first-person camera hangs in, in the world's axes, as
    /// `rig_frame` gives the rig's: at the camera's own object
    /// (`camera_container`), turned to where the player looks. The two are
    /// one but where a clip shifts the ribcage on its spine (blind fire).
    pub fn camera_frame(&self) -> ([f32; 3], [f32; 4]) {
        let container = quat_rotate(quat_axis(1, self.body_yaw), self.camera_container());
        (
            std::array::from_fn(|k| self.position[k] + container[k]),
            quat_mul(quat_axis(1, self.yaw), quat_axis(0, self.shown_pitch())),
        )
    }

    /// What the world does to the first-person camera's box ("The
    /// camera's collider" in the weapon-handling spec): how far the camera
    /// has to move for its box to be clear of everything solid, in the
    /// frame the rig hangs in. `position` and `rotation` are the camera as
    /// the weapon motion has just placed it (`Hands::frame`); `size` and
    /// `centre` are the box on the player prefab, in the camera's own axes.
    /// For `Hands::push_camera`, last in a frame.
    pub fn camera_push(
        &self,
        world: &dyn World,
        position: [f32; 3],
        rotation: [f32; 4],
        size: [f32; 3],
        centre: [f32; 3],
    ) -> [f32; 3] {
        /// What the game adds to each push, in metres.
        const MARGIN: f32 = 0.001;
        let (origin, turn) = self.camera_frame();
        let facing = quat_mul(turn, rotation);
        let (beside, middle) = (quat_rotate(turn, position), quat_rotate(facing, centre));
        let at: [f32; 3] = std::array::from_fn(|k| origin[k] + beside[k] + middle[k]);
        let push = world.push_out(at, size.map(|s| s / 2.0), facing, MARGIN);
        quat_rotate([-turn[0], -turn[1], -turn[2], turn[3]], push)
    }

    /// How deep the weapon would stick into something, in metres
    /// ("Turn-away" in the weapon-handling spec). `line` is the weapon's
    /// line in the rig's frame (`Hands::weapon_line`). If it hits, the
    /// depth is what is left of it past the hit. If it does not, a second
    /// line is cast from the character's own axis, at the height the
    /// first starts at, out to that start: a hit there means the weapon's
    /// near end is through something already, and counts as the whole
    /// length. For `Hands::set_overlap` and `Firearm::overlap`, once a
    /// frame, when the weapon has been placed.
    pub fn weapon_overlap(&self, world: &dyn World, line: ([f32; 3], [f32; 3])) -> f32 {
        self.placed_weapon_overlap(world, line, [0.0; 3])
    }

    /// `weapon_overlap` with the rig moved by `apart` in its own frame (a
    /// mount's move of the rig onto its point: `mounted_rig_apart`). The game
    /// measures from the hands as they are placed (`WeaponOverlapping`: the
    /// hands' place through the moved root), so a weapon resting on a sill
    /// runs clear of it (recorded: the overlap nought on every mounted frame
    /// of `courses8`'s window and corners and `courses7`'s sill).
    pub fn placed_weapon_overlap(&self, world: &dyn World, line: ([f32; 3], [f32; 3]), apart: [f32; 3]) -> f32 {
        self.placed_weapon_overlap_met(world, line, apart).0
    }

    /// `placed_weapon_overlap`, and whether what the weapon met is another
    /// character (`OverlapLn`'s `overlapsWithPlayer`, written only where a
    /// line hits): the turn-away then goes by its own threshold, and the
    /// movement's overlap is nought (`MovementContext.OverlapDepth`).
    pub fn placed_weapon_overlap_met(&self, world: &dyn World, line: ([f32; 3], [f32; 3]), apart: [f32; 3]) -> (f32, bool) {
        let line = (std::array::from_fn(|k| line.0[k] + apart[k]), std::array::from_fn(|k| line.1[k] + apart[k]));
        let (origin, turn) = self.rig_frame();
        let out = |at: [f32; 3]| -> [f32; 3] {
            let turned = quat_rotate(turn, at);
            std::array::from_fn(|k| origin[k] + turned[k])
        };
        let (from, to) = (out(line.0), out(line.1));
        let length = (0..3).map(|k| (to[k] - from[k]).powi(2)).sum::<f32>().sqrt();
        if let Some((hit, character)) = world.line_meets(from, to) {
            return ((length - hit).max(0.0), character);
        }
        let axis = [self.position[0], from[1], self.position[2]];
        match world.line_meets(axis, from) {
            Some((_, character)) => (length, character),
            None => (0.0, false),
        }
    }

    /// The weight's two notices as it now stands, encumbered and
    /// over-encumbered (`MovementSettings::encumbered`): what the health is
    /// told (`Physical.EncumberedChanged`, `OverEncumberedChanged`).
    pub fn encumbered(&self) -> (bool, bool) {
        self.settings.encumbered()
    }

    /// Whether the movement state lets the hands act at all
    /// (`Player.StateIsSuitableForHandInput`: not one of the install's
    /// `UnsuitableStates`).
    pub fn state_suits_hands(&self) -> bool {
        !self.settings.no_hands_states.contains(&self.state.number())
    }

    /// Whether the movement state lets the hands take a press of the
    /// trigger: not in a sprint or a crawl. For `Firearm::set_trigger`.
    pub fn can_press_trigger(&self) -> bool {
        // Nor in a vault or a climb of something high enough to take a
        // hand (`FirearmController.CheckVaulting`).
        let s = &self.settings.vaulting;
        let clear = if self.do_vault { s.vault.max_without_hand } else { s.climb.max_without_hand };
        let vaulting = self.is_vaulting && self.vault.height > clear;
        !vaulting && !self.settings.no_hands_states.contains(&self.state.number())
    }

    /// How tired the character is and whether it holds its breath, for
    /// the weapon's breathing (`HandsInput::wind`).
    pub fn wind(&self) -> crate::weapons::breath::Wind {
        crate::weapons::breath::Wind {
            stamina: self.stamina.breath_stamina(),
            oxygen: self.stamina.breath_oxygen(),
            held: self.stamina.holding_breath(),
            tremor: self.conditions.tremor_on(),
            fracture: self.conditions.arms_fractured(),
        }
    }

    /// Whether a vault or a climb has taken the sights down since this
    /// was last asked: one over something high enough to take a hand does,
    /// through the aim key's own call. A host answers with
    /// `Firearm::set_aim(false, ..)`.
    pub fn take_aim_taken(&mut self) -> bool {
        std::mem::take(&mut self.aim_taken)
    }

    /// Whether a movement state that does not allow aiming has been
    /// entered since this was last asked. For `Firearm::lower_aim`.
    pub fn take_aim_lowered(&mut self) -> bool {
        std::mem::take(&mut self.aim_lowered)
    }

    /// The first-person pose of each state the body's animator has entered
    /// since this was last asked, in order; `None` for a state without
    /// one. For `Hands::assign_pose`.
    pub fn take_poses(&mut self) -> Vec<Option<trp_data::weapons::states::FirstPersonPose>> {
        self.body.take_poses()
    }

    /// Whether the sprint has switched on since this was last asked:
    /// `sprinting`, which the key sets in a walk forward some time before
    /// the sprinting state begins. For `Firearm::sprint_started`.
    pub fn take_sprint_started(&mut self) -> bool {
        std::mem::take(&mut self.sprint_started)
    }
}
