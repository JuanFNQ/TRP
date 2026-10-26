//! The left shoulder in the movement (`docs/spec/left-shoulder.md`): its
//! key, and the body's states and actions that take the stance's
//! animation off and put it back, each where the game's state objects call
//! its `LeftStanceController` (their `Enter` and `Exit`, the states built
//! on others calling their base's first).

use super::*;
use crate::left_stance::{Call, HandsStart};

/// The wall's wait before the left shoulder is taken off or put back by it,
/// in seconds (`FirearmController.float_6`, 1).
const LEFT_STANCE_OVERLAP_WAIT: f32 = 1.0;

impl Movement {
    /// The left shoulder's key (`FirearmController.ChangeLeftStance`):
    /// only with a firearm in hand, never in blind fire or mounted; the
    /// operation in hand passes it on or drops it (`Idling.ToggleLeftStance`,
    /// which also refuses lying down: `Input::left_stance_may` says the rest).
    pub(super) fn ask_left_stance(&mut self, input: &Input) {
        if !(input.left_stance && input.blind_firearm) || self.blind_fire != 0 || self.mount.is_some() {
            return;
        }
        // (`RemoveLeftHandItem`: nothing is ever held in the left hand here.)
        if input.left_stance_may && !self.prone {
            self.left_stance.toggle();
        }
    }

    /// What the thing in hand tells the stance (`Firearm`'s
    /// `Event::LeftStance`): an operation begun, or the inventory opened or
    /// shut. The resting operation puts the animation back only with the
    /// inventory shut and no wall having taken it off (`Idling.
    /// SetLeftStanceAnimOnStartOperation`).
    pub fn left_stance_call(&mut self, call: Call) {
        let firearm = self.left_stance_firearm;
        match call {
            Call::Start(HandsStart::Idle) => {
                if !self.inventory && !self.left_stance_overlap {
                    self.left_stance.to_cache_from_hands(firearm);
                }
            }
            Call::Start(HandsStart::Draw { blocked }) => {
                // A weapon drawn is a controller made afresh: its wall's
                // wait starts again (`FirearmController.float_7`).
                self.left_stance_overlap_clock = 0.0;
                if blocked {
                    self.left_stance.set_force(false);
                } else {
                    self.left_stance.to_cache_from_hands(firearm);
                }
            }
            Call::Start(HandsStart::Other) => self.left_stance.disable_from_hands(),
            Call::Inventory(true) => self.left_stance.disable_from_open_inventory(),
            Call::Inventory(false) => self.left_stance.to_cache_from_close_inventory(firearm),
        }
    }

    /// Whether the walk keys push forward (`Player.IsForwardInputDirection`).
    pub fn forward_input(&self) -> bool {
        self.input_direction[1] > 0.0
    }

    /// The stance the left shoulder's controller holds.
    pub fn left_stance(&self) -> crate::left_stance::LeftStance {
        self.left_stance
    }

    /// A movement state left (its `Exit`, called before the next one's
    /// `Enter`).
    pub(super) fn left_stance_exit(&mut self, from: State) {
        let firearm = self.left_stance_firearm;
        match from {
            // `SprintPlayerState.Exit`: back at once.
            State::Sprint => self.left_stance.to_cache_from_body(true, firearm),
            State::Jump => self.left_stance.to_cache_from_body(false, firearm),
            // `PronePlayerState.Exit`, and the roll's (its own, then its
            // base's), off the ground only.
            State::ProneIdle if !self.prone => self.left_stance.to_cache_from_body(false, firearm),
            State::Roll if !self.prone => {
                self.left_stance.to_cache_from_body(false, firearm);
                self.left_stance.to_cache_from_body(false, firearm);
            }
            // `VaultingMovementState.Exit`.
            State::Vault | State::Climb => self.left_stance.to_cache_from_body(false, firearm),
            _ => {}
        }
    }

