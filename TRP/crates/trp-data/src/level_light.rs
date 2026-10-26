//! A level's light as the game sets it up (`docs/spec/lighting.md`): its sun
//! (the level's directional light), its level settings (the engine's flat
//! ambient, the fog, a figure for the shaders the sun's shadow does not
//! reach) and the figures its simple sky works its ambient harmonics out of
//! on every frame (`TODSkySimple`), with the dome the sky is drawn on. Read
//! from the scene file of the level's scripts in the install; the range is
//! lit as Factory by day (`FACTORY_DAY`).

use anyhow::{bail, Context, Result};

use crate::curve::Curve;
use crate::eye_burn::{curve, pointer};
use crate::particles::Gradient;
use crate::screen::{scene_paths, tree_of, Scene, ScenePictures};
use crate::unity::extract::TextureData;
use crate::unity::reader::Reader;
use crate::unity::Value;
use crate::Install;

/// The scene of Factory by day's scripts, which holds its weather, its sun
/// and its level settings (Factory as reworked; the older `Factory_Day` is
/// not what the game loads: its sky is not the one recorded).
pub const FACTORY_DAY: &str = "Factory_Rework_Day_Scripts.unity";

/// The engine's class of a light.
const LIGHT: i32 = 108;

/// The engine's class of a mesh renderer.
const MESH_RENDERER: i32 = 23;

/// The engine's class of a material.
const MATERIAL: i32 = 21;

/// A level's directional light (the engine's `Light`).
#[derive(Debug, Clone, PartialEq)]
pub struct Sun {
    /// Its path in the scene (`Weather_Factory/Light`).
    pub path: String,
    /// Its turn in the world, in the engine's axes (quaternion x, y, z, w).
    pub rotation: [f32; 4],
    /// The way its light goes (its transform's forward, the engine's +z
    /// turned by `rotation`).
    pub forward: [f32; 3],
    pub color: [f32; 4],
    pub intensity: f32,
    /// Its shadows (`LightShadows`: 0 none, 1 hard, 2 soft) and their
    /// strength.
    pub shadows: i32,
    pub shadow_strength: f32,
    /// The layers it lights.
    pub culling_mask: u32,
}

/// What a level's settings set (`LevelSettings.ApplySettings`): the
/// engine's ambient and fog and the shaders' level-wide figures.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// `AmbientMode` (3 flat) and its sky colour (also the flat ambient,
    /// `RenderSettings.ambientLight`), equator and ground, and intensity.
    pub ambient_mode: i32,
    pub sky_color: [f32; 4],
    pub equator_color: [f32; 4],
    pub ground_color: [f32; 4],
    pub ambient_intensity: f32,
    /// `_MinAmbientColor`, which the smoke grenades' shaders take.
    pub min_smoke_ambient: [f32; 4],
    pub fog: bool,
    pub fog_color: [f32; 4],
    pub fog_density: f32,
    pub fog_end: f32,
    /// `FogMode`: 1 linear, 2 exponential, 3 exponential squared.
    pub fog_mode: i32,
    pub fog_start: f32,
    pub sun_color: [f32; 4],
    /// `_DirectionLightShadow`: how much of the sun the shaders that take
    /// no shadow leave out.
    pub direction_light_shadow: f32,
    /// The sky's scattering's height falloff and zero level, which the
    /// camera's scattering takes from here.
    pub height_falloff: f32,
    pub zero_level: f32,
    /// `_northDirection`: the degrees a time-of-day sky turns the sun and
    /// moon about up (`TOD_Sky.UpdateCelestials`; 180 where a level has no
    /// settings).
    pub north: f32,
    /// The skybox's material (a pointer: the file among the scene's
    /// externals, 0 itself, and the object's number).
    pub skybox: (i32, i64),
    /// The level's camera prefab (`CameraPrefab`, a pointer to its
    /// `Camera`), which the game makes the player's camera from.
    pub camera: (i32, i64),
}

/// The environment manager's figures for the open air
/// (`EnvironmentManager`): what the camera's exposure follows outside every
/// indoor trigger, and how fast it gets there.
#[derive(Debug, Clone, PartialEq)]
pub struct Outdoor {
    pub fade_time: f32,
    pub exposure_speed: f32,
    /// The exposure's middle grey (`OutdoorExposureOffset`).
    pub exposure_offset: f32,
}

/// The dome's sky as its material draws it (`Skybox/Cubemap`): the cubemap,
/// its tint, its exposure and its turn about up in degrees.
#[derive(Debug, Clone)]
pub struct DomeSky {
    pub cubemap: TextureData,
    pub tint: [f32; 4],
    pub exposure: f32,
    pub rotation: f32,
}

