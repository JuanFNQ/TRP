//! Engine-wide settings of the game build: the physics step and gravity.
//!
//! They sit in `globalgamemanagers`, a serialized file without type trees.
//! Both objects start with the fields wanted here, so only those are read,
//! and the values are checked for plausibility.

use anyhow::{bail, Context, Result};

use crate::install::Install;
use crate::unity::reader::Reader;
use crate::unity::SerializedFile;

const TIME_MANAGER: i32 = 5;
const PHYSICS_MANAGER: i32 = 55;

#[derive(Debug, Clone, Copy)]
pub struct EngineSettings {
    /// Seconds per physics step. The weapon springs advance once per step.
    pub fixed_timestep: f32,
    /// The longest a frame may count for, in seconds: a longer one is cut
    /// to this before anything is timed by it.
    pub maximum_timestep: f32,
    pub gravity: [f32; 3],
}

impl EngineSettings {
    pub fn load(install: &Install) -> Result<Self> {
        let path = install.root.join("EscapeFromTarkov_Data").join("globalgamemanagers");
        let data = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let meta = SerializedFile::parse(&data).context("globalgamemanagers")?;
        let object = |class_id: i32| -> Result<&[u8]> {
            let o = meta
                .objects
                .iter()
                .find(|o| o.class_id == class_id)
                .with_context(|| format!("globalgamemanagers has no object of class {class_id}"))?;
            Ok(&data[o.offset..o.offset + o.size])
        };

        let mut r = Reader::new(object(TIME_MANAGER)?);
        let (fixed_timestep, maximum_timestep) = (r.f32()?, r.f32()?);
        if !(0.001..=0.1).contains(&fixed_timestep) {
            bail!("fixed timestep {fixed_timestep} is not plausible; the time settings layout differs");
        }
        if !(fixed_timestep..=2.0).contains(&maximum_timestep) {
            bail!("maximum timestep {maximum_timestep} is not plausible; the time settings layout differs");
        }
        let mut r = Reader::new(object(PHYSICS_MANAGER)?);
        let gravity = [r.f32()?, r.f32()?, r.f32()?];
        if !(-50.0..=-1.0).contains(&gravity[1]) || gravity[0] != 0.0 || gravity[2] != 0.0 {
            bail!("gravity {gravity:?} is not plausible; the physics settings layout differs");
        }
        Ok(Self { fixed_timestep, maximum_timestep, gravity })
    }
}
