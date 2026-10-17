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
}

impl Clip {
    pub fn read(assets: &Assets, obj: ObjRef) -> Result<Self> {
        Self::of(&assets.read(obj)?)
    }

    /// A clip from its read fields.
    pub fn of(v: &Value) -> Result<Self> {
        let muscle = v.req("m_MuscleClip")?;
        let data = muscle.req("m_Clip.data")?;

        let streamed_clip = data.req("m_StreamedClip")?;
        let words = streamed_clip.req("data")?.as_uints().unwrap_or(&[]);
        let mut streamed = Vec::new();
        let mut at = 0;
        while at + 2 <= words.len() {
            let time = f32::from_bits(words[at]);
            let count = words[at + 1] as usize;
            at += 2;
            if at + count * 5 > words.len() {
                bail!("streamed clip data is truncated");
            }
            let mut keys = Vec::with_capacity(count);
            for _ in 0..count {
                keys.push(StreamedKey {
                    curve: words[at] as usize,
                    coeff: [1, 2, 3, 4].map(|i| f32::from_bits(words[at + i])),
                });
                at += 5;
            }
            streamed.push(StreamedFrame { time, keys });
        }

        let dense_clip = data.req("m_DenseClip")?;
        let constant = data.req("m_ConstantClip.data")?.as_floats().unwrap_or(&[]).to_vec();

        let events = v
            .get("m_Events")
            .and_then(Value::as_array)
            .unwrap_or(&[])
            .iter()
            .map(|e| ClipEvent {
                time: e.get("time").and_then(Value::as_f32).unwrap_or(0.0),
                function: e.get("functionName").and_then(Value::as_str).unwrap_or("").to_owned(),
                data: e.get("data").and_then(Value::as_str).unwrap_or("").to_owned(),
            })
            .collect();
        let mut clip = Clip {
            events,
            name: v.req("m_Name")?.as_str().unwrap_or("").to_owned(),
            start_time: muscle.req("m_StartTime")?.as_f32().unwrap_or(0.0),
            stop_time: muscle.req("m_StopTime")?.as_f32().unwrap_or(0.0),
            looping: muscle.req("m_LoopTime")?.as_bool().unwrap_or(false),
            tracks: Vec::new(),
            floats: Vec::new(),
            reference_pose: muscle.get("m_ValueArrayReferencePose").and_then(Value::as_floats).unwrap_or(&[]).to_vec(),
            root_stays_level: muscle.get("m_LoopBlendPositionXZ").and_then(Value::as_bool).unwrap_or(false),
            root_stays_upright: muscle.get("m_LoopBlendPositionY").and_then(Value::as_bool).unwrap_or(false),
            streamed,
            streamed_curves: streamed_clip.req("curveCount")?.as_usize().unwrap_or(0),
            dense_frames: dense_clip.req("m_FrameCount")?.as_usize().unwrap_or(0),
            dense_curves: dense_clip.req("m_CurveCount")?.as_usize().unwrap_or(0),
            dense_rate: dense_clip.req("m_SampleRate")?.as_f32().unwrap_or(30.0),
            dense_begin: dense_clip.req("m_BeginTime")?.as_f32().unwrap_or(0.0),
            dense: dense_clip.req("m_SampleArray")?.as_floats().unwrap_or(&[]).to_vec(),
            constant,
            others: Vec::new(),
        };
        if clip.dense.len() < clip.dense_frames * clip.dense_curves {
            bail!("dense clip data is truncated");
        }

        let mut curve = 0;
        for b in v.req("m_ClipBindingConstant.genericBindings")?.as_array().unwrap_or(&[]) {
            let is_transform = b.req("typeID")?.as_i64() == Some(class::TRANSFORM as i64);
            let property = match (is_transform, b.req("attribute")?.as_i64().unwrap_or(0)) {
                (true, 1) => Some(Property::Position),
                (true, 2) => Some(Property::Rotation),
                (true, 3) => Some(Property::Scale),
                (true, 4) => Some(Property::Euler),
                _ => None,
            };
            let width = match property {
                Some(Property::Rotation) => 4,
                Some(_) => 3,
                None => 1,
            };
            let custom_type = b.req("customType")?.as_i64().unwrap_or(0) as u8;
            if let Some(property) = property {
                clip.tracks.push(Track { path: b.req("path")?.as_u32().unwrap_or(0), property, curve, custom_type });
            } else if b.req("typeID")?.as_i64() == Some(class::ANIMATOR as i64) {
                let name = b.req("attribute")?.as_u32().unwrap_or(0);
                clip.floats.push(FloatCurve { name, curve, custom_type });
            } else {
                let (path, attribute) = (b.req("path")?.as_u32().unwrap_or(0), b.req("attribute")?.as_u32().unwrap_or(0));
                clip.others.push(Binding { path, class: b.req("typeID")?.as_i64().unwrap_or(0) as i32, attribute, curve });
            }
            curve += width;
        }
        if curve > clip.curve_count() {
            bail!("clip {:?} binds {curve} curves but stores {}", clip.name, clip.curve_count());
        }
        Ok(clip)
    }

