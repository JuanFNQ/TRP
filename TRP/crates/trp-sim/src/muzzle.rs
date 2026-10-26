//! A weapon's muzzle flash (firing spec, "The muzzle flash"; `MuzzleJet.
//! UpdateOrCreateMesh`, `MuzzleJet.RandomizeMaterial`, `MuzzleManager.Shot`,
//! `ManualUpdate`): the jets' mesh as the game builds it when the weapon is
//! put together, each shot's draws for its material, and the shot's
//! figures over its time. The host draws them with the jets' shader.

use std::collections::VecDeque;

use trp_data::muzzle::{FumeData, HazeData, MuzzleData, SmokeData, SparksData};
use trp_data::unity::extract::Hierarchy;

use crate::hands::slerp3;
use crate::math::{quat_mul, quat_rotate};
use crate::recoil::Rng;

/// What `RandomizeMaterial` scales its four draws by.
const RANDOM_SCALE: f32 = 878_788.8;

/// One corner of a jet's quad, in the axes of the object the mesh hangs
/// from (the game's: left-handed, z ahead).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JetVertex {
    pub position: [f32; 3],
    /// The jet's axis.
    pub normal: [f32; 3],
    /// Its atlas cell.
    pub uv0: [f32; 2],
    /// The corner's direction, times the particle's size.
    pub uv1: [f32; 2],
    /// The draws and the share along the jet, 0 to 1 (`Color32` over 255).
    pub color: [f32; 4],
    pub tangent: [f32; 4],
}

/// The draws a shot gives the material (`_RandomVals`, `_RotMx`,
/// `_CellShift`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShotDraws {
    pub random_vals: [f32; 4],
    pub rot_mx: [f32; 4],
    pub cell_shift: [f32; 2],
}

/// `Random.Range` for whole numbers: from `low` to below `high`.
fn whole(rng: &mut Rng, low: i32, high: i32) -> i32 {
    (low + (rng.unit() * (high - low) as f32) as i32).min(high - 1)
}

/// `new Vector2(0.5 − Random.value, 0.5 − Random.value).normalized`.
fn direction(rng: &mut Rng) -> [f32; 2] {
    let v = [0.5 - rng.unit(), 0.5 - rng.unit()];
    let length = (v[0] * v[0] + v[1] * v[1]).sqrt();
    if length > 1e-5 {
        [v[0] / length, v[1] / length]
    } else {
        [0.0, 0.0]
    }
}

/// A `Color32` channel from a share (`(byte)(255 × share)`), over 255.
fn byte(share: f32) -> f32 {
    (255.0 * share).clamp(0.0, 255.0).trunc() / 255.0
}

/// The place and turn of `child` in `parent`'s axes, from the local
/// places, turns and scales between them (`GetMatrixRelativeToParent`):
/// none where `parent` is not above it (the game's nought and no turn).
fn relative(hierarchy: &Hierarchy, parent: usize, child: usize) -> ([f32; 3], [f32; 4]) {
    if parent == child {
        return ([0.0; 3], [0.0, 0.0, 0.0, 1.0]);
    }
    let mut place = [0.0f32; 3];
    let mut turn = [0.0, 0.0, 0.0, 1.0];
    let mut node = child;
    loop {
        let n = &hierarchy.nodes[node];
        // This node's own local TRS applied to what is below it.
        let scaled = [place[0] * n.scale[0], place[1] * n.scale[1], place[2] * n.scale[2]];
        let turned = quat_rotate(n.rotation, scaled);
        place = [turned[0] + n.translation[0], turned[1] + n.translation[1], turned[2] + n.translation[2]];
        turn = quat_mul(n.rotation, turn);
        match n.parent {
            Some(up) if up == parent => return (place, turn),
            Some(up) => node = up,
            None => return ([0.0; 3], [0.0, 0.0, 0.0, 1.0]),
        }
    }
}

