//! Unity's particle system as the install stores it.
//! After the particles spec: a system's figures and modules, its renderer and its material, and the
//! values that vary over a particle's life (`MinMaxCurve`,
//! `MinMaxGradient`, `Gradient`). Read here; run by `trp_sim::particles`.

use anyhow::{Context, Result};

use crate::formats::curve::Curve;
use crate::unity::extract::{mesh_of, read_material, read_mesh, read_texture, script_class, MeshData, TextureData};
use crate::unity::{class, Assets, ObjRef, Value};

mod curves;
mod system_data;
mod system_read;
mod muzzle;
mod decals;
mod effects;
mod effect_entries;

pub use curves::*;
pub use system_data::*;
pub use system_read::*;
pub use muzzle::*;
pub use decals::*;
pub use effects::*;
pub use effect_entries::*;
use system_read::{read_light, vec3};
use decals::{rotate};
use effect_entries::{float, lerp, shader_queue};
