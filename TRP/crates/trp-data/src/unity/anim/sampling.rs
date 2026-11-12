//! Sampling a clip's curves at a time, plain or smoothed.

use super::*;

impl Clip {
    pub fn duration(&self) -> f32 {
        (self.stop_time - self.start_time).max(0.0)
    }

    /// Evaluates every curve at `time` seconds from the start of the clip.
    /// Each streamed curve's keys in time order, where the frames lie in
    /// it (they do in every clip read: the engine stores them so).
    fn keys_by_curve(&self) -> Option<&Vec<Vec<(f32, [f32; 4])>>> {
        self.by_curve
            .get_or_init(|| {
                let ordered = self.streamed.windows(2).all(|pair| pair[0].time <= pair[1].time);
                ordered.then(|| {
                    let mut by_curve = vec![Vec::new(); self.streamed_curves];
                    for frame in &self.streamed {
                        for key in &frame.keys {
                            if let Some(list) = by_curve.get_mut(key.curve) {
                                list.push((frame.time, key.coeff));
                            }
                        }
                    }
                    by_curve
                })
            })
            .as_ref()
    }

    /// Each streamed curve's latest key at or before `t`: by each curve's
    /// keys where they are had, else by walking the frames up to `t`.
    fn latest_keys(&self, t: f32) -> Vec<Option<(f32, [f32; 4])>> {
        if let Some(by_curve) = self.keys_by_curve() {
            return by_curve.iter().map(|keys| keys.partition_point(|(time, _)| *time <= t).checked_sub(1).map(|at| keys[at])).collect();
        }
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
        active
    }

    /// One curve's value at a time, as `sample` gives it among the rest
    /// (the same keys and arithmetic), for a reader of a few curves (the
    /// root's travel and turn).
    pub fn sample_curve(&self, time: f32, curve: usize) -> f32 {
        let t = self.start_time + time.clamp(0.0, self.duration());
        if curve < self.streamed_curves {
            let slot = match self.keys_by_curve() {
                Some(by_curve) => by_curve.get(curve).and_then(|keys| keys.partition_point(|(time, _)| *time <= t).checked_sub(1).map(|at| keys[at])),
                None => self.latest_keys(t).get(curve).copied().flatten(),
            };
            return match slot {
                Some((key_time, c)) => {
                    let dt = if key_time.is_finite() { t - key_time } else { 0.0 };
                    ((c[0] * dt + c[1]) * dt + c[2]) * dt + c[3]
                }
                None => 0.0,
            };
        }
        let dense = curve - self.streamed_curves;
        if dense < self.dense_curves {
            if self.dense_frames == 0 {
                return 0.0;
            }
            let frame = ((t - self.dense_begin) * self.dense_rate).clamp(0.0, (self.dense_frames - 1) as f32);
            let lo = frame.floor() as usize;
            let hi = (lo + 1).min(self.dense_frames - 1);
            let mix = frame - lo as f32;
            let (a, b) = (lo * self.dense_curves, hi * self.dense_curves);
            return self.dense[a + dense] + (self.dense[b + dense] - self.dense[a + dense]) * mix;
        }
        self.constant.get(dense - self.dense_curves).copied().unwrap_or(0.0)
    }

    pub fn sample(&self, time: f32, values: &mut Vec<f32>) {
        let t = self.start_time + time.clamp(0.0, self.duration());
        values.clear();
        values.resize(self.curve_count(), 0.0);

        // Each key holds the cubic for the span that starts at it, so the
        // value of a curve comes from its latest key at or before `t`.
        let active = self.latest_keys(t);
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
    /// difference showing 30 times a second. Each span between two samples is a cubic that leaves
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
            match self.keys_by_curve() {
                // Each curve's two latest keys at or before `t` and its first
                // two after it within the window, as the walk below finds.
                Some(by_curve) => {
                    for (slot, keys) in around.iter_mut().zip(by_curve) {
                        let at = keys.partition_point(|(time, _)| *time <= t);
                        slot.before = [at.checked_sub(2).map(|i| keys[i]), at.checked_sub(1).map(|i| keys[i])];
                        let within = |i: usize| keys.get(i).filter(|(time, _)| *time <= t + 2.0 * longest).map(|(time, coeff)| (*time, coeff[3]));
                        slot.after = [within(at), within(at).and(within(at + 1))];
                    }
                }
                None => {
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
