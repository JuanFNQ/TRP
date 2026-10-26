//! The mount's own state (`IdleWeaponMountingState`; mounting spec,
//! "Entering" to "What ends it, and what it ignores"). While it lasts it is
//! the state the commands go to (`Player.CurrentManagedState`), and its
//! frame runs after the movement state's own (`MovementState.OnStateMove`);
//! the movement state in force goes on underneath, without the animator's
//! root motion and without turning the body.

use super::*;
use crate::mounting::{self, Asking, Data, Grid, MountEvent, Point, Search, Side, WeaponMount};

/// A mount under way: entering, held or leaving.
#[derive(Debug, Clone, Copy)]
pub(super) struct Mounted {
    pub(super) data: Data,
    /// The side's number (0 forward, -1 left, 1 right).
    side: i32,
    /// The approach's or the exit's time so far (`float_4`).
    timer: f32,
    /// The look's target pitch and yaw (`float_5`, `float_6`), the lean at
    /// the press and the entry's target lean (`float_7`, `float_8`).
    target_pitch: f32,
    target_yaw: f32,
    entry_tilt: f32,
    target_tilt: f32,
    /// The yaw offset the approach ends at (`float_9`), the yaw offset into
    /// the span (`float_10`) and the span's half forward, whole at a side
    /// (`float_11`).
    yaw_shift: f32,
    yaw_offset: f32,
    span: f32,
    exiting: bool,
    /// The lower pitch limit in force (`_downPitchLimit`).
    down_limit: f32,
    start_position: [f32; 3],
    start_look: [f32; 2],
    start_body: f32,
    /// `TransitionMounting`, `TransitionProgress`.
    transition: bool,
    progress: f32,
    /// A side mount's drop of the point (`CurrentMountingPointVerticalOffset`).
    drop: f32,
    /// `MountedSmoothedTilt`, `MountedSmoothedTiltForCamera`.
    mounted_tilt: f32,
    camera_tilt: f32,
    /// What the body's `Tilt` was last set to by the mount (only a side
    /// mount sets it: `PlayerAnimatorSetTilt`).
    body_tilt: Option<f32>,
}

/// The rig's origin from the ribcage `origin` in the rig's frame `turn`, on
/// a mount at `point` (`MountingRotationFunction`): the place that puts
/// `pivot`, turned by `before`, on the point, by the transition's share.
pub fn mounted_apart(point: [f32; 3], origin: [f32; 3], turn: [f32; 4], before: [f32; 4], pivot: [f32; 3], progress: f32) -> [f32; 3] {
    let reach = quat_rotate(before, pivot);
    let on_point: [f32; 3] = std::array::from_fn(|k| point[k] - reach[k] - origin[k]);
    let back = [-turn[0], -turn[1], -turn[2], turn[3]];
    quat_rotate(back, on_point).map(|v| v * progress)
}

/// What a host draws a mount by (`MountingRotationFunction`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MountView {
    /// The point the pivot is put on, lowered by a side mount's drop.
    pub point: [f32; 3],
    pub side: Side,
    /// How far the weapon is moved from the ribcage to the mount, 0 to 1
    /// (`TransitionProgress`; it can pass 1 at the approach's end).
    pub progress: f32,
    /// Whether the pivot is the bipod's.
    pub on_bipod: bool,
    /// The exit is under way.
    pub leaving: bool,
    /// The approach or the exit is under way (`TransitionMounting`).
    pub moving: bool,
}

impl Movement {
    /// The weapon in hand's mount figures and whether its bipod is out, as
    /// they now stand (a host calls this when the weapon or its bipod
    /// changes); none for anything but a firearm.
    pub fn set_mount_weapon(&mut self, weapon: Option<WeaponMount>, bipod_out: bool) {
        self.mount_weapon = weapon.map(|w| (w, bipod_out));
    }

    /// Whether the mount's state is on (`MovementContext.IsInMountedState`):
    /// entering, held or leaving.
    pub fn mounted(&self) -> bool {
        self.mount.is_some()
    }

    /// The mount as a host draws it, while there is one.
    pub fn mount_view(&self) -> Option<MountView> {
        let m = self.mount.as_ref()?;
        let p = m.data.point;
        Some(MountView {
            point: [p.at[0], p.at[1] + m.drop, p.at[2]],
            side: p.side,
            progress: m.progress,
            on_bipod: m.data.on_bipod,
            leaving: m.exiting,
            moving: m.transition,
        })
    }

