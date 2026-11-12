//! Particle curves and gradients: constant, curve, random between two.

use super::*;

/// The effects' bundle, where the shared systems are (`Effects`).
pub const EFFECTS: &str = "assets/systems/effects/particlesystems/effects.bundle";

/// A value that can vary (`MinMaxCurve`): a constant, a curve over the
/// share of life times `scalar`, between two curves, or between two
/// constants; where it is between two, each particle has its own draw.
#[derive(Debug, Clone, PartialEq)]
pub struct MinMaxCurve {
    /// `minMaxState`: 0 a constant, 1 a curve, 2 two curves, 3 two
    /// constants.
    pub mode: u8,
    pub scalar: f32,
    pub min_scalar: f32,
    pub max_curve: Curve,
    pub min_curve: Curve,
}

impl MinMaxCurve {
    pub fn constant(value: f32) -> Self {
        Self { mode: 0, scalar: value, min_scalar: 0.0, max_curve: Curve::default(), min_curve: Curve::default() }
    }

    pub fn read(v: &Value) -> Result<Self> {
        Ok(Self {
            mode: v.req("minMaxState")?.as_i64().context("minMaxState")? as u8,
            scalar: float(v, "scalar")?,
            min_scalar: float(v, "minScalar")?,
            max_curve: crate::character::player::curve(v.req("maxCurve")?)?,
            min_curve: crate::character::player::curve(v.req("minCurve")?)?,
        })
    }

    /// At a share of life `t`, with a particle's own draw from 0 to 1.
    pub fn evaluate(&self, t: f32, draw: f32) -> f32 {
        match self.mode {
            1 => self.max_curve.evaluate(t) * self.scalar,
            2 => lerp(self.min_curve.evaluate(t), self.max_curve.evaluate(t), draw) * self.scalar,
            3 => lerp(self.min_scalar, self.scalar, draw),
            _ => self.scalar,
        }
    }
}

/// A gradient (`Gradient`): its colour keys and its alpha keys apart, each
/// at its time, blended between or stepped.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Gradient {
    pub colors: Vec<(f32, [f32; 3])>,
    pub alphas: Vec<(f32, f32)>,
    pub stepped: bool,
}

impl Gradient {
    pub fn read(v: &Value) -> Result<Self> {
        let count = |name: &str| v.get(name).and_then(Value::as_i64).unwrap_or(0).clamp(0, 8) as usize;
        let key = |k: usize| -> Result<[f32; 4]> {
            let c = v.req(&format!("key{k}"))?;
            Ok([float(c, "r")?, float(c, "g")?, float(c, "b")?, float(c, "a")?])
        };
        let time = |name: &str| -> Result<f32> { Ok(v.req(name)?.as_i64().context("a key's time")? as f32 / 65535.0) };
        let colors = (0..count("m_NumColorKeys")).map(|k| Ok((time(&format!("ctime{k}"))?, { let c = key(k)?; [c[0], c[1], c[2]] }))).collect::<Result<_>>()?;
        let alphas = (0..count("m_NumAlphaKeys")).map(|k| Ok((time(&format!("atime{k}"))?, key(k)?[3]))).collect::<Result<_>>()?;
        Ok(Self { colors, alphas, stepped: v.get("m_Mode").and_then(Value::as_i64) == Some(1) })
    }

    /// Its colour and alpha at `t`: before the first key the first's,
    /// after the last the last's.
    pub fn evaluate(&self, t: f32) -> [f32; 4] {
        let rgb = keyed(&self.colors, t, self.stepped, [1.0; 3], |a, b, s| std::array::from_fn(|i| lerp(a[i], b[i], s)));
        let a = keyed(&self.alphas, t, self.stepped, 1.0, lerp);
        [rgb[0], rgb[1], rgb[2], a]
    }
}

/// A key list's value at `t`.
fn keyed<T: Copy>(keys: &[(f32, T)], t: f32, stepped: bool, none: T, blend: impl Fn(T, T, f32) -> T) -> T {
    let Some(first) = keys.first() else { return none };
    if t <= first.0 {
        return first.1;
    }
    for pair in keys.windows(2) {
        let ((t0, a), (t1, b)) = (pair[0], pair[1]);
        if t <= t1 {
            if stepped {
                return b;
            }
            let span = t1 - t0;
            return if span > f32::EPSILON { blend(a, b, (t - t0) / span) } else { b };
        }
    }
    keys[keys.len() - 1].1
}

/// A colour that can vary (`MinMaxGradient`): a colour, a gradient, between
/// two colours, between two gradients, or a random colour of a gradient.
#[derive(Debug, Clone, PartialEq)]
pub struct MinMaxGradient {
    /// `minMaxState`: 0 a colour, 1 a gradient, 2 two colours, 3 two
    /// gradients, 4 a random colour of the gradient.
    pub mode: u8,
    pub min_color: [f32; 4],
    pub max_color: [f32; 4],
    pub min_gradient: Gradient,
    pub max_gradient: Gradient,
}

impl MinMaxGradient {
    /// One colour throughout (mode 0).
    pub fn constant(color: [f32; 4]) -> Self {
        Self { mode: 0, min_color: color, max_color: color, min_gradient: Gradient::default(), max_gradient: Gradient::default() }
    }

    pub fn read(v: &Value) -> Result<Self> {
        let color = |name: &str| -> Result<[f32; 4]> {
            let c = v.req(name)?;
            Ok([float(c, "r")?, float(c, "g")?, float(c, "b")?, float(c, "a")?])
        };
        Ok(Self {
            mode: v.req("minMaxState")?.as_i64().context("minMaxState")? as u8,
            min_color: color("minColor")?,
            max_color: color("maxColor")?,
            min_gradient: Gradient::read(v.req("minGradient")?)?,
            max_gradient: Gradient::read(v.req("maxGradient")?)?,
        })
    }

    /// At a share of life `t`, with a particle's own draw.
    pub fn evaluate(&self, t: f32, draw: f32) -> [f32; 4] {
        match self.mode {
            1 => self.max_gradient.evaluate(t),
            2 => std::array::from_fn(|i| lerp(self.min_color[i], self.max_color[i], draw)),
            3 => {
                let (a, b) = (self.min_gradient.evaluate(t), self.max_gradient.evaluate(t));
                std::array::from_fn(|i| lerp(a[i], b[i], draw))
            }
            4 => self.max_gradient.evaluate(draw),
            _ => self.max_color,
        }
    }
}