/// The jets' mesh (`UpdateOrCreateMesh`, `FillJet`, `FillParticle`): a quad
/// for each particle of each jet, its corners and its six indices.
pub fn jet_mesh(data: &MuzzleData, hierarchy: &Hierarchy, rng: &mut Rng) -> (Vec<JetVertex>, Vec<u32>) {
    let cell = [1.0 / data.atlas[0] as f32, 1.0 / data.atlas[1] as f32];
    let mut vertices = Vec::new();
    for jet in &data.jets {
        let (place, turn) = relative(hierarchy, data.mesh_parent, jet.node);
        let axis = quat_rotate(turn, [0.0, -1.0, 0.0]);
        let (least, most) = jet.particles.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), p| (a.min(p.position), b.max(p.position)));
        let norm = [least, 1.0 / (most - least)];
        let add_length = jet.bounds[2] - jet.bounds[1];
        let common = byte(rng.unit());
        for particle in &jet.particles {
            let v = direction(rng);
            let v = [v[0] * particle.size, v[1] * particle.size];
            let v2 = [v[1], -v[0]];
            let corners = [v, [-v2[0], -v2[1]], [-v[0], -v[1]], v2];
            let u0 = whole(rng, -255, 255) as f32 * cell[0];
            let u1 = cell[0] + u0;
            let w0 = whole(rng, -255, 255) as f32 * cell[1];
            let w1 = cell[1] + w0;
            let cells = [[u0, w1], [u0, w0], [u1, w0], [u1, w1]];
            let share = (particle.position - norm[0]) * norm[1];
            let share = if share.is_finite() { share } else { 0.0 };
            let color = [common, whole(rng, 127, 255) as f32 / 255.0, byte(share), byte(jet.chance)];
            let position = [place[0] + axis[0] * particle.position, place[1] + axis[1] * particle.position, place[2] + axis[2] * particle.position];
            let tangent = [add_length * share, particle.axis_shift, particle.shift[0], particle.shift[1]];
            for k in 0..4 {
                vertices.push(JetVertex { position, normal: axis, uv0: cells[k], uv1: corners[k], color, tangent });
            }
        }
    }
    let quads = (vertices.len() / 4) as u32;
    let indices = (0..quads).flat_map(|q| [4 * q, 4 * q + 1, 4 * q + 2, 4 * q + 2, 4 * q + 3, 4 * q]).collect();
    (vertices, indices)
}

/// A weapon's flash over time: when the last shot's ends.
#[derive(Debug, Clone)]
pub struct MuzzleFlash {
    end: f32,
}

impl Default for MuzzleFlash {
    fn default() -> Self {
        Self { end: f32::NEG_INFINITY }
    }
}

impl MuzzleFlash {
    /// A shot (`MuzzleManager.Shot`): its time from now, and the material's
    /// new draws (`RandomizeMaterial`).
    pub fn shot(&mut self, data: &MuzzleData, now: f32, rng: &mut Rng) -> ShotDraws {
        self.end = now + data.shot_length;
        let random_vals = [0; 4].map(|_| (1.0 + rng.unit()) * RANDOM_SCALE);
        let d = direction(rng);
        let cell = [1.0 / data.atlas[0] as f32, 1.0 / data.atlas[1] as f32];
        let cell_shift = [whole(rng, -255, 255) as f32 * cell[0], whole(rng, -255, 255) as f32 * cell[1]];
        ShotDraws { random_vals, rot_mx: [d[0], d[1], -d[1], d[0]], cell_shift }
    }

    /// `_ShotVals` now (`ManualUpdate`, `SetT`): the shot's share of its
    /// time, and the two curves at it (each held at its ends); before any
    /// shot, as after one.
    pub fn shot_vals(&self, data: &MuzzleData, now: f32) -> [f32; 2] {
        let t = 1.0 - (self.end - now) / data.shot_length;
        let t = if t.is_finite() { t } else { f32::MAX };
        [1.0 - data.light_curve.evaluate(t), data.move_curve.evaluate(t)]
    }
}

/// `FastRndom.Int(input, start, range)`: the number shifted left 7 and
/// xored, right 3 and xored, its size modulo the range, from the start.
fn hashed(input: i32, start: i32, range: i32) -> i32 {
    if range == 0 {
        return start;
    }
    let mut n = input;
    n ^= n.wrapping_shl(7);
    n ^= n >> 3;
    start + n.wrapping_abs() % range
}

/// `MuzzleSmoke.GetRnd`: between the draws of a place's whole number and
/// the next, by a smooth step.
pub fn smoke_draw(t: f32) -> f32 {
    let n = t as i32;
    let a = hashed(n.wrapping_add(8_324_234), -1000, 2000) as f32 * 0.001;
    let b = hashed(n.wrapping_add(1).wrapping_add(8_324_234), -1000, 2000) as f32 * 0.001;
    let f = t - n as f32;
    let f = f * f * (3.0 - 2.0 * f);
    a + (b - a) * f
}

/// `FastRndom`'s running draws (firing spec, "The muzzle's fumes"): the game's own
/// tables, made by Unity's generator at the game's seeds (`unity_random`).
pub use crate::unity_random::FastRandom;

/// A shot's puffs from one fume (`MuzzleFume.Emit` and the manager's
/// `IFumeEmitter.Emit`): its object's place and forward (minus its up), and
/// how far the object has moved since its place was noted, in the game's
/// axes. `rng` is the engine's own draw for the size (`Random.Range`).
pub fn fume_puffs(fume: &FumeData, place: [f32; 3], forward: [f32; 3], moved: [f32; 3], fast: &mut FastRandom, rng: &mut Rng) -> Vec<crate::particles::EmitParams> {
    let start = add(place, scale(forward, fume.start_pos));
    let count = if fume.count_range < 1 { fume.count_min } else { fast.int(fume.count_min, fume.count_range) };
    (0..count.max(0))
        .map(|_| {
            let mut position = start;
            if fume.emitter_radius >= f32::EPSILON {
                position = add(position, scale(fast.vector(), fume.emitter_radius));
            }
            let time = fast.float();
            let way = add(forward, scale(fast.vector(), fume.conus_size));
            let length = square(way).sqrt();
            let way = if length > 1e-6 { scale(way, 1.0 / length) } else { [0.0; 3] };
            let velocity = add(scale(moved, 0.5), scale(way, fume.speeds.evaluate(time) * fume.speed));
            let size = (fume.sizes.evaluate(time) + rng.range(-fume.sizes_rnd, fume.sizes_rnd)) * fume.size;
            let lifetime = fume.lifetimes.evaluate(time) * fume.lifetime;
            // `Color32` of the colour: each channel held to 0..1, times 255,
            // rounded.
            let color = fume.color.evaluate(time).map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
            // The manager's own: a seed, then the turn.
            let seed = fast.uint();
            let rotation = fast.rotation();
            crate::particles::EmitParams { position, velocity, size, lifetime, color, rotation, seed }
        })
        .collect()
}