/// The simple sky's figures (`TODSkySimple`).
#[derive(Debug, Clone, PartialEq)]
pub struct SkySimple {
    /// The cubemap it hands the shaders as their reflection
    /// (`AtmosphereCubemap`, `_MySimpleGlobalReflectionProbe`).
    pub atmosphere: (i32, i64),
    /// The sun's colour for the harmonics, by the sun's height from 0 (down)
    /// to 1 (up) (`LightColorGradient`).
    pub light_color: Gradient,
    /// The ambient added from straight above, and from everywhere, by the
    /// same height (`AddTopAmbient`, `AddAmbient`).
    pub add_top_ambient: Gradient,
    pub add_ambient: Gradient,
    /// What a clear sky adds to the ambient (`CloudnessLightColor`).
    pub cloudness: [f32; 4],
    /// By the sun's height from -1 to 1: the bounced sun's intensity, and
    /// the ambient's brightness, saturation and contrast.
    pub bounce: Curve,
    pub brightness: Curve,
    pub saturation: Curve,
    pub contrast: Curve,
    /// How far the harmonics are taken toward a dim grey
    /// (`HarmonicsDaylight`).
    pub harmonics_daylight: f32,
}

/// The dome the sky is drawn on (`Weather_Factory/Atmosphere`): its mesh
/// renderer's material, by the `Skybox/Cubemap` shader.
#[derive(Debug, Clone, PartialEq)]
pub struct Dome {
    pub path: String,
    /// Its material (a pointer, as `Settings::skybox`).
    pub material: (i32, i64),
}

/// A level's light as its scene sets it up.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelLight {
    /// The scene file read (`level525`).
    pub file: String,
    /// The files a pointer's first number counts in, from 1.
    pub externals: Vec<String>,
    pub sun: Sun,
    pub settings: Settings,
    pub sky: SkySimple,
    pub dome: Option<Dome>,
    pub outdoor: Outdoor,
}

impl LevelLight {
    /// Factory by day's, from the install.
    pub fn factory_day(install: &Install) -> Result<Self> {
        Self::of_scene(install, FACTORY_DAY)
    }

    /// The light set up by the scene whose path ends so.
    pub fn of_scene(install: &Install, scene: &str) -> Result<Self> {
        let index = scene_paths(install)?.iter().position(|path| path.ends_with(scene)).with_context(|| format!("no scene {scene} in the build"))?;
        let file = format!("level{index}");
        let scene = Scene::load(install, &file, true).with_context(|| file.clone())?;
        let with_script = |class: &str| (0..scene.nodes.len()).find(|&node| scene.behaviour(node, class).is_some()).with_context(|| format!("no {class} in {file}"));

        let sky_node = with_script("TODSkySimple")?;
        let fields = scene.fields(scene.behaviour(sky_node, "TODSkySimple").expect("found"));
        let (sky, sun_part) = sky_simple(fields).context("TODSkySimple")?;
        ensure_here(sun_part, "the sky's light")?;
        let sun_node = scene.part_owner(sun_part.1).context("the sky's light is no part of the scene")?;
        let sun = light(&scene, sun_node)?;

        let settings_node = with_script("LevelSettings")?;
        let settings = settings(scene.fields(scene.behaviour(settings_node, "LevelSettings").expect("found"))).context("LevelSettings")?;

        // The dome: the mesh renderer beside the sky's script whose
        // material draws a cubemap's sky (its first material).
        let dome = scene.nodes[sky_node].children.iter().find_map(|&child| {
            let bytes = scene.component(child, MESH_RENDERER)?;
            Some(Dome { path: scene.path(child), material: first_material(bytes)? })
        });

        let manager = with_script("EnvironmentManager")?;
        let mut r = Reader::new(scene.fields(scene.behaviour(manager, "EnvironmentManager").expect("found")));
        let outdoor = Outdoor { fade_time: r.f32()?, exposure_speed: r.f32()?, exposure_offset: r.f32()? };
        Ok(Self { file, externals: scene.externals.clone(), sun, settings, sky, dome, outdoor })
    }

    /// The file of the build a pointer of the scene's leads into.
    pub fn file_of(&self, pointer: (i32, i64)) -> Result<String> {
        if pointer.0 == 0 {
            return Ok(self.file.clone());
        }
        let listed = self.externals.get(pointer.0 as usize - 1).with_context(|| format!("no file {} among {}'s", pointer.0, self.file))?;
        Ok(listed.rsplit('/').next().unwrap_or(listed).to_owned())
    }

