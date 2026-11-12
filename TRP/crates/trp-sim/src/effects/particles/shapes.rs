//! Emission shapes, texture sheets and the small vector maths.

use super::*;

/// Where on its shape a particle is born and which way it goes, in the
/// system's own axes (`ShapeModule`, as the documentation describes each
/// kind). `PROVISIONAL`: a cone's way leans out from its axis by its angle
/// in proportion to how far out on the base it is born.
pub(super) fn on_shape(shape: &Shape, rng: &mut Rng) -> ([f32; 3], [f32; 3]) {
    let arc = shape.arc.to_radians();
    let around = rng.unit() * arc;
    let thickness = shape.radius_thickness.clamp(0.0, 1.0);
    let (at, way) = match shape.kind {
        // A cone's base: a point on the disc (its area evenly), its way
        // leaning out with it.
        4 => {
            let inner = 1.0 - thickness;
            let reach = (inner * inner + (1.0 - inner * inner) * rng.unit()).sqrt();
            let lean = shape.angle.to_radians() * reach;
            let at = [shape.radius * reach * around.cos(), shape.radius * reach * around.sin(), 0.0];
            (at, [lean.sin() * around.cos(), lean.sin() * around.sin(), lean.cos()])
        }
        // A sphere or hemisphere: a point in its ball (its volume evenly,
        // within its thickness), its way out from the middle.
        0 | 2 => {
            let way = loop {
                let v = [rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)];
                let l = length(v);
                if l > 1e-3 && l <= 1.0 {
                    break scale(v, 1.0 / l);
                }
            };
            let way = if shape.kind == 2 && way[2] < 0.0 { [way[0], way[1], -way[2]] } else { way };
            let inner = 1.0 - thickness;
            let reach = (inner.powi(3) + (1.0 - inner.powi(3)) * rng.unit()).cbrt();
            (scale(way, shape.radius * reach), way)
        }
        // Anything else (not built): from the middle, along its axis.
        _ => ([0.0; 3], [0.0, 0.0, 1.0]),
    };
    let mut way = way;
    if shape.random_direction > 0.0 {
        let r = [rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)];
        way = normalized(add(scale(way, 1.0 - shape.random_direction), scale(normalized(r), shape.random_direction)));
    }
    if shape.spherical_direction > 0.0 && length(at) > 1e-6 {
        way = normalized(add(scale(way, 1.0 - shape.spherical_direction), scale(normalized(at), shape.spherical_direction)));
    }
    // The shape's own place, turn (Unity's order: z, then x, then y) and
    // size.
    let turn = euler(shape.rotation);
    let scaled = std::array::from_fn(|k| at[k] * shape.scale[k]);
    let mut at = add(shape.position, rotate(turn, scaled));
    if shape.random_position > 0.0 {
        at = add(at, [rng.range(-1.0, 1.0) * shape.random_position, rng.range(-1.0, 1.0) * shape.random_position, rng.range(-1.0, 1.0) * shape.random_position]);
    }
    (at, rotate(turn, way))
}

/// A particle's picture in its system's sheet (`UVModule`, a grid): the
/// frame its curve gives at its share of life (a share of the sheet, or of
/// a row), the sheet's cell; frame 0 at the top left.
pub(super) fn sheet_of(data: &ParticleSystemData, p: &Particle, t: f32) -> [f32; 4] {
    let Some(sheet) = data.texture_sheet.as_ref().filter(|s| s.mode == 0) else { return [0.0, 0.0, 1.0, 1.0] };
    let [across, down] = sheet.tiles.map(|n| n.max(1));
    let frames = if sheet.single_row { across } else { across * down };
    let mut share = sheet.frame_over_time.evaluate(t, draw(p.seed, SHEET));
    if sheet.cycles > 1.0 {
        share = (share * sheet.cycles).fract();
    }
    let start = sheet.start_frame.evaluate(t, draw(p.seed, START_FRAME));
    let frame = ((share * frames as f32 + start).floor().max(0.0) as u32) % frames;
    let (column, row) = if sheet.single_row {
        let row = if sheet.random_row { ((draw(p.seed, ROW) * down as f32) as u32).min(down - 1) } else { sheet.row_index.min(down - 1) };
        (frame, row)
    } else {
        (frame % across, frame / across)
    };
    let (w, h) = (1.0 / across as f32, 1.0 / down as f32);
    // The picture's rows bottom first, the sheet's top first.
    let v0 = 1.0 - (row + 1) as f32 * h;
    [column as f32 * w, v0, (column + 1) as f32 * w, v0 + h]
}

/// A particle's size at its share of life.
pub(super) fn size_of(data: &ParticleSystemData, p: &Particle, t: f32) -> f32 {
    p.size * data.size_over_lifetime.as_ref().map_or(1.0, |c| c.evaluate(t, engine_draw(p.seed, SIZE_ID, 0)))
}

/// Unity's `Quaternion.Euler`: about z, then x, then y, degrees.
fn euler(degrees: [f32; 3]) -> [f32; 4] {
    let half = degrees.map(|d| d.to_radians() * 0.5);
    let about = |axis: usize| {
        let mut q = [0.0, 0.0, 0.0, half[axis].cos()];
        q[axis] = half[axis].sin();
        q
    };
    quat_mul(about(1), quat_mul(about(0), about(2)))
}

fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

pub(super) fn conjugate(q: [f32; 4]) -> [f32; 4] {
    [-q[0], -q[1], -q[2], q[3]]
}

pub(super) fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let u = [q[0], q[1], q[2]];
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let t = cross(u, v).map(|c| 2.0 * c);
    let c = cross(u, t);
    [v[0] + q[3] * t[0] + c[0], v[1] + q[3] * t[1] + c[1], v[2] + q[3] * t[2] + c[2]]
}

pub(super) fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(super) fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(super) fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(super) fn scale(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn normalized(a: [f32; 3]) -> [f32; 3] {
    let l = length(a);
    if l > 1e-9 { scale(a, 1.0 / l) } else { [0.0, 0.0, 1.0] }
}

pub(super) fn length(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}
