//! The game's built-in settings object, `eftsettings`.
//!
//! It lives in `resources.assets`, a serialized file with no type trees, so
//! its bytes are read with a layout taken from the class definition. See
//! `docs/spec/movement.md`, "Settings sources".

use std::collections::HashMap;

use anyhow::{anyhow, bail, Context, Result};

use crate::curve::{Curve, Key};
use crate::install::Install;
use crate::unity::reader::Reader;
use crate::unity::{class, SerializedFile};

/// Fields in declaration order, one `kind name` per line.
const LAYOUT: &str = include_str!("hard_settings_layout.txt");
const OBJECT_NAME: &str = "eftsettings";

#[derive(Debug, Clone)]
enum Field {
    Number(f64),
    Vec2([f32; 2]),
    Curve(Curve),
    /// A list of small numbers: the game's own numbers for movement states, mostly.
    Bytes(Vec<u8>),
    /// A vector (`RollCheckShift`).
    Vec3([f32; 3]),
    /// A list of vectors (`BleedingCenters`).
    Vec3s(Vec<[f32; 3]>),
    /// The ballistic presets (`ColliderPresets`).
    Presets(Vec<BallisticPreset>),
    Other,
}

#[derive(Clone)]
pub struct HardSettings {
    fields: HashMap<String, Field>,
}

impl HardSettings {
    pub fn load(install: &Install) -> Result<Self> {
        let path = install.root.join("EscapeFromTarkov_Data").join("resources.assets");
        let data = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let meta = SerializedFile::parse(&data).context("resources.assets")?;
        for object in meta.objects.iter().filter(|o| o.class_id == class::MONO_BEHAVIOUR) {
            let bytes = &data[object.offset..object.offset + object.size];
            if let Ok(settings) = Self::parse(bytes) {
                return Ok(settings);
            }
        }
        bail!("no settings object named {OBJECT_NAME} in {}", path.display())
    }

    /// Parses one script object, failing unless it is the settings object.
    fn parse(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        r.skip(12)?; // owning game object
        r.skip(4)?; // enabled flag, padded
        r.skip(12)?; // script
        let name_len = r.u32()? as usize;
        if name_len != OBJECT_NAME.len() || r.bytes(name_len)? != OBJECT_NAME.as_bytes() {
            bail!("a different object");
        }
        r.align(4);

        let mut fields = HashMap::new();
        for line in LAYOUT.lines() {
            let Some((kind, name)) = line.trim().split_once(' ') else { continue };
            let value = match kind {
                "float" => Field::Number(r.f32()? as f64),
                "int" => Field::Number(r.i32()? as f64),
                "double" => Field::Number(r.f64()?),
                "bool" => {
                    let v = r.u8()?;
                    r.align(4);
                    Field::Number(v as f64)
                }
                "vec2" => Field::Vec2([r.f32()?, r.f32()?]),
                "vec3" => Field::Vec3([r.f32()?, r.f32()?, r.f32()?]),
                "vec3s" => {
                    let count = r.u32()? as usize;
                    anyhow::ensure!(count < 4096, "vec3s {name}: {count} vectors");
                    Field::Vec3s((0..count).map(|_| Ok([r.f32()?, r.f32()?, r.f32()?])).collect::<Result<_>>()?)
                }
                "bytes" => {
                    let count = r.u32()? as usize;
                    let list = r.bytes(count)?.to_vec();
                    r.align(4);
                    Field::Bytes(list)
                }
                "curve" => Field::Curve(read_curve(&mut r).with_context(|| format!("curve {name}"))?),
                // A reference to another object (a file index and a path id).
                "pptr" => {
                    r.skip(12)?;
                    Field::Other
                }
                "quats" => {
                    let count = r.u32()? as usize;
                    r.skip(count * 16)?;
                    Field::Other
                }
                // `BallisticPreset[]`: a name, a material number, a list of
                // figures, a flag and a curve each.
                "ballistics" => {
                    let count = r.u32()? as usize;
                    if count > 4096 {
                        bail!("{count} ballistic presets is not plausible");
                    }
                    let mut presets = Vec::with_capacity(count);
                    for _ in 0..count {
                        let len = r.u32()? as usize;
                        let preset_name = String::from_utf8_lossy(r.bytes(len)?).into_owned();
                        r.align(4);
                        let material = r.i32()?;
                        let figures = r.u32()? as usize;
                        anyhow::ensure!(figures < 64, "a preset of {name} with {figures} figures");
                        let figures = (0..figures).map(|_| r.f32()).collect::<Result<Vec<_>>>()?;
                        let depend_on_scale = r.u8()? != 0;
                        r.align(4);
                        let scale_curve = read_curve(&mut r).with_context(|| format!("a preset of {name}"))?;
                        presets.push(BallisticPreset { name: preset_name, material, figures, depend_on_scale, scale_curve });
                    }
                    Field::Presets(presets)
                }
                other => bail!("layout has an unknown kind {other}"),
            };
            fields.insert(name.to_owned(), value);
        }
        let settings = Self { fields };
        settings.check()?;
        Ok(settings)
    }

