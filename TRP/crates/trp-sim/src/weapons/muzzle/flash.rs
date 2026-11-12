//! The muzzle jet mesh, the flash and its light.

use super::*;

/// What `RandomizeMaterial` scales its four draws by.
pub(super) const RANDOM_SCALE: f32 = 878_788.8;

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
pub(super) fn direction(rng: &mut Rng) -> [f32; 2] {
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
    pub(super) end: f32,
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

/// The muzzle's light (firing spec, "The muzzle's light"; `MuzzleLight`,
/// `MuzzleManager.UpdateLights`, `Shot`, `ManualLateUpdate`). No weapon of
/// the install carries one of its own (`probe muzzles`), so each
/// is the one the manager makes: an engine light with its defaults (a point
/// light, no shadows), (1, 0.65, 0.45), 5 cm along the fireport's parent's
/// x from the fireport, its figures the class's own: a reach drawn between
/// 6 and 12 m at each shot, and twice `LightIntensityCurve`, a line from 1
/// at the shot to nothing half the flash's time later, held at nothing past
/// it. A weapon whose jets' chances sum to nothing has none lit.
#[derive(Debug, Clone, Copy)]
pub struct MuzzleLight {
    /// The reach drawn at the last shot (the engine light's own 10 before).
    pub range: f32,
}

/// `MuzzleLight.Range`, `[MinMaxVec2]`'s default.
pub const LIGHT_RANGE: [f32; 2] = [6.0, 12.0];

/// The light's colour (`MuzzleLight.Start`).
pub const LIGHT_COLOR: [f32; 3] = [1.0, 0.65, 0.45];

/// How far along the fireport's parent's x the made light sits, back.
pub const LIGHT_SHIFT: f32 = -0.05;

/// `LightIntensityCurve`'s default (`AnimationCurve.Linear(0, 1, 0.5, 0)`):
/// its last key's time, in the flash's shares.
const LIGHT_CURVE_END: f32 = 0.5;

impl Default for MuzzleLight {
    fn default() -> Self {
        Self { range: 10.0 }
    }
}

impl MuzzleLight {
    /// Whether the weapon's light is ever lit: its jets' `Chance`s sum above
    /// nothing (`_summaryJetsProbability`).
    pub fn lit_by(data: &MuzzleData) -> bool {
        data.jets.iter().map(|jet| jet.chance).sum::<f32>() > 0.0
    }

    /// A shot (`OnShot`): a new reach. `PROVISIONAL`: the engine's draw
    /// (`Random.Range`) is this work's.
    pub fn shot(&mut self, rng: &mut Rng) {
        self.range = rng.range(LIGHT_RANGE[0], LIGHT_RANGE[1]);
    }

    /// The light's intensity now (`SetIntensity`), nothing where it is off
    /// (the curve at or below 0.001): `t` as the flash's (`ManualLateUpdate`).
    pub fn intensity(flash: &MuzzleFlash, data: &MuzzleData, now: f32) -> f32 {
        let t = 1.0 - (flash.end - now) / data.shot_length;
        let curve = if t.is_finite() { (1.0 - t / LIGHT_CURVE_END).clamp(0.0, 1.0) } else { 0.0 };
        if curve > 0.001 { 2.0 * curve } else { 0.0 }
    }
}
