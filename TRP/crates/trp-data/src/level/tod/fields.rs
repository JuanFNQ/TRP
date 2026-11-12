//! A component's typed fields and its weighted curves.

use super::*;

/// A field's value as read.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    Number(f32),
    Int(i32),
    Flag(bool),
    Color([f32; 4]),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Gradient(Gradient),
    Curve(WeightedCurve),
    Pointer((i32, i64)),
}

/// A key of an animation curve with its weights (`Keyframe`: its weighted
/// mode, 0 none, 1 in, 2 out, 3 both, and its two weights).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeightedKey {
    pub time: f32,
    pub value: f32,
    pub in_slope: f32,
    pub out_slope: f32,
    pub mode: i32,
    pub in_weight: f32,
    pub out_weight: f32,
}

/// An animation curve whose keys may be weighted.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WeightedCurve {
    pub keys: Vec<WeightedKey>,
}

impl WeightedCurve {
    /// As a script keeps it (`AnimationCurve`): its keys, then its wraps
    /// and rotation order.
    pub(crate) fn read(r: &mut Reader) -> Result<Self> {
        let count = r.i32()?.max(0);
        let keys = (0..count)
            .map(|_| Ok(WeightedKey { time: r.f32()?, value: r.f32()?, in_slope: r.f32()?, out_slope: r.f32()?, mode: r.i32()?, in_weight: r.f32()?, out_weight: r.f32()? }))
            .collect::<Result<_>>()?;
        r.skip(12)?;
        Ok(Self { keys })
    }

    /// Its value at `t`; outside its keys the end values hold (`ClampForever`,
    /// as every curve here wraps). A segment is a cubic Bézier in time and
    /// value: from a key to the next, the first handle a weight of the span
    /// along the first key's out slope, the second the next key's weight back
    /// along its in slope, a weight a third where the key does not weigh
    /// that side (which is the plain Hermite segment); an infinite slope
    /// holds the first key's value.
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
        if span <= 0.0 || !a.out_slope.is_finite() || !b.in_slope.is_finite() {
            return a.value;
        }
        let third = 1.0 / 3.0;
        let wa = if a.mode & 2 != 0 { a.out_weight } else { third };
        let wb = if b.mode & 1 != 0 { b.in_weight } else { third };
        let s = (t - a.time) / span;
        if wa == third && wb == third {
            let (s2, s3) = (s * s, s * s * s);
            return (2.0 * s3 - 3.0 * s2 + 1.0) * a.value + (s3 - 2.0 * s2 + s) * a.out_slope * span + (-2.0 * s3 + 3.0 * s2) * b.value + (s3 - s2) * b.in_slope * span;
        }
        // The Bézier's own parameter where its time is `s` (its time's
        // handles at `wa` and `1 - wb` of the span), by halving.
        let x = |u: f32| {
            let v = 1.0 - u;
            3.0 * v * v * u * wa + 3.0 * v * u * u * (1.0 - wb) + u * u * u
        };
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..40 {
            let mid = 0.5 * (lo + hi);
            if x(mid) < s {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let u = 0.5 * (lo + hi);
        let v = 1.0 - u;
        let p1 = a.value + wa * span * a.out_slope;
        let p2 = b.value - wb * span * b.in_slope;
        v * v * v * a.value + 3.0 * v * v * u * p1 + 3.0 * v * u * u * p2 + u * u * u * b.value
    }
}

/// A script's fields by their names, a nested class's as `Class.Field`.
#[derive(Debug, Clone, Default)]
pub struct Fields(pub Vec<(String, FieldValue)>);

impl Fields {
    /// Reads fields of that layout from the start of a script's bytes.
    pub fn read(bytes: &[u8], layout: &[(&'static str, Kind)]) -> Result<Self> {
        let mut fields = Fields::default();
        let mut r = Reader::new(bytes);
        fields.read_into(&mut r, layout, "")?;
        Ok(fields)
    }

    fn read_into(&mut self, r: &mut Reader, layout: &[(&'static str, Kind)], prefix: &str) -> Result<()> {
        for &(name, kind) in layout {
            let full = format!("{prefix}{name}");
            let value = match kind {
                F => FieldValue::Number(r.f32()?),
                I => FieldValue::Int(r.i32()?),
                Flag => {
                    let on = r.u8()? != 0;
                    r.align(4);
                    FieldValue::Flag(on)
                }
                Color => FieldValue::Color(color(r)?),
                Vec2 => FieldValue::Vec2([r.f32()?, r.f32()?]),
                Vec3 => FieldValue::Vec3([r.f32()?, r.f32()?, r.f32()?]),
                Kind::Gradient => FieldValue::Gradient(gradient(r).with_context(|| full.clone())?),
                Kind::Curve => FieldValue::Curve(WeightedCurve::read(r).with_context(|| full.clone())?),
                Pointer => FieldValue::Pointer(pointer(r)?),
                Class(inner) => {
                    self.read_into(r, inner, &format!("{full}."))?;
                    continue;
                }
            };
            self.0.push((full, value));
        }
        Ok(())
    }

    pub(super) fn get(&self, name: &str) -> Result<&FieldValue> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, v)| v).with_context(|| format!("no field {name}"))
    }

    pub fn f32(&self, name: &str) -> Result<f32> {
        match self.get(name)? {
            FieldValue::Number(v) => Ok(*v),
            other => bail!("{name} is {other:?}, not a number"),
        }
    }

    pub fn int(&self, name: &str) -> Result<i32> {
        match self.get(name)? {
            FieldValue::Int(v) => Ok(*v),
            other => bail!("{name} is {other:?}, not a whole number"),
        }
    }

    pub fn color(&self, name: &str) -> Result<[f32; 4]> {
        match self.get(name)? {
            FieldValue::Color(v) => Ok(*v),
            other => bail!("{name} is {other:?}, not a colour"),
        }
    }

    pub fn vec2(&self, name: &str) -> Result<[f32; 2]> {
        match self.get(name)? {
            FieldValue::Vec2(v) => Ok(*v),
            other => bail!("{name} is {other:?}, not a pair"),
        }
    }

    pub fn gradient(&self, name: &str) -> Result<&Gradient> {
        match self.get(name)? {
            FieldValue::Gradient(v) => Ok(v),
            other => bail!("{name} is {other:?}, not a gradient"),
        }
    }

    pub fn curve(&self, name: &str) -> Result<&WeightedCurve> {
        match self.get(name)? {
            FieldValue::Curve(v) => Ok(v),
            other => bail!("{name} is {other:?}, not a curve"),
        }
    }

    pub fn flag(&self, name: &str) -> Result<bool> {
        match self.get(name)? {
            FieldValue::Flag(v) => Ok(*v),
            other => bail!("{name} is {other:?}, not a flag"),
        }
    }

    /// A pointer: its file (0 the file itself) and its object.
    pub fn pointer(&self, name: &str) -> Result<(i32, i64)> {
        match self.get(name)? {
            FieldValue::Pointer(v) => Ok(*v),
            other => bail!("{name} is {other:?}, not a pointer"),
        }
    }
}
