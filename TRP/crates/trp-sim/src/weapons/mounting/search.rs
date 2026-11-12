//! Vector helpers and the search for a place to mount on, standing or prone.

use super::*;

pub(super) fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(super) fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(super) fn scale(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(super) fn magnitude(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

/// `Vector3.normalized`: nothing for a vector shorter than 1e-5.
pub(super) fn normalized(a: [f32; 3]) -> [f32; 3] {
    let m = magnitude(a);
    if m > 1e-5 { scale(a, 1.0 / m) } else { [0.0; 3] }
}

const UP: [f32; 3] = [0.0, 1.0, 0.0];

/// The way a yaw faces, and its right (Unity's `Quaternion.Euler(0, yaw, 0)`).
fn forward_of(yaw: f32) -> [f32; 3] {
    let (s, c) = yaw.to_radians().sin_cos();
    [s, 0.0, c]
}

fn right_of(yaw: f32) -> [f32; 3] {
    let (s, c) = yaw.to_radians().sin_cos();
    [c, 0.0, -s]
}

/// `Quaternion.LookRotation(direction).eulerAngles.y`: the bearing of a
/// direction, 0 to 360.
pub fn bearing(direction: [f32; 3]) -> f32 {
    direction[0].atan2(direction[2]).to_degrees().rem_euclid(360.0)
}

/// A missed ray's hit as the engine leaves it: nothing at all, its point
/// and normal nought and its distance 0.
fn missed() -> Hit {
    Hit { distance: 0.0, point: [0.0; 3], normal: [0.0; 3] }
}

/// A batch of rays (`SetRaycasts`): from `start`, stepped along `along`
/// over `size` in `steps`, each cast along `direction` for `length`. The
/// game steps by adding the step in single precision; only the first
/// `steps` are read.
fn cast_batch(world: &dyn World, start: [f32; 3], along: [f32; 3], size: f32, steps: usize, direction: [f32; 3], length: f32, layer: Layer) -> Vec<Hit> {
    let step = size / steps as f32;
    let mut at = 0.0f32;
    let mut hits = Vec::with_capacity(steps);
    for _ in 0..steps {
        let from = add(start, scale(along, at));
        hits.push(if at < size { world.ray(from, direction, length, layer).unwrap_or_else(missed) } else { missed() });
        at += step;
    }
    hits
}

fn hit_any(hit: &Hit) -> bool {
    hit.point != [0.0; 3] || hit.distance != 0.0 || hit.normal != [0.0; 3]
}

/// A search under way (`MountingPointDetectionSystem`): what it will find
/// and how many frames from the key it answers on (a batch of rays is
/// answered the frame after it is cast: mounting spec, "Frames").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Search {
    answer: Option<Point>,
    frames: u32,
}

/// What a standing search starts from (`FindMountingPoint`): the grid's
/// root (the body's `Weapon_root` bone) in the world, the look's yaw, the
/// feet's height, and the pivot heights the grid is raised by: the
/// mount's (the bipod's with it out) for the column, the mount's for the
/// row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    pub root: [f32; 3],
    pub yaw: f32,
    pub feet: f32,
    pub column_raise: f32,
    pub row_raise: f32,
}

impl Search {
    /// A search from standing (or crouched), worked out at the key against
    /// the world as it stands then (PROVISIONAL: the game casts each batch
    /// as the last is answered; a character still for the two or three
    /// frames sees the same).
    pub fn standing(world: &dyn World, s: &Settings, grid: Grid) -> Self {
        let (answer, batches) = standing_search(world, s, grid);
        Self { answer, frames: batches }
    }

    /// A frame has passed: the answer, if this is the frame it comes on
    /// (found or not).
    pub fn frame(&mut self) -> Option<Option<Point>> {
        self.frames = self.frames.saturating_sub(1);
        (self.frames == 0).then_some(self.answer)
    }
}

/// The standing search's answer, and how many batches of rays it cast.
fn standing_search(world: &dyn World, s: &Settings, grid: Grid) -> (Option<Point>, u32) {
    let forward = forward_of(grid.yaw);
    let right = right_of(grid.yaw);
    let down = [0.0, -1.0, 0.0];
    // The column straight ahead (`FindHorizontalMountingPoint`).
    let height = grid.root[1] - grid.feet;
    let low = s.grid[0] + grid.column_raise;
    let high = s.grid[1] + grid.column_raise;
    let top = (height + s.column / 2.0).clamp(low, high);
    let bottom = (height - s.column / 2.0).clamp(low, high);
    let size = (top - bottom).abs() + (grid.column_raise / 2.0).abs();
    let start = [grid.root[0], grid.feet + top, grid.root[2]];
    let column = cast_batch(world, start, down, size, s.column_steps, forward, s.ray, Layer::Detailed);
    let mut batches = 1;
    let mut edge: Option<Hit> = None;
    let (mut nearest_height, mut taken) = (f32::MAX, 0usize);
    for (i, hit) in column.iter().enumerate() {
        let after_gap = i == 0 || !hit_any(&column[i - 1]) || column[i - 1].distance > s.edge;
        let off = if hit_any(hit) { (hit.point[1] - grid.root[1]).abs() } else { f32::MAX };
        let last = edge.map_or(0.0, |e| e.distance);
        let stepped_out = last - hit.distance > s.edge * 0.2 && taken + 1 == i;
        if hit_any(hit) && hit.distance <= s.edge && ((after_gap && nearest_height > off) || stepped_out) {
            edge = Some(*hit);
            nearest_height = off;
            taken = i;
        }
    }
    if let Some(edge) = edge {
        let above = add(edge.point, scale(UP, s.top_offset));
        if world.ray(above, forward, s.top_row, Layer::Detailed).is_none() {
            batches += 1;
            let tops = cast_batch(world, above, forward, s.top_row, s.top_steps, down, s.top_ray, Layer::Detailed);
            let mut found: Option<[f32; 3]> = None;
            for hit in &tops {
                if hit.point == [0.0; 3] {
                    continue;
                }
                if hit.distance == 0.0 {
                    found = None;
                    break;
                }
                let higher = found.map_or(f32::NEG_INFINITY, |f| f[1]);
                if hit.point[1] > edge.point[1] && hit.point[1] > higher && dot(hit.normal, UP) >= s.top_dot {
                    found = Some(hit.point);
                }
            }
            if let Some(top) = found {
                return (Some(Point { at: add(top, scale(UP, s.top_point)), direction: scale(edge.normal, -1.0), side: Side::Forward }), batches);
            }
        }
    }
    // The row at the sides (`FindVerticalMountingPoint`): on the detailed
    // layers, then on the coarse one.
    let raised = (grid.root[1] - grid.feet).clamp(s.grid[0] + grid.row_raise, s.grid[1] + grid.row_raise - s.grid_from_top);
    let centre = [grid.root[0], grid.feet + raised, grid.root[2]];
    let start = add(centre, scale(right, s.row / 2.0));
    for layer in [Layer::Detailed, Layer::Coarse] {
        batches += 1;
        if let Some(point) = row_search(world, s, start, forward, right, layer) {
            return (Some(point), batches);
        }
    }
    (None, batches)
}

