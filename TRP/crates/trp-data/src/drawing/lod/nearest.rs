//! A grid for the nearest triangle and the measured deviation of a shape.

use super::*;

/// A surface's triangles sorted into a grid of equal cubes, for the nearest
/// point to a sample.
pub(super) struct Grid {
    pub(super) points: Vec<[f32; 3]>,
    pub(super) triangles: Vec<[u32; 3]>,
    low: [f32; 3],
    cell: f32,
    dims: [i32; 3],
    /// Each cell's triangles: `items[starts[c]..starts[c + 1]]`.
    starts: Vec<u32>,
    items: Vec<u32>,
}

impl Grid {
    pub(super) fn of(shape: &Shape) -> Self {
        let (mut low, mut high) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
        for t in &shape.triangles {
            for &i in t {
                let p = shape.points[i as usize];
                for k in 0..3 {
                    low[k] = low[k].min(p[k]);
                    high[k] = high[k].max(p[k]);
                }
            }
        }
        let extent = (0..3).map(|k| high[k] - low[k]).fold(0.0f32, f32::max).max(1e-6);
        // About one and a half cells a triangle's cube root along the
        // longest side: a few triangles a cell on a surface.
        let across = ((shape.triangles.len() as f32).cbrt() * 1.5).clamp(1.0, 64.0);
        let cell = extent / across;
        let dims: [i32; 3] = std::array::from_fn(|k| (((high[k] - low[k]) / cell).floor() as i32 + 1).max(1));
        let cells = (dims[0] * dims[1] * dims[2]) as usize;
        let index = |c: [i32; 3]| ((c[2] * dims[1] + c[1]) * dims[0] + c[0]) as usize;
        let cell_of = |p: [f32; 3]| -> [i32; 3] { std::array::from_fn(|k| (((p[k] - low[k]) / cell).floor() as i32).clamp(0, dims[k] - 1)) };
        let span = |t: &[u32; 3]| -> ([i32; 3], [i32; 3]) {
            let ps = t.map(|i| shape.points[i as usize]);
            let lo: [f32; 3] = std::array::from_fn(|k| ps[0][k].min(ps[1][k]).min(ps[2][k]));
            let hi: [f32; 3] = std::array::from_fn(|k| ps[0][k].max(ps[1][k]).max(ps[2][k]));
            (cell_of(lo), cell_of(hi))
        };
        let mut counts = vec![0u32; cells + 1];
        for t in &shape.triangles {
            let (a, b) = span(t);
            for z in a[2]..=b[2] {
                for y in a[1]..=b[1] {
                    for x in a[0]..=b[0] {
                        counts[index([x, y, z]) + 1] += 1;
                    }
                }
            }
        }
        for c in 0..cells {
            counts[c + 1] += counts[c];
        }
        let mut fill = counts.clone();
        let mut items = vec![0u32; counts[cells] as usize];
        for (n, t) in shape.triangles.iter().enumerate() {
            let (a, b) = span(t);
            for z in a[2]..=b[2] {
                for y in a[1]..=b[1] {
                    for x in a[0]..=b[0] {
                        let c = index([x, y, z]);
                        items[fill[c] as usize] = n as u32;
                        fill[c] += 1;
                    }
                }
            }
        }
        Self { points: shape.points.clone(), triangles: shape.triangles.clone(), low, cell, dims, starts: counts, items }
    }

