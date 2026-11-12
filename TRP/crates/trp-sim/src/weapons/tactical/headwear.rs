//! Head-mounted lights and the requests to switch them.

use super::*;

/// The head light's coroutine: the devices' models and the sound after
/// 0.8 s, the lock lifted 0.6 s after that (tactical-devices spec, "The
/// head light").
pub const HEAD_SWITCH_AFTER: f32 = 0.8;

pub const HEAD_UNLOCK_AFTER: f32 = 0.6;

/// The worn things a head device can be fitted to (the model under the
/// head's bone that the game collects them from).
pub const HEAD_SLOTS: [&str; 4] = ["Headwear", "FaceCover", "Eyewear", "Earpiece"];

/// What the head light's coroutine does on a frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HeadStep {
    /// The devices' models switched and the tactical sound played.
    pub switched: bool,
    /// The lock lifted.
    pub unlocked: bool,
}

/// The head light: its lock (`IsHeadLightsAnimationActive`) and where its
/// coroutine is, by the seconds since its last wait began.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HeadLight {
    /// Seconds into the first wait, the switch still to come.
    to_switch: Option<f32>,
    /// Seconds into the second, the lock still on.
    to_unlock: Option<f32>,
}

impl HeadLight {
    pub fn locked(&self) -> bool {
        self.to_switch.is_some() || self.to_unlock.is_some()
    }

    /// The lock on, to lift once `seconds` more have passed: a press made
    /// before, its switch done (a replay begun as a recording did).
    pub fn unlocking_in(seconds: f32) -> Self {
        Self { to_switch: None, to_unlock: Some(HEAD_UNLOCK_AFTER - seconds) }
    }

    /// A press taken: the lock on and the coroutine started. Refused (false)
    /// while the lock is on; the hands' state and the head's devices are the
    /// caller's to have asked.
    pub fn press(&mut self) -> bool {
        if self.locked() {
            return false;
        }
        self.to_switch = Some(0.0);
        true
    }

    /// A frame of `dt` seconds after the one the press was on.
    pub fn step(&mut self, dt: f32) -> HeadStep {
        let mut out = HeadStep::default();
        if let Some(t) = self.to_switch.as_mut() {
            *t += dt;
            if *t >= HEAD_SWITCH_AFTER {
                self.to_switch = None;
                self.to_unlock = Some(0.0);
                out.switched = true;
                return out;
            }
        } else if let Some(t) = self.to_unlock.as_mut() {
            *t += dt;
            if *t >= HEAD_UNLOCK_AFTER {
                self.to_unlock = None;
                out.unlocked = true;
            }
        }
        out
    }
}

/// The head's devices: every device on what is worn on the head, its
/// parts' too, in the order the slots are listed.
pub fn head_devices<'a>(worn: impl Iterator<Item = &'a Item>) -> Vec<&'a Item> {
    let mut out = Vec::new();
    for thing in worn {
        if thing.kind.light_modes.is_some() {
            out.push(thing);
        }
        out.extend(devices(thing));
    }
    out
}

/// What a head light key asks (`SwitchHeadLights(toggle, switchMode)`):
/// H each device's on flipped, Ctrl+H each one's mode one up.
pub fn head_request(found: &[&Item], toggle: bool, switch_mode: bool) -> Vec<(Id, Light)> {
    found
        .iter()
        .map(|d| {
            let mut light = d.light;
            if toggle {
                light.on = !light.on;
            }
            if switch_mode {
                light.mode = next_mode(light.mode, d.kind.light_modes.unwrap_or(0));
            }
            (d.id, light)
        })
        .collect()
}