    /// The dome's sky: its material (read by a material's tree borrowed from
    /// the effects' bundle) and the cubemap it names.
    pub fn dome_sky(&self, install: &Install) -> Result<DomeSky> {
        let dome = self.dome.as_ref().context("the level has no dome")?;
        let file = self.file_of(dome.material)?;
        let tree = tree_of(install, crate::sound::EFFECTS, MATERIAL).context("a material's tree")?;
        let mut pictures = ScenePictures::new(install)?;
        let material = pictures.read_by(&file, dome.material.1, MATERIAL, &tree)?;
        let properties = material.req("m_SavedProperties")?;
        let named = |list: &str, name: &str| -> Option<&Value> {
            properties.get(list)?.as_array()?.iter().find(|pair| pair.get("first").and_then(Value::as_str) == Some(name))?.get("second")
        };
        let float = |name: &str| named("m_Floats", name).and_then(Value::as_f32).with_context(|| format!("the dome's {name}"));
        let tint = named("m_Colors", "_Tint").and_then(|c| Some([c.get("r")?.as_f32()?, c.get("g")?.as_f32()?, c.get("b")?.as_f32()?, c.get("a")?.as_f32()?])).context("the dome's _Tint")?;
        let texture = named("m_TexEnvs", "_Tex").and_then(|env| env.get("m_Texture")?.as_pptr()).context("the dome's _Tex")?;
        let cube_file = match texture.file_id {
            0 => file.clone(),
            n => pictures.file_name(&file, n)?,
        };
        let cubemap = pictures.cubemap(&cube_file, texture.path_id)?;
        Ok(DomeSky { cubemap, tint, exposure: float("_Exposure")?, rotation: float("_Rotation")? })
    }
}

/// A pointer must lead into the scene itself.
fn ensure_here(pointer: (i32, i64), what: &str) -> Result<()> {
    if pointer.0 != 0 || pointer.1 == 0 {
        bail!("{what} is not in the scene ({pointer:?})");
    }
    Ok(())
}

pub(crate) fn color(r: &mut Reader) -> Result<[f32; 4]> {
    Ok([r.f32()?, r.f32()?, r.f32()?, r.f32()?])
}

/// A gradient as a script keeps it (`Gradient`, 2022.3): eight keys of
/// colour and alpha together, eight times for the colours and eight for the
/// alphas (each a share of 65535), its mode, its colour space, how many
/// colour keys and how many alpha keys are used, padded to four bytes.
pub(crate) fn gradient(r: &mut Reader) -> Result<Gradient> {
    let keys: Vec<[f32; 4]> = (0..8).map(|_| color(r)).collect::<Result<_>>()?;
    let color_times: Vec<u16> = (0..8).map(|_| r.u16()).collect::<Result<_>>()?;
    let alpha_times: Vec<u16> = (0..8).map(|_| r.u16()).collect::<Result<_>>()?;
    let (mode, _space, colors, alphas) = (r.u8()?, r.i8()?, r.u8()?.min(8) as usize, r.u8()?.min(8) as usize);
    let time = |t: u16| t as f32 / 65535.0;
    Ok(Gradient {
        colors: (0..colors).map(|k| (time(color_times[k]), [keys[k][0], keys[k][1], keys[k][2]])).collect(),
        alphas: (0..alphas).map(|k| (time(alpha_times[k]), keys[k][3])).collect(),
        stepped: mode == 1,
    })
}

/// `TODSkySimple`'s fields in their order, and its light's pointer.
fn sky_simple(fields: &[u8]) -> Result<(SkySimple, (i32, i64))> {
    let mut r = Reader::new(fields);
    let atmosphere = pointer(&mut r)?;
    let sun = pointer(&mut r)?;
    let _ambient_light = pointer(&mut r)?;
    let light_color = gradient(&mut r)?;
    let add_top_ambient = gradient(&mut r)?;
    let add_ambient = gradient(&mut r)?;
    let cloudness = color(&mut r)?;
    let (bounce, brightness, saturation, contrast) = (curve(&mut r)?, curve(&mut r)?, curve(&mut r)?, curve(&mut r)?);
    let harmonics_daylight = r.f32()?;
    Ok((SkySimple { atmosphere, light_color, add_top_ambient, add_ambient, cloudness, bounce, brightness, saturation, contrast, harmonics_daylight }, sun))
}