/// A shot's sparks from one muzzle's (`MuzzleSparks.Emit`): its object's
/// place and forward (minus its up) in the game's axes, at the time `now`.
/// `rng` is the engine's own draw of the tables' entry
/// (`Random.Range(0, 128)`).
pub fn spark_emits(sparks: &SparksData, place: [f32; 3], forward: [f32; 3], now: f32, fast: &mut FastRandom, rng: &mut Rng) -> Vec<crate::sparks::SparkEmit> {
    let start = add(place, scale(forward, sparks.start_pos));
    let count = if sparks.count_range < 1 { sparks.count_min } else { fast.int(sparks.count_min, sparks.count_range) };
    let time = now + sparks.emission_time_shift;
    (0..count.max(0))
        .map(|_| {
            let mut position = start;
            if sparks.emitter_radius >= f32::EPSILON {
                position = add(position, scale(fast.vector(), sparks.emitter_radius));
            }
            let way = add(forward, scale(fast.vector(), sparks.conus_size));
            let length = square(way).sqrt();
            let way = if length > 1e-6 { scale(way, 1.0 / length) } else { [0.0; 3] };
            // Each table has 128 (`OnValidate`); held within the shortest.
            let shortest = [sparks.lifetime.len(), sparks.speed.len(), sparks.gravity.len(), sparks.drag.len(), sparks.emission.len(), sparks.size.len(), sparks.turbulence.len(), sparks.frequency.len()].into_iter().min().unwrap_or(0);
            let at = ((rng.unit() * 128.0) as usize).min(shortest.saturating_sub(1));
            crate::sparks::SparkEmit {
                position,
                velocity: scale(way, sparks.speed[at]),
                time,
                gravity: sparks.gravity[at],
                drag: sparks.drag[at],
                lifetime: sparks.lifetime[at],
                emission: sparks.emission[at],
                size: sparks.size[at],
                turbulence: sparks.turbulence[at],
                frequency: sparks.frequency[at],
            }
        })
        .collect()
}

/// Whether a shot has this haze (`HeatHazeEmitter.OnShot`: a draw from
/// 0.001 to 1 not above its chance; none with no puffs). Its puffs come
/// its delay later (`haze_puffs`).
pub fn haze_shot(haze: &HazeData, rng: &mut Rng) -> bool {
    haze.count > 0 && rng.range(0.001, 1.0) <= haze.chance
}

/// A haze's puffs (`HeatHazeEmitter.Emit`), its delay after the shot: its
/// object's place and its place from its parent in the parent's turn
/// (`rotation × inverse(localRotation) × localPosition`), in the game's
/// axes. One size for all; each its own velocity (that place plus a draw in
/// a ball of `Angle`, times `VelocityFactor`), half the size ahead along it,
/// a turn of 0 to 89 degrees, a lifetime between its two; white.
pub fn haze_puffs(haze: &HazeData, place: [f32; 3], from_parent: [f32; 3], rng: &mut Rng) -> Vec<crate::particles::EmitParams> {
    let size = rng.range(haze.size[0], haze.size[1]);
    (0..haze.count.max(0))
        .map(|_| {
            let ball = loop {
                let v = [rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)];
                if square(v) <= 1.0 {
                    break v;
                }
            };
            let velocity = scale(add(from_parent, scale(ball, haze.angle)), haze.velocity_factor);
            let rotation = (rng.unit() * 90.0).floor().min(89.0);
            let length = square(velocity).sqrt();
            let ahead = if length > 1e-6 { scale(velocity, size * 0.5 / length) } else { [0.0; 3] };
            let lifetime = rng.range(haze.lifetime[0], haze.lifetime[1]);
            crate::particles::EmitParams { position: add(place, ahead), velocity, size, lifetime, color: [255; 4], rotation, seed: rng.unit().to_bits() }
        })
        .collect()
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn square(a: [f32; 3]) -> f32 {
    a[0] * a[0] + a[1] * a[1] + a[2] * a[2]
}

/// A point of a trail (`MuzzleSmoke.Point`).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Point {
    skip: bool,
    position: [f32; 3],
    velocity: [f32; 3],
    turbulence: [f32; 3],
    /// Its colour's alpha, a byte.
    alpha: u8,
    diffusion: f32,
    number: i32,
}