    pub fn curve_count(&self) -> usize {
        self.streamed_curves + self.dense_curves + self.constant.len()
    }

    /// How the clip keeps its curves: how many as cubics from key to key,
    /// how many as samples at a steady rate with straight lines between
    /// (and that rate, in samples a second), and how many as one figure.
    /// A curve's number says which: the cubics come first, then the
    /// samples, then the figures.
    pub fn kept(&self) -> (usize, usize, f32, usize) {
        (self.streamed_curves, self.dense_curves, self.dense_rate, self.constant.len())
    }

    /// The curves that trace the root's path through the clip, x y z, if
    /// the clip stores one. The engine marks these as motion curves and
    /// numbers the three components 7, 8 and 9.
    pub fn root_path(&self) -> Option<[usize; 3]> {
        let component = |number: u32| {
            self.floats.iter().find(|f| f.custom_type == MOTION_CURVE && f.name == number).map(|f| f.curve)
        };
        Some([component(7)?, component(8)?, component(9)?])
    }

    /// The curves that trace the root's turning through the clip, as a
    /// quaternion x y z w, if the clip stores one: the engine's motion
    /// curves 10 to 13.
    pub fn root_turn(&self) -> Option<[usize; 4]> {
        let component = |number: u32| {
            self.floats.iter().find(|f| f.custom_type == MOTION_CURVE && f.name == number).map(|f| f.curve)
        };
        Some([component(10)?, component(11)?, component(12)?, component(13)?])
    }

