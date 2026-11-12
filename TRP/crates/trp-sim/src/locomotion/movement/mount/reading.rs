//! Reading the mount: its view, figures, rig origin, events, tilt and leaving it.

use super::*;

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
        Some(std::array::from_fn(|k| lerp(origin[k], on_point[k], view.progress)))
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
}
