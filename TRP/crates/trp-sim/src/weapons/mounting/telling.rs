//! The events and motion a mount tells its host.

use super::*;

/// What the mount did in a frame, for the host: the weapon's motion, the
/// weapon's animator, the sounds and the ergonomics go by it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MountEvent {
    /// Entered (`EnterMountedState`): the approach's length, the side,
    /// whether the bipod's pivot is the mount's, and lying down.
    Entered { approach: f32, side: Side, on_bipod: bool, prone: bool },
    /// The exit has begun (`StartExiting`): it lasts the exit time.
    Leaving { side: Side },
    /// The mount's state is taken off (`ExitMountedState`).
    Left { side: Side },
    /// A search found nothing, lying down there was nothing to search with,
    /// or what it found was refused (`TrySetPlayerMountingPointData` false,
    /// then `NoPointFound`): `OnNothingFound`, the battle screen's mount
    /// icon flashes red (`BattleUiBipodPanel.OnMountUnavailable`).
    Refused,
}

/// What the weapon's motion is told of a mount (`SetMountingData`): the
/// pivot it turns about, in the moved bone's axes; whether at a side; and
/// whether the bipod is out (`IsBipodUsed`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HandsMount {
    pub pivot: [f32; 3],
    pub side: bool,
    pub bipod: bool,
}

/// The weapon's motion's figures for a weapon on a mount (`recoil`, its
/// camera snap, its return, the breath out of stamina, the sway with the
/// bipod): what the motion reads while `HandsMount` is set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MountedMotion {
    pub recoil: f32,
    pub return_speed: f32,
    pub camera_snap: f32,
    pub breath: f32,
    pub sway: f32,
    pub tactical: bool,
}

impl WeaponMount {
    /// What the weapon's motion goes by on a mount at `side`, the bipod out
    /// or not.
    pub fn motion(&self, s: &Settings, side: bool, bipod_out: bool) -> MountedMotion {
        let on_bipod = !side && bipod_out;
        let bipod = self.bipod.filter(|_| bipod_out);
        MountedMotion {
            recoil: if side { self.recoil_side } else if let (true, Some(b)) = (on_bipod, bipod) { b.recoil } else { self.recoil_forward },
            return_speed: if let (true, Some(b)) = (on_bipod, bipod) { b.return_speed } else { self.return_speed },
            camera_snap: if let (true, Some(b)) = (on_bipod, bipod) { b.camera_snap } else { self.camera_snap },
            breath: if side { self.breath_side } else if let (true, Some(b)) = (on_bipod, bipod) { b.breath } else { self.breath_forward },
            sway: if on_bipod { s.sway_with_bipod } else { 1.0 },
            tactical: on_bipod,
        }
    }

    /// The pivot a mount turns the weapon about: the bipod's forward with it
    /// out, else the mount's.
    pub fn pivot_for(&self, side: bool, bipod_out: bool) -> [f32; 3] {
        if !side && bipod_out { self.bipod_pivot.unwrap_or([0.0; 3]) } else { self.pivot }
    }
}

/// After the movement's frame: what the mount did goes to the weapon and
/// its motion, as the game's `EnterMountedState`, `StartExitingMountedState`
/// and `ExitMountedState` tell them in the same call (the animator's
/// `Mounted`, `SetMountingData`, the mount's way of moving), and the
/// ergonomics take the class skill's mounted gain (`mount_ergonomics`: the
/// class's `WeaponBuffs::mount_ergonomics`, while mounted and forward on the
/// bipod). The events are handed back for the host's sounds and screen.
pub fn tell(
    movement: &mut crate::locomotion::movement::Movement,
    mut firearm: Option<&mut crate::weapons::firearm::Firearm>,
    mut hands: Option<&mut crate::weapons::hands::Hands>,
    mount_ergonomics: [f32; 2],
) -> Vec<MountEvent> {
    let events = movement.take_mount_events();
    for event in &events {
        match *event {
            MountEvent::Entered { side, on_bipod, .. } => {
                let bipod_out = firearm.as_ref().and_then(|f| f.bipod()).unwrap_or(false);
                if let Some(f) = firearm.as_deref_mut() {
                    f.set_mounted(true, on_bipod);
                }
                // A canted sight gives way to a straight one; with none, the
                // sights go down (`OnFoundPoint`, `ToggleFirearmAimByBodyAction`).
                let canted = hands.as_deref_mut().is_some_and(|h| h.straighten_for_mount());
                if let (true, Some(f)) = (canted, firearm.as_deref_mut()) {
                    if f.aiming() {
                        f.set_aim(false, true);
                    }
                }
                if let Some(h) = hands.as_deref_mut() {
                    let at_side = side != Side::Forward;
                    let pivot = h.settings.mount.pivot_for(at_side, bipod_out);
                    h.set_mount_strategy(true);
                    h.set_mounting(Some(HandsMount { pivot, side: at_side, bipod: bipod_out }));
                    h.set_mount_ergonomics(if on_bipod { mount_ergonomics[1] } else { mount_ergonomics[0] });
                }
            }
            MountEvent::Leaving { .. } => {
                if let Some(f) = firearm.as_deref_mut() {
                    f.set_mounted(false, false);
                }
                if let Some(h) = hands.as_deref_mut() {
                    h.set_mounting(None);
                }
            }
            MountEvent::Left { .. } => {
                if let Some(f) = firearm.as_deref_mut() {
                    f.set_mounted(false, false);
                }
                if let Some(h) = hands.as_deref_mut() {
                    h.set_mounting(None);
                    h.set_mount_strategy(false);
                    h.set_mount_ergonomics(0.0);
                }
            }
            MountEvent::Refused => {}
        }
    }
    events
}

/// The arms' drain multiplier aiming on a mount: at a side, forward with
/// the bipod out, forward (`Physical`).
pub fn drain(s: &Settings, side: Side, bipod_out: bool) -> f32 {
    match side {
        Side::Left | Side::Right => s.drain[0],
        Side::Forward if bipod_out => s.drain[2],
        Side::Forward => s.drain[1],
    }
}
