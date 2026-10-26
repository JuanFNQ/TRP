//! A level's clouds and its moon as the game sets them up
//! (`docs/spec/lighting.md`, "Customs' clouds" and "The moon"): the
//! weather's cloud controller (`EFT.Rendering.Clouds.CloudController`: the
//! ambient's colour by the sun's height, the sun, the layer's settings),
//! the layer's settings (`CloudLayer`, Customs' `CloudLayerSettings_main`),
//! the weather's remap of its cloudiness to the cloud map's four channels
//! (`CloudRemapData`, Customs' `CloudRemapData_main`), the cloud map
//! itself; and the sky's moon (`TOD_Resources`: the sphere its quality
//! picks, the moon's material's picture and its tiling).
//!
//! The settings and the remap are script assets of the level's own files,
//! their fields after the engine's head, in their classes' order.

use anyhow::{ensure, Context, Result};

use crate::particles::Gradient;
use crate::screen::{build_mesh, object_bytes, tree_of, BuildMesh, ScenePictures};
use crate::tod::{Fields, Kind, TimeOfDayLevel, WeightedCurve};
use crate::unity::extract::TextureData;
use crate::unity::reader::Reader;
use crate::unity::Value;
use crate::Install;

use Kind::*;

/// `CloudLayer`'s serialized fields: the layer's opacity, the shadows'
/// multiplier, tint and size, the planet's middle and radius, then its one
/// layer (`CloudMap`: the map, its altitude, turn, tint and exposure, the
/// wind's way and speeds, whether lit, its density, the ambient's share).
/// The opacities of the map's channels, the steps, the resolutions and the
/// shadows' casting are not kept: the remap and the quality set them.
pub const CLOUD_LAYER: &[(&str, Kind)] = &[
    ("Opacity", F),
    ("ShadowMultiplier", F),
    ("ShadowTint", Color),
    ("ShadowSize", F),
    ("PlanetCenterRadius", Color),
    (
        "LayerA",
        Class(&[
            ("CloudTexture", Pointer),
            ("Altitude", F),
            ("Rotation", F),
            ("Tint", Color),
            ("Exposure", F),
            ("ScrollOrientation", F),
            ("MinScrollSpeed", F),
            ("MaxScrollSpeed", F),
            ("Lighting", Flag),
            ("Thickness", F),
            ("AmbientProbeDimmer", F),
        ]),
    ),
];

/// `CloudRemapData`'s: a curve for each of the map's channels by the
/// weather's cloudiness from 0 to 1.
pub const CLOUD_REMAP: &[(&str, Kind)] = &[("Layer1", Class(&[("R", Curve), ("G", Curve), ("B", Curve), ("A", Curve)]))];

/// The layer's settings as kept.
#[derive(Debug, Clone, PartialEq)]
pub struct CloudLayerSettings {
    pub opacity: f32,
    /// As kept (the shadows' bake takes an eighth of it).
    pub shadow_multiplier: f32,
    pub shadow_tint: [f32; 4],
    /// The cookie's width on the ground, in metres.
    pub shadow_size: f32,
    /// The planet's middle (xyz) and radius (w), in metres.
    pub planet_center_radius: [f32; 4],
    pub map: (i32, i64),
    pub altitude: f32,
    /// Degrees.
    pub rotation: f32,
    pub tint: [f32; 4],
    /// In EV, taken off (`ConvertEv100ToExposure(-Exposure)`).
    pub exposure: f32,
    /// The wind's way, degrees from the world's x.
    pub scroll_orientation: f32,
    /// Kilometres an hour.
    pub min_scroll_speed: f32,
    pub max_scroll_speed: f32,
    pub lighting: bool,
    pub thickness: f32,
    pub ambient_probe_dimmer: f32,
}

impl CloudLayerSettings {
    pub fn read(fields: &Fields) -> Result<Self> {
        let l = |name: &str| format!("LayerA.{name}");
        Ok(Self {
            opacity: fields.f32("Opacity")?,
            shadow_multiplier: fields.f32("ShadowMultiplier")?,
            shadow_tint: fields.color("ShadowTint")?,
            shadow_size: fields.f32("ShadowSize")?,
            planet_center_radius: fields.color("PlanetCenterRadius")?,
            map: fields.pointer(&l("CloudTexture"))?,
            altitude: fields.f32(&l("Altitude"))?,
            rotation: fields.f32(&l("Rotation"))?,
            tint: fields.color(&l("Tint"))?,
            exposure: fields.f32(&l("Exposure"))?,
            scroll_orientation: fields.f32(&l("ScrollOrientation"))?,
            min_scroll_speed: fields.f32(&l("MinScrollSpeed"))?,
            max_scroll_speed: fields.f32(&l("MaxScrollSpeed"))?,
            lighting: fields.flag(&l("Lighting"))?,
            thickness: fields.f32(&l("Thickness"))?,
            ambient_probe_dimmer: fields.f32(&l("AmbientProbeDimmer"))?,
        })
    }
}

/// The remap's four curves.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CloudRemap {
    pub r: WeightedCurve,
    pub g: WeightedCurve,
    pub b: WeightedCurve,
    pub a: WeightedCurve,
}

