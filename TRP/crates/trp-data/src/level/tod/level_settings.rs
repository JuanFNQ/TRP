//! The clouds', wind, sky, weather and time-of-day settings of a level.

use super::*;

/// `CloudController`'s fields (on the weather's object): the ambient's
/// colour by the sun's height, the sun's light, the layer's settings.
pub const CLOUD_CONTROLLER: &[(&str, Kind)] = &[("_ambientColor", Kind::Gradient), ("_sun", Pointer), ("_settings", Pointer)];

/// `AmbientHighlight` (on the weather's `ambientManager`): the ambient
/// drawn again over the surfaces of one stencil type after the lighting
/// (`docs/spec/lighting.md`, "The ambient highlight"): its blur toward the
/// ambient from above, its material, each setting (the least and most
/// multiplier, the curve between them by the sun's height, the stencil
/// type: 0 static, 1 characters, 2 hands) and its directional lights added
/// to the harmonics (way, colour, intensity; the way's object not read).
#[derive(Debug, Clone, PartialEq)]
pub struct AmbientHighlight {
    pub blur: f32,
    pub settings: Vec<HighlightSettings>,
    pub additional_lights: Vec<([f32; 4], f32)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HighlightSettings {
    pub min_multiplier: f32,
    pub max_multiplier: f32,
    pub intensity: WeightedCurve,
    pub stencil: i32,
}

impl AmbientHighlight {
    /// Its fields as the scene keeps them: the blur, the material's
    /// pointer, the settings (each two floats, a curve, a whole number),
    /// the lights (each a pointer, a colour and an intensity).
    pub(super) fn read(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        let blur = r.f32()?;
        pointer(&mut r)?;
        let count = r.i32()?.max(0);
        let settings = (0..count)
            .map(|_| Ok(HighlightSettings { min_multiplier: r.f32()?, max_multiplier: r.f32()?, intensity: WeightedCurve::read(&mut r)?, stencil: r.i32()? }))
            .collect::<Result<_>>()?;
        let lights = r.i32()?.max(0);
        let additional_lights = (0..lights)
            .map(|_| {
                pointer(&mut r)?;
                Ok((color(&mut r)?, r.f32()?))
            })
            .collect::<Result<_>>()?;
        Ok(Self { blur, settings, additional_lights })
    }
}

/// `WindController`'s fields (the weather's wind, `Weather/WindZone`): its
/// compute shader, its wires' figures (`WiresController`, inline), the
/// rain's, the clouds' and the far trees' multipliers of the wind, the wind
/// it starts with.
pub const WIND_CONTROLLER: &[(&str, Kind)] = &[
    ("_windShader", Pointer),
    (
        "Wires",
        Class(&[
            ("StrengthMultiplyer", F),
            ("MaxValue", F),
            ("FreqDelta", F),
            ("WindScale", F),
            ("WindFreq", F),
            ("MaxWinFreq", F),
            ("DetailWindAmp", F),
            ("DetailWindScale", F),
            ("DetailWindFreq", F),
            ("WiresMaxWindOffsetMultiplier", F),
            ("WiresMaxWindAmplitudeMultiplier", F),
        ]),
    ),
    ("RainWindMultiplier", F),
    ("CloudWindMultiplier", F),
    ("BilboardTreesWindMultiplier", F),
    ("defaultWindVector", Vec2),
];

/// `TOD_Resources`' fields (on the sky dome): the sky's meshes and
/// materials.
pub const TOD_RESOURCES: &[(&str, Kind)] = &[
    ("Quad", Pointer),
    ("SphereHigh", Pointer),
    ("SphereMedium", Pointer),
    ("SphereLow", Pointer),
    ("IcosphereHigh", Pointer),
    ("IcosphereMedium", Pointer),
    ("IcosphereLow", Pointer),
    ("HalfIcosphereHigh", Pointer),
    ("HalfIcosphereMedium", Pointer),
    ("HalfIcosphereLow", Pointer),
    ("SpaceMaterial", Pointer),
    ("AtmosphereMaterial", Pointer),
    ("SunMaterial", Pointer),
    ("MoonMaterial", Pointer),
];

/// `TOD_DayParameters` and `TOD_NightParameters`.
const DAY: &[(&str, Kind)] = &[
    ("SkyColor", Kind::Gradient),
    ("LightColor", Kind::Gradient),
    ("RayColor", Kind::Gradient),
    ("CloudColor", Kind::Gradient),
    ("AmbientColor", Kind::Gradient),
    ("LightIntensity", F),
    ("ShadowStrength", F),
    ("ColorMultiplier", F),
    ("AmbientMultiplier", F),
    ("ReflectionMultiplier", F),
];

/// `TOD_Sky`'s fields in the order the build keeps them (the cycle's and
/// the atmosphere's, auto-properties' fields, among the first, not where
/// the decompiled class shows them; the sizes add up to the script's 2,340
/// bytes in Customs).
pub const TOD_SKY: &[(&str, Kind)] = &[
    ("ColorSpace", I),
    ("ColorRange", I),
    ("SkyQuality", I),
    ("CloudQuality", I),
    ("MeshQuality", I),
    ("Cycle", Class(&[("Hour", F), ("Day", I), ("Month", I), ("Year", I)])),
    ("World", Class(&[("Latitude", F), ("Longitude", F), ("UTC", F)])),
    (
        "Atmosphere",
        Class(&[("RayleighMultiplier", F), ("MieMultiplier", F), ("Brightness", F), ("ScatteringBrightness", F), ("Contrast", F), ("Directionality", F), ("Fogginess", F)]),
    ),
    ("Day", Class(DAY)),
    ("Night", Class(DAY)),
    ("Sun", Class(&[("MeshColor", Kind::Gradient), ("MeshSize", F), ("MeshBrightness", F), ("MeshContrast", F)])),
    (
        "Moon",
        Class(&[("MeshColor", Kind::Gradient), ("MeshSize", F), ("MeshBrightness", F), ("MeshContrast", F), ("HaloColor", Kind::Gradient), ("HaloSize", F), ("Position", I)]),
    ),
    ("Stars", Class(&[("Tiling", F), ("Brightness", F), ("Position", I)])),
    ("Light", Class(&[("UpdateInterval", F), ("MinimumHeight", F)])),
    ("Fog", Class(&[("Mode", I), ("HeightBias", F)])),
    ("Ambient", Class(&[("Mode", I), ("UpdateInterval", F)])),
    ("Reflection", Class(&[("Mode", I), ("ClearFlags", I), ("CullingMask", I), ("TimeSlicing", I), ("UpdateInterval", F)])),
    ("rotationSpeed", F),
];

/// The game's time-of-day controller (`ToDController`).
const TOD_CONTROLLER: &[(&str, Kind)] = &[
    ("RayleighMultiplierMinMax", Vec2),
    ("RayleighMultiplier", Kind::Curve),
    ("MieMultiplierMinMax", Vec2),
    ("MieMultiplier", Kind::Curve),
    ("DirectionalityMult", F),
    ("Directionality", Kind::Curve),
    ("BrightnessMinMax", Vec2),
    ("Brightness", Kind::Curve),
    ("ScatteringBrightnessMultiplier", F),
    ("ScatteringBrightness", Kind::Curve),
    ("BrightnessOvercast", F),
    ("ContrastMinMax", Vec2),
    ("Contrast", Kind::Curve),
    ("MoonLightMinMax", Vec2),
    ("LightColor", Kind::Gradient),
    ("AmbientLightScript", Pointer),
    ("AmbientHighlightScript", Pointer),
    ("HighlightWithoutTopHarmonics", Flag),
    ("TopHarmonicIntensity", Kind::Curve),
    ("HorizontHarmonicIntensity", Kind::Curve),
    ("BounceHarmonicIntensity", Kind::Curve),
    ("ForwarHarmonicIntensity", Kind::Curve),
    ("ForwarHarmonicMultiplyer", Kind::Gradient),
    ("BackwardHarmonicMultiplyer", Kind::Gradient),
    ("AddTopAmbient", Kind::Gradient),
    ("AddAmbient", Kind::Gradient),
    ("AmbientBrightness", Kind::Curve),
    ("AmbientSaturation", Kind::Curve),
    ("AmbientContrast", Kind::Curve),
    ("CloudnessLightColor", Color),
];

/// `WeatherController`'s fields as far as its time-of-day controller and
/// the two cubemaps' multipliers after it: the wind, the rain and the clouds
/// are components of its own (pointers), the clouds' remap an asset.
pub const WEATHER: &[(&str, Kind)] = &[
    ("WindController", Pointer),
    ("RainController", Pointer),
    ("PlayerCamera", Pointer),
    ("CloudController", Pointer),
    ("CloudsRemap", Pointer),
    ("MinMaxAmmount", Vec2),
    ("MinLyingWater", F),
    ("FogMultyplyer", Kind::Curve),
    ("TimeOfDayController", Class(TOD_CONTROLLER)),
    ("isDrawDebugGui", Flag),
    ("CubemapDayMult", F),
    ("CubemapNightMult", F),
];

/// A level lit by its time of day, as its scripts' scene sets it up.
#[derive(Debug, Clone)]
pub struct TimeOfDayLevel {
    /// The scene file read (`level15`).
    pub file: String,
    pub externals: Vec<String>,
    /// `TOD_Sky`'s fields.
    pub sky: Fields,
    /// `WeatherController`'s (its controller's as `TimeOfDayController.*`).
    pub weather: Fields,
    /// The sky dome's turn and place in the world (`TOD_Components.DomeTransform`,
    /// the sky's own object), in the engine's axes.
    pub dome_rotation: [f32; 4],
    pub dome_position: [f32; 3],
    /// The sun's light (its turn as the level file has it; the sky turns it
    /// on every frame).
    pub sun: Sun,
    pub settings: Settings,
    pub outdoor: Outdoor,
    /// The atmosphere's mesh (the dome's `Atmosphere`, an icosphere of
    /// radius one), whose vertices its shader works the sky's scattering
    /// out at.
    pub atmosphere_mesh: BuildMesh,
    /// The level's indoor triggers (`IndoorTrigger`, under the environment
    /// manager): the camera's figures within each.
    pub indoor_triggers: Vec<IndoorTrigger>,
    /// The sun's light's place in the world (the clouds' cookie is laid
    /// from it).
    pub sun_position: [f32; 3],
    /// The weather's `CloudController`'s fields, where it has one, and its
    /// `WindController`'s.
    pub cloud_controller: Option<Fields>,
    /// The ambient manager's `AmbientHighlight`, where it has one.
    pub ambient_highlight: Option<AmbientHighlight>,
    pub wind_controller: Option<Fields>,
    /// The dome's `TOD_Resources`' fields.
    pub resources: Option<Fields>,
}

/// An indoor trigger's figures (`IndoorTrigger`'s fields, in order: whether
/// a bunker, the fade time, the bunker's depth and low pass, the exposure's
/// speed and its middle grey, the rain's volume, the bounds).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IndoorTrigger {
    pub bunker: bool,
    pub fade_time: f32,
    pub exposure_speed: f32,
    pub exposure_offset: f32,
}

