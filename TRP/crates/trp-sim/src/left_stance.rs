//! The left shoulder (`docs/spec/left-shoulder.md`): the weapon carried on
//! the left side of the body (`LeftStance`, the game's `Mouse4`). This is
//! its state as the game's `LeftStanceController` keeps it: the stance
//! wanted, kept through everything, and what the body's animator was last
//! told of it, with the flags that decide when the body's actions and the
//! hands' may change that. It holds no timer and draws nothing: the body's
//! animator plays the move (its `LS` layer, by `LeftStance` and
//! `ForceLeftStanceInOneFrame`), and the weapon follows the body's
//! `Leftstance_curve` (`crate::hands::LeftShoulder`).

/// What the body's actions last asked while they could not be heard
/// (`ELeftStanceBodyActionType`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum BodyRequest {
    #[default]
    None,
    /// The stance's animation off (`DisableAnimLeftStance`).
    Disable,
    /// The animation back to the stance wanted (`SetToCacheValue`).
    ToCache,
}

/// What a hands operation does to the stance as it starts
/// (`SetLeftStanceAnimOnStartOperation`): the weapon's resting operation
/// and its draw put the animation back to the stance wanted, every other
/// operation takes it off, and the weapon's fire and its malfunctions leave
/// it as it is (they have no event at all).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandsStart {
    /// The weapon at rest (`Idling`): back to the stance wanted, unless the
    /// inventory is open or a wall has taken it off (`DisableLeftStanceByOverlap`).
    Idle,
    /// The weapon drawn (`SpawnOperation`): back to the stance wanted, or,
    /// for a weapon that may not have it (`BlockLeftStance`, none of the
    /// install's), off for good.
    Draw { blocked: bool },
    /// Any other operation of the hands (`FirearmOperation`'s, and every
    /// other thing in hand's draw): off, from the hands
    /// (`DisableLeftStanceAnimFromHandsAction`).
    Other,
}

/// What the thing in hand tells the stance (`trp_sim::Firearm`'s events,
/// passed to `Movement::left_stance_call`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    /// An operation has begun.
    Start(HandsStart),
    /// The inventory opened or shut (`FirearmController.SetInventoryOpened`).
    Inventory(bool),
}

/// The game's `LeftStanceController`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LeftStance {
    /// The stance wanted (`LeftStance`, `MovementContext.LeftStanceEnabled`):
    /// the key flips it, and it is kept whatever takes the animation off.
    pub wanted: bool,
    /// What the animator was last given by the controller's own rules
    /// (`LastAnimValue`).
    pub last_anim: bool,
    /// What the body's animator is told (`PlayerAnimator.SetLeftStance`,
    /// the weapon motion's `LeftStance`), and whether the stance is to come
    /// in at once out of a sprint (`SetLeftStanceForceInOneFrame`).
    pub animator: bool,
    pub force_one_frame: bool,
    last_request: BodyRequest,
    /// `_allowBodyChangeAnim`: false until the hands first put the
    /// animation back (the weapon's rest), false again while a hands
    /// operation or the open inventory holds it off.
    allow_body_change: bool,
    /// `_allowBodyChangeAnimOnUnblockFromBodyAction`.
    allow_on_unblock: bool,
    /// `_blockingLeftStanceOperationInProcess`: set by a hands operation's
    /// taking it off, and never cleared (the game's), which makes the
    /// inventory's shutting put nothing back after the first hands action.
    blocking_operation: bool,
    /// `_blockChangeToLeftStanceFromBodyAction`: a body action holds the
    /// animation off.
    blocked_by_body: bool,
}

impl LeftStance {
    /// `SetAnimatorLeftStance`: the animator's flag, and the force off with it.
    fn set_animator(&mut self, enable: bool) {
        if !enable {
            self.force_one_frame = false;
        }
        self.animator = enable;
    }

    /// `SetAnimatorLeftStanceInOneFrame`.
    fn set_animator_in_one_frame(&mut self) {
        self.force_one_frame = true;
        self.animator = true;
    }

    /// The key's flip (`ToggleLeftStance`): the stance wanted, and the
    /// animator with it unless a body action or the hands hold it off.
    pub fn toggle(&mut self) {
        self.wanted = !self.wanted;
        if !self.blocked_by_body && self.allow_body_change {
            self.set_animator(self.wanted);
            self.last_anim = self.wanted;
        }
    }

    /// `SetLeftStanceForce`: wanted and told at once.
    pub fn set_force(&mut self, value: bool) {
        self.wanted = value;
        self.set_animator(value);
    }

    /// A body action takes the animation off (`DisableLeftStanceAnimFromBodyAction`),
    /// or asks for it to be taken off once the hands let it be, and holds it.
    pub fn disable_from_body(&mut self) {
        if self.allow_body_change {
            self.set_animator(false);
            self.last_anim = false;
        } else {
            self.last_request = BodyRequest::Disable;
        }
        self.block_from_body();
    }

