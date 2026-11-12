//! Loading the root motion table and asking it a state's velocity, length and weights.

use super::*;

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