    /// A copy holding only the chosen bone tracks and animator curves,
    /// which is far cheaper to sample than a clip of a whole body.
    pub fn reduced(&self, keep_track: impl Fn(&Track) -> bool, keep_float: impl Fn(&FloatCurve) -> bool) -> Clip {
        // Old curve index to new, in the old order so that each block
        // (streamed, dense, constant) stays together.
        let mut wanted = vec![false; self.curve_count()];
        for t in self.tracks.iter().filter(|t| keep_track(t)) {
            let width = if t.property == Property::Rotation { 4 } else { 3 };
            wanted[t.curve..t.curve + width].iter_mut().for_each(|w| *w = true);
        }
        for f in self.floats.iter().filter(|f| keep_float(f)) {
            wanted[f.curve] = true;
        }
        let mut new_index = vec![usize::MAX; wanted.len()];
        let mut next = 0;
        for (old, keep) in wanted.iter().enumerate() {
            if *keep {
                new_index[old] = next;
                next += 1;
            }
        }
        let kept = |range: std::ops::Range<usize>| wanted[range].iter().filter(|w| **w).count();
        let (streamed_end, dense_end) = (self.streamed_curves, self.streamed_curves + self.dense_curves);
        let streamed_curves = kept(0..streamed_end);
        let dense_curves = kept(streamed_end..dense_end);

        let streamed = self
            .streamed
            .iter()
            .filter_map(|frame| {
                let keys: Vec<StreamedKey> = frame
                    .keys
                    .iter()
                    .filter(|k| wanted.get(k.curve).copied().unwrap_or(false))
                    .map(|k| StreamedKey { curve: new_index[k.curve], coeff: k.coeff })
                    .collect();
                (!keys.is_empty()).then(|| StreamedFrame { time: frame.time, keys })
            })
            .collect();
        let mut dense = Vec::with_capacity(self.dense_frames * dense_curves);
        for frame in 0..self.dense_frames {
            let row = &self.dense[frame * self.dense_curves..(frame + 1) * self.dense_curves];
            dense.extend(row.iter().enumerate().filter(|(c, _)| wanted[streamed_end + c]).map(|(_, v)| *v));
        }
        let constant = self.constant.iter().enumerate().filter(|(c, _)| wanted[dense_end + c]).map(|(_, v)| *v).collect();
        let reference_pose = if self.reference_pose.len() == wanted.len() {
            self.reference_pose.iter().enumerate().filter(|(c, _)| wanted[*c]).map(|(_, v)| *v).collect()
        } else {
            Vec::new()
        };
        Clip {
            events: self.events.clone(),
            name: self.name.clone(),
            start_time: self.start_time,
            stop_time: self.stop_time,
            looping: self.looping,
            tracks: self
                .tracks
                .iter()
                .filter(|t| keep_track(t))
                .map(|t| Track { curve: new_index[t.curve], ..*t })
                .collect(),
            floats: self
                .floats
                .iter()
                .filter(|f| keep_float(f))
                .map(|f| FloatCurve { curve: new_index[f.curve], ..*f })
                .collect(),
            reference_pose,
            root_stays_level: self.root_stays_level,
            root_stays_upright: self.root_stays_upright,
            streamed,
            streamed_curves,
            dense_frames: self.dense_frames,
            dense_curves,
            dense_rate: self.dense_rate,
            dense_begin: self.dense_begin,
            dense,
            constant,
            others: Vec::new(),
        }
    }

    pub fn duration(&self) -> f32 {
        (self.stop_time - self.start_time).max(0.0)
    }

    /// Evaluates every curve at `time` seconds from the start of the clip.
    pub fn sample(&self, time: f32, values: &mut Vec<f32>) {
        let t = self.start_time + time.clamp(0.0, self.duration());
        values.clear();
        values.resize(self.curve_count(), 0.0);

        // Each key holds the cubic for the span that starts at it, so the
        // value of a curve comes from its latest key at or before `t`.
        let mut active: Vec<Option<(f32, [f32; 4])>> = vec![None; self.streamed_curves];
        for frame in &self.streamed {
            if frame.time > t {
                break;
            }
            for key in &frame.keys {
                if let Some(slot) = active.get_mut(key.curve) {
                    *slot = Some((frame.time, key.coeff));
                }
            }
        }
        for (value, slot) in values.iter_mut().zip(&active) {
            if let Some((key_time, c)) = slot {
                // The opening frame sits at minus infinity and holds constants.
                let dt = if key_time.is_finite() { t - key_time } else { 0.0 };
                *value = ((c[0] * dt + c[1]) * dt + c[2]) * dt + c[3];
            }
        }

        if self.dense_frames > 0 {
            let frame = ((t - self.dense_begin) * self.dense_rate).clamp(0.0, (self.dense_frames - 1) as f32);
            let lo = frame.floor() as usize;
            let hi = (lo + 1).min(self.dense_frames - 1);
            let mix = frame - lo as f32;
            let (a, b) = (lo * self.dense_curves, hi * self.dense_curves);
            for c in 0..self.dense_curves {
                values[self.streamed_curves + c] = self.dense[a + c] + (self.dense[b + c] - self.dense[a + c]) * mix;
            }
        }

        let base = self.streamed_curves + self.dense_curves;
        values[base..].copy_from_slice(&self.constant);
    }

