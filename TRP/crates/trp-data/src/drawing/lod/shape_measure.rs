//! TRP's measure of a level of detail: shapes, closing them and deviations.

use super::*;

/// Whether rigid models' groups are measured as their trees are read
/// (`extract::read_hierarchy`), set once by the app as the run begins: with
/// a picker of levels on. The skins' readers of every level
/// (`arms::load_levels`, `dress::load_levels`) measure always.
static MEASURING: AtomicBool = AtomicBool::new(false);

pub fn set_measuring(on: bool) {
    MEASURING.store(on, Ordering::Relaxed);
}

pub fn measuring() -> bool {
    MEASURING.load(Ordering::Relaxed)
}

/// A level's surface as measured: its triangles' corners in one frame,
/// metres.
#[derive(Debug, Clone, Default)]
pub struct Shape {
    pub points: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    /// How many of the last triangles are caps (`closed`): surface the other
    /// level's samples may land on, not sampled themselves.
    pub caps: usize,
}

impl Shape {
    /// Adds a mesh's triangles (`indices`, three a triangle, into
    /// `points`), its points placed by `place` (before any caps).
    pub fn add(&mut self, points: &[[f32; 3]], indices: &[u32], place: &dyn Fn([f32; 3]) -> [f32; 3]) {
        let base = self.points.len() as u32;
        self.points.extend(points.iter().map(|&p| place(p)));
        let count = points.len() as u32;
        for t in indices.chunks_exact(3) {
            if t.iter().all(|&i| i < count) {
                self.triangles.push([base + t[0], base + t[1], base + t[2]]);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }

    /// The triangles sampled: all but the caps.
    pub(super) fn sampled(&self) -> &[[u32; 3]] {
        &self.triangles[..self.triangles.len() - self.caps.min(self.triangles.len())]
    }

    /// The surface with its openings closed. An opening's rim is a loop of
    /// the sides only one triangle uses (corners welded where they
    /// coincide, to a tenth of a millimetre), walked as the surface's own
    /// triangles bound it: from an open side, in its triangle's
    /// winding, to the next by turning about its end through the fan of
    /// triangles that side's triangle is in, until the first side comes
    /// round again. The rims are so the surface's as though each corner
    /// where separate fans meet were split, one corner a fan: two rims that
    /// touch at a corner (the bear body's coarser right wrist, two rims of
    /// about 3 cm about one middle) are two loops, and the same loops
    /// whatever order the sides are met in; the loops are taken by their
    /// sides in order, each from its least.
    /// Where the triangles do not say (their windings disagree, or a side is
    /// shared by more than two), the walk goes on by the open side there
    /// first in the corners' order. A loop that lies near a plane (no point
    /// of it off its plane by more than a quarter of its mean radius:
    /// `ESTIMATE`, the app's choice, a waist's or a neck's ring and not a
    /// strap's edge) is closed by a fan to its middle, a cap (`caps`). A
    /// part's opening is where it joins another (a body's waist, neck and
    /// wrists), what lies across it hidden by that other part; one level
    /// closes it with a cap, another leaves it open, and neither is seen. So
    /// a cap is surface the other level's samples may land on, but its own
    /// points are not measured (`deviations`). A long open edge that does
    /// not lie in a plane (a strap's) is left as it is.
    pub fn closed(&self) -> Shape {
        // Corners welded at a tenth of a millimetre.
        let mut canon: HashMap<[i64; 3], u32> = HashMap::new();
        let welded: Vec<u32> = self
            .points
            .iter()
            .enumerate()
            .map(|(n, p)| *canon.entry(p.map(|v| (v * 1e4).round() as i64)).or_insert(n as u32))
            .collect();
        let key = |a: u32, b: u32| (a.min(b), a.max(b));
        // Each side's count of triangles (either way), and each side as a
        // triangle winds it to that triangle's third corner.
        let mut uses: HashMap<(u32, u32), u32> = HashMap::new();
        let mut third: HashMap<(u32, u32), u32> = HashMap::new();
        for t in &self.triangles {
            let w = t.map(|i| welded[i as usize]);
            if w[0] == w[1] || w[1] == w[2] || w[0] == w[2] {
                continue;
            }
            for k in 0..3 {
                let (a, b, c) = (w[k], w[(k + 1) % 3], w[(k + 2) % 3]);
                *uses.entry(key(a, b)).or_insert(0) += 1;
                third.entry((a, b)).or_insert(c);
            }
        }
        let count = |a: u32, b: u32| uses.get(&key(a, b)).copied().unwrap_or(0);
        // The open sides as their triangles wind them, in order, and each
        // corner's open neighbours in order (where the triangles do not say).
        let mut sides: Vec<(u32, u32)> = third.keys().copied().filter(|&(a, b)| count(a, b) == 1).collect();
        sides.sort_unstable();
        let mut beside: HashMap<u32, Vec<u32>> = HashMap::new();
        for &(a, b) in &sides {
            beside.entry(a).or_default().push(b);
            beside.entry(b).or_default().push(a);
        }
        for ways in beside.values_mut() {
            ways.sort_unstable();
        }
        // The open side after `(a, b)`: turning about `b` from `(a, b)`'s
        // triangle, through each triangle across a shared side, to the first
        // open side leaving `b`; none where the triangles do not say.
        let after = |a: u32, b: u32| -> Option<(u32, u32)> {
            let mut from = a;
            for _ in 0..256 {
                let c = *third.get(&(from, b))?;
                if count(b, c) == 1 {
                    return Some((b, c));
                }
                if count(b, c) != 2 || !third.contains_key(&(c, b)) {
                    return None;
                }
                from = c;
            }
            None
        };
        let mut out = self.clone();
        let mut walked: std::collections::HashSet<(u32, u32)> = std::collections::HashSet::new();
        for &(start, first) in &sides {
            if walked.contains(&key(start, first)) {
                continue;
            }
            walked.insert(key(start, first));
            // Walk the loop until its first side comes round again.
            let mut chain = vec![start];
            let (mut from, mut at) = (start, first);
            let mut closed = false;
            loop {
                let next = after(from, at);
                if at == start && next.is_none_or(|(_, to)| to == first || walked.contains(&key(at, to))) {
                    closed = true;
                    break;
                }
                if chain.len() >= 4096 {
                    break;
                }
                chain.push(at);
                let on = match next {
                    Some((_, to)) if !walked.contains(&key(at, to)) => Some(to),
                    _ => beside.get(&at).and_then(|ways| ways.iter().copied().find(|&to| !walked.contains(&key(at, to)))),
                };
                let Some(on) = on else { break };
                walked.insert(key(at, on));
                (from, at) = (at, on);
            }
            if !closed || chain.len() < 3 {
                continue;
            }
            let points: Vec<[f32; 3]> = chain.iter().map(|&i| self.points[i as usize]).collect();
            let middle: [f32; 3] = std::array::from_fn(|k| points.iter().map(|p| p[k]).sum::<f32>() / points.len() as f32);
            // Its plane by Newell's sum, and how far it strays from it.
            let mut normal = [0.0f32; 3];
            for (n, p) in points.iter().enumerate() {
                let q = points[(n + 1) % points.len()];
                normal[0] += (p[1] - q[1]) * (p[2] + q[2]);
                normal[1] += (p[2] - q[2]) * (p[0] + q[0]);
                normal[2] += (p[0] - q[0]) * (p[1] + q[1]);
            }
            let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
            if length < 1e-12 {
                continue;
            }
            let normal = normal.map(|v| v / length);
            let off = |p: &[f32; 3]| (0..3).map(|k| (p[k] - middle[k]) * normal[k]).sum::<f32>().abs();
            let radius = points.iter().map(|p| (0..3).map(|k| (p[k] - middle[k]).powi(2)).sum::<f32>().sqrt()).sum::<f32>() / points.len() as f32;
            if points.iter().map(off).fold(0.0f32, f32::max) > 0.25 * radius {
                continue;
            }
            let centre = out.points.len() as u32;
            out.points.push(middle);
            for n in 0..chain.len() {
                out.triangles.push([centre, chain[n], chain[(n + 1) % chain.len()]]);
            }
            out.caps += chain.len();
        }
        out
    }
}

/// How far two surfaces lie apart at most, both ways (their Hausdorff
/// distance, as sampled): the largest distance from a corner or a
/// triangle's middle of either to the nearest point of the other's
/// surface. Infinite where only one has triangles; 0 where neither has.
pub fn deviation(a: &Shape, b: &Shape) -> f32 {
    match (a.is_empty(), b.is_empty()) {
        (true, true) => 0.0,
        (true, false) | (false, true) => f32::INFINITY,
        (false, false) => one_way(a, &Grid::of(b), 0.0).max(one_way(b, &Grid::of(a), 0.0)),
    }
}

/// Each level's deviation from the first (the fullest), each level's
/// openings closed first (`Shape::closed`: its caps landed on, not
/// sampled), the first's 0, each never less than a finer level's (a coarser
/// level drawn where a finer one could not be is no gain).
pub fn deviations(levels: &[Shape]) -> Vec<f32> {
    let levels: Vec<Shape> = levels.iter().map(Shape::closed).collect();
    let Some(fullest) = levels.first() else { return Vec::new() };
    let near = Grid::of(fullest);
    let mut out = vec![0.0];
    for level in &levels[1..] {
        let far = match (fullest.is_empty(), level.is_empty()) {
            (true, true) => 0.0,
            (true, false) | (false, true) => f32::INFINITY,
            (false, false) => one_way(level, &near, 0.0).max(one_way(fullest, &Grid::of(level), 0.0)),
        };
        let last = *out.last().unwrap_or(&0.0);
        out.push(far.max(last));
    }
    out
}

/// Where two surfaces lie farthest apart (for the probe and the tests):
/// `a`'s samples (not its caps') farther than `beyond` from `b`'s surface,
/// each with its distance, the farthest first.
pub fn far_samples(a: &Shape, b: &Shape, beyond: f32) -> Vec<([f32; 3], f32)> {
    let grid = Grid::of(b);
    let mut stamps = vec![0u32; grid.triangles.len()];
    let mut out = Vec::new();
    let middles = a.sampled().iter().map(|t| {
        let [p, q, r] = t.map(|i| a.points[i as usize]);
        std::array::from_fn(|k| (p[k] + q[k] + r[k]) / 3.0)
    });
    for (n, sample) in a.sampled().iter().flatten().map(|&i| a.points[i as usize]).chain(middles).enumerate() {
        let d = grid.nearest(sample, 0.0, &mut stamps, n as u32 + 1);
        if d > beyond {
            out.push((sample, d));
        }
    }
    out.sort_by(|x, y| y.1.total_cmp(&x.1));
    out.dedup_by(|x, y| x.0 == y.0);
    out
}

/// The largest distance from `a`'s samples (the corners its triangles use,
/// and each triangle's middle, its caps' left out: `Shape::closed`) to
/// `to`'s surface, caps and all; a sample nearer than the largest found so
/// far is not followed further.
fn one_way(a: &Shape, to: &Grid, start: f32) -> f32 {
    let mut used = vec![false; a.points.len()];
    for t in a.sampled() {
        for &i in t {
            used[i as usize] = true;
        }
    }
    let mut stamps = vec![0u32; to.triangles.len()];
    let mut query = 0u32;
    let mut most = start;
    let corners = a.points.iter().zip(&used).filter(|(_, &u)| u).map(|(p, _)| *p);
    let middles = a.sampled().iter().map(|t| {
        let [p, q, r] = t.map(|i| a.points[i as usize]);
        std::array::from_fn(|k| (p[k] + q[k] + r[k]) / 3.0)
    });
    for sample in corners.chain(middles) {
        query = query.wrapping_add(1);
        let near = to.nearest(sample, most, &mut stamps, query);
        if near > most {
            most = near;
        }
    }
    most
}
