//! The eyes' burn after a flash (grenade spec, "A flash"; `EyeBurn` on the
//! camera's Resources prefab `Cam2`): its figures and curves as the prefab
//! keeps them, and the pictures of its spots' and its trail's materials
//! (`_MainTex`; every material's ramp is empty, so the engine's
//! `grayscaleRamp`).

use anyhow::{bail, Context, Result};

use crate::curve::{Curve, Key};
use crate::unity::extract::TextureData;
use crate::unity::reader::Reader;
use crate::unity::Value;
use crate::Install;

/// What a flash's burn is drawn with.
pub struct EyeBurnData {
    /// The spots' pictures, by the materials' order (`_eyeBurnEffectMaterial`),
    /// and the trail's (`_eyeBurnTrailMaterial`).
    pub spots: Vec<TextureData>,
    pub trail: Vec<TextureData>,
    /// The size of a spot at no strength and at full (`_scaleMinMax`).
    pub scale: [f32; 2],
    /// How long a spot lasts at full strength (`_eyeBurnMaxTime`).
    pub max_time: f32,
    /// A trail's spots: one each so much of the view's turn, each shifted
    /// so much of its size on (`TrailSpotEvery`, `TrailSpotShift`), at most
    /// so many (`MaxTrailLength`), each sized by its place
    /// (`trailDecreaseCurve`).
    pub trail_every: f32,
    pub trail_shift: f32,
    pub max_trail: u32,
    pub trail_curve: Curve,
    /// A spot's size by its life left (`_scaleCoefCurve`).
    pub scale_curve: Curve,
}

/// A pointer as a script keeps it: the file's place among the file's
/// externals (0 itself) and the object's number.
pub(crate) fn pointer(r: &mut Reader) -> Result<(i32, i64)> {
    Ok((r.i32()?, r.i64()?))
}

/// An animation curve as a script keeps it (`AnimationCurve`: its keys,
/// each time, value, slopes in and out, weighting and weights; then its
/// wraps and its rotation order). Weighted keys are not read.
pub(crate) fn curve(r: &mut Reader) -> Result<Curve> {
    let count = r.i32()?;
    let mut keys = Vec::new();
    for _ in 0..count.max(0) {
        let (time, value, in_slope, out_slope) = (r.f32()?, r.f32()?, r.f32()?, r.f32()?);
        let weighted = r.i32()?;
        let _weights = (r.f32()?, r.f32()?);
        if weighted != 0 {
            bail!("a weighted key, not read");
        }
        keys.push(Key { time, value, in_slope, out_slope });
    }
    r.skip(12)?;
    Ok(Curve { keys })
}

/// Reads the burn's figures and pictures from the install (with the rest
/// of the camera's: `crate::camera_effects`).
pub fn read(install: &Install) -> Result<EyeBurnData> {
    Ok(crate::camera_effects::read(install)?.eye_burn)
}

/// The same from the camera's prefab already read (`scene`, of `file`), with
/// a material's tree and the reader of the file's pictures.
pub(crate) fn read_from(file: &str, scene: &crate::screen::Scene, tree: &crate::unity::typetree::TypeTree, pictures: &mut crate::screen::ScenePictures) -> Result<EyeBurnData> {
    let behaviour = scene.behaviour(0, "EyeBurn").context("Cam2 has no EyeBurn")?;
    let mut r = Reader::new(scene.fields(behaviour));
    let _screen_material = pointer(&mut r)?;
    let pointers = |r: &mut Reader| -> Result<Vec<(i32, i64)>> { (0..r.i32()?.max(0)).map(|_| pointer(r)).collect() };
    let spot_materials = pointers(&mut r)?;
    let trail_materials = pointers(&mut r)?;
    let scale = [r.f32()?, r.f32()?];
    let (max_time, trail_every, trail_shift) = (r.f32()?, r.f32()?, r.f32()?);
    let trail_curve = curve(&mut r)?;
    let max_trail = r.i32()?.max(0) as u32;
    let scale_curve = curve(&mut r)?;
    // Each material's own picture, read by a material's tree borrowed from
    // a bundle of the install's.
    let mut main_texture = |(outside, id): (i32, i64)| -> Result<TextureData> {
        anyhow::ensure!(outside == 0, "a material outside {file}");
        let material = pictures.read_by(file, id, 21, tree)?;
        let envs = material.req("m_SavedProperties")?.req("m_TexEnvs")?.as_array().context("the material's pictures")?;
        let texture = envs
            .iter()
            .find(|pair| pair.get("first").and_then(Value::as_str) == Some("_MainTex"))
            .and_then(|pair| pair.get("second")?.get("m_Texture")?.as_pptr())
            .context("the material has no _MainTex")?;
        anyhow::ensure!(texture.file_id == 0, "a picture outside {file}");
        pictures.texture(file, texture.path_id)
    };
    let spots = spot_materials.into_iter().map(&mut main_texture).collect::<Result<Vec<_>>>()?;
    let trail = trail_materials.into_iter().map(&mut main_texture).collect::<Result<Vec<_>>>()?;
    Ok(EyeBurnData { spots, trail, scale, max_time, trail_every, trail_shift, max_trail, trail_curve, scale_curve })
}
