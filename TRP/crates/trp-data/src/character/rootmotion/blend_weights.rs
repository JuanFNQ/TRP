//! The weights of 1D, cartesian and directional blend trees.

/// Unity's freeform Cartesian blend. Each child's influence falls off
/// linearly toward every other child and is cut to zero beyond it; the
/// results are normalized.
pub fn freeform_cartesian(positions: &[[f32; 2]], at: [f32; 2]) -> Vec<f32> {
    let mut weights: Vec<f32> = positions
        .iter()
        .enumerate()
        .map(|(i, pi)| {
            let to_point = [at[0] - pi[0], at[1] - pi[1]];
            let mut weight = 1.0f32;
            for (j, pj) in positions.iter().enumerate() {
                if i == j {
                    continue;
                }
                let to_other = [pj[0] - pi[0], pj[1] - pi[1]];
                let len2 = to_other[0] * to_other[0] + to_other[1] * to_other[1];
                if len2 <= 0.0 {
                    continue;
                }
                let h = 1.0 - (to_point[0] * to_other[0] + to_point[1] * to_other[1]) / len2;
                if h <= 0.0 {
                    return 0.0;
                }
                weight = weight.min(h);
            }
            weight
        })
        .collect();
    let sum: f32 = weights.iter().sum();
    if sum > 0.0 {
        weights.iter_mut().for_each(|w| *w /= sum);
    }
    weights
}

/// The engine's simple directional blend. The children sit around the
/// origin, each in its own direction, with at most one at the origin
/// itself. The sample falls in the wedge between the two children nearest
/// in direction on either side, and its weights are its barycentric
/// coordinates in the triangle those two make with the origin; what falls
/// to the origin goes to the child there, or is shared among all.
pub fn simple_directional(positions: &[[f32; 2]], at: [f32; 2]) -> Vec<f32> {
    let n = positions.len();
    let mut weights = vec![0.0f32; n];
    if n < 2 {
        weights.iter_mut().for_each(|w| *w = 1.0);
        return weights;
    }
    let is_origin = |p: [f32; 2]| p[0] == 0.0 && p[1] == 0.0;
    let centre = positions.iter().position(|p| is_origin(*p));
    if is_origin(at) {
        match centre {
            Some(c) => weights[c] = 1.0,
            None => weights.iter_mut().for_each(|w| *w = 1.0 / n as f32),
        }
        return weights;
    }
    // The nearest child in direction on each side of the sample.
    let (mut left, mut right): (Option<(usize, f32)>, Option<(usize, f32)>) = (None, None);
    for (i, p) in positions.iter().enumerate() {
        if is_origin(*p) {
            continue;
        }
        let length = (p[0] * p[0] + p[1] * p[1]).sqrt();
        let unit = [p[0] / length, p[1] / length];
        let along = unit[0] * at[0] + unit[1] * at[1];
        let side = if unit[0] * at[1] - unit[1] * at[0] > 0.0 { &mut left } else { &mut right };
        if side.is_none_or(|(_, best)| along > best) {
            *side = Some((i, along));
        }
    }
    let mut to_centre = 1.0;
    if let (Some((a, _)), Some((b, _))) = (left, right) {
        let (pa, pb) = (positions[a], positions[b]);
        let det = pb[1] * pa[0] - pb[0] * pa[1];
        let mut wa = (pb[1] * at[0] - pb[0] * at[1]) / det;
        let mut wb = (pa[0] * at[1] - pa[1] * at[0]) / det;
        to_centre = 1.0 - wa - wb;
        // Outside the triangle the sample is brought back to its edge.
        if to_centre < 0.0 {
            to_centre = 0.0;
            let sum = wa + wb;
            wa /= sum;
            wb /= sum;
        } else if to_centre > 1.0 {
            to_centre = 1.0;
            wa = 0.0;
            wb = 0.0;
        }
        weights[a] = wa;
        weights[b] = wb;
    }
    match centre {
        Some(c) => weights[c] = to_centre,
        None => weights.iter_mut().for_each(|w| *w += to_centre / n as f32),
    }
    weights
}