    /// Evaluates every curve at `time` as [`Clip::sample`] does, but for
    /// the curves kept as samples at a steady rate, which here go through
    /// their samples **on a curve without corners** in place of straight
    /// lines. **For drawing only**: the engine joins such samples by
    /// straight lines, and so does everything the simulation goes by.
    ///
    /// Straight lines are exact at the samples and wrong between them
    /// wherever the thing sampled curves, and two such curves that undo
    /// one another (a weapon turned in the hand, and a hand's place on the
    /// weapon that stays where it was in the world) then leave their
    /// difference showing 30 times a second (`docs/findings.md`,
    /// 2026-10-13). Each span between two samples is a cubic that leaves
    /// the one sample and reaches the other with the slope there: the
    /// mean of the two steps either side of the sample, none where the
    /// curve turns back there, and never more than three times the smaller
    /// step, so that a jump from one sample to the next is not overshot
    /// before or after it. A curve that is a straight line stays one.
    ///
    /// **A curve kept as cubics is taken the same way where it is samples
    /// under another name**: a key at every sample of the clip and a
    /// straight line from each to the next (the Five-seveN's look is kept
    /// so, and breaks at every sample as the AKM's does). A span is taken
    /// on the curve only if it is straight and no longer than a sample
    /// and a half; a span that is a curve of its maker's, or a straight
    /// line across many samples, is left as it is.
    pub fn sample_smooth(&self, time: f32, values: &mut Vec<f32>) {
        self.sample(time, values);
        let t = self.start_time + time.clamp(0.0, self.duration());
        // The slope at a sample from the step into it and the step out
        // of it (each a slope itself: per sample, or per second).
        let slope = |into: f32, out: f32| -> f32 {
            if into * out <= 0.0 {
                return 0.0;
            }
            let mean = (into + out) * 0.5;
            mean.signum() * mean.abs().min(3.0 * into.abs().min(out.abs()))
        };
        // The cubic from one value to another over a span of some length,
        // leaving with one slope and reaching with another, a share `u`
        // of the way along.
        let curve = |from: f32, to: f32, leaving: f32, reaching: f32, long: f32, u: f32| -> f32 {
            let (h00, h10, h01, h11) = (2.0 * u * u * u - 3.0 * u * u + 1.0, u * u * u - 2.0 * u * u + u, -2.0 * u * u * u + 3.0 * u * u, u * u * u - u * u);
            h00 * from + h10 * long * leaving + h01 * to + h11 * long * reaching
        };

        if self.streamed_curves > 0 && self.dense_rate > 0.0 {
            let longest = 1.5 / self.dense_rate;
            // For each curve, its two keys at or before `t` (the older
            // first: a time and the span's cubic) and the two after (a
            // time and the value there).
            #[derive(Clone, Copy, Default)]
            struct Around {
                before: [Option<(f32, [f32; 4])>; 2],
                after: [Option<(f32, f32)>; 2],
            }
            let mut around = vec![Around::default(); self.streamed_curves];
            for frame in &self.streamed {
                if frame.time > t + 2.0 * longest {
                    break;
                }
                for key in &frame.keys {
                    let Some(slot) = around.get_mut(key.curve) else { continue };
                    if frame.time <= t {
                        slot.before = [slot.before[1], Some((frame.time, key.coeff))];
                    } else if let Some(free) = slot.after.iter_mut().find(|after| after.is_none()) {
                        *free = Some((frame.time, key.coeff[3]));
                    }
                }
            }
            for (c, slot) in around.iter().enumerate() {
                let (Some((key_time, coeff)), Some((next_time, next))) = (slot.before[1], slot.after[0]) else { continue };
                let long = next_time - key_time;
                if !(key_time.is_finite() && next_time.is_finite() && long > 1e-6 && long <= longest) {
                    continue;
                }
                let (from, step) = (coeff[3], (next - coeff[3]) / long);
                // Straight: what the span's cubic has past its line is
                // nothing beside the step it makes.
                let bent = coeff[0].abs() * long * long * long + coeff[1].abs() * long * long;
                if bent > 1e-3 * (next - from).abs() + 1e-7 {
                    continue;
                }
                // A neighbouring span counts if it is a sample long too.
                let leaving = match slot.before[0] {
                    Some((time, before)) if time.is_finite() && key_time - time > 1e-6 && key_time - time <= longest => slope((from - before[3]) / (key_time - time), step),
                    _ => step,
                };
                let reaching = match slot.after[1] {
                    Some((time, after)) if time.is_finite() && time - next_time > 1e-6 && time - next_time <= longest => slope(step, (after - next) / (time - next_time)),
                    _ => step,
                };
                values[c] = curve(from, next, leaving, reaching, long, ((t - key_time) / long).clamp(0.0, 1.0));
            }
        }

        if self.dense_frames < 3 {
            return;
        }
        let frame = ((t - self.dense_begin) * self.dense_rate).clamp(0.0, (self.dense_frames - 1) as f32);
        let lo = (frame.floor() as usize).min(self.dense_frames - 2);
        let u = frame - lo as f32;
        let (before, after) = (lo.checked_sub(1), (lo + 2 < self.dense_frames).then_some(lo + 2));
        let row = |frame: usize| &self.dense[frame * self.dense_curves..(frame + 1) * self.dense_curves];
        let (from, to) = (row(lo), row(lo + 1));
        for c in 0..self.dense_curves {
            let step = to[c] - from[c];
            // At the clip's ends there is one step and no other.
            let leaving = before.map_or(step, |frame| slope(from[c] - row(frame)[c], step));
            let reaching = after.map_or(step, |frame| slope(step, row(frame)[c] - to[c]));
            values[self.streamed_curves + c] = curve(from[c], to[c], leaving, reaching, 1.0, u);
        }
    }
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
mod tests {
    use super::*;

