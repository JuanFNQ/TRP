//! The animator's values, the idle queries and the trigger.

use super::*;

impl Firearm {
    /// The weapon's animator.
    pub fn animator(&self) -> &WeaponAnimator {
        &self.animator
    }

    /// The marks the animator delivered in the last step.
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    /// Puts the animator's resting state where a step of `dt` seconds
    /// will bring it to a point in its play, in cycles: a replay starts
    /// where the recording's stood after its first frame.
    pub fn rest_at(&mut self, time: f32, dt: f32) {
        self.animator.play(&self.settings.rest_state, time);
        self.animator.update(0.0);
        self.animator.update(-dt);
        self.animator.deliver();
    }

    /// The rounds as the animator is told them.
    pub(super) fn animator_rounds(&mut self) {
        if self.has_chambers() {
            return self.animate_chambers();
        }
        if !self.settings.no_chamber {
            self.animator.set("AmmoInChamber", self.chamber as u8 as f32);
        }
        self.animator.set("AmmoInMag", self.magazine.unwrap_or(0) as f32);
    }

    /// What the idling operation tells the animator of the weapon as it
    /// starts (`SetAnimatorValues`).
    pub(super) fn animator_values(&mut self) {
        let (fitted, variant) = (self.magazine.is_some(), self.mag_variant as f32);
        self.animator_rounds();
        if self.settings.modes.len() > 1 {
            self.animator.set("FireMode", mode_number(self.mode()));
        }
        self.animator.set_flag("MagInWeapon", fitted);
        self.animator.set("MagInWeaponFloat", fitted as u8 as f32);
        self.animator.set("RelTypeOld", if fitted { variant } else { -1.0 });
    }

    pub fn mode(&self) -> FireMode {
        self.settings.modes.get(self.mode).copied().unwrap_or(FireMode::Single)
    }

    /// Whether the weapon is at rest: its logic idling and its animator in
    /// its resting state, on no way out of it.
    pub fn is_idle(&self) -> bool {
        self.is_idling() && self.animator.rests_in(&self.settings.rest_state)
    }

    /// Whether the hands let the player look at a thing to work it
    /// (`FirearmController.CanInteract`): while the logic idles, whatever
    /// the animator is still playing out, or fires; not in a draw, a
    /// reload, a look or a put-away (interactions spec, "What is looked
    /// at").
    pub fn can_interact(&self) -> bool {
        matches!(
            self.operation,
            Operation::Idle | Operation::Single { .. } | Operation::Automatic { .. } | Operation::CylinderFire { .. } | Operation::BarrelsFire { .. }
        )
    }

    /// Whether the idling operation is the one in hand, whatever the
    /// animator is still playing out.
    pub fn is_idling(&self) -> bool {
        self.operation == Operation::Idle
    }

    /// Whether the look all over the weapon is the operation in hand.
    pub fn looks_over(&self) -> bool {
        matches!(self.operation, Operation::Utility { examine: true, .. })
    }

    /// Whether the weapon is in the hands at all: drawn, being drawn or
    /// being put away.
    pub fn in_hand(&self) -> bool {
        self.operation != Operation::Away
    }

    /// Whether a run of automatic fire is under way.
    pub fn auto_fire(&self) -> bool {
        matches!(self.operation, Operation::Automatic { .. })
    }

    /// Tells the weapon where the finger is. Only changes matter.
    /// `allowed` is whether the character's movement state lets the hands
    /// take a press (`Movement::can_press_trigger`): a weapon at rest
    /// drops one made in a sprint or a crawl, and holding the trigger
    /// until the state ends does not fire.
    pub fn set_trigger(&mut self, pressed: bool, allowed: bool) {
        self.hands_free = allowed;
        if pressed == self.held {
            return;
        }
        self.held = pressed;
        self.trigger_changed(pressed);
    }

