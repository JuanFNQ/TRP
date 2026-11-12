//! The edit build screen's rules that are not the engine's.
//! After the workbench spec, "The edit build screen": a build's slots and their classes, what
//! part a slot takes, a build put on a weapon (`WeaponAssembler`), and
//! where the slots' views lie about the weapon (`ItemObserveScreen.
//! UpdatePositions`: cast onto an ellipse, measured along it by
//! `EllipseCalculator`'s tables, spaced by `Spacer.SpaceOut`). No engine
//! types; the app draws it.

use std::sync::Arc;

use crate::items::inventory::{conflict_at, is, Id, Item, Kind, Refusal};

mod slots;
mod placement;

pub use slots::*;
pub use placement::*;

#[cfg(test)]
mod tests;