    #[test]
    fn path_hash_is_crc32() {
        assert_eq!(path_hash(""), 0);
        assert_eq!(path_hash("123456789"), 0xcbf4_3926);
    }

    /// Angles from the first frame of the AKM idle clip for the left hand
    /// marker, against the rotation the prefab stores for the same bone.
    #[test]
    fn euler_order_reproduces_a_stored_rest_rotation() {
        let q = euler_to_quat([-76.80162, 17.92335, 125.08804]);
        let rest = [-0.391, -0.488, 0.732, 0.271];
        for (a, b) in q.iter().zip(rest) {
            assert!((a - b).abs() < 2e-3, "{q:?} != {rest:?}");
        }
    }

    /// A clip of curves kept as samples and nothing else, ten a second.
    fn sampled(curves: usize, samples: Vec<f32>) -> Clip {
        let frames = samples.len() / curves;
        Clip {
            events: Vec::new(),
            name: String::new(),
            start_time: 0.0,
            stop_time: (frames - 1) as f32 / 10.0,
            looping: false,
            tracks: Vec::new(),
            floats: Vec::new(),
            reference_pose: Vec::new(),
            root_stays_level: false,
            root_stays_upright: false,
            streamed: Vec::new(),
            streamed_curves: 0,
            dense_frames: frames,
            dense_curves: curves,
            dense_rate: 10.0,
            dense_begin: 0.0,
            dense: samples,
            constant: Vec::new(),
            others: Vec::new(),
        }
    }

