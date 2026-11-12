//! Animation clips.
//!
//! A clip is a flat list of float curves stored in three blocks, in this
//! order: streamed (cubic segments with keys at arbitrary times), dense
//! (uniformly sampled) and constant. Bindings then assign runs of curves to
//! properties of transforms, which are named by a hash of their path.

use anyhow::{bail, Result};

use super::assets::{Assets, ObjRef};
use super::class;
use super::value::Value;

mod clip_read;
mod sampling;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Property {
    /// Three curves.
    Position,
    /// Four curves: a quaternion `xyzw`.
    Rotation,
    /// Three curves.
    Scale,
    /// Three curves: Euler angles in degrees. See [`euler_to_quat`].
    Euler,
}

#[derive(Debug, Clone, Copy)]
pub struct Track {
    /// CRC-32 of the transform's path below the animated root.
    pub path: u32,
    pub property: Property,
    /// Index of the first of this track's curves.
    pub curve: usize,
    /// The binding's `customType` field. Zero in every clip seen so far.
    pub custom_type: u8,
}

/// Converts a track's Euler angles to a quaternion `xyzw`.
///
/// The rotations apply about X first, then Y, then Z. That order was
/// established from the data: it is the one that reproduces the rest-pose
/// quaternions stored in the weapon prefab from the first frame of its idle
/// clip. `probe clips <weapon> <clip> check` repeats the comparison.
pub fn euler_to_quat(degrees: [f32; 3]) -> [f32; 4] {
    let [(sx, cx), (sy, cy), (sz, cz)] = degrees.map(|d| (d.to_radians() * 0.5).sin_cos());
    [
        sx * cy * cz - cx * sy * sz,
        cx * sy * cz + sx * cy * sz,
        cx * cy * sz - sx * sy * cz,
        cx * cy * cz + sx * sy * sz,
    ]
}

/// A single curve that feeds the animator parameter of the same name.
#[derive(Debug, Clone, Copy)]
pub struct FloatCurve {
    /// CRC-32 of the parameter's name. For a motion curve, its number
    /// in the engine's fixed list instead.
    pub name: u32,
    pub curve: usize,
    /// 0 for a parameter's curve, [`MOTION_CURVE`] for one of the engine's
    /// own (the root's path among them).
    pub custom_type: u8,
}

/// The `customType` the engine gives its own motion curves.
pub const MOTION_CURVE: u8 = 8;

/// A curve that drives a property of a component of an object: the
/// CRC-32 of the object's path below the root, the component's class, the
/// CRC-32 of the property's name (`m_Color.a`, `m_IsActive`), and the
/// curve's index.
#[derive(Debug, Clone, Copy)]
pub struct Binding {
    pub path: u32,
    pub class: i32,
    pub attribute: u32,
    pub curve: usize,
}

#[derive(Debug, Clone)]
struct StreamedKey {
    curve: usize,
    /// Cubic coefficients, highest power first; the last is the value at the key.
    coeff: [f32; 4],
}

#[derive(Debug, Clone)]
struct StreamedFrame {
    time: f32,
    keys: Vec<StreamedKey>,
}

/// A call a clip makes as its playing passes a time (`AnimationEvent`): the
/// time in seconds from the clip's start, the method called on the
/// animator's object, and the string it is given.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipEvent {
    pub time: f32,
    pub function: String,
    pub data: String,
}

#[derive(Debug, Clone)]
pub struct Clip {
    /// Its animation events (`m_Events`).
    pub events: Vec<ClipEvent>,
    pub name: String,
    pub start_time: f32,
    pub stop_time: f32,
    pub looping: bool,
    pub tracks: Vec<Track>,
    /// Curves bound to the animator itself rather than to a bone.
    pub floats: Vec<FloatCurve>,
    /// For a clip made to be added onto another pose: the value of every
    /// curve in the pose it is measured from. Empty otherwise.
    pub reference_pose: Vec<f32>,
    /// Whether the root's travel stays in the pose rather than going out
    /// as root motion: sideways and ahead, and up and down. The clip's
    /// import settings decide each.
    pub root_stays_level: bool,
    pub root_stays_upright: bool,
    /// Its other curves: a property of a component of an object below the
    /// root (a script's colour, an object's being on).
    pub others: Vec<Binding>,
    streamed: Vec<StreamedFrame>,
    streamed_curves: usize,
    dense_frames: usize,
    dense_curves: usize,
    dense_rate: f32,
    dense_begin: f32,
    dense: Vec<f32>,
    constant: Vec<f32>,
    /// Each streamed curve's keys in time order (a frame's time and the
    /// cubic of the span that starts at it), made on the first sample:
    /// none where the frames are not in time order, which are then walked
    /// as they lie.
    by_curve: std::sync::OnceLock<Option<Vec<Vec<(f32, [f32; 4])>>>>,
}

impl Clip {
}

/// Hash that bindings use for a transform path such as `Weapon_root/weapon`.
pub fn path_hash(path: &str) -> u32 {
    let mut crc = !0u32;
    for &byte in path.as_bytes() {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

#[cfg(test)]
mod tests;

/// Clips a controller plays, keyed by the name its states know each one by.
///
/// For an override controller that is the name of the clip being replaced,
/// with clips of the base controller that are not overridden included
/// under their own names.
pub fn controller_clips(assets: &Assets, controller: ObjRef) -> Result<Vec<(String, ObjRef)>> {
    let v = assets.read(controller)?;
    let resolve = |ptr: &Value| ptr.as_pptr().and_then(|p| assets.resolve_loaded(controller.file, p));
    let mut out: Vec<(String, ObjRef)> = Vec::new();
    if assets.class_id(controller) != Some(class::ANIMATOR_OVERRIDE_CONTROLLER) {
        for clip in v.req("m_AnimationClips")?.as_array().unwrap_or(&[]) {
            let Some(obj) = resolve(clip) else { continue };
            let Some(name) = assets.name_of(obj) else { continue };
            if !out.iter().any(|(n, _)| *n == name) {
                out.push((name, obj));
            }
        }
        return Ok(out);
    }
    if let Some(base) = v.get("m_Controller").and_then(resolve) {
        out = controller_clips(assets, base)?;
    }
    for pair in v.req("m_Clips")?.as_array().unwrap_or(&[]) {
        let (Some(original), Some(replacement)) =
            (pair.get("m_OriginalClip").and_then(resolve), pair.get("m_OverrideClip").and_then(resolve))
        else {
            continue;
        };
        let Some(name) = assets.name_of(original) else { continue };
        match out.iter_mut().find(|(n, _)| *n == name) {
            Some(entry) => entry.1 = replacement,
            None => out.push((name, replacement)),
        }
    }
    Ok(out)
}