    /// Guards against a layout that has drifted from the data: a few values
    /// spread through the object must be in their plausible ranges.
    fn check(&self) -> Result<()> {
        let within = |name: &str, lo: f32, hi: f32| -> Result<()> {
            let v = self.float(name)?;
            if !(lo..=hi).contains(&v) {
                bail!("settings layout mismatch: {name} = {v}");
            }
            Ok(())
        };
        within("IdleStateMotionPreservation", 0.0, 1.0)?;
        within("DecelerationSpeed", 0.1, 20.0)?;
        within("POSE_CHANGING_SPEED", 0.1, 100.0)?;
        within("TRANSFORM_ROTATION_LERP_SPEED", 0.1, 1000.0)?;
        within("DELTA_LEVEL", 0.001, 1.0)?;
        within("RollCheckGroundCastLength", 0.0, 10.0)?;
        for curve in ["MovementAccelerationRange", "LIFT_VELOCITY_BY_SPEED", "JUMP_DELAY_BY_SPEED"] {
            if self.curve(curve)?.keys.is_empty() {
                bail!("settings layout mismatch: {curve} has no keys");
            }
        }
        Ok(())
    }

    pub fn float(&self, name: &str) -> Result<f32> {
        match self.fields.get(name) {
            Some(Field::Number(v)) => Ok(*v as f32),
            _ => Err(anyhow!("settings have no number {name}")),
        }
    }

    pub fn vec2(&self, name: &str) -> Result<[f32; 2]> {
        match self.fields.get(name) {
            Some(Field::Vec2(v)) => Ok(*v),
            _ => Err(anyhow!("settings have no pair {name}")),
        }
    }

    pub fn vec3(&self, name: &str) -> Result<[f32; 3]> {
        match self.fields.get(name) {
            Some(Field::Vec3(v)) => Ok(*v),
            _ => Err(anyhow!("settings have no vector {name}")),
        }
    }

    /// Every vector, by name.
    pub fn vectors(&self) -> Vec<(&str, [f32; 3])> {
        let mut out: Vec<_> = self
            .fields
            .iter()
            .filter_map(|(name, f)| match f {
                Field::Vec3(v) => Some((name.as_str(), *v)),
                _ => None,
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(b.0));
        out
    }

    pub fn vec3s(&self, name: &str) -> Result<&[[f32; 3]]> {
        match self.fields.get(name) {
            Some(Field::Vec3s(v)) => Ok(v),
            _ => Err(anyhow!("settings have no list of vectors {name}")),
        }
    }

    pub fn curve(&self, name: &str) -> Result<&Curve> {
        match self.fields.get(name) {
            Some(Field::Curve(c)) => Ok(c),
            _ => Err(anyhow!("settings have no curve {name}")),
        }
    }

    /// The ballistic presets, by the material's number (`ColliderPresets`:
    /// ballistics spec, "Surfaces").
    pub fn ballistic_presets(&self) -> Result<&[BallisticPreset]> {
        match self.fields.get("ColliderPresets") {
            Some(Field::Presets(list)) => Ok(list),
            _ => Err(anyhow!("settings have no ballistic presets")),
        }
    }

    /// A list of the game's numbers, such as the movement states that do
    /// not allow aiming.
    pub fn bytes(&self, name: &str) -> Result<&[u8]> {
        match self.fields.get(name) {
            Some(Field::Bytes(list)) => Ok(list),
            _ => Err(anyhow!("settings have no list {name}")),
        }
    }

    /// Names and values of every number, for inspection.
    pub fn numbers(&self) -> Vec<(&str, f64)> {
        let mut out: Vec<_> = self
            .fields
            .iter()
            .filter_map(|(name, f)| match f {
                Field::Number(v) => Some((name.as_str(), *v)),
                _ => None,
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(b.0));
        out
    }

    pub fn curves(&self) -> Vec<(&str, &Curve)> {
        let mut out: Vec<_> = self
            .fields
            .iter()
            .filter_map(|(name, f)| match f {
                Field::Curve(c) => Some((name.as_str(), c)),
                _ => None,
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(b.0));
        out
    }
}

/// A material's ballistic figures (`BallisticPreset`): its name, the
/// material's number (`MaterialType`), the six figures (penetration level,
/// penetration chance, ricochet chance, fragmentation chance, deviation
/// chance, deviation), and whether the level goes by an object's scale and
/// the curve it does so by.
#[derive(Debug, Clone)]
pub struct BallisticPreset {
    pub name: String,
    pub material: i32,
    pub figures: Vec<f32>,
    pub depend_on_scale: bool,
    pub scale_curve: Curve,
}

fn read_curve(r: &mut Reader) -> Result<Curve> {
    let count = r.u32()? as usize;
    if count > 4096 {
        bail!("{count} keys is not a plausible curve");
    }
    let mut keys = Vec::with_capacity(count);
    for _ in 0..count {
        let key = Key { time: r.f32()?, value: r.f32()?, in_slope: r.f32()?, out_slope: r.f32()? };
        r.skip(12)?; // weighted mode and the two weights
        keys.push(key);
    }
    r.skip(12)?; // wrap modes before and after, rotation order
    Ok(Curve { keys })
}
