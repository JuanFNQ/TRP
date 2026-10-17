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
const TAG_MANAGER: i32 = 78;

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

/// The physics engine's settings of the build (`PhysicsManager`, Unity
/// 2022.3's layout; `probe layers`), and the layers' names (`TagManager`):
/// what Unity hands PhysX, and so what `trp_physics` sets it up with.
#[derive(Debug, Clone)]
pub struct PhysicsSettings {
    pub gravity: [f32; 3],
    pub bounce_threshold: f32,
    pub default_max_depenetration: f32,
    pub sleep_threshold: f32,
    pub contact_offset: f32,
    pub solver_iterations: u32,
    pub solver_velocity_iterations: u32,
    pub adaptive_force: bool,
    /// Persistent contact manifolds (`m_ContactsGeneration` 1).
    pub pcm: bool,
    /// Which layer meets which: a mask for each of the 32.
    pub layer_matrix: [u32; 32],
    /// 0 patch, 1 one-directional, 2 two-directional.
    pub friction_type: u32,
    pub enhanced_determinism: bool,
    pub improved_patch_friction: bool,
    /// 0 PGS, 1 TGS.
    pub solver_type: u32,
    pub max_angular_speed: f32,
    /// The 32 layers' names, empty where a layer has none.
    pub layers: Vec<String>,
}

impl PhysicsSettings {
    pub fn load(install: &Install) -> Result<Self> {
        let path = install.root.join("EscapeFromTarkov_Data").join("globalgamemanagers");
        let data = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let meta = SerializedFile::parse(&data).context("globalgamemanagers")?;
        let object = |class_id: i32| -> Result<&[u8]> {
            let o = meta.objects.iter().find(|o| o.class_id == class_id).with_context(|| format!("globalgamemanagers has no object of class {class_id}"))?;
            Ok(&data[o.offset..o.offset + o.size])
        };
        let p = object(PHYSICS_MANAGER)?;
        let word = |at: usize| -> Result<u32> { Ok(u32::from_le_bytes(p.get(at..at + 4).context("the physics settings are short")?.try_into()?)) };
        let float = |at: usize| -> Result<f32> { Ok(f32::from_bits(word(at)?)) };
        if word(64)? != 32 {
            bail!("the physics settings' layout differs: no layer matrix at byte 64");
        }
        let mut layer_matrix = [0u32; 32];
        for (i, row) in layer_matrix.iter_mut().enumerate() {
            *row = word(68 + 4 * i)?;
        }
        let byte = |at: usize| -> Result<u8> { p.get(at).copied().context("the physics settings are short") };
        let settings = Self {
            gravity: [float(0)?, float(4)?, float(8)?],
            bounce_threshold: float(24)?,
            default_max_depenetration: float(28)?,
            sleep_threshold: float(32)?,
            contact_offset: float(36)?,
            solver_iterations: word(40)?,
            solver_velocity_iterations: word(44)?,
            adaptive_force: byte(50)? != 0,
            pcm: word(60)? == 1,
            layer_matrix,
            friction_type: word(252)?,
            enhanced_determinism: byte(256)? != 0,
            improved_patch_friction: byte(258)? != 0,
            solver_type: word(260)?,
            max_angular_speed: float(264)?,
            layers: Self::layer_names(object(TAG_MANAGER)?).unwrap_or_default(),
        };
        let plausible = (-50.0..=-1.0).contains(&settings.gravity[1])
            && (0.0..=100.0).contains(&settings.bounce_threshold)
            && (0.0..=1.0).contains(&settings.sleep_threshold)
            && (0.0001..=1.0).contains(&settings.contact_offset)
            && (1..=255).contains(&settings.solver_iterations)
            && settings.friction_type <= 2
            && settings.solver_type <= 1
            && (0.1..=10_000.0).contains(&settings.max_angular_speed);
        if !plausible {
            bail!("the physics settings' layout differs: {settings:?}");
        }
        Ok(settings)
    }

    /// The tags' list, then the layers' 32 names.
    fn layer_names(bytes: &[u8]) -> Option<Vec<String>> {
        let mut at = 0usize;
        let list = |at: &mut usize| -> Option<Vec<String>> {
            let count = u32::from_le_bytes(bytes.get(*at..*at + 4)?.try_into().ok()?) as usize;
            *at += 4;
            (0..count)
                .map(|_| {
                    let len = u32::from_le_bytes(bytes.get(*at..*at + 4)?.try_into().ok()?) as usize;
                    let text = String::from_utf8_lossy(bytes.get(*at + 4..*at + 4 + len)?).into_owned();
                    *at = (*at + 4 + len + 3) & !3;
                    Some(text)
                })
                .collect()
        };
        list(&mut at)?;
        list(&mut at)
    }

    /// A layer's number by its name.
    pub fn layer(&self, name: &str) -> Option<u16> {
        self.layers.iter().position(|known| known == name).map(|at| at as u16)
    }

    /// Whether two layers meet.
    pub fn meet(&self, a: u16, b: u16) -> bool {
        self.layer_matrix.get(a as usize).is_some_and(|row| row >> b & 1 == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The physics settings as the install read on 2026-10-15 has them
    /// (`probe layers`).
    #[test]
    fn the_physics_settings_are_read() {
        let Ok(install) = Install::locate() else { return };
        let p = PhysicsSettings::load(&install).expect("the physics settings");
        assert_eq!(p.gravity, [0.0, -9.81, 0.0]);
        assert_eq!((p.bounce_threshold, p.sleep_threshold, p.contact_offset), (2.0, 0.005, 0.01));
        assert_eq!((p.solver_iterations, p.solver_velocity_iterations), (3, 1));
        assert_eq!(p.max_angular_speed, 7.0);
        assert!(p.pcm && !p.adaptive_force && !p.improved_patch_friction && !p.enhanced_determinism);
        assert_eq!((p.friction_type, p.solver_type), (0, 0));
        let (deadbody, terrain, loot) = (p.layer("Deadbody").unwrap(), p.layer("Terrain").unwrap(), p.layer("Loot").unwrap());
        assert_eq!((deadbody, terrain, loot), (23, 11, 15));
        assert!(p.meet(deadbody, deadbody) && p.meet(deadbody, terrain) && !p.meet(loot, loot) && !p.meet(loot, deadbody));
    }
}
