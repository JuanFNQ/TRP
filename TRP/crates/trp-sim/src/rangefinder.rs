//! The handheld rangefinder in the hands (`docs/spec/rangefinder.md`): its
//! controller (`PortableRangeFinderController`) on its rig's own animator,
//! drawn, at rest, aimed through on the aim key, looked over on the examine
//! key, put away; and its readout (`TacticalRangeFinderController`), a ray
//! every half second while it is aimed, the host casting it.

use crate::weapon_animator::{WeaponAnimator, WeaponAnimatorData};

/// The aiming speed the controller sets, with no aim sway and no weight
/// (`RANGEFINDER_AIM_SPEED`, `ManualSetVariables(2, 0, 0, 0)`).
pub const AIM_SPEED: f32 = 2.0;
/// The readout's first-person figures (the prefab's
/// `TacticalRangeFinderController`): seconds between rays, the farthest a
/// ray reaches, and the text where it meets nothing.
pub const READOUT_PERIOD: f32 = 0.5;
pub const READOUT_REACH: f32 = 999.0;
pub const NO_DISTANCE: &str = "---.-";

/// What the rangefinder's hands tell the host.
#[derive(Debug, Clone, PartialEq)]
pub enum RangefinderEvent {
    /// Drawn: the draw's mark (`WeapIn`).
    Appeared,
    /// Put away (`WeapOut`): what is taken in hand next is made now.
    Hidden,
    Sound(String),
    BodyAction(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operation {
    Away,
    Spawn,
    Idle,
    Remove,
}

/// The readout's state: the time since its last ray, and its text.
#[derive(Debug, Clone, PartialEq)]
pub struct Readout {
    since: f32,
    pub text: String,
}

impl Default for Readout {
    fn default() -> Self {
        Self { since: 0.0, text: NO_DISTANCE.to_owned() }
    }
}

/// A distance as the readout writes it (`ThreeDigitsAndDot`): clamped to 0
/// to 999 and written with three whole digits and one decimal (`000.0`);
/// none, `---.-`.
pub fn readout_text(distance: Option<f32>) -> String {
    match distance {
        Some(d) => format!("{:05.1}", d.clamp(0.0, READOUT_REACH)),
        None => NO_DISTANCE.to_owned(),
    }
}

pub struct RangefinderHands {
    animator: WeaponAnimator,
    operation: Operation,
    hide_asked: bool,
    drawn_smooth: bool,
    /// The aim as the hands have it (`Fire`, `IsAiming`), and the optic and
    /// readout as the controller switched them (which may differ: the
    /// game's `SetAiming` switches those whatever the aim did).
    aiming: bool,
    optic_on: bool,
    pub readout: Readout,
    /// A ray is wanted this step.
    cast: bool,
}

impl RangefinderHands {
    pub fn new(animator: &WeaponAnimatorData) -> Self {
        Self { animator: WeaponAnimator::new(animator), operation: Operation::Away, hide_asked: false, drawn_smooth: false, aiming: false, optic_on: false, readout: Readout::default(), cast: false }
    }

    pub fn set_drawn_smooth(&mut self, on: bool) {
        self.drawn_smooth = on;
    }

    /// Draws it (`PortableRangeFinderSpawnOperation`): the animator made
    /// afresh and told it is active.
    pub fn draw(&mut self) {
        self.animator.reset();
        self.animator.set_flag("Active", true);
        self.hide_asked = false;
        self.set_optic(false);
        self.aiming = false;
        self.operation = Operation::Spawn;
    }

    /// Gives way to something else (`HideWeapon` → `PortableRangeFinderRemove`):
    /// at rest at once, during the draw once it is in hand.
    pub fn hide(&mut self) {
        match self.operation {
            Operation::Away | Operation::Remove => {}
            Operation::Idle => self.remove(),
            Operation::Spawn => self.hide_asked = true,
        }
    }

    fn remove(&mut self) {
        self.animator.set_flag("Active", false);
        self.animator.set_flag("Fire", false);
        self.aiming = false;
        self.set_optic(false);
        self.hide_asked = false;
        self.operation = Operation::Remove;
    }

    /// The aim asked for (`PortableRangeFinderIdling.SetAiming`), at rest
    /// only: the hands take it where the character `can` aim; the optic
    /// and the readout are switched to what was asked, whether or not.
    pub fn set_aim(&mut self, wanted: bool, can: bool) {
        if self.operation != Operation::Idle {
            return;
        }
        let take = !wanted || can;
        if take {
            self.aiming = wanted;
            self.animator.set_flag("Fire", wanted);
        }
        self.set_optic(wanted);
    }

    fn set_optic(&mut self, on: bool) {
        if on && !self.optic_on {
            // As it comes on, a ray at once and its timer from nought
            // (`TacticalRangeFinderController.OnEnable`).
            self.readout.since = 0.0;
            self.cast = true;
        }
        self.optic_on = on;
    }

    pub fn set_inventory(&mut self, open: bool) {
        if self.operation == Operation::Idle {
            if open {
                self.set_aim(false, true);
            }
            self.animator.set_flag("Inventory", open);
        }
    }

    /// The examine key, at rest only.
    pub fn look(&mut self) {
        if self.operation == Operation::Idle && !self.aiming {
            self.animator.trigger("Look");
        }
    }

    pub fn in_hand(&self) -> bool {
        self.operation != Operation::Away
    }

    pub fn idling(&self) -> bool {
        self.operation == Operation::Idle
    }

    pub fn aiming(&self) -> bool {
        self.aiming
    }

    /// The optic and the readout are on.
    pub fn optic_on(&self) -> bool {
        self.optic_on
    }

    /// Whether a ray is to be cast this step: the host casts it and gives
    /// back what it met (`take_distance`).
    pub fn wants_ray(&self) -> bool {
        self.cast
    }

    /// The ray's distance, or none: the readout's text.
    pub fn take_distance(&mut self, distance: Option<f32>) {
        self.cast = false;
        self.readout.text = readout_text(distance);
    }

    pub fn operation_name(&self) -> &'static str {
        match self.operation {
            Operation::Away => "",
            Operation::Spawn => "PortableRangeFinderSpawnOperation",
            Operation::Idle => "PortableRangeFinderIdling",
            Operation::Remove => "PortableRangeFinderRemove",
        }
    }

    pub fn animator(&self) -> &WeaponAnimator {
        &self.animator
    }

    pub fn step(&mut self, dt: f32) -> Vec<RangefinderEvent> {
        let mut events = Vec::new();
        // The readout's clock, while it is on (`Update`).
        if self.optic_on {
            self.readout.since += dt;
            if self.readout.since >= READOUT_PERIOD {
                self.readout.since = 0.0;
                self.cast = true;
            }
        }
        self.animator.set_drawn_smooth(self.drawn_smooth);
        self.animator.update(dt);
        for mark in self.animator.deliver() {
            match (mark.name.as_str(), self.operation) {
                ("WeapIn", Operation::Spawn) => {
                    self.operation = Operation::Idle;
                    events.push(RangefinderEvent::Appeared);
                    if self.hide_asked {
                        self.remove();
                    }
                }
                ("WeapOut", Operation::Remove) => {
                    self.operation = Operation::Away;
                    events.push(RangefinderEvent::Hidden);
                }
                ("Sound", _) => events.push(RangefinderEvent::Sound(mark.text)),
                ("ThirdAction", _) => events.push(RangefinderEvent::BodyAction(mark.int)),
                _ => {}
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_readout_writes_three_digits_and_a_decimal() {
        assert_eq!(readout_text(Some(45.27)), "045.3");
        assert_eq!(readout_text(Some(0.4)), "000.4");
        assert_eq!(readout_text(Some(1234.0)), "999.0");
        assert_eq!(readout_text(None), "---.-");
    }
}
