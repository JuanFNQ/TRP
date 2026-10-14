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

impl Curve {
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
    fn move_key_keeps_time_order() {
        let mut c = Curve::flat(&[(0.0, 0.0), (1.0, 1.0)]);
        c.move_key(0, 2.0, 7.0);
        assert_eq!(c.keys.iter().map(|k| k.time).collect::<Vec<_>>(), [1.0, 2.0]);
        assert_eq!(c.evaluate(3.0), 7.0);
    }
}