    pub(super) fn trigger_changed(&mut self, pressed: bool) {
        match &mut self.operation {
            Operation::Idle => self.idle_trigger(pressed),
            Operation::Single { second_shot, .. } => {
                *second_shot |= pressed && self.settings.can_queue_second_shot;
                self.trigger_to(self.trigger && pressed);
                // A bolt action's bolt is worked once the trigger is no
                // longer held (`BoltActionFireOperation.SetTriggerPressed`).
                if self.settings.bolt_action {
                    self.animator.set_flag("BoltActionReload", !self.trigger);
                }
            }
            Operation::Automatic { .. } => self.trigger_to(pressed),
            // A press cuts the filling of a tube short, once a round has
            // shown in the hand; before that it is lost.
            Operation::Shells { can_abort, aborted, .. } => *aborted |= pressed && *can_abort,
            Operation::ChambersReload { can_abort, aborted, .. } => *aborted |= pressed && *can_abort,
            // A shot from a cylinder or from barrels only lets go of the
            // trigger.
            Operation::CylinderFire { .. } | Operation::BarrelsFire { .. } => {
                self.trigger_to(self.trigger && pressed);
                // A rocket launcher's animator has the trigger
                // (`RocketLauncherFire.PrepareShot`).
                if self.settings.rocket_launcher {
                    self.animator.set_flag("Fire", self.trigger);
                }
            }
            // The look all over is ended by the trigger, pressed or let
            // go, and the idling operation takes it from there
            // (`UtilityOperation.SetTriggerPressed`).
            Operation::Utility { examine: true, .. } => {
                (self.hands_busy, self.reload_flag) = (false, false);
                self.to_idle();
                self.idle_trigger(pressed);
            }
            Operation::LauncherIdle | Operation::LauncherFire { .. } | Operation::LauncherUtility { .. } => self.launcher_trigger(pressed),
            // A reload, a look at the magazine, a draw and a put-away take
            // no notice of the trigger.
            _ => {}
        }
    }

    /// The trigger as the idling operation takes it. A press is dropped
    /// where the movement state gives the hands no say, and while a
    /// reload's animation has not brought the weapon back to rest.
    ///
    /// A switch of the fire mode does not stop it: the weapon idles
    /// through the switch, the mode is the new one from the key on, and a
    /// press 0.3 s into the switch's animation fires (the fifteenth
    /// recording, `64-fire-in-firemode-switch`; firing spec, "The trigger
    /// in a fire-mode switch").
    pub(super) fn idle_trigger(&mut self, pressed: bool) {
        if pressed && (!self.hands_free || self.reload_flag) {
            return;
        }
        // A magazine whose top round the chamber refuses is said to the
        // shooter at each press, whatever follows (`Idling.
        // ShowIncompatibleNotification`).
        if pressed && !self.ammo_compatible() {
            self.events.push(Event::IncompatibleAmmo);
        }
        // A malfunction takes the trigger: no shot (`Idling.SetTriggerPressed`).
        if self.malfunction_trigger(pressed) {
            return;
        }
        self.animator_rounds();
        // Chambers of their own have operations of their own, and so has
        // a rocket launcher.
        if self.settings.rocket_launcher {
            return self.rocket_trigger(pressed);
        }
        if self.settings.cylinder.is_some() {
            return self.cylinder_trigger(pressed);
        }
        if self.settings.barrels > 0 {
            return self.barrels_trigger(pressed);
        }
        self.trigger_to(pressed);
        if pressed && self.loaded() {
            // One shot a press in the two single modes; anything else
            // fires for as long as the trigger is held.
            if matches!(self.mode(), FireMode::Single | FireMode::SemiAuto) {
                self.start_single();
            } else {
                self.start_automatic();
            }
            return;
        }
        // On an empty chamber the animator has the trigger, and a press
        // only clicks.
        self.animator.set_flag("Fire", pressed);
        if pressed {
            self.events.push(Event::DryFire);
        }
    }
}
