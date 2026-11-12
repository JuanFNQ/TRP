//! Weapons not in hand, drawn where the game hangs them on the body.
//!
//! Weapons not in hand, drawn where the game hangs them on the body
//! (`PlayerBody.SlotView`, `CreateAndParent`; body-animator spec, "Weapons
//! on the body"): the sling's and the back's weapons, the holster's pistol
//! and a knife the game shows on the body, each its whole model as built
//! (its parts), still (its animator off), its `Weapon_root` on its slot's
//! bone. The game switches their renderers off for the player's own
//! character, whom its player sees in first person alone; every other
//! player sees them. So here they are drawn when the character is seen from
//! outside, as others see it, and not at all in first person (no shadow).
//! The options' `WEAPONS ON THE BODY`. The pistol hangs by the holster of
//! the legs worn, a lower suit chosen on OVERALL bringing its own, and on
//! legs without one is not hung (`LegsView`, `HaveHolster`). With
//! `MAGAZINE ROUNDS` a hung weapon shows its magazine's rounds and its
//! chambered round. A hung weapon glows and shimmers
//! with its item's heat as it cools (`heat_view`), seen from outside alone:
//! the shimmer the game's code would still emit in first person (particles,
//! not the renderers it turns off) is read from code, not seen, and not
//! built.
//!
//! Since 0.236.0 on every body the game draws so (`Body`), each by its own
//! equipment, its own legs' holster and the thing its hands hold, which its
//! slots leave out (`_itemInHands`): the character's; the gear page's and
//! the OVERALL tab's figure (`figure_view`: the game's menu figure is a
//! body made with the equipment and the item in hand, `PlayerModelLoader.
//! CreatePlayerBody`, its renderers on), still, the weapon in hand left
//! off; with `OVERALL FIGURE`, the OVERALL tab's own figure
//! (`Body::Overall`), the thing it was made holding left off, so the
//! weapon it holds is never hung too; each dummy, standing (what its hands hold left off) and dead (the
//! thing it held left off for good, as its corpse keeps the living body's
//! binding: `dummy::Dummy::in_hand`). Not the character's corpse: the game
//! keeps the living body as the corpse (`Player.CreateCorpse`,
//! `reinitBody: false`), its renderers off as its own player's, and never
//! on again.

use std::collections::HashSet;

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use trp_data::character::arms::LegsHolster;
use trp_data::character::body::SlotBones;
use trp_sim::items::inventory::{Id, Item};

use crate::player::body_view::{BodyParts, BodyRig};
use crate::platform::convert;
use crate::screens::inventory_view::{Carried, HandsHeld};
use crate::world::loose::Loose;

mod slung_state;
mod hanging;

pub use slung_state::*;
pub use hanging::*;
use slung_state::{Hung};
#[cfg(test)]
use slung_state::{Want};

#[cfg(test)]
mod tests;