/// One corner of a trail's quad as drawn (`MuzzleSmoke.Draw`): its place
/// (the game's axes), its side (−1 or 1), its number along the trail, its
/// diffusion, its direction and its alpha (over 255).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmokeCorner {
    pub position: [f32; 3],
    pub side: f32,
    pub number: f32,
    pub diffusion: f32,
    pub direction: [f32; 3],
    pub alpha: f32,
}

/// A gun smoke's trail (`MuzzleSmoke`).
#[derive(Debug, Clone, Default)]
pub struct Smoke {
    on: bool,
    shot_factor: f32,
    points: VecDeque<Point>,
    pos: [f32; 3],
    velocity: [f32; 3],
    speed: f32,
    next_number: i32,
    turbulence_at: f32,
    length_share: f32,
    count: i32,
    smoking: f32,
}

impl Smoke {
    /// Whether it is on (its frame worked: `enabled`).
    pub fn on(&self) -> bool {
        self.on
    }

    /// `Clear`: its points let go, off.
    pub fn clear(&mut self) {
        self.points.clear();
        self.smoking = 0.0;
        self.on = false;
    }

    /// A shot (`Shot`): the factor up; at whole, the trail begun again.
    pub fn shot(&mut self, f: &SmokeData, rng: &mut Rng) {
        self.shot_factor = (self.shot_factor + f.increasing_by_shot).min(1.0);
        if self.shot_factor >= 1.0 {
            self.clear();
            self.on = true;
            self.count = 1;
            self.length_share = 1.0 / (f.smoke_length + rng.range(0.0, f.smoke_length_randomness));
        }
    }

    /// `CreatePoint`: at the head, its alpha by the smoking and the speed.
    fn create(&mut self, f: &SmokeData, skip: bool) {
        let alpha = ((self.smoking * f.speed_smoke_strength.evaluate(self.speed)).clamp(0.0, 1.0) * 255.0) as u8;
        let number = self.next_number;
        self.next_number = self.next_number.wrapping_add(1);
        self.points.push_front(Point { skip, position: self.pos, velocity: [0.0; 3], turbulence: [0.0; 3], alpha, diffusion: 0.0, number });
    }

    /// A frame while on (`LateUpdateValues`): the head, the points made,
    /// moved and let go. `place` is the object's (the game's axes),
    /// `forward` the camera's.
    pub fn frame(&mut self, f: &SmokeData, dt: f32, place: [f32; 3], forward: [f32; 3]) {
        if !self.on || dt < f32::EPSILON {
            return;
        }
        self.shot_factor = (self.shot_factor - dt * f.drop_time).max(0.0);
        let last = self.pos;
        self.pos = slerp3(self.pos, place, 0.125);
        self.velocity = slerp3(self.velocity, scale(sub(self.pos, last), 1.0 / dt), 1.0 / 32.0);
        self.speed += ((square(self.velocity).sqrt() * f.speed_multiplier) - self.speed) * 0.25;
        let share = self.count as f32 * self.length_share;
        self.smoking = if share >= 1.0 { 0.0 } else { f.smoke.evaluate(share) * f.smoke_strength };
        self.initialize(f, forward);
        if !self.points.is_empty() {
            self.process(f, dt);
        }
    }

    /// `InitializePoints`.
    fn initialize(&mut self, f: &SmokeData, forward: [f32; 3]) {
        let brake = f.brake_distance * f.brake_distance;
        if self.smoking > 0.001 {
            if self.points.is_empty() {
                self.create(f, false);
                self.create(f, true);
            } else {
                self.points[0].position = self.pos;
            }
            let far = self.points.get(1).is_some_and(|second| square(sub(self.points[0].position, second.position)) > brake);
            if far {
                self.count += 1;
                self.turbulence_at += f.speed_turbulence_density.evaluate(self.speed) * f.turbulence_density;
                let mut v = self.velocity;
                v[1] += 0.1;
                // `Vector3.Cross(v, forward).normalized`.
                let cross = [v[1] * forward[2] - v[2] * forward[1], v[2] * forward[0] - v[0] * forward[2], v[0] * forward[1] - v[1] * forward[0]];
                let length = square(cross).sqrt();
                let unit = if length > 1e-5 { scale(cross, 1.0 / length) } else { [0.0; 3] };
                let turbulence = scale(unit, smoke_draw(self.turbulence_at) * f.turbulence_intensity);
                let strength = f.speed_turbulence_strength.evaluate(self.speed);
                let start = f.speed_start_diffusion.evaluate(self.speed) * f.smoke_end;
                let first = &mut self.points[0];
                first.turbulence = turbulence;
                first.velocity = add(first.velocity, scale(add(self.velocity, scale(turbulence, strength)), f.smoke_velocity));
                first.diffusion = start;
                first.skip = false;
                self.create(f, true);
            }
        } else if self.points.len() >= 2 && self.points[0].skip && square(sub(self.points[0].position, self.points[1].position)) > brake {
            self.points[0].skip = false;
        }
        if self.points.is_empty() {
            self.clear();
        } else if self.points.len() == 2 {
            if self.points[0].diffusion >= f.smoke_end {
                self.points.clear();
            }
        } else if self.points.len() >= 3 && self.points[self.points.len() - 2].diffusion >= f.smoke_end {
            self.points.pop_back();
        }
    }

