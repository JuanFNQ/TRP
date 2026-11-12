//! Unity animation curves: keyed values joined by cubic Hermite segments.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Key {
    pub time: f32,
    pub value: f32,
    pub in_slope: f32,
    pub out_slope: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Curve {
    pub keys: Vec<Key>,
}

/// How a curve goes on before its first key or after its last, as a
/// serialized `AnimationCurve` keeps it (`m_PreInfinity`, `m_PostInfinity`:
/// 0 back and forth, 1 over again, 2 held). `AnimationCurve.Evaluate` takes
/// a time outside the keys back into them by it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Wrap {
    /// Back and forth over the keys' span (`WrapMode.PingPong`).
    PingPong,
    /// Over again from the first key (`WrapMode.Loop`).
    Repeat,
    /// The end key's value held (`WrapMode.ClampForever`), and anything
    /// the file names that is not one of the other two.
    #[default]
    Clamp,
}

impl Wrap {
    /// The wrap by the number a file keeps for it.
    pub fn of(number: i64) -> Self {
        match number {
            0 => Self::PingPong,
            1 => Self::Repeat,
            _ => Self::Clamp,
        }
    }
}

impl Curve {
    /// Value at `t`, a time outside the keys taken back into them by the
    /// wrap on that side (`before` the first key, `after` the last), as
    /// `AnimationCurve.Evaluate` does: a curve that repeats starts again at
    /// its first key, one that ping-pongs runs back from its last to its
    /// first and on again, one held keeps its end value.
    pub fn evaluate_wrapped(&self, t: f32, before: Wrap, after: Wrap) -> f32 {
        let (Some(first), Some(last)) = (self.keys.first(), self.keys.last()) else { return 0.0 };
        let (from, span) = (first.time, last.time - first.time);
        if span <= 0.0 || (t >= from && t <= last.time) {
            return self.evaluate(t);
        }
        // The remainder taken whole (not of the time over the span), so a
        // curve that starts over gives what it always gave to the last bit.
        let t = match if t < from { before } else { after } {
            Wrap::PingPong => {
                let k = (t - from).rem_euclid(2.0 * span);
                from + if k > span { 2.0 * span - k } else { k }
            }
            Wrap::Repeat => from + (t - from).rem_euclid(span),
            Wrap::Clamp => t,
        };
        self.evaluate(t)
    }

    /// A curve through the given points with flat tangents at every key.
    pub fn flat(points: &[(f32, f32)]) -> Self {
        Self { keys: points.iter().map(|&(time, value)| Key { time, value, in_slope: 0.0, out_slope: 0.0 }).collect() }
    }

    /// A curve through the given points with straight segments between them.
    pub fn linear(points: &[(f32, f32)]) -> Self {
        let slope = |a: (f32, f32), b: (f32, f32)| (b.1 - a.1) / (b.0 - a.0);
        let keys = (0..points.len())
            .map(|i| Key {
                time: points[i].0,
                value: points[i].1,
                in_slope: if i > 0 { slope(points[i - 1], points[i]) } else { 0.0 },
                out_slope: if i + 1 < points.len() { slope(points[i], points[i + 1]) } else { 0.0 },
            })
            .collect();
        Self { keys }
    }

    /// Value at `t`. Outside the keyed range the end values hold.
    pub fn evaluate(&self, t: f32) -> f32 {
        let (Some(first), Some(last)) = (self.keys.first(), self.keys.last()) else { return 0.0 };
        if t <= first.time {
            return first.value;
        }
        if t >= last.time {
            return last.value;
        }
        let next = self.keys.partition_point(|k| k.time <= t);
        let (a, b) = (self.keys[next - 1], self.keys[next]);
        let span = b.time - a.time;
        // An infinite slope marks a stepped segment.
        if span <= 0.0 || !a.out_slope.is_finite() || !b.in_slope.is_finite() {
            return a.value;
        }
        let s = (t - a.time) / span;
        let (s2, s3) = (s * s, s * s * s);
        (2.0 * s3 - 3.0 * s2 + 1.0) * a.value
            + (s3 - 2.0 * s2 + s) * a.out_slope * span
            + (-2.0 * s3 + 3.0 * s2) * b.value
            + (s3 - s2) * b.in_slope * span
    }

