//! Hands that hold nothing (`docs/spec/inventory.md`, "Empty hands"). To
//! the game they hold something all the same: a rig like a weapon's with an
//! animator of its own, and a controller with operations like a weapon's.
//! This is that controller, on the rig's own animator.

use crate::firearm::Event;
use crate::weapon_animator::{WeaponAnimator, WeaponAnimatorData};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operation {
    /// Not in the hands: something else is, or nothing has been yet.
    Away,
    /// Appearing, until the mark in the animator's state for it.
    Spawn,
    Idle,
}

pub struct EmptyHands {
    animator: WeaponAnimator,
    operation: Operation,
    /// The hands are busy, as the body's animator is told: while they
    /// appear, and from the moment they are to go.
    hands_busy: bool,
    /// Whether what is drawn of the animator is taken on curves without
    /// corners (`set_drawn_smooth`).
    drawn_smooth: bool,
}

impl EmptyHands {
    /// Empty hands that are not there yet; `draw` makes them appear.
    pub fn new(animator: &WeaponAnimatorData) -> Self {
        Self { animator: WeaponAnimator::new(animator), operation: Operation::Away, hands_busy: false, drawn_smooth: false }
    }

    /// Asks for the hands' animations to be drawn on curves without
    /// corners through their clips' samples, or as the engine has them,
    /// as `Firearm::set_drawn_smooth` does for a weapon: the drawing
    /// alone.
    pub fn set_drawn_smooth(&mut self, on: bool) {
        self.drawn_smooth = on;
    }

    /// Makes the hands appear (`SpawnOperation`): their animator, made
    /// afresh, is told it is active and leaves its first state for the one
    /// that brings the hands in. At that state's mark they are there
    /// (`Event::Appeared`).
    pub fn draw(&mut self) {
        self.animator.reset();
        self.animator.set_flag("Active", true);
        self.hands_busy = true;
        self.operation = Operation::Spawn;
    }

    /// The hands give way to whatever is taken in hand next (`Remove`):
    /// the body is told they are busy and they are gone at once. Their
    /// animator's own state for going is not waited for.
    pub fn hide(&mut self) {
        if self.operation != Operation::Away {
            self.hands_busy = true;
            self.operation = Operation::Away;
        }
    }

    /// The inventory opens or shuts. The animator has the flag set like
    /// any other, and none of its states answers to it.
    pub fn set_inventory(&mut self, open: bool) {
        if self.operation == Operation::Idle {
            self.animator.set_flag("Inventory", open);
        }
    }

    pub fn in_hand(&self) -> bool {
        self.operation != Operation::Away
    }

    pub fn hands_busy(&self) -> bool {
        self.hands_busy
    }

    /// Whether the hands are there and at rest: the one operation of
    /// theirs that takes a backpack's drop.
    pub fn idling(&self) -> bool {
        self.operation == Operation::Idle
    }

    /// Whether the animator is in its resting state: what the game asks
    /// before it begins a gesture of the hands on the world.
    pub fn animator_idling(&self) -> bool {
        self.operation != Operation::Away && self.animator.current_is("Hands.IDLE")
    }

    pub fn animator(&self) -> &WeaponAnimator {
        &self.animator
    }

    /// One frame of `dt` seconds. `pitch` is the view's, in degrees below
    /// level, which the animator is told every frame: the resting hands
    /// are a blend of two clips by it.
    pub fn step(&mut self, dt: f32, pitch: f32) -> Vec<Event> {
        let mut events = Vec::new();
        self.animator.set("Aim_angle", pitch);
        self.animator.set_drawn_smooth(self.drawn_smooth);
        self.animator.update(dt);
        for mark in self.animator.deliver() {
            match (mark.name.as_str(), self.operation) {
                ("WeapIn", Operation::Spawn) => {
                    self.operation = Operation::Idle;
                    self.hands_busy = false;
                    events.push(Event::Appeared);
                }
                ("Sound", _) => events.push(Event::Sound(mark.text)),
                ("ThirdAction", _) => events.push(Event::BodyAction(mark.int)),
                _ => {}
            }
        }
        events
    }
}