    /// The mount's data and figures while there is one, for a comparison:
    /// the data, the yaw offset, the mounted tilts, the timer.
    pub fn mount_figures(&self) -> Option<(Data, [f32; 4])> {
        self.mount.as_ref().map(|m| (m.data, [m.yaw_offset, m.mounted_tilt, m.camera_tilt, m.timer]))
    }

    /// Where the first-person rig's origin goes while mounted
    /// (`MountingRotationFunction`): from the ribcage toward the place that
    /// puts the pivot on the point, by the transition's share. `pivot` is
    /// where the pivot is in the rig's frame (from its origin, turned with
    /// it): the weapon's `Weapon_root` bone's place plus its turn of the
    /// pivot. The pivot is turned as the rig was turned on the frame before
    /// (`Weapon_root.TransformPoint` before the hands' root is turned to
    /// this frame's view): recorded, the pivot comes off the point by the
    /// frame's turn, 10.4 mm a degree at the M4A1's 0.61 m
    /// (`courses7\10-anything`; mounting spec, "The weapon's place").
    pub fn mounted_rig_origin(&self, pivot: [f32; 3]) -> Option<[f32; 3]> {
        let view = self.mount_view()?;
        let (origin, _) = self.rig_frame();
        let reach = quat_rotate(self.rig_turn_before, pivot);
        let on_point: [f32; 3] = std::array::from_fn(|k| view.point[k] - reach[k]);
        Some(std::array::from_fn(|k| origin[k] + (on_point[k] - origin[k]) * view.progress))
    }

    /// `mounted_rig_origin` as an offset from the ribcage in the rig's own
    /// frame (turned with the view): nothing off a mount. `pivot` as there.
    pub fn mounted_rig_apart(&self, pivot: [f32; 3]) -> [f32; 3] {
        let Some(view) = self.mount_view() else { return [0.0; 3] };
        let (origin, turn) = self.rig_frame();
        mounted_apart(view.point, origin, turn, self.rig_turn_before, pivot, view.progress)
    }

    /// The rig's turn as this frame began (the frame before's view), which
    /// the mount turns its pivot by.
    pub fn rig_turn_before(&self) -> [f32; 4] {
        self.rig_turn_before
    }

    /// The data of the last mount entered, kept after it is left.
    pub fn last_mount_data(&self) -> Option<Data> {
        self.mount_last
    }

    /// What the mount did since this was last asked.
    pub fn take_mount_events(&mut self) -> Vec<MountEvent> {
        std::mem::take(&mut self.mount_events)
    }

    /// The lean the weapon's motion reads (`ProceduralWeaponAnimation.
    /// SmoothedTilt`): the movement's smoothed lean, but while mounted the
    /// mount's camera lean (a forward mount's is 0).
    pub fn weapon_tilt(&self) -> f32 {
        self.weapon_tilt
    }

    /// The walk's lock on, put on with this movement direction (movement
    /// spec, "Stopped by what stands in the way"), for a host that starts a
    /// character already pressed against something.
    pub fn set_blocked(&mut self, direction: [f32; 2]) {
        self.blocked = true;
        self.blocked_direction = direction;
    }

    /// The weapon's own operations' ends of a mount: a reload, the looks,
    /// the fire mode's look (`StartExitingMountedState`), or the stock
    /// folded and the thing in hand changing (`ExitMountedState`, at once).
    pub fn leave_mount(&mut self, at_once: bool) {
        if at_once {
            self.leave_mount_now();
        } else {
            self.start_leaving_mount();
        }
    }

    /// The lean as a mounted state takes it (`IdleWeaponMountingState.
    /// SetTilt`): anything but none starts the exit; none does nothing.
    pub(super) fn mount_tilt(&mut self, tilt: f32) {
        if tilt != 0.0 {
            self.start_leaving_mount();
        }
    }