    /// A hands operation takes it off (`DisableLeftStanceAnimFromHandsAction`).
    pub fn disable_from_hands(&mut self) {
        self.set_animator(false);
        self.allow_body_change = false;
        self.allow_on_unblock = false;
        self.last_anim = false;
        self.blocking_operation = true;
    }

    /// The inventory opened takes it off (`DisableLeftStanceAnimFromOpenInventory`).
    pub fn disable_from_open_inventory(&mut self) {
        self.set_animator(false);
        self.allow_body_change = false;
        self.allow_on_unblock = false;
        self.last_anim = false;
    }

    /// `UnblockChangeAnimationFromBodyAction`.
    fn unblock_from_body(&mut self) {
        self.blocked_by_body = false;
        if self.allow_on_unblock {
            self.allow_body_change = true;
        }
    }

    /// `BlockChangeAnimationFromBodyAction`.
    fn block_from_body(&mut self) {
        self.blocked_by_body = true;
    }

    /// A body action is over (`SetAnimatorLeftStanceToCacheFromBodyAction`):
    /// the animation back to the stance wanted, at once out of a sprint
    /// where it is on; or asked for, once the hands let it be. Nothing with
    /// no firearm in hand (`HasFirearmInHands`).
    pub fn to_cache_from_body(&mut self, force_in_one_frame: bool, firearm: bool) {
        if !firearm {
            return;
        }
        if self.allow_body_change {
            if self.wanted && force_in_one_frame {
                self.set_animator_in_one_frame();
            } else {
                self.set_animator(self.wanted);
            }
            self.last_anim = self.wanted;
        } else {
            self.last_request = BodyRequest::ToCache;
        }
        self.unblock_from_body();
    }

    /// The inventory shut (`SetAnimatorLeftStanceToCacheFromCloseInventory`):
    /// as the hands' putting back, unless a hands operation ever took the
    /// animation off.
    pub fn to_cache_from_close_inventory(&mut self, firearm: bool) {
        if !self.blocking_operation {
            self.to_cache_from_hands(firearm);
        }
    }

    /// The hands put it back (`SetAnimatorLeftStanceToCacheFromHandsAction`):
    /// what a body action asked meanwhile is done, else the stance wanted
    /// unless a body action holds it; the body's actions may change it
    /// again, now or once the one holding it is over.
    pub fn to_cache_from_hands(&mut self, firearm: bool) {
        if !firearm {
            return;
        }
        if self.last_request != BodyRequest::None && self.last_request != BodyRequest::ToCache {
            if self.last_request == BodyRequest::Disable && !self.blocked_by_body {
                self.set_animator(false);
                self.last_anim = false;
            }
        } else if !self.blocked_by_body {
            self.set_animator(self.wanted);
            self.last_anim = self.wanted;
        }
        if self.blocked_by_body {
            self.allow_on_unblock = true;
            return;
        }
        self.allow_body_change = true;
        self.last_request = BodyRequest::None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The weapon at rest lets the key through; a sprint takes the
    /// animation off and its end puts it back at once; a reload takes it
    /// off and the rest puts it back; the stance wanted is kept throughout.
    #[test]
    fn the_stance_is_kept_through_what_takes_its_animation_off() {
        let mut s = LeftStance::default();
        s.toggle();
        assert!(s.wanted && !s.animator, "before the hands first rest, only the wish flips");
        s.toggle();
        s.to_cache_from_hands(true);
        s.toggle();
        assert!(s.wanted && s.animator && s.last_anim && !s.force_one_frame);
        s.disable_from_body();
        assert!(s.wanted && !s.animator);
        s.toggle();
        s.toggle();
        assert!(s.wanted && !s.animator, "held off by the body: the key flips the wish alone");
        s.to_cache_from_body(true, true);
        assert!(s.animator && s.force_one_frame, "out of a sprint, in one frame");
        s.disable_from_hands();
        assert!(!s.animator && !s.force_one_frame);
        s.to_cache_from_close_inventory(true);
        assert!(!s.animator, "a hands operation's taking off is never undone by the inventory");
        s.to_cache_from_hands(true);
        assert!(s.animator && s.wanted);
        s.to_cache_from_body(false, false);
        assert!(s.animator, "nothing without a firearm");
    }

    /// A body action that came while the hands held the animation off waits
    /// for them: the body's end of it only lets the body change it again,
    /// and the hands' next putting back puts it back.
    #[test]
    fn a_body_action_waits_for_the_hands() {
        let mut s = LeftStance::default();
        s.to_cache_from_hands(true);
        s.toggle();
        s.disable_from_hands();
        s.disable_from_body();
        assert!(!s.animator);
        s.to_cache_from_hands(true);
        assert!(!s.animator, "the body still holds it off");
        s.to_cache_from_body(false, true);
        assert!(!s.animator, "the body let go, its asking kept for the hands");
        s.to_cache_from_hands(true);
        assert!(s.animator && s.wanted);
    }
}
