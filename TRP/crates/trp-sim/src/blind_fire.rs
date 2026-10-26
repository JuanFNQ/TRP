//! Blind fire's hand-over between the parts (`docs/spec/blind-fire.md`).
//!
//! The game's gate (`MovementContext.SetBlindFire`) sets the movement's
//! figure, tells the weapon's operation, which passes the value on to the
//! weapon's motion, and tells the body's animator, all in one call. Here the
//! three are the host's: the movement takes its commands and keeps its
//! figure (`Movement::take_blind_fire`), and these hand what it took to the
//! weapon (`Firearm::set_blind_fire`) and its motion
//! (`Hands::start_blind_fire`), on the same frame.

use crate::firearm::Firearm;
use crate::hands::Hands;
use crate::movement::Movement;

/// After the movement's frame: what blind fire's gate took this frame, if
/// anything, goes to the weapon's operation, and what that passes on to the
/// weapon's motion. Anything but a firearm in hand passes 0 (the gate has
/// put it to 0 already).
pub fn tell(movement: &mut Movement, firearm: Option<&mut Firearm>, hands: Option<&mut Hands>) {
    let Some(value) = movement.take_blind_fire() else { return };
    let passed = firearm.map_or(0, |firearm| firearm.set_blind_fire(value));
    if let Some(hands) = hands {
        hands.start_blind_fire(passed);
    }
}

/// An operation of the weapon that ends blind fire has begun
/// (`FirearmEvent::BlindFireEnded`: `SetBlindFire(0)` at its start): the
/// movement's figure and the body's animator go to 0, and so does the
/// weapon's motion's blender, at once. PROVISIONAL: the body's animator
/// hears it on the next frame, where the game's call comes before its
/// update; read from code, not recorded.
pub fn ended(movement: &mut Movement, hands: Option<&mut Hands>) {
    movement.end_blind_fire();
    // The weapon's flag is down already; it is told 0 again by the next
    // frame's `tell`, which changes nothing.
    if let Some(hands) = hands {
        hands.start_blind_fire(0);
    }
}
