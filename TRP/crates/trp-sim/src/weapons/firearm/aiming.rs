//! Aiming, lowering the aim and telling the body of it.

use super::*;

impl Firearm {
    /// Whether the sights are up.
    pub fn aiming(&self) -> bool {
        self.aiming
    }

    /// Whether a reload is in hand: from its start until the new magazine
    /// is in, or the round chambered where the chamber was empty. The
    /// animation runs on for a while after. The body's animator is told
    /// this (`movement::Input::reloading`): it is what holds a lying body
    /// still.
    pub fn reloading(&self) -> bool {
        matches!(self.operation, Operation::Reload { .. } | Operation::Shells { .. } | Operation::ChambersReload { .. })
    }

    /// The aim key, pressed or let go. `allowed` is whether the
    /// character's movement state lets the sights come up
    /// (`Movement::can_aim`). Letting go always lowers them, except while
    /// a reload is in hand by a character who does not know the weapon
    /// well, where the key does nothing either way. What else lowers them
    /// is `lower_aim` and `sprint_started`.
    pub fn set_aim(&mut self, want: bool, allowed: bool) {
        // In blind fire the call is refused outright, either way
        // (`FirearmController.SetAim`).
        if self.blind_fire {
            return;
        }
        // Whoever uses the key, an obstacle no longer has the sights to
        // give back.
        self.interrupted = false;
        let was = self.aiming;
        self.aim(want, allowed);
        if self.aiming != was {
            self.events.push(Event::Aimed(self.aiming));
        }
    }

    /// The call the aim key makes, as the operation in hand takes it.
    fn aim(&mut self, want: bool, allowed: bool) {
        let blocked = self.overlap > self.settings.stop_aiming_at;
        match self.operation {
            // A weapon at rest that is pushed back by an obstacle does not
            // come up.
            Operation::Idle => {
                if !want || (allowed && !blocked) {
                    self.aiming = want;
                }
            }
            // In a shot the game does not ask anything.
            Operation::Single { .. } | Operation::Automatic { .. } => self.aiming = want,
            Operation::LauncherIdle | Operation::LauncherFire { .. } => self.launcher_aim(want, allowed),
            // In a reload only a character who knows the weapon well has
            // the key.
            Operation::Reload { .. } => {
                if self.settings.mastering >= MASTERING_TO_AIM_IN_A_RELOAD && (!want || allowed) && !(want && blocked) {
                    self.aiming = want;
                }
            }
            // Nor through a look at the weapon, a draw or a put-away,
            // whoever the character is.
            _ => {}
        }
    }

    /// The weapon would stick `depth` metres into something, as measured
    /// at the end of a frame (`Movement::weapon_overlap`). Past the limit
    /// the sights go down; once it is clear again, and the turn-away has
    /// run down (`Hands::turn_away_value`), they come back by themselves,
    /// unless the aim key was used in between. `allowed` is whether the
    /// movement state lets the sights be up (`Movement::can_aim`).
    pub fn overlap(&mut self, depth: f32, turn_away: f32, allowed: bool) {
        self.overlap = depth;
        let limit = self.settings.stop_aiming_at;
        if depth > limit && self.aiming {
            self.set_aim(false, allowed);
            self.interrupted = true;
        } else if depth < limit && turn_away < TURN_AWAY_RUN_DOWN && self.interrupted && !self.aiming {
            // Asked once: a raise that is refused is not tried again.
            self.set_aim(true, allowed);
        }
    }

    /// The body has done what puts the sights down: it has entered a
    /// movement state that does not allow aiming
    /// (`Movement::take_aim_lowered`). They do not come back by themselves.
    /// Only an idling weapon obeys: in a shot, a burst or a reload in hand
    /// the game's call does nothing, and the sights stay up until the next
    /// one.
    pub fn lower_aim(&mut self) {
        if self.is_idling() {
            self.aiming = false;
        }
        // A bolt action's shot does answer: the sights go down and the
        // trigger is let go, which works the bolt
        // (`BoltActionFireOperation.OnAimingDisabled`).
        if self.settings.bolt_action && matches!(self.operation, Operation::Single { .. }) {
            self.aiming = false;
            self.trigger_changed(false);
        }
    }

    /// The character's sprint has switched on
    /// (`Movement::take_sprint_started`), which is at the key and before
    /// the sprinting state begins: the sights go down as above, and the
    /// trigger is let go whatever the weapon is doing.
    pub fn sprint_started(&mut self) {
        self.lower_aim();
        self.trigger_changed(false);
    }

    /// The animator told of the sights as they now stand
    /// (`FirearmController.SetAimingInAnimator`; more-weapons spec, "One
    /// barrel loaded alone, and the rocket launcher"): `AimingIn` up and
    /// `AimingOut` down with them up, the other way with them down, and
    /// `PrevIsAimingFloatState` the other way about; and, but for a rocket
    /// launcher, whose own clips' marks set it, `IsAimingFloat`. With a
    /// part with its own animations on the weapon the game tells it none
    /// of that and clears the flags instead (`ResetAimingAnimationsFlags`:
    /// `AimingIn` and `AimingOut` down, `PrevIsAimingFloatState` 0), and
    /// the body's animator is not told either (`tells_body_of_aim`).
    pub(super) fn tell_aim(&mut self) {
        let on = self.aiming;
        self.aim_told = on;
        // With the launcher in use its animator is the one told
        // (`FirearmsAnimator.SetAiming` on the hands' animator).
        if let Some(launcher) = self.launcher.as_mut().filter(|launcher| launcher.speaks) {
            launcher.animator.set("IsAimingFloat", if on { 1.0 } else { 0.0 });
            launcher.animator.set_flag("AimingIn", on);
            launcher.animator.set_flag("AimingOut", !on);
            launcher.animator.set("PrevIsAimingFloatState", if on { 0.0 } else { 1.0 });
            self.aim_flags = true;
            return;
        }
        if self.settings.unique_animation > 0 {
            self.animator.set_flag("AimingIn", false);
            self.animator.set_flag("AimingOut", false);
            self.animator.set("PrevIsAimingFloatState", 0.0);
            self.aim_flags = false;
            return;
        }
        if !self.settings.rocket_launcher {
            self.animator.set("IsAimingFloat", if on { 1.0 } else { 0.0 });
        }
        self.animator.set_flag("AimingIn", on);
        self.animator.set_flag("AimingOut", !on);
        self.animator.set("PrevIsAimingFloatState", if on { 0.0 } else { 1.0 });
        self.aim_flags = true;
    }

    /// Whether the body's animator hears of the sights going up and down
    /// (its `isAiming`, set by `SetAimingInAnimator` alone): not with a
    /// part with its own animations on the weapon.
    pub fn tells_body_of_aim(&self) -> bool {
        self.settings.unique_animation <= 0
    }
}
