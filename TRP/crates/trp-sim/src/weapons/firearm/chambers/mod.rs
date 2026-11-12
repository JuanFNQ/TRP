//! Weapons whose rounds lie in chambers of their own and not in a magazine under a bolt.
//! A revolver's or a launcher's cylinder, and barrels loaded one by one
//! (`docs/spec/more-weapons.md`). The game has an
//! operation of its own for the firing and for the reloading of each
//! (`FireCylinderMagOperation`, `ReloadCylinderMagOperation`,
//! `FireOnlyBarrelFireOperation`, `ReloadMultiBarrelOperation`); these are
//! those, on the weapon's own animator.

use super::{Earn, Event, FireMode, Firearm, Operation, FIRE_STATE_START};

mod shown;
mod cylinder;
mod barrels;
mod own_chamber;


/// What a chamber of a cylinder, or a barrel, holds: a live round, and the
/// case of one fired from it, which stays until a reload takes it out. The
/// game keeps the two apart, the round in the chamber's slot (`Chambers`,
/// a cylinder's `Camoras`) and the case on the weapon (`Weapon.
/// ShellsInChambers`), so a chamber may hold both: a round the inventory
/// put in while the weapon was not in hand lies beside the case it found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Chamber {
    pub round: bool,
    pub case: bool,
}

/// What the weapon's model holds at one of its chambers of its own, as the
/// game's `Firearms` has it (more-weapons spec, "The rounds and cases seen
/// in chambers of their own"): a round at the chamber's point
/// (`patron_in_weapon`, `_ammoObjectInWeapon`) and a case at its shell port
/// (`_shellsInShellPort`). They change at the game's own moments, which are
/// not the logic's: a shot takes the round out at the press and moves it to
/// the port at `FiringBullet`; a reload counts its rounds in at its end and
/// shows each as it is loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Shown {
    pub round: bool,
    pub case: bool,
}

/// The share of the player's velocity a case thrown by a reload takes
/// (`Player.Velocity * 0.33f`: a cylinder's one by one, barrels' together).
const RELOAD_SHARE: f32 = 0.33;

/// The state a cylinder fires double action from, with the hammer down:
/// the game plays it outright (`FireOperation.StartFireAnimation`).
const DOUBLE_ACTION_FIRE: &str = "Hands.DOUBLE_ACTION_FIRE";

impl Firearm {
}
