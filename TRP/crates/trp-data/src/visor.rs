//! The face shield's mask over the first person's view (inventory spec, "The
//! face shield's mask"): the camera's `VisorEffect` on its Resources prefab
//! `Cam2`, the pictures it names, and the camera's `HysteresisFilter`, which
//! fades it in and out (`CameraManager`: the filter's value is the effect's
//! intensity).

use anyhow::{ensure, Context, Result};

use crate::curve::Curve;
use crate::eye_burn::{curve, pointer};
use crate::screen::{Scene, ScenePictures};
use crate::unity::extract::TextureData;
use crate::unity::reader::Reader;
use crate::Install;

/// A face shield's mask kind (`FaceShieldComponent.EMask`, the item's
/// `FaceShieldMask`): none (its scratches, blur and distortion, no frame),
/// narrow (a slit, nothing else), wide (the frame and all the rest).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MaskKind {
    #[default]
    None,
    Narrow,
    Wide,
}

impl MaskKind {
    /// By the game's number (0, 1, 2).
    pub fn of_number(number: i32) -> Option<Self> {
        match number {
            0 => Some(Self::None),
            1 => Some(Self::Narrow),
            2 => Some(Self::Wide),
            _ => None,
        }
    }

    /// By the item's word (`NoMask`, `Narrow`, `Wide`).
    pub fn of_word(word: &str) -> Option<Self> {
        match word {
            "NoMask" => Some(Self::None),
            "Narrow" => Some(Self::Narrow),
            "Wide" => Some(Self::Wide),
            _ => None,
        }
    }
}

/// The mask's figures and pictures, as `Cam2` keeps them.
#[derive(Debug, Clone)]
pub struct VisorData {
    /// `EdgeFadeCurve`, of the intensity: the lower edge's fade.
    pub edge_fade: Curve,
    /// `VisorMasks`: each kind's picture, none where it names none (so the
    /// shader's own white).
    pub masks: Vec<(MaskKind, Option<TextureData>)>,
    /// `Mask`, the picture every kind starts from (`SetDefault`).
    pub mask: TextureData,
    /// `Scratches` and how strongly they show (`ScratcesIntensity`).
    pub scratches: TextureData,
    pub scratches_intensity: f32,
    /// `MinLightValue`, and whether the light counts (`IsAffectedByLight`):
    /// with it off, the scratches' colour is taken by the least light.
    pub min_light: f32,
    pub affected_by_light: bool,
    /// The blur's mask, its size and passes (`BlurMask`, `downsample`,
    /// `blurSize`, `blurIterations`).
    pub blur_mask: TextureData,
    pub downsample: u32,
    pub blur_size: f32,
    pub blur_iterations: u32,
    /// The glass's distortion (`DistortMask`, `DistortIntensity`).
    pub distort_mask: TextureData,
    pub distort_intensity: f32,
    /// The walk's swing across and up and its turn (`DX`, `DY`,
    /// `WalkRotation`), each a curve that repeats after its last key.
    pub dx: Curve,
    pub dy: Curve,
    pub walk_rotation: Curve,
    /// The camera's filter (`HysteresisFilter` on `Cam2`): `Forward` as the
    /// shield comes down, `Backward` as it goes up.
    pub forward: Curve,
    pub backward: Curve,
}

impl VisorData {
    /// The picture a kind's mask is drawn with (`SetMask`): its own from the
    /// list where the list has it (none: white), else the default.
    pub fn mask_of(&self, kind: MaskKind) -> Option<&TextureData> {
        match self.masks.iter().find(|(listed, _)| *listed == kind) {
            Some((_, picture)) => picture.as_ref(),
            None => Some(&self.mask),
        }
    }
}

/// Reads them from the install: `VisorEffect`'s stored fields in their
/// order, and the camera's `HysteresisFilter`'s two curves.
pub fn read(install: &Install) -> Result<VisorData> {
    let (_, file, root) = crate::screen::resource_paths(install)?.into_iter().find(|(path, ..)| path == "cam2").context("no Resources prefab cam2")?;
    let camera = Scene::load_prefab(install, &file, root)?;
    let mut pictures = ScenePictures::new(install)?;
    let mut picture = |(outside, id): (i32, i64)| -> Result<Option<TextureData>> {
        if id == 0 {
            return Ok(None);
        }
        ensure!(outside == 0, "a picture of the visor's outside {file}");
        Ok(Some(pictures.texture(&file, id)?))
    };

    let behaviour = camera.behaviour(0, "VisorEffect").context("Cam2 has no VisorEffect")?;
    let mut r = Reader::new(camera.fields(behaviour));
    let _intensity = r.f32()?;
    let edge_fade = curve(&mut r)?;
    let _shader = pointer(&mut r)?;
    let count = r.i32()?.max(0);
    let mut listed = Vec::new();
    for _ in 0..count {
        let kind = r.i32()?;
        listed.push((kind, pointer(&mut r)?));
    }
    let mask = pointer(&mut r)?;
    let scratches = pointer(&mut r)?;
    let scratches_intensity = r.f32()?;
    let _glass_damage = pointer(&mut r)?;
    let min_light = r.f32()?;
    let affected_by_light = r.i32()? & 0xff != 0;
    let _prefab = pointer(&mut r)?;
    let blur_mask = pointer(&mut r)?;
    let downsample = r.i32()?.clamp(0, 2) as u32;
    let blur_size = r.f32()?;
    let blur_iterations = r.i32()?.clamp(1, 4) as u32;
    let distort_mask = pointer(&mut r)?;
    let distort_intensity = r.f32()?;
    let _swing_value = r.f32()?;
    let dx = curve(&mut r)?;
    let dy = curve(&mut r)?;
    let _hit_swing = curve(&mut r)?;
    let walk_rotation = curve(&mut r)?;

    let mut masks = Vec::new();
    for (kind, at) in listed {
        let kind = MaskKind::of_number(kind).with_context(|| format!("a visor mask of kind {kind}"))?;
        masks.push((kind, picture(at)?));
    }
    let need = |found: Option<TextureData>, name: &str| found.with_context(|| format!("the visor's {name} names no picture"));
    let mask = need(picture(mask)?, "mask")?;
    let scratches = need(picture(scratches)?, "scratches")?;
    let blur_mask = need(picture(blur_mask)?, "blur mask")?;
    let distort_mask = need(picture(distort_mask)?, "distortion mask")?;

    let filter = camera.behaviour(0, "HysteresisFilter").context("Cam2 has no HysteresisFilter")?;
    let mut r = Reader::new(camera.fields(filter));
    let forward = curve(&mut r)?;
    let backward = curve(&mut r)?;

    Ok(VisorData {
        edge_fade,
        masks,
        mask,
        scratches,
        scratches_intensity,
        min_light,
        affected_by_light,
        blur_mask,
        downsample,
        blur_size,
        blur_iterations,
        distort_mask,
        distort_intensity,
        dx,
        dy,
        walk_rotation,
        forward,
        backward,
    })
}