    /// The mount's key (`Player.TryMountWeapon`, `WeaponMountingComponent.
    /// TryMountWeapon`).
    pub(super) fn ask_mount(&mut self, input: &Input, world: &dyn World) {
        if !(input.mount_may && self.grounded && self.blind_fire == 0) {
            return;
        }
        let Some((weapon, bipod_out)) = self.mount_weapon else { return };
        if self.mount.is_some() {
            self.start_leaving_mount();
            return;
        }
        let root = self.grid_root();
        if self.prone {
            // Lying down only the bipod mounts: out, no step aside, no keys.
            if bipod_out && self.step == 0 && is_zero(input.move_dir) {
                let pivot = weapon.bipod_pivot.unwrap_or([0.0; 3]);
                self.mount_search = None;
                match mounting::search_prone(world, &self.mount_settings, root, self.yaw, self.surface_pitch, pivot[1].abs(), pivot[2].abs()) {
                    Some(point) => self.found_mount(point, world),
                    None => self.mount_events.push(MountEvent::Refused),
                }
            } else {
                self.mount_events.push(MountEvent::Refused);
            }
            return;
        }
        let column_raise = if bipod_out { weapon.bipod_pivot.unwrap_or([0.0; 3])[2] } else { weapon.pivot[2] };
        let grid = Grid { root, yaw: self.yaw, feet: self.position[1], column_raise, row_raise: weapon.pivot[2] };
        self.mount_search = Some(Search::standing(world, &self.mount_settings, grid));
        self.mount_search_fresh = true;
    }

    /// The search's grid root: the body's `Weapon_root` bone in the world.
    pub fn grid_root(&self) -> [f32; 3] {
        let (at, _) = self.in_player(self.body.weapon_bone());
        let turned = quat_rotate(quat_axis(1, self.body_yaw), at);
        std::array::from_fn(|k| self.position[k] + turned[k])
    }

    /// A frame after the frame of the key: a search under way is answered
    /// on its frame (`yield return null`: after the frame's updates, before
    /// the animator's).
    pub(super) fn answer_mount_search(&mut self, world: &dyn World) {
        if std::mem::take(&mut self.mount_search_fresh) {
            return;
        }
        let Some(search) = self.mount_search.as_mut() else { return };
        let Some(answer) = search.frame() else { return };
        self.mount_search = None;
        match answer {
            Some(point) => self.found_mount(point, world),
            None => self.mount_events.push(MountEvent::Refused),
        }
    }

    /// A point found (`OnFoundPoint`): the data, then the mount; refused,
    /// nothing (`NoPointFound`). A door or a moving platform under the
    /// point: none in this world.
    fn found_mount(&mut self, point: Point, world: &dyn World) {
        let Some((weapon, bipod_out)) = self.mount_weapon else { return };
        let [height, centre] = self.capsule_figures();
        let asking = Asking {
            position: self.position,
            yaw: self.yaw,
            pitch: self.pitch,
            pose_level: self.pose_level,
            prone: self.prone,
            grounded: self.grounded,
            weapon,
            bipod_out,
            capsule: [self.mount_settings.capsule_radius, height, self.mount_settings.step_offset, centre],
        };
        match mounting::data_for(world, &self.mount_settings, point, &asking) {
            Some(data) => self.enter_mount(data, world),
            None => self.mount_events.push(MountEvent::Refused),
        }
    }

