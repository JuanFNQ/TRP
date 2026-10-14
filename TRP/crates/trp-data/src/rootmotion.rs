//! The player's root motion table.
//!
//! The game precomputes, for every state of the body animator, how fast the
//! character moves as a function of the animator's parameters. A state is a
//! tree: clips carry a velocity, and blend trees mix their children by
//! parameters. This is what the game uses for characters that do not run a
//! full animator, and it is where walking and sprinting speeds come from.
//!
//! Format and evaluation are described in `docs/spec/movement.md`.

use std::collections::HashMap;

use anyhow::{bail, Context, Result};

use crate::unity::anim::path_hash;
use crate::unity::extract::script_class;
use crate::unity::{class, Assets, Value};

pub const TABLE_BUNDLE: &str = "assets/content/characters/rootmotiontable/playerrootmotiontable.asset.bundle";

/// Weights below this are treated as zero, as the game does.
const SKIP_WEIGHT: f32 = 0.01;

/// Range and grid spacing of one animator parameter.
#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

impl Parameter {
    fn steps(&self) -> usize {
        ((self.max - self.min).abs() / self.step).ceil() as usize + 1
    }

    /// Grid cell and position within it for a value.
    fn locate(&self, value: f32) -> (usize, usize, f32) {
        let value = value.clamp(self.min, self.max);
        let total = self.steps();
        if total == 1 {
            return (0, 0, 0.0);
        }
        let mut a = ((value - self.min) / self.step).floor() as usize;
        if a >= total {
            a = total - 1;
        }
        let t = (value - (self.min + a as f32 * self.step)) / self.step;
        let b = if a + 1 < total { a + 1 } else { a };
        (a, b, t.clamp(0.0, 1.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendType {
    Simple1D,
    FreeformDirectional2D,
    FreeformCartesian2D,
    SimpleDirectional2D,
    Direct,
}

#[derive(Debug, Clone)]
pub enum Node {
    Clip {
        name: String,
        looping: bool,
        /// Seconds.
        length: f32,
        time_scale: f32,
        /// Metres per second in the character's frame: x right, y up, z forward.
        velocity: [f32; 3],
        /// The game reads this clip's motion from curves instead of the
        /// velocity. Used for one-off moves such as vaults and kicks.
        uses_curves: bool,
    },
    Tree {
        name: String,
        blend: BlendType,
        param_x: u32,
        param_y: u32,
        thresholds: Vec<f32>,
        /// Where each child sits in a two-dimensional blend.
        positions: Vec<[f32; 2]>,
        direct_params: Vec<u32>,
        /// Precomputed child weights over the grid of both parameters,
        /// indexed `[(x * steps_y + y) * children + child]`.
        steps_x: usize,
        steps_y: usize,
        weights: Vec<f32>,
        children: Vec<Node>,
    },
}

impl Node {
    pub fn name(&self) -> &str {
        match self {
            Node::Clip { name, .. } | Node::Tree { name, .. } => name,
        }
    }
}

/// Values of the animator's float parameters, by name hash.
pub trait Parameters {
    fn get(&self, hash: u32) -> f32;
}

impl Parameters for HashMap<u32, f32> {
    fn get(&self, hash: u32) -> f32 {
        self.get(&hash).copied().unwrap_or(0.0)
    }
}

#[derive(Clone)]
pub struct RootMotionTable {
    pub parameters: Vec<Parameter>,
    /// States by the hash of their full path, such as `Base Layer.Idle`.
    pub states: HashMap<u32, (String, Node)>,
    by_hash: HashMap<u32, usize>,
}

impl RootMotionTable {
    pub fn load(assets: &mut Assets) -> Result<Self> {
        let files = assets.load(TABLE_BUNDLE)?;
        let table_obj = files
            .iter()
            .flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR))
            .find(|&obj| script_class(assets, obj).as_deref() == Some("RootMotionBlendTable"))
            .context("bundle has no RootMotionBlendTable")?;
        let table = assets.read(table_obj)?;
        let mut parameters = Vec::new();
        for p in table.req("Parameters")?.as_array().unwrap_or(&[]) {
            parameters.push(Parameter {
                name: p.req("ParamName")?.as_str().unwrap_or("").to_owned(),
                min: p.req("MinValue")?.as_f32().unwrap_or(0.0),
                max: p.req("MaxValue")?.as_f32().unwrap_or(0.0),
                step: p.req("StepDensity")?.as_f32().unwrap_or(1.0),
            });
        }
        let data_obj = table
            .req("GeneratedData")?
            .as_pptr()
            .and_then(|p| assets.resolve_loaded(table_obj.file, p))
            .context("table has no generated data")?;
        let data = assets.read(data_obj)?;
        let bytes = text_asset_bytes(&data)?;
        Self::parse(parameters, bytes)
    }

    pub fn parse(parameters: Vec<Parameter>, bytes: &[u8]) -> Result<Self> {
        let mut r = Bin { data: bytes, pos: 0 };
        let mut states = HashMap::new();
        while r.pos < bytes.len() {
            let name = r.string()?;
            let key = r.u32()?;
            let node = read_node(&mut r).with_context(|| format!("state {name}"))?;
            states.insert(key, (name, node));
        }
        Ok(Self::from_states(parameters, states))
    }

    pub fn from_states(parameters: Vec<Parameter>, states: HashMap<u32, (String, Node)>) -> Self {
        let by_hash = parameters.iter().enumerate().map(|(i, p)| (path_hash(&p.name), i)).collect();
        Self { parameters, states, by_hash }
    }

    /// Key of the state with this name, if exactly one state has it.
    pub fn state_named(&self, name: &str) -> Option<u32> {
        let mut found = self.states.iter().filter(|(_, (n, _))| n == name).map(|(k, _)| *k);
        let first = found.next()?;
        found.next().is_none().then_some(first)
    }

    /// Velocity of a state in metres per second, in the character's frame.
    ///
    /// `normalized_time` is how far the state has played, in loops: clips
    /// that do not loop stop moving once it reaches 1.
    pub fn velocity(&self, state: u32, normalized_time: f32, params: &dyn Parameters) -> [f32; 3] {
        match self.states.get(&state) {
            Some((_, node)) => self.node_velocity(node, normalized_time, params),
            None => [0.0; 3],
        }
    }

    fn node_velocity(&self, node: &Node, normalized_time: f32, params: &dyn Parameters) -> [f32; 3] {
        match node {
            Node::Clip { looping, velocity, uses_curves, .. } => {
                if *uses_curves || (!*looping && normalized_time >= 1.0) {
                    [0.0; 3]
                } else {
                    *velocity
                }
            }
            Node::Tree { children, .. } => {
                let weights = self.weights(node, params);
                let mut out = [0.0f32; 3];
                for (child, weight) in children.iter().zip(weights) {
                    if weight < SKIP_WEIGHT {
                        continue;
                    }
                    let v = self.node_velocity(child, normalized_time, params);
                    for i in 0..3 {
                        out[i] += v[i] * weight;
                    }
                }
                out
            }
        }
    }

    /// Length in seconds of one cycle of a state's blended animation: the
    /// clips' lengths weighted like their motion.
    pub fn cycle_length(&self, state: u32, params: &dyn Parameters) -> f32 {
        match self.states.get(&state) {
            Some((_, node)) => self.node_length(node, params),
            None => 0.0,
        }
    }

    fn node_length(&self, node: &Node, params: &dyn Parameters) -> f32 {
        match node {
            Node::Clip { length, .. } => *length,
            Node::Tree { children, .. } => children
                .iter()
                .zip(self.weights(node, params))
                .filter(|(_, w)| w.abs() >= SKIP_WEIGHT)
                .map(|(child, w)| self.node_length(child, params) * w)
                .sum(),
        }
    }

    /// Blend weights of a tree's children for the given parameters.
    pub fn weights(&self, node: &Node, params: &dyn Parameters) -> Vec<f32> {
        let Node::Tree {
            blend, param_x, param_y, thresholds, positions, direct_params, steps_x, steps_y, weights, children, ..
        } = node
        else {
            return Vec::new();
        };
        let n = children.len();
        match blend {
            BlendType::Simple1D => {
                let value = params.get(*param_x);
                (0..n).map(|i| weight_1d(thresholds, i, value)).collect()
            }
            BlendType::Direct => direct_params.iter().take(n).map(|h| params.get(*h)).collect(),
            // Computed rather than looked up: the stored grid only spans
            // each parameter's listed range, and the local player's
            // parameters go beyond it (sprint speed above 1).
            BlendType::FreeformCartesian2D if positions.len() == n => {
                freeform_cartesian(positions, [params.get(*param_x), params.get(*param_y)])
            }
            BlendType::FreeformDirectional2D if positions.len() == n => {
                freeform_directional(positions, [params.get(*param_x), params.get(*param_y)])
            }
            BlendType::SimpleDirectional2D if positions.len() == n => {
                simple_directional(positions, [params.get(*param_x), params.get(*param_y)])
            }
            _ => {
                let (Some(px), Some(py)) = (self.parameter(*param_x), self.parameter(*param_y)) else {
                    return vec![0.0; n];
                };
                if *steps_x == 0 || *steps_y == 0 {
                    return vec![0.0; n];
                }
                let (xa, xb, tx) = px.locate(params.get(*param_x));
                let (ya, yb, ty) = py.locate(params.get(*param_y));
                let at = |x: usize, y: usize, c: usize| {
                    weights[(x.min(steps_x - 1) * steps_y + y.min(steps_y - 1)) * n + c]
                };
                (0..n)
                    .map(|c| {
                        let a = at(xa, ya, c) + (at(xa, yb, c) - at(xa, ya, c)) * ty;
                        let b = at(xb, ya, c) + (at(xb, yb, c) - at(xb, ya, c)) * ty;
                        a + (b - a) * tx
                    })
                    .collect()
            }
        }
    }

    fn parameter(&self, hash: u32) -> Option<&Parameter> {
        self.by_hash.get(&hash).map(|&i| &self.parameters[i])
    }
}

impl RootMotionTable {
    /// Largest difference between the stored weight grid of a
    /// two-parameter tree and the weights computed directly, over the
    /// whole grid. A check that the direct computation matches the game's.
    pub fn grid_mismatch(&self, node: &Node) -> Option<f32> {
        self.grid_mismatch_in(node, false)
    }

    /// The same, optionally only over the part of the grid that lies among
    /// the tree's children (within the box that holds them all).
    pub fn grid_mismatch_in(&self, node: &Node, among_children: bool) -> Option<f32> {
        let Node::Tree { blend, param_x, param_y, positions, steps_x, steps_y, weights, children, .. } = node else {
            return None;
        };
        let compute: fn(&[[f32; 2]], [f32; 2]) -> Vec<f32> = match blend {
            BlendType::FreeformCartesian2D => freeform_cartesian,
            BlendType::FreeformDirectional2D => freeform_directional,
            BlendType::SimpleDirectional2D => simple_directional,
            _ => return None,
        };
        let (px, py) = (self.parameter(*param_x)?, self.parameter(*param_y)?);
        let n = children.len();
        if positions.len() != n || weights.len() < steps_x * steps_y * n {
            return None;
        }
        let bound = |axis: usize, pick: fn(f32, f32) -> f32, start: f32| positions.iter().map(|p| p[axis]).fold(start, pick);
        let low = [bound(0, f32::min, f32::MAX), bound(1, f32::min, f32::MAX)];
        let high = [bound(0, f32::max, f32::MIN), bound(1, f32::max, f32::MIN)];
        let mut worst = 0.0f32;
        for x in 0..*steps_x {
            for y in 0..*steps_y {
                let at = [px.min + x as f32 * px.step, py.min + y as f32 * py.step];
                if among_children && (0..2).any(|k| at[k] < low[k] || at[k] > high[k]) {
                    continue;
                }
                let direct = compute(positions, at);
                for c in 0..n {
                    worst = worst.max((direct[c] - weights[(x * steps_y + y) * n + c]).abs());
                }
            }
        }
        Some(worst)
    }
}

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

fn text_asset_bytes(value: &Value) -> Result<&[u8]> {
    match value.req("m_Script")? {
        Value::Bytes(b) => Ok(b),
        // Valid UTF-8 is kept as text, byte for byte.
        Value::Str(s) => Ok(s.as_bytes()),
        _ => bail!("table data has an unexpected type"),
    }
}

fn read_node(r: &mut Bin) -> Result<Node> {
    let kind = r.u8()?;
    let name = r.string()?;
    let looping = r.u8()? != 0;
    let mut node = match kind {
        1 => {
            let _name_again = r.string()?;
            let time_scale = r.f32()?;
            let length = r.f32()?;
            let _clip_index = r.i32()?;
            let velocity = [r.f32()?, r.f32()?, r.f32()?];
            let stores_rotation = r.u8()? != 0;
            let uses_curves = r.u8()? != 0;
            if stores_rotation || uses_curves {
                let curves = r.i16()?;
                for _ in 0..curves {
                    let keys = r.i32()? as usize;
                    // Wrap modes, then per key six floats and a weighted mode.
                    r.skip(4 + keys * (6 * 4 + 2))?;
                }
            }
            Node::Clip { name, looping, length, time_scale, velocity, uses_curves }
        }
        2 => {
            // Zero is the game's "none" and does not occur in the table.
            let blend = match r.u8()? {
                1 => BlendType::Simple1D,
                2 => BlendType::FreeformDirectional2D,
                3 => BlendType::FreeformCartesian2D,
                4 => BlendType::SimpleDirectional2D,
                5 => BlendType::Direct,
                other => bail!("unknown blend type {other}"),
            };
            let param_x = r.u32()?;
            let param_y = r.u32()?;
            let _child_count = r.i32()?;
            let thresholds = (0..r.i32()?).map(|_| r.f32()).collect::<Result<Vec<_>>>()?;
            let positions = (0..r.i32()?).map(|_| Ok([r.f32()?, r.f32()?])).collect::<Result<Vec<_>>>()?;
            let direct_params = (0..r.i32()?).map(|_| r.u32()).collect::<Result<Vec<_>>>()?;
            let steps_x = r.i32()? as usize;
            let steps_y = r.i32()? as usize;
            let grid_children = r.i32()? as usize;
            let _stores_rotation = r.u8()?;
            let count = steps_x * steps_y * grid_children;
            if count * 4 > r.data.len() - r.pos {
                bail!("weight grid of {name} runs past the end of the table");
            }
            let weights = (0..count).map(|_| r.f32()).collect::<Result<Vec<_>>>()?;
            Node::Tree {
                name,
                blend,
                param_x,
                param_y,
                thresholds,
                positions,
                direct_params,
                steps_x,
                steps_y,
                weights,
                children: Vec::new(),
            }
        }
        other => bail!("unknown node type {other}"),
    };
    let child_count = r.i32()?;
    let mut read_children = Vec::new();
    for _ in 0..child_count {
        read_children.push(read_node(r)?);
    }
    if let Node::Tree { children, .. } = &mut node {
        *children = read_children;
    }
    Ok(node)
}

/// Little-endian reader for the .NET `BinaryWriter` layout.
struct Bin<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bin<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        if n > self.data.len() - self.pos {
            bail!("root motion table is truncated at {}", self.pos);
        }
        let out = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    fn skip(&mut self, n: usize) -> Result<()> {
        self.take(n).map(|_| ())
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    /// Length-prefixed string; the length is a 7-bit variable-length integer.
    fn string(&mut self) -> Result<String> {
        let mut len = 0usize;
        let mut shift = 0;
        loop {
            let byte = self.u8()?;
            len |= ((byte & 0x7f) as usize) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
        }
        Ok(String::from_utf8_lossy(self.take(len)?).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_dimensional_weights_form_a_partition() {
        let thresholds = [0.0, 0.5, 1.0];
        for blend in [-1.0, 0.0, 0.2, 0.5, 0.9, 1.0, 2.0] {
            let w: Vec<f32> = (0..3).map(|i| weight_1d(&thresholds, i, blend)).collect();
            assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-5, "{blend}: {w:?}");
        }
        assert_eq!(weight_1d(&thresholds, 1, 0.25), 0.5);
        assert_eq!(weight_1d(&thresholds, 0, 0.25), 0.5);
        assert_eq!(weight_1d(&thresholds, 2, 0.25), 0.0);
    }

    #[test]
    fn parameter_grid_lookup() {
        let p = Parameter { name: "Speed".into(), min: 0.0, max: 1.0, step: 0.1 };
        assert_eq!(p.steps(), 11);
        let (a, b, t) = p.locate(0.25);
        assert_eq!((a, b), (2, 3));
        assert!((t - 0.5).abs() < 1e-4);
        assert_eq!(p.locate(5.0).0, 10);
    }
}