/// One cast of the side row (`FindHorizontalGridPoint`).
fn row_search(world: &dyn World, s: &Settings, start: [f32; 3], forward: [f32; 3], right: [f32; 3], layer: Layer) -> Option<Point> {
    let n = s.row_steps;
    if n < 3 {
        return None;
    }
    let hits = cast_batch(world, start, scale(right, -1.0), s.row, n, forward, s.ray, layer);
    let mut both = vec![false; n + 1];
    let mut least = 1.0f32;
    for i in 1..n - 1 {
        both[i] = hits[i - 1].distance != 0.0 && hits[i + 1].distance != 0.0;
        if !(hits[i].distance > s.edge) {
            least = least.min(dot(hits[i].normal, forward));
        }
    }
    let blocked: Vec<bool> = (0..n).map(|j| j >= 1 && j < n - 1 && hits[j].normal != [0.0; 3] && (dot(hits[j].normal, forward) - least).abs() > 0.001).collect();
    let middle = n / 2;
    let (mut chosen, mut chosen_at, mut chosen_off, mut side) = (missed(), None::<usize>, 0usize, Side::Forward);
    for k in 1..n - 1 {
        let hit = hits[k];
        if blocked[k] || hit.point == [0.0; 3] || dot(hit.normal, UP).abs() > s.side_dot || hit.distance > s.edge {
            continue;
        }
        let off = middle.abs_diff(k);
        let free_after = hits[k + 1].point == [0.0; 3] && !both[k + 1];
        let free_before = hits[k - 1].point == [0.0; 3] && !both[k - 1];
        let better = chosen_at.is_none() || chosen_off > off || (chosen_off == off && chosen.distance > hit.distance);
        if (free_after || free_before) && better {
            side = if hits[k - 1].point == [0.0; 3] { Side::Right } else { Side::Left };
            chosen = hit;
            chosen_at = Some(k);
            chosen_off = off;
        }
    }
    // With nothing chosen the game goes on from the world's origin: its
    // point is the missed hit's nought (kept: the check below then finds
    // nothing on open ground).
    let point = chosen.point;
    let away = if side == Side::Right { right } else { scale(right, -1.0) };
    let facing = normalized([chosen.normal[0], 0.0, chosen.normal[2]]);
    let along = normalized(sub(away, scale(facing, dot(away, facing))));
    let round = add(point, scale(add(along, facing), s.wall_check));
    if world.ray(round, scale(facing, -1.0), s.wall_check, layer).is_some() {
        return None;
    }
    let origin = add(sub(point, scale(facing, s.wall_in)), scale(along, s.wall_check));
    let again = world.ray(origin, scale(along, -1.0), s.wall_check, layer).filter(|h| dot(h.normal, UP).abs() <= s.side_dot)?;
    let at = add(add(point, scale(along, s.wall_check - again.distance)), scale(normalized(add(along, facing)), s.side_point));
    Some(Point { at, direction: scale(facing, -1.0), side })
}

/// The search lying down (`FindMountingPointProne`), answered at once: the
/// grid's root turned by the look's yaw and the ground's pitch; at the
/// bipod's reach ahead a free sphere and a ray down onto level enough
/// ground. `reach` and `height` are the bipod pivot's, unsigned.
pub fn search_prone(world: &dyn World, s: &Settings, root: [f32; 3], yaw: f32, ground_pitch: f32, reach: f32, height: f32) -> Option<Point> {
    let turn = crate::base::math::quat_mul(quat_axis(1, yaw), quat_axis(0, ground_pitch));
    let forward = quat_rotate(turn, [0.0, 0.0, 1.0]);
    let up = quat_rotate(turn, UP);
    let length = PRONE_RAY_MOST.min(height * 2.0);
    let at = add(root, scale(forward, reach));
    if !world.sphere_clear(at, PRONE_SPHERE, Layer::Coarse) {
        return None;
    }
    let hit = world.ray(at, scale(up, -1.0), length, Layer::Coarse)?;
    (hit.distance != 0.0 && dot(hit.normal, up) >= s.prone_dot).then(|| Point { at: add(hit.point, scale(hit.normal, s.top_point)), direction: forward, side: Side::Forward })
}