    /// `IdleWeaponMountingState.Enter`, and the rest of `EnterMountedState`.
    fn enter_mount(&mut self, data: Data, world: &dyn World) {
        let forward = data.point.side == Side::Forward;
        let side = data.point.side.number();
        // The lean becomes the slow lean's; the lean at the press is kept.
        self.slow_leaning = true;
        let entry_tilt = self.smoothed_tilt;
        self.tilt = 0.0;
        self.step = 0;
        // The left shoulder off for good.
        self.left_stance_mounted();
        let limit = data.yaw_limit;
        let mut span = delta_angle(limit[0], limit[1]).abs();
        if forward {
            span /= 2.0;
        }
        let (mut target_yaw, mut yaw_shift, mut target_tilt) = (data.target_yaw, 0.0, 0.0);
        if !forward {
            let (mut yaw, other) = mounting_both_ways(self.yaw);
            if yaw < limit[0] || yaw > limit[1] {
                yaw = other;
            }
            target_yaw = if yaw >= limit[0] && yaw <= limit[1] {
                self.yaw
            } else if self.yaw < limit[0] {
                limit[0]
            } else {
                limit[1]
            };
            yaw_shift = if (data.target_yaw - limit[0]).abs() <= 1e-6 * data.target_yaw.abs().max(1.0) { target_yaw - limit[0] } else { target_yaw - limit[1] };
            let toward = if side as f32 * yaw_shift < 0.0 { side as f32 } else if side == 0 { -yaw_shift } else { 0.0 };
            target_tilt = sign(toward) * lerp(0.0, MAX_TILT, yaw_shift.abs() / span);
        }
        let up = data.pitch_limit[0];
        let down = data.pitch_limit[1];
        self.pitch_target = [up, down];
        let target_pitch = if forward && !self.prone {
            0.0
        } else if self.pitch < self.pitch_limit[0] {
            up
        } else if self.pitch > self.pitch_limit[1] {
            self.pitch_limit[1]
        } else {
            self.pitch
        };
        let pose = lerp(data.pose_limit[0], data.pose_limit[1], inverse_lerp(up, down, target_pitch));
        self.set_pose_level(pose, world);
        self.mount = Some(Mounted {
            data,
            side,
            timer: 0.0,
            target_pitch,
            target_yaw,
            entry_tilt,
            target_tilt,
            yaw_shift,
            yaw_offset: 0.0,
            span,
            exiting: false,
            down_limit: down,
            start_position: self.position,
            start_look: [self.yaw, self.pitch],
            start_body: self.body_yaw,
            transition: true,
            progress: 0.0,
            drop: 0.0,
            mounted_tilt: 0.0,
            camera_tilt: 0.0,
            body_tilt: None,
        });
        self.mount_events.push(MountEvent::Entered { approach: data.approach, side: data.point.side, on_bipod: data.on_bipod, prone: self.prone });
        self.mount_last = Some(data);
    }


    /// `StartExitingMountedState`, `IdleWeaponMountingState.StartExiting`.
    pub(super) fn start_leaving_mount(&mut self) {
        let Some(m) = self.mount.as_mut() else { return };
        if m.exiting {
            return;
        }
        m.exiting = true;
        m.timer = 0.0;
        m.transition = true;
        m.progress = 1.0;
        if m.side != 0 {
            m.body_tilt = Some(m.mounted_tilt);
        }
        let side = m.data.point.side;
        self.mount_events.push(MountEvent::Leaving { side });
    }

    /// `ExitMountedState`, with `IdleWeaponMountingState.Exit`.
    pub(super) fn leave_mount_now(&mut self) {
        let Some(m) = self.mount.take() else { return };
        // The state is off before its own exit runs: the lean set with
        // force reaches the smoothed lean (`SmoothedTilt` takes no value
        // while mounted).
        if m.side == 0 {
            self.tilt = 0.0;
        } else {
            self.tilt = m.mounted_tilt.clamp(-MAX_TILT, MAX_TILT);
            self.smoothed_tilt = self.tilt;
        }
        self.ignore_root_motion = false;
        self.yaw_limit = None;
        self.pitch_target = if self.prone { PRONE_PITCH_RANGE } else { UPRIGHT_PITCH_RANGE };
        self.weapon_tilt = self.smoothed_tilt;
        self.mount_events.push(MountEvent::Left { side: m.data.point.side });
    }

    /// The mount's frame (`IdleWeaponMountingState.ManualAnimatorMoveUpdate`),
    /// after each of the movement state's own updates in the frame.
    /// `before` is where the character stood before the movement state's
    /// update moved it: the game moves by the last motion asked for in a
    /// frame, so an approach's takes the place of the state's.
    pub(super) fn update_mount(&mut self, dt: f32, world: &dyn World, before: [f32; 3]) {
        let Some(m) = self.mount else { return };
        let s = self.mount_settings;
        if m.exiting {
            if m.timer > s.exit_time {
                self.leave_mount_now();
                return;
            }
            let Some(m) = self.mount.as_mut() else { return };
            if m.timer <= s.exit_time {
                m.timer += dt;
                m.progress = 1.0 - m.timer / s.exit_time;
            }
        } else if m.timer <= m.data.approach {
            self.approach_mount(dt, world, before);
        } else if squared(sub3(self.position, m.data.target)) > mounting::reach_squared().0 {
            self.start_leaving_mount();
        } else {
            let l = m.data.yaw_limit;
            self.yaw_limit = Some(((l[0] + l[1]) / 2.0, (l[1] - l[0]) / 2.0));
            self.pitch_target = [m.data.pitch_limit[0], m.down_limit];
            self.mounted_body(world);
        }
    }

