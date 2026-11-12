//! Rests, mounting, blind fire and measuring the overlap.

use super::*;

impl Hands {
    /// Where the weapon's two springs and the camera's rotation spring
    /// rest: the lean, blind fire's turn of the camera, and whatever
    /// first-person poses are blended in.
    pub(super) fn set_rests(&mut self) {
        let (lean, turn, camera, blind) = (self.lean_rest, self.pose_rotation, self.pose_camera, self.blind_camera);
        self.hands_position.zero = self.pose_position;
        // A weapon held closer rests that much nearer along its own y.
        if self.closer {
            self.hands_position.zero[1] = HELD_CLOSER;
        }
        self.hands_rotation.zero = [turn[0], lean + turn[1], turn[2]];
        self.camera_rotation.zero = [camera[0] + blind[0], camera[1] + blind[1], lean + camera[2] + blind[2]];
        // The mount's camera rests on the lean alone (`MountedWeaponCamera`).
        if self.mount_strategy {
            self.camera_rotation.zero = [0.0, 0.0, lean];
        }
    }

    /// What a mount tells the weapon's motion (`ProceduralWeaponAnimation.
    /// SetMountingData`): mounted with this pivot and side, or not. The
    /// recoil takes its mounted figures; the breath, the sway and the
    /// tactical blend go by it. And the weapon's figures are worked out
    /// again (`UpdateWeaponVariables`, `RecalculateRecoilParamsOnChangeWeapon`),
    /// which ends in the recoil's rotation spring put back to nought
    /// (`HandRotationRecoil.Reset`): the breath's hip wander held in its
    /// offset goes with it, on entering, on starting to leave and on leaving
    /// (recorded: the M700's turn of 0.69 degrees lying, its hands' spring's
    /// 0.20 alone on the frame of the mount, `local\traces\m700-bipod`).
    pub fn set_mounting(&mut self, mount: Option<crate::weapons::mounting::HandsMount>) {
        self.mounted = mount;
        self.mount_figures();
        self.recoil.hand_rotation.reset();
    }

    /// The recoil's mounted figures, as the mount and the bipod stand.
    pub(super) fn mount_figures(&mut self) {
        let figures = self.mounted.map(|m| self.settings.mount.motion(&self.settings.mount_settings, m.side, self.bipod_used));
        self.recoil.hand_rotation.set_mounted(figures.map(|f| (f.recoil, f.return_speed, self.settings.mount.return_speed)));
    }

    /// The mount's way of moving in force or not (`SetMountingStratеgy`, and
    /// `SetStrategy` back at the mount's end).
    pub fn set_mount_strategy(&mut self, on: bool) {
        self.mount_strategy = on;
        self.set_rests();
    }

    /// The bipod out or in (`IsBipodUsed`): the recoil, the breath and the
    /// sway read it while mounted.
    pub fn set_bipod(&mut self, out: bool) {
        self.bipod_used = out;
        if self.mounted.is_some() {
            self.mount_figures();
        }
    }

    /// Whether mounted (`IsMountedState`), and the mount's way in force.
    pub fn mounting(&self) -> (Option<crate::weapons::mounting::HandsMount>, bool) {
        (self.mounted, self.mount_strategy)
    }

    /// Where the rig's origin is from the camera's object, in the camera's
    /// axes (a mount moves the rig and the mounted camera follows it): a
    /// host that places the rig sets it each frame, before `frame`.
    pub fn set_rig_shift(&mut self, shift: [f32; 3]) {
        self.rig_shift = shift;
    }

    /// Blind fire's side as the weapon's operation passed it on
    /// (`Firearm::set_blind_fire`; `ProceduralWeaponAnimation.
    /// StartBlindFire`): the blender's target, from this frame's moment
    /// (taken up by the frame's camera pass, `blind_fire_frame`). It reads
    /// where it was on this frame and moves from the next.
    pub fn start_blind_fire(&mut self, side: i32) {
        self.blind_asked = Some(side as f32);
    }

    /// Blind fire's blender on a new frame of `frame_dt` seconds: the
    /// frames' clock moves on to it, a target asked during it is taken from
    /// its moment, and the value is read.
    pub(super) fn blind_fire_frame(&mut self, frame_dt: f32) {
        self.blind_clock += frame_dt;
        if let Some(target) = self.blind_asked.take() {
            self.blind.set_target(target, self.blind_clock);
        }
        self.blind_value = self.blind.value(self.blind_clock);
    }

    /// Blind fire's blender as this frame's camera pass read it: 0 at
    /// rest, toward 1 over the top and -1 to the right; and its target.
    pub fn blind_fire(&self) -> (f32, f32) {
        (self.blind_value, self.blind.target)
    }

