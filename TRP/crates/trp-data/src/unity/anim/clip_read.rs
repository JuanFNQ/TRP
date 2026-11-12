//! Reading a clip's curves, keeping, reducing and cutting them.

use super::*;

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
            by_curve: std::sync::OnceLock::new(),
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
        self.with_curves(&wanted, &keep_track, &keep_float, true)
    }

    /// A copy holding only the curves `wanted` marks, by their numbers (one
    /// mark a curve), in their order: the tracks and animator curves whose
    /// curves it keeps, and each curve's keys, samples, figure and reference
    /// as they are here. Each curve's value is its own (`sample`,
    /// `sample_smooth`, `sample_curve` go curve by curve), so every curve it
    /// keeps samples to the bit as it does here: an animator splits its
    /// clips so, into the curves of the bones it always works out and the
    /// rest (`trp_sim::animator`). The frames left without a key are dropped
    /// only where the frames lie in time order, as every clip read has
    /// them; where they do not, every frame is kept, so that a walk of them
    /// stops where this clip's does.
    pub fn part(&self, wanted: &[bool]) -> Clip {
        assert_eq!(wanted.len(), self.curve_count(), "one mark a curve");
        let ordered = self.streamed.windows(2).all(|pair| pair[0].time <= pair[1].time);
        let kept = |curve: usize| wanted.get(curve).copied().unwrap_or(false);
        self.with_curves(wanted, &|t: &Track| kept(t.curve), &|f: &FloatCurve| kept(f.curve), ordered)
    }

    /// The copy `reduced` and `part` make: the curves `wanted` marks, the
    /// tracks and animator curves the two tests keep, renumbered in order;
    /// the frames left without a key dropped where `drop_empty`.
    fn with_curves(&self, wanted: &[bool], keep_track: &dyn Fn(&Track) -> bool, keep_float: &dyn Fn(&FloatCurve) -> bool, drop_empty: bool) -> Clip {
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
                (!keys.is_empty() || !drop_empty).then(|| StreamedFrame { time: frame.time, keys })
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
            by_curve: std::sync::OnceLock::new(),
            others: Vec::new(),
        }
    }
}