    /// `TryApproachTargetPosition`. Where the character is, it reads before
    /// the frame's scheduled motion (`before`).
    fn approach_mount(&mut self, dt: f32, world: &dyn World, before: [f32; 3]) {
        let Some(mut m) = self.mount else { return };
        self.ignore_root_motion = true;
        m.timer += dt;
        let t = m.timer / m.data.approach;
        m.progress = t;
        m.yaw_offset = lerp_angle(0.0, m.yaw_shift, t);
        let x = lerp_angle(m.start_look[0], m.target_yaw, t);
        let y = lerp_angle(m.start_look[1], m.target_pitch, t);
        self.body_yaw = lerp_turn(m.start_body, m.data.body_yaw, t);
        // `Vector2.Lerp` of the two looks, then the view's limits.
        let look = [lerp(m.start_look[0], x, t), lerp(m.start_look[1], y, t)];
        let range = self.pitch_limit.map(|limit| limit + self.surface_pitch);
        self.yaw = look[0];
        self.pitch = look[1].clamp(range[0], range[1]);
        self.note_rotation();
        if !self.prone {
            let tilt = lerp(m.entry_tilt, m.target_tilt, t);
            self.tilt = tilt.clamp(-MAX_TILT, MAX_TILT);
            m.mounted_tilt = tilt;
            if m.side != 0 {
                m.camera_tilt = tilt;
                m.body_tilt = Some(tilt);
                m.drop = mounting::SIDE_DROP * m.yaw_offset.abs() / m.span;
            }
        }
        self.mount = Some(m);
        let along: [f32; 3] = std::array::from_fn(|k| lerp(m.start_position[k], m.data.target[k], t));
        let motion = [along[0] - before[0], 0.0, along[2] - before[2]];
        let off = [before[0] - m.data.target[0], 0.0, before[2] - m.data.target[2]];
        if squared(motion) > 0.0 {
            if self.walk_refused() {
                self.leave_mount_now();
                return;
            }
            // The state's own motion of this frame is replaced.
            self.position = before;
            self.apply_motion(motion, dt, world);
            let size = squared(motion).sqrt();
            self.movement_direction = self.to_local([motion[0] / size, motion[2] / size]);
            self.inert = squared(off) > mounting::reach_squared().1;
        }
        if m.timer > m.data.approach {
            if squared(off) > mounting::reach_squared().0 {
                self.leave_mount_now();
                return;
            }
            self.position = m.data.target;
            self.apply_motion([0.0; 3], dt, world);
            self.inert = false;
            if let Some(m) = self.mount.as_mut() {
                m.transition = false;
            }
        }
    }

    /// `UpdatePlayerBody`: the lean and the stance by the yaw and the pitch,
    /// and a side mount's drop.
    fn mounted_body(&mut self, world: &dyn World) {
        let Some(m) = self.mount else { return };
        let share = m.yaw_offset.abs() / m.span;
        if !self.prone {
            // `UpdateCurrentTilt`.
            let side = m.side as f32;
            if side * m.yaw_offset < 0.0 || m.side == 0 {
                let toward = if side * m.yaw_offset < 0.0 { side } else { -m.yaw_offset };
                let tilt = sign(toward) * lerp(0.0, MAX_TILT, share);
                self.tilt = tilt.clamp(-MAX_TILT, MAX_TILT);
                if let Some(m) = self.mount.as_mut() {
                    m.mounted_tilt = tilt;
                    if m.side != 0 {
                        m.camera_tilt = tilt;
                        m.body_tilt = Some(tilt);
                    }
                }
            }
            // `UpdatePoseLevel`.
            let up = m.data.pitch_limit[0];
            let level = lerp(m.data.pose_limit[0], m.data.pose_limit[1], inverse_lerp(up, m.down_limit, self.pitch));
            self.set_pose_level(level, world);
        }
        if m.side != 0 {
            if let Some(m) = self.mount.as_mut() {
                m.drop = mounting::SIDE_DROP * share;
            }
        }
    }

