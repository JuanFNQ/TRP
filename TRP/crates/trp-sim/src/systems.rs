//! The controller's systems a host may switch off (settings spec,
//! "Features"). The game has no such switches: these are the runtime's
//! own, by the user's word (2026-10-14: "make the switches work for
//! mechanics that are fully done. For now, when set to off, lets make it
//! just stop"). All on, as `Default` has them, is the game; a system
//! switched off does not run, and nothing is put in its place. Feeding a
//! system from a host's own game instead (STALKER's weight, its hunger) is
//! still to come.

use crate::movement::Input;

/// Which of the controller's systems run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Systems {
    /// The walk, inertia, the sprint, the stance, the lean, the step
    /// aside, the jump and the crawl: off, none of their keys is heard and
    /// the character stays where it stands. The view still turns and the
    /// breath can still be held.
    pub movement: bool,
    /// Vaulting and climbing: off, the vault key held tries neither (the
    /// jump on the same key is the movement's).
    pub vaulting: bool,
    /// Stamina: off, nothing is paid for and the pools stay full.
    pub stamina: bool,
    /// The carried weight: off, it weighs nothing to the movement or to
    /// the stamina (`Movement::set_weight` takes no weight).
    pub weight: bool,
    /// Exhaustion, what spent pools do: off, spent legs end no sprint and
    /// refuse no sprint, jump, vault or held breath, they slow nothing,
    /// the breath is not let go when the oxygen runs out, and the weapon's
    /// breathing goes as for a rested character.
    pub exhaustion: bool,
    /// The weapon's motion in the hands: off, nothing moves the weapon or
    /// the camera on it but the animation and the recoil (no springs fed
    /// by the turning, the steps, the breath or the walk, no lean's roll
    /// and no turn-away from what it would stick into).
    pub weapon_motion: bool,
    /// The recoil: off, a shot kicks neither the weapon nor the camera.
    pub recoil: bool,
    /// Blind fire (`docs/spec/blind-fire.md`): off, its keys are not heard
    /// and it never starts. It is a system of its own: the movement's
    /// switch off leaves it be.
    pub blind_fire: bool,
    /// Mounting and the bipod (`docs/spec/mounting.md`): off, the mount's
    /// key is not heard and no mount starts (a host does not toggle the
    /// bipod either). A system of its own, as blind fire is.
    pub mounting: bool,
    /// The left shoulder (`docs/spec/left-shoulder.md`): off, its key is not
    /// heard and the weapon never goes to the left side. A system of its
    /// own, as blind fire is.
    pub left_shoulder: bool,
}

impl Default for Systems {
    fn default() -> Self {
        Self { movement: true, vaulting: true, stamina: true, weight: true, exhaustion: true, weapon_motion: true, recoil: true, blind_fire: true, mounting: true, left_shoulder: true }
    }
}

impl Systems {
    /// The input with the keys of the systems switched off taken out.
    pub fn heard(&self, input: &Input) -> Input {
        let mut heard = *input;
        if !self.vaulting {
            heard.vault = false;
        }
        if !self.blind_fire {
            (heard.blind_toggle, heard.blind_end) = (0, false);
        }
        if !self.mounting {
            heard.mount = false;
        }
        if !self.left_shoulder {
            heard.left_stance = false;
        }
        if !self.movement {
            let Input { look_delta, aiming, reloading, hands_busy, hands_idle, hands_in_inventory, hold_breath, breath_end, weapon_occupied, free_look, screen, inventory, head_look, head_reset, camera_pitch, blind_toggle, blind_end, blind_firearm, blind_may_start, mount, mount_may, overlap_depth, left_stance, left_stance_may, .. } = heard;
            heard = Input {
                look_delta,
                aiming,
                reloading,
                hands_busy,
                hands_idle,
                hands_in_inventory,
                hold_breath,
                breath_end,
                weapon_occupied,
                free_look,
                screen,
                inventory,
                head_look,
                head_reset,
                camera_pitch,
                blind_toggle,
                blind_end,
                blind_firearm,
                blind_may_start,
                mount,
                mount_may,
                overlap_depth,
                left_stance,
                left_stance_may,
                ..Input::default()
            };
        }
        heard
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_system_switched_off_hears_none_of_its_keys() {
        let input = Input { move_dir: [0.0, 1.0], jump: true, vault: true, sprint_toggle: true, look_delta: [2.0, 1.0], hold_breath: true, aiming: true, ..Input::default() };
        let all = Systems::default();
        let heard = all.heard(&input);
        assert!(heard.vault && heard.jump && heard.move_dir == [0.0, 1.0]);
        let heard = Systems { vaulting: false, ..all }.heard(&input);
        assert!(!heard.vault && heard.jump);
        let heard = Systems { movement: false, ..all }.heard(&input);
        assert!(!heard.vault && !heard.jump && !heard.sprint_toggle && heard.move_dir == [0.0, 0.0]);
        assert!(heard.look_delta == [2.0, 1.0] && heard.hold_breath && heard.aiming);
    }

    #[test]
    fn blind_fire_is_a_system_of_its_own() {
        let input = Input { blind_toggle: 1, blind_end: true, blind_firearm: true, move_dir: [0.0, 1.0], ..Input::default() };
        let all = Systems::default();
        let heard = Systems { movement: false, ..all }.heard(&input);
        assert!(heard.blind_toggle == 1 && heard.blind_end && heard.move_dir == [0.0, 0.0]);
        let heard = Systems { blind_fire: false, ..all }.heard(&input);
        assert!(heard.blind_toggle == 0 && !heard.blind_end && heard.move_dir == [0.0, 1.0]);
    }
}