    /// The distance from `p` to the nearest point of the surface, searched
    /// cube by cube outward; given up as soon as a point within `enough` is
    /// found (it cannot be the largest).
    pub(super) fn nearest(&self, p: [f32; 3], enough: f32, stamps: &mut [u32], query: u32) -> f32 {
        let at: [i32; 3] = std::array::from_fn(|k| (((p[k] - self.low[k]) / self.cell).floor() as i32).clamp(0, self.dims[k] - 1));
        let reach = self.dims.iter().copied().max().unwrap_or(1);
        let mut best = f32::INFINITY;
        for ring in 0..=reach {
            let lo: [i32; 3] = std::array::from_fn(|k| (at[k] - ring).max(0));
            let hi: [i32; 3] = std::array::from_fn(|k| (at[k] + ring).min(self.dims[k] - 1));
            for z in lo[2]..=hi[2] {
                for y in lo[1]..=hi[1] {
                    for x in lo[0]..=hi[0] {
                        // The ring's shell only.
                        if (x - at[0]).abs() < ring && (y - at[1]).abs() < ring && (z - at[2]).abs() < ring {
                            continue;
                        }
                        let c = ((z * self.dims[1] + y) * self.dims[0] + x) as usize;
                        for &n in &self.items[self.starts[c] as usize..self.starts[c + 1] as usize] {
                            if stamps[n as usize] == query {
                                continue;
                            }
                            stamps[n as usize] = query;
                            let t = self.triangles[n as usize];
                            let d = point_triangle(p, self.points[t[0] as usize], self.points[t[1] as usize], self.points[t[2] as usize]);
                            best = best.min(d);
                        }
                    }
                }
            }
            // Anything not yet tried lies a ring's width or more away.
            if best <= enough || best <= ring as f32 * self.cell {
                break;
            }
        }
        best
    }
}

/// The distance from a point to a triangle (its nearest point on it:
/// Ericson's regions of the corners, edges and face).
fn point_triangle(p: [f32; 3], a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f32 {
    let sub = |u: [f32; 3], v: [f32; 3]| [u[0] - v[0], u[1] - v[1], u[2] - v[2]];
    let dot = |u: [f32; 3], v: [f32; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
    let length = |u: [f32; 3]| dot(u, u).sqrt();
    let along = |o: [f32; 3], d: [f32; 3], t: f32| [o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t];
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return length(ap);
    }
    let bp = sub(p, b);
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return length(bp);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return length(sub(p, along(a, ab, v)));
    }
    let cp = sub(p, c);
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return length(cp);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return length(sub(p, along(a, ac, w)));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return length(sub(p, along(b, sub(c, b), w)));
    }
    let denom = va + vb + vc;
    if denom.abs() < 1e-30 {
        // A triangle with no area: its corners' nearest.
        return length(ap).min(length(bp)).min(length(cp));
    }
    let (v, w) = (vb / denom, vc / denom);
    let q = [a[0] + ab[0] * v + ac[0] * w, a[1] + ab[1] * v + ac[1] * w, a[2] + ab[2] * v + ac[2] * w];
    length(sub(p, q))
}

/// The measures made this run, by the group's object (its file's name and
/// path id): a group read again (another dummy's body, a weapon made anew)
/// is not measured again.
fn kept() -> &'static Mutex<HashMap<(String, i64), Vec<f32>>> {
    static KEPT: OnceLock<Mutex<HashMap<(String, i64), Vec<f32>>>> = OnceLock::new();
    KEPT.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The group with its measure (`LodGroup::deviation`): the one kept this
/// run where it was measured before (a group of a bundle, by its object
/// in `assets`), else each level's surface had from `shapes` (the fullest
/// first, one a level) and measured. A level whose surface could not be
/// had leaves the group unmeasured.
pub fn measured(assets: Option<&Assets>, group: &LodGroup, shapes: impl FnOnce() -> Option<Vec<Shape>>) -> LodGroup {
    let key = group.id.zip(assets).map(|(id, assets)| (assets.file(id.file).name.clone(), id.path_id));
    if let Some(key) = &key {
        if let Some(found) = kept().lock().ok().and_then(|kept| kept.get(key).cloned()) {
            return LodGroup { deviation: found, ..group.clone() };
        }
    }
    let deviation = shapes().filter(|levels| levels.len() == group.levels.len()).map(|levels| deviations(&levels)).unwrap_or_default();
    if let (Some(key), false) = (key, deviation.is_empty()) {
        if let Ok(mut kept) = kept().lock() {
            kept.insert(key, deviation.clone());
        }
    }
    LodGroup { deviation, ..group.clone() }
}