/// How much a radian of direction counts against a unit of relative
/// distance in the freeform directional blend.
const DIRECTION_WEIGHT: f32 = 1.0;

/// The engine's freeform directional blend: like the Cartesian one, but
/// worked out in terms of direction and distance from the origin, so that
/// children in the same direction blend by distance and children at the
/// same distance by angle.
pub fn freeform_directional(positions: &[[f32; 2]], at: [f32; 2]) -> Vec<f32> {
    freeform_directional_weighted(positions, at, DIRECTION_WEIGHT)
}

/// The same with the weight of direction against distance given, for
/// checking the figure against the engine's own results.
pub fn freeform_directional_weighted(positions: &[[f32; 2]], at: [f32; 2], direction_weight: f32) -> Vec<f32> {
    let n = positions.len();
    let length = |p: [f32; 2]| (p[0] * p[0] + p[1] * p[1]).sqrt();
    let is_origin = |p: [f32; 2]| p[0] == 0.0 && p[1] == 0.0;
    // From `a` round to `b`, positive counter-clockwise, in radians times
    // the weight the engine gives direction against distance.
    let angle = |a: [f32; 2], b: [f32; 2]| {
        let cosine = ((a[0] * b[0] + a[1] * b[1]) / (length(a) * length(b))).clamp(-1.0, 1.0);
        let turn = cosine.acos() * direction_weight;
        if a[0] * b[1] - a[1] * b[0] < 0.0 { -turn } else { turn }
    };
    let distances: Vec<f32> = positions.iter().map(|p| length(*p)).collect();
    let sample_distance = length(at);
    // From each child to the sample: (angle, difference in distance).
    let to_sample: Vec<[f32; 2]> = positions
        .iter()
        .zip(&distances)
        .map(|(p, d)| {
            let turn = if is_origin(at) || is_origin(*p) { 0.0 } else { angle(*p, at) };
            [turn, sample_distance - d]
        })
        .collect();
    let mut weights: Vec<f32> = (0..n)
        .map(|i| {
            let mut weight = 1.0f32;
            for j in 0..n {
                if i == j {
                    continue;
                }
                // Distances count relative to the pair's average.
                let scale = 2.0 / (distances[i] + distances[j]);
                let mut between = [0.0, (distances[j] - distances[i]) * scale];
                if distances[i] != 0.0 && distances[j] != 0.0 {
                    between[0] = angle(positions[i], positions[j]);
                }
                // A child at the origin has no direction of its own.
                if is_origin(positions[i]) {
                    between[0] = to_sample[j][0];
                } else if is_origin(positions[j]) {
                    between[0] = to_sample[i][0];
                }
                let toward = [to_sample[i][0], to_sample[i][1] * scale];
                let h = 1.0
                    - (between[0] * toward[0] + between[1] * toward[1]) / (between[0] * between[0] + between[1] * between[1]);
                if !(h > 0.0) {
                    return 0.0;
                }
                weight = weight.min(h);
            }
            weight
        })
        .collect();
    let sum: f32 = weights.iter().sum();
    if sum > 0.0 {
        weights.iter_mut().for_each(|w| *w /= sum);
    }
    weights
}

/// Weight of child `index` in a one-dimensional blend: 1 at its own
/// threshold, falling linearly to 0 at its neighbours'.
pub fn weight_1d(thresholds: &[f32], index: usize, blend: f32) -> f32 {
    let Some(&own) = thresholds.get(index) else { return 0.0 };
    let (neighbour, at_end) = if blend >= own {
        (thresholds.get(index + 1).copied(), index + 1 == thresholds.len())
    } else {
        (index.checked_sub(1).map(|i| thresholds[i]), index == 0)
    };
    let Some(neighbour) = neighbour.filter(|_| !at_end) else { return 1.0 };
    let outside = if blend >= own { neighbour < blend } else { neighbour > blend };
    if outside {
        0.0
    } else if (own - neighbour).abs() > f32::EPSILON {
        (blend - neighbour) / (own - neighbour)
    } else {
        1.0
    }
}