    /// Replaces the key at `index` with a new flat-tangent key, keeping the
    /// keys in time order. Does nothing if there is no such key.
    pub fn move_key(&mut self, index: usize, time: f32, value: f32) {
        if index >= self.keys.len() {
            return;
        }
        self.keys.remove(index);
        let at = self.keys.partition_point(|k| k.time < time);
        self.keys.insert(at, Key { time, value, in_slope: 0.0, out_slope: 0.0 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_segments_and_clamped_ends() {
        let c = Curve::linear(&[(0.0, 1.0), (2.0, 5.0)]);
        assert_eq!(c.evaluate(-1.0), 1.0);
        assert!((c.evaluate(0.5) - 2.0).abs() < 1e-5);
        assert_eq!(c.evaluate(9.0), 5.0);
    }

    #[test]
    fn flat_tangents_ease_in_and_out() {
        let c = Curve::flat(&[(0.0, 0.0), (1.0, 1.0)]);
        assert!((c.evaluate(0.5) - 0.5).abs() < 1e-5);
        assert!(c.evaluate(0.25) < 0.25);
    }

    #[test]
    fn wraps_take_a_time_back_into_the_keys() {
        let c = Curve::linear(&[(0.0, 0.0), (0.25, 0.15)]);
        let at = |t: f32, before: Wrap, after: Wrap| c.evaluate_wrapped(t, before, after);
        // Inside the keys every wrap is the curve itself.
        for wrap in [Wrap::PingPong, Wrap::Repeat, Wrap::Clamp] {
            assert!((at(0.1, wrap, wrap) - 0.06).abs() < 1e-6);
        }
        // Back and forth: at 0.30 as at 0.20, at 0.5 at the start, at 0.6 as at 0.1.
        assert!((at(0.30, Wrap::Clamp, Wrap::PingPong) - at(0.20, Wrap::Clamp, Wrap::Clamp)).abs() < 1e-6);
        assert!(at(0.5, Wrap::Clamp, Wrap::PingPong).abs() < 1e-6);
        assert!((at(0.6, Wrap::Clamp, Wrap::PingPong) - 0.06).abs() < 1e-6);
        // Over again: at 0.30 as at 0.05.
        assert!((at(0.30, Wrap::Clamp, Wrap::Repeat) - 0.03).abs() < 1e-6);
        // Held: the last key's value after, the first's before.
        assert!((at(0.30, Wrap::Clamp, Wrap::Clamp) - 0.15).abs() < 1e-6);
        assert!(at(-0.1, Wrap::Clamp, Wrap::Repeat).abs() < 1e-6);
        // Before the first key by its own wrap.
        assert!((at(-0.05, Wrap::Repeat, Wrap::Clamp) - 0.12).abs() < 1e-6);
        assert!((at(-0.05, Wrap::PingPong, Wrap::Clamp) - 0.03).abs() < 1e-6);
        assert_eq!((Wrap::of(0), Wrap::of(1), Wrap::of(2), Wrap::of(7)), (Wrap::PingPong, Wrap::Repeat, Wrap::Clamp, Wrap::Clamp));
    }

    #[test]
    fn move_key_keeps_time_order() {
        let mut c = Curve::flat(&[(0.0, 0.0), (1.0, 1.0)]);
        c.move_key(0, 2.0, 7.0);
        assert_eq!(c.keys.iter().map(|k| k.time).collect::<Vec<_>>(), [1.0, 2.0]);
        assert_eq!(c.evaluate(3.0), 7.0);
    }
}