    /// The curve for drawing goes through every sample, keeps a straight
    /// line straight, follows a curved one between its samples where
    /// straight lines cut across it, has no corner at a sample, and does
    /// not overshoot a jump.
    #[test]
    fn the_curve_for_drawing_has_no_corners_and_overshoots_nothing() {
        // Three curves: a straight line, a parabola, and a jump.
        let line = |t: f32| 3.0 * t - 1.0;
        let parabola = |t: f32| 40.0 * t * t;
        let jump = |frame: usize| if frame < 5 { 0.0 } else { 1.0 };
        let clip = sampled(3, (0..10).flat_map(|frame| [line(frame as f32 / 10.0), parabola(frame as f32 / 10.0), jump(frame)]).collect());
        let (mut straight, mut smooth) = (Vec::new(), Vec::new());
        // At the samples the two agree.
        for frame in 0..10 {
            clip.sample(frame as f32 / 10.0, &mut straight);
            clip.sample_smooth(frame as f32 / 10.0, &mut smooth);
            for c in 0..3 {
                assert!((straight[c] - smooth[c]).abs() < 1e-5, "sample {frame}, curve {c}: {} and {}", straight[c], smooth[c]);
            }
        }
        let (mut worst_straight, mut worst_smooth) = (0.0f32, 0.0f32);
        let mut last: Option<Vec<f32>> = None;
        let mut slopes: Vec<f32> = Vec::new();
        for step in 0..=900 {
            let t = step as f32 / 1000.0;
            clip.sample(t, &mut straight);
            clip.sample_smooth(t, &mut smooth);
            assert!((smooth[0] - line(t)).abs() < 1e-4, "the line at {t}: {}", smooth[0]);
            // Away from the clip's two ends, where there is one step only.
            if (0.1..=0.8).contains(&t) {
                worst_straight = worst_straight.max((straight[1] - parabola(t)).abs());
                worst_smooth = worst_smooth.max((smooth[1] - parabola(t)).abs());
            }
            assert!((-1e-6..=1.0 + 1e-6).contains(&smooth[2]), "the jump is overshot at {t}: {}", smooth[2]);
            if let Some(last) = &last {
                slopes.push((smooth[1] - last[1]) * 1000.0);
            }
            last = Some(smooth.clone());
        }
        // Straight lines cut across the parabola by a tenth (40 times a
        // step squared over eight); the curve follows it.
        assert!((worst_straight - 0.1).abs() < 1e-3, "{worst_straight}");
        assert!(worst_smooth < 1e-3, "{worst_smooth}");
        // No corner: the slope from one thousandth to the next never
        // changes by more than the parabola's own bend allows (80 a
        // second each second), where straight lines change it by 8 at
        // every sample.
        let sharpest = slopes.windows(2).map(|pair| (pair[1] - pair[0]).abs()).fold(0.0f32, f32::max);
        assert!(sharpest < 0.5, "{sharpest}");
    }

    /// A clip of one curve kept as cubics, each key a time, a value and
    /// the cubic's two upper terms (none for a straight line to the next
    /// key); its samples would be ten a second.
    fn keyed(keys: &[(f32, f32, [f32; 2])]) -> Clip {
        let mut clip = sampled(1, vec![0.0, 0.0]);
        (clip.dense_frames, clip.dense_curves, clip.dense) = (0, 0, Vec::new());
        clip.stop_time = keys.last().unwrap().0;
        clip.streamed_curves = 1;
        for (index, (time, value, bend)) in keys.iter().enumerate() {
            let line = keys.get(index + 1).map_or(0.0, |next| (next.1 - value) / (next.0 - time));
            clip.streamed.push(StreamedFrame { time: *time, keys: vec![StreamedKey { curve: 0, coeff: [bend[0], bend[1], line, *value] }] });
        }
        clip.streamed.push(StreamedFrame { time: f32::INFINITY, keys: Vec::new() });
        clip
    }

