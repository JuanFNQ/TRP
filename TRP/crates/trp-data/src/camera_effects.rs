//! What the camera's health effects are drawn with (health spec, "A hit and
//! health's other screen effects"; grenade spec, "A flash"): the camera's
//! Resources prefab `Cam2` and its effects prefab (`EffectsPrefab`, named by
//! `EffectsController._effectsPrefab`), each read once for all of them.

use anyhow::{ensure, Context, Result};

use crate::curve::Curve;
use crate::eye_burn::{curve, pointer, EyeBurnData};
use crate::screen::{Scene, ScenePictures};
use crate::unity::extract::TextureData;
use crate::unity::reader::Reader;
use crate::Install;

/// The figures of health's screen effects.
#[derive(Debug, Clone)]
pub struct HealthScreenData {
    /// A hit's blur (`FastBlur` on `Cam2`): how long it runs and its curve
    /// over that time (`_hitTime`, `_hitCurve`), and its squares' sides and
    /// how often it is passed down and back (`_upsampleTexDimension`,
    /// `_downsampleTexDimension`, `_blurCount`).
    pub hit_time: f32,
    pub hit_curve: Curve,
    pub hit_squares: [u32; 2],
    pub hit_passes: u32,
    /// Pain's blur (the effects prefab's `CC_RadialBlur`): its most
    /// (`amount`) and its centre.
    pub radial_most: f32,
    pub radial_center: [f32; 2],
    /// Exhaustion's vignette (its `CC_FastVignette`): its centre, sharpness
    /// and most darkness, and the flicker the darkness goes by
    /// (`EffectsCurveData.FastVineteFlicker`).
    pub vignette_center: [f32; 2],
    pub vignette_sharpness: f32,
    pub vignette_darkness: f32,
    pub flicker: Curve,
    /// Near death's blend (its second `CC_Blend`): its most (`amount`).
    pub near_death_most: f32,
}

/// Blood on the screen's figures (`BloodOnScreen` on `Cam2`), as stored
/// (its `Start` changes some: `trp_sim::blood`).
#[derive(Debug, Clone)]
pub struct BloodData {
    pub refraction: f32,
    /// The blur's passes down and back (`DownsamplingCount`).
    pub downsampling: u32,
    /// How long blood shows after the last (`MaxBloodTime`).
    pub max_time: f32,
    pub initial_drops: u32,
    pub color: [f32; 4],
    /// The drops' and the distortion map's side: 2 to this power.
    pub dimension: u32,
    pub start_scale: [f32; 2],
    pub end_scale: [f32; 2],
    pub count_range: [f32; 2],
    pub max_ray: [f32; 2],
    pub start_x: [f32; 2],
    pub start_y: [f32; 2],
    pub end_x: [f32; 2],
    pub end_y: [f32; 2],
    pub lifetime: [f32; 2],
    pub off_glass_scale: f32,
    pub off_glass_lifetime: f32,
    /// The levels as `SetValues` gives them to the shader (`Mode` 0 the
    /// grey's for every channel, else each channel's), over 255: in at
    /// least, at most, the gamma; out at least, at most.
    pub levels: [[f32; 4]; 5],
    /// The heavy bleeding's vignette: its centre and sharpness.
    pub center: [f32; 2],
    pub sharpness: f32,
}

/// The camera's sharpening and its desaturation (`CC_Sharpen`, and the
/// `DesaturateEffect` beside it that provides the desaturation's figures:
/// health spec, "Desaturation"): the sharpening's strength and clamp,
/// whether it desaturates, the ramp picture the brightness looks up, its
/// offset, and the painkiller's ring's least and most radius and falloff.
#[derive(Debug, Clone)]
pub struct SharpenData {
    /// The prefab's; in play the player's graphics setting (`Sharpen` in
    /// `Graphics.ini`, `CameraManager.SetSharpen`) takes its place.
    pub strength: f32,
    pub clamp: f32,
    pub desaturate: bool,
    pub ramp: TextureData,
    pub ramp_offset: [f32; 3],
    pub min_max_radius: [f32; 2],
    pub radius_falloff: f32,
}