impl TimeOfDayLevel {
    /// The figures most of the level's triggers that are no bunker share
    /// (Customs': 0.14, 4 and 1 s in 71 of its 79), as many as share them.
    pub fn common_indoor(&self) -> Option<(IndoorTrigger, usize)> {
        let mut counts: Vec<(IndoorTrigger, usize)> = Vec::new();
        for trigger in self.indoor_triggers.iter().filter(|t| !t.bunker) {
            match counts.iter_mut().find(|(t, _)| t == trigger) {
                Some((_, n)) => *n += 1,
                None => counts.push((*trigger, 1)),
            }
        }
        counts.into_iter().max_by_key(|(_, n)| *n)
    }
}

impl TimeOfDayLevel {
    /// Customs' (`custom_Scripts.unity`).
    pub fn customs(install: &Install) -> Result<Self> {
        Self::of_scene(install, CUSTOMS)
    }

    pub fn of_scene(install: &Install, scene_end: &str) -> Result<Self> {
        let index = scene_paths(install)?.iter().position(|path| path.ends_with(scene_end)).with_context(|| format!("no scene {scene_end} in the build"))?;
        let file = format!("level{index}");
        let scene = Scene::load(install, &file, true).with_context(|| file.clone())?;
        let with_script = |class: &str| (0..scene.nodes.len()).find(|&node| scene.behaviour(node, class).is_some()).with_context(|| format!("no {class} in {file}"));
        let script = |node: usize, class: &str| scene.fields(scene.behaviour(node, class).expect("found"));

        let dome = with_script("TOD_Sky")?;
        let sky = Fields::read(script(dome, "TOD_Sky"), TOD_SKY).context("TOD_Sky")?;
        let weather_node = with_script("WeatherController")?;
        let weather = Fields::read(script(weather_node, "WeatherController"), WEATHER).context("WeatherController")?;
        let cloud_controller = scene.behaviour(weather_node, "CloudController").map(|b| Fields::read(scene.fields(b), CLOUD_CONTROLLER).context("CloudController")).transpose()?;
        let resources = scene.behaviour(dome, "TOD_Resources").map(|b| Fields::read(scene.fields(b), TOD_RESOURCES).context("TOD_Resources")).transpose()?;
        let wind_controller = match weather.pointer("WindController")? {
            (0, id) => scene.behaviours.iter().find(|b| b.id == id).map(|b| Fields::read(scene.fields(b), WIND_CONTROLLER).context("WindController")).transpose()?,
            _ => None,
        };
        let sun_node = scene.below(dome, "Light").context("the sky dome has no Light")?;
        let sun = light(&scene, sun_node)?;
        let level = with_script("LevelSettings")?;
        let settings = settings(script(level, "LevelSettings")).context("LevelSettings")?;
        let ambient_highlight = (0..scene.nodes.len())
            .find_map(|node| scene.behaviour(node, "AmbientHighlight"))
            .map(|b| AmbientHighlight::read(scene.fields(b)).context("AmbientHighlight"))
            .transpose()?;
        let manager = with_script("EnvironmentManager")?;
        let mut r = Reader::new(script(manager, "EnvironmentManager"));
        let outdoor = Outdoor { fade_time: r.f32()?, exposure_speed: r.f32()?, exposure_offset: r.f32()? };
        // The mesh filter's mesh, after its own object's pointer.
        let atmosphere = scene.below(dome, "Atmosphere").context("the sky dome has no Atmosphere")?;
        let filter = scene.component(atmosphere, MESH_FILTER).context("the atmosphere has no mesh filter")?;
        let mut r = Reader::new(filter.get(12..).context("a short mesh filter")?);
        let mesh = (r.i32()?, r.i64()?);
        let externals = scene.externals.clone();
        let mesh_file = if mesh.0 == 0 { file.clone() } else { externals.get(mesh.0 as usize - 1).context("the atmosphere's mesh's file")?.rsplit('/').next().unwrap_or_default().to_owned() };
        let atmosphere_mesh = build_mesh(install, &mesh_file, mesh.1).context("the atmosphere's mesh")?;
        let indoor_triggers = scene
            .behaviours
            .iter()
            .filter(|b| scene.behaviour(b.node, "IndoorTrigger").is_some_and(|t| t.id == b.id))
            .filter_map(|b| {
                let mut r = Reader::new(scene.fields(b));
                let bunker = r.i32().ok()? & 0xff != 0;
                let fade_time = r.f32().ok()?;
                r.skip(8).ok()?;
                Some(IndoorTrigger { bunker, fade_time, exposure_speed: r.f32().ok()?, exposure_offset: r.f32().ok()? })
            })
            .collect();
        Ok(Self {
            atmosphere_mesh,
            indoor_triggers,
            sun_position: scene.world_position(sun_node),
            cloud_controller,
            ambient_highlight,
            wind_controller,
            resources,
            externals: scene.externals.clone(),
            dome_rotation: scene.world_rotation(dome),
            dome_position: scene.world_position(dome),
            file,
            sky,
            weather,
            sun,
            settings,
            outdoor,
        })
    }

    /// The file of the build a pointer of the scene's leads into.
    pub fn file_of(&self, pointer: (i32, i64)) -> Result<String> {
        if pointer.0 == 0 {
            return Ok(self.file.clone());
        }
        let listed = self.externals.get(pointer.0 as usize - 1).with_context(|| format!("no file {} among {}'s", pointer.0, self.file))?;
        Ok(listed.rsplit('/').next().unwrap_or(listed).to_owned())
    }
}