    /// The turn of the view while mounted (`IdleWeaponMountingState.Rotate`),
    /// in place of the movement state's. Nothing turns in the approach or
    /// the exit.
    pub(super) fn mount_rotate(&mut self, delta: [f32; 2], overlap_depth: f32, world: &dyn World) {
        let Some(mut m) = self.mount else { return };
        if m.timer <= m.data.approach || m.exiting {
            return;
        }
        let s = self.mount_settings;
        let up = m.data.pitch_limit[0];
        if m.side == 0 {
            // `CalculateDynamicDownLimit`, then the limits at once.
            m.down_limit = if overlap_depth >= mounting::down_limit_overlap() { self.pitch } else { m.data.pitch_limit[1] };
            self.pitch_limit = [up, m.down_limit];
        }
        // `ClampRotation`: the state's sensitivity, then cut to its clamp.
        let mut delta = delta;
        if s.turn_clamp > 0.0 {
            delta = delta.map(|d| d * s.sensitivity);
            let size = length(delta);
            if size > s.turn_clamp {
                delta = delta.map(|d| d * s.turn_clamp / size);
            }
        }
        let (low, high) = match m.side {
            0 => (-m.span, m.span),
            side if side > 0 => (-m.span, 0.0),
            _ => (0.0, m.span),
        };
        let before = m.yaw_offset;
        m.yaw_offset = (before + delta[0]).clamp(low, high);
        if m.yaw_offset.abs() > before.abs() {
            let share = m.yaw_offset.abs() / m.span;
            let side = m.side as f32;
            let toward = if side * m.yaw_offset < 0.0 { side } else if m.side == 0 { sign(-m.yaw_offset) } else { 0.0 };
            let right = quat_rotate(quat_axis(1, self.body_yaw), [1.0, 0.0, 0.0]);
            let shift = right.map(|r| r * toward * share * mounting::TILT_POSITION_OFFSET);
            // The head's capsule carried by the turn and the shift is to
            // meet nothing at all (`RotationOverlapPrediction`, compared
            // with nothing exactly); settings made without the prefab's
            // capsule ask the host about the character's own instead.
            let blocked = if self.settings.head.is_some() {
                self.head_turn_push(shift, 2.0 * delta[0], world) != [0.0; 3]
            } else {
                world.turn_blocked(self.position, shift, 2.0 * delta[0], self.capsule())
            };
            if blocked {
                m.yaw_offset = before;
                delta[0] = 0.0;
            }
        }
        self.mount = Some(m);
        [self.yaw, self.pitch] = self.turned(delta);
        self.note_rotation();
    }

    /// The movement state's lean as the body's animator is shown it, and
    /// the lean the weapon's motion reads, once the smoothing has run: the
    /// smoothed lean takes no value while mounted (`SmoothedTilt`'s setter),
    /// and the weapon's reads the mount's camera lean (`SmoothTilt`).
    pub(super) fn mounted_tilt_shown(&self) -> Option<f32> {
        self.mount.as_ref().and_then(|m| m.body_tilt)
    }

    pub(super) fn mount_camera_tilt(&self) -> Option<f32> {
        self.mount.as_ref().map(|m| m.camera_tilt)
    }

    /// The body's `Tilt` as its animator is told it (a side mount sets it;
    /// otherwise the smoothed lean, which takes no value while mounted).
    pub fn body_tilt_shown(&self) -> f32 {
        self.mounted_tilt_shown().unwrap_or(self.smoothed_tilt)
    }

    /// The gesture the body's animator is shown (`InteractionIndex`), if one.
    pub fn shown_interaction(&self) -> Option<u8> {
        self.shown_interaction
    }

    /// The aiming arms' drain multiplier while mounted (`Physical`).
    pub(super) fn mount_drain(&self) -> f32 {
        match (self.mount.as_ref(), self.mount_weapon) {
            (Some(m), Some((_, bipod_out))) => mounting::drain(&self.mount_settings, m.data.point.side, bipod_out),
            _ => 1.0,
        }
    }
}

fn squared(v: [f32; 3]) -> f32 {
    v[0] * v[0] + v[1] * v[1] + v[2] * v[2]
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `ClampAngle360Sensitive`'s first steps: a yaw within a turn either way,
/// and the same yaw the other way round.
fn mounting_both_ways(yaw: f32) -> (f32, f32) {
    let mut a = yaw;
    if a < -360.0 {
        a += 360.0;
    }
    if a > 360.0 {
        a -= 360.0;
    }
    (a, if a > 0.0 { a - 360.0 } else { a + 360.0 })
}