impl CloudRemap {
    pub fn read(fields: &Fields) -> Result<Self> {
        Ok(Self { r: fields.curve("Layer1.R")?.clone(), g: fields.curve("Layer1.G")?.clone(), b: fields.curve("Layer1.B")?.clone(), a: fields.curve("Layer1.A")?.clone() })
    }

    /// The map's channels' opacities at a cloudiness from 0 to 1
    /// (`RemapCloudValue`).
    pub fn at(&self, normalized_cloudiness: f32) -> [f32; 4] {
        [self.r.evaluate(normalized_cloudiness), self.g.evaluate(normalized_cloudiness), self.b.evaluate(normalized_cloudiness), self.a.evaluate(normalized_cloudiness)]
    }
}

/// A level's clouds.
#[derive(Debug, Clone)]
pub struct LevelClouds {
    /// The ambient's colour added to the clouds' harmonics by the sun's
    /// height (`_ambientColor`, at half the sun's way's height plus a half).
    pub ambient_color: Gradient,
    pub settings: CloudLayerSettings,
    pub remap: CloudRemap,
    /// The cloud map (RGBA, 8 bits a channel, rows bottom to top), its
    /// edges as its own sampler takes them.
    pub map: TextureData,
}

impl LevelClouds {
    /// The clouds of a level read with its time of day (Customs' weather).
    pub fn read(install: &Install, level: &TimeOfDayLevel) -> Result<Self> {
        let controller = level.cloud_controller.as_ref().context("the level's weather has no cloud controller")?;
        let ambient_color = controller.gradient("_ambientColor")?.clone();
        let at = controller.pointer("_settings")?;
        let settings_file = level.file_of(at)?;
        let settings = CloudLayerSettings::read(&Fields::read(&asset_fields(install, &settings_file, at.1)?, CLOUD_LAYER).context("the cloud layer's settings")?)?;
        let at = level.weather.pointer("CloudsRemap")?;
        let remap = CloudRemap::read(&Fields::read(&asset_fields(install, &level.file_of(at)?, at.1)?, CLOUD_REMAP).context("the clouds' remap")?)?;
        // The map's pointer counts from the settings' own file.
        let mut pictures = ScenePictures::new(install)?;
        let map_file = pictures.file_name(&settings_file, settings.map.0)?;
        let map = pictures.texture(&map_file, settings.map.1).context("the cloud map")?;
        ensure!(map.format == 4, "the cloud map is of format {}, not RGBA32", map.format);
        Ok(Self { ambient_color, settings, remap, map })
    }
}

/// The sky's moon: the sphere its mesh quality draws it with, and its
/// material's picture with its tiling (`_MainTex`, its scale and offset).
#[derive(Debug, Clone)]
pub struct LevelMoon {
    pub mesh: BuildMesh,
    pub picture: TextureData,
    pub tiling: [f32; 4],
}

impl LevelMoon {
    pub fn read(install: &Install, level: &TimeOfDayLevel) -> Result<Self> {
        let resources = level.resources.as_ref().context("the sky has no resources")?;
        let sphere = match level.sky.int("MeshQuality")? {
            0 => "SphereLow",
            1 => "SphereMedium",
            _ => "SphereHigh",
        };
        let at = resources.pointer(sphere)?;
        let mesh = build_mesh(install, &level.file_of(at)?, at.1).with_context(|| format!("the moon's {sphere}"))?;
        let at = resources.pointer("MoonMaterial")?;
        let file = level.file_of(at)?;
        let tree = tree_of(install, crate::sound::EFFECTS, MATERIAL).context("a material's tree")?;
        let mut pictures = ScenePictures::new(install)?;
        let material = pictures.read_by(&file, at.1, MATERIAL, &tree)?;
        let env = material
            .req("m_SavedProperties")?
            .get("m_TexEnvs")
            .and_then(Value::as_array)
            .and_then(|envs| envs.iter().find(|pair| pair.get("first").and_then(Value::as_str) == Some("_MainTex")))
            .and_then(|pair| pair.get("second"))
            .context("the moon's _MainTex")?;
        let pair = |name: &str| -> Option<[f32; 2]> {
            let v = env.get(name)?;
            Some([v.get("x")?.as_f32()?, v.get("y")?.as_f32()?])
        };
        let (scale, offset) = (pair("m_Scale").context("the moon's tiling")?, pair("m_Offset").context("the moon's offset")?);
        let texture = env.get("m_Texture").and_then(Value::as_pptr).context("the moon's picture")?;
        let picture_file = pictures.file_name(&file, texture.file_id)?;
        let picture = pictures.texture(&picture_file, texture.path_id).context("the moon's picture")?;
        Ok(Self { mesh, picture, tiling: [scale[0], scale[1], offset[0], offset[1]] })
    }
}

/// The engine's class number of a material.
const MATERIAL: i32 = 21;

/// A script asset's own fields, after the engine's head (its object, its
/// switch, its script, its name).
fn asset_fields(install: &Install, file: &str, id: i64) -> Result<Vec<u8>> {
    let (class, bytes) = object_bytes(install, file, id)?;
    ensure!(class == 114, "{file} {id} is of class {class}, not a script's asset");
    let mut r = Reader::new(&bytes);
    r.skip(12 + 4 + 12)?;
    let name = r.u32()? as usize;
    r.skip(name)?;
    r.align(4);
    Ok(bytes[bytes.len() - r.remaining()..].to_vec())
}