    /// `ProcessPoints`.
    fn process(&mut self, f: &SmokeData, dt: f32) {
        let fall = f.gravity * dt;
        for p in self.points.iter_mut().filter(|p| !p.skip) {
            p.velocity[1] -= fall;
            p.velocity = scale(p.velocity, f.drag);
            let moved = scale(add(p.velocity, p.turbulence), dt);
            p.position = add(p.position, moved);
            p.diffusion += square(moved) * f.diffusion_by_velocity;
        }
    }

    /// `Draw`: each point's direction, and a quad's four corners between
    /// each two neighbours, newest first.
    pub fn corners(&self) -> Vec<SmokeCorner> {
        let n = self.points.len();
        if n < 2 {
            return Vec::new();
        }
        let direction = |i: usize| -> [f32; 3] {
            let p = &self.points;
            if i == 0 {
                sub(p[n - 1].position, p[0].position)
            } else if i + 1 < n {
                sub(p[i + 1].position, p[i - 1].position)
            } else {
                sub(p[i].position, p[i - 1].position)
            }
        };
        let corner = |i: usize, side: f32| {
            let p = &self.points[i];
            SmokeCorner { position: p.position, side, number: p.number as f32, diffusion: p.diffusion, direction: direction(i), alpha: p.alpha as f32 / 255.0 }
        };
        (1..n).flat_map(|i| [corner(i - 1, -1.0), corner(i, -1.0), corner(i, 1.0), corner(i - 1, 1.0)]).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trp_data::curve::{Curve, Key};
    use trp_data::muzzle::{Jet, JetMaterial, JetParticle};
    use trp_data::unity::extract::{Node, TextureData};
    use trp_data::unity::ObjRef;

    fn node(name: &str, parent: Option<usize>, translation: [f32; 3], rotation: [f32; 4]) -> Node {
        Node {
            name: name.to_owned(),
            parent,
            translation,
            rotation,
            scale: [1.0; 3],
            active: true,
            renderer: None,
            scripts: Vec::new(),
            script_objects: Vec::new(),
            grip: None,
            placer: None,
            transform: ObjRef { file: 0, path_id: 0 },
            colliders: Vec::new(),
        }
    }

    fn data() -> MuzzleData {
        let key = |time, value, slope| Key { time, value, in_slope: slope, out_slope: slope };
        let texture = TextureData { name: String::new(), width: 1, height: 1, format: 4, mip_count: 1, srgb: false, data: vec![0; 4], streaming_mipmaps: false, point: false, clamp: [false; 2], faces: 1 };
        MuzzleData {
            shot_length: 0.12,
            move_curve: Curve { keys: vec![key(0.0, -0.53, 1.75), key(1.0, 1.2166, 0.45)] },
            light_curve: Curve { keys: vec![key(0.0, 0.0, 2.0), key(0.128, 1.0, 0.017), key(0.609, 1.0, 0.057), key(1.0, 0.0, 0.0)] },
            atlas: [2, 2],
            mesh_parent: 1,
            material: JetMaterial { texture, tint: [1.0, 0.46, 0.26, 1.0], inv_window: 1.37, randomize_phase: 2.0, hdr: 1.2, z_shift: 0.94, render_queue: -1 },
            jets: vec![Jet {
                node: 3,
                particles: (0..8).map(|i| JetParticle { position: i as f32 * 0.02, size: 0.05, shift: [0.0, 0.0], axis_shift: 0.0 }).collect(),
                bounds: [0.0, 0.1, 0.3],
                chance: 0.15,
            }],
            smokes: Vec::new(),
            fumes: Vec::new(),
            sparks: Vec::new(),
            hazes: Vec::new(),
            pivots: Vec::new(),
        }
    }

    /// The AKM's smoke (`the_akm_s_muzzle_flash_is_read`).
    fn smoke_data() -> SmokeData {
        let key = |time, value, slope| Key { time, value, in_slope: slope, out_slope: slope };
        let texture = TextureData { name: String::new(), width: 1, height: 1, format: 4, mip_count: 1, srgb: false, data: vec![0; 4], streaming_mipmaps: false, point: false, clamp: [false; 2], faces: 1 };
        SmokeData {
            node: 0,
            smoke_end: 0.03,
            brake_distance: 0.01,
            drag: 0.95,
            gravity: -1.1,
            smoke_velocity: 0.4,
            turbulence_density: 0.4,
            turbulence_intensity: 0.1,
            diffusion_by_velocity: 32.0,
            speed_multiplier: 1.2,
            speed_turbulence_density: Curve { keys: vec![key(0.0, 1.0, 0.1), key(9.0, 5.0, 0.0)] },
            speed_turbulence_strength: Curve { keys: vec![key(0.0, 1.0, 30.0), key(8.0, 80.0, 0.0)] },
            speed_smoke_strength: Curve { keys: vec![key(0.0, 1.0, -0.26), key(2.27, 0.65, -0.05), key(10.0, 0.1, -0.23)] },
            speed_start_diffusion: Curve { keys: vec![key(33.0, 0.05, 0.01), key(35.0, 0.5, 0.005)] },
            smoke: Curve { keys: vec![key(0.0, 0.0, 0.1), key(0.2, 1.0, 0.0), key(0.45, 1.0, 0.0), key(1.0, 0.0, 0.0)] },
            smoke_strength: 0.3,
            smoke_length: 20.0,
            smoke_length_randomness: 10.0,
            increasing_by_shot: 0.4,
            drop_time: 0.5,
            material: trp_data::muzzle::SmokeMaterial { texture, width: 0.005, uv_y_scale: 0.03, diffusion_strength: 60.0, start_fade: 0.0003, end_fade: 0.05, end: 0.03, inv_fade: 1.0, alpha: 1.0 },
        }
    }

    #[test]
    fn the_smoke_s_draws_are_the_game_s_hash() {
        // `FastRndom.Int(8324234, −1000, 2000)` worked by hand.
        let mut n: i32 = 8_324_234;
        n ^= n.wrapping_shl(7);
        n ^= n >> 3;
        assert_eq!(hashed(8_324_234, -1000, 2000), -1000 + n.wrapping_abs() % 2000);
        // Whole places give their own draw; between, a smooth step.
        let (a, b) = (smoke_draw(3.0), smoke_draw(4.0));
        assert!((-1.0..1.0).contains(&a) && (-1.0..1.0).contains(&b));
        assert!((smoke_draw(3.5) - (a + b) / 2.0).abs() < 1e-5);
    }

    /// The AKM's first fume (`muzzleflash_000`; firing spec, "The muzzle's
    /// fumes").
    fn akm_fume() -> FumeData {
        let key = |time, value, slope| Key { time, value, in_slope: slope, out_slope: slope };
        FumeData {
            node: 0,
            start_pos: 0.0,
            emitter_radius: 0.0,
            conus_size: 0.0,
            sizes: Curve { keys: vec![key(0.0, 0.4025, 0.0), key(1.0, 0.7521, 0.0)] },
            speeds: Curve { keys: vec![key(0.3999, 0.8285, 1.0922), key(0.5778, 1.0, 1.3817)] },
            lifetimes: Curve { keys: vec![key(0.4, 0.7, 0.0), key(0.6, 1.0, -0.0401)] },
            sizes_rnd: 0.068,
            size: 0.4,
            speed: 5.0,
            lifetime: 0.2,
            color: trp_data::particles::Gradient { colors: vec![(0.0, [1.0; 3]), (1.0, [1.0; 3])], alphas: vec![(0.0, 0.8627), (0.494, 0.8392), (1.0, 0.0824)], stepped: false },
            count_min: 1,
            count_range: 1,
        }
    }

    #[test]
    fn a_fume_s_puffs_are_the_game_s() {
        // `FastRndom.Int(start, range)` and `Uint()` from their first
        // state, worked by hand.
        let mut n: i32 = 242_313_459;
        n ^= n.wrapping_shl(7);
        n ^= n >> 3;
        let mut u: u32 = 242_313_459;
        u ^= u << 7;
        u ^= u >> 3;
        let mut fast = FastRandom::default();
        assert_eq!(fast.int(3, 5), 3 + n.wrapping_abs() % 5);
        assert_eq!(fast.uint(), u);
        // The three tables' draws come off the one cursor, stepping back.
        let mut a = FastRandom::default();
        let mut b = FastRandom::default();
        assert_eq!(a.float(), b.float());
        assert_eq!(a.rotation(), b.rotation());
        assert!((square(a.vector()) - 1.0).abs() < 1e-5);
        // One puff a shot, straight ahead, with half the muzzle's movement.
        let f = akm_fume();
        let mut rng = Rng::new(3);
        for _ in 0..50 {
            let puffs = fume_puffs(&f, [1.0, 1.5, 0.0], [0.0, 0.0, 1.0], [0.02, 0.0, 0.0], &mut fast, &mut rng);
            assert_eq!(puffs.len(), 1);
            let p = puffs[0];
            assert_eq!(p.position, [1.0, 1.5, 0.0]);
            assert!((p.velocity[0] - 0.01).abs() < 1e-6 && p.velocity[1] == 0.0);
            assert!(p.velocity[2] >= 0.8285 * 5.0 - 1e-3 && p.velocity[2] <= 5.0 + 1e-3, "{:?}", p.velocity);
            assert!(p.lifetime >= 0.14 - 1e-4 && p.lifetime <= 0.2 + 1e-4, "{}", p.lifetime);
            assert!(p.size >= (0.4025 - 0.068) * 0.4 - 1e-4 && p.size <= (0.7521 + 0.068) * 0.4 + 1e-4, "{}", p.size);
            assert!((21..=220).contains(&p.color[3]), "{:?}", p.color);
            assert!((0.0..360.0).contains(&p.rotation));
        }
    }

    #[test]
    fn a_muzzle_s_sparks_are_the_game_s() {
        // The AKM's port's: a count of −5 and a range of 7, so most shots
        // none and some one; each from its tables' one entry.
        let table = |v: f32| vec![v; 128];
        let s = SparksData {
            node: 0,
            start_pos: 0.0,
            emitter_radius: 0.0,
            conus_size: 0.1,
            count_min: -5,
            count_range: 7,
            lifetime: table(0.6),
            speed: (0..128).map(|i| i as f32).collect(),
            gravity: table(0.4),
            drag: table(0.0016),
            emission: vec![255; 128],
            size: vec![13; 128],
            turbulence: vec![27; 128],
            frequency: vec![134; 128],
            emission_time_shift: -0.01,
        };
        let (mut fast, mut rng) = (FastRandom::default(), Rng::new(9));
        let counts: Vec<usize> = (0..200).map(|_| spark_emits(&s, [0.0; 3], [0.0, 0.0, 1.0], 5.0, &mut fast, &mut rng).len()).collect();
        assert!(counts.iter().all(|&n| n <= 1) && counts.contains(&0) && counts.contains(&1), "{counts:?}");
        let one = (0..200).find_map(|_| spark_emits(&s, [0.0; 3], [0.0, 0.0, 1.0], 5.0, &mut fast, &mut rng).first().copied()).expect("a spark");
        assert_eq!((one.time, one.lifetime, one.size), (4.99, 0.6, 13));
        // Its speed the table's entry, along a way strayed by the cone.
        let speed = square(one.velocity).sqrt();
        assert!(speed.fract().abs() < 1e-4 || (1.0 - speed.fract()).abs() < 1e-4, "{speed}");
        assert!(one.velocity[2] > 0.0);
    }

    #[test]
    fn a_shot_s_haze_is_the_game_s() {
        let haze = HazeData { node: 0, lifetime: [0.46, 1.45], size: [0.29, 0.86], count: 3, velocity_factor: 10.0, angle: 0.04, delay: 0.15, chance: 0.25 };
        let mut rng = Rng::new(5);
        // A quarter of the shots, about; none with no puffs.
        let had = (0..4000).filter(|_| haze_shot(&haze, &mut rng)).count();
        assert!((800..1200).contains(&had), "{had}");
        assert!(!haze_shot(&HazeData { count: 0, chance: 1.0, ..haze.clone() }, &mut rng));
        // Three puffs of one size, each along the place from its parent
        // (2 cm ahead) times ten, give or take the ball's 0.04 times ten.
        let puffs = haze_puffs(&haze, [0.0, 1.5, 0.0], [0.0, 0.0, 0.02], &mut rng);
        assert_eq!(puffs.len(), 3);
        assert!(puffs.iter().all(|p| p.size == puffs[0].size && (0.29..=0.86).contains(&p.size)));
        for p in &puffs {
            assert!((p.velocity[2] - 0.2).abs() <= 0.4 + 1e-5 && p.velocity[0].abs() <= 0.4 + 1e-5, "{:?}", p.velocity);
            let ahead = std::array::from_fn::<f32, 3, _>(|k| p.position[k] - [0.0, 1.5, 0.0][k]);
            assert!((square(ahead).sqrt() - p.size * 0.5).abs() < 1e-4);
            assert!((0.0..90.0).contains(&p.rotation) && p.rotation.fract() == 0.0);
            assert!((0.46..=1.45).contains(&p.lifetime) && p.color == [255; 4]);
        }
    }

    /// The game works its trail once a frame, its drag and spread by the
    /// frame: the trail's life grows with the frame's time cubed. So the app
    /// ages it at the game's own frame rate setting (`muzzle_view`), where it
    /// lives the few seconds the user sees in the game at 120 (the user,
    /// 2026-10-23: "The jet stream expands and dissipates over a few seconds
    /// in the actual game but lives for a long time in ours").
    #[test]
    fn the_trail_s_life_goes_by_the_frame_rate() {
        let f = smoke_data();
        let mut lives = Vec::new();
        for fps in [60.0f32, 120.0, 300.0] {
            let dt = 1.0 / fps;
            let mut smoke = Smoke::default();
            let mut rng = Rng::new(4);
            for _ in 0..3 {
                smoke.shot(&f, &mut rng);
            }
            // The muzzle swung 0.3 m across in a fifth of a second, then
            // held still, after a second at rest where the trail's head
            // eased in.
            let (mut t, mut frames) = (0.0f32, 0u32);
            let forward = [0.0, 0.0, 1.0];
            while smoke.on() {
                let x = 0.3 + 0.3 * ((t - 1.0) / 0.2).clamp(0.0, 1.0);
                smoke.frame(&f, dt, [x, 1.5, 0.6], forward);
                t += dt;
                frames += 1;
                assert!(frames < 200_000, "it never ended at {fps}");
            }
            lives.push(t);
        }
        // 1.33, 4.18 and 58.2 seconds (measured 2026-10-23).
        assert!((1.0..1.7).contains(&lives[0]), "{lives:?}");
        assert!((3.5..5.0).contains(&lives[1]), "{lives:?}");
        assert!(lives[2] > 40.0, "{lives:?}");
    }

    #[test]
    fn the_third_shot_begins_a_trail_that_rises_spreads_and_ends() {
        let f = smoke_data();
        let mut smoke = Smoke::default();
        let mut rng = Rng::new(4);
        smoke.shot(&f, &mut rng);
        smoke.shot(&f, &mut rng);
        assert!(!smoke.on(), "two shots: not yet");
        smoke.shot(&f, &mut rng);
        assert!(smoke.on(), "the third");
        // The muzzle at rest away from the origin: the head eases in from
        // the origin (as the game's does the first time), leaving points.
        let place = [0.3, 1.5, 0.6];
        let forward = [0.0, 0.0, 1.0];
        for _ in 0..12 {
            smoke.frame(&f, 1.0 / 60.0, place, forward);
        }
        assert!(smoke.points.len() >= 3, "{}", smoke.points.len());
        // Each point worked rises (gravity −1.1) and spreads as it goes.
        let before: Vec<(i32, f32, f32)> = smoke.points.iter().filter(|p| !p.skip).map(|p| (p.number, p.position[1], p.diffusion)).collect();
        assert!(!before.is_empty());
        let x = 0.3;
        for _ in 0..10 {
            smoke.frame(&f, 1.0 / 60.0, [x, 1.5, 0.6], forward);
        }
        let rose = before.iter().any(|&(number, y, d)| smoke.points.iter().any(|p| p.number == number && p.position[1] > y && p.diffusion >= d));
        assert!(rose, "{before:?}");
        let corners = smoke.corners();
        assert_eq!(corners.len(), 4 * (smoke.points.len().max(1) - 1));
        // Left alone, it spreads past its end and goes off.
        let mut frames = 0;
        while smoke.on() {
            smoke.frame(&f, 1.0 / 60.0, [x, 1.5, 0.6], forward);
            frames += 1;
            assert!(frames < 100_000, "it never ended");
        }
    }

    #[test]
    fn a_jet_s_quads_lie_along_its_axis_from_its_place() {
        // A weapon object, a muzzle 0.4 m ahead of it, and the jet turned to
        // point down its barrel: its own down along the weapon's ahead.
        let quarter = [-std::f32::consts::FRAC_1_SQRT_2, 0.0, 0.0, std::f32::consts::FRAC_1_SQRT_2];
        let hierarchy = Hierarchy { nodes: vec![node("root", None, [0.0; 3], [0.0, 0.0, 0.0, 1.0]), node("weapon", Some(0), [0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]), node("muzzle", Some(1), [0.0, 0.0, 0.4], [0.0, 0.0, 0.0, 1.0]), node("jet", Some(2), [0.0, 0.0, 0.05], quarter)] };
        let (vertices, indices) = jet_mesh(&data(), &hierarchy, &mut Rng::new(1));
        assert_eq!((vertices.len(), indices.len()), (32, 48));
        for (k, v) in vertices.iter().enumerate() {
            let along = (k / 4) as f32 * 0.02;
            assert!((v.position[2] - (0.45 + along)).abs() < 1e-5 && v.position[1].abs() < 1e-5, "{v:?}");
            assert!((v.normal[2] - 1.0).abs() < 1e-5);
            assert!((v.color[3] - 38.0 / 255.0).abs() < 1e-6, "the chance as a byte");
        }
        // The share along the jet: 0 for the first, 1 for the last.
        assert_eq!((vertices[0].color[2], vertices[31].color[2]), (0.0, 1.0));
        assert!((vertices[31].tangent[0] - 0.2).abs() < 1e-6);
    }

    #[test]
    fn a_shot_lights_its_time_and_goes_dark() {
        let data = data();
        let mut flash = MuzzleFlash::default();
        assert_eq!(flash.shot_vals(&data, 5.0)[0], 1.0, "nothing before a shot");
        let draws = flash.shot(&data, 5.0, &mut Rng::new(2));
        assert!(draws.random_vals.iter().all(|v| (RANDOM_SCALE..2.0 * RANDOM_SCALE).contains(v)));
        assert!((draws.rot_mx[0] - draws.rot_mx[3]).abs() < 1e-6 && (draws.rot_mx[1] + draws.rot_mx[2]).abs() < 1e-6);
        assert!(flash.shot_vals(&data, 5.0)[0] > 0.99, "dark at the shot");
        assert!(flash.shot_vals(&data, 5.0 + 0.04)[0] < 0.01, "lit a third in");
        assert!((flash.shot_vals(&data, 5.2)[0] - 1.0).abs() < 1e-6, "dark past its time");
    }
}