impl SharpenData {
    /// A level's camera's (`Cam2` has none: the raid's camera is the
    /// level's own, which the recordings' camera is): the sharpening
    /// (`CC_Sharpen`: past its shader, its strength, clamp and switch of
    /// the desaturation), and the desaturation's figures from the
    /// `DesaturateEffect` beside it (past its shader: the ramp, its
    /// offset, the weather's, the health's and the mask's desaturation, the
    /// ring's least and most radius, its radius and falloff), which the
    /// sharpening takes while it has one (`DesaturateEffectSettingsProvider`).
    pub fn of_camera(install: &Install, file: &str, part: i64) -> Result<Self> {
        let camera = Scene::load_prefab_of_script(install, file, part)?;
        let sharpen = camera.behaviour(0, "CC_Sharpen").context("the camera has no CC_Sharpen")?;
        let mut r = past_shader(&camera, sharpen)?;
        let (strength, clamp) = (r.f32()?, r.f32()?);
        let desaturate = r.u8()? != 0;
        let provider = camera.behaviour(0, "DesaturateEffect").context("the camera has no DesaturateEffect")?;
        let mut r = Reader::new(camera.fields(provider));
        pointer(&mut r)?;
        let (outside, ramp) = pointer(&mut r)?;
        let ramp_file = match outside {
            0 => file.to_owned(),
            n => camera.externals.get(n as usize - 1).map(|name| name.rsplit('/').next().unwrap_or(name).to_owned()).with_context(|| format!("the desaturation's ramp in no file {n} of {file}"))?,
        };
        let ramp_offset = [r.f32()?, r.f32()?, r.f32()?];
        let (_weather, _health, _mask) = (r.f32()?, r.f32()?, r.f32()?);
        let min_max_radius = [r.f32()?, r.f32()?];
        let (_radius, radius_falloff) = (r.f32()?, r.f32()?);
        let ramp = ScenePictures::new(install)?.texture(&ramp_file, ramp)?;
        Ok(Self { strength, clamp, desaturate, ramp, ramp_offset, min_max_radius, radius_falloff })
    }
}

/// All the camera's: the eyes' burn, health's figures, near death's
/// picture (`screen_neardeath_MULT`), blood's figures and its drop's
/// picture (`BloodDrop`'s `_MainTex`).
pub struct CameraEffects {
    pub eye_burn: EyeBurnData,
    pub health: HealthScreenData,
    pub near_death: TextureData,
    pub blood: BloodData,
    pub blood_drop: TextureData,
}

/// A script's fields past its `CC_Base` (`shader`).
fn past_shader<'a>(scene: &'a Scene, behaviour: &crate::screen::Behaviour) -> Result<Reader<'a>> {
    let mut r = Reader::new(scene.fields(behaviour));
    pointer(&mut r)?;
    Ok(r)
}

/// Reads them from the install.
pub fn read(install: &Install) -> Result<CameraEffects> {
    let (_, file, root) = crate::screen::resource_paths(install)?.into_iter().find(|(path, ..)| path == "cam2").context("no Resources prefab cam2")?;
    let camera = Scene::load_prefab(install, &file, root)?;
    let tree = crate::screen::tree_of(install, crate::sound::EFFECTS, 21)?;
    let mut pictures = ScenePictures::new(install)?;
    let eye_burn = crate::eye_burn::read_from(&file, &camera, &tree, &mut pictures)?;

    let blur = camera.behaviour(0, "FastBlur").context("Cam2 has no FastBlur")?;
    let mut r = Reader::new(camera.fields(blur));
    let _value = r.f32()?;
    let _material = pointer(&mut r)?;
    let hit_squares = [r.i32()?.max(1) as u32, r.i32()?.max(1) as u32];
    let hit_passes = r.i32()?.max(0) as u32;
    let hit_time = r.f32()?;
    let hit_curve = curve(&mut r)?;

    let controller = camera.behaviour(0, "EffectsController").context("Cam2 has no EffectsController")?;
    let (outside, prefab) = pointer(&mut Reader::new(camera.fields(controller)))?;
    ensure!(outside == 0, "the effects prefab outside {file}");
    let effects = Scene::load_prefab(install, &file, prefab)?;
    let flicker = curve(&mut Reader::new(effects.fields(effects.behaviour(0, "EffectsCurveData").context("the effects prefab has no EffectsCurveData")?)))?;

    let mut r = past_shader(&effects, effects.behaviour(0, "CC_RadialBlur").context("the effects prefab has no CC_RadialBlur")?)?;
    let radial_most = r.f32()?;
    let _samples = r.i32()?;
    let radial_center = [r.f32()?, r.f32()?];
    let quality = r.i32()?;
    ensure!(quality == 1, "the radial blur's pass {quality}: only pass 1 is built");

    let mut r = past_shader(&effects, effects.behaviour(0, "CC_FastVignette").context("the effects prefab has no CC_FastVignette")?)?;
    let vignette_center = [r.f32()?, r.f32()?];
    let (vignette_sharpness, vignette_darkness) = (r.f32()?, r.f32()?);
    ensure!(r.i32()? & 0xff == 0, "the vignette desaturates: its pass 1 is not built");

    // `Init` takes the prefab's second blend for low health.
    let blend = effects.behaviours_of(0, "CC_Blend").nth(1).context("the effects prefab has no second CC_Blend")?;
    let mut r = past_shader(&effects, blend)?;
    let (outside, texture) = pointer(&mut r)?;
    ensure!(outside == 0, "near death's picture outside {file}");
    let near_death_most = r.f32()?;
    let mode = r.i32()?;
    ensure!(mode == 1, "near death's blend mode {mode}: only mode 1 is built");
    let near_death = pictures.texture(&file, texture)?;

    let (blood, drop_material) = read_blood(&camera)?;
    ensure!(drop_material.0 == 0, "the blood drop's material outside {file}");
    let material = pictures.read_by(&file, drop_material.1, 21, &tree)?;
    let envs = material.req("m_SavedProperties")?.req("m_TexEnvs")?.as_array().context("the drop material's pictures")?;
    let drop = envs
        .iter()
        .find(|pair| pair.get("first").and_then(crate::unity::Value::as_str) == Some("_MainTex"))
        .and_then(|pair| pair.get("second")?.get("m_Texture")?.as_pptr())
        .context("the drop material has no _MainTex")?;
    ensure!(drop.file_id == 0, "the blood drop's picture outside {file}");
    let blood_drop = pictures.texture(&file, drop.path_id)?;

    Ok(CameraEffects {
        eye_burn,
        health: HealthScreenData { hit_time, hit_curve, hit_squares, hit_passes, radial_most, radial_center, vignette_center, vignette_sharpness, vignette_darkness, flicker, near_death_most },
        near_death,
        blood,
        blood_drop,
    })
}

