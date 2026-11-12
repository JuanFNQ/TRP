//! Tactical devices: the parts that carry a light or a laser, their modes and their figures.
//! After the tactical-devices spec: the parts that carry a light
//! (`LightComponent`: the classes `Flashlight`, `TacticalCombo` and
//! `LightLaser`), how many modes each has (`ModesCount`), which objects of
//! its model each mode shows (`TacticalComboVisualController`: `mode_000`
//! off, `mode_00<n+1>` mode n), and the figures of its lights (Unity's
//! `Light`, `IkLight`, `VolumetricLight`) and lasers (`LaserBeam` and the
//! materials of `Custom/LaserBeam` and `Custom/LaserPoint`) as its prefab
//! has them.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};

use crate::setup::database::{Database, Preset};
use crate::unity::extract::{read_hierarchy, read_material, read_texture, Hierarchy, MaterialData, TextureData};
use crate::unity::{class, Assets, ObjRef, Value};

mod device_parts;
mod device_read;

pub use device_parts::*;
pub use device_read::*;
use device_parts::{read_flare};

/// The three item classes that carry a light (`HasLightComponent`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    Flashlight,
    TacticalCombo,
    LightLaser,
}

impl DeviceClass {
    /// The class of an item's template, where it is one of the three.
    pub fn of(db: &Database, template: &str) -> Option<Self> {
        let t = db.items.get(template)?;
        if db.is_a(t, "TacticalCombo") {
            Some(Self::TacticalCombo)
        } else if db.is_a(t, "Flashlight") {
            Some(Self::Flashlight)
        } else if db.is_a(t, "LightLaser") {
            Some(Self::LightLaser)
        } else {
            None
        }
    }
}

/// How many modes a device's template has (`ModesCount`), 0 where it says
/// nothing.
pub fn modes_of(db: &Database, template: &str) -> u32 {
    db.items.get(template).and_then(|t| t.props.get("ModesCount")).and_then(|count| count.as_u64()).unwrap_or(0) as u32
}