    /// A movement state entered (its `Enter`, after its base's), the
    /// animator's state that entered it of that part (a roll's way in).
    pub(super) fn left_stance_enter(&mut self, to: State, entered_part: i32) {
        let firearm = self.left_stance_firearm;
        let s = &self.settings.vaulting;
        match to {
            // `IdlePlayerState.Enter`, and the states built on it.
            State::Idle | State::BlindFire | State::Sidestep => self.left_stance.to_cache_from_body(false, firearm),
            State::ToProne => {
                self.left_stance.to_cache_from_body(false, firearm);
                self.left_stance.disable_from_body();
            }
            State::FromProne => {
                self.left_stance.to_cache_from_body(false, firearm);
                self.left_stance.to_cache_from_body(false, firearm);
            }
            // `PronePlayerState.Enter` (no base), and the roll's base and
            // own way in (`DisableLeftStanceOnEnter`).
            State::ProneIdle => self.left_stance.disable_from_body(),
            State::Roll => {
                self.left_stance.disable_from_body();
                if entered_part == part::IN {
                    self.left_stance.disable_from_body();
                }
            }
            // `MovePlayerState.Enter`: neither sprinting nor lying down.
            State::Move if !self.sprinting && !self.prone => self.left_stance.to_cache_from_body(false, firearm),
            // `SprintPlayerState.Enter`, after the move's (which the sprint
            // makes do nothing).
            State::Sprint => self.left_stance.disable_from_body(),
            State::Jump => self.left_stance.disable_from_body(),
            // Over something high enough to take a hand.
            State::Vault if self.vault.height > s.vault.max_without_hand => self.left_stance.disable_from_body(),
            State::Climb if self.vault.height >= s.climb.max_without_hand => self.left_stance.disable_from_body(),
            _ => {}
        }
    }

    /// Each frame, after the movement state's: the kick's approach holds
    /// the animation off (`ApproachState.Approach`), and so does an
    /// animated interaction of the body, whose end puts it back
    /// (`Player.VisualPass`: `IsInteractionPlaying`; `PROVISIONAL`: the
    /// gesture shown on the body's interactions' layer stands for it).
    pub(super) fn left_stance_frame(&mut self) {
        if self.state == State::Approach {
            self.left_stance.disable_from_body();
        }
        let playing = self.shown_interaction.is_some();
        if playing {
            self.left_stance.disable_from_body();
        }
        if self.left_stance_interaction && !playing {
            self.left_stance.to_cache_from_body(false, self.left_stance_firearm);
        }
        self.left_stance_interaction = playing;
    }

    /// The weapon against a wall with the left shoulder
    /// (`FirearmController.WeaponOverlapping`), at the frame's end, after
    /// the weapon is felt along its line (`Hands::measure_overlap_shoulder`;
    /// `measured` false where blind fire's own line was clear and the plain
    /// one not measured): its wait runs on (`float_7`, a second, begun again
    /// at each of its changes and at a draw, and two seconds after the
    /// inventory is shut); with the stance wanted, not lying down nor in the
    /// inventory and the wait over, a weapon meeting something at the full
    /// curve takes the animation off from the hands, and once it is all the
    /// way back and clear, a line 0.2 m further left tried clear by
    /// `clear_left` puts it back.
    pub fn left_stance_overlap(&mut self, depth: f32, measured: bool, clear_left: impl FnOnce() -> bool, frame_dt: f32) {
        if self.left_stance_inventory_was && !self.inventory {
            self.left_stance_overlap_clock = -1.0;
        }
        self.left_stance_inventory_was = self.inventory;
        if self.left_stance_overlap_clock <= 1.0 {
            self.left_stance_overlap_clock += frame_dt / LEFT_STANCE_OVERLAP_WAIT;
        }
        if !measured {
            return;
        }
        let curve = self.body.left_stance_curve();
        let free = self.left_stance.wanted && !self.prone && !self.inventory && self.left_stance_overlap_clock > 1.0;
        if depth > 0.0 && free && curve >= 1.0 {
            self.left_stance.disable_from_hands();
            self.left_stance_overlap = true;
            self.left_stance_overlap_clock = 0.0;
        }
        let free = self.left_stance.wanted && !self.prone && !self.inventory && self.left_stance_overlap_clock > 1.0;
        if !self.left_stance.last_anim && depth <= 0.0 && self.left_stance_overlap && free && curve <= 0.0 && clear_left() {
            self.left_stance.to_cache_from_hands(self.left_stance_firearm);
            self.left_stance_overlap = false;
            self.left_stance_overlap_clock = 0.0;
        }
    }

    /// The mount entered turns the stance off for good
    /// (`IdleWeaponMountingState.Enter`).
    pub(super) fn left_stance_mounted(&mut self) {
        if self.left_stance.wanted {
            self.left_stance.toggle();
        }
    }
}