/// The camera's distortion (`DistortCameraFX` on `Cam2`; firing spec, "The
/// muzzle's heat haze"): how far it moves the frame, in pixels at full
/// value, and how many times it blurs its half-size copy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Distortion {
    pub pixels: f32,
    pub blur_iterations: u32,
}

/// Its stored fields in their order (`Shader`, `Distortion`,
/// `BlurIterations`).
pub fn read_distortion(install: &Install) -> Result<Distortion> {
    let (_, file, root) = crate::screen::resource_paths(install)?.into_iter().find(|(path, ..)| path == "cam2").context("no Resources prefab cam2")?;
    let camera = Scene::load_prefab(install, &file, root)?;
    let behaviour = camera.behaviour(0, "DistortCameraFX").context("Cam2 has no DistortCameraFX")?;
    let mut r = Reader::new(camera.fields(behaviour));
    let _shader = pointer(&mut r)?;
    let pixels = r.f32()?;
    let blur_iterations = r.i32()?.clamp(1, 4) as u32;
    Ok(Distortion { pixels, blur_iterations })
}

/// `BloodOnScreen`'s stored fields in their order, and its drop's material.
fn read_blood(camera: &Scene) -> Result<(BloodData, (i32, i64))> {
    let behaviour = camera.behaviour(0, "BloodOnScreen").context("Cam2 has no BloodOnScreen")?;
    let mut r = Reader::new(camera.fields(behaviour));
    let pair = |r: &mut Reader| -> Result<[f32; 2]> { Ok([r.f32()?, r.f32()?]) };
    let _debug = r.i32()?;
    let _color_value = r.f32()?;
    let refraction = r.f32()?;
    let downsampling = r.i32()?.max(0) as u32;
    let max_time = r.f32()?;
    let initial_drops = r.i32()?.max(0) as u32;
    let color = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
    let _unique = r.i32()?;
    let _shader = pointer(&mut r)?;
    let _blit = pointer(&mut r)?;
    let _fade = pointer(&mut r)?;
    let drop_material = pointer(&mut r)?;
    let dimension = r.i32()?.max(0) as u32;
    let (start_scale, end_scale, count_range, max_ray) = (pair(&mut r)?, pair(&mut r)?, pair(&mut r)?, pair(&mut r)?);
    let (start_x, start_y, end_x, end_y) = (pair(&mut r)?, pair(&mut r)?, pair(&mut r)?, pair(&mut r)?);
    let lifetime = pair(&mut r)?;
    let (off_glass_scale, off_glass_lifetime) = (r.f32()?, r.f32()?);
    let mode = r.i32()?;
    // In: the grey's, then red's, green's and blue's least, most and gamma;
    // out: each one's least and most.
    let mut input = [[0.0f32; 3]; 4];
    for channel in &mut input {
        *channel = [r.f32()?, r.f32()?, r.f32()?];
    }
    let mut output = [[0.0f32; 2]; 4];
    for channel in &mut output {
        *channel = pair(&mut r)?;
    }
    let center = pair(&mut r)?;
    let sharpness = r.f32()?;
    let of = |f: &dyn Fn(usize) -> f32| -> [f32; 4] {
        if mode == 0 {
            [f(0), f(0), f(0), 1.0]
        } else {
            [f(1), f(2), f(3), 1.0]
        }
    };
    let levels = [
        of(&|c| input[c][0] / 255.0),
        of(&|c| input[c][1] / 255.0),
        of(&|c| input[c][2]),
        of(&|c| output[c][0] / 255.0),
        of(&|c| output[c][1] / 255.0),
    ];
    Ok((
        BloodData { refraction, downsampling, max_time, initial_drops, color, dimension, start_scale, end_scale, count_range, max_ray, start_x, start_y, end_x, end_y, lifetime, off_glass_scale, off_glass_lifetime, levels, center, sharpness },
        drop_material,
    ))
}