/// `LevelSettings`' fields in their order, as far as the fog's and the
/// shaders' figures.
pub(crate) fn settings(fields: &[u8]) -> Result<Settings> {
    let mut r = Reader::new(fields);
    // The camera's, PRISM's and the post-processing's prefabs.
    let camera = pointer(&mut r)?;
    for _ in 0..2 {
        pointer(&mut r)?;
    }
    let skybox = pointer(&mut r)?;
    r.skip(24)?; // the rain's bounds
    let ambient_mode = r.i32()?;
    let (sky_color, equator_color, ground_color) = (color(&mut r)?, color(&mut r)?, color(&mut r)?);
    let ambient_intensity = r.f32()?;
    r.skip(3 * 16 + 4)?; // night vision's ambient
    let min_smoke_ambient = color(&mut r)?;
    let _ssr = r.f32()?;
    let fog = r.u8()? != 0;
    r.align(4);
    let fog_color = color(&mut r)?;
    let (fog_density, fog_end, fog_mode, fog_start) = (r.f32()?, r.f32()?, r.i32()?, r.f32()?);
    let sun_color = color(&mut r)?;
    r.skip(12)?; // the sun's scratches and shafts
    // The lens flares and the sun behind: a script's flags are each padded
    // to four bytes.
    for _ in 0..2 {
        r.u8()?;
        r.align(4);
    }
    let _lying_water = r.f32()?;
    let (direction_light_shadow, height_falloff, zero_level, north) = (r.f32()?, r.f32()?, r.f32()?, r.f32()?);
    Ok(Settings {
        ambient_mode,
        sky_color,
        equator_color,
        ground_color,
        ambient_intensity,
        min_smoke_ambient,
        fog,
        fog_color,
        fog_density,
        fog_end,
        fog_mode,
        fog_start,
        sun_color,
        direction_light_shadow,
        height_falloff,
        zero_level,
        north,
        skybox,
        camera,
    })
}

/// A directional light as the engine keeps it (`Light`, 2022.3's layout):
/// its object, on, kind, shape, colour, intensity, range, spot's angles,
/// cookie's size, then its shadows' kind, resolutions, strength and the
/// rest, its cookie, halo, baking, flare, render mode and the layers it
/// lights.
pub(crate) fn light(scene: &Scene, node: usize) -> Result<Sun> {
    let bytes = scene.component(node, LIGHT).context("the sky's light has no Light")?;
    let mut r = Reader::new(bytes);
    pointer(&mut r)?;
    let _enabled = r.u8()?;
    r.align(4);
    let (kind, _shape) = (r.i32()?, r.i32()?);
    if kind != 1 {
        bail!("the sky's light is not directional (kind {kind})");
    }
    let color = color(&mut r)?;
    let intensity = r.f32()?;
    r.skip(4 * 4)?; // range, spot angle, inner spot angle, cookie size
    let shadows = r.i32()?;
    r.skip(8)?; // resolution, custom resolution
    let shadow_strength = r.f32()?;
    r.skip(3 * 4 + 64)?; // bias, normal bias, near plane, culling matrix
    r.u8()?; // its use
    r.align(4);
    pointer(&mut r)?; // cookie
    r.u8()?; // halo
    r.align(4);
    r.skip(4 * 4)?; // baking: probe occlusion, mask channel, bake type, mixed mode
    r.u8()?; // baked
    r.align(4);
    pointer(&mut r)?; // flare
    let _render_mode = r.i32()?;
    let culling_mask = r.u32()?;
    let rotation = scene.world_rotation(node);
    Ok(Sun { path: scene.path(node), rotation, forward: rotate(rotation, [0.0, 0.0, 1.0]), color, intensity, shadows, shadow_strength, culling_mask })
}

/// A mesh renderer's first material (`Renderer`, 2022.3: its object, on,
/// shadows, shadows received, dynamic occlusion, static shadows, motion
/// vectors, light probes, reflection probes, ray tracing modes and the
/// rest in its first bytes, then its lightmaps, then the list of
/// materials).
fn first_material(bytes: &[u8]) -> Option<(i32, i64)> {
    let mut r = Reader::new(bytes);
    pointer(&mut r).ok()?;
    // Enabled, cast shadows, receive shadows, dynamic occlusion, static
    // shadow caster, motion vectors, light probe usage, reflection probe
    // usage, ray tracing mode, ray trace procedural: ten bytes, padded;
    // the rendering layer mask, renderer priority, lightmap index and its
    // dynamic one, the lightmap's scale and offset twice.
    r.skip(12).ok()?;
    r.skip(4 + 4 + 2 + 2 + 16 + 16).ok()?;
    let count = r.i32().ok()?;
    (count > 0).then(|| pointer(&mut r).ok()).flatten()
}

/// A vector turned by a quaternion (x, y, z, w).
pub fn rotate([x, y, z, w]: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    // v + 2w (q × v) + 2 q × (q × v)
    let q = [x, y, z];
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let t = cross(q, v).map(|c| 2.0 * c);
    let u = cross(q, t);
    [v[0] + w * t[0] + u[0], v[1] + w * t[1] + u[1], v[2] + w * t[2] + u[2]]
}
