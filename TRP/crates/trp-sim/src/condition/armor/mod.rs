//! The armour a character wears and what it does to a hit.
//! As specified in `docs/spec/body-hits.md`: the pieces and the order a hit meets them in, each piece's resistance
//! and chance of being gone through, what it does to a bullet at a body
//! collider (deflected, blocked, worn), and what it leaves of a hit's
//! damage (`ProceedDamageThroughArmor`, `ArmorComponent.ApplyDamage`).
//!
//! Nothing here knows of an engine: the ballistics ask a wearer about a
//! bullet (`trp_sim::ballistics`), and a host hands a delivered hit to
//! [`Wearer::take_shot`] and its damage to health.

use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use serde_json::Value as Json;
use trp_data::Database;

use crate::combat::ballistics::material;

mod rules;
mod pieces;
mod wearer;
mod damage;

pub use rules::*;
pub use pieces::*;
pub use wearer::*;
pub use damage::*;
use rules::mask_of;
use damage::{apply_damage};

#[cfg(test)]
mod tests;