    /// Blind fire's part of the camera's pass (`ZeroAdjustments`,
    /// `CalculateCameraPosition`): the blender read off the frame's
    /// moment; while it is off 0 the strength by the view's pitch (whole
    /// under 45 degrees either way, down to nothing at 90); and the camera
    /// rotation spring's rest given the prefab's turn over the top or to
    /// the right, times half the value's size, times the strength.
    pub(super) fn blind_fire_rest(&mut self, frame_dt: f32) {
        self.blind_fire_frame(frame_dt);
        let value = self.blind_value;
        if value != 0.0 {
            let pitch = self.rig_view.map_or(self.view[1], |view| view[1]).abs();
            self.blind_strength = if pitch < 45.0 { 1.0 } else { (90.0 - pitch) / 45.0 };
        }
        let d = &self.settings.data.blind_fire;
        let turn = if value > 0.0 { d.camera[0] } else { d.camera[1] };
        let share = (value / 2.0).abs() * self.blind_strength;
        self.blind_camera = turn.map(|degrees| degrees * share);
        self.set_rests();
    }

    /// In blind fire the weapon is first felt for obstacles along a line
    /// from its end point (`FirearmController.WeaponOverlapping`): the
    /// prefab's point over the top or to the right by the value's sign,
    /// times the strength, with the held-closer shift, all times 1.9, in
    /// the frame the moved bone hangs in, down the barrel for the weapon's
    /// length. None while the blender is at 0.
    pub fn blind_fire_line(&self) -> Option<([f32; 3], [f32; 3])> {
        let value = self.blind_value;
        if value == 0.0 {
            return None;
        }
        let d = &self.settings.data.blind_fire;
        let end = if value > 0.0 { d.end[0] } else { d.end[1] };
        let held = if self.closer { HELD_CLOSER } else { 0.0 };
        let point = [end[0] * self.blind_strength * 1.9, (end[1] * self.blind_strength + held) * 1.9, end[2] * self.blind_strength * 1.9];
        let (at, turn) = self.weapon_root;
        let reach = quat_rotate(turn, point);
        let from: [f32; 3] = std::array::from_fn(|i| at[i] + reach[i]);
        let along = quat_rotate(turn, [0.0, -self.settings.weapon_length, 0.0]);
        Some((from, std::array::from_fn(|i| from[i] + along[i])))
    }

    /// How deep the weapon sticks into something as the game measures it at
    /// the end of a frame, `measure` casting a line in the rig's frame
    /// (`Movement::weapon_overlap`): in blind fire the line from its end
    /// point first, whose depth stands if it is clear (under 0.02 m); else,
    /// and out of blind fire, the plain line (`weapon_line`). A host hands
    /// the depth to `set_overlap` and `Firearm::overlap`.
    pub fn measure_overlap(&self, measure: impl FnMut(([f32; 3], [f32; 3])) -> f32) -> f32 {
        self.measure_overlap_shoulder(false, measure).0
    }

    /// `measure_overlap`, the plain line taken from where the left shoulder
    /// puts the weapon while it is wanted (`WeaponOverlapping`: the hands'
    /// place in the moved bone's own axes, not its parent's; the same way
    /// along the weapon root's). With it, whether the plain line was measured
    /// (not in blind fire with its own line clear), which the left shoulder's
    /// drop and return go by.
    pub fn measure_overlap_shoulder(&self, left_stance: bool, mut measure: impl FnMut(([f32; 3], [f32; 3])) -> f32) -> (f32, bool) {
        if let Some(line) = self.blind_fire_line() {
            let depth = measure(line);
            if depth < OVERLAP_THAT_COUNTS {
                return (depth, false);
            }
        }
        (measure(if left_stance { self.shoulder_line(0.0) } else { self.weapon_line() }), true)
    }

    /// The left shoulder's line (`shift` 0), or the one its way back is
    /// tried along (`shift` 0.2: as far again to the left of the weapon
    /// root and as much longer; `WeaponOverlapping`), in the rig's frame.
    pub fn shoulder_line(&self, shift: f32) -> ([f32; 3], [f32; 3]) {
        let (at, turn) = self.weapon_root;
        let (local_at, local_turn) = self.shoulder_root;
        let placed = quat_rotate(turn, local_at);
        let root: [f32; 3] = std::array::from_fn(|i| at[i] + placed[i]);
        let reach = quat_rotate(quat_mul(turn, local_turn), self.hands_position.value());
        let left = quat_rotate(turn, [-shift, 0.0, 0.0]);
        let from: [f32; 3] = std::array::from_fn(|i| root[i] + reach[i] + left[i]);
        let along = quat_rotate(turn, [0.0, -(self.settings.weapon_length + shift), 0.0]);
        (from, std::array::from_fn(|i| from[i] + along[i]))
    }

    /// The body's animator has entered a state: its first-person pose, if
    /// it has one, comes in, and every other pose held goes out.
    pub fn assign_pose(&mut self, pose: Option<&FirstPersonPose>) {
        // Told to the character's own eyes only
        // (`MovementContext.AssignAnimatorPose`).
        if self.outside() {
            return;
        }
        let mut held = pose.is_none();
        for (other, _, coming_in) in &mut self.poses {
            *coming_in = pose.is_some_and(|pose| pose.name == other.name);
            held |= *coming_in;
        }
        if let (false, Some(pose)) = (held, pose) {
            self.poses.push((pose.clone(), 0.0, true));
        }
    }
}