    /// A curve kept as cubics with a key at every sample and straight
    /// lines between is taken on the curve for drawing as samples are; a
    /// straight line across many samples, and a span that is a curve of
    /// its maker's, are left as they are.
    #[test]
    fn keys_at_every_sample_are_drawn_as_samples_are() {
        let parabola = |t: f32| 40.0 * t * t;
        let clip = keyed(&(0..10).map(|key| (key as f32 / 10.0, parabola(key as f32 / 10.0), [0.0, 0.0])).collect::<Vec<_>>());
        let (mut straight, mut smooth) = (Vec::new(), Vec::new());
        let (mut worst_straight, mut worst_smooth) = (0.0f32, 0.0f32);
        for step in 100..=800 {
            let t = step as f32 / 1000.0;
            clip.sample(t, &mut straight);
            clip.sample_smooth(t, &mut smooth);
            worst_straight = worst_straight.max((straight[0] - parabola(t)).abs());
            worst_smooth = worst_smooth.max((smooth[0] - parabola(t)).abs());
        }
        assert!((worst_straight - 0.1).abs() < 1e-3 && worst_smooth < 1e-3, "{worst_straight}, {worst_smooth}");
        // A straight line a second long, between two rests: it stays one.
        let ramp = keyed(&[(0.0, 0.0, [0.0, 0.0]), (0.1, 0.0, [0.0, 0.0]), (1.1, 5.0, [0.0, 0.0]), (1.2, 5.0, [0.0, 0.0])]);
        // And a curved span a sample long: it stays the curve it is.
        let curved = keyed(&[(0.0, 0.0, [0.0, 0.0]), (0.1, 1.0, [300.0, -40.0]), (0.2, 2.0, [0.0, 0.0]), (0.3, 3.0, [0.0, 0.0])]);
        for (clip, times) in [(&ramp, [0.15, 0.6, 1.05]), (&curved, [0.12, 0.15, 0.18])] {
            for t in times {
                clip.sample(t, &mut straight);
                clip.sample_smooth(t, &mut smooth);
                assert_eq!(straight, smooth, "at {t}");
            }
        }
    }

    #[test]
    fn dense_curves_interpolate_and_constants_follow() {
        let clip = Clip {
            events: Vec::new(),
            name: String::new(),
            start_time: 0.0,
            stop_time: 1.0,
            looping: false,
            tracks: Vec::new(),
            floats: Vec::new(),
            reference_pose: Vec::new(),
            root_stays_level: false,
            root_stays_upright: false,
            streamed: vec![
                StreamedFrame {
                    time: f32::NEG_INFINITY,
                    keys: vec![StreamedKey { curve: 0, coeff: [0.0, 0.0, 0.0, 5.0] }],
                },
                // From 0.5 s the curve is 1 + 2 * dt.
                StreamedFrame { time: 0.5, keys: vec![StreamedKey { curve: 0, coeff: [0.0, 0.0, 2.0, 1.0] }] },
                StreamedFrame { time: f32::INFINITY, keys: Vec::new() },
            ],
            streamed_curves: 1,
            dense_frames: 3,
            dense_curves: 1,
            dense_rate: 2.0,
            dense_begin: 0.0,
            dense: vec![0.0, 10.0, 20.0],
            constant: vec![9.0],
            others: Vec::new(),
        };
        let mut v = Vec::new();
        clip.sample(0.25, &mut v);
        assert_eq!(v, [5.0, 5.0, 9.0]);
        clip.sample(0.75, &mut v);
        assert_eq!(v, [1.5, 15.0, 9.0]);
        clip.sample(5.0, &mut v);
        assert_eq!(v, [2.0, 20.0, 9.0]);

        // Cut down to the first and last curves, the same values come out
        // under their new numbers.
        let mut whole = clip.clone();
        whole.floats = (0..3).map(|curve| FloatCurve { name: curve as u32, curve, custom_type: 0 }).collect();
        whole.reference_pose = vec![1.0, 2.0, 3.0];
        let cut = whole.reduced(|_| false, |f| f.name != 1);
        assert_eq!(cut.curve_count(), 2);
        assert_eq!(cut.floats.iter().map(|f| (f.name, f.curve)).collect::<Vec<_>>(), [(0, 0), (2, 1)]);
        assert_eq!(cut.reference_pose, [1.0, 3.0]);
        cut.sample(0.75, &mut v);
        assert_eq!(v, [1.5, 9.0]);
    }
}

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
